//! The editor's own cases: setts made from codes and stripes, and the code and setting it writes.

use stitchcraft_plan::Rgb;

use super::*;

const BLUE: Rgb = Rgb::new(0, 0, 0xFF);
const WHITE: Rgb = Rgb::new(0xFF, 0xFF, 0xFF);
const RED: Rgb = Rgb::new(0xFF, 0, 0);
const BLACK: Rgb = Rgb::new(0, 0, 0);
const GREEN: Rgb = Rgb::new(0x28, 0x9C, 0x18);

fn stripe(colour: Rgb, width: f64) -> Stripe {
    Stripe { colour, width, look: Look::Sewn }
}

/// The widths of `stripes`, to a millionth.
fn widths(stripes: &[Stripe]) -> Vec<f64> {
    stripes.iter().map(|s| (s.width * 1e6).round() / 1e6).collect()
}

#[test]
fn req_fill_trt_015_a_threadcount_s_counts_are_threads_of_the_width_given() {
    // The register's own form of a mirrored threadcount, its pivots marked: each thread 0.2 mm.
    let drafted = Draft::from_code("B/24 W4 B24 R2 K24 G24 W/2", 0.2).unwrap();
    let draft = drafted.draft;
    assert_eq!(widths(&draft.warp), [4.8, 0.8, 4.8, 0.4, 4.8, 4.8, 0.4]);
    let colours: Vec<Rgb> = draft.warp.iter().map(|s| s.colour).collect();
    assert_eq!(colours, [BLUE, WHITE, BLUE, RED, BLACK, GREEN, WHITE]);
    assert_eq!((draft.weft, draft.mirrored, drafted.unread), (None, true, Vec::<String>::new()));
    // Laid as Ink/Stitch's editor starts.
    assert_eq!((draft.rotate, draft.scale, draft.offset, draft.narrowest), (0.0, 100.0, [0.0, 0.0], 1.0));
    // Each thread 1 mm: the counts are the widths.
    assert_eq!(widths(&Draft::from_code("B24 W4", 1.0).unwrap().draft.warp), [24.0, 4.0]);
}

#[test]
fn req_fill_trt_015_a_code_with_the_dots_of_a_repeated_sett_repeats() {
    let draft = Draft::from_code("...B24 W4...", 0.5).unwrap().draft;
    assert!(!draft.mirrored);
    assert_eq!(draft.code(), "...(#0000ff)12 (#ffffff)2...");
    // A palette code says so the same way, and without its dots is mirrored.
    assert!(!Draft::from_code("...(#ff0000)5 (#0000ff)5...", 0.2).unwrap().draft.mirrored);
    assert!(Draft::from_code("(#ff0000)5 (#0000ff)5", 0.2).unwrap().draft.mirrored);
}

#[test]
fn req_fill_trt_015_the_register_s_threadcount_takes_its_colours_from_its_palette() {
    let register = "Threadcount:\nB24W4B24R2K24G24W2\n\nPallet:\nB=0000FFBLUE;W=FFFFFFWHITE;R=C80000RED;K=101010BLACK;G=006818GREEN;\n\n\
                    Threadcount given over a half sett with full count at the pivots.";
    let drafted = Draft::from_code(register, 0.25).unwrap();
    let colours: Vec<Rgb> = drafted.draft.warp.iter().map(|s| s.colour).collect();
    let (red, black, green) = (Rgb::new(0xC8, 0, 0), Rgb::new(0x10, 0x10, 0x10), Rgb::new(0, 0x68, 0x18));
    assert_eq!(colours, [BLUE, WHITE, BLUE, red, black, green, WHITE]);
    assert_eq!(widths(&drafted.draft.warp), [6.0, 1.0, 6.0, 0.5, 6.0, 6.0, 0.5]);
    assert!(drafted.draft.mirrored && drafted.unread.is_empty());
    // A full sett repeats, and lines broken by a carriage return and a line feed read the same.
    let full = register.replace("over a half sett with full count at the pivots", "over a full sett").replace('\n', "\r\n");
    let drafted = Draft::from_code(&full, 0.25).unwrap();
    assert!(!drafted.draft.mirrored);
    assert_eq!(drafted.draft.warp.len(), 7);
}

#[test]
fn req_fill_trt_015_the_code_written_marks_the_pivots_the_unsewn_and_the_weft() {
    // Mirrored: the first stripe and the last marked as pivots; unsewn and continued stripes marked.
    let draft = Draft {
        warp: vec![
            stripe(BLUE, 2.4),
            Stripe { colour: WHITE, width: 0.4, look: Look::Gap },
            Stripe { colour: RED, width: 1.0, look: Look::Continued },
        ],
        weft: None,
        mirrored: true,
        rotate: 0.0,
        scale: 100.0,
        offset: [0.0, 0.0],
        narrowest: 1.0,
    };
    assert_eq!(draft.code(), "(#0000ff)/2.4 (#ffffff)?0.4 (#ff0000)/*1");
    // A weft of its own after a `|`, the last of its stripes the second pivot.
    let apart = Draft { weft: Some(vec![stripe(GREEN, 3.0), stripe(BLACK, 0.5)]), ..draft.clone() };
    assert_eq!(apart.code(), "(#0000ff)/2.4 (#ffffff)?0.4 (#ff0000)*1 | (#289c18)3 (#000000)/0.5");
    // One stripe is marked once, and no stripe gives no code.
    let one = Draft { warp: vec![stripe(RED, 5.0)], ..draft.clone() };
    assert_eq!(one.code(), "(#ff0000)/5");
    assert_eq!(Draft { warp: Vec::new(), ..draft }.code(), "");
    // A weft with no stripe leaves the `|` last.
    assert_eq!(Draft { weft: Some(Vec::new()), ..one }.code(), "(#ff0000)/5 |");
}

#[test]
fn req_fill_trt_015_the_setting_written_reads_back_as_the_sett() {
    let drafted = Draft::from_code("R/8 K4 G/12 | Y/2 DB/6", 0.25).unwrap();
    // A `|` in a threadcount gives the weft no stripe, as Ink/Stitch's editor reads one.
    assert_eq!(drafted.draft.weft, Some(Vec::new()));
    let laid = Draft { rotate: 30.0, scale: 150.0, offset: [3.0, 1.5], narrowest: 0.5, ..drafted.draft };
    let setting = laid.setting();
    assert_eq!(
        setting,
        r#"{"symmetry": true, "equal_warp_weft": false, "rotate": 30.0, "scale": 150.0, "offset_x": 3.0, "offset_y": 1.5, "palette": "(#ff0000)/2 (#000000)1 (#289c18)3 (#ffff00)0.5 (#00008c)1.5 |", "min_stripe_width": 0.5}"#
    );
    let read = sett::read(&setting).unwrap().sett;
    assert_eq!((read.rotate, read.scale, read.offset, read.mirrored, read.narrowest), (30.0, 150.0, [3.0, 1.5], true, 0.5));
    assert_eq!(read.warp, laid.warp);
    assert!(read.weft.is_empty());
    // Warp and weft alike: `equal_warp_weft` on, and the weft read as the warp.
    let alike = Draft { weft: None, ..laid };
    let read = sett::read(&alike.setting()).unwrap().sett;
    assert!(alike.setting().contains(r#""equal_warp_weft": true"#));
    assert_eq!(read.weft, read.warp);
}

#[test]
fn diag_sc_w0328_the_editor_names_letters_the_register_doesn_t_use() {
    let drafted = Draft::from_code("B24 X4 B24 QZ2", 0.2).unwrap();
    assert_eq!(drafted.unread, ["X", "QZ"]);
    let looks: Vec<Look> = drafted.draft.warp.iter().map(|s| s.look).collect();
    assert_eq!(looks, [Look::Sewn, Look::Gap, Look::Sewn, Look::Gap]);
}

#[test]
fn req_fill_trt_015_a_code_that_can_t_be_read_says_why() {
    assert_eq!(Draft::from_code("B1.2.3", 0.2).unwrap_err(), "the count `1.2.3` of a stripe isn't a number");
    assert_eq!(Draft::from_code("Threadcount:\nB?24 W4", 0.2).unwrap_err(), "the letters `B` of its threadcount have no count");
    assert_eq!(Draft::from_code("(#ff0000)5 | (#00ff00)5 | (#0000ff)5", 0.2).unwrap_err(), "its code has more than one `|`");
}
