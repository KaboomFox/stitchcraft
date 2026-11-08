//! The generate step's unit tests: settings read, methods picked, and what an element offers.

use stitchcraft_core::ElementId;
use stitchcraft_plan::{Rgb, Thread};

use super::*;
use crate::design::{Cap, FillRule, GradientStop, Segment, Subpath};

fn element(shape: Shape, params: &[(&str, &str)]) -> Element {
    let thread = Thread::new(Rgb::new(0, 0, 0));
    Element {
        id: ElementId::new("e").unwrap(),
        name: None,
        shape,
        thread,
        params: params.iter().copied().collect(),
        automatic: false,
        hints: Default::default(),
        clips: Vec::new(),
    }
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

fn codes(diagnostics: &[Diagnostic]) -> Vec<Code> {
    diagnostics.iter().map(|d| d.code).collect()
}

#[test]
fn diag_sc_w0402_a_fill_s_longest_stitch_below_twice_the_shortest_is_raised() {
    // A 10 mm square on the reference machine, whose shortest stitch is 0.3 mm: rows stitched 0.2 mm
    // long would sew stitches it merges. Its longest stitch is raised to 0.6 mm, and its underlay's
    // with it, as a running stitch's length is.
    let segments = [p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)].map(Segment::Line).to_vec();
    let square = Shape::Fill {
        path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] },
        rule: FillRule::NonZero,
        gradient: None,
    };
    let sewn = |params: &[(&str, &str)]| {
        generate(
            &element(square.clone(), params),
            &DesignSettings::default(),
            stitchcraft_plan::profiles::REFERENCE,
            &Neighbours::default(),
            &Budget::DEFAULT,
        )
    };
    let raised = sewn(&[("max_stitch_length_mm", "0.2"), ("fill_underlay", "false")]);
    let said: Vec<String> = raised.diagnostics.iter().map(ToString::to_string).collect();
    assert_eq!(
        said,
        ["warning SC-W0402: The stitch length 0.2 mm (`max_stitch_length_mm`) is shorter than twice the shortest stitch (0.3 mm), so 0.6 mm \
              is used."]
    );
    let points: Vec<Point> = raised.generated.unwrap().groups.concat();
    let along: Vec<f64> = points.windows(2).filter(|w| (w[0].y() - w[1].y()).abs() < 1e-9).map(|w| w[0].distance(w[1])).collect();
    // Of the stitches along the rows, all but their ends' and the needle's way in are 0.6 mm long.
    let raised_ones = along.iter().filter(|length| (*length - 0.6).abs() < 1e-6).count();
    assert!(raised_ones * 10 > along.len() * 9, "{raised_ones} of {}: {along:?}", along.len());
    let underlay = sewn(&[("fill_underlay_max_stitch_length_mm", "0.25")]);
    assert_eq!(
        underlay.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["warning SC-W0402: The stitch length 0.25 mm (`fill_underlay_max_stitch_length_mm`) is shorter than twice the shortest stitch \
              (0.3 mm), so 0.6 mm is used."]
    );
    // Without underlay, its longest stitch says nothing; nor does a longest stitch of 3 mm. A legacy fill
    // has no underlay whatever its settings say (`REQ-FILL-005`): it says only that it is sewn so.
    assert!(sewn(&[("fill_underlay", "false"), ("fill_underlay_max_stitch_length_mm", "0.25")]).diagnostics.is_empty());
    let legacy = sewn(&[("fill_method", "legacy_fill"), ("fill_underlay_max_stitch_length_mm", "0.25")]);
    assert_eq!(codes(&legacy.diagnostics), [Code::FillLegacyAsTatami]);
    assert!(sewn(&[("max_stitch_length_mm", "3")]).diagnostics.is_empty());
}

#[test]
fn a_fill_s_top_rows_are_laid_as_its_top_stitches_are_without_sewing_it() {
    let segments = [p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)].map(Segment::Line).to_vec();
    let square = Shape::Fill {
        path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] },
        rule: FillRule::NonZero,
        gradient: None,
    };
    let laid = |shape: &Shape, params: &[(&str, &str)], budget: &Budget| {
        rows(&element(shape.clone(), params), &DesignSettings::default(), stitchcraft_plan::profiles::REFERENCE, budget)
    };
    // Rows a quarter of a millimetre apart, across the 10 mm square, each one segment from side to side.
    let across = |rows: &Rows| rows.segments.as_ref().map(|segments| (segments.len(), segments.iter().map(|s| s.start.distance(s.end)).sum::<f64>()));
    let plain = laid(&square, &[], &Budget::DEFAULT);
    assert_eq!((across(&plain), plain.diagnostics), (Some((40, 400.0)), vec![]));
    // Grown by `expand_mm` as the top stitches are, and the generator's own rows: every top needle point
    // of the fill without its underlay lies on one of them.
    let grown = laid(&square, &[("expand_mm", "1")], &Budget::DEFAULT);
    assert_eq!(across(&grown).map(|(count, _)| count), Some(48));
    let sewn = generate(
        &element(square.clone(), &[("fill_underlay", "false")]),
        &DesignSettings::default(),
        stitchcraft_plan::profiles::REFERENCE,
        &Neighbours::default(),
        &Budget::DEFAULT,
    );
    let tops = plain.segments.unwrap_or_default();
    let on_a_row = |q: &Point| tops.iter().any(|s| (q.y() - s.start.y()).abs() < 1e-9 && q.x() >= -1e-9 && q.x() <= 10.0 + 1e-9);
    let points: Vec<Point> = sewn.generated.unwrap().groups.into_iter().flatten().collect();
    assert!(points.iter().filter(|q| on_a_row(q)).count() * 10 > points.len() * 9, "most needle points lie on the rows");
    // A contour fill has no rows: its path round its rings, 1 mm apart here, squares 10, 8, 6, 4 and 2 mm
    // across, each sewn round to 1 mm short of where it was entered, the outline from its corner at the
    // origin, and the steps between them.
    let contour = laid(&square, &[("fill_method", "contour_fill"), ("row_spacing_mm", "1")], &Budget::DEFAULT);
    let path = contour.segments.unwrap_or_default();
    let long: f64 = path.iter().map(|s| s.start.distance(s.end)).sum();
    assert!((115.0..130.0).contains(&long) && contour.diagnostics.is_empty(), "{long} mm: {:?}", contour.diagnostics);
    assert_eq!(path.first().map(|s| s.start), Some(p(0.0, 0.0)));
    assert!(path.iter().all(|s| [s.start, s.end].iter().all(|q| (0.0..=10.0).contains(&q.x()) && (0.0..=10.0).contains(&q.y()))));
    assert!(path.windows(2).all(|w| w[0].end == w[1].start), "one path");
    // Nor has a meander fill: its path through its tile, one path in the square, as the element's seed
    // lays it, so that another seed lays another over a square 20 mm a side.
    let segments = [p(20.0, 0.0), p(20.0, 20.0), p(0.0, 20.0)].map(Segment::Line).to_vec();
    let large = Shape::Fill {
        path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] },
        rule: FillRule::NonZero,
        gradient: None,
    };
    let meander = |seed: &str| laid(&large, &[("fill_method", "meander_fill"), ("random_seed", seed)], &Budget::DEFAULT);
    let wandered = meander("1");
    let path = wandered.segments.clone().unwrap_or_default();
    assert!(path.len() > 50 && wandered.diagnostics.is_empty(), "{} segments: {:?}", path.len(), wandered.diagnostics);
    assert!(path.windows(2).all(|w| w[0].end == w[1].start), "one path");
    assert!(path.iter().all(|s| [s.start, s.end].iter().all(|q| (0.0..=20.0).contains(&q.x()) && (0.0..=20.0).contains(&q.y()))));
    assert_ne!(meander("2").segments, wandered.segments);
    // Not a fill, a fill not sewn, a method not sewn yet, and a budget spent: no rows, and the same words
    // as generating it.
    let line = Shape::stroke(Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(5.0, 0.0))], closed: false }] });
    assert_eq!(laid(&line, &[], &Budget::DEFAULT), Rows { segments: None, diagnostics: vec![] });
    assert_eq!(laid(&square, &[("fill", "false")], &Budget::DEFAULT), Rows { segments: None, diagnostics: vec![] });
    let offset = laid(&square, &[("fill_method", "guided_fill"), ("guided_fill_strategy", "1")], &Budget::DEFAULT);
    assert_eq!((offset.segments, codes(&offset.diagnostics)), (None, vec![Code::StitchTypeNotYet]));
    // A guided fill without a guide shows the rows of the tatami fill it sews.
    let unguided = laid(&square, &[("fill_method", "guided_fill")], &Budget::DEFAULT);
    assert_eq!(unguided.segments.map(|segments| segments.len()), Some(40));
    // A tartan fill shows the lines of its colours, without a sett Ink/Stitch's default one's, which it
    // says, and nothing when its sett can't be read.
    let tartan = laid(&square, &[("fill_method", "tartan_fill")], &Budget::DEFAULT);
    assert!(tartan.segments.is_some_and(|segments| !segments.is_empty()));
    assert_eq!(codes(&tartan.diagnostics), [Code::FillTartanNoSett]);
    let unread = laid(&square, &[("fill_method", "tartan_fill"), ("tartan", "{")], &Budget::DEFAULT);
    assert_eq!((unread.segments, codes(&unread.diagnostics)), (None, vec![Code::FillTartanSettUnreadable]));
    // A linear gradient fill shows its colours' rows, its own colour's fading out without a gradient, which
    // it says, and nothing when its settings can't be read.
    let gradient = laid(&square, &[("fill_method", "linear_gradient_fill")], &Budget::DEFAULT);
    assert!(gradient.segments.is_some_and(|segments| !segments.is_empty()));
    assert_eq!(codes(&gradient.diagnostics), [Code::FillGradientNone]);
    let unread = laid(&square, &[("fill_method", "linear_gradient_fill"), ("row_spacing_mm", "wide")], &Budget::DEFAULT);
    assert_eq!((unread.segments, codes(&unread.diagnostics)), (None, vec![Code::ParamInvalid]));
    let spent = laid(&square, &[], &Budget { max_stitches: 1, max_work: 10 });
    assert_eq!((spent.segments, codes(&spent.diagnostics)), (None, vec![Code::BudgetExhausted]));
    assert!(spent.diagnostics.iter().all(|d| d.element.as_ref().map(ElementId::as_str) == Some("e")));
}

#[test]
fn a_fill_is_sewn_by_its_method_and_without_a_longest_stitch_is_a_bug() {
    let segments = [p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)].map(Segment::Line).to_vec();
    let square = Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] };
    let region = region::build(&square, FillRule::NonZero, &mut Budget::DEFAULT.meter()).unwrap().region;
    let shared = |max| Some(Shared { min_stitch: Mm::new(0.3).unwrap(), max_stitch: max, jump: Mm::new(3.0).unwrap(), force_locks: false });
    let neighbours = Neighbours::default();
    let sew = |set: &ParamSet, stitch_type, shared, diagnostics: &mut Vec<Diagnostic>| {
        let (rng, meter) = (&mut SplitMix64::new(1), &mut Budget::DEFAULT.meter());
        let sewing = Sewing { set: Cow::Borrowed(set), stitch_type, shared, hints: Hints::default(), own: Rgb::new(0, 0, 0), gradient: None };
        fill(&sewing, &neighbours, &region, rng, diagnostics, meter).unwrap()
    };
    let mut diagnostics = Vec::new();
    let sewn = sew(&ParamSet::new(), StitchType::TatamiFill, shared(Mm::new(4.0).ok()), &mut diagnostics);
    assert_eq!((sewn.is_some(), codes(&diagnostics)), (true, vec![]));
    // A contour fill and a meander fill, each by its own generator.
    for stitch_type in [StitchType::ContourFill, StitchType::MeanderFill] {
        let sewn = sew(&ParamSet::new(), stitch_type, shared(Mm::new(4.0).ok()), &mut diagnostics);
        assert_eq!((sewn.is_some_and(|s| !s.runs.is_empty()), codes(&diagnostics)), (true, vec![]), "{stitch_type:?}");
    }
    // A tartan fill by its own, in a thread for each run, here Ink/Stitch's default sett, which it says.
    let mut said = Vec::new();
    let sewn = sew(&ParamSet::new(), StitchType::TartanFill, shared(Mm::new(4.0).ok()), &mut said);
    assert_eq!((sewn.is_some_and(|s| !s.runs.is_empty() && s.runs.len() == s.colours.len()), codes(&said)), (true, vec![Code::FillTartanNoSett]));
    // Without a longest stitch, the same bug as a tatami fill's, and settings that can't be read skip it.
    let mut said = Vec::new();
    assert!(sew(&ParamSet::new(), StitchType::TartanFill, shared(None), &mut said).is_none());
    assert_eq!(codes(&said), [Code::InternalCheckFailed]);
    let wrong = [("rows_per_thread", "many")].into_iter().collect();
    let mut said = Vec::new();
    assert!(sew(&wrong, StitchType::TartanFill, shared(Mm::new(4.0).ok()), &mut said).is_none());
    assert_eq!(codes(&said), [Code::ParamInvalid]);
    // A guided fill's strategy not sewn yet, whose settings it does not read, wrong as they are here.
    let tatami = [("row_spacing_mm", "wide"), ("guided_fill_strategy", "2")].into_iter().collect();
    let sewn = sew(&tatami, StitchType::GuidedFill, shared(Mm::new(4.0).ok()), &mut diagnostics);
    assert_eq!((sewn.is_none(), codes(&diagnostics)), (true, vec![Code::StitchTypeNotYet]));
    // Copying its guide, it reads them, and is skipped.
    let wrong = [("row_spacing_mm", "wide")].into_iter().collect();
    let mut said = Vec::new();
    assert!(sew(&wrong, StitchType::GuidedFill, shared(Mm::new(4.0).ok()), &mut said).is_none());
    assert_eq!(codes(&said), [Code::ParamInvalid]);
    // Every other method reads its own settings: only a bug would bring one to the tatami fill's, which
    // skip it rather than sew it as a tatami fill.
    let set = ParamSet::new();
    let other = Sewing {
        set: Cow::Borrowed(&set),
        stitch_type: StitchType::GuidedFill,
        shared: shared(Mm::new(4.0).ok()),
        hints: Hints::default(),
        own: Rgb::new(0, 0, 0),
        gradient: None,
    };
    let mut said = Vec::new();
    assert!(fill_settings(&other, &mut said).is_none());
    assert_eq!(codes(&said), [Code::StitchTypeNotYet]);
    // A contour fill without a longest stitch is the same bug as a tatami fill's.
    let mut diagnostics = Vec::new();
    assert!(sew(&ParamSet::new(), StitchType::ContourFill, shared(None), &mut diagnostics).is_none());
    assert_eq!(codes(&diagnostics), [Code::InternalCheckFailed]);
    // The registry gives a fill a longest stitch of its own (`ParamSpec::in_family`), so only a bug in
    // StitchCraft can leave it out.
    let mut diagnostics = Vec::new();
    let sewn = sew(&ParamSet::new(), StitchType::TatamiFill, shared(None), &mut diagnostics);
    assert_eq!((sewn.is_none(), codes(&diagnostics)), (true, vec![Code::InternalCheckFailed]));
}

#[test]
fn a_tartan_s_threads_are_read_from_its_settings_and_every_other_element_s_is_its_own() {
    let segments = [p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)].map(Segment::Line).to_vec();
    let square = Shape::Fill {
        path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] },
        rule: FillRule::NonZero,
        gradient: None,
    };
    let (black, red, blue) = (Rgb::new(0, 0, 0), Rgb::new(255, 0, 0), Rgb::new(0, 0, 255));
    assert_eq!(colours(&element(square.clone(), &[])), [black]);
    let line = Shape::stroke(Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(5.0, 0.0))], closed: false }] });
    assert_eq!(colours(&element(line, &[("fill_method", "tartan_fill")])), [black]);
    // Its underlay in its own thread, then its sett's colours; without underlay, the colours alone.
    let sett = r#"{"palette": "(#ff0000)/5 (#0000ff)/5"}"#;
    assert_eq!(colours(&element(square.clone(), &[("fill_method", "tartan_fill"), ("tartan", sett)])), [black, red, blue]);
    let bare = [("fill_method", "tartan_fill"), ("tartan", sett), ("fill_underlay", "false")];
    assert_eq!(colours(&element(square.clone(), &bare)), [red, blue]);
    // An underlay setting that can't be read is read as on, as the fill's default.
    let wrong = [("fill_method", "tartan_fill"), ("tartan", sett), ("fill_underlay", "maybe")];
    assert_eq!(colours(&element(square, &wrong)), [black, red, blue]);
}

#[test]
fn req_fill_grd_008_a_gradient_fill_s_threads_are_read_from_its_paint() {
    let (black, red, blue) = (Rgb::new(0, 0, 0), Rgb::new(255, 0, 0), Rgb::new(0, 0, 255));
    let segments = [p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)].map(Segment::Line).to_vec();
    let stops = vec![GradientStop { offset: 0.0, colour: red, opacity: 1.0 }, GradientStop { offset: 1.0, colour: blue, opacity: 1.0 }];
    let square = Shape::Fill {
        path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] },
        rule: FillRule::NonZero,
        gradient: Some(Gradient { start: p(0.0, 0.0), end: p(10.0, 0.0), stops }),
    };
    // Its underlay in the first stop's colour, then the stops' colours; without underlay, the colours
    // alone. Another fill method sews the shape in its own thread.
    let method = ("fill_method", "linear_gradient_fill");
    assert_eq!(colours(&element(square.clone(), &[method])), [red, red, blue]);
    assert_eq!(colours(&element(square.clone(), &[method, ("fill_underlay", "false")])), [red, blue]);
    assert_eq!(colours(&element(square, &[])), [black]);
}

#[test]
fn each_fill_method_with_an_underlay_gives_it_and_sews_its_top_without_it() {
    // The underlay a fill in a run of underlays first sews apart, and the fill without it that sews its top.
    let shared = Some(Shared { min_stitch: Mm::new(0.3).unwrap(), max_stitch: Mm::new(4.0).ok(), jump: Mm::new(3.0).unwrap(), force_locks: false });
    let methods = [
        (StitchType::TatamiFill, true),
        (StitchType::ContourFill, true),
        (StitchType::MeanderFill, true),
        (StitchType::CircularFill, true),
        (StitchType::GuidedFill, true),
        (StitchType::TartanFill, true),
        (StitchType::LinearGradientFill, true),
        (StitchType::CrossStitch, false),
    ];
    for (stitch_type, underlaid) in methods {
        let set = ParamSet::new();
        let sewing = Sewing { set: Cow::Borrowed(&set), stitch_type, shared, hints: Hints::default(), own: Rgb::new(0, 0, 0), gradient: None };
        let method = FillMethod::read(&sewing, &mut Vec::new()).unwrap();
        let layers = |method: &FillMethod| method.underlay().map(|(layers, inset, travel)| (layers.len(), inset, travel.min_stitch));
        assert_eq!(layers(&method), underlaid.then_some((1, 0.0, 0.3)), "{stitch_type:?}");
        assert_eq!(layers(&method.without_underlay()), underlaid.then_some((0, 0.0, 0.3)), "{stitch_type:?}");
    }
    // A guided fill's top leaves out the underlay of the tatami fill it sews without a guide too.
    let set = ParamSet::new();
    let sewing = Sewing {
        set: Cow::Borrowed(&set),
        stitch_type: StitchType::GuidedFill,
        shared,
        hints: Hints::default(),
        own: Rgb::new(0, 0, 0),
        gradient: None,
    };
    let method = FillMethod::read(&sewing, &mut Vec::new()).unwrap();
    let FillMethod::Guided(guided) = &method else { panic!("{method:?}") };
    let FillMethod::Guided(top) = method.without_underlay() else { panic!("{method:?}") };
    let plain = |guided: &Guided| guided.plain.fill.as_ref().map(|fill| fill.underlay.len());
    assert_eq!((plain(guided), plain(&top)), (Some(1), Some(0)));
}

#[test]
fn the_settings_that_choose_it_give_an_element_s_stitch_type() {
    let path = Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(5.0, 0.0))], closed: false }] };
    let fill = || Shape::Fill { path: path.clone(), rule: FillRule::NonZero, gradient: None };
    let wide = || Shape::Stroke { path: path.clone(), width: Mm::new(3.0).unwrap(), join: Join::UNSET, cap: Cap::Butt };
    let chosen = |shape: Shape, params: &[(&str, &str)]| {
        let mut diagnostics = Vec::new();
        (stitch_type(&element(shape, params), &DesignSettings::default(), &mut diagnostics), codes(&diagnostics))
    };
    assert_eq!(chosen(fill(), &[]), (Some(StitchType::TatamiFill), vec![]));
    assert_eq!(chosen(fill(), &[("fill_method", "contour_fill")]), (Some(StitchType::ContourFill), vec![]));
    // Ink/Stitch's legacy fill is sewn as a tatami fill.
    assert_eq!(chosen(fill(), &[("fill_method", "legacy_fill")]), (Some(StitchType::TatamiFill), vec![Code::FillLegacyAsTatami]));
    assert!(legacy(&element(fill(), &[("fill_method", "legacy_fill")]).params) && !legacy(&element(fill(), &[]).params));
    assert!(StitchType::ALL.iter().all(|t| sewn_as(*t) == *t || *t == StitchType::LegacyFill));
    assert_eq!(chosen(wide(), &[]), (Some(StitchType::RunningStitch), vec![]));
    assert_eq!(chosen(wide(), &[("stroke_method", "manual_stitch")]), (Some(StitchType::ManualStitch), vec![]));
    // `satin_column` on, the satin's method; off, the stroke's, whatever the other says.
    assert_eq!(chosen(wide(), &[("satin_column", "true"), ("stroke_method", "manual_stitch")]), (Some(StitchType::SatinColumn), vec![]));
    assert_eq!(chosen(wide(), &[("satin_column", "true"), ("satin_method", "e_stitch")]), (Some(StitchType::EStitch), vec![]));
    assert_eq!(chosen(wide(), &[("satin_method", "plaid")]), (Some(StitchType::RunningStitch), vec![]));
    // A satin column too narrow to stitch across is a stroke.
    assert_eq!(chosen(Shape::stroke(path.clone()), &[("satin_column", "true")]), (Some(StitchType::RunningStitch), vec![Code::SatinTooNarrow]));
    // A setting that chooses it and cannot be read is said.
    assert_eq!(chosen(wide(), &[("satin_column", "true"), ("satin_method", "plaid")]), (None, vec![Code::ParamInvalid]));
    assert_eq!(chosen(wide(), &[("satin_column", "maybe")]), (None, vec![Code::ParamInvalid]));
    // A method of another family, which only a bug in the method lists could offer.
    let mut diagnostics = Vec::new();
    assert_eq!(method_type(Family::Stroke, "tatami_fill", &mut diagnostics), None);
    let said: Vec<String> = diagnostics.iter().map(ToString::to_string).collect();
    assert_eq!(said, ["error SC-E0009: `stroke_method` offers `tatami_fill`, which is not a stitch type of strokes."]);
}

#[test]
fn a_circular_fill_s_preview_is_the_pieces_of_its_spiral() {
    // A circular fill has no rows either: a preview that can't afford its stitches shows the pieces of its
    // spiral, circles 1 mm apart round the middle of the 10 mm square, cut by its sides.
    let segments = [p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)].map(Segment::Line).to_vec();
    let square = Shape::Fill {
        path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] },
        rule: FillRule::NonZero,
        gradient: None,
    };
    let params = [("fill_method", "circular_fill"), ("row_spacing_mm", "1")];
    let laid = rows(&element(square, &params), &DesignSettings::default(), stitchcraft_plan::profiles::REFERENCE, &Budget::DEFAULT);
    assert_eq!(laid.diagnostics, Vec::new());
    let path = laid.segments.unwrap_or_default();
    let inside = |q: Point| (-1e-9..=10.0 + 1e-9).contains(&q.x()) && (-1e-9..=10.0 + 1e-9).contains(&q.y());
    assert!(path.iter().all(|s| inside(s.start) && inside(s.end)), "{path:?}");
    // About as long as the circles inside the square: those 1 to 5 mm round its middle whole, 94 mm, and
    // what its sides leave of those 6 and 7 mm round, 10 mm.
    let long: f64 = path.iter().map(|s| s.start.distance(s.end)).sum();
    assert!((95.0..110.0).contains(&long), "{long} mm");
}

#[test]
fn a_cross_stitch_fill_s_preview_is_its_crosses_diagonals() {
    // A cross stitch fill has no rows: its preview shows the 2 diagonals of each cross. A 4 mm square on
    // the canvas's 2 mm grid covers 4 cells whole, and each sews a cross.
    let segments = [p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)].map(Segment::Line).to_vec();
    let square = Shape::Fill {
        path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] },
        rule: FillRule::NonZero,
        gradient: None,
    };
    let params = [("fill_method", "cross_stitch"), ("pattern_size_mm", "2")];
    let laid = rows(&element(square, &params), &DesignSettings::default(), stitchcraft_plan::profiles::REFERENCE, &Budget::DEFAULT);
    assert_eq!(laid.diagnostics, Vec::new());
    let mut diagonals: Vec<[f64; 4]> = laid.segments.unwrap().iter().map(|s| [s.start.x(), s.start.y(), s.end.x(), s.end.y()]).collect();
    diagonals.sort_by(|a, b| a.iter().zip(b).map(|(x, y)| x.total_cmp(y)).find(|o| o.is_ne()).unwrap_or(std::cmp::Ordering::Equal));
    let cell = |x: f64, y: f64| [[x, y, x + 2.0, y + 2.0], [x, y + 2.0, x + 2.0, y]];
    let mut expected: Vec<[f64; 4]> = [cell(0.0, 0.0), cell(2.0, 0.0), cell(0.0, 2.0), cell(2.0, 2.0)].concat();
    expected.sort_by(|a, b| a.iter().zip(b).map(|(x, y)| x.total_cmp(y)).find(|o| o.is_ne()).unwrap_or(std::cmp::Ordering::Equal));
    assert_eq!(diagonals, expected);
}
