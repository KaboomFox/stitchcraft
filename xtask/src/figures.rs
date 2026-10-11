//! Figures in hand-written pages, written by `cargo xtask docs` from the shots they show.
//!
//! A page marks a figure with `<!-- shot: ID -->` and `<!-- /shot -->`, each on a line of its own, and
//! `cargo xtask docs` writes the figure between them. A shot without panels becomes one image with the
//! shot's alternative text. A shot with panels shows each panel's image with its caption, and the image's
//! alternative text is the shot's followed by the caption. Laid out in columns (the default), the figure is
//! a table with a column per panel, the caption on top. Laid out in rows, each caption is a line in bold
//! with its image below it, as wide as the page allows: pictures side by side shrink to share the page's
//! width, and a wide picture's stitches then become too small to tell apart. The links are relative to the
//! page. `--check` fails when a figure is stale, as for a generated page, so the
//! text a page shows with an image is always its shot's.

use std::fmt::Write as _;

use crate::shots::{self, Layout, Shot};

/// The line that opens a figure, before the shot's id and `-->`.
const OPEN: &str = "<!-- shot: ";
/// The line that closes a figure.
const CLOSE: &str = "<!-- /shot -->";
/// Where pages live.
const SRC: &str = "docs/src/";

/// Whether `text` has a figure to write.
pub fn has_figures(text: &str) -> bool {
    text.lines().any(|line| line.trim_start().starts_with(OPEN))
}

/// `text`, the page at `page` (a path from the repository root, under `docs/src`), with every figure
/// written anew from `shots`.
pub fn refresh(page: &str, text: &str, shots: &[Shot]) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut lines = text.split_inclusive('\n').enumerate();
    while let Some((index, line)) = lines.next() {
        out.push_str(line);
        let Some(id) = line.trim().strip_prefix(OPEN).and_then(|rest| rest.strip_suffix("-->")).map(str::trim) else { continue };
        let at = format!("{page}:{}", index + 1);
        let shot = shots.iter().find(|s| s.id == id).ok_or_else(|| format!("{at}: there is no shot `{id}` in docs/shots.toml"))?;
        let close = lines.by_ref().map(|(_, old)| old).find(|old| old.trim() == CLOSE);
        let Some(close) = close else { return Err(format!("{at}: the figure of `{id}` has no `{CLOSE}` line")) };
        out.push_str(&figure(page, shot)?);
        out.push_str(close);
    }
    Ok(out)
}

/// The Markdown of `shot`'s figure on the page at `page`.
fn figure(page: &str, shot: &Shot) -> Result<String, String> {
    let inside = page.strip_prefix(SRC).ok_or_else(|| format!("{page} is not under {SRC}"))?;
    let up = "../".repeat(inside.matches('/').count());
    let images = shots::GENERATED.strip_prefix(SRC).unwrap_or(shots::GENERATED);
    let link = |name: &str| format!("{up}{images}/{name}.png");
    if shot.panels.is_empty() {
        return Ok(format!("![{}]({})\n", shot.alt, link(&shot.id)));
    }
    let mut out = String::new();
    let captions: Vec<&str> = shot.panels.iter().map(|panel| panel.caption.as_str()).collect();
    let images: Vec<String> =
        shot.images().iter().zip(&captions).map(|(name, caption)| format!("![{}, {caption}]({})", shot.alt, link(name))).collect();
    match shot.layout {
        Layout::Columns => {
            let _ = writeln!(out, "| {} |", captions.join(" | "));
            let _ = writeln!(out, "|{}", ":-:|".repeat(captions.len()));
            let _ = writeln!(out, "| {} |", images.join(" | "));
        }
        Layout::Rows => {
            let rows: Vec<String> = captions.iter().zip(&images).map(|(caption, image)| format!("**{caption}**\n\n{image}\n")).collect();
            out.push_str(&rows.join("\n"));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::shots::Panel;

    fn shot(id: &str, captions: &[&str]) -> Shot {
        let panels = captions.iter().map(|c| Panel { caption: (*c).to_string(), params: BTreeMap::new() }).collect();
        let toml = format!("id = \"{id}\"\nkind = \"stitch\"\nalt = \"A wave in running stitch\"\nfixture = \"f.svg\"");
        Shot { panels, ..toml::from_str(&toml).unwrap() }
    }

    #[test]
    fn a_figure_is_written_between_its_lines_with_links_from_the_page() {
        let shots = [shot("wave", &["1.5 mm", "4 mm"]), shot("one", &[])];
        let page = "Before.\n\n<!-- shot: wave -->\nstale\nlines\n<!-- /shot -->\n\n<!-- shot: one -->\n<!-- /shot -->\nAfter.\n";
        let fresh = refresh("docs/src/user/stitches/running.md", page, &shots).unwrap();
        assert_eq!(
            fresh,
            "Before.\n\n<!-- shot: wave -->\n| 1.5 mm | 4 mm |\n|:-:|:-:|\n\
             | ![A wave in running stitch, 1.5 mm](../../images/generated/wave-1.png) | ![A wave in running stitch, 4 mm](../../images/generated/wave-2.png) |\n\
             <!-- /shot -->\n\n<!-- shot: one -->\n![A wave in running stitch](../../images/generated/one.png)\n<!-- /shot -->\nAfter.\n"
        );
        assert_eq!(refresh("docs/src/user/stitches/running.md", &fresh, &shots).unwrap(), fresh, "written once, it stays");
        assert!(has_figures(page) && !has_figures("No figure.\n"));
    }

    #[test]
    fn a_figure_in_rows_puts_each_caption_above_its_image() {
        let shots = [Shot { layout: Layout::Rows, ..shot("wave", &["1.5 mm", "4 mm"]) }];
        let page = "<!-- shot: wave -->\n<!-- /shot -->\n";
        assert_eq!(
            refresh("docs/src/user/stitches/running.md", page, &shots).unwrap(),
            "<!-- shot: wave -->\n**1.5 mm**\n\n![A wave in running stitch, 1.5 mm](../../images/generated/wave-1.png)\n\n\
             **4 mm**\n\n![A wave in running stitch, 4 mm](../../images/generated/wave-2.png)\n<!-- /shot -->\n"
        );
        let toml = "id = \"w\"\nkind = \"stitch\"\nlayout = \"rows\"";
        assert_eq!(toml::from_str::<Shot>(toml).unwrap().layout, Layout::Rows);
        assert!(toml::from_str::<Shot>("id = \"w\"\nkind = \"stitch\"\nlayout = \"grid\"").is_err(), "no other layout");
    }

    #[test]
    fn a_figure_must_name_a_shot_and_be_closed() {
        let shots = [shot("wave", &[])];
        assert!(refresh("docs/src/a.md", "<!-- shot: nope -->\n<!-- /shot -->\n", &shots).unwrap_err().contains("no shot `nope`"));
        assert!(refresh("docs/src/a.md", "<!-- shot: wave -->\nopen\n", &shots).unwrap_err().contains("has no `<!-- /shot -->` line"));
        assert!(refresh("README.md", "<!-- shot: wave -->\n<!-- /shot -->\n", &shots).unwrap_err().contains("is not under docs/src/"));
    }
}
