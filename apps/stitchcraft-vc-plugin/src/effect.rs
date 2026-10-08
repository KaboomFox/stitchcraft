//! What the plug-ins do with VectorCraft's JSON. Safe Rust, tested natively; `abi.rs` only moves bytes.

use serde_json::{Value, json};

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

    #[test]
    fn malformed_input_is_an_error_not_a_panic() {
        assert_eq!(hello(b"not json", b"{}"), Err(RunError::BadInput));
        assert_eq!(hello(b"{}", b"{}"), Err(RunError::BadInput));
        assert_eq!(hello(br#"{"objects": []}"#, b"oops"), Err(RunError::BadInput));
        assert!(RunError::BadInput.code() < 0 && RunError::Encode.code() < 0);
    }
}
