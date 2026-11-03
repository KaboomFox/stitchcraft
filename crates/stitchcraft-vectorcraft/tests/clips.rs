//! `REQ-VC-017`: what a clip group holds is cut to its clipping object, as VectorCraft clips it, each element
//! holding a clip of the object's area (`REQ-GEN-005`, which `crates/stitchcraft-engine/tests/clips.rs` sews).
//! The documents are written as VectorCraft saves them: Object › Clipping Mask › Make puts the clipping object
//! first in a group of `"clip": true`, takes its paints and effects away, and marks a path `"clipping": true`.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::collections::BTreeMap;

use common::{RED, document, element, ids, read_ok, said, square};
use stitchcraft_core::units::MM_PER_POINT;
use stitchcraft_core::{Budget, Code, Diagnostic, Fix, Point};
use stitchcraft_engine::design::{Element, FillRule, Path, Reading};
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Role, StitchKind};
use stitchcraft_vectorcraft::{VectorCraft, read, read_with};

/// How far a needle point may lie past a clip's edge, in points: the cut is exact, and a needle point on the
/// edge is placed to the machine's tenth of a millimetre.
const EDGE: f64 = 0.06 / MM_PER_POINT;

/// The appearance Make Clipping Mask leaves a clipping object: a fill and a stroke of paint none.
const UNPAINTED: &str = r#"{"items": [{"kind": "fill", "paint": {"type": "none"}}, {"kind": "stroke", "paint": {"type": "none"}, "width": 0.0}]}"#;

/// A red fill and a blue stroke 1 pt wide, as appearance items.
const RED_FILL: &str = r#"{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}}}"#;
const BLUE_STROKE: &str = r#"{"kind": "stroke", "width": 1, "paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 1}}}"#;

/// The subpath of `points`, in points, closed or not, as VectorCraft writes it.
fn subpath(points: &[(f64, f64)], closed: bool) -> String {
    let anchors: Vec<String> = points.iter().map(|(x, y)| format!(r#"{{"p": [{x}, {y}]}}"#)).collect();
    format!(r#"{{"closed": {closed}, "anchors": [{}]}}"#, anchors.join(","))
}

/// The rectangle from (`x0`, `y0`) to (`x1`, `y1`), as a closed subpath.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> String {
    subpath(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true)
}

/// A path with `id`, `appearance`, `extra` fields in its kind (`"rule": "EvenOdd",`) and `subpaths`.
fn path(id: u32, appearance: &str, extra: &str, subpaths: &[String]) -> String {
    format!(r#"{{"id": {id}, "appearance": {appearance}, "kind": {{"type": "path", {extra} "path": {{"subpaths": [{}]}}}}}}"#, subpaths.join(","))
}

/// `object`, hidden.
fn hidden(object: &str) -> String {
    object.replacen(r#"{"id""#, r#"{"visible": false, "id""#, 1)
}

/// A clipping object as Make Clipping Mask leaves it: the rectangle `[x0, y0, x1, y1]`, with `id`, unpainted.
fn clipping(id: u32, [x0, y0, x1, y1]: [f64; 4]) -> String {
    path(id, UNPAINTED, r#""clipping": true,"#, &[rect(x0, y0, x1, y1)])
}

/// A group of `kind` (`group` or `layer`) with `id`, `appearance` and `extra` fields in its kind (`"clip":
/// true,`), holding `children`.
fn container(kind: &str, id: u32, appearance: &str, extra: &str, children: &[String]) -> String {
    format!(r#"{{"id": {id}, "appearance": {appearance}, "kind": {{"type": "{kind}", {extra} "children": [{}]}}}}"#, children.join(","))
}

/// A clip group with `id` and `appearance`, holding `children`, the first its clipping object.
fn clip_group(id: u32, appearance: &str, children: &[String]) -> String {
    container("group", id, appearance, r#""clip": true,"#, children)
}

/// An object with `id` of the kind `kind`, with nothing else in its kind.
fn bare(id: u32, kind: &str) -> String {
    format!(r#"{{"id": {id}, "kind": {{"type": "{kind}"}}}}"#)
}

/// An appearance of `items` and StitchCraft's effect of `family`, without parameters.
fn with_effect(items: &str, family: &str) -> String {
    format!(r#"{{"items": [{items}], "effects": [{{"id": "plugin.dev.stitchcraft.{family}", "params": {{}}}}]}}"#)
}

/// The box round the points of `path`, its segments' ends and control points, in points to a hundredth:
/// [x0, y0, x1, y1]. The clips here are drawn with lines, whose box that is.
fn bounds(path: &Path) -> [f64; 4] {
    let points: Vec<Point> = path.points().collect();
    let round = |v: f64| (v / MM_PER_POINT * 100.0).round() / 100.0 + 0.0;
    let (xs, ys) = (points.iter().map(|p| p.x()), points.iter().map(|p| p.y()));
    [xs.clone().fold(f64::MAX, f64::min), ys.clone().fold(f64::MAX, f64::min), xs.fold(f64::MIN, f64::max), ys.fold(f64::MIN, f64::max)].map(round)
}

/// The box round each shape of each clip of `element`, the nearest clip first.
fn boxes(element: &Element) -> Vec<Vec<[f64; 4]>> {
    element.clips.iter().map(|clip| clip.shapes.iter().map(|shape| bounds(&shape.path)).collect()).collect()
}

/// The rule of each shape of each clip of `element`.
fn rules(element: &Element) -> Vec<Vec<FillRule>> {
    element.clips.iter().map(|clip| clip.shapes.iter().map(|shape| shape.rule).collect()).collect()
}

/// The warnings of `read` said of the object `label`, each as its code and message.
fn of<'r>(read: &'r VectorCraft, label: &str) -> Vec<(Code, &'r str)> {
    let about = |w: &&Diagnostic| w.element.as_ref().is_some_and(|e| e.as_str() == format!("vc:{label}"));
    read.warnings.iter().filter(about).map(|w| (w.code, w.message.as_str())).collect()
}

/// The needle points of each element, by its id, in points.
type Points = BTreeMap<String, Vec<(f64, f64)>>;

/// The needle points that each element of `read` sews, locks left out, by its id, in points, planned for the
/// reference machine with the design's origin at the document's, and what planning says.
fn sewn(read: &VectorCraft) -> (Points, Vec<String>) {
    let mut design = read.design.clone();
    design.settings.origin = Some(Point::new(0.0, 0.0).unwrap());
    let outcome = stitchcraft_engine::plan(&design, REFERENCE, &Budget::DEFAULT);
    let plan = outcome.plan.unwrap();
    let mut points = Points::new();
    for stitch in plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role != Role::Lock) {
        let id = stitch.origin.element.and_then(|e| plan.elements.get(e.index())).unwrap();
        points.entry(id.to_string()).or_default().push((stitch.at.x() / MM_PER_POINT, stitch.at.y() / MM_PER_POINT));
    }
    (points, outcome.diagnostics.iter().map(ToString::to_string).collect())
}

/// The box round `points`: [x0, y0, x1, y1].
fn around(points: &[(f64, f64)]) -> [f64; 4] {
    let (xs, ys) = (points.iter().map(|p| p.0), points.iter().map(|p| p.1));
    [xs.clone().fold(f64::MAX, f64::min), ys.clone().fold(f64::MAX, f64::min), xs.fold(f64::MIN, f64::max), ys.fold(f64::MIN, f64::max)]
}

/// Whether the box `got` is the box `want` within `slack` points on each side.
fn about(got: [f64; 4], want: [f64; 4], slack: f64) -> bool {
    got.iter().zip(want).all(|(g, w)| (g - w).abs() <= slack)
}

#[test]
fn req_vc_017_what_a_clip_group_holds_sews_cut_to_its_clipping_object() {
    // A square 1 in a side filled red, a line across it and a satin column, clipped to a square half its size
    // in its middle.
    let fill = path(4, &with_effect(RED_FILL, "fill"), "", &[rect(0.0, 0.0, 72.0, 72.0)]);
    let stroke = path(5, &with_effect(BLUE_STROKE, "stroke"), "", &[subpath(&[(-36.0, 36.0), (108.0, 36.0)], false)]);
    let wide = BLUE_STROKE.replace(r#""width": 1"#, r#""width": 8.503937"#);
    let satin = path(6, &with_effect(&wide, "satin"), "", &[subpath(&[(0.0, 45.0), (72.0, 45.0)], false)]);
    let read = read_ok(&document(4, &clip_group(2, "{}", &[clipping(3, [18.0, 18.0, 54.0, 54.0]), fill, stroke, satin])));
    // The clipping object, unpainted, sews nothing, and nothing is said: the clip is applied.
    assert_eq!(ids(&read), ["vc:4:fill", "vc:5:stroke", "vc:6:satin"]);
    assert_eq!(said(&read), []);
    for element in read.design.elements() {
        assert_eq!((boxes(element), rules(element)), (vec![vec![[18.0, 18.0, 54.0, 54.0]]], vec![vec![FillRule::NonZero]]), "{}", element.id);
    }
    // The fill sews its rows across the clip, edge to edge, and the line the piece inside it. The satin column
    // is sewn whole, which is said.
    let (points, said) = sewn(&read);
    assert!(about(around(&points["vc:4:fill"]), [18.0, 18.0, 54.0, 54.0], EDGE), "{:?}", around(&points["vc:4:fill"]));
    assert!(about(around(&points["vc:5:stroke"]), [18.0, 36.0, 54.0, 36.0], EDGE), "{:?}", around(&points["vc:5:stroke"]));
    let satin = around(&points["vc:6:satin"]);
    assert!(satin[0] < 1.0 && satin[2] > 71.0, "{satin:?}");
    assert!(matches!(said.as_slice(), [one] if one.contains("SC-W0217")), "{said:?}");
}

#[test]
fn req_vc_017_a_shape_digitized_automatically_is_cut_before_it_is_digitized() {
    // Read with --auto, the red square is an automatic element, cut to the clip before it is digitized: it is
    // filled where the clip leaves it.
    let bytes = document(4, &clip_group(2, "{}", &[clipping(3, [18.0, 18.0, 54.0, 54.0]), square(4, 0.0, 0.0, RED)]));
    let read = read_with(&bytes, &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap();
    let square = element(&read, "vc:4:fill");
    assert!(square.automatic);
    assert_eq!(boxes(square), [[[18.0, 18.0, 54.0, 54.0]]]);
    let (points, _) = sewn(&read);
    assert!(about(around(&points["vc:4:fill"]), [18.0, 18.0, 36.0, 36.0], 1.0), "{:?}", around(&points["vc:4:fill"]));
}

#[test]
fn req_vc_017_the_clipping_object_s_area_is_the_one_vectorcraft_clips_to() {
    // Each clip group holds its clipping object first, then a red square, which takes the clip.
    let clipped = |id: u32, clip: String, square_id: u32| clip_group(id, "{}", &[clip, square(square_id, 200.0, 200.0, RED)]);
    let objects = [
        // A path by its fill rule, painted or not, and hidden too.
        clipped(2, path(3, UNPAINTED, r#""rule": "EvenOdd", "clipping": true,"#, &[rect(0.0, 0.0, 10.0, 10.0)]), 4),
        clipped(5, path(6, &with_effect(BLUE_STROKE, "stroke"), "", &[rect(0.0, 0.0, 20.0, 20.0)]), 7),
        clipped(8, hidden(&clipping(9, [0.0, 0.0, 30.0, 30.0])), 10),
        // A compound path's paths together, by its own rule.
        clipped(
            11,
            format!(
                r#"{{"id": 12, "kind": {{"type": "compound", "rule": "EvenOdd", "children": [{}, {}]}}}}"#,
                path(13, "{}", "", &[rect(0.0, 0.0, 40.0, 40.0)]),
                path(14, "{}", "", &[rect(10.0, 10.0, 30.0, 30.0)])
            ),
            15,
        ),
        // A group's members that show, each by its rule. A guide and a symbol add nothing.
        clipped(
            16,
            container(
                "group",
                17,
                "{}",
                "",
                &[
                    path(18, "{}", r#""rule": "EvenOdd","#, &[rect(0.0, 0.0, 5.0, 5.0)]),
                    path(19, "{}", "", &[rect(50.0, 50.0, 60.0, 60.0)]),
                    hidden(&path(20, "{}", "", &[rect(0.0, 0.0, 99.0, 99.0)])),
                    path(21, "{}", r#""guide": true,"#, &[rect(0.0, 0.0, 98.0, 98.0)]),
                    bare(22, "symbolinstance"),
                ],
            ),
            23,
        ),
        // A Shaper group's shapes, its sources left out, and the members of a layer that show, though it clips.
        clipped(
            24,
            container(
                "group",
                25,
                "{}",
                "",
                &[
                    container("group", 26, "{}", "", &[path(27, "{}", "", &[rect(0.0, 0.0, 97.0, 97.0)])]),
                    path(28, "{}", "", &[rect(1.0, 1.0, 2.0, 2.0)]),
                ],
            )
            .replacen(r#""id": 25,"#, r#""id": 25, "shaper": {"edges": []},"#, 1),
            29,
        ),
        clipped(
            30,
            container(
                "layer",
                31,
                "{}",
                r#""clip": true,"#,
                &[path(32, "{}", "", &[rect(3.0, 3.0, 4.0, 4.0)]), path(33, "{}", "", &[rect(5.0, 5.0, 6.0, 6.0)])],
            ),
            34,
        ),
        // A clip group's own clipping object, and not what it clips.
        clipped(35, clip_group(36, "{}", &[clipping(37, [7.0, 7.0, 8.0, 8.0]), square(38, 0.0, 0.0, RED)]), 39),
    ];
    let read = read_ok(&document(4, &objects.join(",")));
    let clip = |id: &str| (boxes(element(&read, id)), rules(element(&read, id)));
    let (nonzero, evenodd) = (FillRule::NonZero, FillRule::EvenOdd);
    assert_eq!(clip("vc:4:fill"), (vec![vec![[0.0, 0.0, 10.0, 10.0]]], vec![vec![evenodd]]));
    assert_eq!(clip("vc:7:fill"), (vec![vec![[0.0, 0.0, 20.0, 20.0]]], vec![vec![nonzero]]));
    assert_eq!(clip("vc:10:fill"), (vec![vec![[0.0, 0.0, 30.0, 30.0]]], vec![vec![nonzero]]));
    assert_eq!(clip("vc:15:fill"), (vec![vec![[0.0, 0.0, 40.0, 40.0]]], vec![vec![evenodd]]));
    assert_eq!(element(&read, "vc:15:fill").clips[0].shapes[0].path.subpaths.len(), 2);
    assert_eq!(clip("vc:23:fill"), (vec![vec![[0.0, 0.0, 5.0, 5.0], [50.0, 50.0, 60.0, 60.0]]], vec![vec![evenodd, nonzero]]));
    assert_eq!(clip("vc:29:fill"), (vec![vec![[1.0, 1.0, 2.0, 2.0]]], vec![vec![nonzero]]));
    assert_eq!(clip("vc:34:fill"), (vec![vec![[3.0, 3.0, 4.0, 4.0], [5.0, 5.0, 6.0, 6.0]]], vec![vec![nonzero, nonzero]]));
    assert_eq!(clip("vc:39:fill"), (vec![vec![[7.0, 7.0, 8.0, 8.0]]], vec![vec![nonzero]]));
    // The painted clipping object sews, and its own clip doesn't cut it. VectorCraft draws nothing of a group
    // there, nor of what it holds, and nothing is said.
    assert_eq!(
        ids(&read),
        ["vc:4:fill", "vc:6:stroke", "vc:7:fill", "vc:10:fill", "vc:15:fill", "vc:23:fill", "vc:29:fill", "vc:34:fill", "vc:39:fill"]
    );
    assert!(element(&read, "vc:6:stroke").clips.is_empty());
    assert_eq!(said(&read), []);
}

#[test]
fn req_vc_017_each_clip_group_around_an_element_cuts_it_the_nearest_first() {
    // Clip group 2 clips to the square from 0 to 50, and holds clip group 4, which clips to the square from 20
    // to 70. The red square in the inner group is cut to both. The inner clipping object, painted, is cut to
    // the outer clip alone, and the outer clipping object to neither.
    let blue = r#"{"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 1}}}]}"#;
    let inner = clip_group(4, "{}", &[path(5, blue, r#""clipping": true,"#, &[rect(20.0, 20.0, 70.0, 70.0)]), square(6, 30.0, 30.0, RED)]);
    let outer = clip_group(2, "{}", &[path(3, blue, r#""clipping": true,"#, &[rect(0.0, 0.0, 50.0, 50.0)]), inner, square(7, 0.0, 0.0, RED)]);
    let read = read_ok(&document(4, &[outer, square(8, 100.0, 0.0, RED)].join(",")));
    assert_eq!(ids(&read), ["vc:3:fill", "vc:5:fill", "vc:6:fill", "vc:7:fill", "vc:8:fill"]);
    let (a, b, none) = ([0.0, 0.0, 50.0, 50.0], [20.0, 20.0, 70.0, 70.0], Vec::<Vec<[f64; 4]>>::new());
    assert_eq!(boxes(element(&read, "vc:3:fill")), none);
    assert_eq!(boxes(element(&read, "vc:5:fill")), [[a]]);
    assert_eq!(boxes(element(&read, "vc:6:fill")), [[b], [a]]);
    assert_eq!(boxes(element(&read, "vc:7:fill")), [[a]]);
    assert_eq!(boxes(element(&read, "vc:8:fill")), none);
    // The square in both is sewn where they meet.
    let (points, _) = sewn(&read);
    assert!(about(around(&points["vc:6:fill"]), [30.0, 30.0, 50.0, 50.0], EDGE), "{:?}", around(&points["vc:6:fill"]));
}

#[test]
fn diag_sc_w0807_stitchcraft_s_effects_on_a_clipping_object_are_named() {
    // VectorCraft runs a group's StitchCraft effects on its clipping object too, and clips to the stitches
    // they draw. What sews is cut to the object as drawn, which is said, for an effect on the clip group or
    // on a group around it, and for the Auto effect.
    let clipped = |id: u32, appearance: &str| clip_group(id, appearance, &[clipping(id + 1, [0.0, 0.0, 20.0, 20.0]), square(id + 2, 0.0, 0.0, RED)]);
    let fill = r#"{"effects": [{"id": "plugin.dev.stitchcraft.fill", "params": {}}]}"#;
    let auto = r#"{"effects": [{"id": "plugin.dev.stitchcraft.auto", "params": {}}]}"#;
    let objects = [
        clipped(2, fill),
        container("group", 5, fill, "", &[clipped(6, "{}")]),
        clipped(9, auto),
        // Without effects, or with a clip of nothing for them to reach, nothing is said of them.
        clipped(12, "{}"),
        clip_group(15, fill, &[bare(16, "symbolinstance"), square(17, 0.0, 0.0, RED)]),
    ];
    let read = read_ok(&document(4, &objects.join(",")));
    let w = Code::VcFeatureIgnored;
    assert_eq!(said(&read), [(w, "vc:2".to_string()), (w, "vc:6".to_string()), (w, "vc:9".to_string())]);
    assert_eq!(
        read.warnings[0].to_string(),
        "warning SC-W0807: A group clips what it holds to an object that StitchCraft's effects reach too. VectorCraft clips to the \
         stitches they draw of it, and what sews is cut to the object as it is drawn, which the preview doesn't show."
    );
    let hint = "Apply StitchCraft's effects to the objects it clips, not to it or to a group around it.";
    assert!(read.warnings.iter().all(|w| w.fix == Some(Fix::Hint(hint.to_string()))));
    // Each square is sewn by the effect, cut to its clip.
    assert_eq!(ids(&read), ["vc:4:fill", "vc:8:fill", "vc:11:fill", "vc:14:fill", "vc:17:fill"]);
    assert!(read.design.elements().iter().all(|e| e.clips.len() == 1), "{:?}", read.design.elements());
}

#[test]
fn diag_sc_w0807_what_a_clip_can_t_read_adds_nothing_to_it_and_is_named() {
    // Text, whose glyphs StitchCraft doesn't read, and what VectorCraft computes as it draws it: each adds
    // nothing to the clip, which then leaves nothing of the red square (SC-W0013).
    let kinds = [
        ("text", "text", Some("Type › Create Outlines")),
        ("image", "an image", Some("Draw a path round it")),
        ("placeddocument", "a placed document", Some("Draw a path round it")),
        ("mesh", "a gradient mesh", Some("Draw a path round it")),
        ("blend", "a blend", Some("Object › Expand.")),
        ("envelope", "an envelope", Some("Object › Expand.")),
        ("repeat", "a repeat", Some("Object › Expand.")),
        ("intertwine", "an Intertwine object", Some("Object › Intertwine › Release")),
        ("hologram", "an object of the kind \"hologram\"", None),
    ];
    let groups: Vec<u32> = (0..).map(|k: u32| 2 + 3 * k).take(kinds.len()).collect();
    let objects: Vec<String> =
        kinds.iter().zip(&groups).map(|((kind, ..), &id)| clip_group(id, "{}", &[bare(id + 1, kind), square(id + 2, 0.0, 0.0, RED)])).collect();
    let read = read_ok(&document(4, &objects.join(",")));
    for ((_, what, hint), id) in kinds.iter().zip(&groups) {
        let message = format!("A group clips what it holds to {what}, which StitchCraft doesn't read, and it adds nothing to the clip.");
        assert_eq!(of(&read, &id.to_string()), [(Code::VcFeatureIgnored, message.as_str())]);
        let warning = read.warnings.iter().find(|w| w.message == message).unwrap();
        match (hint, &warning.fix) {
            (Some(hint), Some(Fix::Hint(given))) => assert!(given.contains(hint), "{given}"),
            (None, None) => {}
            _ => panic!("{warning:?}"),
        }
    }
    // The text is named as an object too, as text anywhere is. Each square's clip has no shape, and the square
    // sews nothing.
    assert_eq!(of(&read, "3"), [(Code::VcFeatureIgnored, "Text is not stitched.")]);
    assert_eq!(read.warnings.len(), kinds.len() + 1);
    assert!(read.design.elements().iter().all(|e| e.clips.len() == 1 && e.clips[0].shapes.is_empty()));
    let outcome = stitchcraft_engine::plan(&read.design, REFERENCE, &Budget::DEFAULT);
    assert_eq!(outcome.diagnostics.iter().filter(|d| d.code == Code::ClippedAway).count(), kinds.len());
    // Text in a group there, and an object without a kind, read as the kind "".
    let text = clip_group(2, "{}", &[container("group", 3, "{}", "", &[bare(4, "text")]), square(5, 0.0, 0.0, RED)]);
    let kindless = clip_group(6, "{}", &[r#"{"id": 7}"#.to_string(), square(8, 0.0, 0.0, RED)]);
    let read = read_ok(&document(4, &[text, kindless].join(",")));
    let said: Vec<&str> = read.warnings.iter().map(|w| w.message.as_str()).collect();
    assert_eq!(
        said,
        [
            "A group clips what it holds to text, which StitchCraft doesn't read, and it adds nothing to the clip.",
            "A group clips what it holds to an object of the kind \"\", which StitchCraft doesn't read, and it adds nothing to the clip."
        ]
    );
}

#[test]
fn diag_sc_w0808_a_clipping_path_that_can_t_be_used_adds_nothing_to_the_clip() {
    // A coordinate that isn't a number, a path more than 10 m away, and one that draws nothing, as a path and in
    // a compound path. The clipping object is named as an object that can't be used too, as any is.
    let null = r#"{"closed": true, "anchors": [{"p": [0, 0]}, {"p": [null, 10]}, {"p": [10, 10]}]}"#.to_string();
    let far = rect(0.0, 0.0, 40_000.0, 10.0);
    let point = subpath(&[(5.0, 5.0)], false);
    let compound = format!(r#"{{"id": 15, "kind": {{"type": "compound", "children": [{}]}}}}"#, path(16, "{}", "", std::slice::from_ref(&null)));
    let clips = [path(3, UNPAINTED, "", &[null]), path(7, UNPAINTED, "", &[far]), path(11, UNPAINTED, "", &[point]), compound];
    let objects: Vec<String> =
        clips.into_iter().zip([2, 6, 10, 14]).map(|(clip, id)| clip_group(id, "{}", &[clip, square(id + 3, 0.0, 0.0, RED)])).collect();
    let read = read_ok(&document(4, &objects.join(",")));
    let whys = ["a coordinate of its path is not a number", "it lies more than 10 m from the document's origin", "it draws nothing"];
    for (id, why) in [(2, whys[0]), (6, whys[1]), (10, whys[2]), (14, whys[0])] {
        let message = format!("A group clips what it holds to a path that can't be used, which adds nothing to the clip: {why}.");
        assert_eq!(of(&read, &id.to_string()), [(Code::VcGeometryUnusable, message.as_str())]);
        assert_eq!(of(&read, &(id + 1).to_string()).len(), 1);
    }
    let hint = "Check its anchors, or remove stray objects far from the design.";
    assert!(read.warnings.iter().all(|w| w.fix == Some(Fix::Hint(hint.to_string()))));
    assert!(read.design.elements().iter().all(|e| e.clips.len() == 1 && e.clips[0].shapes.is_empty()));
}

#[test]
fn diag_sc_w0807_a_clipping_path_s_fill_rule_that_doesn_t_read_is_named() {
    // VectorCraft writes NonZero and EvenOdd. Another rule is said of the clip group, and of the object, and
    // nonzero is used.
    let clip = path(3, UNPAINTED, r#""rule": "Spiral","#, &[rect(0.0, 0.0, 9.0, 9.0)]);
    let read = read_ok(&document(4, &clip_group(2, "{}", &[clip, square(4, 0.0, 0.0, RED)])));
    let message = "The fill rule \"Spiral\" is not one StitchCraft reads, and nonzero is used.";
    assert_eq!((of(&read, "2"), of(&read, "3")), (vec![(Code::VcFeatureIgnored, message)], vec![(Code::VcFeatureIgnored, message)]));
    assert_eq!(rules(element(&read, "vc:4:fill")), [[FillRule::NonZero]]);
}

/// The least work that reads `bytes`, which any less doesn't, for want of it.
fn least_work(bytes: &[u8]) -> u64 {
    let reads = |work: u64| read(bytes, &Budget { max_stitches: 1, max_work: work }).map_err(|refused| refused.code);
    let (mut low, mut high) = (0, 1_000_000);
    while low < high {
        let mid = low + (high - low) / 2;
        if reads(mid).is_ok() { high = mid } else { low = mid + 1 }
    }
    assert!((0..low).all(|work| reads(work) == Err(Code::BudgetExhausted)));
    low
}

#[test]
fn req_vc_017_a_clip_costs_what_reading_it_does_and_each_element_its_segments_more() {
    // The same group, clipping and not: the clip costs a unit for the clipping object and 5 for its subpath
    // of 4 anchors, and each of the 2 fills a unit for the clip and 4 for its segments, 16 in all.
    let group =
        |clip: &str| container("group", 2, "{}", clip, &[clipping(3, [0.0, 0.0, 9.0, 9.0]), square(4, 0.0, 0.0, RED), square(5, 50.0, 0.0, RED)]);
    let (plain, clipped) = (document(4, &group("")), document(4, &group(r#""clip": true,"#)));
    assert_eq!(least_work(&clipped) - least_work(&plain), 16);
    // Clipped again round that, each fill holds 2 clips: 1 for the outer clipping object, 5 for its subpath and
    // 5 more for each fill.
    let twice = |clip: &str| document(4, &container("group", 6, "{}", clip, &[clipping(7, [0.0, 0.0, 9.0, 9.0]), group(r#""clip": true,"#)]));
    assert_eq!(least_work(&twice(r#""clip": true,"#)) - least_work(&twice("")), 16);
}
