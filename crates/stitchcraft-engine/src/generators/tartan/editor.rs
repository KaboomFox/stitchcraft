//! The editor for setts (milestone M10.7): a tartan's `tartan` setting made from its stripes, or from a code,
//! as Ink/Stitch's tartan editor makes and saves one (`docs/src/design/algorithms/tartan.md` › The editor,
//! read at `d59c9ab`).
//!
//! **Why an editor.** A sett is one setting, an object of JSON whose palette code lists the stripes, and it
//! is easy to get wrong written by hand. Ink/Stitch gives it an editor of its own, which makes the code from
//! stripes picked one by one, or reads a code pasted in: its own palette code, a threadcount of colour letters
//! and their counts, or the threadcount the Scottish Register of Tartans' search gives, with a palette of its
//! own. StitchCraft's editor does the same ([`Draft`]), so that VectorCraft's Tartan filter can set a sett.
//!
//! **What it reads.** A code reads as the fill reads it (`super::sett`), but that each thread of a threadcount
//! is as wide as the editor is told. Ink/Stitch's editor starts at 0.2 mm, where its fill reads a thread as
//! 0.5 mm. The code says whether the sett is mirrored, as it says so to Ink/Stitch's editor. A sett whose code
//! has the dots of a repeated one, `...`, repeats, and so does a register's threadcount that says `full sett`.
//! The others are mirrored.
//!
//! **What it writes** ([`Draft::code`], [`Draft::setting`]). The palette code as Ink/Stitch's editor writes it.
//! Each stripe is its colour in brackets, a `?` before the width of one left unsewn and a `*` before one sewn
//! in the colour before it, and its width in millimetres. The first stripe and the last are marked `/` when
//! the sett is mirrored, or the whole lies between `...` when it repeats, and a `|` parts the warp's stripes
//! from the weft's when they differ. The setting is the object of JSON Ink/Stitch's editor saves, with the keys
//! the fill reads and `equal_warp_weft`.

use super::sett::{self, Coded, Look, Stripe};

/// A sett as the editor makes it: its stripes and how the whole is laid.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    /// The warp's stripes, across the fill from left to right before it is turned.
    pub warp: Vec<Stripe>,
    /// The weft's stripes, down the fill from its top; `None` when they are the warp's.
    pub weft: Option<Vec<Stripe>>,
    /// Whether the sett is mirrored at its first and last stripes, and otherwise repeated.
    pub mirrored: bool,
    /// How far it is turned, in degrees, counter-clockwise in y-up axes (`rotate`).
    pub rotate: f64,
    /// How much its widths are scaled, in percent (`scale`).
    pub scale: f64,
    /// How far it is moved right and down, in millimetres (`offset_x`, `offset_y`).
    pub offset: [f64; 2],
    /// The width at or below which a stripe sews as a run, in millimetres (`min_stripe_width`).
    pub narrowest: f64,
}

/// What reading a code gave the editor: the sett, and the colours of the code that couldn't be read, each
/// left unsewn.
#[derive(Clone, Debug, PartialEq)]
pub struct Drafted {
    /// The sett, laid as Ink/Stitch's editor starts: not turned, moved or scaled, and with stripes of 1 mm or
    /// less sewn as runs.
    pub draft: Draft,
    /// The colours that couldn't be read, in the code's order.
    pub unread: Vec<String>,
}

impl Draft {
    /// The sett `code` gives, each thread of a threadcount `thread` mm wide, as the module's docs say; why
    /// not, when the code can't be read.
    pub fn from_code(code: &str, thread: f64) -> Result<Drafted, String> {
        let Coded { warp, weft, same, unread } = sett::coded(code, thread)?;
        let plain = code.replace('/', "").replace("...", "");
        let mirrored = if plain.contains("Threadcount") { !plain.contains("full sett") } else { !code.contains("...") };
        let draft = Draft { warp, weft: (!same).then_some(weft), mirrored, rotate: 0.0, scale: 100.0, offset: [0.0, 0.0], narrowest: 1.0 };
        Ok(Drafted { draft, unread })
    }

    /// The sett's palette code, as the module's docs say.
    pub fn code(&self) -> String {
        let mut parts: Vec<String> = self.warp.iter().map(written).collect();
        if let Some(weft) = &self.weft {
            parts.push("|".to_string());
            parts.extend(weft.iter().map(written));
        }
        if !self.mirrored {
            return format!("...{}...", parts.join(" "));
        }
        // The pivots, the first stripe and the last, each marked once.
        let last = parts.len().saturating_sub(1);
        for at in [0, last] {
            if let Some(part) = parts.get_mut(at).filter(|part| !part.contains(")/")) {
                *part = part.replacen(')', ")/", 1);
            }
        }
        parts.join(" ")
    }

    /// The `tartan` setting of the sett: an object of JSON as Ink/Stitch's editor saves it, its keys in the
    /// order that editor writes them. NaN and the infinities, which JSON can't write, are written as 0.
    pub fn setting(&self) -> String {
        let number = |n: f64| serde_json::Number::from_f64(n).map_or_else(|| "0".to_string(), |n| n.to_string());
        format!(
            "{{\"symmetry\": {}, \"equal_warp_weft\": {}, \"rotate\": {}, \"scale\": {}, \"offset_x\": {}, \"offset_y\": {}, \"palette\": {}, \
             \"min_stripe_width\": {}}}",
            self.mirrored,
            self.weft.is_none(),
            number(self.rotate),
            number(self.scale),
            number(self.offset[0]),
            number(self.offset[1]),
            serde_json::Value::from(self.code()),
            number(self.narrowest)
        )
    }
}

/// `stripe` as a palette code writes it: its colour in brackets, `#` and 6 hexadecimal digits, its mark and
/// its width in millimetres, to a millionth.
fn written(stripe: &Stripe) -> String {
    let mark = match stripe.look {
        Look::Sewn => "",
        Look::Gap => "?",
        Look::Continued => "*",
    };
    let width = format!("{:.6}", stripe.width);
    let width = width.trim_end_matches('0').trim_end_matches('.');
    format!("({}){mark}{width}", stripe.colour)
}

#[cfg(test)]
mod tests;
