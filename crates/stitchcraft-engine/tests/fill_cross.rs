//! A cross stitch fill (`REQ-FILL-CRS-001` to `REQ-FILL-CRS-006`), sewn as whole designs: the grid, the
//! coverage, the crosses and their runs are tested in the generator's modules (`generators::cross`); these
//! cases sew designs through every stage, with what is said of them.

// Test code may unwrap, index and panic (clippy.toml allows the first 2 in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use stitchcraft_core::{Budget, Code, Point};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::{DesignSettings, Element, Hints};
use stitchcraft_engine::generate::{approach, rows};
use stitchcraft_engine::generators::{Approach, Neighbours};
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Role, StitchKind};
use stitchcraft_testkit::designs::{RED, fill, line, messages, p, planned};

/// The square `size` mm a side from (`x`, `y`).
fn square((x, y): (f64, f64), size: f64) -> Vec<(f64, f64)> {
    vec![(x, y), (x + size, y), (x + size, y + size), (x, y + size)]
}

/// A cross stitch fill of `rings` with `own` settings, named `cross`.
fn cross(rings: &[&[(f64, f64)]], own: &[(&str, &str)]) -> Element {
    fill("cross", rings, &RED, &[&[("fill_method", "cross_stitch")], own].concat())
}

/// The needle points `outcome` sews as its elements' top stitches.
fn needles(outcome: &PlanOutcome) -> Vec<Point> {
    let plan = outcome.plan.as_ref().unwrap_or_else(|| panic!("{:?}", messages(outcome)));
    plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role == Role::Top).map(|s| s.at).collect()
}

/// The jumps `outcome` sews.
fn jumps(outcome: &PlanOutcome) -> usize {
    outcome.plan.as_ref().unwrap().stitches().filter(|s| s.kind == StitchKind::Jump).count()
}

#[test]
fn req_fill_crs_001_a_square_on_the_grid_sews_its_cells_and_another_square_s_crosses_line_up_with_them() {
    // Two squares 12 mm a side, 3 mm apart on the canvas's 3 mm grid: 16 crosses each, of 4 threads, each
    // square one run from a corner back to it. The second starts 3 mm from where the first ends, no farther
    // than the collapse length, and is sewn straight on from it: the only jump is the design's first.
    let left = square((0.0, 0.0), 12.0);
    let right = square((15.0, 0.0), 12.0);
    let sewn = planned(vec![cross(&[&left, &right], &[])]);
    assert_eq!(messages(&sewn), ["warning SC-W0307: This fill's area falls into 2 parts, sewn one after another, each on its own."]);
    let points = needles(&sewn);
    assert_eq!(points.len(), 2 * (16 * 10 + 1));
    // Every needle point lies on a corner or a middle of a cell of the grid.
    let on_grid = |v: f64| ((v / 1.5).round() * 1.5 - v).abs() < 1e-9;
    assert!(points.iter().all(|q| on_grid(q.x()) && on_grid(q.y())), "{points:?}");
    assert_eq!(jumps(&sewn), 1);
    // Off the canvas's grid, moved 1 mm right, the same, from x = 1. On it, the square moved 1 mm right
    // covers two thirds of the column of cells from x = 0, which sews, and a third of the column from x = 12,
    // which doesn't: 4 columns of crosses from x = 0 to 12.
    let moved = square((1.0, 0.0), 12.0);
    let off = needles(&planned(vec![cross(&[&moved], &[("canvas_grid_origin", "false")])]));
    assert_eq!((off.len(), off.iter().map(|q| q.x()).fold(f64::INFINITY, f64::min)), (16 * 10 + 1, 1.0));
    let on = needles(&planned(vec![cross(&[&moved], &[])]));
    let xs = on.iter().map(|q| q.x());
    assert_eq!((on.len(), xs.clone().fold(f64::INFINITY, f64::min), xs.fold(f64::NEG_INFINITY, f64::max)), (16 * 10 + 1, 0.0, 12.0));
}

#[test]
fn diag_sc_w0319_a_fill_that_covers_no_cell_enough_sews_nothing() {
    // A square 1 mm a side covers a ninth of a 3 mm cell. Alone it sews nothing; beside a part that sews, it
    // is counted; and 2 such parts are said together.
    let speck = |x: f64| square((x, 1.0), 1.0);
    let big = square((30.0, 0.0), 12.0);
    let alone = planned(vec![cross(&[&speck(1.0)], &[]), line("line", (0.0, 20.0), 10.0, &RED, &[])]);
    assert_eq!(
        messages(&alone),
        [
            "warning SC-W0304: This fill covers 1 mm², less than 1.4 mm², so a running stitch round it or a satin column across it would probably \
             sew it better.",
            "warning SC-W0319: This cross stitch fill covers no cell of its grid by 50 % or more, so it sews nothing."
        ]
    );
    let one = planned(vec![cross(&[&speck(1.0), &big], &[])]);
    assert_eq!(
        messages(&one)[1],
        "warning SC-W0319: 1 of this cross stitch fill's parts covers no cell of its grid by 50 % or more, so it sews nothing."
    );
    let two = planned(vec![cross(&[&speck(1.0), &speck(10.0), &big], &[("fill_coverage", "12.5")])]);
    assert_eq!(
        messages(&two)[1],
        "warning SC-W0319: 2 of this cross stitch fill's parts cover no cell of its grid by 12.5 % or more, so they sew nothing."
    );
    // At 11 %, a ninth of a cell is enough.
    assert_eq!(needles(&planned(vec![cross(&[&speck(1.0)], &[("fill_coverage", "11")])])).len(), 11);
}

#[test]
fn diag_sc_w0320_an_odd_thread_count_sews_the_even_count_below_it_and_1_sews_2() {
    let one = square((0.0, 0.0), 3.0);
    let sewn = |count: &str| planned(vec![cross(&[&one], &[("cross_thread_count", count)])]);
    for (count, threads) in [("1", 2), ("3", 2), ("5", 4)] {
        let odd = sewn(count);
        assert_eq!(
            messages(&odd),
            [format!(
                "warning SC-W0320: `cross_thread_count` is {count}, and a full cross takes an even number of passes on each \
                 diagonal, so it sews {threads}."
            )]
        );
        assert_eq!(needles(&odd).len(), 2 * threads + 3, "{count}");
    }
    // An even count says nothing, and 0 is raised to 1, which sews 2.
    for (count, points) in [("2", 7), ("4", 11), ("6", 15)] {
        let even = sewn(count);
        assert_eq!((messages(&even), needles(&even).len()), (Vec::<String>::new(), points), "{count}");
    }
    assert_eq!(
        messages(&sewn("0")),
        [
            "warning SC-W0102: `cross_thread_count` is 0, outside 1 to 20; 1 is used.",
            "warning SC-W0320: `cross_thread_count` is 1, and a full cross takes an even number of passes on each diagonal, so it sews 2."
        ]
    );
}

#[test]
fn diag_sc_w0321_a_grid_too_fine_for_the_shortest_stitch_leaves_needle_points_out() {
    // Cells of 0.4 mm: a stitch from a corner to the middle is 0.28 mm, shorter than the reference
    // machine's shortest stitch of 0.3 mm, so the middles are left out where they lie too near.
    let small = square((0.0, 0.0), 1.2);
    let fine = planned(vec![cross(&[&small], &[("pattern_size_mm", "0.4"), ("cross_thread_count", "2")])]);
    assert_eq!(
        messages(&fine),
        ["warning SC-W0321: This cross stitch fill's cells are 0.4 by 0.4 mm, so the stitch from a corner of a cross to its middle \
             is 0.28 mm long, shorter than the shortest stitch (0.3 mm), and needle points of the crosses are left out."]
    );
    let points = needles(&fine);
    assert!(points.len() < 9 * 6 + 1, "{}", points.len());
    assert!(points.windows(2).all(|w| w[0].distance(w[1]) >= 0.3 - 1e-9), "{points:?}");
    // At 0.43 mm, the stitch is 0.304 mm, and nothing is said.
    let fit = planned(vec![cross(&[&square((0.0, 0.0), 4.3)], &[("pattern_size_mm", "0.43"), ("cross_thread_count", "2")])]);
    assert_eq!(messages(&fit), Vec::<String>::new());
    assert_eq!(needles(&fit).len(), 100 * 6 + 1);
}

#[test]
fn diag_sc_w0402_a_longest_cross_stitch_below_twice_the_shortest_is_raised_to_it() {
    let one = square((0.0, 0.0), 3.0);
    let sewn = planned(vec![cross(&[&one], &[("max_cross_stitch_length_mm", "0.5"), ("cross_thread_count", "2")])]);
    assert_eq!(
        messages(&sewn),
        ["warning SC-W0402: The stitch length 0.5 mm (`max_cross_stitch_length_mm`) is shorter than twice the shortest stitch (0.3 \
          mm), so 0.6 mm is used."]
    );
    let points = needles(&sewn);
    assert!(points.windows(2).all(|w| w[0].distance(w[1]) <= 0.6 + 1e-9));
    // A half diagonal of 2.12 mm in 4 stitches and a diagonal in 8, at most 0.6 mm each.
    assert_eq!(points.len(), 1 + 4 + 4 + 8 + 4 + 4 + 8);
}

#[test]
fn diag_sc_w0011_a_cross_stitch_method_of_a_later_version_is_skipped() {
    let one = square((0.0, 0.0), 3.0);
    let sewn = planned(vec![cross(&[&one], &[("cross_stitch_method", "half_cross")]), line("line", (0.0, 10.0), 10.0, &RED, &[])]);
    assert_eq!(
        messages(&sewn),
        ["warning SC-W0011: This element's cross stitch method, `half_cross`, is not sewn by this version of StitchCraft yet, so it \
          is skipped."]
    );
}

#[test]
fn req_fill_crs_006_expand_joins_parts_that_meet_and_a_part_shrunk_away_is_sewn_at_its_size() {
    // 2 rectangles 0.2 mm apart, each covering 49 % of the cells along the gap: apart, those cells sew no
    // cross; grown by 0.1 mm, they are one part, which covers the cells by 99.6 %.
    let left = [(0.0, 0.0), (4.4, 0.0), (4.4, 3.0), (0.0, 3.0)];
    let right = [(4.6, 0.0), (9.0, 0.0), (9.0, 3.0), (4.6, 3.0)];
    let apart = planned(vec![cross(&[&left, &right], &[("cross_thread_count", "2")])]);
    assert_eq!(needles(&apart).len(), 2 * (6 + 1));
    let grown = planned(vec![cross(&[&left, &right], &[("cross_thread_count", "2"), ("expand_mm", "0.1")])]);
    let two_parts = "warning SC-W0307: This fill's area falls into 2 parts, sewn one after another, each on its own.";
    assert_eq!((messages(&grown), needles(&grown).len()), (vec![two_parts.to_string()], 3 * 6 + 1));
    // Shrunk by 1 mm, a strip 1.5 mm wide is shrunk away and sewn at its own size, and a square 6 mm wide is
    // shrunk to 4 mm, which covers a cell 3 mm wide by 4/9.
    let strip = [(0.0, 0.0), (1.5, 0.0), (1.5, 6.0), (0.0, 6.0)];
    let shrunk = planned(vec![cross(&[&strip], &[("expand_mm", "-1"), ("fill_coverage", "45")])]);
    assert_eq!(messages(&shrunk), ["warning SC-W0309: `expand_mm` shrinks this fill away entirely, so its crosses are sewn over all of it."]);
    assert_eq!(needles(&shrunk).len(), 2 * 10 + 1);
    let wide = planned(vec![cross(&[&square((0.0, 0.0), 6.0)], &[("expand_mm", "-1"), ("fill_coverage", "44")])]);
    assert_eq!((messages(&wide), needles(&wide).len()), (Vec::<String>::new(), 4 * 10 + 1));
}

#[test]
fn req_gen_004_a_fine_grid_over_a_large_part_runs_out_of_its_budget_before_it_takes_memory() {
    // A square 3 m wide on a grid of 0.1 mm reaches 9 × 10^8 cells, past the default budget's work, which
    // they spend before any is looked at.
    let exhausted = |side: f64| {
        let sewn = planned(vec![cross(&[&square((0.0, 0.0), side)], &[("pattern_size_mm", "0.1")])]);
        assert!(sewn.plan.is_none());
        sewn.diagnostics.into_iter().find(|d| d.code == Code::BudgetExhausted).map(|d| (d.element.map(|e| e.to_string()), d.message))
    };
    let work = exhausted(3000.0).unwrap();
    assert_eq!(work.0.as_deref(), Some("cross"));
    assert!(work.1.contains("work"), "{}", work.1);
    // A square 1 m wide reaches 10^8 cells, within the work, but its crosses are more than the design's
    // stitches: no more of them are held than could be sewn.
    assert_eq!(exhausted(1000.0).unwrap().1, "The design needs more than 2000000 stitches, the limit for one design.");
}

#[test]
fn a_cross_stitch_fill_offers_its_area_and_its_preview_shows_its_crosses_diagonals() {
    let one = cross(&[&square((0.0, 0.0), 6.0)], &[]);
    assert!(matches!(approach(&one, &DesignSettings::default(), &Budget::DEFAULT), Some(Approach::Area(parts)) if parts.len() == 1));
    let preview = rows(&one, &DesignSettings::default(), REFERENCE, &Budget::DEFAULT);
    let segments = preview.segments.unwrap();
    assert_eq!(segments.len(), 2 * 4);
    assert!(segments.iter().all(|s| (s.start.distance(s.end) - 3.0 * 2f64.sqrt()).abs() < 1e-9));
    // A method of a later version shows nothing, and says why, as sewing it does.
    let later = cross(&[&square((0.0, 0.0), 6.0)], &[("cross_stitch_method", "smyrna_cross")]);
    let preview = rows(&later, &DesignSettings::default(), REFERENCE, &Budget::DEFAULT);
    assert_eq!((preview.segments, preview.diagnostics.len()), (None, 1));
    // Generated after a line, the fill starts at its corner nearest where the line ended.
    let generated = stitchcraft_engine::generate::generate(
        &Element { hints: Hints::default(), ..one },
        &DesignSettings::default(),
        REFERENCE,
        &Neighbours { needle: Some(p(10.0, 7.0)), next: None },
        &Budget::DEFAULT,
    );
    assert_eq!(generated.generated.unwrap().groups[0][0], p(6.0, 6.0));
}
