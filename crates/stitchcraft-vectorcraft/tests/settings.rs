//! The design settings and the machine that StitchCraft's Design Settings filter keeps with a VectorCraft
//! document (`REQ-VC-013`): read with the document, and the machine before it.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::{RED, document, square};
use serde_json::{Value, json};
use stitchcraft_core::units::MM_PER_POINT;
use stitchcraft_core::{Budget, Code, Mm};
use stitchcraft_engine::design::{DesignSettings, Reading};
use stitchcraft_plan::profiles::{BROTHER_PE800_SMALL, REFERENCE};
use stitchcraft_vectorcraft::settings::{SETTINGS_FILTER, kept_data};
use stitchcraft_vectorcraft::{VectorCraft, machine, plugin_id, read_with};

/// `document` with `data` kept under the plug-in id `id`, as VectorCraft 0.16 keeps a filter's data.
fn keeping(document: &[u8], id: &str, data: &Value) -> Vec<u8> {
    let mut file: Value = serde_json::from_slice(document).unwrap();
    file["document"]["unknown"] = json!({"pluginData": {id: data}});
    serde_json::to_vec(&file).unwrap()
}

/// `document` read, with `--auto` or without.
fn read(document: &[u8], auto: bool) -> VectorCraft {
    read_with(document, &Budget::DEFAULT, Reading { auto }).unwrap()
}

/// Settings with a shortest jump of 10 mm and a narrowest satin stroke of 4 mm.
fn settings() -> DesignSettings {
    DesignSettings { collapse_len: Mm::new(10.0).unwrap(), min_satin_stroke_width: Mm::new(4.0).unwrap(), ..DesignSettings::default() }
}

#[test]
fn req_vc_013_a_document_has_the_settings_and_the_machine_the_filter_keeps() {
    let plain = document(4, &square(2, 0.0, 0.0, RED));
    let kept = keeping(&plain, &plugin_id(SETTINGS_FILTER), &kept_data(&settings(), &BROTHER_PE800_SMALL));
    let with = read(&kept, false);
    assert_eq!((with.design.settings, with.machine, with.warnings.len()), (settings(), Some(&BROTHER_PE800_SMALL), 0));
    assert_eq!(machine(&kept, &Budget::DEFAULT), Some(&BROTHER_PE800_SMALL));
    // A document the filter never ran on has the defaults and no machine of its own, and so has one that
    // keeps only another plug-in's data.
    let other = keeping(&plain, "org.example.plotter", &json!({"profile": REFERENCE.id, "collapse_len_mm": 10}));
    for document in [plain, other] {
        let without = read(&document, false);
        assert_eq!((without.design.settings, without.machine), (DesignSettings::default(), None));
        assert_eq!(machine(&document, &Budget::DEFAULT), None);
    }
    // Nor is there a machine in what doesn't read as a document.
    assert_eq!(machine(b"not a document", &Budget::DEFAULT), None);
}

#[test]
fn req_vc_013_a_document_whose_plug_in_data_is_damaged_still_reads() {
    // VectorCraft keeps what filters keep under `pluginData`, by plug-in id. A damaged file may hold
    // anything there, which VectorCraft opens, and reads as nothing kept.
    let mut file: Value = serde_json::from_slice(&document(4, &square(2, 0.0, 0.0, RED))).unwrap();
    for damaged in [json!(5), json!(["a"]), json!("x"), Value::Null] {
        file["document"]["unknown"] = json!({"pluginData": damaged});
        let bytes = serde_json::to_vec(&file).unwrap();
        let read = read(&bytes, false);
        assert_eq!((read.design.settings, read.machine, read.warnings.len()), (DesignSettings::default(), None, 0), "{damaged}");
        assert_eq!(machine(&bytes, &Budget::DEFAULT), None);
    }
}

#[test]
fn req_vc_013_automatic_digitizing_goes_by_the_settings_kept() {
    // A square's stroke, 3 mm wide and drawn inside it: by default a satin column, which is centred on its
    // path, and said so. With the narrowest satin stroke kept at 4 mm, a running stitch.
    let width = 3.0 / MM_PER_POINT;
    let stroke =
        format!(r#"{{"kind": "stroke", "width": {width}, "align": "Inside", "paint": {{"type": "solid", "color": {{"model": "gray", "k": 1}}}}}}"#);
    let plain = document(4, &square(2, 0.0, 0.0, &format!(r#"{{"items": [{stroke}]}}"#)));
    let said = |document: &[u8]| read(document, true).warnings.iter().map(|w| w.code).collect::<Vec<Code>>();
    assert_eq!(said(&plain), [Code::VcFeatureIgnored]);
    assert_eq!(said(&keeping(&plain, &plugin_id(SETTINGS_FILTER), &kept_data(&settings(), REFERENCE))), Vec::<Code>::new());
}

#[test]
fn diag_sc_w0812_what_the_filter_keeps_that_does_not_read_is_named_after_the_rest() {
    let plain = document(4, &square(2, 0.0, 0.0, RED));
    let kept = keeping(&plain, &plugin_id(SETTINGS_FILTER), &json!({"profile": "singer", "min_satin_stroke_width_mm": "wide"}));
    let read = read(&kept, false);
    assert_eq!((read.design.settings, read.machine), (DesignSettings::default(), None));
    let said: Vec<String> = read.warnings.iter().map(|w| w.to_string()).collect();
    assert_eq!(
        said,
        [
            "warning SC-W0812: The design setting `min_satin_stroke_width_mm` kept with the document, \"wide\", is not a length of 0 or more, so it keeps its default.",
            "warning SC-W0812: The machine kept with the document, \"singer\", is not one this StitchCraft has, so the design is planned for the machine `--profile` or the export names, or else for brother-pe800-5x7.",
        ]
    );
    assert_eq!(machine(&kept, &Budget::DEFAULT), None);
}
