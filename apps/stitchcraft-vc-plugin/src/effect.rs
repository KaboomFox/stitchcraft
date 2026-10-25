//! What the plug-ins do with VectorCraft's JSON. Safe Rust, tested natively; `abi.rs` only moves bytes.
//!
//! A family's live effect reads the object it is given with the VectorCraft adapter, as an element of its
//! family with its record's parameters, and shows it as [`crate::preview`] decides: its stitches, a fill's
//! rows or its outline, or the object as it is, with why ([`sew`]).

use serde_json::{Value, json};
use stitchcraft_core::{Code, Diagnostic, Fix};
use stitchcraft_params::Family;
use stitchcraft_vectorcraft::{effect_error, read_effect};

use crate::preview::{FUEL_BASE, show, spendable};

/// Why a run failed. The ABI shim turns these into the negative return codes VectorCraft expects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunError {
    /// The input or the parameters were not the JSON the ABI promises.
    BadInput,
    /// The output could not be encoded.
    Encode,
}

impl RunError {
    /// The ABI v1 return code (negative means failure).
    pub const fn code(self) -> i64 {
        match self {
            RunError::BadInput => -1,
            RunError::Encode => -2,
        }
    }
}

/// A family's live effect: the object VectorCraft gives it, read as an element of `family` with the
/// record's parameters, and shown within what VectorCraft lets the effect spend. When it shows no
/// stitches, the output says why instead, and VectorCraft shows that as the plug-in's last error and draws
/// the object as it is.
pub fn sew(family: Family, input: &[u8], params: &[u8]) -> Result<Vec<u8>, RunError> {
    let spend = spendable(FUEL_BASE, input.len(), params.len());
    Ok(match read_effect(family, input, params, &crate::preview::reading(spend)) {
        Ok(effect) => show(family, &effect, spend),
        Err(refusal) => effect_error(&refusal),
    })
}

/// `SC-E0813`: a filter needs what the VectorCraft that ran it doesn't give a filter, which `message` says,
/// and VectorCraft `version` or later does. The filter declines with it, and VectorCraft changes nothing.
pub(crate) fn too_old(message: &str, version: &str) -> Diagnostic {
    Diagnostic::new(Code::VcTooOld, message).with_fix(Fix::Hint(format!("Update VectorCraft to {version} or later.")))
}

/// The *hello* live effect: every input object's geometry, unchanged.
///
/// Input is VectorCraft's `{"objects": [{"type", "path", "fillRule", "bounds"}]}`; the output keeps only
/// each object's `path`, which is what a live effect returns.
pub fn hello(input: &[u8], params: &[u8]) -> Result<Vec<u8>, RunError> {
    let input: Value = serde_json::from_slice(input).map_err(|_| RunError::BadInput)?;
    let _params: Value = serde_json::from_slice(params).map_err(|_| RunError::BadInput)?;
    let objects = input.get("objects").and_then(Value::as_array).ok_or(RunError::BadInput)?;
    let out: Vec<Value> = objects.iter().map(|o| json!({ "path": o.get("path").cloned().unwrap_or(Value::Null) })).collect();
    serde_json::to_vec(&json!({ "objects": out })).map_err(|_| RunError::Encode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{self, input, params};

    #[test]
    fn hello_returns_the_geometry_unchanged() {
        let path = json!({"subpaths": [{"closed": true, "anchors": [{"p": [0, 0]}, {"p": [10, 0]}, {"p": [10, 5]}]}]});
        let input = json!({"objects": [{"type": "path", "path": path, "fillRule": "nonzero", "bounds": [0, 0, 10, 5]}]});
        let params = json!({"_context": {"mode": "effect", "bounds": [0, 0, 10, 5]}});
        let out = hello(&serde_json::to_vec(&input).unwrap(), &serde_json::to_vec(&params).unwrap()).unwrap();
        let out: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(out, json!({"objects": [{"path": path}]}));
    }

    /// The output of `family`'s effect on `path`, with every parameter at its default but `set`.
    fn run(family: Family, path: &Value, set: &[(&str, Value)]) -> Value {
        run_on(family, &input(path), set)
    }

    /// The output of `family`'s effect on the object VectorCraft gives as `input`, with every parameter at
    /// its default but `set`.
    fn run_on(family: Family, input: &[u8], set: &[(&str, Value)]) -> Value {
        serde_json::from_slice(&sew(family, input, &params(family, set)).unwrap()).unwrap()
    }

    /// Millimetres, in points.
    fn points(mm: f64) -> f64 {
        mm * 72.0 / 25.4
    }

    /// How far apart across a line along x the stitches of an effect's output lie, in millimetres: the
    /// spread of their y.
    fn across(out: &Value) -> f64 {
        let subpaths = out["objects"][0]["path"]["subpaths"].as_array().unwrap_or_else(|| panic!("no stitches: {out}"));
        let ys: Vec<f64> = subpaths.iter().flat_map(|s| s["anchors"].as_array().unwrap()).map(|a| a["p"][1].as_f64().unwrap()).collect();
        let (least, most) = ys.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &y| (lo.min(y), hi.max(y)));
        (most - least) * 25.4 / 72.0
    }

    #[test]
    fn req_vc_010_a_satin_column_drawn_as_one_path_shows_the_stitches_stitch_export_sews() {
        use stitchcraft_core::Budget;
        use stitchcraft_engine::design::DesignSettings;
        use stitchcraft_engine::generate::generate;
        use stitchcraft_engine::generators::Neighbours;
        use stitchcraft_plan::profiles::REFERENCE;
        use stitchcraft_vectorcraft::effect_output;

        // A line with a bend, at the exact level, which has all that VectorCraft allows.
        let line = testing::line(&[(0.0, 0.0), (60.0, 0.0), (90.0, 30.0)]);
        let exact = params(Family::Satin, &[(stitchcraft_vectorcraft::PREVIEW_LEVEL, json!("exact"))]);
        for mm in [2.0, 3.0, 6.0] {
            let out = sew(Family::Satin, &testing::stroked(&line, points(mm)), &exact).unwrap();
            // The same object in a document, with the satin effect: what `stitch export` sews of it.
            let object = json!({
                "id": 2,
                "appearance": {
                    "items": [{"kind": "stroke", "width": points(mm), "join": "Miter", "miter_limit": 10, "cap": "Butt",
                               "paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 0}}}],
                    "effects": [{"id": "plugin.dev.stitchcraft.satin"}]
                },
                "kind": {"type": "path", "path": line}
            });
            let document = json!({"format": "vectorcraft", "version": 4, "document": {"version": 1, "artboards": [], "next_id": 99,
                "layers": [{"id": 1, "kind": {"type": "layer", "color": {"Preset": 0}, "children": [object]}}]}});
            let read = stitchcraft_vectorcraft::read(&serde_json::to_vec(&document).unwrap(), &Budget::DEFAULT).unwrap();
            let [element] = read.design.elements() else { panic!("{:?}", read.design.elements()) };
            let generated = generate(element, &DesignSettings::default(), REFERENCE, &Neighbours::default(), &Budget::DEFAULT);
            let exported = effect_output(Family::Satin, &generated.generated.unwrap().groups).unwrap();
            assert_eq!(String::from_utf8(out).unwrap(), String::from_utf8(exported).unwrap(), "{mm} mm");
        }
    }

    #[test]
    fn req_vc_010_a_satin_column_and_a_zigzag_stroke_are_as_wide_as_their_stroke() {
        let line = testing::line(&[(0.0, 0.0), (100.0, 0.0)]);
        for mm in [1.5, 3.0, 5.0] {
            let satin = run_on(Family::Satin, &testing::stroked(&line, points(mm)), &[]);
            let zigzag = run_on(Family::Stroke, &testing::stroked(&line, points(mm)), &[("stroke_method", json!("zigzag_stitch"))]);
            for (what, out) in [("satin", satin), ("zigzag", zigzag)] {
                assert!((across(&out) - mm).abs() < 0.01, "{what} {mm} mm: {}", across(&out));
            }
        }
    }

    fn square() -> Value {
        testing::square(100.0)
    }

    fn rails() -> Value {
        testing::rails(100.0)
    }

    /// The error of a run's output, which VectorCraft shows as the plug-in's last error.
    fn error(out: &Value) -> &str {
        out["error"].as_str().unwrap_or_else(|| panic!("no error: {out}"))
    }

    #[test]
    fn each_family_sews_its_object_with_the_manifest_s_defaults() {
        for (family, path) in [(Family::Stroke, square()), (Family::Satin, rails()), (Family::Fill, square())] {
            let out = run(family, &path, &[]);
            assert!(out.get("error").is_none(), "{family:?}: {out}");
            let subpaths = out["objects"][0]["path"]["subpaths"].as_array().unwrap();
            assert!(!subpaths.is_empty(), "{family:?}");
            // A fill's stitches are ribbons, closed; a stroke's and a satin's are paths along them, which
            // leave `closed` out.
            let closed = family == Family::Fill;
            assert!(subpaths.iter().all(|s| s["closed"].as_bool().unwrap_or(false) == closed), "{family:?}");
        }
    }

    #[test]
    fn diag_sc_i0810_a_satin_column_drawn_as_one_path_given_no_stroke_is_left_as_it_is() {
        let out = run(Family::Satin, &square(), &[]);
        assert_eq!(
            error(&out),
            "info SC-I0810: This satin column is drawn as one path, as wide as its stroke, and VectorCraft gives the live effect no stroke \
             for it: the effect leaves it as it is. Give the object a stroke as wide as the column, or draw its 2 rails and join them with \
             Object › Compound Path › Make."
        );
        // VectorCraft keeps the first 300 characters of a plug-in's error (`objects::decode` in its `crates/plugins`).
        assert!(error(&out).chars().count() <= 300);
        // Given a stroke, it shows its stitches.
        assert!(run_on(Family::Satin, &testing::stroked(&square(), points(3.0)), &[]).get("error").is_none());
    }

    #[test]
    fn diag_sc_i0810_a_zigzag_stroke_given_no_stroke_is_left_as_it_is() {
        let out = run(Family::Stroke, &square(), &[("stroke_method", json!("zigzag_stitch"))]);
        assert_eq!(
            error(&out),
            "info SC-I0810: This zigzag stroke is as wide as its stroke, and VectorCraft gives the live effect no stroke for it: the effect \
             leaves it as it is. Give the object a stroke as wide as the zigzag."
        );
        assert!(error(&out).chars().count() <= 300);
        // At any preview level.
        for level in ["exact", "outline"] {
            let out =
                run(Family::Stroke, &square(), &[("stroke_method", json!("zigzag_stitch")), (stitchcraft_vectorcraft::PREVIEW_LEVEL, json!(level))]);
            assert!(error(&out).starts_with("info SC-I0810: This zigzag stroke"), "{level}");
        }
    }

    #[test]
    fn diag_sc_i0810_an_object_that_sews_no_stitch_is_left_as_it_is() {
        assert_eq!(
            error(&run(Family::Fill, &square(), &[("fill", json!(false))])),
            "info SC-I0810: This object sews no stitch, so the live effect leaves it as it is."
        );
    }

    #[test]
    fn what_cannot_be_sewn_is_the_live_effect_s_error() {
        // A method that arrives in a later version is said, rather than the object vanishing.
        assert!(error(&run(Family::Fill, &square(), &[("fill_method", json!("contour_fill"))])).starts_with("warning SC-W0011: "));
        assert!(error(&run(Family::Fill, &square(), &[("fill_method", json!("plaid"))])).starts_with("error SC-E0101: "));
        assert!(error(&run(Family::Stroke, &json!({"subpaths": []}), &[])).starts_with("warning SC-W0808: It draws nothing"));
    }

    #[test]
    fn malformed_input_is_an_error_not_a_panic() {
        assert_eq!(hello(b"not json", b"{}"), Err(RunError::BadInput));
        assert_eq!(hello(b"{}", b"{}"), Err(RunError::BadInput));
        assert_eq!(hello(br#"{"objects": []}"#, b"oops"), Err(RunError::BadInput));
        assert!(RunError::BadInput.code() < 0 && RunError::Encode.code() < 0);
    }
}
