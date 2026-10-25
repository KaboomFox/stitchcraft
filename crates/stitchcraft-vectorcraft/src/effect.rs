//! What a live effect is given and gives back (VectorCraft's plug-in ABI v1): the object it is given, as
//! the element its stitch family sews, and needle points, as geometry VectorCraft draws.
//!
//! **What it is given.** One object's geometry, `{"objects": [{"type", "path", "fillRule", "bounds"}]}`,
//! where the path is the `PathData` a document holds ([`crate::geometry`]). Beside it come the effect's
//! parameters as VectorCraft resolved them from the plug-in's manifest, and VectorCraft's own `_context`,
//! which is left out. The parameters are read as an effect record's are, and the family says whether a
//! stroke is a satin column, as it does for a record. The object comes without its paints, so a stroke
//! effect's element has the stand-in stroke of [`Shape::stroke`]: a running stitch does not use its width,
//! and a satin column drawn with rails does not need it. The plug-in leaves a satin column drawn as one path
//! and a zigzag stroke as they are, since both are as wide as their stroke (`SC-I0810`).
//!
//! **What it gives back.** Geometry only, drawn with the object's own paints. A stroke's or a satin
//! column's stitches come back as open paths from needle point to needle point, for the object's stroke to
//! draw. A fill's come back as one thin closed ribbon for each stitch, for its fill to fill, since a path
//! of stitches would be filled whole. When the object cannot be sewn, or its stitches draw nothing, the
//! effect gives back why, as `{"error": …}`: VectorCraft then draws the object as it is and shows the
//! message as the plug-in's last error.

use serde::Serialize;
use serde_json::{Map, Value};
use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Fix, Point};
use stitchcraft_engine::design::{Element, FillRule, Shape};
use stitchcraft_engine::generators::satin::SATIN_COLUMN;
use stitchcraft_params::ParamSet;
use stitchcraft_params::stitch_type::Family;
use stitchcraft_plan::{Rgb, Thread};

use crate::PREVIEW_LEVEL;
use crate::geometry::{self, UNUSABLE_HINT};
use crate::values::{sews, text};
use crate::wire::{EffectInput, EffectObject};
use crate::writer::Geometry;

/// How wide a fill's ribbons are, in millimetres: half a thread, so that rows a quarter of a millimetre
/// apart still show as rows.
pub const RIBBON_MM: f64 = 0.2;

/// What a live effect is given, read: the element its family sews, and what its preview should show.
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    /// The object's path as the family's shape, with the effect's settings.
    pub element: Element,
    /// The preview level the parameters ask for ([`PREVIEW_LEVEL`]), as its text; `None` without one.
    pub preview_level: Option<String>,
}

/// The live effect of `family` that `input` and `params` give, as VectorCraft passes them to `vc_run`,
/// read within `budget`. `SC-E0806` when they do not read as the ABI says, `SC-W0808` when the object's
/// path cannot be used, and `SC-E0004` when reading it needs more work than the budget allows.
pub fn read_effect(family: Family, input: &[u8], params: &[u8], budget: &Budget) -> Result<Effect, Diagnostic> {
    let input: EffectInput = serde_json::from_slice(input).map_err(|e| refused(&format!("Its objects do not read: {e}.")))?;
    let params: Map<String, Value> = serde_json::from_slice(params).map_err(|e| refused(&format!("Its parameters do not read: {e}.")))?;
    let Some(EffectObject { path: Some(data), fill_rule }) = input.objects.into_iter().next() else {
        return Err(refused("It holds no path."));
    };
    let path = match geometry::path(&[&data], &mut budget.meter()) {
        Ok(Ok(path)) => path,
        Ok(Err(why)) => {
            let message = format!("{}, so the live effect leaves the object as it is.", why.why());
            return Err(Diagnostic::new(Code::VcGeometryUnusable, message).with_fix(Fix::Hint(UNUSABLE_HINT.to_string())));
        }
        Err(_) => return Err(Diagnostic::new(Code::BudgetExhausted, "Reading the live effect's object needed more work than the budget allows.")),
    };
    let mut set = ParamSet::new();
    for (key, value) in params.iter().filter(|(key, _)| sews(key)) {
        if let Some(text) = text(value) {
            set.set(key.clone(), text);
        }
    }
    let preview_level = params.get(PREVIEW_LEVEL).and_then(text);
    let shape = match family {
        Family::Fill => Shape::Fill { path, rule: if fill_rule.as_deref() == Some("evenodd") { FillRule::EvenOdd } else { FillRule::NonZero } },
        Family::Stroke | Family::Satin => {
            set.set(SATIN_COLUMN, if family == Family::Satin { "true" } else { "false" });
            Shape::stroke(path)
        }
    };
    let id = ElementId::new(format!("vc:effect:{}", family.id()))
        .map_err(|e| Diagnostic::new(Code::InternalCheckFailed, format!("A live effect's element has no id: {e}.")))?;
    Ok(Effect { element: Element { id, name: None, shape, thread: Thread::new(Rgb::new(0, 0, 0)), params: set, automatic: false }, preview_level })
}

/// What a live effect of `family` returns for an element sewn with the needle points `groups`, each a
/// run of stitches, in millimetres: a fill's stitches as ribbons ([`effect_ribbons`]), and a stroke's or a
/// satin column's as lines ([`effect_lines`]). `None` when they draw nothing: no group has a stitch of
/// any length.
pub fn effect_output(family: Family, groups: &[Vec<Point>]) -> Option<Vec<u8>> {
    match family {
        Family::Fill => effect_ribbons(groups.iter().flat_map(|group| group.windows(2)).filter_map(|w| Some((*w.first()?, *w.get(1)?)))),
        Family::Stroke | Family::Satin => effect_lines(groups.iter().map(Vec::as_slice)),
    }
}

/// What a live effect returns to draw `segments`, in millimetres, with the object's fill: each one a
/// closed ribbon [`RIBBON_MM`] wide, since a fill fills a path whole, and a line has no area. `None` when
/// none has a length.
pub fn effect_ribbons(segments: impl IntoIterator<Item = (Point, Point)>) -> Option<Vec<u8>> {
    let mut geometry = Geometry::new();
    for corners in segments.into_iter().filter_map(|(a, b)| ribbon(&a, &b)) {
        geometry.subpath(&corners, true);
    }
    geometry.finish()
}

/// What a live effect returns to draw `lines`, polylines in millimetres, with the object's stroke: open
/// paths through their points. `None` when none has 2 points.
pub fn effect_lines<'a>(lines: impl IntoIterator<Item = &'a [Point]>) -> Option<Vec<u8>> {
    let mut geometry = Geometry::new();
    for line in lines {
        geometry.subpath(line, false);
    }
    geometry.finish()
}

/// What a live effect returns instead of stitches: VectorCraft shows `diagnostic`, with its hint, as the
/// plug-in's last error, and draws the object as it is.
pub fn effect_error(diagnostic: &Diagnostic) -> Vec<u8> {
    let error = match &diagnostic.fix {
        Some(fix) => format!("{diagnostic} {}", fix.describe()),
        None => diagnostic.to_string(),
    };
    serde_json::to_vec(&ErrorOut { error }).unwrap_or_default()
}

/// `SC-E0806` for what VectorCraft passed a live effect that does not read as its plug-in ABI says, which
/// `what` explains.
fn refused(what: &str) -> Diagnostic {
    Diagnostic::new(Code::VcUnreadable, format!("What VectorCraft passed the live effect does not read as its plug-in ABI v1 says. {what}"))
}

/// The corners of the stitch from `a` to `b` as a closed ribbon [`RIBBON_MM`] wide; `None` for a stitch
/// of no length.
fn ribbon(a: &Point, b: &Point) -> Option<[Point; 4]> {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let length = (dx * dx + dy * dy).sqrt();
    if length <= 0.0 {
        return None;
    }
    let (nx, ny) = (-dy / length * RIBBON_MM / 2.0, dx / length * RIBBON_MM / 2.0);
    let corner = |p: &Point, side: f64| Point::new(p.x() + side * nx, p.y() + side * ny).ok();
    Some([corner(a, 1.0)?, corner(b, 1.0)?, corner(b, -1.0)?, corner(a, -1.0)?])
}

#[derive(Serialize)]
struct ErrorOut {
    error: String,
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use stitchcraft_core::units::MM_PER_POINT;

    use super::*;

    fn input(path: &Value, rule: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({"objects": [{"type": "path", "path": path, "fillRule": rule, "bounds": [0, 0, 72, 72]}]})).unwrap()
    }

    fn square() -> Value {
        json!({"subpaths": [{"closed": true, "anchors": [{"p": [0, 0]}, {"p": [72, 0]}, {"p": [72, 72]}, {"p": [0, 72]}]}]})
    }

    #[test]
    fn an_effect_s_object_is_its_family_s_element_with_its_parameters() {
        let params =
            br#"{"row_spacing_mm": 0.3, "underpath": false, "fill_method": "tatami_fill", "preview_level": "rows", "_context": {"mode": "effect"}}"#;
        let fill = read_effect(Family::Fill, &input(&square(), "evenodd"), params, &Budget::DEFAULT).unwrap();
        assert!(matches!(&fill.element.shape, Shape::Fill { rule: FillRule::EvenOdd, path } if path.subpaths.len() == 1));
        let read: Vec<(&str, &str)> = fill.element.params.iter().collect();
        assert_eq!(read, [("fill_method", "tatami_fill"), ("row_spacing_mm", "0.3"), ("underpath", "false")]);
        // The preview level is the plug-in's, beside the element.
        assert_eq!(fill.preview_level.as_deref(), Some("rows"));
        let satin = read_effect(Family::Satin, &input(&square(), "nonzero"), b"{}", &Budget::DEFAULT).unwrap();
        assert!(matches!(satin.element.shape, Shape::Stroke { .. }));
        assert_eq!((satin.element.params.get(SATIN_COLUMN), satin.preview_level), (Some("true"), None));
        let stroke = read_effect(Family::Stroke, &input(&square(), "nonzero"), b"{}", &Budget::DEFAULT).unwrap().element;
        assert_eq!((stroke.params.get(SATIN_COLUMN), stroke.id.as_str()), (Some("false"), "vc:effect:stroke"));
    }

    /// The error a live effect gives back for `diagnostic`, as VectorCraft shows it.
    fn shown(diagnostic: &Diagnostic) -> String {
        let error: Value = serde_json::from_slice(&effect_error(diagnostic)).unwrap();
        error["error"].as_str().unwrap().to_string()
    }

    #[test]
    fn diag_sc_e0806_what_a_live_effect_cannot_read_is_its_error() {
        let refused = |input: &[u8], params: &[u8]| shown(&read_effect(Family::Fill, input, params, &Budget::DEFAULT).unwrap_err());
        let start = "error SC-E0806: What VectorCraft passed the live effect does not read as its plug-in ABI v1 says.";
        assert_eq!(
            refused(br#"{"objects": 3}"#, b"{}"),
            format!("{start} Its objects do not read: invalid type: integer `3`, expected a sequence at line 1 column 13.")
        );
        assert_eq!(
            refused(&input(&square(), "nonzero"), b"[]"),
            format!("{start} Its parameters do not read: invalid type: sequence, expected a map at line 1 column 0.")
        );
        assert_eq!(refused(br#"{"objects": []}"#, b"{}"), format!("{start} It holds no path."));
        let poor = Budget { max_stitches: 1, max_work: 1 };
        let exhausted = read_effect(Family::Fill, &input(&square(), "nonzero"), b"{}", &poor).unwrap_err();
        assert_eq!(shown(&exhausted), "error SC-E0004: Reading the live effect's object needed more work than the budget allows.");
    }

    #[test]
    fn diag_sc_w0808_a_live_effect_leaves_geometry_it_cannot_use_as_it_is() {
        let dot = input(&json!({"subpaths": [{"anchors": [{"p": [0, 0]}]}]}), "nonzero");
        let unusable = read_effect(Family::Stroke, &dot, b"{}", &Budget::DEFAULT).unwrap_err();
        assert_eq!(
            shown(&unusable),
            "warning SC-W0808: It draws nothing, so the live effect leaves the object as it is. Check its anchors, or remove stray objects far \
             from the design."
        );
    }

    #[test]
    fn stitches_come_back_as_paths_and_a_fill_s_as_ribbons() {
        // Coordinates are written to the nearest thousandth of a point: half of one, in millimetres.
        const WRITTEN: f64 = 0.0005 * MM_PER_POINT;
        let p = |x: f64, y: f64| Point::new(x, y).unwrap();
        let groups = vec![vec![p(0.0, 0.0), p(25.4 / 72.0 * 10.0, 0.0)], vec![p(1.0, 1.0)]];
        let out: Value = serde_json::from_slice(&effect_output(Family::Stroke, &groups).unwrap()).unwrap();
        let subpaths = &out["objects"][0]["path"]["subpaths"];
        assert_eq!(subpaths.as_array().unwrap().len(), 1, "a group of one needle point draws nothing");
        assert!(subpaths[0].get("closed").is_none(), "VectorCraft reads a subpath without `closed` as open");
        let end = subpaths[0]["anchors"][1]["p"][0].as_f64().unwrap();
        assert_eq!(end, 10.0);
        let out: Value = serde_json::from_slice(&effect_output(Family::Fill, &groups).unwrap()).unwrap();
        let ribbon = &out["objects"][0]["path"]["subpaths"][0];
        assert_eq!((ribbon["closed"].as_bool(), ribbon["anchors"].as_array().unwrap().len()), (Some(true), 4));
        // The ribbon is as wide as RIBBON_MM, across the stitch.
        let y0 = ribbon["anchors"][0]["p"][1].as_f64().unwrap();
        let y3 = ribbon["anchors"][3]["p"][1].as_f64().unwrap();
        assert!(((y0 - y3).abs() * MM_PER_POINT - RIBBON_MM).abs() <= 2.0 * WRITTEN, "{y0} {y3}");
        // Across a slanted stitch too: from (0, 0) to (3, 4) mm, the ribbon's sides lie 0.1 mm to its left and
        // right, along (-0.8, 0.6).
        let slanted: Value = serde_json::from_slice(&effect_output(Family::Fill, &[vec![p(0.0, 0.0), p(3.0, 4.0)]]).unwrap()).unwrap();
        let corners: Vec<[f64; 2]> = slanted["objects"][0]["path"]["subpaths"][0]["anchors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|anchor| [anchor["p"][0].as_f64().unwrap() * MM_PER_POINT, anchor["p"][1].as_f64().unwrap() * MM_PER_POINT])
            .collect();
        let expected = [[-0.08, 0.06], [2.92, 4.06], [3.08, 3.94], [0.08, -0.06]];
        for (corner, expected) in corners.iter().zip(expected) {
            assert!((corner[0] - expected[0]).abs() <= WRITTEN && (corner[1] - expected[1]).abs() <= WRITTEN, "{corners:?}");
        }
        // A stitch of no length has no ribbon, and stitches that draw nothing are no output.
        assert_eq!(effect_output(Family::Fill, &[vec![p(1.0, 1.0), p(1.0, 1.0)]]), None);
        assert_eq!(effect_output(Family::Satin, &[vec![p(1.0, 1.0)], vec![]]), None);
    }
}
