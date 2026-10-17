//! What the plug-ins do with VectorCraft's JSON. Safe Rust, tested natively; `abi.rs` only moves bytes.
//!
//! A family's live effect sews the object it is given as the family, with its record's parameters, and
//! gives the stitches back as geometry for VectorCraft to draw ([`sew`]). The VectorCraft adapter reads
//! the object and writes the stitches; the engine sews it as it sews an element of a design: `generate`,
//! on the reference machine, with no neighbours, since a live effect sees one object. These stitches are
//! the exact level of M6.3's previews, which adds cheaper levels for objects whose stitches cost more to
//! write than a live effect may spend.
//!
//! **When it shows no stitches.** The effect leaves the object as it is, and says why as the plug-in's
//! last error, when the object cannot be sewn, when it sews no stitch, and when it is a satin column drawn
//! as one path: such a column is as wide as its stroke, and VectorCraft gives a live effect no stroke
//! (`SC-I0810`). A drawing that vanished, because a setting or a method it cannot sew yet left it no
//! stitches, could not be clicked to change the setting. Warnings about stitches that are shown have no
//! way to VectorCraft; the reports of the command line give them.

use serde_json::{Value, json};
use stitchcraft_core::{Budget, Code, Diagnostic, Fix};
use stitchcraft_engine::design::{DesignSettings, Element, Shape};
use stitchcraft_engine::generate::generate;
use stitchcraft_engine::generators::Neighbours;
use stitchcraft_engine::normalize::satin::{Shape as Column, recognize};
use stitchcraft_params::Family;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_vectorcraft::{effect_element, effect_error, effect_output};

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

/// A family's live effect: the object VectorCraft gives it sewn as `family`, with the record's parameters,
/// and its stitches given back as geometry. When it shows no stitches, the output says why instead, and
/// VectorCraft shows that as the plug-in's last error and draws the object as it is.
pub fn sew(family: Family, input: &[u8], params: &[u8]) -> Result<Vec<u8>, RunError> {
    let budget = Budget::DEFAULT;
    let element = match effect_element(family, input, params, &budget) {
        Ok(element) => element,
        Err(refusal) => return Ok(effect_error(&refusal)),
    };
    if let Some(unseen) = centre_line(family, &element, &budget) {
        return Ok(effect_error(&unseen));
    }
    let generation = generate(&element, &DesignSettings::default(), REFERENCE, &Neighbours::default(), &budget);
    let groups = generation.generated.map(|generated| generated.groups).unwrap_or_default();
    match effect_output(family, &groups) {
        Ok(Some(stitches)) => Ok(stitches),
        Ok(None) => Ok(effect_error(&no_stitch(generation.diagnostics))),
        Err(_) => Err(RunError::Encode),
    }
}

/// `SC-I0810` when `element`, of `family`, is a satin column drawn as one path, its centre line: the
/// column is as wide as its stroke, which VectorCraft does not give a live effect.
fn centre_line(family: Family, element: &Element, budget: &Budget) -> Option<Diagnostic> {
    let (Family::Satin, Shape::Stroke { path, .. }) = (family, &element.shape) else { return None };
    let Ok(Column::CentreLine { .. }) = recognize(path, &mut budget.meter()).ok()?.shape else { return None };
    let message = "This satin column is drawn as one path, so it is as wide as its stroke, which VectorCraft does not give a live effect: \
                   the effect leaves it as it is.";
    Some(Diagnostic::new(Code::VcEffectUnchanged, message).with_fix(Fix::Hint("To see its stitches in VectorCraft, draw it with 2 rails.".into())))
}

/// Why an element sews no stitch: the most serious of what sewing it said, the first of those, or
/// `SC-I0810` when it said nothing, as for a fill set not to be sewn.
fn no_stitch(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .into_iter()
        .min_by_key(Diagnostic::severity)
        .unwrap_or_else(|| Diagnostic::new(Code::VcEffectUnchanged, "This object sews no stitch, so the live effect leaves it as it is."))
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

    #[test]
    fn hello_returns_the_geometry_unchanged() {
        let path = json!({"subpaths": [{"closed": true, "anchors": [{"p": [0, 0]}, {"p": [10, 0]}, {"p": [10, 5]}]}]});
        let input = json!({"objects": [{"type": "path", "path": path, "fillRule": "nonzero", "bounds": [0, 0, 10, 5]}]});
        let params = json!({"_context": {"mode": "effect", "bounds": [0, 0, 10, 5]}});
        let out = hello(&serde_json::to_vec(&input).unwrap(), &serde_json::to_vec(&params).unwrap()).unwrap();
        let out: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(out, json!({"objects": [{"path": path}]}));
    }

    /// VectorCraft's input for one object whose path is `path`, and the parameters as it resolves them
    /// from `family`'s manifest: every one at its default, then `set`.
    fn run(family: Family, path: &Value, set: &[(&str, Value)]) -> Value {
        let input = json!({"objects": [{"type": "path", "path": path, "fillRule": "nonzero", "bounds": [0, 0, 100, 100]}]});
        let mut params = serde_json::to_value(crate::manifest::family(family)).unwrap()["params"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, spec)| (key.clone(), spec["default"].clone()))
            .collect::<serde_json::Map<_, _>>();
        params.insert("_context".into(), json!({"mode": "effect", "bounds": [0, 0, 100, 100]}));
        for (key, value) in set {
            params.insert((*key).into(), value.clone());
        }
        let out = sew(family, &serde_json::to_vec(&input).unwrap(), &serde_json::to_vec(&params).unwrap()).unwrap();
        serde_json::from_slice(&out).unwrap()
    }

    fn square() -> Value {
        json!({"subpaths": [{"closed": true, "anchors": [{"p": [0, 0]}, {"p": [100, 0]}, {"p": [100, 100]}, {"p": [0, 100]}]}]})
    }

    /// A satin column's two rails, 20 points (7 mm) apart.
    fn rails() -> Value {
        json!({"subpaths": [{"anchors": [{"p": [0, 0]}, {"p": [100, 0]}]}, {"anchors": [{"p": [0, 20]}, {"p": [100, 20]}]}]})
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
            // A fill's stitches are ribbons, closed; a stroke's and a satin's are paths along them.
            let closed = family == Family::Fill;
            assert!(subpaths.iter().all(|s| s["closed"] == closed), "{family:?}");
        }
    }

    #[test]
    fn diag_sc_i0810_a_satin_column_drawn_as_one_path_is_left_as_it_is() {
        assert_eq!(
            error(&run(Family::Satin, &square(), &[])),
            "info SC-I0810: This satin column is drawn as one path, so it is as wide as its stroke, which VectorCraft does not give a live \
             effect: the effect leaves it as it is. To see its stitches in VectorCraft, draw it with 2 rails."
        );
    }

    #[test]
    fn diag_sc_i0810_an_object_that_sews_no_stitch_is_left_as_it_is() {
        assert_eq!(
            error(&run(Family::Fill, &square(), &[("fill", json!(false))])),
            "info SC-I0810: This object sews no stitch, so the live effect leaves it as it is."
        );
    }

    #[test]
    fn what_cannot_be_sewn_is_the_live_effect_s_error_and_the_most_serious_first() {
        // A method that arrives in a later version is said, rather than the object vanishing.
        assert!(error(&run(Family::Fill, &square(), &[("fill_method", json!("contour_fill"))])).starts_with("warning SC-W0011: "));
        assert!(error(&run(Family::Fill, &square(), &[("fill_method", json!("plaid"))])).starts_with("error SC-E0101: "));
        assert!(error(&run(Family::Stroke, &json!({"subpaths": []}), &[])).starts_with("warning SC-W0808: It draws nothing"));
        // The most serious comes first, and the first of those.
        let said = vec![
            Diagnostic::new(Code::VcNotPrinted, "a"),
            Diagnostic::new(Code::VcGeometryUnusable, "b"),
            Diagnostic::new(Code::VcUnreadable, "c"),
            Diagnostic::new(Code::BudgetExhausted, "d"),
        ];
        assert_eq!(no_stitch(said).message, "c");
    }

    #[test]
    fn malformed_input_is_an_error_not_a_panic() {
        assert_eq!(hello(b"not json", b"{}"), Err(RunError::BadInput));
        assert_eq!(hello(b"{}", b"{}"), Err(RunError::BadInput));
        assert_eq!(hello(br#"{"objects": []}"#, b"oops"), Err(RunError::BadInput));
        assert!(RunError::BadInput.code() < 0 && RunError::Encode.code() < 0);
    }
}
