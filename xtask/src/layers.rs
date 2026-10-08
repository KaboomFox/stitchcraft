//! Crate layering rules (`docs/src/design/architecture.md`).
//!
//! The rule engine works on a small model so it can be unit-tested; [`model_from_metadata`] builds that
//! model from `cargo metadata`. The table is append-only: a new crate gets a row, existing rows never
//! move down a layer. (Same design as VectorCraft's `xtask/src/layers.rs`.)

use serde_json::Value;

use crate::util::{self, Findings};

/// Where a workspace crate sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// A library at layer N; it may depend only on lower layers (and listed same-layer crates).
    Layer(u8),
    /// Test support: may depend on any library; others may use it only as a dev-dependency.
    Testkit,
    /// A shipped binary or plug-in: may depend on any library, never on another app.
    App,
    /// Repository tooling: exempt.
    Tool,
}

/// The layering table.
pub const TABLE: &[(&str, Class)] = &[
    ("stitchcraft-core", Class::Layer(0)),
    ("stitchcraft-params", Class::Layer(0)),
    ("stitchcraft-plan", Class::Layer(1)),
    ("stitchcraft-engine", Class::Layer(2)),
    ("stitchcraft-formats", Class::Layer(2)),
    ("stitchcraft-render", Class::Layer(2)),
    ("stitchcraft-svg", Class::Layer(3)),
    ("stitchcraft-vectorcraft", Class::Layer(3)),
    ("stitchcraft-testkit", Class::Testkit),
    ("stitchcraft-cli", Class::App),
    ("stitchcraft-vc-plugin", Class::App),
    ("xtask", Class::Tool),
];

/// Allowed edges inside one layer: (from, to).
pub const SAME_LAYER: &[(&str, &str)] = &[("stitchcraft-params", "stitchcraft-core")];

/// External crates allowed only in the listed packages (normal dependencies).
pub const RESTRICTED: &[(&str, &[&str])] = &[
    ("clap", &["stitchcraft-cli", "xtask"]),
    ("rayon", &["stitchcraft-cli"]),
    ("tiny-skia", &["stitchcraft-render"]),
    ("roxmltree", &["stitchcraft-svg"]),
    ("svgtypes", &["stitchcraft-svg"]),
];

/// Kind of a dependency edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `[dependencies]`.
    Normal,
    /// `[dev-dependencies]`.
    Dev,
    /// `[build-dependencies]`.
    Build,
}

/// One workspace package and its dependencies.
#[derive(Clone, Debug)]
pub struct Package {
    /// Package name.
    pub name: String,
    /// (dependency package name, kind).
    pub deps: Vec<(String, Kind)>,
}

/// The layering rule violations in `packages`.
pub fn check(packages: &[Package]) -> Vec<String> {
    let class_of = |name: &str| TABLE.iter().find(|(n, _)| *n == name).map(|(_, c)| *c);
    let members: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();
    let mut errors = Vec::new();
    for package in packages {
        let Some(from) = class_of(&package.name) else {
            errors.push(format!("`{}` is not in the layering table (xtask/src/layers.rs): add a row", package.name));
            continue;
        };
        for (dep, kind) in &package.deps {
            if members.contains(&dep.as_str()) {
                let Some(to) = class_of(dep) else { continue };
                if let Some(problem) = edge_problem(&package.name, from, dep, to, *kind) {
                    errors.push(problem);
                }
            } else if *kind == Kind::Normal
                && let Some((_, allowed)) = RESTRICTED.iter().find(|(name, _)| name == dep)
                && !allowed.contains(&package.name.as_str())
            {
                errors.push(format!("`{}` may not depend on `{dep}` (allowed in: {})", package.name, allowed.join(", ")));
            }
        }
    }
    errors
}

fn edge_problem(from_name: &str, from: Class, to_name: &str, to: Class, kind: Kind) -> Option<String> {
    let edge = format!("`{from_name}` → `{to_name}`");
    match (from, to, kind) {
        (Class::Tool, _, _) => None,
        (_, Class::App, _) => Some(format!("{edge}: nothing may depend on an app")),
        (_, Class::Tool, _) => Some(format!("{edge}: nothing may depend on xtask")),
        (Class::Testkit, Class::Layer(_), _) => None,
        (_, Class::Testkit, Kind::Dev) => None,
        (_, Class::Testkit, _) => Some(format!("{edge}: the testkit is a dev-dependency only")),
        (Class::App, Class::Layer(_), _) => None,
        (Class::Layer(_), Class::Layer(_), Kind::Dev) => None,
        (Class::Layer(a), Class::Layer(b), _) => {
            let allowed = b < a || (a == b && SAME_LAYER.contains(&(from_name, to_name)));
            (!allowed).then(|| format!("{edge}: L{a} may not depend on L{b}"))
        }
    }
}

/// Builds the model from `cargo metadata --no-deps` output.
pub fn model_from_metadata(metadata: &Value) -> Vec<Package> {
    let Some(packages) = metadata.get("packages").and_then(Value::as_array) else { return Vec::new() };
    packages
        .iter()
        .filter_map(|p| {
            let name = p.get("name")?.as_str()?.to_string();
            let deps = p
                .get("dependencies")
                .and_then(Value::as_array)
                .map(|deps| {
                    deps.iter()
                        .filter_map(|d| {
                            let dep = d.get("name")?.as_str()?.to_string();
                            let kind = match d.get("kind").and_then(Value::as_str) {
                                Some("dev") => Kind::Dev,
                                Some("build") => Kind::Build,
                                _ => Kind::Normal,
                            };
                            Some((dep, kind))
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(Package { name, deps })
        })
        .collect()
}

/// `cargo xtask layers`.
pub fn run() -> Result<(), String> {
    let output = util::cargo()
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(util::root())
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let metadata: Value = serde_json::from_slice(&output.stdout).map_err(|e| format!("cargo metadata output: {e}"))?;
    let packages = model_from_metadata(&metadata);
    let mut findings = Findings::default();
    for e in check(&packages) {
        findings.error(e);
    }
    findings.finish("layers", &format!("{} crates follow the layering table", packages.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, deps: &[(&str, Kind)]) -> Package {
        Package { name: name.into(), deps: deps.iter().map(|(d, k)| ((*d).to_string(), *k)).collect() }
    }

    #[test]
    fn downward_edges_pass() {
        let ws = [
            pkg("stitchcraft-core", &[]),
            pkg("stitchcraft-params", &[("stitchcraft-core", Kind::Normal)]),
            pkg("stitchcraft-plan", &[("stitchcraft-core", Kind::Normal)]),
            pkg("stitchcraft-engine", &[("stitchcraft-plan", Kind::Normal), ("stitchcraft-testkit", Kind::Dev)]),
            pkg("stitchcraft-testkit", &[("stitchcraft-engine", Kind::Normal)]),
            pkg("stitchcraft-cli", &[("stitchcraft-engine", Kind::Normal), ("clap", Kind::Normal)]),
        ];
        assert_eq!(check(&ws), Vec::<String>::new());
    }

    #[test]
    fn upward_and_sideways_edges_fail() {
        let ws = [
            pkg("stitchcraft-plan", &[("stitchcraft-engine", Kind::Normal)]),
            pkg("stitchcraft-engine", &[("stitchcraft-formats", Kind::Normal)]),
            pkg("stitchcraft-formats", &[]),
        ];
        let errors = check(&ws);
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(errors.iter().any(|e| e.contains("L1 may not depend on L2")));
        assert!(errors.iter().any(|e| e.contains("L2 may not depend on L2")));
    }

    #[test]
    fn testkit_is_dev_only_and_apps_are_leaves() {
        let ws = [
            pkg("stitchcraft-engine", &[("stitchcraft-testkit", Kind::Normal)]),
            pkg("stitchcraft-testkit", &[]),
            pkg("stitchcraft-svg", &[("stitchcraft-cli", Kind::Normal)]),
            pkg("stitchcraft-cli", &[]),
        ];
        let errors = check(&ws);
        assert!(errors.iter().any(|e| e.contains("dev-dependency only")), "{errors:?}");
        assert!(errors.iter().any(|e| e.contains("nothing may depend on an app")), "{errors:?}");
    }

    #[test]
    fn restricted_externals_and_unknown_crates_fail() {
        let ws = [pkg("stitchcraft-engine", &[("clap", Kind::Normal)]), pkg("stitchcraft-new", &[])];
        let errors = check(&ws);
        assert!(errors.iter().any(|e| e.contains("may not depend on `clap`")), "{errors:?}");
        assert!(errors.iter().any(|e| e.contains("not in the layering table")), "{errors:?}");
    }

    #[test]
    fn table_names_are_unique() {
        let mut names: Vec<&str> = TABLE.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TABLE.len());
    }
}
