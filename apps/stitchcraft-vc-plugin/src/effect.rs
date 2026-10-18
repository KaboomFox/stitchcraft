//! What the plug-ins do with VectorCraft's JSON. Safe Rust, tested natively; `abi.rs` only moves bytes.
//!
//! A family's live effect reads the object it is given with the VectorCraft adapter, as an element of its
//! family with its record's parameters, and shows it as [`crate::preview`] decides: its stitches, a fill's
//! rows or its outline, or the object as it is, with why ([`sew`]).

use serde_json::{Value, json};
use stitchcraft_params::Family;
use stitchcraft_vectorcraft::{effect_error, read_effect};

use crate::preview::{show, spendable};

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
    let spend = spendable(input.len(), params.len());
    Ok(match read_effect(family, input, params, &crate::preview::reading(spend)) {
        Ok(effect) => show(family, &effect, spend),
        Err(refusal) => effect_error(&refusal),
    })
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
        serde_json::from_slice(&sew(family, &input(path), &params(family, set)).unwrap()).unwrap()
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
    fn diag_sc_i0810_a_satin_column_drawn_as_one_path_is_left_as_it_is() {
        let out = run(Family::Satin, &square(), &[]);
        assert_eq!(
            error(&out),
            "info SC-I0810: This satin column is drawn as one path, as wide as its stroke, which VectorCraft does not give a live effect: \
             the effect leaves it as it is. To see its stitches in VectorCraft, draw its 2 rails, join them with Object › Compound Path \
             › Make and apply the effect to that."
        );
        // VectorCraft keeps the first 300 characters of a plug-in's error (`objects::decode` in its `crates/plugins`).
        assert!(error(&out).chars().count() <= 300);
    }

    #[test]
    fn diag_sc_i0810_a_zigzag_stroke_is_left_as_it_is() {
        let out = run(Family::Stroke, &square(), &[("stroke_method", json!("zigzag_stitch"))]);
        assert_eq!(
            error(&out),
            "info SC-I0810: This zigzag stroke is as wide as its stroke, which VectorCraft does not give a live effect: the effect leaves \
             it as it is. Exported with `stitch export`, it sews as wide as its stroke."
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
