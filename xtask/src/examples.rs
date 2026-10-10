//! The command-line examples the docs show, declared in `docs/examples.toml` and run with the `stitch`
//! binary itself, so a page shows what the command prints today and nothing else.
//!
//! `cargo xtask docs` builds the binary, runs each example in an empty directory and writes the output to
//! `docs/src/user/reference/generated/<id>.txt`, where pages include it. `--check` fails when the output
//! changed, as for every generated page. An example can copy files of the repository into its directory
//! (`files`) and run commands that are not shown (`setup`). Each command of `run` is shown after `$ `,
//! followed by what it printed and, when it is not 0, its exit status. The binary writes its standard
//! output before its standard error, and the text keeps that order.
//!
//! A command is the words of a shell command line: spaces separate them, and double quotes keep spaces in
//! one. Nothing else is interpreted, and the first word is `stitch`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::util;

/// The declarations.
pub const DECLARATIONS: &str = "docs/examples.toml";
/// Where the output goes; pages include `<id>.txt` from here.
pub const OUTPUT: &str = "docs/src/user/reference/generated";

#[derive(Deserialize, Default)]
struct Declarations {
    #[serde(default)]
    example: Vec<Example>,
}

/// One example.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Example {
    /// The output's name: `<id>.txt`.
    id: String,
    /// Files of the repository to copy into the example's directory, by path from the repository root.
    #[serde(default)]
    files: Vec<String>,
    /// Commands run first, and not shown.
    #[serde(default)]
    setup: Vec<String>,
    /// The commands shown, with their output.
    run: Vec<String>,
}

/// What one command did.
struct Ran {
    /// Standard output, then standard error.
    printed: String,
    /// The exit status; `None` when the process was ended by a signal.
    status: Option<i32>,
}

/// Every example's output: (path from the repository root, text).
pub fn pages(root: &Path) -> Result<Vec<(String, String)>, String> {
    let declared: Declarations = toml::from_str(&util::read(&root.join(DECLARATIONS))?).map_err(|e| format!("{DECLARATIONS}: {e}"))?;
    if declared.example.is_empty() {
        return Ok(Vec::new());
    }
    let stitch = binary(root)?;
    declared.example.iter().map(|example| Ok((format!("{OUTPUT}/{}.txt", example.id), shown(root, &stitch, example)?))).collect()
}

/// Builds the `stitch` binary and returns its path, as cargo reports it.
fn binary(root: &Path) -> Result<PathBuf, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["build", "--quiet", "--package", "stitchcraft-cli", "--bin", "stitch", "--message-format=json"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cargo build: {e}"))?;
    if !output.status.success() {
        return Err(format!("cargo build of `stitch` failed: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    executable(&String::from_utf8_lossy(&output.stdout)).ok_or_else(|| "cargo build did not report the `stitch` binary".to_string())
}

/// The `stitch` executable in cargo's JSON messages.
fn executable(messages: &str) -> Option<PathBuf> {
    messages.lines().filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok()).find_map(|message| {
        let target = message.get("target")?;
        let is_bin = target.get("kind")?.as_array()?.iter().any(|kind| kind == "bin");
        (message.get("reason")? == "compiler-artifact" && target.get("name")? == "stitch" && is_bin)
            .then(|| message.get("executable")?.as_str().map(PathBuf::from))
            .flatten()
    })
}

/// The text of `example`: each shown command and what it printed, run in a fresh directory.
fn shown(root: &Path, stitch: &Path, example: &Example) -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!("stitchcraft-example-{}-{}", std::process::id(), example.id));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let result = run_in(root, stitch, example, &dir);
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn run_in(root: &Path, stitch: &Path, example: &Example, dir: &Path) -> Result<String, String> {
    let id = &example.id;
    for file in &example.files {
        let from = root.join(file);
        let name = from.file_name().ok_or_else(|| format!("example `{id}`: `{file}` is not a file"))?;
        std::fs::copy(&from, dir.join(name)).map_err(|e| format!("example `{id}`: {file}: {e}"))?;
    }
    for command in &example.setup {
        let ran = execute(stitch, command, dir).map_err(|e| format!("example `{id}`: {e}"))?;
        if ran.status != Some(0) {
            return Err(format!("example `{id}`: setup `{command}` failed:\n{}", ran.printed));
        }
    }
    let mut text = String::new();
    for command in &example.run {
        let ran = execute(stitch, command, dir).map_err(|e| format!("example `{id}`: {e}"))?;
        text.push_str(&transcript(command, &ran));
    }
    Ok(text)
}

/// Runs `command` with the binary `stitch` in `dir`. The directory's own path never shows in the output.
fn execute(stitch: &Path, command: &str, dir: &Path) -> Result<Ran, String> {
    let words = words(command)?;
    let (program, args) = words.split_first().ok_or_else(|| "an empty command".to_string())?;
    if program != "stitch" {
        return Err(format!("`{command}` does not run `stitch`"));
    }
    let output = Command::new(stitch).args(args).current_dir(dir).output().map_err(|e| format!("`{command}`: {e}"))?;
    let printed = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let shown_dir = format!("{}{}", dir.display(), std::path::MAIN_SEPARATOR);
    Ok(Ran { printed: printed.replace(&shown_dir, ""), status: output.status.code() })
}

/// One command as a terminal shows it.
fn transcript(command: &str, ran: &Ran) -> String {
    let mut text = format!("$ {command}\n{}", ran.printed);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    match ran.status {
        Some(0) => {}
        Some(code) => text.push_str(&format!("(exit status {code})\n")),
        None => text.push_str("(ended by a signal)\n"),
    }
    text
}

/// The words of a command line: spaces separate them, and double quotes keep spaces in one.
fn words(command: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let (mut word, mut quoted, mut started) = (String::new(), false, false);
    for c in command.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err(format!("`{command}` has a quote that is not closed"));
    }
    if started {
        words.push(word);
    }
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_split_on_spaces_and_kept_whole_in_quotes() {
        assert_eq!(
            words("stitch bug-report d.svg --says \"two words\" -o \"\"").unwrap(),
            ["stitch", "bug-report", "d.svg", "--says", "two words", "-o", ""]
        );
        assert_eq!(words("  stitch   explain SC-W0702 ").unwrap(), ["stitch", "explain", "SC-W0702"]);
        assert!(words("stitch \"open").is_err());
    }

    #[test]
    fn a_transcript_shows_the_command_its_output_and_a_failing_status() {
        let done = Ran { printed: "wrote TS-01.pes".to_string(), status: Some(0) };
        assert_eq!(transcript("stitch testsheet TS-01", &done), "$ stitch testsheet TS-01\nwrote TS-01.pes\n");
        let bug = Ran { printed: "error\n".to_string(), status: Some(4) };
        assert_eq!(transcript("stitch plan d.svg", &bug), "$ stitch plan d.svg\nerror\n(exit status 4)\n");
        let killed = Ran { printed: String::new(), status: None };
        assert_eq!(transcript("stitch plan d.svg", &killed), "$ stitch plan d.svg\n(ended by a signal)\n");
    }

    #[test]
    fn the_binary_is_found_in_cargo_s_messages() {
        let messages = concat!(
            r#"{"reason":"compiler-artifact","target":{"name":"stitchcraft_cli","kind":["lib"]},"executable":null}"#,
            "\n",
            r#"{"reason":"compiler-artifact","target":{"name":"stitch","kind":["bin"]},"executable":"/t/debug/stitch"}"#,
            "\n",
            r#"{"reason":"build-finished","success":true}"#,
        );
        assert_eq!(executable(messages), Some(PathBuf::from("/t/debug/stitch")));
        assert_eq!(executable("not json"), None);
    }
}
