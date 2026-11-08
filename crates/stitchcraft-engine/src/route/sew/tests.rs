use stitchcraft_core::Budget;

use super::*;
use crate::design::DesignSettings;
use crate::route::read::routables;
use crate::route::tests::{p, stroke};

fn satin(routable: usize, first: usize, last: usize, back: bool) -> Op {
    Op::Satin { routable, first, last, back }
}

fn run(routable: usize, points: &[(f64, f64)], under: bool) -> Op {
    Op::Run { routable, points: points.iter().map(|&(x, y)| p(x, y)).collect(), under }
}

#[test]
fn a_jump_trims_when_more_than_1_mm_of_it_lies_outside_the_elements_it_joins() {
    // Strokes `x` mm apart, and a jump straight across from one's start to the other's: all of it outside.
    let long_at = |x: f64| {
        let elements = [stroke("a", &[(0.0, 0.0), (0.0, 5.0)], &[]), stroke("b", &[(x, 0.0), (x, 5.0)], &[])];
        let routables = routables(&elements, &DesignSettings::default(), &mut Vec::new(), &mut Budget::DEFAULT.meter()).unwrap();
        let mut graph = Graph::new(false);
        graph.piece(0, 0, [p(0.0, 0.0), p(0.0, 5.0)]);
        graph.piece(1, 0, [p(x, 0.0), p(x, 5.0)]);
        long(&graph, &routables, [0, 2], &mut Budget::DEFAULT.meter()).unwrap()
    };
    // Exactly 1 mm doesn't trim.
    assert_eq!([long_at(0.5), long_at(1.0), long_at(1.0 + 1e-9), long_at(3.0)], [false, false, true, true]);
}

#[test]
fn pieces_in_a_row_are_one_element() {
    // A column's pieces that follow on along it, forwards or back.
    assert_eq!(merged(vec![satin(0, 0, 0, false), satin(0, 1, 1, false), satin(0, 2, 2, false)]), [satin(0, 0, 2, false)]);
    assert_eq!(merged(vec![satin(0, 5, 5, true), satin(0, 4, 4, true), satin(0, 3, 3, true)]), [satin(0, 3, 5, true)]);
    // Not across a gap, from another column, the other way, or back along it the way they go forwards.
    for pair in [
        [satin(0, 0, 0, false), satin(0, 2, 2, false)],
        [satin(0, 0, 0, false), satin(1, 1, 1, false)],
        [satin(0, 0, 0, false), satin(0, 1, 1, true)],
        [satin(0, 4, 4, true), satin(0, 5, 5, true)],
        [satin(0, 1, 1, false), satin(0, 0, 0, false)],
        [satin(0, 0, 0, false), run(0, &[(0.0, 0.0), (1.0, 0.0)], true)],
    ] {
        assert_eq!(merged(pair.to_vec()), pair, "{pair:?}");
    }
    // Runs along one element, their joint once, under a satin when both are.
    let [a, b, c] = [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)];
    assert_eq!(merged(vec![run(0, &[a, b], true), run(0, &[b, c], false)]), [run(0, &[a, b, c], false)]);
    assert_eq!(merged(vec![run(0, &[a, b], true), run(0, &[b, c], true)]), [run(0, &[a, b, c], true)]);
    assert_eq!(merged(vec![run(0, &[a, b], false), run(0, &[c, a], false)]), [run(0, &[a, b, c, a], false)]);
    // Not along another element, or when the second goes back the way the first came.
    for pair in [[run(0, &[a, b], true), run(1, &[b, c], true)], [run(0, &[a, b], true), run(0, &[b, a], true)]] {
        assert_eq!(merged(pair.to_vec()), pair, "{pair:?}");
    }
}

#[test]
fn a_trim_leaves_out_the_runs_right_after_it_and_the_last_element_trims_too() {
    let made = |satin: bool| Made { piece: Piece { element: stroke("m", &[(0.0, 0.0), (1.0, 0.0)], &[]), from: 0, under: !satin }, satin };
    let trims = |kept: &[Made]| kept.iter().map(|made| (made.satin, made.piece.element.params.get(TRIM_AFTER) == Some("true"))).collect::<Vec<_>>();
    let kept = trimmed(vec![made(true), made(false), made(false), made(true), made(false)], &BTreeSet::from([0]));
    assert_eq!(trims(&kept), [(true, true), (true, false), (false, true)]);
    // A run before a trim stays, and trims.
    let kept = trimmed(vec![made(false), made(true), made(true)], &BTreeSet::from([0, 1]));
    assert_eq!(trims(&kept), [(false, true), (true, true), (true, true)]);
}

#[test]
fn pieces_are_numbered_in_order_and_with_preserve_order_keep_their_column_s_name() {
    let mut named_column = stroke("c", &[(0.0, 0.0), (1.0, 0.0)], &[]);
    named_column.name = Some("Arm".to_string());
    let elements = [named_column, stroke("d", &[(0.0, 0.0), (1.0, 0.0)], &[])];
    let made = |from: usize, satin: bool| Made { piece: Piece { element: elements[from].clone(), from, under: false }, satin };
    let names = |preserve_order: bool| {
        let mut all = vec![made(0, true), made(0, false), made(1, true), made(0, true)];
        named(&mut all, &elements, preserve_order);
        all.into_iter().map(|made| made.piece.element.name.unwrap()).collect::<Vec<_>>()
    };
    assert_eq!(names(false), ["AutoSatin 1", "AutoSatin Running Stitch 2", "AutoSatin 3", "AutoSatin 4"]);
    assert_eq!(names(true), ["Arm", "AutoSatin Running Stitch 1", "AutoSatin 2", "Arm"]);
}

#[test]
fn a_run_has_its_element_s_lengths_and_no_point_twice() {
    let elements = [stroke("s", &[(0.0, 0.0), (5.0, 0.0)], &[("min_jump_stitch_length_mm", "2"), ("repeats", "3")])];
    let routables = routables(&elements, &DesignSettings::default(), &mut Vec::new(), &mut Budget::DEFAULT.meter()).unwrap();
    let points = [p(0.0, 0.0), p(0.0, 1e-7), p(1.0, 0.0), p(1.0, 0.0)];
    let made = super::run(&routables[0], &elements, &points, false, 3).unwrap();
    assert_eq!(made.element.id.as_str(), "s#3");
    assert_eq!(made.element.params.iter().collect::<Vec<_>>(), [("min_jump_stitch_length_mm", "2")]);
    assert_eq!(made.element.shape.path().subpaths[0].segments.len(), 1, "{:?}", made.element.shape);
    assert_eq!(super::run(&routables[0], &elements, &[], false, 3), None);
}
