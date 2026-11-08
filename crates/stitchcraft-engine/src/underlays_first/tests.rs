//! The runs of underlays first and the steps they are sewn in, from what the order reads of each element
//! (`REQ-ASM-008`, `REQ-ASM-009`).

use stitchcraft_core::{ElementId, Severity};
use stitchcraft_plan::Thread;

use super::*;
use crate::design::Shape;

const RED: Rgb = Rgb::new(200, 0, 0);
const BLUE: Rgb = Rgb::new(0, 0, 200);

/// An element that sets `underlays_first`, reads and has an underlay, in no thread yet.
const SETS: Candidate = Candidate { sets: true, reads: true, threads: Vec::new(), ends: false, underlay: true };

/// `candidate` in the threads `threads`.
fn sewn_in(candidate: &Candidate, threads: &[Rgb]) -> Candidate {
    Candidate { threads: threads.to_vec(), ..candidate.clone() }
}

/// An element that sets `underlays_first` in red, with an underlay.
fn sets() -> Candidate {
    sewn_in(&SETS, &[RED])
}

/// An element in red that doesn't set it.
fn plain() -> Candidate {
    Candidate { sets: false, reads: false, underlay: false, ..sets() }
}

fn whole(element: usize) -> Step {
    Step { element, layer: Layer::Whole }
}

fn underlay(element: usize) -> Step {
    Step { element, layer: Layer::Underlay }
}

fn top(element: usize) -> Step {
    Step { element, layer: Layer::Top }
}

#[test]
fn req_asm_009_a_run_of_2_sews_both_underlays_then_both_tops() {
    let (order, alone) = steps(&[plain(), sets(), sets(), plain()]);
    assert_eq!(order, [whole(0), underlay(1), underlay(2), top(1), top(2), whole(3)]);
    assert_eq!(alone, []);
    // 3 in a row: the underlays in their order, then the tops in the same order.
    let (order, _) = steps(&[sets(), sets(), sets()]);
    assert_eq!(order, [underlay(0), underlay(1), underlay(2), top(0), top(1), top(2)]);
}

#[test]
fn req_asm_009_an_element_of_a_run_without_underlay_sews_whole_in_the_second_pass() {
    let bare = Candidate { underlay: false, ..sets() };
    let (order, alone) = steps(&[sets(), bare.clone(), sets()]);
    assert_eq!(order, [underlay(0), underlay(2), top(0), whole(1), top(2)]);
    assert_eq!(alone, []);
    // A run of elements none of which has an underlay sews as without the setting.
    assert_eq!(steps(&[bare.clone(), bare]).0, [whole(0), whole(1)]);
}

#[test]
fn req_asm_008_a_run_of_one_sews_whole_and_is_named() {
    let (order, alone) = steps(&[plain(), sets(), plain()]);
    assert_eq!(order, [whole(0), whole(1), whole(2)]);
    assert_eq!(alone, [(1, Alone::Lone)]);
    // At the design's start and end too.
    assert_eq!(steps(&[sets()]), (vec![whole(0)], vec![(0, Alone::Lone)]));
    // Elements that don't set it are in no run, and nothing is said of them.
    assert_eq!(steps(&[plain(), plain()]), (vec![whole(0), whole(1)], vec![]));
    assert_eq!(steps(&[]), (vec![], vec![]));
}

#[test]
fn req_asm_008_a_trim_or_a_stop_ends_a_run_after_its_element() {
    let ends = Candidate { ends: true, ..sets() };
    // After the middle one of 3, the first 2 are a run and the third is alone.
    let (order, alone) = steps(&[sets(), ends.clone(), sets()]);
    assert_eq!(order, [underlay(0), underlay(1), top(0), top(1), whole(2)]);
    assert_eq!(alone, [(2, Alone::Lone)]);
    // After the last of a run, the run is whole, and the next 2 are a run of their own.
    let (order, alone) = steps(&[sets(), ends.clone(), sets(), sets()]);
    assert_eq!(order, [underlay(0), underlay(1), top(0), top(1), underlay(2), underlay(3), top(2), top(3)]);
    assert_eq!(alone, []);
    // After the first, it is alone, and the next 2 are a run.
    let (order, alone) = steps(&[ends.clone(), sets(), sets()]);
    assert_eq!(order, [whole(0), underlay(1), underlay(2), top(1), top(2)]);
    assert_eq!(alone, [(0, Alone::Lone)]);
    // After every element, each is alone.
    assert_eq!(steps(&[ends.clone(), ends]).1, [(0, Alone::Lone), (1, Alone::Lone)]);
}

#[test]
fn req_asm_008_threads_part_runs_and_an_element_in_several_threads_is_in_no_run() {
    // Red, red, blue, blue: 2 runs.
    let blue = sewn_in(&SETS, &[BLUE]);
    let (order, alone) = steps(&[sets(), sets(), blue.clone(), blue.clone()]);
    assert_eq!(order, [underlay(0), underlay(1), top(0), top(1), underlay(2), underlay(3), top(2), top(3)]);
    assert_eq!(alone, []);
    // Red, blue, red: each alone.
    assert_eq!(steps(&[sets(), blue, sets()]).1, [(0, Alone::Lone), (1, Alone::Lone), (2, Alone::Lone)]);
    // An element sewn in 2 threads, red first, is in no run, and parts the elements beside it.
    let threads = sewn_in(&SETS, &[RED, BLUE]);
    let (order, alone) = steps(&[sets(), threads.clone(), sets()]);
    assert_eq!(order, [whole(0), whole(1), whole(2)]);
    assert_eq!(alone, [(0, Alone::Lone), (1, Alone::Threads), (2, Alone::Lone)]);
    // So is an element sewn in no thread, which sews nothing.
    assert_eq!(steps(&[sewn_in(&SETS, &[]), sets()]).1, [(0, Alone::Threads), (1, Alone::Lone)]);
    // One that doesn't set it in 2 threads is said nothing of.
    assert_eq!(steps(&[Candidate { sets: false, ..threads }]), (vec![whole(0)], vec![]));
}

#[test]
fn req_asm_008_an_element_whose_settings_dont_read_is_in_no_run_and_said_nothing_of_here() {
    let unread = Candidate { reads: false, ..sets() };
    let (order, alone) = steps(&[sets(), unread, sets()]);
    assert_eq!(order, [whole(0), whole(1), whole(2)]);
    assert_eq!(alone, [(0, Alone::Lone), (2, Alone::Lone)]);
}

/// A line in `thread`, with `params`.
fn line(id: &str, thread: Thread, params: &[(&str, &str)]) -> Element {
    let path = crate::design::Path {
        subpaths: vec![crate::design::Subpath {
            start: stitchcraft_core::Point::ORIGIN,
            segments: vec![crate::design::Segment::Line(stitchcraft_core::Point::new(10.0, 0.0).unwrap())],
            closed: false,
        }],
    };
    Element {
        id: ElementId::new(id).unwrap(),
        name: None,
        shape: Shape::stroke(path),
        thread,
        params: params.iter().copied().collect(),
        automatic: false,
        hints: Default::default(),
        clips: Vec::new(),
    }
}

#[test]
fn req_asm_008_the_order_reads_each_element_s_settings_and_threads_by_colour() {
    let settings = DesignSettings::default();
    let red = Thread::named(RED, "Red");
    let on = [("underlays_first", "true")];
    // A running stitch that sets it, in red: it reads and has no underlay.
    let of = |params: &[(&str, &str)], thread: &Thread| Candidate::of(&line("e", thread.clone(), params), &settings);
    assert_eq!(of(&on, &red), Candidate { sets: true, reads: true, threads: vec![RED], ends: false, underlay: false });
    // Another name for red is red: the threads are their colours.
    assert_eq!(of(&on, &Thread::named(RED, "Scarlet")).threads, [RED]);
    // Without the setting, only its threads and its end are read.
    assert_eq!(of(&[("trim_after", "true")], &red), Candidate { sets: false, reads: false, threads: vec![RED], ends: true, underlay: false });
    assert!(of(&[("stop_after", "true"), ("underlays_first", "true")], &red).ends);
    // A setting that doesn't read: `underlays_first` sets nothing, and `trim_after` ends nothing.
    assert!(!of(&[("underlays_first", "maybe")], &red).sets);
    assert!(!of(&[("trim_after", "maybe")], &red).ends);
    // Any setting that its stitch type reads and doesn't read: it is in no run.
    assert!(!of(&[("underlays_first", "true"), ("running_stitch_length_mm", "long")], &red).reads);
}

#[test]
fn each_element_alone_is_named_with_why() {
    let element = line("svg:path1", Thread::new(RED), &[]);
    let lone = alone(&element, Alone::Lone);
    assert_eq!(
        lone.to_string(),
        "info SC-I0506: This element sets `underlays_first`, but neither element next to it in sewing order joins it: they don't set it, or sew in \
         another thread, or a trim or a stop parts them. So it sews its underlay just before its top stitches, as it does without the setting."
    );
    assert_eq!(
        lone.fix.map(|fix| fix.describe()).as_deref(),
        Some("Set `underlays_first` on the elements that cross or overlap it, in its thread, or move the trim or the stop.")
    );
    assert_eq!((lone.element.as_ref().map(ElementId::as_str), lone.code.severity()), (Some("svg:path1"), Severity::Info));
    let threads = alone(&element, Alone::Threads);
    assert_eq!(
        threads.to_string(),
        "info SC-I0506: This element sets `underlays_first`, but it is sewn in several threads, so it joins no run, and sews its underlay just \
         before its top stitches, as it does without the setting."
    );
    assert_eq!((threads.fix, threads.element), (None, Some(element.id)));
}
