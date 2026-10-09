//! The Brother PEC palette: the 64 thread colours a PES file's PEC block refers to by index.
//!
//! Source: the PEC thread table as published in pyembroidery (MIT licence), read through the library's
//! public API, not copied from its source. Entries 62 and 63 follow its fork pystitch instead (MIT,
//! `main` at `b72b557`, pull request 140), where pyembroidery 1.5.1 lists two thread colours. `NOTICE`
//! records the origin. A machine shows its own rendering of the named thread for an index; these RGB
//! values are what matching measures against. Index 0 is unused by the format.
//!
//! Entries 62–64 are not threads: Brother machines treat them as applique steps ("Applique Material",
//! "Applique Position", "Applique"), so automatic colour matching never picks them.

use crate::palette::{Palette, PaletteEntry};
use crate::profile::PaletteId;
use crate::thread::Rgb;

const fn e(index: u8, name: &'static str, rgb: u32, matchable: bool) -> PaletteEntry {
    PaletteEntry { index, name, color: Rgb::from_hex(rgb), matchable }
}

/// The Brother PEC palette.
pub static BROTHER_PEC: Palette = Palette {
    id: PaletteId::BrotherPec,
    name: "Brother PEC",
    entries: &[
        e(1, "Prussian Blue", 0x0E1F7C, true),
        e(2, "Blue", 0x0A55A3, true),
        e(3, "Teal Green", 0x008777, true),
        e(4, "Cornflower Blue", 0x4B6BAF, true),
        e(5, "Red", 0xED171F, true),
        e(6, "Reddish Brown", 0xD15C00, true),
        e(7, "Magenta", 0x913697, true),
        e(8, "Light Lilac", 0xE49ACB, true),
        e(9, "Lilac", 0x915FAC, true),
        e(10, "Mint Green", 0x9ED67D, true),
        e(11, "Deep Gold", 0xE8A900, true),
        e(12, "Orange", 0xFEBA35, true),
        e(13, "Yellow", 0xFFFF00, true),
        e(14, "Lime Green", 0x70BC1F, true),
        e(15, "Brass", 0xBA9800, true),
        e(16, "Silver", 0xA8A8A8, true),
        e(17, "Russet Brown", 0x7D6F00, true),
        e(18, "Cream Brown", 0xFFFFB3, true),
        e(19, "Pewter", 0x4F5556, true),
        e(20, "Black", 0x000000, true),
        e(21, "Ultramarine", 0x0B3D91, true),
        e(22, "Royal Purple", 0x770176, true),
        e(23, "Dark Gray", 0x293133, true),
        e(24, "Dark Brown", 0x2A1301, true),
        e(25, "Deep Rose", 0xF64A8A, true),
        e(26, "Light Brown", 0xB27624, true),
        e(27, "Salmon Pink", 0xFCBBC5, true),
        e(28, "Vermilion", 0xFE370F, true),
        e(29, "White", 0xF0F0F0, true),
        e(30, "Violet", 0x6A1C8A, true),
        e(31, "Seacrest", 0xA8DDC4, true),
        e(32, "Sky Blue", 0x2584BB, true),
        e(33, "Pumpkin", 0xFEB343, true),
        e(34, "Cream Yellow", 0xFFF36B, true),
        e(35, "Khaki", 0xD0A660, true),
        e(36, "Clay Brown", 0xD15400, true),
        e(37, "Leaf Green", 0x66BA49, true),
        e(38, "Peacock Blue", 0x134A46, true),
        e(39, "Gray", 0x878787, true),
        e(40, "Warm Gray", 0xD8CCC6, true),
        e(41, "Dark Olive", 0x435607, true),
        e(42, "Flesh Pink", 0xFDD9DE, true),
        e(43, "Pink", 0xF993BC, true),
        e(44, "Deep Green", 0x003822, true),
        e(45, "Lavender", 0xB2AFD4, true),
        e(46, "Wisteria Violet", 0x686AB0, true),
        e(47, "Beige", 0xEFE3B9, true),
        e(48, "Carmine", 0xF73866, true),
        e(49, "Amber Red", 0xB54B64, true),
        e(50, "Olive Green", 0x132B1A, true),
        e(51, "Dark Fuchsia", 0xC70156, true),
        e(52, "Tangerine", 0xFE9E32, true),
        e(53, "Light Blue", 0xA8DEEB, true),
        e(54, "Emerald Green", 0x00673E, true),
        e(55, "Purple", 0x4E2990, true),
        e(56, "Moss Green", 0x2F7E20, true),
        e(57, "Flesh Pink", 0xFFCCCC, true),
        e(58, "Harvest Gold", 0xFFD911, true),
        e(59, "Electric Blue", 0x095BA6, true),
        e(60, "Lemon Yellow", 0xF0F970, true),
        e(61, "Fresh Green", 0xE3F35B, true),
        e(62, "Applique Material", 0xFFC864, false),
        e(63, "Applique Position", 0xFFC8C8, false),
        e(64, "Applique", 0xFFC8C8, false),
    ],
};
