//! Thread palettes and nearest-colour matching.
//!
//! Some formats store a palette index per colour block instead of a colour (PES stores Brother PEC
//! indices). Writing such a format means picking, for each thread, the palette entry that looks most like
//! it: the smallest CIEDE2000 difference ([`color`]), ties broken by the lower index so the choice never
//! depends on iteration order.

pub mod color;

mod brother_pec;

pub use brother_pec::BROTHER_PEC;

use crate::profile::PaletteId;
use crate::thread::Rgb;
use color::{Lab, ciede2000};

/// One palette entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteEntry {
    /// The index the format stores.
    pub index: u8,
    /// The thread's name, as the machine shows it.
    pub name: &'static str,
    /// The colour matching measures against.
    pub color: Rgb,
    /// Whether automatic matching may choose this entry (false for machine-special entries).
    pub matchable: bool,
}

/// A table of thread colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Which palette this is.
    pub id: PaletteId,
    /// Its name.
    pub name: &'static str,
    /// The entries, by ascending index.
    pub entries: &'static [PaletteEntry],
}

impl Palette {
    /// The matchable entry closest to `color` (CIEDE2000; lower index on ties), or `None` for a palette
    /// without matchable entries.
    pub fn nearest(&self, color: Rgb) -> Option<&'static PaletteEntry> {
        let target = Lab::from_rgb(color);
        let mut best: Option<(&'static PaletteEntry, f64)> = None;
        for entry in self.entries.iter().filter(|e| e.matchable) {
            let distance = ciede2000(target, Lab::from_rgb(entry.color));
            if best.is_none_or(|(_, d)| distance < d) {
                best = Some((entry, distance));
            }
        }
        best.map(|(entry, _)| entry)
    }

    /// The entry with `index`.
    pub fn entry(&self, index: u8) -> Option<&'static PaletteEntry> {
        self.entries.iter().find(|e| e.index == index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// REQ-THREAD-001: spot checks against the published table (pyembroidery's PEC thread set).
    #[test]
    fn brother_pec_matches_its_source_at_spot_checks() {
        let p = &BROTHER_PEC;
        assert_eq!(p.entries.len(), 64);
        for (index, name, rgb) in [
            (1, "Prussian Blue", 0x0E1F7C),
            (5, "Red", 0xED171F),
            (20, "Black", 0x000000),
            (29, "White", 0xF0F0F0),
            (37, "Leaf Green", 0x66BA49),
            (54, "Emerald Green", 0x00673E),
            (61, "Fresh Green", 0xE3F35B),
            (64, "Applique", 0xFFC8C8),
        ] {
            let entry = p.entry(index).unwrap();
            assert_eq!((entry.name, entry.color), (name, Rgb::from_hex(rgb)), "index {index}");
        }
        assert!(p.entries.windows(2).all(|w| w[0].index + 1 == w[1].index), "indices are 1..=64 in order");
        assert!(p.entry(0).is_none());
    }

    #[test]
    fn every_thread_colour_matches_itself() {
        for entry in BROTHER_PEC.entries.iter().filter(|e| e.matchable) {
            assert_eq!(BROTHER_PEC.nearest(entry.color).map(|e| e.index), Some(entry.index), "{}", entry.name);
        }
    }

    #[test]
    fn applique_entries_are_never_chosen() {
        let pink = BROTHER_PEC.nearest(Rgb::from_hex(0xFFC8C8)).unwrap();
        assert!(pink.index < 62, "chose {}", pink.name);
    }

    #[test]
    fn matching_is_perceptual_not_rgb_distance() {
        let name = |rgb| BROTHER_PEC.nearest(Rgb::from_hex(rgb)).map(|e| e.name);
        // Distance in RGB would pick "Red" (#ED171F) for pure red and "Deep Gold" for CSS orange. People
        // see Vermilion (ΔE00 3.4, against 5.0 for Red) and Pumpkin (4.4) as closer, and so does CIEDE2000.
        assert_eq!(name(0xFF0000), Some("Vermilion"));
        assert_eq!(name(0xFFA500), Some("Pumpkin"));
        assert_eq!(name(0x808080), Some("Gray"));
        assert_eq!(name(0xFFFFFF), Some("White"));
    }
}
