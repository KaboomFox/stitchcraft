//! A palette's threads as a GIMP palette (`.gpl`, `REQ-CLI-011`): the swatch library VectorCraft loads, so
//! that its Image Trace, with the Document Library palette, traces a picture in a machine's threads
//! (`docs/src/design/image-to-embroidery.md` › What VectorCraft does and what StitchCraft does).
//!
//! GIMP's palette is text: its header, its name, how many columns to show it in, comment lines, and a line for
//! each colour, its red, green and blue from 0 to 255 and its name after a tab. A machine's threads are the
//! entries that colour matching may choose ([`PaletteEntry::matchable`]): an entry a machine keeps for
//! something else, as Brother's appliqué steps are, is no thread to trace with. Each colour is its entry's
//! own, so a picture traced in them is written as those entries, and the file names each thread as the
//! machine shows it.

use std::fmt::Write as _;

use super::{Palette, PaletteEntry};

impl Palette {
    /// The palette's threads as a GIMP palette named after it, with `about` as a comment: each entry that
    /// colour matching may choose, in the palette's order.
    pub fn gpl(&self, about: &str) -> String {
        let one_line = |text: &str| text.split(['\n', '\r', '\t']).collect::<Vec<_>>().join(" ");
        let mut out = format!("GIMP Palette\nName: {}\nColumns: 8\n# {}\n", one_line(self.name), one_line(about));
        for entry in self.entries.iter().filter(|entry| entry.matchable) {
            let PaletteEntry { color, name, .. } = entry;
            let _ = writeln!(out, "{:3} {:3} {:3}\t{}", color.r, color.g, color.b, one_line(name));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::palette::BROTHER_PEC;
    use crate::thread::Rgb;

    #[test]
    fn req_cli_011_a_palettes_threads_are_a_gimp_palette_of_their_own_colours_and_names() {
        let text = BROTHER_PEC.gpl("The threads.");
        let mut lines = text.lines();
        assert_eq!(lines.by_ref().take(4).collect::<Vec<_>>(), ["GIMP Palette", "Name: Brother PEC", "Columns: 8", "# The threads."]);
        let colours: Vec<(Rgb, &str)> = lines
            .map(|line| {
                let (rgb, name) = line.split_once('\t').unwrap();
                let [r, g, b]: [u8; 3] = rgb.split_whitespace().map(|c| c.parse().unwrap()).collect::<Vec<_>>().try_into().unwrap();
                (Rgb::new(r, g, b), name)
            })
            .collect();
        // Every thread but the appliqué steps, 62 to 64, in the palette's order, each read back as its entry.
        assert_eq!(colours.len(), 61);
        assert_eq!(colours.first(), Some(&(Rgb::from_hex(0x0E1F7C), "Prussian Blue")));
        assert_eq!(text.lines().nth(4), Some(" 14  31 124\tPrussian Blue"));
        let entries = BROTHER_PEC.entries.iter().filter(|entry| entry.matchable);
        for (&(colour, name), entry) in colours.iter().zip(entries) {
            assert_eq!((colour, name), (entry.color, entry.name));
            assert_eq!(BROTHER_PEC.nearest(colour).map(|e| e.index), Some(entry.index), "{name}");
        }
        // A name or a comment on more than one line, or with a tab, stays on its own.
        assert!(BROTHER_PEC.gpl("Two\nlines\tand a tab").contains("\n# Two lines and a tab\n"));
    }
}
