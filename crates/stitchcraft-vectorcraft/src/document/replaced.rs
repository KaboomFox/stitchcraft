//! A path's own Auto effect inside a group with one (`SC-W0920`, `REQ-VC-034`).
//!
//! **Why a notice.** The nearest StitchCraft effects say how a path sews (`REQ-VC-016`), so a path with an
//! Auto effect of its own inside an automatic group sews by its own settings alone. VectorCraft stores an
//! effect with every parameter, its defaults too (`merged_params` in its `crates/effects`, read at
//! `3f8d7a0`), so the reader can't tell a setting the person chose from one left at its default, and can't
//! merge the group's under the path's. A person who set the group's overlap and then gave one path a wider
//! satin column loses the overlap there. The notice names the path and each setting it loses.

use serde_json::Value;
use stitchcraft_engine::auto::AutoParams;

use crate::values::text;

/// What `SC-W0920` hints.
pub(super) const HINT: &str = "To sew it by the group's settings, remove its own Auto effect, or set those settings on it too.";

/// What `SC-W0920` says of the object `who` whose own Auto effect has the parameters `own`, inside a group
/// whose Auto effect has `group`: each of the group's settings that is not at its default and that the
/// object's own effect sets otherwise, in the order of the settings' registry, with both values.
pub(super) fn message(who: &str, own: &Value, group: &Value) -> String {
    let lost: Vec<String> = AutoParams::SPECS
        .iter()
        .filter_map(|spec| {
            let value = |params: &Value| params.get(spec.key).and_then(text).unwrap_or_else(|| spec.default.to_string());
            let (theirs, ours) = (value(group), value(own));
            (!same(&theirs, spec.default) && !same(&theirs, &ours)).then(|| format!("{} {theirs} (its own: {ours})", spec.key))
        })
        .collect();
    let start = format!("{who} has StitchCraft's Auto effect of its own inside a group with one, and sews by its own settings alone");
    match lost.as_slice() {
        [] => format!("{start}, which set everything the group sets."),
        _ => format!("{start}: it doesn't take the group's {}.", lost.join(", ")),
    }
}

/// Whether the settings `a` and `b` are the same value: the same number, or the same text.
fn same(a: &str, b: &str) -> bool {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn diag_sc_w0920_names_each_group_setting_the_object_sets_otherwise() {
        let group = json!({"auto_overlap_mm": 0.6, "auto_max_satin_width_mm": 3, "auto_base_layer": true, "auto_fill_angle": "x"});
        // The overlap is lost; the widest column is the group's too; the base layer is at its default.
        let own = json!({"auto_overlap_mm": 1, "auto_max_satin_width_mm": 3.0});
        let said = message("The object", &own, &group);
        let lost = said.split(": it doesn't take the group's ").nth(1).unwrap();
        assert!(lost.contains("auto_overlap_mm 0.6 (its own: 1)"), "{said}");
        assert!(!said.contains("auto_max_satin_width_mm") && !said.contains("auto_base_layer"), "{said}");
        // A setting the object leaves out is its default, which loses the group's.
        assert!(message("The object", &json!({}), &group).contains("auto_overlap_mm 0.6 (its own: "));
        assert_eq!(
            message("The object", &group, &group),
            "The object has StitchCraft's Auto effect of its own inside a group with one, and sews by its own settings alone, which set \
             everything the group sets."
        );
        assert!(same("1", "1.0") && !same("1", "2") && same("x", "x") && !same("x", "1"));
    }
}
