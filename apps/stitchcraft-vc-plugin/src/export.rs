//! What an exporter does: the document VectorCraft gives it, sewn into a machine file as `stitch export`
//! sews it (`REQ-VC-011`).
//!
//! From 0.14, VectorCraft lists an exporter plug-in in File › Export › Export As by its name, and writes a
//! file with the extension it lists (VectorCraft's `docs/plugins.md`, Exporters). It gives the exporter the
//! document as a native `.vectorcraft` file's JSON, without the data of its larger images, and the
//! parameters of its dialog, with the extension and the name of the file. The exporter returns the file in
//! base64, with what the export left out or changed, for the user, or declines with a message, and then
//! VectorCraft writes nothing.
//!
//! **As `stitch export` sews it.** The exporter reads the document with the VectorCraft adapter, plans it
//! with the engine for the machine its `profile` names, writes it in its format with the file's name as the
//! design name, and reads the file back, which catches a writer's bug before the file is sewn (`SC-E0009`).
//! `stitch export` does each with the same function and the same budget (`sew` in
//! `apps/stitchcraft-cli/src/commands/plan.rs`), so the file is the one `stitch export` writes of the
//! document, which the compatibility gate checks in VectorCraft (Level B). `auto` digitizes the shapes
//! without settings, as `--auto` does.
//!
//! **What it says.** Everything said, the reader's warnings first, then the engine's and the format's, is
//! the export's warnings, one line each ([`Diagnostic::one_line`]), as `stitch export` prints them.
//! VectorCraft shows at most 100 ([`MAX_WARNINGS`]), so when there are more the last says how many more
//! `stitch export` lists. It shows 500 characters of each ([`MAX_WARNING_CHARS`]) and cuts a longer one
//! in the middle of a word, so a longer line ends at its last whole sentence that fits. `SC-W0913`, the
//! longest, is written to fit with its hint (`stitchcraft_engine::auto::thick`).
//!
//! **Errors.** When anything said is an error, the exporter writes nothing and declines with the first
//! error, as `stitch export` writes nothing (`REQ-CLI-003`). VectorCraft shows the first 300 characters of
//! a decline, so the number of errors comes first when there are more.
//!
//! **The budget.** `stitch export`'s own ([`Budget::DEFAULT`]), so that the file is the same. VectorCraft
//! lets an export run 10 billion instructions, and 4,000 more for each byte of the document, and stops one
//! that needs more with its own error, `plug-in exceeded its instruction budget`. `stitch export` writes
//! that design. The contract tests measure what a fill of the reference hoop's whole field takes
//! (`compat/vc-contract`).
//!
//! **The picture.** VectorCraft stops the export of a traced photo, which needs about 5 times the
//! instructions an export is allowed. So the browser demo's page runs this module itself, where nothing
//! counts instructions, and it also asks for what `stitch export --preview` draws: with [`PICTURE`] as the
//! `mode` of its `_context`, the exporter answers the picture of the file read back, a PNG, in place of the
//! file. VectorCraft's own mode is `export`, so File › Export never gets a picture
//! (`docs/src/design/vectorcraft-integration.md`, An export that finishes).

use serde::{Deserialize, Serialize};
use stitchcraft_core::{Budget, Code, Diagnostic, Severity};
use stitchcraft_engine::auto::{AutoParams, MAX_SATIN_WIDTH};
use stitchcraft_engine::design::{DesignSettings, Reading};
use stitchcraft_formats::{Encoded, read_back};
use stitchcraft_params::find;
use stitchcraft_plan::profiles::{self, BUILTIN, REFERENCE};
use stitchcraft_plan::{FormatId, MachineProfile, StitchPlan, TrimSupport};
use stitchcraft_render::{Settings, Style};
use stitchcraft_vectorcraft::{AUTO_BY_DEFAULT, base64};

use crate::dialog::own;
use crate::effect::RunError;
use crate::manifest::{Param, Schema};

/// The parameter that digitizes the shapes without settings automatically, as `stitch export --auto`
/// does.
pub const AUTO: &str = "auto";
/// The parameter that names the machine profile the design is planned for, as `stitch export --profile`
/// does, and as the Design Settings filter keeps it with the document.
pub use stitchcraft_vectorcraft::settings::PROFILE;
/// The option of an exporter's [`PROFILE`] that plans the document for the machine it keeps, which the
/// Design Settings filter sets, or for the reference machine when it keeps none (`REQ-VC-013`).
pub const DOCUMENT: &str = "document";
/// The `mode` of an export's `_context` that asks for the picture `stitch export --preview` draws of the
/// file, in place of the file. VectorCraft's own exports say `export`, and the browser demo's page says this
/// one when it runs the module itself.
pub const PICTURE: &str = "picture";
/// The most warnings VectorCraft shows of an export (`MAX_WARNINGS` in its `crates/plugins/src/export.rs`).
pub const MAX_WARNINGS: usize = 100;
/// The most characters VectorCraft shows of one warning, which the engine keeps its longest warning to.
pub const MAX_WARNING_CHARS: usize = stitchcraft_engine::auto::WARNING_CHARS;

/// An exporter's dialog: [`AUTO`], then the machine's [`PROFILE`]: [`DOCUMENT`], by default, or one of the
/// built-in profiles.
pub(crate) fn options() -> Vec<(&'static str, Param)> {
    let machines = std::iter::once(DOCUMENT).chain(BUILTIN.iter().map(|profile| profile.id)).collect();
    vec![
        (AUTO, own(Schema::Bool { default: AUTO_BY_DEFAULT }, "Digitize automatically", &auto_help(), AUTO_PAGE)),
        (PROFILE, own(Schema::Choice { options: machines, default: DOCUMENT }, "Machine", &profile_help(), PROFILE_PAGE)),
    ]
}

/// What [`AUTO`] does, for its help, with the widths automatic digitizing goes by: the narrowest satin stroke,
/// and the default of the widest satin column, which an object without effects takes.
fn auto_help() -> String {
    let widest = find(&[&AutoParams::GROUP], MAX_SATIN_WIDTH).map_or("", |spec| spec.default);
    format!(
        "Chooses the stitches of each object without a StitchCraft effect by its width, as `stitch export --auto` does: a line up \
         to {} mm wide runs, a line up to {widest} mm wide is a satin column as wide as it, and a wider line or an area is a \
         tatami fill. Objects with a StitchCraft effect sew as their effects say. Off, the objects without one sew as drawn, as \
         `stitch export --no-auto` sews them.",
        DesignSettings::MIN_SATIN_STROKE_WIDTH.get(),
    )
}

/// Whether an exporter digitizes automatically when the host passes no [`AUTO`]: the default of every host.
const fn auto_by_default() -> bool {
    AUTO_BY_DEFAULT
}

/// The page that says how objects without a StitchCraft effect are digitized.
const AUTO_PAGE: &str = "user/how-to/embroider-a-vectorcraft-design.html#choose-each-objects-stitches";

/// What a machine profile is, for the help of a dialog that names one.
pub(crate) const MACHINE_HELP: &str =
    "The machine and hoop the design is planned for: the design must fit its hoop, and its stitches its longest and shortest stitch.";

/// What [`PROFILE`] does, for its help.
fn profile_help() -> String {
    format!(
        "{MACHINE_HELP} `{DOCUMENT}` is the machine the document keeps, which Object › Plug-ins › StitchCraft Design Settings sets, or \
         {} when it keeps none, as `stitch export` plans it without `--profile`. `stitch profiles` lists the others.",
        REFERENCE.id
    )
}

/// The page of the built-in machine profiles.
pub(crate) const PROFILE_PAGE: &str = "user/reference/profiles.html";

/// The parameters VectorCraft passes an exporter: its dialog's, by the names of [`AUTO`] and [`PROFILE`],
/// and the file's.
#[derive(Debug, Deserialize)]
struct Params {
    /// On unless the host turns it off ([`AUTO_BY_DEFAULT`]).
    #[serde(default = "auto_by_default")]
    auto: bool,
    #[serde(default)]
    profile: Option<String>,
    #[serde(rename = "_context", default)]
    context: Context,
}

/// What VectorCraft says of the file an export writes.
#[derive(Debug, Default, Deserialize)]
struct Context {
    /// Its name, without the extension: the document's when the export is given no path.
    #[serde(default)]
    name: String,
    /// What is asked for: VectorCraft's `export`, the file, or [`PICTURE`].
    #[serde(default)]
    mode: String,
}

/// What an exporter returns: the file in base64, and what the export left out or changed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Exported {
    data_base64: String,
    warnings: Vec<String>,
}

/// An exporter's decline: the message VectorCraft shows instead of writing the file.
#[derive(Serialize)]
struct Declined {
    error: String,
}

/// The exporter of `format`: the document VectorCraft gives it, `input`, sewn with the parameters
/// `params` into a file of `format` as `stitch export` sews it, or declined with the first error said.
/// Parameters that aren't the JSON VectorCraft's checks let through, such as a profile its manifest
/// doesn't offer, are [`RunError::BadInput`].
pub fn export(format: FormatId, input: &[u8], params: &[u8]) -> Result<Vec<u8>, RunError> {
    let params: Params = serde_json::from_slice(params).map_err(|_| RunError::BadInput)?;
    let asked = match params.profile.as_deref() {
        None | Some(DOCUMENT) => None,
        Some(id) => Some(profiles::find(id).ok_or(RunError::BadInput)?),
    };
    let wanted = Wanted { name: &params.context.name, picture: params.context.mode == PICTURE };
    let out = match sewn(format, input, Reading { auto: params.auto, marked_as_drawn: params.auto, ..Reading::default() }, asked, wanted) {
        Ok((file, said)) => serde_json::to_vec(&Exported { data_base64: base64(&file), warnings: warnings(&said) }),
        Err(said) => serde_json::to_vec(&Declined { error: decline(&said) }),
    };
    out.map_err(|_| RunError::Encode)
}

/// What an export is asked for: the file under its name, or the picture of that file.
#[derive(Clone, Copy)]
struct Wanted<'a> {
    /// The design's name in the file.
    name: &'a str,
    /// Whether the picture `stitch export --preview` draws is wanted in place of the file ([`PICTURE`]).
    picture: bool,
}

/// The file of `format` that `stitch export` writes of the document `input`, read as `reading` asks and
/// planned for the machine `asked`, or else the one the document keeps, or else the reference machine, or
/// the picture of that file, as `wanted` says, with everything said; or everything said, which then holds
/// an error and leaves the file unwritten.
fn sewn(
    format: FormatId,
    input: &[u8],
    reading: Reading,
    asked: Option<&'static MachineProfile>,
    wanted: Wanted<'_>,
) -> Result<(Vec<u8>, Vec<Diagnostic>), Vec<Diagnostic>> {
    let read = stitchcraft_vectorcraft::read_in(input, &Budget::DEFAULT, reading, &mut crate::fonts::built_in()).map_err(|unread| vec![unread])?;
    let profile = asked.or(read.machine).unwrap_or(REFERENCE);
    // Planned for the format it is written in, whose limits the file then keeps.
    let outcome = stitchcraft_engine::plan(&read.design, &profile.writing(format), &Budget::DEFAULT);
    let mut said = read.warnings;
    said.extend(outcome.diagnostics);
    let Some(plan) = outcome.plan else { return Err(with_an_error(said)) };
    written(&plan, format, wanted.name, said, wanted.picture.then_some(profile.trims))
}

/// The file of `format`, named `name`, that `plan` is written as, every thread of it, those of an element
/// sewn in several among them (`REQ-THREAD-003`), and read back, with `said` and what writing it says; or
/// everything said, which then holds an error and leaves the file unwritten. With `picture`, how the
/// machine cuts, the file read back is drawn as `stitch export --preview` draws it, and the picture, a PNG,
/// is given in the file's place.
fn written(
    plan: &StitchPlan,
    format: FormatId,
    name: &str,
    mut said: Vec<Diagnostic>,
    picture: Option<TrimSupport>,
) -> Result<(Vec<u8>, Vec<Diagnostic>), Vec<Diagnostic>> {
    let file = match stitchcraft_formats::encode(plan, format, name) {
        Ok(Encoded { bytes, notes }) => {
            said.extend(notes);
            bytes
        }
        Err(e) => {
            said.push(e.diagnostic());
            return Err(said);
        }
    };
    // The picture is of the file read back, so a file that can't be read back has none.
    let out = match read_back(&file, plan).and_then(|back| picture.map(|trims| drawn(&back, trims)).transpose()) {
        Ok(png) => png.unwrap_or(file),
        Err(bug) => {
            said.push(bug);
            file
        }
    };
    if said.iter().any(is_error) { Err(said) } else { Ok((out, said)) }
}

/// The picture `stitch export --preview` draws of the file read back, `back`, on a machine that cuts as
/// `trims` says: the realistic style at the default scale, as a PNG file (`picture` in
/// `apps/stitchcraft-cli/src/commands/plan.rs`).
fn drawn(back: &StitchPlan, trims: TrimSupport) -> Result<Vec<u8>, Diagnostic> {
    let settings = Settings::new(Style::Realistic, Settings::DEFAULT_SCALE)
        .ok_or(Diagnostic::new(Code::InternalCheckFailed, "The picture's default settings are out of range."))?;
    stitchcraft_render::preview(back, trims, settings, &mut Budget::DEFAULT.meter()).map(|image| image.png).map_err(|e| e.diagnostic())
}

/// `said`, which came with no plan, with an error that says so when none of it does: the engine always
/// says why it gives no plan, and a plan given without a word is a bug in StitchCraft.
fn with_an_error(mut said: Vec<Diagnostic>) -> Vec<Diagnostic> {
    if !said.iter().any(is_error) {
        said.push(Diagnostic::new(Code::InternalCheckFailed, "The engine gave no plan and said no error."));
    }
    said
}

/// Whether `d` is an error, which leaves the file unwritten.
fn is_error(d: &Diagnostic) -> bool {
    d.severity() == Severity::Error
}

/// What an export says, one line each: everything `said`, or when it is more than VectorCraft shows, all
/// but the last that fit and then how many more there are. Each line is as long as VectorCraft shows at
/// most (`shown`).
pub fn warnings(said: &[Diagnostic]) -> Vec<String> {
    let line = |d: &Diagnostic| shown(d.one_line());
    if said.len() <= MAX_WARNINGS {
        return said.iter().map(line).collect();
    }
    let fit = MAX_WARNINGS - 1;
    let more = format!("{} more, which `stitch export` lists.", said.len() - fit);
    said.iter().take(fit).map(line).chain([more]).collect()
}

/// `line` as VectorCraft shows it whole: itself when it has [`MAX_WARNING_CHARS`] characters or fewer, and
/// else its sentences that fit, a sentence ending at a full stop before a space. A line whose first
/// sentence doesn't fit is cut one character short and ends with an ellipsis.
fn shown(line: String) -> String {
    // Where the character after the last one shown starts, if there is one.
    let Some((end, _)) = line.char_indices().nth(MAX_WARNING_CHARS) else {
        return line;
    };
    let fits = line.get(..end).unwrap_or(&line);
    // The full stop of a sentence's end is shown, and the space after it may be the first character cut.
    let stop = line.match_indices(". ").map(|(at, _)| at + 1).take_while(|at| *at <= end).last();
    match stop {
        Some(stop) => line.get(..stop).unwrap_or(fits).to_string(),
        None => fits.chars().take(MAX_WARNING_CHARS - 1).chain(['…']).collect(),
    }
}

/// The message an export declines with: the first error `said`, after the number of errors when there
/// are more. VectorCraft shows a declined export's message alone, without its warnings. So when a text was
/// left out of its embroidery font (`SC-W1111`), the first such warning follows the error: a document of
/// one text whose font isn't built in has nothing to stitch, and the warning is what says why.
fn decline(said: &[Diagnostic]) -> String {
    let errors: Vec<&Diagnostic> = said.iter().filter(|d| is_error(d)).collect();
    let unlettered = said.iter().find(|d| d.code == Code::TextNotLettered).map(|d| format!(" A text was left out: {}", d.one_line()));
    let unlettered = unlettered.unwrap_or_default();
    match errors.as_slice() {
        [] => "StitchCraft wrote nothing, and said no error.".to_string(),
        [one] => format!("{}{unlettered}", one.one_line()),
        [first, ..] => format!("{} errors stop the export. The first is {}{unlettered}", errors.len(), first.one_line()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use stitchcraft_core::{Fix, Mm};
    use stitchcraft_engine::design::DesignSettings;
    use stitchcraft_plan::profiles::BROTHER_PE800_SMALL;
    use stitchcraft_vectorcraft::settings::SETTINGS_FILTER;

    use super::*;
    use crate::testing::PATCH;

    /// A VectorCraft document whose one layer holds `objects`.
    fn document(objects: &Value) -> Vec<u8> {
        let layer = json!({"id": 1, "kind": {"type": "layer", "color": {"Preset": 0}, "children": objects}});
        serde_json::to_vec(
            &json!({"format": "vectorcraft", "version": 4, "document": {"version": 1, "artboards": [], "next_id": 99, "layers": [layer]}}),
        )
        .unwrap()
    }

    /// A black line 50 mm long and `mm` wide with the StitchCraft `effects`, the object with id `id`.
    fn line(id: u64, mm: f64, effects: &Value) -> Value {
        let black = json!({"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 0}});
        let stroke = json!({"kind": "stroke", "width": mm * 72.0 / 25.4, "paint": black});
        let path = json!({"subpaths": [{"anchors": [{"p": [0, 0]}, {"p": [141.73, 0]}]}]});
        json!({"id": id, "appearance": {"items": [stroke], "effects": effects}, "kind": {"type": "path", "path": path}})
    }

    /// The parameters of an export to a file named `name`, with `set`.
    fn params(name: &str, set: &Value) -> Vec<u8> {
        let mut params = set.clone();
        params["_context"] = json!({"mode": "export", "extension": "pes", "name": name});
        serde_json::to_vec(&params).unwrap()
    }

    /// What an export of `input` to `format` with `params` returns, read.
    fn exported(format: FormatId, input: &[u8], params: &[u8]) -> Value {
        serde_json::from_slice(&export(format, input, params).unwrap()).unwrap()
    }

    /// What `stitch export` writes of `input` as a file of `format` named `name`, for `profile`, and says,
    /// one line each: its reader, its engine and its format writer called as `sew` calls them in
    /// `apps/stitchcraft-cli/src/commands/plan.rs`.
    fn stitch_export(input: &[u8], reading: Reading, profile: &MachineProfile, format: FormatId, name: &str) -> (Vec<u8>, Vec<String>) {
        let read = stitchcraft_vectorcraft::read_with(input, &Budget::DEFAULT, reading).unwrap();
        let outcome = stitchcraft_engine::plan(&read.design, profile, &Budget::DEFAULT);
        let Encoded { bytes, notes } = stitchcraft_formats::encode(outcome.plan.as_ref().unwrap(), format, name).unwrap();
        let said: Vec<Diagnostic> = read.warnings.into_iter().chain(outcome.diagnostics).chain(notes).collect();
        assert!(!said.iter().any(is_error), "{said:?}");
        (bytes, said.iter().map(Diagnostic::one_line).collect())
    }

    /// The file and the warnings an export returned, or what it declined with.
    fn file(out: &Value) -> (String, Vec<String>) {
        let data = out["dataBase64"].as_str().unwrap_or_else(|| panic!("no file: {out}"));
        let warnings = out["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap().to_string()).collect();
        (data.to_string(), warnings)
    }

    #[test]
    fn req_vc_011_an_exporter_writes_the_file_stitch_export_writes_and_says_what_it_says() {
        for format in FormatId::ALL {
            let (bytes, said) = stitch_export(PATCH, Reading::default(), REFERENCE, *format, "patch");
            assert_eq!(file(&exported(*format, PATCH, &params("patch", &json!({})))), (base64(&bytes), said.clone()), "{format:?}");
            // The patch's placement line and its group's paint, each naming its object, and the hint after it.
            // Its start marker is used, and says nothing.
            assert_eq!(said.len(), 2, "{format:?}: {said:?}");
            let line = "info SC-I0811 (vc:10): The object \"stitch:ignore placement line\" is named stitch:ignore, and is left out with \
                        everything in it. Rename it to stitch it.";
            assert_eq!(said.first().map(String::as_str), Some(line));
        }
        // The design is named as VectorCraft names the file.
        let dst = exported(FormatId::Dst, PATCH, &params("Poster", &json!({})));
        let (poster, _) = stitch_export(PATCH, Reading::default(), REFERENCE, FormatId::Dst, "Poster");
        assert!(poster.starts_with(b"LA:Poster          "));
        assert_eq!(dst["dataBase64"].as_str(), Some(base64(&poster).as_str()), "{dst}");
    }

    #[test]
    fn req_vc_011_auto_digitizes_the_shapes_without_settings_as_auto_does() {
        // A line 3 mm wide without a StitchCraft effect runs, and digitized automatically is a satin column.
        let input = document(&json!([line(2, 3.0, &json!([]))]));
        let set = |auto: bool| params("line", &json!({ AUTO: auto }));
        let (plain, auto) = (exported(FormatId::PesV1, &input, &set(false)), exported(FormatId::PesV1, &input, &set(true)));
        assert_eq!(file(&plain), {
            let (bytes, said) = stitch_export(&input, Reading::default(), REFERENCE, FormatId::PesV1, "line");
            (base64(&bytes), said)
        });
        let (bytes, said) = stitch_export(&input, Reading { auto: true, ..Reading::default() }, REFERENCE, FormatId::PesV1, "line");
        assert_eq!(file(&auto), (base64(&bytes), said));
        // The shape is named as the Layers panel shows it (`REQ-AUTO-127`).
        let rule = "info SC-I0912 (\"Column 1, Black\", vc:2:stroke): This shape sews by the rule column (3 mm wide).";
        assert_eq!(file(&auto).1, ["info SC-I0901: 1 shape is digitized automatically: it sews as a satin column.", rule]);
        assert_ne!(file(&auto).0, file(&plain).0);
    }

    #[test]
    fn req_vc_036_the_exporters_digitize_automatically_by_default() {
        // The dialog's option is on, and a host that passes no `auto` gets it on too.
        let (_, option) = options().into_iter().find(|(id, _)| *id == AUTO).unwrap();
        assert_eq!(option.schema, Schema::Bool { default: true });
        let input = document(&json!([line(2, 3.0, &json!([]))]));
        let automatic = {
            let (bytes, said) = stitch_export(&input, Reading { auto: true, ..Reading::default() }, REFERENCE, FormatId::PesV1, "line");
            (base64(&bytes), said)
        };
        assert_eq!(file(&exported(FormatId::PesV1, &input, &params("line", &json!({})))), automatic);
        // Off, it is what `stitch export --no-auto` writes: the shapes as drawn.
        let plain = {
            let (bytes, said) = stitch_export(&input, Reading::default(), REFERENCE, FormatId::PesV1, "line");
            (base64(&bytes), said)
        };
        assert_eq!(file(&exported(FormatId::PesV1, &input, &params("line", &json!({ AUTO: false })))), plain);
        // On, a shape grouped with a start marker is set by hand, as `stitch export` reads it by default
        // (`REQ-AUTO-166`): the patch's Background, without its Fill effect.
        let mut patch: Value = serde_json::from_slice(PATCH).unwrap();
        let background = patch.pointer_mut("/document/layers/0/kind/children/1/kind/children/0").unwrap();
        assert_eq!(background["name"], json!("Background"));
        background["appearance"]["effects"] = json!([]);
        let patch = serde_json::to_vec(&patch).unwrap();
        let sewn = |marked_as_drawn| {
            let (bytes, said) =
                stitch_export(&patch, Reading { auto: true, marked_as_drawn, ..Reading::default() }, REFERENCE, FormatId::PesV1, "patch");
            (base64(&bytes), said)
        };
        let exported = file(&exported(FormatId::PesV1, &patch, &params("patch", &json!({}))));
        assert_eq!(exported, sewn(true));
        assert_ne!(exported, sewn(false));
    }

    /// The parameters of a run that asks for the picture of the file named `name`, with `set`.
    fn picture_params(name: &str, set: &Value) -> Vec<u8> {
        let mut params = set.clone();
        params["_context"] = json!({"mode": PICTURE, "extension": "pes", "name": name});
        serde_json::to_vec(&params).unwrap()
    }

    #[test]
    fn req_vc_025_an_exporter_asked_for_the_picture_answers_what_stitch_export_preview_draws() {
        for format in FormatId::ALL {
            let (bytes, said) = stitch_export(PATCH, Reading::default(), REFERENCE, *format, "patch");
            // As `stitch export --preview` draws it: the file read back, in the realistic style at the default scale.
            let read = stitchcraft_vectorcraft::read_with(PATCH, &Budget::DEFAULT, Reading::default()).unwrap();
            let plan = stitchcraft_engine::plan(&read.design, &REFERENCE.writing(*format), &Budget::DEFAULT).plan.unwrap();
            let settings = Settings::new(Style::Realistic, Settings::DEFAULT_SCALE).unwrap();
            let image =
                stitchcraft_render::preview(&read_back(&bytes, &plan).unwrap(), REFERENCE.trims, settings, &mut Budget::DEFAULT.meter()).unwrap();
            assert!(image.png.starts_with(b"\x89PNG\r\n\x1a\n"), "{format:?}");
            // The picture comes in the file's place, with what the export says.
            assert_eq!(file(&exported(*format, PATCH, &picture_params("patch", &json!({})))), (base64(&image.png), said.clone()), "{format:?}");
            // VectorCraft's own mode, and a run that names none, give the file.
            assert_eq!(file(&exported(*format, PATCH, &params("patch", &json!({})))).0, base64(&bytes), "{format:?}");
            let unnamed = serde_json::to_vec(&json!({"_context": {"name": "patch"}})).unwrap();
            assert_eq!(file(&exported(*format, PATCH, &unnamed)).0, base64(&bytes), "{format:?}");
        }
    }

    #[test]
    fn req_vc_025_a_design_that_writes_no_file_has_no_picture() {
        // The patch doesn't fit the small hoop, so nothing is written, and nothing is drawn.
        let small = exported(FormatId::PesV1, PATCH, &picture_params("patch", &json!({ PROFILE: BROTHER_PE800_SMALL.id })));
        assert!(small.get("dataBase64").is_none(), "{small}");
        assert!(small["error"].as_str().is_some_and(|error| error.starts_with("error SC-E0701: ")), "{small}");
    }

    #[test]
    fn the_options_help_says_what_they_do_with_the_widths_auto_goes_by() {
        let options = options();
        let help = |key: &str| options.iter().find(|(k, _)| *k == key).and_then(|(_, param)| param.ui.help.clone()).unwrap_or_default();
        let auto = help(AUTO);
        assert!(auto.contains("a line up to 1 mm wide runs, a line up to 7 mm wide is a satin column"), "{auto}");
        let profile = help(PROFILE);
        assert!(profile.contains(&format!("{DOCUMENT} is the machine the document keeps")) && profile.contains(REFERENCE.id), "{profile}");
        assert_eq!(options.iter().map(|(key, _)| *key).collect::<Vec<_>>(), [AUTO, PROFILE]);
    }

    #[test]
    fn req_vc_011_the_design_is_planned_for_the_machine_its_profile_names() {
        // The patch is 50 × 54 mm: it fits the reference machine's 5 × 7 in hoop, and not its small hoop.
        let on = |profile: &str| exported(FormatId::PesV1, PATCH, &params("patch", &json!({ PROFILE: profile })));
        assert_eq!(on(REFERENCE.id), exported(FormatId::PesV1, PATCH, &params("patch", &json!({}))));
        let small = on(BROTHER_PE800_SMALL.id);
        assert!(small.get("dataBase64").is_none(), "{small}");
        assert!(small["error"].as_str().is_some_and(|error| error.starts_with("error SC-E0701: ")), "{small}");
        // A profile the manifest doesn't offer, which VectorCraft refuses before the export runs.
        assert_eq!(export(FormatId::PesV1, PATCH, &params("patch", &json!({ PROFILE: "singer" }))), Err(RunError::BadInput));
    }

    #[test]
    fn req_vc_013_the_document_s_machine_is_the_export_s_unless_its_dialog_names_another() {
        // The patch kept with the small hoop, which it doesn't fit, and with a shortest jump of 10 mm.
        let mut file: Value = serde_json::from_slice(stitchcraft_vectorcraft::read_part(PATCH)).unwrap();
        let settings = DesignSettings { collapse_len: Mm::new(10.0).unwrap(), ..DesignSettings::default() };
        let kept = stitchcraft_vectorcraft::settings::kept_data(&settings, &BROTHER_PE800_SMALL);
        file["document"]["unknown"] = json!({"pluginData": {stitchcraft_vectorcraft::plugin_id(SETTINGS_FILTER): kept}});
        let kept = serde_json::to_vec(&file).unwrap();
        let on = |profile: &Value| exported(FormatId::PesV1, &kept, &params("patch", &json!({ PROFILE: profile })));
        for asked in [json!(DOCUMENT), Value::Null] {
            assert!(on(&asked)["error"].as_str().is_some_and(|error| error.starts_with("error SC-E0701: ")), "{asked}");
        }
        // Named, the reference machine: as `stitch export --profile` plans the document, with its settings.
        let (bytes, _) = stitch_export(&kept, Reading::default(), REFERENCE, FormatId::PesV1, "patch");
        assert_eq!(on(&json!(REFERENCE.id))["dataBase64"], json!(base64(&bytes)));
        let (plain, _) = stitch_export(PATCH, Reading::default(), REFERENCE, FormatId::PesV1, "patch");
        assert_ne!(bytes, plain, "the shortest jump the document keeps sews the patch otherwise");
    }

    #[test]
    fn req_vc_011_an_error_writes_nothing_and_the_export_says_the_first() {
        let error = |input: &[u8]| exported(FormatId::Dst, input, &params("x", &json!({})))["error"].as_str().map(str::to_string);
        // A document with nothing to stitch, and a file that isn't one.
        let nothing = document(&json!([]));
        assert_eq!(
            error(&nothing).as_deref(),
            Some("error SC-E0010: The design has nothing to stitch: it has no elements, or every one was skipped.")
        );
        assert!(error(b"<svg/>").is_some_and(|error| error.starts_with("error SC-E0806: ")));
        // 2 satin columns of a method this version doesn't know, which leave nothing to stitch: 3 errors.
        let unknown = json!([{"id": "plugin.dev.stitchcraft.satin", "params": {"satin_method": "plaid"}}]);
        let error = error(&document(&json!([line(2, 3.0, &unknown), line(3, 3.0, &unknown)]))).unwrap();
        assert!(error.starts_with("3 errors stop the export. The first is error SC-E0101 (vc:2:satin): "), "{error}");
    }

    #[test]
    fn req_vc_011_vectorcraft_shows_a_hundred_warnings_and_the_last_counts_the_rest() {
        let said =
            |n: usize| -> Vec<Diagnostic> { (0..n).map(|i| Diagnostic::new(Code::FillPartsApart, format!("Fill {i} is in parts."))).collect() };
        assert_eq!(warnings(&said(MAX_WARNINGS)).len(), MAX_WARNINGS);
        assert_eq!(warnings(&said(MAX_WARNINGS)).last().map(String::as_str), Some("warning SC-W0307: Fill 99 is in parts."));
        let shown = warnings(&said(150));
        assert_eq!(shown.len(), MAX_WARNINGS);
        assert_eq!(shown.get(98).map(String::as_str), Some("warning SC-W0307: Fill 98 is in parts."));
        assert_eq!(shown.last().map(String::as_str), Some("51 more, which `stitch export` lists."));
        // Each on one line, its hint after it.
        let hinted = Diagnostic::new(Code::FillPartsApart, "In parts.").with_fix(Fix::Hint("Break it apart.".into()));
        assert_eq!(warnings(&[hinted]), ["warning SC-W0307: In parts. Break it apart."]);
    }

    #[test]
    fn req_vc_011_a_warning_longer_than_vectorcraft_shows_ends_at_a_whole_sentence() {
        let one = |message: String| warnings(&[Diagnostic::new(Code::FillPartsApart, message)]).pop().unwrap();
        let head = "warning SC-W0307: ";
        // As long as VectorCraft shows, a line is whole. One character more, and its last sentence goes.
        let fits = format!("{}. {}.", "a".repeat(300), "b".repeat(MAX_WARNING_CHARS - head.len() - 303));
        assert_eq!(one(fits.clone()).chars().count(), MAX_WARNING_CHARS);
        assert_eq!(one(fits.clone()), format!("{head}{fits}"));
        let longer = format!("{}. {}.", "a".repeat(300), "b".repeat(MAX_WARNING_CHARS - head.len() - 302));
        assert_eq!(one(longer), format!("{head}{}.", "a".repeat(300)));
        // A full stop inside a number ends no sentence, and characters are counted, not bytes.
        let squares = format!("At (0.5, 36.3) mm: {}. {}.", "²".repeat(400), "b".repeat(100));
        assert_eq!(one(squares), format!("{head}At (0.5, 36.3) mm: {}.", "²".repeat(400)));
        // The sentence that ends where the line may: it stays.
        let ends = format!("{}. more", "a".repeat(MAX_WARNING_CHARS - head.len() - 1));
        assert_eq!(one(ends), format!("{head}{}.", "a".repeat(MAX_WARNING_CHARS - head.len() - 1)));
        // Without a sentence that fits, the line is cut and says so with an ellipsis.
        let unbroken = one("c".repeat(600));
        assert_eq!(unbroken, format!("{head}{}…", "c".repeat(MAX_WARNING_CHARS - head.len() - 1)));
        assert_eq!(unbroken.chars().count(), MAX_WARNING_CHARS);
    }

    #[test]
    fn req_thread_003_an_exporter_writes_each_thread_of_an_element_sewn_in_3() {
        // One element in red, green and blue, as a gradient fill of 3 stops sews, planned by hand: the
        // exporter writes the plan a document of one would give.
        let plan = crate::testing::three_threads_plan();
        for format in FormatId::ALL {
            let (bytes, said) = written(&plan, *format, "gradient", Vec::new(), None).unwrap();
            assert_eq!(said, Vec::new(), "{format:?}");
            let back = stitchcraft_formats::decode(&bytes).unwrap().plan;
            assert_eq!((back.blocks.len(), back.stats().color_changes, back.stats().stops), (3, 2, 0), "{format:?}");
        }
        // A PES file names each thread by a Brother colour of its own.
        let (pes, _) = written(&plan, FormatId::PesV1, "gradient", Vec::new(), None).unwrap();
        let threads: Vec<_> = stitchcraft_formats::decode(&pes).unwrap().plan.blocks.iter().map(|block| block.thread.color).collect();
        assert!(threads[0] != threads[1] && threads[1] != threads[2] && threads[0] != threads[2], "{threads:?}");
        // What was said before goes on, and an error leaves the file unwritten.
        let said = vec![Diagnostic::new(Code::FillPartsApart, "In parts.")];
        assert_eq!(written(&plan, FormatId::Dst, "gradient", said.clone(), None).map(|(_, said)| said), Ok(said));
        let error = vec![Diagnostic::new(Code::NothingToStitch, "Nothing.")];
        assert_eq!(written(&plan, FormatId::Dst, "gradient", error.clone(), None), Err(error));
    }

    #[test]
    fn a_decline_says_how_many_errors_there_are_before_the_first() {
        let error = |message: &str| Diagnostic::new(Code::NothingToStitch, message);
        let warning = Diagnostic::new(Code::FillPartsApart, "In parts.");
        assert_eq!(decline(&[warning.clone(), error("First.")]), "error SC-E0010: First.");
        assert_eq!(decline(&[error("First."), warning, error("Second.")]), "2 errors stop the export. The first is error SC-E0010: First.");
        // A bug: no plan, and no error said.
        assert_eq!(with_an_error(Vec::new()).iter().map(|d| d.code).collect::<Vec<_>>(), [Code::InternalCheckFailed]);
        assert_eq!(with_an_error(vec![error("Why.")]).len(), 1);
        assert_eq!(decline(&[]), "StitchCraft wrote nothing, and said no error.");
    }

    #[test]
    fn parameters_that_are_not_what_vectorcraft_passes_are_bad_input() {
        for bad in [&b"oops"[..], br#"{"auto": "yes"}"#, br#"{"profile": 5}"#, br#"{"_context": {"name": 5}}"#] {
            assert_eq!(export(FormatId::PesV1, PATCH, bad), Err(RunError::BadInput), "{}", String::from_utf8_lossy(bad));
        }
        // VectorCraft always passes the file's name; without it, the design is unnamed.
        assert!(exported(FormatId::PesV1, PATCH, b"{}").get("dataBase64").is_some());
    }
}
