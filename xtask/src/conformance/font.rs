//! A font read and each of its glyphs planned (roadmap step M14.1, `REQ-FONT-008`,
//! `docs/src/design/conformance.md`): one real font of the corpus, under the OFL, read from the corpus cache
//! by the font reader (`stitchcraft_svg::font`), and each glyph's design planned as the glyph differential
//! plans a glyph, for the reference machine with a hoop 400 mm a side ([`super::fonts::machine`]).
//!
//! The case checks the font's licence and that its reading said nothing, counts its glyphs in each
//! direction against the case's `[glyphs]`, and plans every glyph. A glyph that plans with an error, or whose
//! plan breaks an invariant, fails the case. Where the corpus isn't fetched the case is skipped, and says so,
//! as the glyph differential does: `cargo xtask corpus` fetches it, and CI's gates job fetches it before the
//! suite runs.

use std::collections::BTreeMap;
use std::path::Path;

use stitchcraft_core::{Budget, Severity};
use stitchcraft_plan::invariants;
use stitchcraft_svg::font::{self, Licence};

use super::runner::Outcome;
use crate::{corpus, util};

/// Runs the case of the corpus's font in the folder `folder`, which has `glyphs` glyphs in each direction,
/// by its id.
pub fn run(root: &Path, folder: &str, glyphs: &BTreeMap<String, usize>, outcome: &mut Outcome) {
    let corpus = match corpus::unpacked(root) {
        Ok(Some(corpus)) => corpus,
        Ok(None) => {
            outcome.skipped = Some("Ink/Stitch's fonts are not fetched: run cargo xtask corpus".to_string());
            return;
        }
        Err(e) => return outcome.failures.push(e),
    };
    let dir = corpus.join("src").join(folder);
    let files = match files(&dir) {
        Ok(files) => files,
        Err(e) => return outcome.failures.push(e),
    };
    let given: Vec<(&str, &[u8])> = files.iter().map(|(path, bytes)| (path.as_str(), bytes.as_slice())).collect();
    let font = match font::read(folder, &given, &Budget::DEFAULT) {
        Ok(font) => font,
        Err(e) => return outcome.failures.push(format!("{folder}: {e}")),
    };
    let licence = font.licence.as_ref().map_or(Licence::Other, |file| file.licence);
    if licence != Licence::Ofl {
        outcome.failures.push(format!("{folder} is under {}, where the case reads a font under the OFL", licence.name()));
    }
    outcome.failures.extend(font.warnings.iter().map(|warning| format!("{folder}: reading the font said {warning}")));
    let counts: BTreeMap<String, usize> = font.variants.iter().map(|variant| (variant.direction.id().to_string(), variant.glyphs.len())).collect();
    if &counts != glyphs {
        outcome.failures.push(format!("{folder} has glyphs {counts:?} by direction, where the case expects {glyphs:?}"));
    }
    let machine = super::fonts::machine();
    let (mut planned, mut stitches) = (0_usize, 0_usize);
    for variant in &font.variants {
        for glyph in variant.glyphs.values() {
            let at = format!("{folder}: the {} glyph `{}` of {}", variant.direction.id(), glyph.label, glyph.file);
            let made = stitchcraft_engine::plan(&glyph.design, &machine, &Budget::DEFAULT);
            let errors: Vec<String> =
                glyph.warnings.iter().chain(&made.diagnostics).filter(|d| d.severity() == Severity::Error).map(ToString::to_string).collect();
            match made.plan {
                Some(plan) if errors.is_empty() => {
                    outcome.failures.extend(invariants::check(&plan, &machine).into_iter().map(|violation| format!("{at}: {violation}")));
                    planned += 1;
                    stitches += plan.stitches().count();
                }
                _ => outcome
                    .failures
                    .push(format!("{at} doesn't plan: {}", if errors.is_empty() { "no stitches".to_string() } else { errors.join("; ") })),
            }
        }
    }
    let directions: Vec<String> = counts.iter().map(|(direction, count)| format!("{count} {direction}")).collect();
    outcome.details.push(format!(
        "{folder} ({}), under the {}: glyphs {}, of which {planned} planned, in {stitches} stitches",
        font.settings.name,
        licence.name(),
        directions.join(" and ")
    ));
}

/// Every file of the folder `dir`, at any depth, by its path in the folder with `/` between its parts.
fn files(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return Err(format!("{}: the corpus has no such font", dir.display()));
    }
    for path in util::every_file(dir) {
        let name = path.strip_prefix(dir).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        files.push((name, util::read_bytes(&path)?));
    }
    Ok(files)
}
