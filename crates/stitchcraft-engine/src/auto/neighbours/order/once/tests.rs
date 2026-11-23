//! The base layer's thread and `SC-W0925` (`REQ-AUTO-185`, `REQ-AUTO-187`).

use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::Budget;

use super::super::tests::{BLUE, GREEN, RED, messages, ordered_by, running, square};
use super::super::{Facts, Graph, Moving, Order, Rules};
use super::told;
use crate::auto::neighbours::Shapes;
use crate::design::DesignSettings;

const ONCE: Rules = Rules { last: true, once: true };

#[test]
fn req_auto_185_the_base_layer_sews_in_the_thread_the_best_order_starts_with() {
    // A blue square, and a larger red one drawn over it. Red, the base layer's, can't sew first: from red the
    // order changes thread twice, and from blue once, so the base layer sews in blue.
    let head = || vec![square("b", (0.0, 0.0), 10.0, &BLUE), square("r", (5.0, -5.0), 20.0, &RED)];
    let (ids, _, chosen) = ordered_by(head(), &Budget::DEFAULT, &Budget::DEFAULT, Some(RED.color), ONCE);
    assert_eq!((ids, chosen), (vec!["b".to_string(), "r".to_string()], Some(BLUE.color)));
    // 2 squares apart change thread once from either: the base layer keeps the thread it was given.
    let apart = || vec![square("r", (0.0, 0.0), 10.0, &RED), square("b", (50.0, 0.0), 10.0, &BLUE)];
    let (ids, _, chosen) = ordered_by(apart(), &Budget::DEFAULT, &Budget::DEFAULT, Some(RED.color), ONCE);
    assert_eq!((ids, chosen), (vec!["r".to_string(), "b".to_string()], None));
    let (ids, _, chosen) = ordered_by(apart(), &Budget::DEFAULT, &Budget::DEFAULT, Some(BLUE.color), ONCE);
    assert_eq!((ids, chosen), (vec!["b".to_string(), "r".to_string()], None));
}

#[test]
fn req_auto_186_with_auto_thread_once_off_the_base_layer_keeps_the_largest_area_s_thread() {
    // The blue square under the larger red one: with the setting off the base layer sews in red, the thread it
    // is given, as before M13.63, and the order changes thread twice from it.
    let head = || vec![square("b", (0.0, 0.0), 10.0, &BLUE), square("r", (5.0, -5.0), 20.0, &RED)];
    let off = Rules { last: true, once: false };
    let (ids, _, chosen) = ordered_by(head(), &Budget::DEFAULT, &Budget::DEFAULT, Some(RED.color), off);
    assert_eq!((ids, chosen), (vec!["b".to_string(), "r".to_string()], None));
}

#[test]
fn req_auto_185_of_orders_as_good_the_base_layer_takes_one_that_sews_more_lines_after_their_fills() {
    // A large green square, and apart a red one with a green line along its edge. From green the line sews
    // with the square, before the red fill. From red the thread changes as often, and the line sews after the
    // fill it borders, so the base layer sews in red.
    let sewn = || {
        vec![square("g", (100.0, 0.0), 30.0, &GREEN), square("r", (0.0, 0.0), 10.0, &RED), running("gl", &[(11.0, 0.0), (11.0, 10.0)], 2.0, &GREEN)]
    };
    let (ids, _, chosen) = ordered_by(sewn(), &Budget::DEFAULT, &Budget::DEFAULT, Some(GREEN.color), ONCE);
    assert_eq!((ids, chosen), (["r", "gl", "g"].map(String::from).to_vec(), Some(RED.color)));
    // Each search the choice needs has the work of the budget: short of it, the order is sewn as before
    // M13.22, from the base layer's thread, and that is said.
    for work in (0..6_000).step_by(250) {
        let (ids, said, chosen) = ordered_by(sewn(), &Budget::DEFAULT, &Budget { max_work: work, ..Budget::DEFAULT }, Some(GREEN.color), ONCE);
        let found = chosen.is_some();
        assert!(found || ids == ["g", "gl", "r"] || ids == ["g", "r", "gl"], "{work}: {ids:?} {:?}", messages(&said));
    }
}

/// A red chimney, a blue outline over its edge, a red roof over the outline, and a blue outline over the roof's
/// edge, as the review set's house draws them.
fn house() -> Vec<crate::auto::neighbours::Sewn> {
    vec![
        square("chimney", (0.0, 0.0), 10.0, &RED),
        running("outline", &[(10.0, 0.0), (10.0, 10.0)], 2.0, &BLUE),
        square("roof", (9.0, 0.0), 10.0, &RED),
        running("roof-outline", &[(19.0, 0.0), (19.0, 10.0)], 2.0, &BLUE),
    ]
}

#[test]
fn req_auto_187_a_shade_drawn_under_and_over_another_is_named_once() {
    let (ids, said, _) = ordered_by(house(), &Budget::DEFAULT, &Budget::DEFAULT, None, ONCE);
    assert_eq!(ids, ["chimney", "outline", "roof", "roof-outline"]);
    let warned: Vec<String> = messages(&said).into_iter().filter(|message| message.contains("SC-W0925")).collect();
    assert_eq!(
        warned,
        ["warning SC-W0925: This shape lies under \"outline\", and \"outline\" lies under \"roof\": their threads lie both under and over \
             each other, so no order threads each thread once. The threads are threaded #0000c8 2 times, #c80000 2 times. [chimney]"]
    );
    // With `auto_thread_once` off nothing is said of it.
    let (_, said, _) = ordered_by(house(), &Budget::DEFAULT, &Budget::DEFAULT, None, Rules { last: true, once: false });
    assert!(messages(&said).iter().all(|message| !message.contains("SC-W0925")));
}

#[test]
fn diag_sc_w0925_names_a_shape_of_a_shade_drawn_under_and_over_another_and_says_how_to_thread_once() {
    let (_, said, _) = ordered_by(house(), &Budget::DEFAULT, &Budget::DEFAULT, None, ONCE);
    let warned: Vec<_> = said.iter().filter(|said| said.code == stitchcraft_core::Code::AutoThreadedAgain).collect();
    assert_eq!(warned.len(), 1);
    assert_eq!(warned.first().and_then(|said| said.element.as_ref()).map(ToString::to_string).as_deref(), Some("chimney"));
    assert!(warned.iter().all(|said| said.fix.is_some()), "{warned:?}");
}

#[test]
fn req_auto_187_a_thread_sewn_twice_for_no_overlap_is_not_named() {
    // A red square, a blue one and a red one with settings keep their order, and red sews twice, though no
    // shape lies over another: nothing is named.
    let set = |mut sewn: crate::auto::neighbours::Sewn| {
        sewn.automatic = false;
        sewn
    };
    let mut sewn: Vec<_> = [square("r1", (0.0, 0.0), 10.0, &RED), square("b", (50.0, 0.0), 10.0, &BLUE), square("r2", (100.0, 0.0), 10.0, &RED)]
        .into_iter()
        .map(set)
        .collect();
    sewn.push(square("g", (150.0, 0.0), 10.0, &GREEN));
    let (ids, said, _) = ordered_by(sewn, &Budget::DEFAULT, &Budget::DEFAULT, None, ONCE);
    assert_eq!(ids, ["g", "r1", "b", "r2"]);
    assert!(messages(&said).iter().all(|message| !message.contains("SC-W0925")), "{:?}", messages(&said));
}

#[test]
fn req_auto_187_the_shapes_named_are_those_of_the_first_thread_and_the_thread_over_it() {
    // 3 threads in one group: red under blue and blue under red, and green under red and red under green, in
    // another run first. The warning is said once, of the first red shape under another thread, and names a
    // blue shape under a red one, as blue lies over that first shape. Finding it has the budget's work: short
    // of it, SC-I0907 says that no shape is named.
    let colours = [&RED, &BLUE, &RED, &GREEN, &RED, &GREEN, &BLUE];
    let sewn: Vec<_> = colours.iter().enumerate().map(|(at, thread)| square(&format!("s{at}"), (20.0 * at as f64, 0.0), 10.0, thread)).collect();
    let shapes = Shapes::new(sewn, &DesignSettings::default(), &Budget::DEFAULT);
    let facts: Vec<Facts> = colours.iter().map(|thread| Facts { colours: vec![thread.color], stop: false, reach: None }).collect();
    let moving: Vec<Moving> = (0..colours.len()).map(|shape| Moving { free: true, made: true, shape }).collect();
    let order = Order {
        facts: &facts,
        moving: &moving,
        graph: Graph::new(colours.len()),
        borders: Vec::new(),
        wished: Vec::new(),
        last: true,
        details: Vec::new(),
        closed: BTreeMap::new(),
        kept: BTreeSet::new(),
        unsought: None,
        under: None,
        once: true,
        overlaps: vec![(6, 0), (0, 1), (1, 2), (3, 4), (4, 5)],
        lined: vec![(2, 1), (0, 1), (4, 3), (5, 3), (2, 2)],
        found: vec![(vec![6], vec![vec![6]]), (vec![0, 1, 2, 3, 4, 5], vec![vec![0], vec![1], vec![3], vec![2, 4], vec![5]])],
        chosen: None,
    };
    let warning = "warning SC-W0925: This shape lies under \"s1\", and \"s1\" lies under \"s2\": their threads lie both under and over each \
                   other, so no order threads each thread once. The threads are threaded #0000c8 once, #00a000 2 times, #c80000 2 \
                   times. [s0]";
    // The blue s1 sews before the red s2 it borders, and the green s3 before the red s4 and the green s5:
    // s3 has a fill of another thread sewn after it on one side, as s1 has, and is named after it.
    let lines = "info SC-I0926: 2 lines sew before a fill of another thread beside them, so that each thread is threaded once: the fill's \
                 rows end on the line and may cover up to half its thread on that side. Those with such fills on the most sides, the \
                 smallest first: \"s1\", \"s3\". [s1]";
    let mut first = None;
    for work in 0..200 {
        let said = messages(&told(&order, &[6, 0, 1, 3, 2, 4, 5], &shapes, &Budget { max_work: work, ..Budget::DEFAULT }));
        if said == [warning.to_string(), lines.to_string()] {
            first = first.or(Some(work));
        } else {
            assert_eq!(
                said,
                [format!(
                    "info SC-I0907: Finding the threads that the shapes as drawn sew both before and after another thread needed more than \
                     the work budget of {work} units, so no shape is named for a thread threaded more than once."
                )]
            );
        }
    }
    assert_eq!(first, Some(150));
}
