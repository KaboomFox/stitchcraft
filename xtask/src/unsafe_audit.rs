//! `cargo xtask unsafe-audit`: `unsafe` lives in exactly one audited file.
//!
//! Rules (`AGENTS.md` › Non-negotiables):
//! 1. Every crate root carries `#![forbid(unsafe_code)]`, except the VectorCraft plug-in, whose ABI shim
//!    must handle raw pointers.
//! 2. The `unsafe` keyword appears only in `apps/stitchcraft-vc-plugin/src/abi.rs`.
//! 3. There, every `unsafe { … }` block is preceded by a `// SAFETY:` comment, and every `unsafe fn` has a
//!    `# Safety` section in its documentation.

use std::path::Path;

use crate::util::{self, Findings};

/// The one file allowed to use `unsafe`.
pub const SHIM: &str = "apps/stitchcraft-vc-plugin/src/abi.rs";
/// The crate root exempt from `#![forbid(unsafe_code)]`.
const EXEMPT_ROOT: &str = "apps/stitchcraft-vc-plugin/src/lib.rs";
const FORBID: &str = "#![forbid(unsafe_code)]";

/// Rust source with comments and string literals blanked, line by line, so keyword searches see only code.
pub fn code_only(source: &str) -> Vec<String> {
    #[derive(PartialEq)]
    enum State {
        Code,
        Str,
        RawStr(usize),
        Block(usize),
    }
    let mut state = State::Code;
    let mut lines = Vec::new();
    for line in source.lines() {
        let mut out = String::with_capacity(line.len());
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while let Some(&c) = chars.get(i) {
            let next = chars.get(i + 1).copied();
            match state {
                State::Code => {
                    if c == '/' && next == Some('/') {
                        break; // line comment: drop the rest of the line
                    } else if c == '/' && next == Some('*') {
                        state = State::Block(1);
                        i += 2;
                        continue;
                    } else if c == '"' {
                        state = State::Str;
                    } else if c == 'r' && (next == Some('"') || next == Some('#')) {
                        let hashes = chars.iter().skip(i + 1).take_while(|h| **h == '#').count();
                        if chars.get(i + 1 + hashes) == Some(&'"') {
                            state = State::RawStr(hashes);
                            i += 2 + hashes;
                            out.push(' ');
                            continue;
                        }
                        out.push(c);
                    } else if c == '\'' && next == Some('"') && chars.get(i + 2) == Some(&'\'') {
                        i += 3; // the char literal '"'
                        out.push(' ');
                        continue;
                    } else {
                        out.push(c);
                    }
                }
                State::Str => {
                    if c == '\\' {
                        i += 2;
                        continue;
                    }
                    if c == '"' {
                        state = State::Code;
                    }
                    out.push(' ');
                }
                State::RawStr(hashes) => {
                    if c == '"' && chars.iter().skip(i + 1).take(hashes).filter(|h| **h == '#').count() == hashes {
                        state = State::Code;
                        i += 1 + hashes;
                        continue;
                    }
                    out.push(' ');
                }
                State::Block(depth) => {
                    if c == '*' && next == Some('/') {
                        state = if depth == 1 { State::Code } else { State::Block(depth - 1) };
                        i += 2;
                        continue;
                    }
                    if c == '/' && next == Some('*') {
                        state = State::Block(depth + 1);
                        i += 2;
                        continue;
                    }
                }
            }
            i += 1;
        }
        lines.push(out);
    }
    lines
}

/// Whether `line` contains `unsafe` as a whole word.
fn has_unsafe_word(line: &str) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices("unsafe").any(|(i, _)| {
        let before = line.get(..i).and_then(|s| s.chars().last());
        let after = line.get(i + "unsafe".len()..).and_then(|s| s.chars().next());
        !before.is_some_and(is_word) && !after.is_some_and(is_word)
    })
}

/// Rule-3 violations in the shim's source.
fn shim_problems(source: &str) -> Vec<String> {
    let raw: Vec<&str> = source.lines().collect();
    let code = code_only(source);
    let mut out = Vec::new();
    for (i, line) in code.iter().enumerate() {
        let compact: String = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if compact.contains("unsafe {") {
            let start = i.saturating_sub(3);
            let has_comment = raw.get(start..i).is_some_and(|prev| prev.iter().any(|l| l.contains("// SAFETY:")));
            if !has_comment {
                out.push(format!("line {}: `unsafe` block without a `// SAFETY:` comment above it", i + 1));
            }
        }
        if compact.contains("unsafe fn") || compact.contains("unsafe extern") {
            let docs: Vec<&&str> = raw
                .get(..i)
                .unwrap_or(&[])
                .iter()
                .rev()
                .take_while(|l| {
                    let t = l.trim_start();
                    t.starts_with("///") || t.starts_with("#[")
                })
                .collect();
            if !docs.iter().any(|l| l.contains("# Safety")) {
                out.push(format!("line {}: `unsafe fn` without a `# Safety` section in its docs", i + 1));
            }
        }
    }
    out
}

/// `cargo xtask unsafe-audit`.
pub fn run() -> Result<(), String> {
    let root = util::root();
    let mut findings = Findings::default();
    for pattern_dir in ["crates", "apps"] {
        let Ok(entries) = std::fs::read_dir(root.join(pattern_dir)) else { continue };
        let mut crates: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        crates.sort();
        for krate in crates {
            check_root(&krate, &mut findings)?;
        }
    }
    check_file_root(&root.join("xtask/src/main.rs"), &mut findings)?;

    let mut files = Vec::new();
    for dir in ["crates", "apps", "xtask", "compat"] {
        files.extend(util::files(&root.join(dir), &["rs"]));
    }
    for path in &files {
        let rel = util::rel(path);
        let source = util::read(path)?;
        if rel == SHIM {
            for p in shim_problems(&source) {
                findings.error(format!("{rel}: {p}"));
            }
            continue;
        }
        for (i, line) in code_only(&source).iter().enumerate() {
            if has_unsafe_word(line) {
                findings.error(format!("{rel}:{}: `unsafe` outside {SHIM}", i + 1));
            }
        }
    }
    findings.finish("unsafe-audit", &format!("{} Rust files: unsafe only in {SHIM}, every block justified", files.len()))
}

fn check_root(krate: &Path, findings: &mut Findings) -> Result<(), String> {
    for name in ["src/lib.rs", "src/main.rs"] {
        let path = krate.join(name);
        if path.is_file() {
            check_file_root(&path, findings)?;
        }
    }
    Ok(())
}

fn check_file_root(path: &Path, findings: &mut Findings) -> Result<(), String> {
    let rel = util::rel(path);
    if rel == EXEMPT_ROOT {
        return Ok(());
    }
    if !util::read(path)?.contains(FORBID) {
        findings.error(format!("{rel}: crate root must contain `{FORBID}`"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_strings_are_not_code() {
        let src = "let a = \"unsafe\"; // unsafe\n/* unsafe */ let b = r#\"unsafe\"#;\nunsafe { x() }";
        let code = code_only(src);
        assert!(!has_unsafe_word(&code[0]));
        assert!(!has_unsafe_word(&code[1]));
        assert!(has_unsafe_word(&code[2]));
        assert!(!has_unsafe_word("unsafe_code"));
    }

    #[test]
    fn shim_blocks_need_safety_comments() {
        let good = "/// # Safety\n/// ok\npub unsafe fn f() {\n    // SAFETY: ok\n    let x = unsafe { g() };\n}";
        assert!(shim_problems(good).is_empty(), "{:?}", shim_problems(good));
        let bad = "pub unsafe fn f() {\n    let x = unsafe { g() };\n}";
        assert_eq!(shim_problems(bad).len(), 2);
    }
}
