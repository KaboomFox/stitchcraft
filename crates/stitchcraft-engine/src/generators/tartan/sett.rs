//! A tartan's sett, read from Ink/Stitch's `tartan` setting, and the sequence of stripes it lays across a
//! fill, as Ink/Stitch reads and lays it (`docs/src/design/algorithms/tartan.md` › The sett, read at
//! `d59c9ab`).
//!
//! **Why a setting of JSON.** Ink/Stitch's tartan editor saves the sett as one attribute of the shape, an
//! object of JSON: its palette code, which gives each stripe a colour and a width, and how the whole sett is
//! scaled, turned, moved and ended. Its files and the 5 fonts of its library that sew tartans count on that
//! form, so StitchCraft reads it as it is, keys StitchCraft doesn't use included, and the attribute is one
//! parameter of the registry.
//!
//! **The palette code.** Each stripe is a colour in brackets, `(#ff0000)` or `(navy)`, then its width in
//! millimetres. A `?` before the width leaves the stripe unsewn, and a `*` sews it in the colour of the last
//! stripe sewn as a fill before it. A `|` parts the warp's stripes from the weft's, and without one the weft is
//! the warp. Slashes, which mark a sett's pivots, and the dots of a repeated sett are passed over, and so is
//! any text between stripes: Ink/Stitch writes a repeated sett's code as a list, `...['(#ff0000)5.0',
//! '(#00ff00)2.0']...`, which reads the same. Whether the sett is mirrored is its `symmetry` setting, not its
//! code. A colour that can't be read leaves its stripe unsewn (`SC-W0328`). Ink/Stitch's other codes, a
//! threadcount from the Scottish Register of Tartans or a code of colour letters, come with M10.7's editor,
//! and until then the sett can't be read (`SC-E0323`).
//!
//! **The full sett** ([`Sett::full`]). A stripe no wider than `min_stripe_width`, once scaled, sews as a run
//! along its middle, and its width goes to the stripes beside it, half before and half after. As Ink/Stitch
//! lays them, the half after it widens each stripe that follows, until a stripe left unsewn or one sewn in the
//! colour before it takes it last, or the next run gives its own half instead. A run at the end leaves its
//! half as a stripe of its own. A mirrored sett then goes back from its last step but one to its second, so
//! that the 2 it turns at are sewn once, and the full sett repeats.

use serde_json::{Map, Value};
use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_plan::Rgb;

/// The sett Ink/Stitch sews for a tartan without one: stripes 5 mm wide of a thread almost black, 5 mm left
/// unsewn between them, mirrored at both ends.
pub(crate) const DEFAULT_PALETTE: &str = "(#101010)/5.0 (#FFFFFF)/?5.0";

/// How a stripe of a palette code is sewn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Look {
    /// In its colour.
    Sewn,
    /// Not at all: a `?` before its width, or a colour that can't be read.
    Gap,
    /// In the colour of the last stripe sewn as a fill before it: a `*` before its width.
    Continued,
}

/// A stripe as its palette code gives it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stripe {
    /// Its colour; black for a colour that can't be read.
    pub(crate) colour: Rgb,
    /// Its width in millimetres, before the sett is scaled.
    pub(crate) width: f64,
    /// How it is sewn.
    pub(crate) look: Look,
}

/// A tartan's sett, as its setting gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct Sett {
    /// The warp's stripes, across the fill from left to right before it is turned.
    pub(crate) warp: Vec<Stripe>,
    /// The weft's, down it from the top: the warp's, without a `|` in the code.
    pub(crate) weft: Vec<Stripe>,
    /// How far the sett is turned about the middle of the part's bounds, in degrees, counter-clockwise in
    /// y-up axes as Ink/Stitch's geometry library turns it.
    pub(crate) rotate: f64,
    /// How much its widths are scaled, in percent.
    pub(crate) scale: f64,
    /// How far its first stripes lie to the left and above where they start, in millimetres: the
    /// offsets' sizes, as Ink/Stitch takes them.
    pub(crate) offset: [f64; 2],
    /// Whether it is mirrored at its ends, and otherwise repeated.
    pub(crate) mirrored: bool,
    /// The width at or below which a stripe, once scaled, sews as a run, in millimetres.
    pub(crate) narrowest: f64,
    /// How wide the wider direction's stripes are as the code gives them, scaled ([`Sett::width`]), which
    /// widens the box the stripes are laid over: measured before a direction that sews nothing is emptied.
    pub(crate) margin: f64,
}

/// What reading a setting gave: the sett, and the colours of its code that couldn't be read, each left
/// unsewn.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Read {
    /// The sett.
    pub(crate) sett: Sett,
    /// The colours that couldn't be read, in the code's order.
    pub(crate) unread: Vec<String>,
}

/// Ink/Stitch's pixels in a millimetre, by which it compares a stripe with the narrowest.
const PX_PER_MM: f64 = 96.0 / 25.4;

/// The sett Ink/Stitch's `tartan` setting `text` gives, or why it can't be read: an empty text gives
/// Ink/Stitch's default sett ([`DEFAULT_PALETTE`]). A setting the object leaves out takes the value Ink/Stitch's
/// editor starts from: no turn or offset, mirrored, at 100 %, and stripes of 1 mm or less sewn as runs.
pub(crate) fn read(text: &str) -> Result<Read, String> {
    if text.trim().is_empty() {
        return palette(DEFAULT_PALETTE, Settings::DEFAULT);
    }
    let value: Value = serde_json::from_str(text).map_err(|_| "it isn't an object of JSON".to_string())?;
    let Value::Object(object) = value else { return Err("it isn't an object of JSON".to_string()) };
    let code = match object.get("palette") {
        Some(Value::String(code)) => code.clone(),
        Some(_) => return Err("its `palette` isn't a text".to_string()),
        None => return Err("it has no `palette`".to_string()),
    };
    let defaults = Settings::DEFAULT;
    let settings = Settings {
        rotate: number(&object, "rotate", defaults.rotate)?,
        scale: number(&object, "scale", defaults.scale)?,
        offset: [number(&object, "offset_x", 0.0)?.abs(), number(&object, "offset_y", 0.0)?.abs()],
        mirrored: truth(&object, "symmetry", defaults.mirrored)?,
        narrowest: number(&object, "min_stripe_width", defaults.narrowest)?,
    };
    if settings.scale <= 0.0 {
        return Err(format!("its `scale` is {}, and a sett is scaled by more than 0 %", settings.scale));
    }
    palette(&code, settings)
}

/// A sett's settings but its code.
#[derive(Clone, Copy, Debug)]
struct Settings {
    rotate: f64,
    scale: f64,
    offset: [f64; 2],
    mirrored: bool,
    narrowest: f64,
}

impl Settings {
    /// Ink/Stitch's, for a tartan without a sett.
    const DEFAULT: Settings = Settings { rotate: 0.0, scale: 100.0, offset: [0.0, 0.0], mirrored: true, narrowest: 1.0 };
}

/// The number `key` of `object`, `default` when it is missing; why not when it is no number.
fn number(object: &Map<String, Value>, key: &str, default: f64) -> Result<f64, String> {
    match object.get(key) {
        None => Ok(default),
        Some(value) => value.as_f64().filter(|n| n.is_finite()).ok_or_else(|| format!("its `{key}` isn't a number")),
    }
}

/// Whether `key` of `object` is on, `default` when it is missing, as Ink/Stitch reads a value: `true`, or a
/// number other than 0. Why not when it is something else, such as a text, which Ink/Stitch would read as on
/// whatever it says.
fn truth(object: &Map<String, Value>, key: &str, default: bool) -> Result<bool, String> {
    match object.get(key) {
        None => Ok(default),
        Some(Value::Bool(on)) => Ok(*on),
        Some(Value::Null) => Ok(false),
        Some(value) => value.as_f64().map(|n| n != 0.0).ok_or_else(|| format!("its `{key}` isn't on or off")),
    }
}

/// The sett palette code `code` gives with `settings`.
fn palette(code: &str, settings: Settings) -> Result<Read, String> {
    let same = !code.contains('|');
    let code = code.replace('/', "").replace("...", "");
    if code.contains("Threadcount") {
        return Err("its code is a threadcount, which StitchCraft reads from M10.7".to_string());
    }
    let mut unread = Vec::new();
    let (warp, weft) = if code.contains('(') {
        let mut directions = code.split('|');
        let warp = directions.next().map(|warp| stripes(warp, &mut unread)).transpose()?.unwrap_or_default();
        let weft = directions.next().map(|weft| stripes(weft, &mut unread)).transpose()?;
        if directions.next().is_some() {
            return Err("its code has more than one `|`".to_string());
        }
        let weft = weft.unwrap_or_else(|| warp.clone());
        (warp, weft)
    } else if lettered(&code) {
        return Err("its code is one of colour letters, which StitchCraft reads from M10.7".to_string());
    } else {
        (Vec::new(), Vec::new())
    };
    let Settings { rotate, scale, offset, mirrored, narrowest } = settings;
    let mut sett = Sett { warp, weft, rotate, scale, offset, mirrored, narrowest, margin: 0.0 };
    sett.margin = sett.width(&sett.warp).max(sett.width(&sett.weft));
    // Each direction sews only with a width and a stripe sewn in its own colour, and without a `|` the weft
    // is the warp as it sews.
    let sewn = |stripes: &[Stripe]| stripes.iter().any(|s| s.look == Look::Sewn) && sett.width(stripes) != 0.0;
    let warp = if sewn(&sett.warp) { sett.warp.clone() } else { Vec::new() };
    let weft = if same {
        warp.clone()
    } else if sewn(&sett.weft) {
        sett.weft.clone()
    } else {
        Vec::new()
    };
    Ok(Read { sett: Sett { warp, weft, ..sett }, unread })
}

/// Whether `code`, which has no brackets, is a code of colour letters, each followed by its count of
/// threads, as Ink/Stitch reads one: `B24 W4 R2`.
fn lettered(code: &str) -> bool {
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        if !c.is_ascii_alphabetic() {
            continue;
        }
        while chars.next_if(char::is_ascii_alphabetic).is_some() {}
        let _ = chars.next_if(|c| matches!(c, '?' | '*'));
        if chars.peek().is_some_and(|c| c.is_ascii_digit() || *c == '.') {
            return true;
        }
    }
    false
}

/// The stripes of one direction of a palette code, each a colour in brackets, a `?` or a `*` or neither, then
/// its width, as Ink/Stitch finds them: wherever they lie in the text, the text between them passed over.
/// The colours that can't be read are added to `unread`; why not, when a width isn't a number.
fn stripes(code: &str, unread: &mut Vec<String>) -> Result<Vec<Stripe>, String> {
    let mut found = Vec::new();
    let mut rest = code;
    while let Some(open) = rest.find('(') {
        let after = rest.get(open + 1..).unwrap_or_default();
        let Some((stripe, tail)) = stripe(after) else {
            rest = after;
            continue;
        };
        let (name, look, width) = stripe;
        let width: f64 = width.parse().map_err(|_| format!("the width `{width}` of a stripe isn't a number"))?;
        let colour = colour(name);
        if colour.is_none() {
            unread.push(name.to_string());
        }
        let look = if colour.is_none() { Look::Gap } else { look };
        found.push(Stripe { colour: colour.unwrap_or(Rgb::new(0, 0, 0)), width, look });
        rest = tail;
    }
    Ok(found)
}

/// The stripe that `text`, which follows a `(`, starts with: its colour's name, how it is sewn and its width as
/// written, and the text after it; `None` when `text` starts with no stripe.
fn stripe(text: &str) -> Option<((&str, Look, &str), &str)> {
    let name_end = text.find(|c: char| !(c.is_ascii_alphanumeric() || c == '#')).unwrap_or(text.len());
    let (name, after) = text.split_at(name_end);
    let after = after.strip_prefix(')').filter(|_| !name.is_empty())?;
    let (look, after) = match after.chars().next() {
        Some('?') => (Look::Gap, after.get(1..)?),
        Some('*') => (Look::Continued, after.get(1..)?),
        _ => (Look::Sewn, after),
    };
    let width_end = after.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(after.len());
    let (width, tail) = after.split_at(width_end);
    (!width.is_empty()).then_some(((name, look, width), tail))
}

/// The colour `name` names: `#` and 3 or 6 hexadecimal digits, or a colour name of SVG. `None` for any other,
/// and for one that shows nothing.
fn colour(name: &str) -> Option<Rgb> {
    let parsed: svgtypes::Color = name.parse().ok()?;
    (parsed.alpha == 255).then_some(Rgb::new(parsed.red, parsed.green, parsed.blue))
}

/// One step of the full sett: a band of a colour, or of none, which it leaves unsewn, and how wide in
/// millimetres; or a run along the line between 2 bands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Item {
    /// A band.
    Band {
        /// Its colour; `None` for a band left unsewn.
        colour: Option<Rgb>,
        /// Its width.
        width: f64,
    },
    /// A run, in its colour.
    Run(Rgb),
}

impl Sett {
    /// How wide `stripes` are once scaled, in millimetres, as Ink/Stitch measures a palette: the stripes as
    /// wide as the narrowest or wider, but those sewn in the colour before them, or else the narrower ones
    /// that are sewn. It sizes the margin round the part ([`super::stripes`]), and a direction measuring 0
    /// sews nothing.
    pub(crate) fn width(&self, stripes: &[Stripe]) -> f64 {
        let (mut wide, mut narrow) = (0.0, 0.0);
        for stripe in stripes {
            let width = stripe.width * (self.scale / 100.0);
            if width >= self.narrowest && stripe.look != Look::Continued {
                wide += width;
            } else if width < self.narrowest && stripe.look != Look::Gap {
                narrow += width;
            }
        }
        if wide == 0.0 { narrow } else { wide }
    }

    /// The full sett of `stripes` ([`Item`]s), scaled, its narrow stripes runs, mirrored or not, as the
    /// module's docs say. Empty for no stripes.
    pub(crate) fn full(&self, stripes: &[Stripe]) -> Vec<Item> {
        let mut last = self.first_fill(stripes);
        let (mut carry, mut first_run, mut last_run) = (0.0, false, false);
        let mut items = Vec::with_capacity(2 * stripes.len() + 1);
        for stripe in stripes {
            let width = stripe.width * (self.scale / 100.0);
            match stripe.look {
                Look::Gap => {
                    items.push(Item::Band { colour: None, width: width + carry });
                    (last, carry, last_run) = (None, 0.0, false);
                }
                Look::Continued => {
                    items.push(Item::Band { colour: last, width: width + carry });
                    (carry, last_run) = (0.0, false);
                }
                Look::Sewn if self.runs(stripe) => {
                    first_run |= items.is_empty();
                    let half = width / 2.0;
                    items.extend([Item::Band { colour: last, width: half + carry }, Item::Run(stripe.colour)]);
                    (carry, last_run) = (half, true);
                }
                Look::Sewn => {
                    items.push(Item::Band { colour: Some(stripe.colour), width: width + carry });
                    (last, last_run) = (Some(stripe.colour), false);
                }
            }
        }
        if carry > 0.0 {
            items.push(Item::Band { colour: last, width: carry });
        }
        if self.mirrored {
            // Back from the last step but one to the second, so the 2 it turns at are sewn once, and a run
            // it turns at too.
            let inner = items.get(1..items.len().saturating_sub(1)).unwrap_or_default();
            let mut back: Vec<Item> = inner.iter().rev().copied().collect();
            if first_run {
                back.pop();
            }
            if last_run && !back.is_empty() {
                back.remove(0);
            }
            items.extend(back);
        }
        items
    }

    /// Whether `stripe`, once scaled, is no wider than the narrowest, and sews as a run: compared in
    /// Ink/Stitch's pixels, as it compares them, so that a stripe at the limit falls the same side.
    fn runs(&self, stripe: &Stripe) -> bool {
        stripe.width * PX_PER_MM * (self.scale / 100.0) <= self.narrowest * PX_PER_MM
    }

    /// The colour a sett starts its runs between before any stripe sewn as a fill: of the stripes wider than
    /// the narrowest and those left unsewn, which have none, the first's for a mirrored sett, as it goes back
    /// to it, and the last's for a repeated one, as the pass before ends with it.
    fn first_fill(&self, stripes: &[Stripe]) -> Option<Rgb> {
        let fills = stripes.iter().filter_map(|stripe| match stripe.look {
            Look::Gap => Some(None),
            Look::Continued => None,
            Look::Sewn => (stripe.width * (self.scale / 100.0) > self.narrowest).then_some(Some(stripe.colour)),
        });
        let mut fills = fills.peekable();
        if self.mirrored { fills.next().flatten() } else { fills.last().flatten() }
    }
}

/// Ink/Stitch's pixel, in millimetres, for the lengths it measures in pixels: a stretch of row shorter than
/// half a pixel is left out ([`super::stripes`]).
pub(crate) const PX: f64 = MM_PER_SVG_PX;

#[cfg(test)]
mod tests;
