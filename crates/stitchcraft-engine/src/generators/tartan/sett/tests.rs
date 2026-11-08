//! The sett's own cases: the setting read, its palette code's stripes, the widths measured, and the full
//! sett laid with its runs, mirrored or repeated.

use super::*;

const RED: Rgb = Rgb::new(255, 0, 0);
const GREEN: Rgb = Rgb::new(0, 255, 0);
const BLUE: Rgb = Rgb::new(0, 0, 255);
const WHITE: Rgb = Rgb::new(255, 255, 255);
const BLACK: Rgb = Rgb::new(0, 0, 0);

/// The sett of the setting whose palette code is `palette`, with the other settings `more` gives.
fn sett(palette: &str, more: &str) -> Sett {
    read(&format!(r#"{{"palette": "{palette}"{more}}}"#)).unwrap().sett
}

fn band(colour: Rgb, width: f64) -> Item {
    Item::Band { colour: Some(colour), width }
}

fn gap(width: f64) -> Item {
    Item::Band { colour: None, width }
}

fn stripe(colour: Rgb, width: f64, look: Look) -> Stripe {
    Stripe { colour, width, look }
}

#[test]
fn req_fill_trt_001_an_empty_setting_is_ink_stitch_s_default_sett() {
    let read = read("  ").unwrap();
    let near_black = Rgb::new(0x10, 0x10, 0x10);
    let stripes = [stripe(near_black, 5.0, Look::Sewn), stripe(WHITE, 5.0, Look::Gap)];
    assert_eq!(read.sett.warp, stripes);
    assert_eq!(read.sett.weft, stripes);
    assert_eq!((read.sett.rotate, read.sett.scale, read.sett.offset, read.sett.mirrored, read.sett.narrowest), (0.0, 100.0, [0.0, 0.0], true, 1.0));
    // The stripe left unsewn counts in the width, 10 mm, as Ink/Stitch measures a palette.
    assert_eq!((read.sett.margin, read.unread.len()), (10.0, 0));
    assert_eq!(read.sett.full(&read.sett.warp), [band(near_black, 5.0), gap(5.0)]);
}

#[test]
fn req_fill_trt_001_a_setting_that_can_t_be_read_says_why() {
    let why = |text: &str| read(text).unwrap_err();
    assert_eq!(why("{"), "it isn't an object of JSON");
    assert_eq!(why("[1]"), "it isn't an object of JSON");
    assert_eq!(why("{}"), "it has no `palette`");
    assert_eq!(why(r#"{"palette": 3}"#), "its `palette` isn't a text");
    assert_eq!(why(r#"{"palette": "", "rotate": "x"}"#), "its `rotate` isn't a number");
    assert_eq!(why(r#"{"palette": "", "offset_y": null}"#), "its `offset_y` isn't a number");
    assert_eq!(why(r#"{"palette": "", "symmetry": "yes"}"#), "its `symmetry` isn't on or off");
    assert_eq!(why(r#"{"palette": "", "scale": 0}"#), "its `scale` is 0, and a sett is scaled by more than 0 %");
    assert_eq!(why(r#"{"palette": "", "scale": -50}"#), "its `scale` is -50, and a sett is scaled by more than 0 %");
    assert_eq!(why(r#"{"palette": "(#ff0000)5 | (#00ff00)5 | (#0000ff)5"}"#), "its code has more than one `|`");
    assert_eq!(why(r#"{"palette": "B24 W1.2.3"}"#), "the count `1.2.3` of a stripe isn't a number");
    assert_eq!(why("{\"palette\": \"Threadcount:\\nB24 W\"}"), "the letters `W` of its threadcount have no count");
    assert_eq!(why(r#"{"palette": "(#ff0000)1.2.3"}"#), "the width `1.2.3` of a stripe isn't a number");
    assert_eq!(why(r#"{"palette": "(#ff0000)5 | (#00ff00)1.2.3"}"#), "the width `1.2.3` of a stripe isn't a number");
}

#[test]
fn req_fill_trt_001_the_setting_s_other_keys_are_read_as_ink_stitch_reads_them() {
    let read = sett(
        "(#ff0000)/5 (#0000ff)/5",
        r#", "rotate": 30, "scale": 150, "offset_x": -3, "offset_y": 1.5, "symmetry": false, "min_stripe_width": 0.5"#,
    );
    // The offsets by their sizes, the widths scaled for the margin.
    assert_eq!((read.rotate, read.scale, read.offset, read.mirrored, read.narrowest, read.margin), (30.0, 150.0, [3.0, 1.5], false, 0.5, 15.0));
    // A switch is on for a number other than 0, off for null or 0, and the editor's keys StitchCraft doesn't use
    // are passed over.
    assert!(sett("(#ff0000)5", r#", "symmetry": 1"#).mirrored);
    assert!(!sett("(#ff0000)5", r#", "symmetry": 0"#).mirrored);
    assert!(!sett("(#ff0000)5", r#", "symmetry": null"#).mirrored);
    assert!(sett("(#ff0000)5", r#", "equal_warp_weft": true, "output": "embroidery""#).mirrored);
}

#[test]
fn req_fill_trt_001_the_palette_code_gives_each_stripe_its_colour_mark_and_width() {
    // Slashes and the dots of a repeated sett are passed over, and the list Ink/Stitch's editor writes reads as
    // its stripes.
    let listed = sett("...['(#ff0000)5.0', '(#000000)1.5', '(#ff0000)2.0', '(#ffffff)2.0']...", "");
    let widths: Vec<(Rgb, f64)> = listed.warp.iter().map(|s| (s.colour, s.width)).collect();
    assert_eq!(widths, [(RED, 5.0), (BLACK, 1.5), (RED, 2.0), (WHITE, 2.0)]);
    // A `?` leaves the stripe unsewn and a `*` sews it in the colour before; colours by name or in 3 digits.
    let marked = sett("(#f00)/4 (navy)?2 (lime)*3 (#0000FF)/1", "");
    let navy = Rgb::new(0, 0, 0x80);
    assert_eq!(
        marked.warp,
        [stripe(RED, 4.0, Look::Sewn), stripe(navy, 2.0, Look::Gap), stripe(GREEN, 3.0, Look::Continued), stripe(BLUE, 1.0, Look::Sewn)]
    );
    // Text between stripes is passed over, and so is a bracket that opens no stripe: one not closed, empty,
    // or without a width.
    let loose = sett("x (#ff0000 () (#00ff00) (#0000ff)3 junk", "");
    assert_eq!(loose.warp, [stripe(BLUE, 3.0, Look::Sewn)]);
    // A code without brackets or colour letters has no stripes.
    let none = sett("", "");
    assert!(none.warp.is_empty() && none.weft.is_empty());
    assert_eq!(none.margin, 0.0);
    assert!(sett("12 / 34", "").warp.is_empty());
}

#[test]
fn diag_sc_w0328_a_colour_that_can_t_be_read_leaves_its_stripe_unsewn_and_is_named() {
    let read = read(r#"{"palette": "(one)2 (#ff0000)3 (transparent)1 | (two)4 (#00ff00)5"}"#).unwrap();
    assert_eq!(read.unread, ["one", "transparent", "two"]);
    assert_eq!(read.sett.warp, [stripe(BLACK, 2.0, Look::Gap), stripe(RED, 3.0, Look::Sewn), stripe(BLACK, 1.0, Look::Gap)]);
    assert_eq!(read.sett.weft, [stripe(BLACK, 4.0, Look::Gap), stripe(GREEN, 5.0, Look::Sewn)]);
}

#[test]
fn req_fill_trt_016_a_code_of_colour_letters_sews_the_register_s_shades_a_thread_0_5_mm_wide() {
    // Each count of threads 0.5 mm a thread, as Ink/Stitch's fill reads one; marks as a palette code's; letters
    // without a count passed over; pivots and the dots of a repeated sett passed over.
    let letters = read(r#"{"palette": "...B/24 w?4 Dg*2 R KX.5 r0..."}"#).unwrap();
    let dark_green = Rgb::new(0x00, 0x40, 0x28);
    assert_eq!(
        letters.sett.warp,
        [
            stripe(BLUE, 12.0, Look::Sewn),
            stripe(WHITE, 2.0, Look::Gap),
            stripe(dark_green, 1.0, Look::Continued),
            stripe(BLACK, 0.25, Look::Gap),
            stripe(RED, 0.0, Look::Sewn)
        ]
    );
    assert_eq!(letters.sett.weft, letters.sett.warp);
    // Letters the register doesn't use, `KX`, are named and leave their stripe unsewn.
    assert_eq!(letters.unread, ["KX"]);
    // With a `|`, every stripe is the warp's and the weft sews nothing, as Ink/Stitch reads the code.
    let apart = read(r#"{"palette": "B24 | R2"}"#).unwrap().sett;
    assert_eq!((apart.warp.len(), apart.weft.len()), (2, 0));
    // A count of no stripe, or letters alone, give no stripe.
    assert!(read(r#"{"palette": "B W R"}"#).unwrap().sett.warp.is_empty());
}

#[test]
fn req_fill_trt_016_a_registered_threadcount_takes_its_palette_s_colours() {
    // The last line after `Threadcount:` and after `Pallet:` count, each letter's last colour, with or without
    // a `#`, and what follows a colour's 6 digits is passed over.
    let code = "Threadcount:\nB2\nThreadcount:\nB24W4R2X1\nPallet:\nB=FF0000OLD;B=#0000FFBLUE;W=fefefeWHITE;R=FF0000\n";
    let registered = read(&serde_json::json!({ "palette": code }).to_string()).unwrap();
    assert_eq!(
        registered.sett.warp,
        [stripe(BLUE, 12.0, Look::Sewn), stripe(BLACK, 2.0, Look::Gap), stripe(RED, 1.0, Look::Sewn), stripe(BLACK, 0.5, Look::Gap)]
    );
    // White's digits are in lower case, which Ink/Stitch's pattern reads only for a and f: unread, as X.
    assert_eq!(registered.unread, ["W", "X"]);
    assert_eq!(
        pallet("A=aaafff;b=#AF09AF;C=12345;D=1234567;E=#gggggg"),
        [("A", Rgb::new(0xAA, 0xAF, 0xFF)), ("b", Rgb::new(0xAF, 0x09, 0xAF)), ("D", Rgb::new(0x12, 0x34, 0x56))]
    );
    // Without a line after `Threadcount:`, no stripe; a palette line that names nothing, no colour.
    assert!(read(r#"{"palette": "Threadcount:"}"#).unwrap().sett.warp.is_empty());
    assert_eq!(pallet("B 0000FF; =FF0000"), []);
}

#[test]
fn req_fill_trt_001_a_direction_sews_only_with_a_width_and_a_stripe_in_its_own_colour() {
    // The weft's stripes are all left unsewn, so the weft sews nothing, but its width still counts for the
    // margin, which is measured first.
    let apart = sett("(#ff0000)/4 (#0000ff)/3 | (#00ff00)?9", "");
    assert_eq!((apart.warp.len(), apart.weft.len(), apart.margin), (2, 0, 9.0));
    // The warp sews nothing, and the weft its own.
    let weft = sett("(#ff0000)?4 | (#00ff00)5", "");
    assert_eq!((weft.warp.len(), weft.weft.len()), (0, 1));
    // Without a `|`, a warp that sews nothing leaves the weft nothing either.
    let neither = sett("(#ff0000)?4 (#0000ff)?3", "");
    assert_eq!((neither.warp.len(), neither.weft.len(), neither.margin), (0, 0, 7.0));
    // A direction with no width sews nothing.
    let flat = sett("(#ff0000)0 | (#00ff00)5", "");
    assert_eq!((flat.warp.len(), flat.weft.len()), (0, 1));
}

#[test]
fn req_fill_trt_001_a_palette_s_width_is_its_wide_stripes_or_else_its_narrow_ones() {
    let sett = sett("(#ff0000)1", "");
    // Wide stripes but those continued; a narrow one is left out once a wide one counts.
    let mixed = [stripe(RED, 2.0, Look::Sewn), stripe(BLUE, 3.0, Look::Continued), stripe(GREEN, 0.5, Look::Sewn), stripe(WHITE, 5.0, Look::Gap)];
    assert_eq!(sett.width(&mixed), 7.0);
    // Without a wide one, the narrow ones but those left unsewn, a continued one among them.
    let narrow = [stripe(RED, 0.5, Look::Sewn), stripe(BLUE, 0.25, Look::Continued), stripe(GREEN, 0.75, Look::Gap)];
    assert_eq!(sett.width(&narrow), 0.75);
    assert_eq!(sett.width(&[stripe(RED, 0.5, Look::Sewn), stripe(GREEN, 0.25, Look::Gap)]), 0.5);
    // A continued stripe as wide as the narrowest is neither wide nor narrow.
    assert_eq!(sett.width(&[stripe(RED, 0.5, Look::Sewn), stripe(BLUE, 1.0, Look::Continued)]), 0.5);
    assert_eq!(sett.width(&[]), 0.0);
}

#[test]
fn req_fill_trt_002_a_narrow_stripe_s_halves_widen_the_stripes_beside_it_and_the_sett_mirrors() {
    // White sews as a run: half its width to red before it, and half to each band after it, until the end,
    // where it is a band of its own. Mirrored, back from the last step but one to the second.
    let inside = sett("(#ff0000)/4 (#ffffff)0.5 (#0000ff)/3", "");
    assert_eq!(
        inside.full(&inside.warp),
        [band(RED, 4.0), band(RED, 0.25), Item::Run(WHITE), band(BLUE, 3.25), band(BLUE, 0.25), band(BLUE, 3.25), Item::Run(WHITE), band(RED, 0.25),]
    );
    // A run first lies in the colour of the first stripe wider than the narrowest, which a mirrored sett goes
    // back to, and it isn't sewn again on the way back.
    let first = sett("(#ffffff)0.5 (#ff0000)/4 (#0000ff)/3", "");
    assert_eq!(
        first.full(&first.warp),
        [band(RED, 0.25), Item::Run(WHITE), band(RED, 4.25), band(BLUE, 3.25), band(BLUE, 0.25), band(BLUE, 3.25), band(RED, 4.25)]
    );
    // A run last, where the sett turns, is sewn once too.
    let last = sett("(#ff0000)/4 (#0000ff)3 (#ffffff)/0.5", "");
    assert_eq!(
        last.full(&last.warp),
        [band(RED, 4.0), band(BLUE, 3.0), band(BLUE, 0.25), Item::Run(WHITE), band(BLUE, 0.25), band(BLUE, 0.25), band(BLUE, 3.0)]
    );
}

#[test]
fn req_fill_trt_002_a_repeated_sett_starts_its_first_run_in_its_last_stripe_s_colour() {
    let repeated = sett("(#ffffff)0.5 (#ff0000)4 (#0000ff)3", r#", "symmetry": false"#);
    assert_eq!(repeated.full(&repeated.warp), [band(BLUE, 0.25), Item::Run(WHITE), band(RED, 4.25), band(BLUE, 3.25), band(BLUE, 0.25)]);
    // A pass that ends with a stripe sewn in the colour before it ends in that colour: the first run lies in
    // red, the last colour of a stripe sewn as a fill in its own colour, and the continued stripe takes the
    // run's half too.
    let continued = sett("(#ffffff)0.5 (#ff0000)4 (#0000ff)*3", r#", "symmetry": false"#);
    assert_eq!(continued.first_fill(&continued.warp), Some(RED));
    assert_eq!(continued.full(&continued.warp), [band(RED, 0.25), Item::Run(WHITE), band(RED, 4.25), band(RED, 3.25)]);
}

#[test]
fn req_fill_trt_002_a_stripe_left_unsewn_takes_a_run_s_half_last_and_a_continued_one_its_colour() {
    // The black stripe left unsewn ends the white run's half, and the blue after it is as wide as drawn.
    let ended = sett("(#ff0000)4 (#ffffff)0.5 (#000000)?2 (#0000ff)3", r#", "symmetry": false"#);
    assert_eq!(ended.full(&ended.warp), [band(RED, 4.0), band(RED, 0.25), Item::Run(WHITE), gap(2.25), band(BLUE, 3.0)]);
    // A continued stripe sews in the colour before it.
    let continued = sett("(#ff0000)4 (#0000ff)*2 (#00ff00)3", "");
    assert_eq!(continued.full(&continued.warp), [band(RED, 4.0), band(RED, 2.0), band(GREEN, 3.0), band(RED, 2.0)]);
    // A run after a stripe left unsewn first lies between unsewn bands.
    let unsewn = sett("(#000000)?3 (#ffffff)0.5 (#ff0000)4", "");
    assert_eq!(unsewn.full(&unsewn.warp)[..4], [gap(3.0), gap(0.25), Item::Run(WHITE), band(RED, 4.25)]);
}

#[test]
fn req_fill_trt_002_stripes_are_scaled_and_a_stripe_as_wide_as_the_narrowest_is_a_run() {
    // At 200 %, 0.8 mm is 1.6 mm, wider than the narrowest.
    let scaled = sett("(#ff0000)/2 (#0000ff)/0.8", r#", "scale": 200"#);
    assert_eq!(scaled.full(&scaled.warp), [band(RED, 4.0), band(BLUE, 1.6)]);
    let at_limit = sett("(#ff0000)/2 (#0000ff)/1", "");
    assert_eq!(at_limit.full(&at_limit.warp), [band(RED, 2.0), band(RED, 0.5), Item::Run(BLUE), band(RED, 0.5), band(RED, 0.5)]);
    // No stripes, no sett.
    assert_eq!(at_limit.full(&[]), []);
}

#[test]
fn req_fill_trt_002_a_stripe_is_compared_with_the_narrowest_in_ink_stitch_s_pixels() {
    // At 40 %, a stripe of 2.5 mm is 1 mm, as wide as the narrowest, and sews as a run in blue, the first
    // stripe wider; one of 1.1 mm at 100 % is wider, and sews as a band.
    let limit = sett("(#ff0000)/2.5 (#0000ff)/5", r#", "scale": 40"#);
    assert_eq!(limit.full(&limit.warp), [band(BLUE, 0.5), Item::Run(RED), band(BLUE, 2.5), band(BLUE, 0.5), band(BLUE, 2.5)]);
    let wider = sett("(#ff0000)/2 (#0000ff)/1.1", "");
    assert_eq!(wider.full(&wider.warp), [band(RED, 2.0), band(BLUE, 1.1)]);
    // At 8 %, a stripe of 7.5 mm is 0.6 mm, but in Ink/Stitch's pixels a little more than a narrowest of 0.6 mm,
    // so it sews as a band.
    let over = sett("(#ff0000)/7.5 (#0000ff)/7.5", r#", "scale": 8, "min_stripe_width": 0.6"#);
    assert_eq!(over.full(&over.warp), [band(RED, 7.5 * 0.08), band(BLUE, 7.5 * 0.08)]);
}

#[test]
fn req_fill_trt_002_a_run_after_a_run_widens_the_band_between_them_by_both_halves() {
    let twice = sett("(#ff0000)4 (#ffffff)0.5 (#00ff00)0.5 (#0000ff)3", r#", "symmetry": false"#);
    assert_eq!(
        twice.full(&twice.warp),
        [band(RED, 4.0), band(RED, 0.25), Item::Run(WHITE), band(RED, 0.5), Item::Run(GREEN), band(BLUE, 3.25), band(BLUE, 0.25)]
    );
}

#[test]
fn req_fill_trt_002_the_first_fill_is_the_first_stripe_wider_than_the_narrowest_once_scaled() {
    // At 200 %, red's 0.75 mm is 1.5 mm, and white's 0.5 mm is 1 mm, as wide as the narrowest.
    let scaled = sett("(#ffffff)0.5 (#ff0000)0.75 (#0000ff)3", r#", "scale": 200"#);
    assert_eq!(scaled.first_fill(&scaled.warp), Some(RED));
    // Green, as wide as the narrowest, is a run, not the first fill.
    let level = sett("(#ffffff)0.5 (#00ff00)1 (#ff0000)4", "");
    assert_eq!(level.first_fill(&level.warp), Some(RED));
}
