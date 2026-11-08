//! The colours of a threadcount's letters: the shades the Scottish Register of Tartans publishes for them, in a
//! table of StitchCraft's own (milestone M10.7, `docs/src/design/algorithms/tartan.md` › Codes).
//!
//! **Why a table.** A threadcount names each stripe's colour by letters, `B` for blue and `DG` for dark green,
//! as the register writes its threadcounts. The register publishes the shades its letters stand for in
//! "Tartan Register colours", <https://www.tartanregister.gov.uk/docs/Colour_shades.pdf>, several for most
//! letters, each by name and value. Every value below is one of that list's. Where it gives a letter more
//! than one shade, the table takes the one Ink/Stitch's tartan sews for the letter, read at `d59c9ab`, so
//! that a sett written in letters sews the same threads in both programs.
//!
//! **Its terms.** The list is Crown copyright. The register's copyright page,
//! <https://www.tartanregister.gov.uk/copyright>, lets the text of its website be reused free of charge in
//! any format, acknowledged as Crown copyright with the title of its source, as `NOTICE` acknowledges it.

use stitchcraft_plan::Rgb;

/// Each colour's letters, as a threadcount writes them, the register's name for it and the shade the table
/// takes, all from "Tartan Register colours" (<https://www.tartanregister.gov.uk/docs/Colour_shades.pdf>).
const SHADES: [(&str, &str, Rgb); 25] = [
    ("LR", "Light Red", Rgb::new(0xE8, 0x78, 0x78)),
    ("R", "Red", Rgb::new(0xFF, 0x00, 0x00)),
    ("DR", "Dark Red", Rgb::new(0x88, 0x00, 0x00)),
    ("O", "Orange", Rgb::new(0xFF, 0x50, 0x00)),
    ("DO", "Dark Orange", Rgb::new(0xBE, 0x78, 0x32)),
    ("LY", "Light Yellow", Rgb::new(0xF9, 0xF5, 0xC8)),
    ("Y", "Yellow", Rgb::new(0xFF, 0xFF, 0x00)),
    ("DY", "Dark Yellow", Rgb::new(0xC8, 0x8C, 0x00)),
    ("LG", "Light Green", Rgb::new(0xAC, 0xD7, 0x4A)),
    ("G", "Green", Rgb::new(0x28, 0x9C, 0x18)),
    ("DG", "Dark Green", Rgb::new(0x00, 0x40, 0x28)),
    ("LB", "Light Blue", Rgb::new(0x82, 0xCF, 0xFD)),
    ("B", "Blue", Rgb::new(0x00, 0x00, 0xFF)),
    ("DB", "Dark Blue", Rgb::new(0x00, 0x00, 0x8C)),
    ("LP", "Light Purple", Rgb::new(0xA8, 0xAC, 0xE8)),
    ("P", "Purple", Rgb::new(0x78, 0x00, 0x78)),
    ("DP", "Dark Purple", Rgb::new(0x1E, 0x09, 0x48)),
    ("W", "White", Rgb::new(0xFF, 0xFF, 0xFF)),
    ("LN", "Light Grey", Rgb::new(0xE0, 0xE0, 0xE0)),
    ("N", "Grey", Rgb::new(0xA0, 0xA0, 0xA0)),
    ("DN", "Dark Grey", Rgb::new(0x55, 0x5A, 0x64)),
    ("K", "Black", Rgb::new(0x00, 0x00, 0x00)),
    ("LT", "Light Brown", Rgb::new(0xA0, 0x7C, 0x58)),
    ("T", "Brown", Rgb::new(0x60, 0x38, 0x00)),
    ("DT", "Dark Brown", Rgb::new(0x44, 0x18, 0x00)),
];

/// The shade the register's `letters` stand for, in capitals or not, as Ink/Stitch reads them; `None` for
/// letters the register doesn't use.
pub(crate) fn shade(letters: &str) -> Option<Rgb> {
    SHADES.iter().find(|(code, ..)| code.eq_ignore_ascii_case(letters)).map(|&(.., rgb)| rgb)
}

/// The register's colours, each its letters, its name and the shade the table takes for it, in the order of
/// its list.
pub fn shades() -> impl Iterator<Item = (&'static str, &'static str, Rgb)> {
    SHADES.iter().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_fill_trt_016_each_letter_is_one_of_the_register_s_shades_in_capitals_or_not() {
        assert_eq!(shade("B"), Some(Rgb::new(0, 0, 0xFF)));
        assert_eq!(shade("b"), Some(Rgb::new(0, 0, 0xFF)));
        assert_eq!(shade("Dg"), Some(Rgb::new(0x00, 0x40, 0x28)));
        assert_eq!(shade("DN"), Some(Rgb::new(0x55, 0x5A, 0x64)));
        // Letters the register doesn't use, and letters run together, are none.
        assert_eq!((shade("X"), shade("BW"), shade("")), (None, None, None));
        // 25 colours, each its own letters, a light and a dark shade for most.
        let letters: Vec<&str> = shades().map(|(letters, ..)| letters).collect();
        assert_eq!(letters.len(), 25);
        assert_eq!(
            letters,
            [
                "LR", "R", "DR", "O", "DO", "LY", "Y", "DY", "LG", "G", "DG", "LB", "B", "DB", "LP", "P", "DP", "W", "LN", "N", "DN", "K", "LT", "T",
                "DT"
            ]
        );
        assert!(shades().all(|(letters, name, rgb)| shade(letters) == Some(rgb) && !name.is_empty()));
    }
}
