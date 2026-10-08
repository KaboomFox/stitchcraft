//! Threads and their colours.
//!
//! A thread is what the operator puts on the machine for one colour block. Its colour is plain sRGB:
//! formats that store palette indices (PES) map it to the nearest palette entry when writing
//! ([`crate::palette`]), and the thread chart in reports lists both, so users see the colour they chose
//! next to the colour their machine will show (Ink/Stitch #2668).

use core::fmt;

/// An sRGB colour with 8 bits per channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

impl Rgb {
    /// The colour (`r`, `g`, `b`).
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    /// The colour written as `0xRRGGBB` (bits above 24 are ignored).
    pub const fn from_hex(rgb: u32) -> Self {
        let [_, r, g, b] = rgb.to_be_bytes();
        Rgb { r, g, b }
    }
}

/// `#rrggbb`, as in CSS and SVG.
impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// A thread for one colour block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thread {
    /// The colour the user chose.
    pub color: Rgb,
    /// A name to show the operator, when there is one ("Red", "Isacord 1902").
    pub name: Option<String>,
}

impl Thread {
    /// A thread of `color` with no name.
    pub const fn new(color: Rgb) -> Self {
        Thread { color, name: None }
    }

    /// A thread of `color` named `name`.
    pub fn named(color: Rgb, name: impl Into<String>) -> Self {
        Thread { color, name: Some(name.into()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_through_display() {
        let c = Rgb::from_hex(0x0A55A3);
        assert_eq!(c, Rgb::new(0x0A, 0x55, 0xA3));
        assert_eq!(c.to_string(), "#0a55a3");
        assert_eq!(Rgb::from_hex(0xFF_FF_FF_FF), Rgb::new(255, 255, 255));
    }
}
