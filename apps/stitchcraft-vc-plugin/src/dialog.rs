//! How VectorCraft's dialogs show each parameter (`REQ-VC-012`).
//!
//! From 0.14, a manifest gives each parameter a label, the unit its value is in, the section of the dialog
//! it is in, help and a help page, and the values of the parameters before it that show it
//! (`visibleWhen`). A release before 0.14 ignores them, so the modules install there as they did, with
//! each parameter's key as its label, its first letter a capital, and every one shown (VectorCraft's
//! `docs/plugins.md`, Manifest, and its `form::humanize`). The parameter registry already says all of it,
//! and the reference pages are made from it too:
//!
//! - the label is the parameter's `label`, and the unit its kind's ([`Kind::unit`]);
//! - the section is its `group`, as the headings of its reference page are;
//! - the help is its help text as plain text, within VectorCraft's 1,000 characters, and the help page
//!   its entry in the parameter reference on the docs site ([`SITE_URL`]);
//! - it shows for the methods it applies to: one of some of the family's methods only while the method
//!   parameter has one of them, and one the registry shows only while another setting has some values
//!   (its `when`) only then too.
//!
//! VectorCraft gives a hidden parameter to the plug-in with its value, so what a dialog shows never
//! changes what sews. The plug-in's own parameters, the preview level and the exporters' options, say
//! the same of themselves.

use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};
use serde_json::Value;
use stitchcraft_core::text::plain;
use stitchcraft_params::{ChoiceOption, Family, Kind, ParamGroup, ParamSpec, StitchType, toggle};

use crate::manifest::{Param, Schema};

/// The docs site, where the docs workflow publishes `main`'s book (`.github/workflows/docs.yml`, `/dev/`).
/// A page of `docs/src/` is there at its path, `.html` for `.md`. The docs' checks find each help page a
/// manifest links to, and its anchor (`xtask/src/docs.rs`).
pub const SITE_URL: &str = "https://kaboomfox.github.io/stitchcraft/dev/";

/// The address of `path`, a page of the docs site and maybe an anchor in it.
pub fn page(path: &str) -> String {
    format!("{SITE_URL}{path}")
}

/// VectorCraft's limits on the keys that say how a dialog shows a parameter (its
/// `crates/plugins/src/dialog.rs`, from 0.14).
pub mod limits {
    /// Longest label or section, in characters.
    pub const MAX_LABEL: usize = 64;
    /// Longest unit, in characters.
    pub const MAX_UNIT: usize = 16;
    /// Longest help, in characters.
    pub const MAX_HELP: usize = 1_000;
    /// Longest help page address, in bytes.
    pub const MAX_HELP_URL: usize = 512;
    /// Most parameters one `visibleWhen` names.
    pub const MAX_CONDITIONS: usize = 8;
    /// Most values a `visibleWhen` lists for one parameter.
    pub const MAX_CONDITION_VALUES: usize = 64;
}

/// How VectorCraft's dialogs show a parameter, every key optional (`ParamUi` in its `crates/plugins`).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ui {
    /// The row's label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<&'static str>,
    /// The unit after the value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
    /// The section of the dialog the row is in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<&'static str>,
    /// What the parameter does, shown when the pointer rests on its label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// The page about it, which a button after its field opens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_url: Option<String>,
    /// The parameters before it that show it, each with the values of theirs that do: all at once.
    #[serde(skip_serializing_if = "Vec::is_empty", serialize_with = "conditions")]
    pub visible_when: Vec<(&'static str, Vec<Value>)>,
}

/// Serializes `visible_when` as VectorCraft reads it: a JSON object of each parameter's values.
fn conditions<S: Serializer>(visible_when: &[(&'static str, Vec<Value>)], serializer: S) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(visible_when.len()))?;
    for (key, values) in visible_when {
        map.serialize_entry(key, values)?;
    }
    map.end()
}

/// How `family`'s dialog shows `spec`, a parameter of the declaration `group` that the manifest declares as
/// `schema`, after the parameters `before`; or `None` when nothing the dialog can show would show it,
/// such as a parameter of no method the dialog offers.
pub fn registry_ui(spec: &ParamSpec, schema: &Schema, group: &ParamGroup, family: Family, before: &[(&'static str, Param)]) -> Option<Ui> {
    Some(Ui {
        label: Some(spec.label),
        unit: unit(spec.kind),
        group: Some(spec.group),
        help: help_then(&spec.help(), &options(spec, schema)),
        help_url: Some(page(&format!("user/reference/params/{}.html#{}", group.slug(), spec.key))),
        visible_when: visible_when(spec, family, before)?,
    })
}

/// What the options a choice offers as `schema` mean, for its help, with their labels from `spec`:
/// VectorCraft's drop-down list shows only their ids, which designs store. Nothing for another kind.
fn options(spec: &ParamSpec, schema: &Schema) -> String {
    match (spec.kind, schema) {
        (Kind::Choice { options }, Schema::Choice { options: offered, .. }) => {
            format!("One of {}.", ChoiceOption::list(options.iter().filter(|option| offered.contains(&option.id))))
        }
        _ => String::new(),
    }
}

/// A parameter of the plug-in's own, which no stitch has: its kind of value, and its row's `label`, `help`
/// and help page, at `path` on the docs site.
pub fn own(schema: Schema, label: &'static str, help: &str, path: &str) -> Param {
    Param { schema, ui: Ui { label: Some(label), help: self::help(help), help_url: Some(page(path)), ..Ui::default() } }
}

/// The unit a parameter of `kind` is in, if any.
pub(crate) fn unit(kind: Kind) -> Option<&'static str> {
    Some(kind.unit()).filter(|unit| !unit.is_empty())
}

/// The help VectorCraft shows of the help text `markdown`: as plain text, whole, or as many of its
/// paragraphs and sentences as fit in [`limits::MAX_HELP`] characters, an ellipsis after them. The help
/// page has all of it.
pub fn help(markdown: &str) -> Option<String> {
    help_then(markdown, "")
}

/// [`help`] of `markdown`, and then the paragraph `after`, which is kept whole: the help is cut to make
/// room for it.
fn help_then(markdown: &str, after: &str) -> Option<String> {
    let after = plain(after);
    let room = match after.chars().count() {
        0 => limits::MAX_HELP,
        n => limits::MAX_HELP.saturating_sub(n + 2),
    };
    let paragraphs: Vec<String> = [cut(&plain(markdown), room), after].into_iter().filter(|text| !text.trim().is_empty()).collect();
    Some(cut(&paragraphs.join("\n\n"), limits::MAX_HELP)).filter(|text| !text.is_empty())
}

/// `text` within `room` characters: whole, or as many of its sentences as fit and an ellipsis after them,
/// or as many of its characters when no sentence ends among them.
fn cut(text: &str, room: usize) -> String {
    if text.chars().count() <= room {
        return text.to_string();
    }
    let kept: String = text.chars().take(room.saturating_sub(1)).collect();
    let end = [kept.rfind(". "), kept.rfind(".\n")].into_iter().flatten().max().map_or(kept.len(), |at| at + 1);
    format!("{}…", kept.get(..end).unwrap_or(&kept).trim_end())
}

/// When `family`'s dialog shows `spec`, given the parameters `before` it: while the method parameter has
/// one of the methods it applies to, unless it applies to all the dialog offers, and while the setting its
/// registry entry names has one of the values that show it. `None` when no value shows it.
fn visible_when(spec: &ParamSpec, family: Family, before: &[(&'static str, Param)]) -> Option<Vec<(&'static str, Vec<Value>)>> {
    let declared = |key: &str| before.iter().find(|(k, _)| *k == key).map(|(_, param)| &param.schema);
    let mut conditions = Vec::new();
    let method = family.method_param();
    if let (true, Some(Schema::Choice { options, .. })) = (spec.key != method, declared(method)) {
        let applied = |id: &str| StitchType::from_id(family, id).is_some_and(|t| spec.applies_to.contains(&t));
        let methods: Vec<Value> = options.iter().copied().filter(|id| applied(id)).map(Value::from).collect();
        if methods.is_empty() {
            return None;
        }
        if methods.len() < options.len() {
            conditions.push((method, methods));
        }
    }
    if let Some(when) = spec.visible_when {
        let values: Vec<Value> = match declared(when.key) {
            Some(Schema::Choice { options, .. }) => when.any_of.iter().copied().filter(|v| options.contains(v)).map(Value::from).collect(),
            Some(Schema::Bool { .. }) => when.any_of.iter().filter_map(|v| toggle(v)).map(Value::Bool).collect(),
            // A setting the dialog doesn't show before it shows nothing here: the row shows, and the tests
            // check that no parameter of the registry needs one.
            _ => return Some(conditions),
        };
        if values.is_empty() {
            return None;
        }
        conditions.push((when.key, values));
    }
    Some(conditions)
}

/// The parameters of `params`, in their order, that VectorCraft's dialog shows while every value is its
/// default: each whose `visibleWhen` names parameters shown, each at a default it lists, as VectorCraft
/// works it out (`Manifest::visible` in its `crates/plugins/src/dialog.rs`).
pub fn shown_at_defaults(params: &[(&'static str, Param)]) -> Vec<&'static str> {
    let mut shown: Vec<&'static str> = Vec::new();
    for (key, param) in params {
        let default = |other: &str| params.iter().find(|(k, _)| *k == other).map(|(_, p)| p.schema.default_value());
        let met = param.ui.visible_when.iter().all(|(other, values)| shown.contains(other) && default(other).is_some_and(|d| values.contains(&d)));
        if met {
            shown.push(key);
        }
    }
    shown
}

/// Checks how `spec` says a dialog shows its parameter, the one after those `before`, as VectorCraft does
/// from 0.14 (`ParamUi::validate`); the error names the first broken rule.
pub fn validate(spec: &Value, before: &[(&String, &Value)]) -> Result<(), String> {
    let text = |key: &str, max: usize, lines: bool| match spec.get(key) {
        None => Ok(()),
        Some(Value::String(s)) if printable(s, max, lines) => Ok(()),
        Some(_) => Err(format!("`{key}` must be 1–{max} printable characters")),
    };
    text("label", limits::MAX_LABEL, false)?;
    text("group", limits::MAX_LABEL, false)?;
    text("unit", limits::MAX_UNIT, false)?;
    text("help", limits::MAX_HELP, true)?;
    if let Some(url) = spec.get("helpUrl") {
        let url = url.as_str().unwrap_or_default();
        let ok = url.strip_prefix("https://").is_some_and(|rest| !rest.is_empty())
            && url.len() <= limits::MAX_HELP_URL
            && !url.chars().any(|c| c.is_whitespace() || c.is_control());
        if !ok {
            return Err(format!("`helpUrl` must be an https:// address of at most {} bytes", limits::MAX_HELP_URL));
        }
    }
    let Some(conditions) = spec.get("visibleWhen") else { return Ok(()) };
    let conditions = conditions.as_object().ok_or("`visibleWhen` must be an object")?;
    if conditions.len() > limits::MAX_CONDITIONS {
        return Err(format!("`visibleWhen` names at most {} parameters", limits::MAX_CONDITIONS));
    }
    for (other, values) in conditions {
        let Some((_, decider)) = before.iter().find(|(key, _)| *key == other) else {
            return Err(format!("`visibleWhen` names `{other}`, which isn't a parameter declared before this one"));
        };
        let values = values.as_array().filter(|v| (1..=limits::MAX_CONDITION_VALUES).contains(&v.len()));
        let values = values.ok_or(format!("`visibleWhen` lists 1 to {} values for `{other}`", limits::MAX_CONDITION_VALUES))?;
        let options: Vec<&str> = decider.get("options").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
        let fits = |v: &Value| match decider.get("type").and_then(Value::as_str) {
            Some("bool") => v.is_boolean(),
            Some("choice") => v.as_str().is_some_and(|s| options.contains(&s)),
            _ => false,
        };
        if !values.iter().all(fits) {
            return Err(format!("`visibleWhen` takes `{other}`'s values, of a checkbox or a choice"));
        }
    }
    Ok(())
}

/// Whether `s` is 1 to `max` characters, not all spaces, without control characters (but line ends,
/// with `lines`).
fn printable(s: &str, max: usize, lines: bool) -> bool {
    !s.trim().is_empty() && s.chars().count() <= max && s.chars().all(|c| !c.is_control() || (lines && c == '\n'))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::json;
    use stitchcraft_engine::registry::PARAMETERS;
    use stitchcraft_params::{Condition, Empty, Origin, Stability};
    use stitchcraft_plan::FormatId;

    use super::*;
    use crate::manifest::{PREVIEW_LEVEL, exporter, family, validate};

    /// The registry's declaration of `key` for `family`, and the declaration it is in.
    fn declared(key: &str, family: Family) -> (&'static ParamGroup, ParamSpec) {
        let found = PARAMETERS.iter().find_map(|group| {
            let spec = group.specs.iter().find(|spec| spec.key == key && spec.applies_to.iter().any(|t| t.family() == family))?;
            Some((*group, spec.in_family(family)))
        });
        found.unwrap_or_else(|| panic!("no {key} for {family:?}"))
    }

    /// The method options `family`'s dialog offers.
    fn methods(params: &[(&'static str, Param)], family: Family) -> Vec<&'static str> {
        match params.iter().find(|(key, _)| *key == family.method_param()).map(|(_, param)| &param.schema) {
            Some(Schema::Choice { options, .. }) => options.clone(),
            other => panic!("{family:?}'s method is {other:?}"),
        }
    }

    #[test]
    fn req_vc_012_each_row_has_the_registry_s_label_unit_section_help_and_page() {
        for each in Family::ALL {
            let manifest = family(*each);
            for (key, param) in manifest.params.iter().filter(|(key, _)| *key != PREVIEW_LEVEL) {
                let (group, spec) = declared(key, *each);
                let ui = &param.ui;
                assert_eq!((ui.label, ui.unit, ui.group), (Some(spec.label), Some(spec.kind.unit()).filter(|u| !u.is_empty()), Some(spec.group)));
                // The help, and for a choice what each option the dialog offers means, from its label.
                let text = ui.help.clone().unwrap_or_default();
                let first = plain(&spec.help()).split(". ").next().map(str::to_string).unwrap_or_default();
                assert!(text.starts_with(&first) && text.chars().count() <= limits::MAX_HELP, "{key}: {text}");
                if let (Kind::Choice { options }, Schema::Choice { options: offered, .. }) = (spec.kind, &param.schema) {
                    let listed = options.iter().filter(|option| offered.contains(&option.id));
                    assert!(text.ends_with(&format!("One of {}.", plain(&ChoiceOption::list(listed)))), "{key}: {text}");
                }
                assert_eq!(ui.help_url, Some(format!("{SITE_URL}user/reference/params/{}.html#{key}", group.slug())));
            }
            // The labels tell the rows apart.
            let labels: BTreeSet<Option<&str>> = manifest.params.iter().map(|(_, param)| param.ui.label).collect();
            assert_eq!(labels.len(), manifest.params.len(), "{each:?}");
            assert!(!labels.contains(&None));
        }
        // The plug-in's own parameters say what they do, and link to the page that explains them.
        let preview = family(Family::Fill).params.into_iter().find(|(key, _)| *key == PREVIEW_LEVEL).map(|(_, param)| param.ui);
        assert!(preview.is_some_and(|ui| ui.label == Some("Preview") && ui.group == Some("Preview") && ui.help.is_some()));
        for (key, param) in exporter(FormatId::Dst).params {
            let url = param.ui.help_url.unwrap_or_default();
            assert!(param.ui.label.is_some() && param.ui.help.is_some() && url.starts_with(SITE_URL), "{key}");
        }
    }

    #[test]
    fn req_vc_012_the_registry_s_groups_are_the_dialog_s_sections_in_its_order() {
        // VectorCraft draws a section where its group first appears, with every row of the group in it. Each
        // group's rows come together in the manifest, so the dialog keeps the order hosts before 0.14 show.
        for each in Family::ALL {
            let params = family(*each).params;
            let mut sections: Vec<Option<&str>> = Vec::new();
            for (_, param) in &params {
                if sections.last() != Some(&param.ui.group) {
                    assert!(!sections.contains(&param.ui.group), "{each:?}: {:?} comes twice", param.ui.group);
                    sections.push(param.ui.group);
                }
            }
            assert!(sections.iter().all(Option::is_some), "{each:?}: every row is in a section");
            assert_eq!(sections.last(), Some(&Some("Preview")));
        }
    }

    #[test]
    fn req_vc_012_a_row_of_some_methods_shows_only_while_one_of_them_is_chosen() {
        for each in Family::ALL {
            let params = family(*each).params;
            let (method, offered) = (each.method_param(), methods(&params, *each));
            for (key, param) in params.iter().filter(|(key, _)| *key != PREVIEW_LEVEL && *key != method) {
                let spec = declared(key, *each).1;
                let applies = |id: &&str| StitchType::from_id(*each, id).is_some_and(|t| spec.applies_to.contains(&t));
                let shown_for: Vec<Value> = offered.iter().copied().filter(applies).map(|id| json!(id)).collect();
                let condition = param.ui.visible_when.iter().find(|(other, _)| *other == method).map(|(_, values)| values.clone());
                let expected = (shown_for.len() < offered.len()).then_some(shown_for);
                assert_eq!(condition, expected, "{each:?} {key}");
            }
            assert!(params.iter().find(|(key, _)| *key == method).is_some_and(|(_, param)| param.ui.visible_when.is_empty()));
        }
        // At its defaults, the stroke dialog shows a running stitch's rows, and not a zigzag's or a manual
        // stitch's.
        let shown = shown_at_defaults(&family(Family::Stroke).params);
        assert!(shown.contains(&"repeats") && shown.contains(&"ties"), "{shown:?}");
        assert!(!shown.contains(&"zigzag_spacing_mm") && !shown.contains(&"max_stitch_length_mm"), "{shown:?}");
    }

    #[test]
    fn req_vc_012_a_row_the_registry_shows_on_a_condition_shows_on_it() {
        for each in Family::ALL {
            let params = family(*each).params;
            for (i, (key, param)) in params.iter().enumerate().filter(|(_, (key, _))| *key != PREVIEW_LEVEL) {
                let Some(when) = declared(key, *each).1.visible_when else { continue };
                // The setting it depends on comes before it, so that the condition can name it.
                let decider = params.get(..i).unwrap_or_default().iter().find(|(other, _)| *other == when.key);
                assert!(decider.is_some(), "{each:?} {key}: {} isn't before it", when.key);
                let values = param.ui.visible_when.iter().find(|(other, _)| *other == when.key).map(|(_, values)| values.len());
                assert!(values.is_some_and(|n| (1..=when.any_of.len()).contains(&n)), "{each:?} {key}: {:?}", param.ui.visible_when);
            }
        }
        let condition = |family: Family, key: &str| family_condition(family, key);
        assert_eq!(condition(Family::Satin, "random_split_phase"), vec![("split_method", vec![json!("default")])]);
        assert_eq!(condition(Family::Satin, "split_staggers"), vec![("split_method", vec![json!("staggered")])]);
        // A toggle's: the length variation shows while random stitch length is on, for the methods it varies.
        let jitter = condition(Family::Stroke, "random_stitch_length_jitter_percent");
        assert_eq!(jitter.last(), Some(&("enable_random_stitch_length", vec![json!(true)])));
        // A lock's size shows for the locks it sizes, and the custom lock, which a text spells, isn't offered.
        let sized = condition(Family::Fill, "lock_start_scale_mm");
        assert_eq!(sized, vec![("lock_start", vec![json!("back_forth")])]);
    }

    /// The `visibleWhen` of `key` in `family`'s dialog.
    fn family_condition(family_: Family, key: &str) -> Vec<(&'static str, Vec<Value>)> {
        let params = family(family_).params;
        params.into_iter().find(|(k, _)| *k == key).map(|(_, param)| param.ui.visible_when).unwrap_or_else(|| panic!("no {key}"))
    }

    /// How a manifest declares [`SPEC`].
    const PERCENT: Schema = Schema::Number { min: 0.0, max: 100.0, default: 0.0 };

    /// A parameter of the satin column alone, for the rows below.
    const SPEC: ParamSpec = ParamSpec {
        key: "split_jitter",
        label: "Split jitter",
        help: " How much a split moves.\n",
        kind: Kind::Percent { min: 0.0, max: 100.0 },
        default: "0",
        family_defaults: &[],
        group: "Split stitches",
        applies_to: &[StitchType::SatinColumn],
        visible_when: Some(Condition { key: "split_method", any_of: &["default", "random"] }),
        stability: Stability::Stable,
        origin: Origin::InkStitch,
    };

    #[test]
    fn a_row_nothing_the_dialog_offers_shows_is_left_out() {
        let group = ParamGroup { name: "SatinParams", help: "", applies_to: &[StitchType::SatinColumn], specs: &[SPEC] };
        let choice = |options: Vec<&'static str>| Param {
            schema: Schema::Choice { default: options.first().copied().unwrap_or(""), options },
            ui: Ui::default(),
        };
        let before = |methods, splits| vec![("satin_method", choice(methods)), ("split_method", choice(splits))];
        let ui = registry_ui(&SPEC, &PERCENT, &group, Family::Satin, &before(vec!["satin_column", "e_stitch"], vec!["default", "staggered"]));
        let conditions = ui.map(|ui| ui.visible_when);
        assert_eq!(conditions, Some(vec![("satin_method", vec![json!("satin_column")]), ("split_method", vec![json!("default")])]));
        // Shown for every method offered, it says nothing of the method.
        let ui = registry_ui(&SPEC, &PERCENT, &group, Family::Satin, &before(vec!["satin_column"], vec!["default"]));
        assert_eq!(ui.map(|ui| ui.visible_when), Some(vec![("split_method", vec![json!("default")])]));
        // No method offered, or no value of its setting, shows it.
        assert_eq!(registry_ui(&SPEC, &PERCENT, &group, Family::Satin, &before(vec!["e_stitch"], vec!["default"])), None);
        assert_eq!(registry_ui(&SPEC, &PERCENT, &group, Family::Satin, &before(vec!["satin_column"], vec!["staggered"])), None);
        // A setting the dialog doesn't show before it: the row shows.
        let ui = registry_ui(
            &SPEC,
            &PERCENT,
            &group,
            Family::Satin,
            &before(vec!["satin_column"], vec!["default"]).into_iter().take(1).collect::<Vec<_>>(),
        );
        assert_eq!(ui.map(|ui| ui.visible_when), Some(Vec::new()));
        let length = ParamSpec { kind: Kind::Length { min: 0.0, max: 9.0, empty: Empty::Never }, visible_when: None, ..SPEC };
        let ui = registry_ui(&length, &PERCENT, &group, Family::Satin, &[]).unwrap();
        assert_eq!((ui.unit, ui.help.as_deref()), (Some("mm"), Some("How much a split moves.")));
        assert_eq!(ui.help_url.as_deref(), Some("https://kaboomfox.github.io/stitchcraft/dev/user/reference/params/satin.html#split_jitter"));
    }

    #[test]
    fn a_choice_s_help_says_what_each_option_it_offers_means() {
        let help = |family_: Family, key: &str| {
            let params = family(family_).params;
            params.into_iter().find(|(k, _)| *k == key).and_then(|(_, param)| param.ui.help).unwrap_or_else(|| panic!("no help for {key}"))
        };
        let ties = help(Family::Fill, "ties");
        assert!(ties.starts_with("Where the element gets lock stitches"), "{ties}");
        assert!(ties.ends_with("\n\nOne of 0 (At the start and the end), 1 (At the start), 2 (At the end), 3 (None)."), "{ties}");
        // Only the options the dialog offers: a custom lock, which a text spells out, isn't.
        let lock = help(Family::Stroke, "lock_start");
        assert!(lock.contains("One of half_stitch (") && !lock.contains("custom"), "{lock}");
        // A long help is cut to keep the options whole, at a sentence, or where VectorCraft's limit makes room
        // for them.
        let long = format!("{}\n\nMore.", "A sentence of some length. ".repeat(60));
        let cut = help_then(&long, "One of `a` (A).").unwrap();
        assert!(cut.ends_with("length.…\n\nOne of a (A).") && cut.chars().count() <= limits::MAX_HELP, "{cut}");
        let unbroken = help_then(&"x".repeat(2000), "One of `a` (A).").unwrap();
        assert_eq!(unbroken, format!("{}…\n\nOne of a (A).", "x".repeat(limits::MAX_HELP - 16)));
        assert_eq!(unbroken.chars().count(), limits::MAX_HELP);
    }

    #[test]
    fn help_too_long_for_vectorcraft_ends_at_a_sentence() {
        let long = "A sentence of some length. ".repeat(60);
        let cut = help(&long).unwrap();
        assert!(cut.chars().count() <= limits::MAX_HELP && cut.ends_with("length.…"), "{cut}");
        assert_eq!(help("Short."), Some("Short.".to_string()));
        assert_eq!(help("  "), None);
        // No sentence ends within the limit: cut at it.
        assert_eq!(help(&"x".repeat(2000)).map(|h| h.chars().count()), Some(limits::MAX_HELP));
    }

    #[test]
    fn a_row_shows_when_each_row_it_names_shows_at_a_value_it_lists() {
        let row = |schema: Schema, visible_when: Vec<(&'static str, Vec<Value>)>| Param { schema, ui: Ui { visible_when, ..Ui::default() } };
        let params = vec![
            ("mode", row(Schema::Choice { options: vec!["a", "b"], default: "a" }, Vec::new())),
            ("outline", row(Schema::Bool { default: true }, vec![("mode", vec![json!("b")])])),
            ("width", row(Schema::Number { min: 0.0, max: 1.0, default: 0.5 }, vec![("outline", vec![json!(true)])])),
            ("seed", row(Schema::Int { min: 0, max: 9, default: 0 }, vec![("mode", vec![json!("a")])])),
        ];
        // The outline shows for mode b only, so its width hides too, though the outline's default shows it.
        assert_eq!(shown_at_defaults(&params), ["mode", "seed"]);
    }

    /// A manifest's text with the parameters `params`, the inside of a JSON object.
    fn manifest(params: &str) -> Vec<u8> {
        format!(r#"{{"id":"a","name":"A","version":"1","kind":"effect","author":"x","description":"d","params":{{{params}}}}}"#).into_bytes()
    }

    #[test]
    fn validation_catches_the_dialog_keys_vectorcraft_rejects() {
        let number = |ui: &str| manifest(&format!(r#""p":{{"type":"number","min":0,"max":1,"default":0{ui}}}"#));
        let long = |n: usize| "x".repeat(n);
        for good in [
            String::new(),
            format!(r#","label":"{}","group":"{}","unit":"{}""#, long(64), long(64), long(16)),
            r#","help":"Two\nlines.","helpUrl":"https://example.org/p#p""#.to_string(),
        ] {
            assert_eq!(validate(&number(&good)), Ok(()), "{good}");
        }
        for bad in [
            r#","label":"""#.to_string(),
            r#","label":"  ""#.to_string(),
            format!(r#","label":"{}""#, long(65)),
            r#","label":"a\nb""#.to_string(),
            format!(r#","group":"{}""#, long(65)),
            format!(r#","unit":"{}""#, long(17)),
            format!(r#","help":"{}""#, long(1001)),
            r#","help":"tab\there""#.to_string(),
            r#","label":5"#.to_string(),
            r#","helpUrl":"http://example.org""#.to_string(),
            r#","helpUrl":"https://""#.to_string(),
            r#","helpUrl":"https://example.org/a b""#.to_string(),
            format!(r#","helpUrl":"https://{}""#, long(505)),
            r#","visibleWhen":[]"#.to_string(),
        ] {
            assert!(validate(&number(&bad)).is_err(), "{bad}");
        }
        // A condition names a checkbox or a choice declared before, with 1 to 64 of its values.
        let choice = r#""m":{"type":"choice","options":["a","b"]}"#;
        let check = r#""c":{"type":"bool"}"#;
        let count = r#""n":{"type":"int"}"#;
        let shown = |when: &str| format!(r#""p":{{"type":"bool","visibleWhen":{when}}}"#);
        assert_eq!(validate(&manifest(&format!(r#"{choice},{check},{}"#, shown(r#"{"m":["b"],"c":[true,false]}"#)))), Ok(()));
        for bad in [
            format!(r#"{},{choice}"#, shown(r#"{"m":["b"]}"#)),
            format!(r#"{choice},{}"#, shown(r#"{"z":["b"]}"#)),
            format!(r#"{choice},{}"#, shown(r#"{"m":["c"]}"#)),
            format!(r#"{choice},{}"#, shown(r#"{"m":[]}"#)),
            format!(r#"{choice},{}"#, shown(&format!(r#"{{"m":[{}]}}"#, vec![r#""a""#; 65].join(",")))),
            format!(r#"{check},{}"#, shown(r#"{"c":["true"]}"#)),
            format!(r#"{count},{}"#, shown(r#"{"n":[1]}"#)),
        ] {
            assert!(validate(&manifest(&bad)).is_err(), "{bad}");
        }
        // A condition names at most 8 parameters.
        let naming = |n: usize| {
            let deciders: Vec<String> = (0..n).map(|i| format!(r#""b{i}":{{"type":"bool"}}"#)).collect();
            let conditions: Vec<String> = (0..n).map(|i| format!(r#""b{i}":[true]"#)).collect();
            validate(&manifest(&format!(r#"{},{}"#, deciders.join(","), shown(&format!("{{{}}}", conditions.join(","))))))
        };
        assert_eq!(naming(limits::MAX_CONDITIONS), Ok(()));
        assert!(naming(limits::MAX_CONDITIONS + 1).is_err_and(|why| why.contains("at most 8")));
    }
}
