//! The colours a person names for a picture's trace (`REQ-AUTO-160`): `--keep-colour`, which gives a
//! colour a thread at any `--colours` (`docs/src/design/image-to-embroidery.md` › A picture in 6 threads).
//!
//! **Why a person names it.** Image Trace chooses a picture's threads by how much of it each would cover, so
//! a small bright colour loses its thread first. Which small colour matters is the person's to say: no rule
//! of the picture's measures chose it reliably on the varied set, as for marks.
//!
//! **A colour, 3 ways** ([`Colour`]). A thread of the machine by its name, in any case, as `stitch palette`
//! and the share list name it; a colour as `#rrggbb`, which is the machine's thread nearest it by the
//! palette's own matching ([`Palette::nearest`]); or a pixel of the picture as `X,Y`, from its left and top
//! edges, which is the thread nearest that pixel's colour, its alpha left aside. Each is read to one entry of
//! the machine's palette ([`chosen`]), before VectorCraft runs, so that a mistake is a usage error that
//! costs no trace. A colour given twice, or 2 colours with one nearest thread, keep that thread once.
//!
//! **A pixel needs the picture decoded.** `stitch` decodes pictures only with its `cutout` feature, which
//! brings in `image`. A build without it, such as the WASI one that makes the golden files, refuses a pixel
//! with a usage error that names the feature and the other 2 ways, rather than guess its colour.
//!
//! The choices are read here and not in the trace, so that `--drop-colour` (`REQ-AUTO-161`, planned) reads
//! its colours the same way.

use std::fmt;

use stitchcraft_plan::Rgb;
use stitchcraft_plan::palette::{Palette, PaletteEntry};

/// How many of the nearest names a name that is no thread is answered with.
const NEAREST_NAMES: usize = 3;
/// The longest name compared with the threads' names: a longer one is no thread and gets no nearest names.
const LONGEST_NAME: usize = 64;

/// A colour a person names: a thread, a colour or a pixel of the picture ([the module](self)).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Colour {
    /// A thread of the machine by its name, in any case.
    Thread(String),
    /// A colour, which is the machine's thread nearest it.
    Rgb(Rgb),
    /// A pixel of the picture, from its left and top edges, which is the thread nearest its colour.
    Pixel {
        /// Pixels from the left edge.
        x: u32,
        /// Pixels from the top edge.
        y: u32,
    },
}

impl fmt::Display for Colour {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Colour::Thread(name) => write!(f, "{name}"),
            Colour::Rgb(rgb) => write!(f, "{rgb}"),
            Colour::Pixel { x, y } => write!(f, "{x},{y}"),
        }
    }
}

/// `text` as a [`Colour`]: `#rrggbb`, `X,Y` or else a thread's name; or why it is none, as clap says it.
pub fn colour(text: &str) -> Result<Colour, String> {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix('#') {
        let rgb = (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())).then(|| u32::from_str_radix(hex, 16).ok()).flatten();
        return rgb.map(|rgb| Colour::Rgb(Rgb::from_hex(rgb))).ok_or_else(|| format!("`{text}` isn't a colour as #rrggbb"));
    }
    if let Some((x, y)) = text.split_once(',') {
        return match (x.trim().parse(), y.trim().parse()) {
            (Ok(x), Ok(y)) => Ok(Colour::Pixel { x, y }),
            _ => Err(format!("`{text}` isn't a pixel as X,Y, whole numbers from the picture's left and top edges")),
        };
    }
    if text.is_empty() {
        return Err("a colour is a thread's name, #rrggbb or a pixel X,Y".to_string());
    }
    Ok(Colour::Thread(text.to_string()))
}

/// Why the colours named can't be read: a usage error, which says why, before VectorCraft runs.
#[derive(Debug, PartialEq, Eq)]
pub enum Unchosen {
    /// The usage error.
    Usage(String),
}

/// The colour of the picture's pixel at `x`, `y`; or why it has none.
pub type Pixels<'a> = dyn FnMut(u32, u32) -> Result<Rgb, Unchosen> + 'a;

/// The entries of `palette` that `colours`, given with `option`, name, each once, in the order first named:
/// a pixel's colour from `pixel`. A name that is no thread of the palette is a usage error that gives the
/// nearest names.
pub fn chosen(colours: &[Colour], option: &str, palette: &Palette, pixel: &mut Pixels<'_>) -> Result<Vec<&'static PaletteEntry>, Unchosen> {
    let mut entries: Vec<&'static PaletteEntry> = Vec::new();
    for colour in colours {
        let entry = match colour {
            Colour::Thread(name) => {
                threads(palette).find(|entry| entry.name.eq_ignore_ascii_case(name)).ok_or_else(|| unknown(option, name, palette))
            }
            Colour::Rgb(rgb) => nearest(option, colour, *rgb, palette),
            Colour::Pixel { x, y } => nearest(option, colour, pixel(*x, *y)?, palette),
        }?;
        if !entries.iter().any(|kept| kept.index == entry.index) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

/// The palette's threads: the entries colour matching may choose, which `stitch palette` writes.
pub fn threads(palette: &Palette) -> impl Iterator<Item = &'static PaletteEntry> {
    palette.entries.iter().filter(|entry| entry.matchable)
}

/// The thread of `palette` nearest `rgb`, which `colour`, given with `option`, names.
fn nearest(option: &str, colour: &Colour, rgb: Rgb, palette: &Palette) -> Result<&'static PaletteEntry, Unchosen> {
    palette.nearest(rgb).ok_or_else(|| Unchosen::Usage(format!("{option} {colour}: {} has no threads to match a colour with", palette.name)))
}

/// The usage error for `name`, given with `option`, which is no thread of `palette`: with the nearest names.
fn unknown(option: &str, name: &str, palette: &Palette) -> Unchosen {
    let mut message = format!("{option} {name}: {} has no thread of that name", palette.name);
    if name.chars().count() <= LONGEST_NAME {
        let wanted = name.to_lowercase();
        let mut near: Vec<(usize, &str)> = threads(palette).map(|entry| (distance(&wanted, &entry.name.to_lowercase()), entry.name)).collect();
        // Nearest first; among equal ones, the palette's order, which a stable sort keeps.
        near.sort_by_key(|(distance, _)| *distance);
        near.dedup_by(|a, b| a.1 == b.1);
        let names: Vec<&str> = near.iter().take(NEAREST_NAMES).map(|(_, name)| *name).collect();
        if !names.is_empty() {
            message.push_str(&format!(". The nearest names are {}", names.join(", ")));
        }
    }
    Unchosen::Usage(message)
}

/// How many characters must be put in, taken out or changed to make `a` into `b` (Levenshtein's
/// distance). Both are at most [`LONGEST_NAME`] characters, or a thread's name.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row.first().copied().unwrap_or_default();
        if let Some(first) = row.first_mut() {
            *first = i + 1;
        }
        for (j, cb) in b.iter().enumerate() {
            let above = row.get(j + 1).copied().unwrap_or_default();
            let left = row.get(j).copied().unwrap_or_default();
            let changed = diagonal + usize::from(ca != *cb);
            diagonal = above;
            if let Some(cell) = row.get_mut(j + 1) {
                *cell = changed.min(above + 1).min(left + 1);
            }
        }
    }
    row.last().copied().unwrap_or_default()
}

/// The colours of the pixels of the picture `bytes`, decoded the first time one is asked for, as the
/// cut-out decodes it: a usage error, with `option`, when it can't be, and for a pixel outside it.
#[cfg(feature = "cutout")]
pub fn pixels<'a>(bytes: &'a [u8], option: &'a str) -> impl FnMut(u32, u32) -> Result<Rgb, Unchosen> + 'a {
    use stitchcraft_core::Budget;
    use stitchcraft_cutout::{Picture, Work};

    let mut decoded: Option<Picture> = None;
    move |x, y| {
        let picture = match &mut decoded {
            Some(picture) => picture,
            None => decoded.insert(
                Picture::decode(bytes, &mut Work::new(Budget::DEFAULT))
                    .map_err(|said| Unchosen::Usage(format!("{option} {x},{y}: the picture's pixels can't be read: {}", said.message)))?,
            ),
        };
        let (width, height) = (picture.width(), picture.height());
        let at = (x < width && y < height).then(|| (u64::from(y) * u64::from(width) + u64::from(x)) * 4);
        let pixel = at.and_then(|at| usize::try_from(at).ok()).and_then(|at| picture.rgba().get(at..at + 3));
        match pixel {
            Some(&[r, g, b]) => Ok(Rgb::new(r, g, b)),
            _ => Err(Unchosen::Usage(format!("{option} {x},{y} is outside the picture, which is {width} × {height} pixels"))),
        }
    }
}

/// A build without the `cutout` feature decodes no picture, so it reads no pixel: a usage error that says
/// so, with `option`.
#[cfg(not(feature = "cutout"))]
pub fn pixels<'a>(_: &'a [u8], option: &'a str) -> impl FnMut(u32, u32) -> Result<Rgb, Unchosen> + 'a {
    move |x, y| {
        Err(Unchosen::Usage(format!(
            "{option} {x},{y}: this stitch was built without its `cutout` feature, which decodes pictures, so it reads no pixel. Name \
             a thread or a colour #rrggbb instead"
        )))
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::palette::BROTHER_PEC;

    use super::*;

    fn names(colours: &[&str], pixel: &mut Pixels<'_>) -> Result<Vec<&'static str>, Unchosen> {
        let colours: Vec<Colour> = colours.iter().map(|text| colour(text).unwrap()).collect();
        chosen(&colours, "--keep-colour", &BROTHER_PEC, pixel).map(|entries| entries.iter().map(|entry| entry.name).collect())
    }

    fn usage(result: Result<Vec<&'static str>, Unchosen>) -> String {
        match result {
            Err(Unchosen::Usage(why)) => why,
            Ok(kept) => panic!("not a usage error: {kept:?}"),
        }
    }

    fn no_pixel(_: u32, _: u32) -> Result<Rgb, Unchosen> {
        panic!("no pixel is read")
    }

    #[test]
    fn req_auto_160_a_colour_is_a_thread_s_name_a_colour_or_a_pixel() {
        assert_eq!(colour("Dark Fuchsia"), Ok(Colour::Thread("Dark Fuchsia".to_string())));
        assert_eq!(colour(" #C70156 "), Ok(Colour::Rgb(Rgb::new(0xc7, 0x01, 0x56))));
        assert_eq!(colour("12, 40"), Ok(Colour::Pixel { x: 12, y: 40 }));
        for wrong in ["#c7015", "#c70156a", "#c7015g", "#"] {
            assert_eq!(colour(wrong), Err(format!("`{wrong}` isn't a colour as #rrggbb")));
        }
        for wrong in ["12,", "-1,4", "a,b", "1.5,2"] {
            assert!(colour(wrong).unwrap_err().contains("isn't a pixel as X,Y"), "{wrong}");
        }
        assert!(colour("  ").is_err());
        let shown: Vec<String> = ["Red", "#c70156", "3,4"].iter().map(|text| colour(text).unwrap().to_string()).collect();
        assert_eq!(shown, ["Red", "#c70156", "3,4"]);
    }

    #[test]
    fn req_auto_160_each_colour_is_the_machine_s_thread_it_names_or_is_nearest_once() {
        let kept = names(&["dark fuchsia", "DARK FUCHSIA", "#c70156", "#ffffff", "White"], &mut no_pixel).unwrap();
        assert_eq!(kept, ["Dark Fuchsia", "White"]);
        let mut pixel = |x: u32, y: u32| Ok(if (x, y) == (1, 2) { Rgb::new(0xed, 0x17, 0x1f) } else { Rgb::new(0, 0, 0) });
        assert_eq!(names(&["1,2", "0,0"], &mut pixel).unwrap(), ["Red", "Black"]);
        // A pixel's error is passed on as it is.
        let mut outside = |_: u32, _: u32| Err(Unchosen::Usage("outside".to_string()));
        assert_eq!(usage(names(&["Red", "9,9"], &mut outside)), "outside");
    }

    #[test]
    fn req_auto_160_a_name_that_is_no_thread_is_answered_with_the_nearest_names() {
        assert_eq!(
            usage(names(&["Dark Fucshia"], &mut no_pixel)),
            "--keep-colour Dark Fucshia: Brother PEC has no thread of that name. The nearest names are Dark Fuchsia, Dark Gray, Dark Brown"
        );
        let long = "x".repeat(LONGEST_NAME + 1);
        assert_eq!(usage(names(&[long.as_str()], &mut no_pixel)), format!("--keep-colour {long}: Brother PEC has no thread of that name"));
        // At the longest, it still gets the nearest names.
        let longest = "x".repeat(LONGEST_NAME);
        assert!(usage(names(&[longest.as_str()], &mut no_pixel)).contains("The nearest names are"));
    }

    #[test]
    fn req_auto_160_the_distance_between_names_counts_each_change() {
        assert_eq!(distance("", ""), 0);
        assert_eq!(distance("red", "red"), 0);
        assert_eq!(distance("", "red"), 3);
        assert_eq!(distance("red", ""), 3);
        assert_eq!(distance("kitten", "sitting"), 3);
        assert_eq!(distance("fucshia", "fuchsia"), 2);
        assert_eq!(distance("ab", "ba"), 2);
        assert_eq!(distance("abc", "abd"), 1);
        assert_eq!(distance("abc", "abcd"), 1);
    }

    #[cfg(feature = "cutout")]
    #[test]
    fn req_auto_160_a_pixel_is_read_from_the_picture_and_one_outside_it_is_a_usage_error() {
        let mut png = Vec::new();
        let image = image::RgbaImage::from_fn(3, 2, |x, y| image::Rgba([u8::try_from(x * 100).unwrap(), u8::try_from(y * 200).unwrap(), 7, 255]));
        image.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let mut pixel = pixels(&png, "--keep-colour");
        assert_eq!(pixel(0, 0).unwrap(), Rgb::new(0, 0, 7));
        assert_eq!(pixel(2, 1).unwrap(), Rgb::new(200, 200, 7));
        for (x, y) in [(3, 0), (0, 2)] {
            match pixel(x, y) {
                Err(Unchosen::Usage(why)) => assert_eq!(why, format!("--keep-colour {x},{y} is outside the picture, which is 3 × 2 pixels")),
                other => panic!("{other:?}"),
            }
        }
        let mut unreadable = pixels(b"not a picture", "--keep-colour");
        let Err(Unchosen::Usage(why)) = unreadable(0, 0) else { panic!("read") };
        assert!(why.starts_with("--keep-colour 0,0: the picture's pixels can't be read: "), "{why}");
    }

    #[cfg(not(feature = "cutout"))]
    #[test]
    fn req_auto_160_a_build_without_the_cut_out_reads_no_pixel() {
        let mut pixel = pixels(b"", "--keep-colour");
        match pixel(3, 4) {
            Err(Unchosen::Usage(why)) => assert!(why.starts_with("--keep-colour 3,4: this stitch was built without its `cutout` feature"), "{why}"),
            other => panic!("{other:?}"),
        }
    }
}
