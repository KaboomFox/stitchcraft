//! The tartan's own cases: the order of its colours and groups, the travel left out, and the threads read
//! for the sewing order.

use stitchcraft_core::Budget;

use super::*;
use crate::generators::tatami::fixture::{p, rectangle};

const RED: Rgb = Rgb::new(255, 0, 0);
const LIME: Rgb = Rgb::new(0, 255, 0);
const BLUE: Rgb = Rgb::new(0, 0, 255);
const BLACK: Rgb = Rgb::new(0, 0, 0);

/// A tartan of the full sett `items`, warp and weft alike, with Ink/Stitch's defaults and no underlay.
pub(super) fn tartan(items: &[Item]) -> Tartan {
    let read = sett::read(r#"{"palette": ""}"#).unwrap();
    Tartan {
        sett: read.sett,
        warp: items.to_vec(),
        weft: items.to_vec(),
        margin: 10.0,
        angle: -45.0,
        spacing: 0.25,
        longest: 4.0,
        staggers: 4.0,
        rows_per_thread: 2,
        run_length: 2.5,
        beans: vec![0],
        skip_last: false,
        stop_at_end: false,
        underlay: Vec::new(),
        inset: 0.0,
        expand: 0.0,
        travel: Travel { length: 2.5, tolerance: 0.2, min_stitch: 0.3, underpath: false, jump: 3.0 },
        own: BLACK,
    }
}

fn lines(colours: &[Rgb]) -> Coloured {
    colours.iter().map(|&colour| (colour, vec![vec![p(0.0, 0.0), p(1.0, 1.0)]])).collect()
}

fn colours(coloured: &Coloured) -> Vec<Rgb> {
    coloured.iter().map(|(colour, _)| *colour).collect()
}

#[test]
fn req_fill_trt_006_the_last_colour_the_fills_and_runs_share_ends_the_fills_and_starts_the_runs() {
    let (fills, runs) = joined(lines(&[RED, LIME, BLUE]), lines(&[LIME, BLACK]));
    assert_eq!((colours(&fills), colours(&runs)), (vec![RED, BLUE, LIME], vec![LIME, BLACK]));
    // Of 2 shared, the one the fills sew last.
    let (fills, runs) = joined(lines(&[RED, LIME, BLUE]), lines(&[BLUE, RED]));
    assert_eq!((colours(&fills), colours(&runs)), (vec![RED, LIME, BLUE], vec![BLUE, RED]));
    let (fills, runs) = joined(lines(&[RED]), lines(&[BLUE]));
    assert_eq!((colours(&fills), colours(&runs)), (vec![RED], vec![BLUE]));
}

#[test]
fn req_fill_trt_006_a_colour_starts_and_ends_with_its_rows() {
    let points = vec![p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(3.0, 0.0), p(4.0, 0.0)];
    assert_eq!(without_travel(points.clone(), &[true, true, false, false, true]), [p(2.0, 0.0), p(3.0, 0.0)]);
    assert_eq!(without_travel(points.clone(), &[false, true, false, true, false]), points);
    assert_eq!(without_travel(points.clone(), &[true; 5]), points);
}

#[test]
fn req_fill_trt_009_several_parts_sew_their_underlays_and_each_colour_together() {
    let group = |colour, kind| Group { colour, kind, run: vec![p(0.0, 0.0)] };
    let groups = vec![
        group(BLACK, Kind::Underlay),
        group(RED, Kind::Fill),
        group(LIME, Kind::Fill),
        group(BLUE, Kind::Run),
        group(BLACK, Kind::Underlay),
        group(LIME, Kind::Fill),
        group(RED, Kind::Fill),
        group(BLUE, Kind::Run),
    ];
    let found: Vec<(Rgb, Kind)> = sorted(groups).iter().map(|group| (group.colour, group.kind)).collect();
    assert_eq!(
        found,
        [
            (BLACK, Kind::Underlay),
            (BLACK, Kind::Underlay),
            (RED, Kind::Fill),
            (RED, Kind::Fill),
            (LIME, Kind::Fill),
            (LIME, Kind::Fill),
            (BLUE, Kind::Run),
            (BLUE, Kind::Run)
        ]
    );
    // An underlay in a colour a fill sews too sorts with that colour, as Ink/Stitch compares the colours.
    let groups =
        vec![group(RED, Kind::Underlay), group(RED, Kind::Fill), group(LIME, Kind::Fill), group(RED, Kind::Underlay), group(RED, Kind::Fill)];
    let found: Vec<Kind> = sorted(groups).iter().map(|group| group.kind).collect();
    assert_eq!(found, [Kind::Underlay, Kind::Fill, Kind::Underlay, Kind::Fill, Kind::Fill]);
}

#[test]
fn the_threads_of_a_tartan_are_read_from_its_settings_for_the_sewing_order() {
    let set = |sett: &str| -> ParamSet { [("tartan", sett)].into_iter().collect() };
    // The underlay's own thread, the warp's colours sewn as fills, the weft's after them, and the runs; the
    // colour shared by the fills and the runs last of the fills and first of the runs.
    let sett = r#"{"palette": "(#ff0000)5 (#00ff00)0.5 (#0000ff)5 (#00ff00)5 | (#000000)4 (#ff0000)0.2", "symmetry": false}"#;
    assert_eq!(threads(&set(sett), Rgb::new(1, 2, 3), true), [Rgb::new(1, 2, 3), RED, BLUE, BLACK, LIME, LIME, RED]);
    assert_eq!(threads(&set(sett), Rgb::new(1, 2, 3), false), [RED, BLUE, BLACK, LIME, LIME, RED]);
    // A colour first reached by a run comes before the colours whose bands come after it: lime's run comes
    // before blue's band, and blue, sewn last of the fills and shared with the runs, starts them.
    let sett = r#"{"palette": "(#ff0000)5 (#00ff00)0.5 (#0000ff)5 (#00ff00)5 (#0000ff)0.5", "symmetry": false}"#;
    assert_eq!(threads(&set(sett), BLACK, false), [RED, LIME, BLUE, BLUE, LIME]);
    // A sett that can't be read, or sews nothing, sews in the element's own thread.
    assert_eq!(threads(&set("{"), BLUE, true), [BLUE]);
    assert_eq!(threads(&set(r#"{"palette": "(red)?3"}"#), BLUE, false), [BLUE]);
    // Settings that can't be read are read as an empty sett, Ink/Stitch's default.
    let unreadable: ParamSet = [("tartan", "x".repeat(5000).as_str())].into_iter().collect();
    assert_eq!(threads(&unreadable, BLUE, false), [Rgb::new(0x10, 0x10, 0x10)]);
}

#[test]
fn a_tartan_s_preview_is_the_lines_of_its_colours_inside_the_fill() {
    // Red and blue bands 5 mm wide across a 10 × 4 rectangle: segments, all inside it.
    let region = Region { parts: vec![rectangle(4.0)] };
    let red_blue = tartan(&[Item::Band { colour: Some(RED), width: 5.0 }, Item::Band { colour: Some(BLUE), width: 5.0 }]);
    let segments = top_path(&region, &red_blue, &mut Budget::DEFAULT.meter()).unwrap();
    assert!(!segments.is_empty());
    let inside = |q: Point| (-1e-9..=10.0 + 1e-9).contains(&q.x()) && (-1e-9..=4.0 + 1e-9).contains(&q.y());
    assert!(segments.iter().all(|s| inside(s.start) && inside(s.end)), "{segments:?}");
    // Sewn, its groups are the colours in their own threads, each a colour block.
    let sewn = tartan_fill(&region, &red_blue, None, None, &mut SplitMix64::new(1), &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(sewn.runs.len(), sewn.colours.len());
    let threads: Vec<Option<Rgb>> = sewn.colours.iter().map(|c| c.thread.as_ref().map(|t| t.color)).collect();
    assert_eq!(threads, [Some(RED), Some(BLUE)]);
    assert!(sewn.colours.iter().all(|c| c.ends_block));
}
