//! Every stitch type StitchCraft sews has a page in the user's guide, with pictures.
//!
//! The guide's stitch types section is where a reader looks a stitch type up, and a type without a page,
//! or a page without pictures, leaves them guessing how it sews. So each stitch type the engine sews must be
//! claimed by one page in `docs/src/user/stitches/`, with a comment on a line of its own that names the
//! types by their Ink/Stitch method ids:
//!
//! ```markdown
//! <!-- stitch-types: running_stitch, manual_stitch -->
//! ```
//!
//! and that page shows at least [`PICTURES`] of the pictures `docs/shots.toml` declares. Which types the
//! engine sews is asked of the engine itself ([`sewn`]), so a stitch type that starts to sew needs its page
//! in the same change, as the playbook for a new stitch type says
//! (`docs/src/contributing/playbook-new-stitch-type.md`).
//! [`UNSHOWN`] lists the types whose pages are still to be written, with the roadmap step that writes them:
//! each is a warning until then, and an error once its page is there, so the list only shrinks.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use stitchcraft_cli::commands::plan::Source;
use stitchcraft_core::{Budget, Code};
use stitchcraft_engine::design::{Design, Reading};
use stitchcraft_params::{Family, StitchType};
use stitchcraft_plan::profiles::REFERENCE;

use crate::util::{self, Findings};
use crate::{figures, markdown, shots};

/// The folder of the stitch types' pages.
const FOLDER: &str = "docs/src/user/stitches";

/// The comment that names the stitch types a page is the page of.
const MARKER: &str = "<!-- stitch-types:";

/// The fewest pictures a stitch type's page shows.
pub const PICTURES: usize = 2;

/// The stitch types sewn without a page with pictures yet, each with the roadmap step that writes it.
/// Ink/Stitch's legacy fill is sewn as a tatami fill, so the tatami fill's page is its page too.
const UNSHOWN: &[(StitchType, &str)] = &[(StitchType::SatinColumn, "M13.11"), (StitchType::TatamiFill, "M13.11"), (StitchType::LegacyFill, "M13.11")];

/// One shape of each family, in millimetres: a hairline, a band 4 mm wide, which sews as a satin column
/// drawn as one path once it is set to be one, and a square.
const SHAPES: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40mm" height="40mm" viewBox="0 0 40 40">
<path id="line" d="M 5 5 H 35" fill="none" stroke="#000000" stroke-width="0.2"/>
<path id="band" d="M 5 15 H 35" fill="none" stroke="#000000" stroke-width="4"/>
<path id="area" d="M 5 22 H 35 V 35 H 5 Z" fill="#000000"/>
</svg>"##;

/// Checks that each stitch type the engine sews has its page, with pictures, among `pages` (every Markdown
/// file, as `cargo xtask docs --check` reads them).
pub fn check(root: &Path, pages: &[(PathBuf, String)], findings: &mut Findings) -> Result<(), String> {
    let folder = root.join(FOLDER);
    let sewn = sewn()?;
    let shots = shots::load(root)?;
    let mut claimed: BTreeMap<StitchType, Vec<String>> = BTreeMap::new();
    for (path, text) in pages.iter().filter(|(path, _)| path.starts_with(&folder)) {
        let page = util::rel(path);
        let mut claims = false;
        for (line, id) in declared(text) {
            claims = true;
            match StitchType::ALL.iter().find(|t| t.id() == id) {
                None => findings.error(format!("{page}:{line}: `{id}` is no stitch type's id")),
                Some(t) if !sewn.contains(t) => {
                    findings.error(format!("{page}:{line}: `{id}` is not sewn yet, so the page of {} comes with it", t.name()));
                }
                Some(t) => claimed.entry(*t).or_default().push(page.clone()),
            }
        }
        let pictures: usize =
            figures::shown(text).iter().filter_map(|id| shots.iter().find(|shot| shot.id == *id)).map(|shot| shot.images().len()).sum();
        if claims && pictures < PICTURES {
            findings.error(format!("{page}: a stitch type's page shows at least {PICTURES} pictures, and this one shows {pictures}"));
        }
    }
    for t in &sewn {
        let pages = claimed.get(t).map_or(&[][..], Vec::as_slice);
        let unshown = UNSHOWN.iter().find(|(u, _)| u == t).map(|(_, step)| step);
        match (pages, unshown) {
            ([], None) => {
                findings.error(format!("{} (`{}`) is sewn, and no page in {FOLDER} claims it with `{MARKER} {} -->`", t.name(), t.id(), t.id()));
            }
            ([], Some(step)) => findings.warn(format!("{} (`{}`) has no page with pictures yet: roadmap step {step}", t.name(), t.id())),
            ([page], Some(_)) => {
                findings.error(format!("{page} is the page of {} now: take it out of `UNSHOWN` in xtask/src/stitch_pages.rs", t.name()));
            }
            ([_], None) => {}
            (pages, _) => findings.error(format!("{} (`{}`) is claimed by more than one page: {}", t.name(), t.id(), pages.join(", "))),
        }
    }
    for (t, _) in UNSHOWN.iter().filter(|(t, _)| !sewn.contains(t)) {
        findings.error(format!("`UNSHOWN` in xtask/src/stitch_pages.rs lists {}, which is not sewn", t.name()));
    }
    Ok(())
}

/// The stitch-type ids `text` claims, each with its line, from the comments outside code blocks.
fn declared(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    for (line, content) in markdown::prose_lines(text) {
        let Some(ids) = content.trim().strip_prefix(MARKER).and_then(|rest| rest.strip_suffix("-->")) else { continue };
        out.extend(ids.split(',').map(str::trim).filter(|id| !id.is_empty()).map(|id| (line, id)));
    }
    out
}

/// The stitch types the engine sews, in [`StitchType::ALL`]'s order: each type set on a shape of its family,
/// planned for the reference machine, and kept unless the engine skips it as not sewn yet (`SC-W0011`).
pub fn sewn() -> Result<Vec<StitchType>, String> {
    let (design, _) = Source::Svg.read(SHAPES.as_bytes(), Reading::default()).map_err(|d| format!("the stitch types' shapes: {d}"))?;
    let mut sewn = Vec::new();
    for &t in StitchType::ALL {
        let (shape, settings): (&str, &[(&str, &str)]) = match t.family() {
            Family::Stroke => ("svg:line:stroke", &[]),
            Family::Satin => ("svg:band:stroke", &[("satin_column", "true")]),
            Family::Fill => ("svg:area:fill", &[]),
        };
        let mut element = design
            .elements()
            .iter()
            .find(|e| e.id.as_str() == shape)
            .cloned()
            .ok_or_else(|| format!("the stitch types' shapes have no `{shape}`"))?;
        for (key, value) in settings.iter().chain(&[(t.family().method_param(), t.id())]) {
            element.params.set(*key, *value);
        }
        let one = Design::new(vec![element], design.settings).map_err(|d| format!("{}: {d}", t.id()))?;
        let outcome = stitchcraft_engine::plan(&one, REFERENCE, &Budget::DEFAULT);
        if !outcome.diagnostics.iter().any(|d| d.code == Code::StitchTypeNotYet) {
            sewn.push(t);
        }
    }
    Ok(sewn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engine_sews_the_stitch_types_of_m1_to_m7_5() {
        let ids: Vec<&str> = sewn().unwrap().iter().map(|t| t.id()).collect();
        assert_eq!(
            ids,
            ["running_stitch", "manual_stitch", "zigzag_stitch", "satin_column", "e_stitch", "s_stitch", "zigzag", "tatami_fill", "legacy_fill"]
        );
    }

    #[test]
    fn a_page_claims_stitch_types_by_their_ids_outside_code_blocks() {
        let text = "# Running stitch\n\n<!-- stitch-types: running_stitch, manual_stitch -->\n\n```markdown\n<!-- stitch-types: e_stitch -->\n```\n";
        assert_eq!(declared(text), [(3, "running_stitch"), (3, "manual_stitch")]);
    }

    /// The findings of [`check`] over the pages `pages`, written in the stitch types' folder.
    fn findings(pages: &[(&str, &str)]) -> Findings {
        let root = util::root();
        let pages: Vec<(PathBuf, String)> = pages.iter().map(|(name, text)| (root.join(FOLDER).join(name), (*text).to_string())).collect();
        let mut findings = Findings::default();
        check(&root, &pages, &mut findings).unwrap();
        findings
    }

    /// A page that claims `ids` and shows the shots `shown`.
    fn page(ids: &str, shown: &[&str]) -> String {
        let figures: String = shown.iter().map(|id| format!("<!-- shot: {id} -->\n<!-- /shot -->\n")).collect();
        format!("# A stitch\n\n<!-- stitch-types: {ids} -->\n\n{figures}")
    }

    #[test]
    fn each_stitch_type_sewn_needs_one_page_with_two_pictures() {
        let found = findings(&[
            ("running.md", &page("running_stitch, manual_stitch", &["running-length", "manual"])),
            ("zigzag.md", &page("zigzag_stitch", &["testsheet-ts-01"])),
            ("e-stitch.md", &page("e_stitch, contour_fill, no_such_type", &["e-stitch-spacing", "e-stitch-swapped"])),
            ("s-stitch.md", &page("s_stitch", &["s-stitch-spacing", "s-stitch-split"])),
            ("s-stitch-again.md", &page("s_stitch", &["s-stitch-spacing", "s-stitch-split"])),
            ("tatami.md", &page("tatami_fill", &["testsheet-ts-01", "testsheet-ts-02"])),
        ]);
        let folder = FOLDER;
        let pictures = found.errors.iter().find(|e| e.contains("zigzag.md")).unwrap();
        assert_eq!(*pictures, format!("{folder}/zigzag.md: a stitch type's page shows at least 2 pictures, and this one shows 1"));
        for expected in [
            format!("{folder}/e-stitch.md:3: `contour_fill` is not sewn yet, so the page of Contour fill comes with it"),
            format!("{folder}/e-stitch.md:3: `no_such_type` is no stitch type's id"),
            "Zigzag satin (`zigzag`) is sewn, and no page in docs/src/user/stitches claims it with `<!-- stitch-types: zigzag -->`".to_string(),
            format!("S-stitch (`s_stitch`) is claimed by more than one page: {folder}/s-stitch.md, {folder}/s-stitch-again.md"),
            format!("{folder}/tatami.md is the page of Tatami fill now: take it out of `UNSHOWN` in xtask/src/stitch_pages.rs"),
        ] {
            assert!(found.errors.contains(&expected), "{expected}\nin {:#?}", found.errors);
        }
        assert_eq!(found.errors.len(), 6, "{:#?}", found.errors);
        assert_eq!(
            found.warnings,
            [
                "Satin column (`satin_column`) has no page with pictures yet: roadmap step M13.11",
                "Legacy fill (`legacy_fill`) has no page with pictures yet: roadmap step M13.11"
            ]
        );
    }
}
