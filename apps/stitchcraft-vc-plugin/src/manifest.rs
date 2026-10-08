//! Plug-in identities and manifests.
//!
//! VectorCraft reads a JSON manifest from every module (`docs/plugins.md` in VectorCraft). We validate our
//! manifests here with VectorCraft's published rules, so a manifest VectorCraft would reject fails our
//! tests instead of a user's install (`REQ-VC-002`).

use serde_json::{Value, json};

/// Reverse-DNS namespace of every StitchCraft plug-in id. Change it here only.
pub const PLUGIN_NAMESPACE: &str = "dev.stitchcraft";

/// VectorCraft ABI v1 limits on manifests (VectorCraft `crates/plugins/src/manifest.rs`).
pub mod limits {
    /// Largest manifest, in bytes.
    pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
    /// Most parameters one plug-in may declare.
    pub const MAX_PARAMS: usize = 64;
    /// Longest id and name, in characters.
    pub const MAX_ID_CHARS: usize = 64;
    /// Longest version string, in bytes.
    pub const MAX_VERSION_BYTES: usize = 32;
    /// Longest description, in bytes.
    pub const MAX_DESCRIPTION_BYTES: usize = 1024;
    /// Longest author string, in bytes.
    pub const MAX_AUTHOR_BYTES: usize = 128;
}

/// The id of plug-in `name` (`dev.stitchcraft.<name>`).
pub fn plugin_id(name: &str) -> String {
    format!("{PLUGIN_NAMESPACE}.{name}")
}

/// The manifest of the M0.6 *hello* live effect: returns the object's geometry unchanged.
pub fn hello() -> Value {
    json!({
        "id": plugin_id("hello"),
        "name": "StitchCraft Hello",
        "version": env!("CARGO_PKG_VERSION"),
        "kind": "effect",
        "author": "StitchCraft contributors",
        "description": "Returns the object's geometry unchanged. Proves the StitchCraft plug-in toolchain and VectorCraft's ABI v1 contract (roadmap step M0.6).",
        "params": {}
    })
}

/// Checks `manifest` against VectorCraft's ABI v1 rules; the error names the first broken rule.
pub fn validate(manifest: &Value) -> Result<(), String> {
    let text = |key: &str| manifest.get(key).and_then(Value::as_str).ok_or_else(|| format!("`{key}` must be a string"));
    let bytes = serde_json::to_vec(manifest).map_err(|e| e.to_string())?;
    if bytes.len() > limits::MAX_MANIFEST_BYTES {
        return Err(format!("manifest is {} bytes (limit {})", bytes.len(), limits::MAX_MANIFEST_BYTES));
    }
    let id = text("id")?;
    let id_ok =
        (1..=limits::MAX_ID_CHARS).contains(&id.chars().count()) && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !id_ok {
        return Err(format!("id `{id}` must be 1–64 characters from A-Z a-z 0-9 . _ -"));
    }
    let name = text("name")?;
    if !(1..=limits::MAX_ID_CHARS).contains(&name.chars().count()) || name.chars().any(char::is_control) {
        return Err(format!("name `{name}` must be 1–64 printable characters"));
    }
    let checks = [("version", limits::MAX_VERSION_BYTES), ("description", limits::MAX_DESCRIPTION_BYTES), ("author", limits::MAX_AUTHOR_BYTES)];
    for (key, max) in checks {
        if text(key)?.len() > max {
            return Err(format!("`{key}` is longer than {max} bytes"));
        }
    }
    if !matches!(text("kind")?, "filter" | "effect") {
        return Err("`kind` must be \"filter\" or \"effect\"".into());
    }
    let params = manifest.get("params").and_then(Value::as_object).ok_or("`params` must be an object")?;
    if params.len() > limits::MAX_PARAMS {
        return Err(format!("{} parameters (limit {})", params.len(), limits::MAX_PARAMS));
    }
    for key in params.keys() {
        let ok = !key.is_empty()
            && !key.starts_with('_')
            && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !matches!(key.as_str(), "id" | "ids" | "params" | "preview");
        if !ok {
            return Err(format!("parameter name `{key}` is not allowed"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_manifest_is_valid() {
        assert_eq!(validate(&hello()), Ok(()));
        assert_eq!(hello()["id"], "dev.stitchcraft.hello");
    }

    #[test]
    fn validation_catches_what_vectorcraft_rejects() {
        let mut bad_id = hello();
        bad_id["id"] = json!("has space");
        assert!(validate(&bad_id).is_err());

        let mut reserved = hello();
        reserved["params"] = json!({"preview": {"type": "bool", "default": false}});
        assert!(validate(&reserved).is_err());

        let mut too_many = hello();
        let params: serde_json::Map<String, Value> =
            (0..=limits::MAX_PARAMS).map(|i| (format!("p{i}"), json!({"type": "bool", "default": false}))).collect();
        too_many["params"] = Value::Object(params);
        assert!(validate(&too_many).is_err());

        let mut bad_kind = hello();
        bad_kind["kind"] = json!("tool");
        assert!(validate(&bad_kind).is_err());
    }
}
