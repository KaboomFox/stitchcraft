//! The diagnostics the SVG reader reports, from real files, exactly as a user reads them (`diag_` cases:
//! every registered code has one, `docs/src/design/diagnostics.md`).

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

mod common;

use common::{fixture, ids, svg, warnings};
use stitchcraft_core::{Budget, Diagnostic};
use stitchcraft_svg::read;

fn refusal(bytes: &[u8]) -> Diagnostic {
    read(bytes, &Budget::DEFAULT).unwrap_err()
}

#[test]
fn diag_sc_e0801_a_file_that_is_not_svg_says_why() {
    let d = refusal(br#"<html xmlns="http://www.w3.org/1999/xhtml"><body/></html>"#);
    assert_eq!(d.to_string(), "error SC-E0801: The file's root element is <html>, not <svg>.");
    assert_eq!(d.fix.map(|f| f.describe()).as_deref(), Some("Open the file in a vector editor and save it again as plain SVG."));
    let says = |bytes: &[u8]| refusal(bytes).message;
    assert_eq!(says(b"<svg><path></svg>"), "The file is not well-formed XML: expected 'path' tag, not 'svg' at 1:12.");
    assert_eq!(says(b"\x1f\x8b\x08\x00"), "The file is compressed SVG (.svgz); StitchCraft reads plain SVG.");
    assert_eq!(says(b"<svg>\xff</svg>"), "The file is not UTF-8 text: byte 5 starts no character.");
    assert_eq!(
        says(b"<?xml version=\"1.0\" encoding=\"windows-1252\"?><svg>\x80</svg>"),
        "The file is windows-1252 text; StitchCraft reads UTF-8 and ISO-8859-1."
    );
    assert_eq!(says(b"\xff\xfe<\x00s\x00"), "The file is UTF-16 text; StitchCraft reads UTF-8.");
    assert_eq!(
        says(br#"<svg xmlns="http://example.org/not-svg"/>"#),
        "The file's <svg> element is in the namespace http://example.org/not-svg, not SVG's."
    );
    assert_eq!(says(br#"<svg width="x"/>"#), "The root element's width, \"x\", is not a length.");
    assert_eq!(says(br#"<svg preserveAspectRatio="sideways"/>"#), "The root element's preserveAspectRatio, \"sideways\", is not one.");
    let nodes = format!("<svg>{}</svg>", "<g/>".repeat(1_000_000));
    assert_eq!(says(nodes.as_bytes()), "The file has more than 1000000 XML nodes, more than StitchCraft reads.");
    // Entities whose values are plain text are read (`REQ-SVG-001`); others could need far more memory than
    // the file, and are refused.
    assert_eq!(
        says(br#"<!DOCTYPE svg [<!ENTITY a "&#38;b;"><!ENTITY b "bb">]><svg>&a;</svg>"#),
        "The file declares the XML entity `a`, whose value holds markup or other entities; StitchCraft reads entities of plain text only."
    );
    assert_eq!(
        says(br#"<!DOCTYPE svg [<!ENTITY a SYSTEM "file:///etc/passwd">]><svg>&a;</svg>"#),
        "The file declares the XML entity `a`, which is not plain text in the file; StitchCraft reads entities of plain text only."
    );
    assert_eq!(
        says(br#"<!DOCTYPE svg [<!ENTITY % p "x">]><svg/>"#),
        "The file declares the XML entity `%`, which is not plain text in the file; StitchCraft reads entities of plain text only."
    );
    let flat = format!(r#"<!DOCTYPE svg [<!ENTITY a "{}">]><svg>{}</svg>"#, "a".repeat(1 << 20), "&a;".repeat(100));
    assert_eq!(says(flat.as_bytes()), "The file's XML entities would make it 102 MB long; StitchCraft reads SVG files of up to 64 MB.");
    assert_eq!(says(br#"<svg width="0" height="10"/>"#), "The drawing's size cannot be used: its width is zero or negative.");
    assert_eq!(says(br#"<svg viewBox="0 0 10"/>"#), "The root element's viewBox, \"0 0 10\", is not four numbers.");
    assert_eq!(says(&vec![b' '; stitchcraft_svg::MAX_BYTES + 1]), "The file is 65 MB; StitchCraft reads SVG files of up to 64 MB.");
    assert!(says(&vec![b' '; stitchcraft_svg::MAX_BYTES]).starts_with("The file is not well-formed XML"), "the limit itself is allowed");
    // A DTD that declares nothing, as older editors wrote, is fine.
    let old = br#"<?xml version="1.0"?><!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd"><svg xmlns="http://www.w3.org/2000/svg"><rect width="1" height="1"/></svg>"#;
    assert_eq!(ids(&svg(old)), ["svg:rect@1:fill"]);
}

#[test]
fn diag_sc_w0802_what_is_not_stitched_is_named() {
    let file = svg(fixture("unsupported.svg"));
    assert_eq!(
        warnings(&file),
        [
            "warning SC-W0802: The file has a style sheet (`<style>`), which StitchCraft does not read: the colours, fills, outlines and hidden elements it sets are not used.",
            "warning SC-W0802: `title` is text, which is not stitched; it is left out.",
            "warning SC-W0802: `photo` is a raster image, which is not stitched; it is left out.",
            "warning SC-W0802: `clone` is a clone (`<use>`), which is not stitched yet; it is left out.",
            "warning SC-W0802: The fill of `patterned` is a `<pattern>`, which is not stitched; it is stitched in the fallback colour, #000080.",
            "warning SC-W0802: `clipped` is clipped (`clip-path`); clipping is not applied, so all of it is stitched.",
            "warning SC-W0802: `inner` is a nested `<svg>` element, which is not read yet; it is left out.",
        ]
    );
    // Each warning about an element names it, and the actionable ones say what to do.
    let text = &file.warnings[1];
    assert_eq!(text.element.as_ref().map(ToString::to_string).as_deref(), Some("svg:title"));
    assert_eq!(text.fix.as_ref().map(|f| f.describe()).as_deref(), Some("Convert the text to paths first (in Inkscape: Path › Object to Path)."));
    // What can be stitched still is.
    assert_eq!(ids(&file), ["svg:patterned:fill", "svg:clipped:fill", "svg:styled:fill"]);

    let more = svg(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkstitch="http://inkstitch.org/namespace" transform="scale(2)">
          <path id="arrow" d="M 0 0 L 10 0" stroke="red" fill="none" marker-end="url(#head)"/>
          <g id="masked" mask="url(#m)"><rect id="shadowed" width="1" height="1" style="filter:url(#blur)"/></g>
          <foreignObject id="html" width="10" height="10"/>
          <flowRoot id="flowed"/>
          <switch id="nothing"><rect requiredExtensions="http://example.org/x" width="1" height="1"/></switch>
          <rect id="settings" width="1" height="1" inkstitch:fill_method="tatami_fill"/>
          <rect id="plain" width="1" height="1" clip-path="none" mask="none" filter="none"/>
          <text id="LONG">a label too long to be an element id is replaced by the element's place</text>
        </svg>"#
        .replace("LONG", &"t".repeat(300)));
    assert_eq!(
        warnings(&more),
        [
            "warning SC-W0802: The root `<svg>` element's transform is not applied.",
            "warning SC-W0802: `arrow` has markers (arrowheads and the like), which are not stitched.",
            "warning SC-W0802: `masked` has a mask; masks are not applied, so all of it is stitched.",
            "warning SC-W0802: `shadowed` has filter effects (blurs, shadows and the like), which are not stitched.",
            "warning SC-W0802: `html` holds content that is not SVG (`<foreignObject>`), which is not stitched; it is left out.",
            "warning SC-W0802: `flowed` is text, which is not stitched; it is left out.",
            "warning SC-W0802: The file has Ink/Stitch embroidery settings, which this version does not read yet: every element uses the default settings.",
            "warning SC-W0802: `text@1` is text, which is not stitched; it is left out.",
        ]
    );
    assert_eq!(more.warnings[0].element, None, "a note about the whole file names no element");
    assert_eq!(ids(&more), ["svg:arrow:stroke", "svg:shadowed:fill", "svg:settings:fill", "svg:plain:fill"]);

    // A paint StitchCraft cannot read is ignored, as a viewer ignores it, and named.
    let paints = svg(r#"<svg xmlns="http://www.w3.org/2000/svg">
          <rect id="odd" width="1" height="1" fill="none" stroke="bogus"/>
          <rect id="also" width="1" height="1" style="fill:rgb(1,2)"/>
        </svg>"#);
    assert_eq!(
        warnings(&paints),
        [
            "warning SC-W0802: The stroke of `odd`, \"bogus\", is not a colour StitchCraft reads; it is ignored, as a viewer ignores it.",
            "warning SC-W0802: The fill of `also`, \"rgb(1,2)\", is not a colour StitchCraft reads; it is ignored, as a viewer ignores it.",
        ]
    );
    assert_eq!(ids(&paints), ["svg:also:fill"], "the inherited black fill");
}

#[test]
fn diag_sc_w0804_geometry_that_cannot_be_used_is_named() {
    let svg = svg(fixture("degenerate.svg"));
    assert_eq!(
        warnings(&svg),
        [
            "warning SC-W0804: The path data of `broken` has an error (invalid number at position 22); it is stitched up to the error.",
            "warning SC-W0804: `empty` draws nothing: its size or its data is empty. It is left out.",
            "warning SC-W0804: `flat` draws nothing: its size or its data is empty. It is left out.",
            "warning SC-W0804: `far` cannot be stitched: it lies more than 10 m from the document's origin. It is left out.",
            "warning SC-W0804: `dot` draws nothing: all of its points are in the same place. It is left out.",
        ]
    );
    assert_eq!(svg.warnings[0].element.as_ref().map(ToString::to_string).as_deref(), Some("svg:broken"));

    // A length that does not read is 0, as in a viewer, and named.
    let lengths = stitchcraft_svg::read(
        br#"<svg xmlns="http://www.w3.org/2000/svg"><line id="tilted" x1="0" y1="eighty" x2="10" y2="0" stroke="red"/></svg>"#,
        &Budget::DEFAULT,
    )
    .unwrap();
    assert_eq!(warnings(&lengths), ["warning SC-W0804: The y1 of `tilted`, \"eighty\", is not a length; 0 is used, as a viewer uses it."]);
    assert_eq!(ids(&lengths), ["svg:tilted:stroke"]);
}
