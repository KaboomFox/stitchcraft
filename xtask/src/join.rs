//! `cargo xtask compat join [--nested] <vectorcraft>`: makes this repository, copied into a VectorCraft
//! checkout as a folder, part of VectorCraft's build (`docs/src/design/adr/0011-movable-into-vectorcraft.md`).
//!
//! - `--nested` keeps this folder its own Cargo workspace and adds it to VectorCraft's `exclude` list, so
//!   VectorCraft's crates can depend on StitchCraft's by path while each project keeps its lockfile and
//!   its gates. (Without the `exclude` entry Cargo resolves our crates' inherited settings against
//!   VectorCraft's workspace, which does not define them.)
//! - Without it, StitchCraft's crates join VectorCraft's workspace: they are added to its `members`, our
//!   `[workspace.dependencies]` are merged into VectorCraft's (reusing VectorCraft's entries where they
//!   satisfy ours), and our root `Cargo.toml` and `Cargo.lock` are removed, leaving one workspace and one
//!   lockfile. The tooling keeps working from the folder (`cargo xtask ci`), on StitchCraft's packages.
//!
//! VectorCraft's `Cargo.toml` is edited in place with `toml_edit`, so its comments and layout survive and
//! the diff a maintainer reviews is only what joining adds. The command refuses, without writing
//! anything, when the two workspaces cannot be merged: a dependency in an incompatible version, a
//! workspace key our crates inherit that VectorCraft does not define, or a package name clash.

use std::collections::BTreeSet;
use std::path::{Component, Path};

use toml_edit::{Array, DocumentMut, InlineTable, Item, Table, Value};

use crate::layers::{self, Class, TABLE};
use crate::util;

/// The comment above the dependencies joining adds to VectorCraft's manifest.
const MARKER: &str = "\n# StitchCraft (machine embroidery), joined by `cargo xtask compat join`: see\n# stitchcraft/docs/src/design/adr/0011-movable-into-vectorcraft.md\n";

/// `cargo xtask compat join [--nested] <vectorcraft>`.
pub fn run(args: &[String]) -> Result<(), String> {
    let nested = args.iter().any(|a| a == "--nested");
    let target = args.iter().find(|a| !a.starts_with("--")).ok_or("usage: cargo xtask compat join [--nested] <vectorcraft checkout>")?;
    let folder = std::fs::canonicalize(util::root()).map_err(|e| format!("{}: {e}", util::root().display()))?;
    let vectorcraft = std::fs::canonicalize(target).map_err(|e| format!("{target}: {e}"))?;
    let prefix = prefix(&folder, &vectorcraft)?;
    let ours_path = folder.join("Cargo.toml");
    let theirs_path = vectorcraft.join("Cargo.toml");
    let ours = util::read(&ours_path)?;
    let theirs = util::read(&theirs_path)?;
    if !ours.lines().any(|l| l.trim() == "[workspace]") {
        return Err(format!("{} has no [workspace]: this folder has already joined a workspace", ours_path.display()));
    }
    if nested {
        let nested_manifest = nest(&theirs, &prefix)?;
        write(&theirs_path, &nested_manifest)?;
        println!("join: VectorCraft's workspace excludes `{prefix}`; its crates may depend on StitchCraft's by path.");
        println!("StitchCraft keeps its own workspace, lockfile and gates: `cd {prefix} && cargo xtask ci`.");
        return Ok(());
    }
    // The rows for VectorCraft's layering table, while this folder is still a workspace of its own.
    let rows = layering_rows(&layers::ranks(&layers::model()?));
    let (merged, report) = merge(&theirs, &ours, &prefix)?;
    write(&theirs_path, &merged)?;
    for file in ["Cargo.toml", "Cargo.lock"] {
        let path = folder.join(file);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    // Cargo adds StitchCraft's dependencies to VectorCraft's lockfile while it resolves the workspace.
    let resolved =
        util::cargo().args(["metadata", "--format-version", "1"]).current_dir(&vectorcraft).output().map_err(|e| format!("cargo metadata: {e}"))?;
    if !resolved.status.success() {
        return Err(format!("the joined workspace does not resolve: {}", String::from_utf8_lossy(&resolved.stderr).trim()));
    }
    println!("join: StitchCraft's crates are members of VectorCraft's workspace.");
    for line in report {
        println!("  {line}");
    }
    println!("\nRows for VectorCraft's layering table (xtask/src/layers.rs, TABLE):\n{rows}");
    println!(
        "Next: run StitchCraft's gates from its folder (`cd {prefix} && cargo xtask ci`). GitHub reads workflows only at the \
         repository root: StitchCraft's are in {prefix}/.github/workflows and run `cargo xtask …` from the folder."
    );
    Ok(())
}

/// Where `folder` is inside `vectorcraft`, with `/` separators.
fn prefix(folder: &Path, vectorcraft: &Path) -> Result<String, String> {
    let inside = folder.strip_prefix(vectorcraft).ok().filter(|p| p.components().next().is_some()).ok_or_else(|| {
        format!(
            "{} is not inside {}: copy this repository into the VectorCraft checkout first (for example \
             `git subtree add --prefix=stitchcraft https://github.com/KaboomFox/stitchcraft main`)",
            folder.display(),
            vectorcraft.display()
        )
    })?;
    let parts: Vec<String> = inside
        .components()
        .map(|c| match c {
            Component::Normal(part) => Ok(part.to_string_lossy().into_owned()),
            _ => Err(format!("unexpected path component in {}", inside.display())),
        })
        .collect::<Result<_, _>>()?;
    Ok(parts.join("/"))
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn parse(text: &str, what: &str) -> Result<DocumentMut, String> {
    text.parse::<DocumentMut>().map_err(|e| format!("{what}: {e}"))
}

fn workspace_mut<'a>(doc: &'a mut DocumentMut, what: &str) -> Result<&'a mut Table, String> {
    doc.get_mut("workspace").and_then(Item::as_table_mut).ok_or_else(|| format!("{what} has no [workspace] table"))
}

/// The array `key` of `table`, created empty when missing.
fn array_mut<'a>(table: &'a mut Table, key: &str) -> Result<&'a mut Array, String> {
    if !table.contains_key(key) {
        table.insert(key, Item::Value(Value::Array(Array::new())));
    }
    table.get_mut(key).and_then(Item::as_array_mut).ok_or_else(|| format!("`workspace.{key}` is not an array"))
}

fn strings(item: Option<&Item>) -> Vec<String> {
    item.and_then(Item::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// VectorCraft's manifest with `prefix` added to its workspace's `exclude` list.
pub fn nest(theirs: &str, prefix: &str) -> Result<String, String> {
    let mut doc = parse(theirs, "VectorCraft's Cargo.toml")?;
    let exclude = array_mut(workspace_mut(&mut doc, "VectorCraft's Cargo.toml")?, "exclude")?;
    if !exclude.iter().any(|v| v.as_str() == Some(prefix)) {
        exclude.push(prefix);
    }
    Ok(doc.to_string())
}

/// VectorCraft's manifest with this workspace (`ours`, in folder `prefix`) joined to it, and a report of
/// what was added; an error naming every reason the two cannot be merged.
pub fn merge(theirs: &str, ours: &str, prefix: &str) -> Result<(String, Vec<String>), String> {
    let mut doc = parse(theirs, "VectorCraft's Cargo.toml")?;
    let our_doc = parse(ours, "StitchCraft's Cargo.toml")?;
    let our_workspace = our_doc.get("workspace").and_then(Item::as_table).ok_or("StitchCraft's Cargo.toml has no [workspace] table")?;
    let workspace = workspace_mut(&mut doc, "VectorCraft's Cargo.toml")?;
    let mut problems = Vec::new();
    let mut report = Vec::new();

    // What our crates inherit must exist there.
    let their_package: BTreeSet<String> =
        workspace.get("package").and_then(Item::as_table).map(|t| t.iter().map(|(k, _)| k.to_string()).collect()).unwrap_or_default();
    for (key, _) in our_workspace.get("package").and_then(Item::as_table).into_iter().flat_map(Table::iter) {
        if !their_package.contains(key) {
            problems.push(format!("`workspace.package.{key}`: StitchCraft's crates inherit it, but VectorCraft's workspace does not define it"));
        }
    }
    if our_workspace.contains_key("lints") && !workspace.contains_key("lints") {
        problems.push("`workspace.lints`: StitchCraft's crates inherit lints, but VectorCraft's workspace defines none".to_string());
    }

    // Members, and the nested workspaces inside our folder (fuzzing, the plug-in contract tests).
    let members: Vec<String> = strings(our_workspace.get("members")).iter().map(|m| format!("{prefix}/{m}")).collect();
    let excluded: Vec<String> = strings(our_workspace.get("exclude")).iter().map(|m| format!("{prefix}/{m}")).collect();
    let their_members = array_mut(workspace, "members")?;
    for member in &members {
        if !their_members.iter().any(|v| v.as_str() == Some(member)) {
            their_members.push(member.as_str());
        }
    }
    let their_exclude = array_mut(workspace, "exclude")?;
    // A nested join excluded the whole folder; joining replaces that.
    their_exclude.retain(|v| v.as_str() != Some(prefix));
    for path in &excluded {
        if !their_exclude.iter().any(|v| v.as_str() == Some(path)) {
            their_exclude.push(path.as_str());
        }
    }
    if their_exclude.is_empty() {
        workspace.remove("exclude");
    }
    report.push(format!("members: {}", members.join(", ")));

    // Dependencies.
    let our_deps = our_workspace.get("dependencies").and_then(Item::as_table).ok_or("StitchCraft's Cargo.toml has no [workspace.dependencies]")?;
    if !workspace.contains_key("dependencies") {
        workspace.insert("dependencies", Item::Table(Table::new()));
    }
    let deps = workspace.get_mut("dependencies").and_then(Item::as_table_mut).ok_or("`workspace.dependencies` is not a table")?;
    let (mut added, mut reused) = (Vec::new(), Vec::new());
    for (name, ours) in our_deps.iter() {
        let path = ours.as_table_like().and_then(|t| t.get("path")).and_then(Item::as_str);
        match (path, deps.get_mut(name)) {
            (Some(_), Some(_)) => problems.push(format!("`{name}`: VectorCraft already has a dependency of this name")),
            (Some(path), None) => {
                let mut table = InlineTable::new();
                table.insert("path", format!("{prefix}/{path}").into());
                deps.insert(name, Item::Value(Value::InlineTable(table)));
                added.push(name.to_string());
            }
            (None, None) => {
                deps.insert(name, ours.clone());
                added.push(name.to_string());
            }
            (None, Some(theirs)) => match reconcile(name, ours, theirs) {
                Ok(note) => reused.push(note),
                Err(problem) => problems.push(problem),
            },
        }
    }
    if let Some(first) = added.first()
        && let Some(mut key) = deps.key_mut(first)
    {
        key.leaf_decor_mut().set_prefix(MARKER);
    }
    report.push(format!("dependencies added: {}", added.join(", ")));
    report.push(format!("dependencies VectorCraft already had: {}", reused.join(", ")));
    report.push("StitchCraft's [profile.*] settings are dropped: VectorCraft's apply to the whole workspace".to_string());
    if problems.is_empty() { Ok((doc.to_string(), report)) } else { Err(format!("cannot join:\n  {}", problems.join("\n  "))) }
}

/// Makes VectorCraft's declaration of `name` serve ours too: same compatibility class, default features
/// where we use them, and our features added. Returns a note for the report.
fn reconcile(name: &str, ours: &Item, theirs: &mut Item) -> Result<String, String> {
    let version =
        |item: &Item| item.as_str().or_else(|| item.as_table_like().and_then(|t| t.get("version")).and_then(Item::as_str)).map(str::to_string);
    let defaults = |item: &Item| item.as_table_like().and_then(|t| t.get("default-features")).and_then(Item::as_bool).unwrap_or(true);
    let features = |item: &Item| strings(item.as_table_like().and_then(|t| t.get("features")));
    let (Some(our_version), Some(their_version)) = (version(ours), version(theirs)) else {
        return Err(format!("`{name}`: one of the declarations has no version"));
    };
    if class(&our_version) != class(&their_version) {
        return Err(format!("`{name}`: StitchCraft needs \"{our_version}\", VectorCraft has \"{their_version}\""));
    }
    if defaults(ours) && !defaults(theirs) {
        return Err(format!("`{name}`: StitchCraft needs its default features, which VectorCraft turns off"));
    }
    let have = features(theirs);
    let missing: Vec<String> = features(ours).into_iter().filter(|f| !have.contains(f)).collect();
    if missing.is_empty() {
        return Ok(name.to_string());
    }
    if theirs.is_str() {
        let mut table = InlineTable::new();
        table.insert("version", their_version.as_str().into());
        *theirs = Item::Value(Value::InlineTable(table));
    }
    let table = theirs.as_table_like_mut().ok_or_else(|| format!("`{name}`: VectorCraft's declaration is neither a string nor a table"))?;
    if table.get("features").is_none() {
        table.insert("features", Item::Value(Value::Array(Array::new())));
    }
    let list = table.get_mut("features").and_then(Item::as_array_mut).ok_or_else(|| format!("`{name}`: `features` is not an array"))?;
    for feature in &missing {
        list.push(feature.as_str());
    }
    Ok(format!("{name} (features added: {})", missing.join(", ")))
}

/// The compatibility class of a Cargo version requirement: `1.5` → `1`, `0.11.4` → `0.11`, `0.0.3` →
/// `0.0.3`. Two caret requirements of one class always allow a common version, and requirements of
/// different classes never do. Requirements with other operators are compared as written.
fn class(requirement: &str) -> String {
    let requirement = requirement.trim();
    let bare = requirement.strip_prefix('^').unwrap_or(requirement);
    if bare.is_empty() || !bare.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return requirement.to_string();
    }
    let parts: Vec<&str> = bare.split('.').collect();
    let significant = parts.iter().position(|p| *p != "0").map_or(parts.len(), |i| i + 1);
    parts.get(..significant).map_or_else(|| bare.to_string(), |p| p.join("."))
}

/// Rows for VectorCraft's layering table: libraries at their rank (VectorCraft's rule is "only strictly
/// lower layers"), the testkit as its testkit, apps and tooling exempt.
fn layering_rows(ranks: &std::collections::BTreeMap<String, u8>) -> String {
    let mut rows = String::new();
    for (name, class) in TABLE {
        let row = match class {
            Class::Layer(_) => format!("Class::Layer({})", ranks.get(*name).copied().unwrap_or(0)),
            Class::Testkit => "Class::Testkit".to_string(),
            Class::App | Class::Tool => "Class::Exempt".to_string(),
        };
        rows.push_str(&format!("    (\"{name}\", {row}),\n"));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The parts of VectorCraft's manifest joining touches, with a comment that must survive.
    const THEIRS: &str = r#"[workspace]
resolver = "3"
members = ["crates/*", "apps/*", "xtask"]

[workspace.package]
version = "0.7.0"
edition = "2024"
license = "MIT OR Apache-2.0"
rust-version = "1.95"
repository = "https://github.com/storytold/vectorcraft"

[workspace.dependencies]
# internal
vectorcraft-geom = { path = "crates/geom" }
thiserror = "2"
serde = { version = "1", features = ["derive", "rc"] }
serde_json = { version = "1", features = ["float_roundtrip"] }

[workspace.lints.clippy]
unwrap_used = "deny"
"#;

    const OURS: &str = r#"[workspace]
resolver = "3"
members = ["crates/*", "apps/*", "xtask"]
exclude = ["compat/vc-contract"]

[workspace.package]
version = "0.0.1"
edition = "2024"
license = "MIT OR Apache-2.0"
rust-version = "1.95"
repository = "https://github.com/KaboomFox/stitchcraft"

[workspace.dependencies]
stitchcraft-core = { path = "crates/stitchcraft-core" }
libm = "0.2"
thiserror = "2"
tiny-skia = { version = "0.11", default-features = false, features = ["std"] }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order"] }

[workspace.lints.rust]
missing_docs = "deny"

[profile.release]
lto = "thin"
"#;

    #[test]
    fn joining_adds_members_and_dependencies_and_keeps_vectorcrafts_text() {
        let (merged, report) = merge(THEIRS, OURS, "stitchcraft").unwrap();
        assert!(merged.contains(r#"members = ["crates/*", "apps/*", "xtask", "stitchcraft/crates/*", "stitchcraft/apps/*", "stitchcraft/xtask"]"#));
        assert!(merged.contains(r#"exclude = ["stitchcraft/compat/vc-contract"]"#));
        assert!(merged.contains("# internal\nvectorcraft-geom"), "VectorCraft's comments survive:\n{merged}");
        assert!(merged.contains(&format!("{MARKER}stitchcraft-core = {{ path = \"stitchcraft/crates/stitchcraft-core\" }}")), "{merged}");
        assert!(merged.contains(r#"tiny-skia = { version = "0.11", default-features = false, features = ["std"] }"#));
        assert!(merged.contains(r#"serde_json = { version = "1", features = ["float_roundtrip", "preserve_order"] }"#), "{merged}");
        assert!(merged.contains(r#"serde = { version = "1", features = ["derive", "rc"] }"#));
        assert!(!merged.contains("lto"), "our profiles are dropped");
        assert_eq!(report.get(1).map(String::as_str), Some("dependencies added: stitchcraft-core, libm, tiny-skia"));
        assert_eq!(
            report.get(2).map(String::as_str),
            Some("dependencies VectorCraft already had: thiserror, serde, serde_json (features added: preserve_order)")
        );
        let joined: DocumentMut = merged.parse().unwrap();
        assert!(joined.get("workspace").is_some());
    }

    #[test]
    fn joining_after_nesting_replaces_the_exclusion() {
        let nested = nest(THEIRS, "stitchcraft").unwrap();
        assert!(nested.contains(r#"exclude = ["stitchcraft"]"#));
        assert_eq!(nest(&nested, "stitchcraft").unwrap(), nested, "nesting twice changes nothing");
        let (merged, _) = merge(&nested, OURS, "stitchcraft").unwrap();
        assert!(merged.contains(r#"exclude = ["stitchcraft/compat/vc-contract"]"#), "{merged}");
    }

    #[test]
    fn workspaces_that_cannot_merge_are_refused_with_every_reason() {
        let theirs = THEIRS.replace("thiserror = \"2\"", "thiserror = \"1\"").replace("rust-version = \"1.95\"\n", "").replace(
            "vectorcraft-geom = { path = \"crates/geom\" }",
            "libm = { version = \"0.2\", default-features = false }\nstitchcraft-core = \"9\"",
        );
        let problems = merge(&theirs, OURS, "stitchcraft").unwrap_err();
        assert_eq!(
            problems,
            "cannot join:\n  \
             `workspace.package.rust-version`: StitchCraft's crates inherit it, but VectorCraft's workspace does not define it\n  \
             `stitchcraft-core`: VectorCraft already has a dependency of this name\n  \
             `libm`: StitchCraft needs its default features, which VectorCraft turns off\n  \
             `thiserror`: StitchCraft needs \"2\", VectorCraft has \"1\""
        );
    }

    #[test]
    fn requirements_compare_by_compatibility_class() {
        assert_eq!(class("1.5"), "1");
        assert_eq!(class("^1"), "1");
        assert_eq!(class("0.11.4"), "0.11");
        assert_eq!(class("0.11"), "0.11");
        assert_eq!(class("0.0.3"), "0.0.3");
        assert_eq!(class("=1.2.3"), "=1.2.3");
    }

    #[test]
    fn the_folder_must_be_inside_the_checkout() {
        assert_eq!(prefix(Path::new("/vc/stitchcraft"), Path::new("/vc")).unwrap(), "stitchcraft");
        assert_eq!(prefix(Path::new("/vc/third/stitchcraft"), Path::new("/vc")).unwrap(), "third/stitchcraft");
        assert!(prefix(Path::new("/elsewhere"), Path::new("/vc")).is_err());
        assert!(prefix(Path::new("/vc"), Path::new("/vc")).is_err());
    }

    #[test]
    fn layering_rows_cover_the_table() {
        let ranks = [("stitchcraft-core".to_string(), 0), ("stitchcraft-params".to_string(), 1)].into();
        let rows = layering_rows(&ranks);
        assert!(rows.contains("(\"stitchcraft-params\", Class::Layer(1)),"));
        assert!(rows.contains("(\"stitchcraft-testkit\", Class::Testkit),"));
        assert!(rows.contains("(\"stitchcraft-xtask\", Class::Exempt),"));
        assert_eq!(rows.lines().count(), TABLE.len());
    }
}
