//! `stitch explain CODE`: a diagnostic's explanation, from the registry (the same text as its rustdoc and
//! its page in the diagnostics index).

use stitchcraft_core::Code;

use super::Outcome;

/// Explains `code` (any case; `W0702` works too).
pub fn run(code: &str) -> Outcome {
    let wanted = code.trim().to_ascii_uppercase();
    let wanted = if wanted.starts_with("SC-") { wanted } else { format!("SC-{wanted}") };
    match Code::ALL.iter().find(|c| c.id() == wanted) {
        Some(code) => Outcome::done(format!("{code}: {}\n\n{}\n", code.title(), code.explanation())),
        None => Outcome::usage(format!("`{code}` is not a StitchCraft diagnostic code (they look like SC-W0702)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::Status;

    #[test]
    fn explains_registered_codes_in_any_spelling() {
        for spelling in ["SC-W0702", "sc-w0702", "W0702"] {
            let out = run(spelling);
            assert_eq!(out.status, Status::Done);
            assert!(out.stdout.starts_with("SC-W0702: Design is larger than the comfort zone\n\nThe design fits the hoop"));
        }
        assert_eq!(run("SC-X9999").status, Status::Usage);
    }
}
