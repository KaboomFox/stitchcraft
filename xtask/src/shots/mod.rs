//! `cargo xtask shots [--check] [--vectorcraft]`: every documentation image is declared, reproducible and
//! current (`docs/src/design/docs-pipeline.md`).
//!
//! Each `[[shot]]` in `docs/shots.toml` says how its image is made. Without `--check` the images are
//! regenerated into `docs/src/images/generated/`; with `--check` they are regenerated in memory and
//! compared with the committed files, so a pull request that changes how something looks must also update
//! its pictures. A stale image's new version is written to `target/shots/` (CI keeps it), and a generated
//! file no shot declares is an error. `stitch` renders with `stitchcraft-render` either a test sheet, or a
//! design, an SVG file or a VectorCraft document, that the engine plans as `stitch plan` plans it, once per
//! panel with the panel's parameters set on every element, and must come out byte for byte the same.
//! `photo` shots are never regenerated; they must exist and link their sew-out report.
//!
//! `picture` shots are the pictures automatic digitizing starts from, which xtask draws itself
//! (`picture.rs`): a design's areas filled flat in their colours, or a photo drawn in code. They are
//! compared byte for byte too.
//!
//! `vectorcraft-render` and `vectorcraft-ui` shots are VectorCraft's own pictures, which only
//! `--vectorcraft` makes, on Linux with the pinned release (`vectorcraft.rs`): the `shots-ui` job's work.
//! Without it they must only be declared well and committed.
//!
//! A shot of a design with panels makes one image per panel, `<id>-1.png` and so on, and pages show them
//! in a figure that `cargo xtask docs` writes ([`crate::figures`]): side by side, or one under another
//! for pictures too wide to share the page's width.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use stitchcraft_cli::commands::plan::Source;
use stitchcraft_core::Budget;
use stitchcraft_engine::design::{Design, Element};
use stitchcraft_engine::testsheets;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Rgb, StitchPlan};
use stitchcraft_render::{Settings, Style};

use crate::compat::scenario::Step;
use crate::util::{self, Findings};

mod picture;
mod vectorcraft;

const SHOTS: &str = "docs/shots.toml";
/// Where generated images live; pages link `images/generated/<id>.png`.
pub const GENERATED: &str = "docs/src/images/generated";
const PHOTOS: &str = "docs/src/images/photos";
/// The kinds of file a photo may be.
const PHOTO_EXTENSIONS: [&str; 3] = ["jpg", "jpeg", "png"];
/// Where `--check` puts the new version of a stale image.
const STALE: &str = "target/shots";

#[derive(Deserialize, Default)]
struct ShotsFile {
    #[serde(default)]
    shot: Vec<Shot>,
}

/// One declared image.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shot {
    /// The image's name: `images/generated/<id>.png`, or `images/photos/<id>.<ext>`.
    pub id: String,
    /// How it is made: `stitch`, `picture`, `vectorcraft-render`, `vectorcraft-ui` or `photo`.
    pub kind: String,
    /// The alternative text; pages that show the image use it.
    #[serde(default)]
    pub alt: String,
    /// `stitch` shots: the test sheet to draw.
    #[serde(default)]
    pub sheet: Option<String>,
    /// `stitch` shots of a test sheet: only its elements whose ids start with one of these, planned alone, for
    /// a picture of part of the sheet. Without any, the whole sheet.
    #[serde(default)]
    pub elements: Vec<String>,
    /// `stitch` shots: or else a design of the repository, an SVG file or a VectorCraft document, planned
    /// by the engine for the reference machine. `picture` shots: a design whose areas are drawn flat.
    #[serde(default)]
    pub fixture: Option<String>,
    /// `picture` shots: or else a picture drawn in code, by its name, such as `pear`.
    #[serde(default)]
    pub picture: Option<String>,
    /// `stitch` shots of a design: one image per panel. Without panels, the design is drawn once, as it is.
    #[serde(default)]
    pub panels: Vec<Panel>,
    /// `stitch` shots of a design: whether it is read as `--auto` reads it, its shapes without settings
    /// digitized automatically.
    #[serde(default)]
    pub auto: bool,
    /// `stitch` shots of a design: the diagnostic codes the plan may report. Any other is an error, so a
    /// picture never shows a design the engine changed without saying so on the page. `picture` shots of a
    /// design: the codes its reader may report.
    #[serde(default)]
    pub allow: Vec<String>,
    /// `stitch` shots: `realistic` (the default) or `simple`.
    #[serde(default)]
    pub style: Option<String>,
    /// `stitch` shots: the colour of the fabric the design is drawn on, as `#rrggbb`: the style's own when
    /// absent, unbleached cotton or white (`REQ-RND-003`).
    #[serde(default)]
    pub fabric: Option<String>,
    /// `stitch` shots, and `picture` shots of a design: pixels per millimetre (the renderer's default when
    /// absent). `vectorcraft-render` shots: pixels per point, VectorCraft's `--scale` (1 when absent).
    #[serde(default)]
    pub scale: Option<f32>,
    /// `photo` shots: the sew-out report.
    #[serde(default)]
    pub report: String,
    /// Shots with panels: how a page lays them out.
    #[serde(default)]
    pub layout: Layout,
    /// VectorCraft shots: the plug-in's modules installed first, as `compat/scenarios/` names them.
    #[serde(default)]
    pub plugins: Vec<String>,
    /// VectorCraft shots: how `vectorcraft-cli` draws the document, in a Level B scenario's steps, without
    /// exports.
    #[serde(default)]
    pub steps: Vec<Step>,
    /// `vectorcraft-ui` shots: the menu bar's title and the rows pressed after it. The picture shows the
    /// menus open.
    #[serde(default)]
    pub menu: Vec<String>,
    /// `vectorcraft-ui` shots: the stitch family whose effect's row is pressed last. The picture shows its
    /// dialog, whose labels are checked against the module's parameters.
    #[serde(default)]
    pub dialog: Option<String>,
    /// `vectorcraft-render` shots: a file of the repository the document is kept as, for pages that sew
    /// it. `cargo xtask shots --vectorcraft` writes it while it sews otherwise than the document drawn.
    #[serde(default)]
    pub document: Option<String>,
}

/// How a page lays out a shot's panels.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    /// Side by side, a column each: for pictures narrow enough to share the page's width.
    #[default]
    Columns,
    /// One under another, each with its caption above it and as wide as the page.
    Rows,
}

/// One panel of a shot: what changes from the design as drawn, and the caption that says so.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Panel {
    /// Shown above the panel and added to its alternative text: "1.5 mm", say.
    pub caption: String,
    /// Parameters set on every element, by their Ink/Stitch keys.
    #[serde(default)]
    pub params: BTreeMap<String, String>,
}

impl Shot {
    /// The names of the images this shot makes, without `.png`: its id, or one per panel.
    pub fn images(&self) -> Vec<String> {
        if self.panels.is_empty() { vec![self.id.clone()] } else { (1..=self.panels.len()).map(|n| format!("{}-{n}", self.id)).collect() }
    }

    /// Whether `path` (absolute, under `root`) is one of this shot's images: a generated one in
    /// [`GENERATED`], or the photo in `images/photos/`.
    pub fn declares(&self, root: &Path, path: &Path) -> bool {
        match self.kind.as_str() {
            "photo" => PHOTO_EXTENSIONS.iter().any(|ext| root.join(PHOTOS).join(format!("{}.{ext}", self.id)) == path),
            _ => self.images().iter().any(|name| root.join(GENERATED).join(format!("{name}.png")) == path),
        }
    }
}

/// The declared shots, in file order.
pub fn load(root: &Path) -> Result<Vec<Shot>, String> {
    let file: ShotsFile = toml::from_str(&util::read(&root.join(SHOTS))?).map_err(|e| format!("{SHOTS}: {e}"))?;
    Ok(file.shot)
}

/// `cargo xtask shots`: the `stitch` and `photo` shots, or with `vectorcraft` VectorCraft's.
pub fn run(check_only: bool, vectorcraft: bool) -> Result<(), String> {
    let root = util::root();
    let shots = load(&root)?;
    let mut findings = Findings::default();
    let mut ids = std::collections::BTreeSet::new();
    for shot in &shots {
        if !ids.insert(shot.id.as_str()) {
            findings.error(format!("{SHOTS}: duplicate shot id `{}`", shot.id));
        }
        if shot.alt.trim().is_empty() {
            findings.error(format!("{SHOTS}: shot `{}` needs alt text", shot.id));
        }
        if vectorcraft {
            continue;
        }
        if matches!(shot.kind.as_str(), "photo" | "stitch" | picture::KIND) {
            for problem in foreign(shot) {
                findings.error(format!("shot `{}`: {problem}", shot.id));
            }
        }
        match shot.kind.as_str() {
            "photo" => {
                let found = PHOTO_EXTENSIONS.iter().any(|ext| root.join(PHOTOS).join(format!("{}.{ext}", shot.id)).is_file());
                if !found {
                    findings.error(format!("photo `{}` is missing from {PHOTOS}/", shot.id));
                }
                if !shot.report.starts_with("https://") {
                    findings.error(format!("photo `{}` must link its sew-out report (`report = \"https://…\"`)", shot.id));
                }
            }
            "stitch" => match stitch(&root, shot) {
                Ok(images) => {
                    for (name, png) in shot.images().iter().zip(&images) {
                        keep(&root, name, png, check_only, &mut findings, Same::Bytes);
                    }
                }
                Err(e) => findings.error(format!("shot `{}`: {e}", shot.id)),
            },
            picture::KIND => match picture::make(&root, shot) {
                Ok(png) => keep(&root, &shot.id, &png, check_only, &mut findings, Same::Bytes),
                Err(e) => findings.error(format!("shot `{}`: {e}", shot.id)),
            },
            vectorcraft::RENDER | vectorcraft::UI => vectorcraft::committed(&root, shot, &mut findings),
            other => findings.error(format!("shot `{}`: unknown kind `{other}`", shot.id)),
        }
    }
    if vectorcraft {
        vectorcraft::run(&root, &shots, check_only, &mut findings);
    }
    let images: std::collections::BTreeSet<String> = shots.iter().flat_map(Shot::images).collect();
    for path in util::files(&root.join(GENERATED), &["png"]) {
        let declared = path.file_stem().and_then(|s| s.to_str()).is_some_and(|stem| images.contains(stem));
        if declared {
            continue;
        }
        if check_only {
            findings.error(format!("{} is not declared in {SHOTS}: declare it or delete it", util::rel(&path)));
        } else if let Err(e) = std::fs::remove_file(&path) {
            findings.error(format!("{}: {e}", util::rel(&path)));
        }
    }
    let verb = if check_only { "current" } else { "regenerated" };
    findings.finish("shots", &format!("{} shots declared, all {verb}", shots.len()))
}

/// What `shot`, a `stitch`, `picture` or `photo` shot, sets that only a VectorCraft shot reads, or only a
/// picture shot.
fn foreign(shot: &Shot) -> Vec<String> {
    let mut found = Vec::new();
    let vectorcraft = !shot.plugins.is_empty() || !shot.steps.is_empty() || !shot.menu.is_empty() || shot.dialog.is_some() || shot.document.is_some();
    if vectorcraft {
        found.push("`plugins`, `steps`, `menu`, `dialog` and `document` are a VectorCraft shot's".to_string());
    }
    if shot.picture.is_some() && shot.kind != picture::KIND {
        found.push("`picture` is a picture shot's".to_string());
    }
    if shot.fabric.is_some() && shot.kind != "stitch" {
        found.push("`fabric` is a stitch shot's".to_string());
    }
    found
}

/// The fabric colour `colour` names, written `#rrggbb`.
fn fabric(colour: &str) -> Result<Rgb, String> {
    let hex = colour.strip_prefix('#').filter(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let rgb = hex.and_then(|hex| u32::from_str_radix(hex, 16).ok());
    rgb.map(Rgb::from_hex).ok_or_else(|| format!("the fabric colour `{colour}` is not written `#rrggbb`"))
}

/// The PNG files of a `stitch` shot, one per image it makes ([`Shot::images`]).
pub fn stitch(root: &Path, shot: &Shot) -> Result<Vec<Vec<u8>>, String> {
    let name = shot.style.as_deref().unwrap_or(Style::Realistic.name());
    let style = Style::from_name(name).ok_or_else(|| format!("unknown style `{name}` (known: realistic, simple)"))?;
    let scale = shot.scale.unwrap_or(Settings::DEFAULT_SCALE);
    let settings = Settings::new(style, scale)
        .ok_or_else(|| format!("scale {scale} is outside {} to {} pixels per millimetre", Settings::MIN_SCALE, Settings::MAX_SCALE))?;
    let settings = match shot.fabric.as_deref() {
        Some(colour) => settings.on(fabric(colour)?),
        None => settings,
    };
    // Each picture shows what the machine its plan is for sews: the sheet's hoop, or the reference machine.
    let (plans, machine) = match (shot.sheet.as_deref(), shot.fixture.as_deref()) {
        // A sheet is drawn as it is: panels would name images that are never drawn.
        (Some(_), None) if !shot.panels.is_empty() => return Err("a test sheet is drawn as it is: panels are for designs (`fixture`)".to_string()),
        (Some(id), None) => {
            let sheet = testsheets::find(id).ok_or_else(|| format!("there is no test sheet `{id}`"))?;
            (vec![part(sheet, shot)?], sheet.profile)
        }
        (None, Some(_)) if !shot.elements.is_empty() => return Err("`elements` picks a test sheet's elements: a design is drawn whole".to_string()),
        (None, Some(fixture)) => (designs(root, fixture, shot)?, REFERENCE),
        _ => return Err("a stitch shot names a test sheet (`sheet = \"TS-01\"`) or a design (`fixture = \"…\"`)".to_string()),
    };
    plans
        .iter()
        .map(|plan| {
            stitchcraft_render::preview(plan, machine.trims, settings, &mut Budget::DEFAULT.meter()).map(|image| image.png).map_err(|e| e.to_string())
        })
        .collect()
}

/// The plan of `sheet`, or of the part of it the elements of `shot` pick, planned as the sheet is planned, and
/// saying only the diagnostic codes `shot` allows.
fn part(sheet: &testsheets::TestSheet, shot: &Shot) -> Result<StitchPlan, String> {
    let id = sheet.id;
    if shot.elements.is_empty() {
        return sheet.plan().map_err(|e| format!("{id} could not be drawn: {e}"));
    }
    let design = sheet.design().ok_or_else(|| format!("{id} is drawn stitch by stitch, and has no elements to pick"))?;
    let design = design.map_err(|e| format!("{id} could not be drawn: {e}"))?;
    let picked: Vec<Element> = design
        .elements()
        .iter()
        .filter(|element| shot.elements.iter().any(|start| element.id.as_str().starts_with(start.as_str())))
        .cloned()
        .collect();
    if picked.is_empty() {
        return Err(format!("{id} has no element whose id starts with {}", shot.elements.join(" or ")));
    }
    let part = Design::new(picked, design.settings).map_err(|d| format!("{id}: {d}"))?;
    let outcome = stitchcraft_engine::plan(&part, sheet.profile, &Budget::DEFAULT);
    let said: Vec<String> =
        outcome.diagnostics.iter().filter(|d| !shot.allow.iter().any(|code| code == d.code.id())).map(ToString::to_string).collect();
    if !said.is_empty() {
        return Err(format!("{id}: {}", said.join("; ")));
    }
    outcome.plan.ok_or_else(|| format!("{id}'s elements {} have no plan", shot.elements.join(", ")))
}

/// The plans of the design in `fixture`, read as `stitch plan` reads it: one per panel of `shot`, each with
/// its panel's parameters set on every element, or the design as it is without panels.
fn designs(root: &Path, fixture: &str, shot: &Shot) -> Result<Vec<StitchPlan>, String> {
    let bytes = util::read_bytes(&root.join(fixture))?;
    let reading = stitchcraft_engine::design::Reading { auto: shot.auto, ..Default::default() };
    let (read, warnings) = Source::of(Path::new(fixture)).read(&bytes, reading).map_err(|d| format!("{fixture}: {d}"))?;
    let unchanged = [Panel { caption: String::new(), params: BTreeMap::new() }];
    let panels = if shot.panels.is_empty() { &unchanged[..] } else { &shot.panels[..] };
    panels
        .iter()
        .map(|panel| {
            let elements: Vec<Element> = read
                .elements()
                .iter()
                .cloned()
                .map(|mut element| {
                    for (key, value) in &panel.params {
                        element.params.set(key.as_str(), value.as_str());
                    }
                    element
                })
                .collect();
            let design = Design::new(elements, read.settings).map_err(|d| format!("{fixture}: {d}"))?;
            let outcome = stitchcraft_engine::plan(&design, REFERENCE, &Budget::DEFAULT);
            let said: Vec<String> = warnings
                .iter()
                .chain(&outcome.diagnostics)
                .filter(|d| !shot.allow.iter().any(|code| code == d.code.id()))
                .map(ToString::to_string)
                .collect();
            if !said.is_empty() {
                return Err(format!("{fixture} {}: {}", panel.caption, said.join("; ")));
            }
            outcome.plan.ok_or_else(|| format!("{fixture} {} has no plan", panel.caption))
        })
        .collect()
}

/// How a new picture is the same as the committed one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Same {
    /// Byte for byte: our renderer's pictures.
    Bytes,
    /// Within VectorCraft's tolerance ([`vectorcraft::close`]): VectorCraft's pictures.
    Close,
}

/// Writes `png` as shot `id`, or with `check_only` compares it with the committed file, unless the committed
/// file is the `same`.
fn keep(root: &Path, id: &str, png: &[u8], check_only: bool, findings: &mut Findings, same: Same) {
    let path = root.join(GENERATED).join(format!("{id}.png"));
    let file = util::rel(&path);
    let current = std::fs::read(&path).ok();
    let kept = current.as_deref().is_some_and(|current| match same {
        Same::Bytes => current == png,
        Same::Close => vectorcraft::close(current, png),
    });
    if kept {
        return;
    }
    if !check_only {
        let written = std::fs::create_dir_all(root.join(GENERATED)).and_then(|()| std::fs::write(&path, png));
        match written {
            Ok(()) => println!("updated {file}"),
            Err(e) => findings.error(format!("{file}: {e}")),
        }
        return;
    }
    let fresh = root.join(STALE).join(format!("{id}.png"));
    let kept = std::fs::create_dir_all(root.join(STALE)).and_then(|()| std::fs::write(&fresh, png)).is_ok();
    let new = if kept { format!(" (the new one is {})", util::rel(&fresh)) } else { String::new() };
    let what = if current.is_some() { "is stale" } else { "is missing" };
    let command = if same == Same::Close { "cargo xtask shots --vectorcraft" } else { "cargo xtask shots" };
    findings.error(format!("{file} {what}{new}: run `{command}`, or add the `docs:refresh-shots` label to the pull request"));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `stitch` shot `probe` of `sheet` or `fixture`, with `panels` that each set one length.
    fn probe(sheet: Option<&str>, fixture: Option<&str>, lengths: &[&str]) -> Shot {
        let panels = lengths
            .iter()
            .map(|length| Panel {
                caption: format!("{length} mm"),
                params: BTreeMap::from([("running_stitch_length_mm".to_string(), (*length).to_string())]),
            })
            .collect();
        Shot {
            id: "probe".to_string(),
            kind: "stitch".to_string(),
            alt: "A probe".to_string(),
            sheet: sheet.map(str::to_string),
            elements: Vec::new(),
            fixture: fixture.map(str::to_string),
            picture: None,
            panels,
            auto: false,
            allow: Vec::new(),
            style: Some("simple".to_string()),
            fabric: None,
            scale: Some(2.0),
            report: String::new(),
            layout: Layout::Columns,
            plugins: Vec::new(),
            steps: Vec::new(),
            menu: Vec::new(),
            dialog: None,
            document: None,
        }
    }

    #[test]
    fn a_stitch_or_photo_shot_sets_no_vectorcraft_field() {
        let shot = |toml: &str| -> Shot { toml::from_str(&format!("id = \"probe\"\nalt = \"A probe\"\n{toml}")).unwrap() };
        let refused = "`plugins`, `steps`, `menu`, `dialog` and `document` are a VectorCraft shot's";
        assert_eq!(foreign(&shot("kind = \"stitch\"\nmenu = [\"Effect\"]")), [refused]);
        assert_eq!(foreign(&shot("kind = \"photo\"\nsteps = [{ cmd = \"select.all\" }]")), [refused]);
        assert_eq!(foreign(&shot("kind = \"picture\"\nplugins = [\"fill\"]")), [refused]);
        assert!(foreign(&shot("kind = \"stitch\"\nfixture = \"docs/fixtures/wave.svg\"")).is_empty());
        assert_eq!(foreign(&shot("kind = \"stitch\"\npicture = \"pear\"")), ["`picture` is a picture shot's"]);
        assert!(foreign(&shot("kind = \"picture\"\npicture = \"pear\"")).is_empty());
        assert_eq!(foreign(&shot("kind = \"picture\"\npicture = \"pear\"\nfabric = \"#000000\"")), ["`fabric` is a stitch shot's"]);
        assert!(foreign(&shot("kind = \"stitch\"\nsheet = \"TS-01\"\nfabric = \"#000000\"")).is_empty());
    }

    #[test]
    fn a_stitch_shot_is_drawn_on_the_fabric_it_names() {
        // The corner of a picture, which no thread reaches, has the fabric's colour: the style's own, white
        // for the simple style, unless the shot names one (`REQ-RND-003`).
        let root = util::root();
        let corner = |colour: Option<&str>| {
            let shot = Shot { fabric: colour.map(str::to_string), ..probe(Some("TS-01"), None, &[]) };
            let image = stitch(&root, &shot)?.remove(0);
            let pixel = tiny_skia::Pixmap::decode_png(&image).map_err(|e| e.to_string())?.pixel(0, 0).ok_or("no pixel")?;
            Ok::<_, String>((pixel.red(), pixel.green(), pixel.blue()))
        };
        assert_eq!(corner(None), Ok((255, 255, 255)));
        assert_eq!(corner(Some("#000000")), Ok((0, 0, 0)));
        assert_eq!(corner(Some("#1a2B3c")), Ok((0x1a, 0x2b, 0x3c)));
        for bad in ["black", "000000", "#00000", "#0000000", "#00000g", "#+00000"] {
            assert_eq!(corner(Some(bad)), Err(format!("the fabric colour `{bad}` is not written `#rrggbb`")));
        }
    }

    #[test]
    fn a_shot_draws_every_image_it_names_and_a_test_sheet_takes_no_panels() {
        let root = util::root();
        let design = probe(None, Some("docs/fixtures/wave.svg"), &["1.5", "4"]);
        assert_eq!(stitch(&root, &design).unwrap().len(), design.images().len());
        // A test sheet is drawn as it is: panels would name images that are never drawn.
        let sheet = stitch(&root, &probe(Some("TS-01"), None, &["1.5", "4"]));
        assert!(sheet.as_ref().is_err_and(|e| e.contains("panels")), "{sheet:?}");
    }

    #[test]
    fn a_shot_of_a_sheet_s_elements_plans_those_alone() {
        let root = util::root();
        // Each part leaves out needle points too near the one before, as docs/shots.toml allows its shots to.
        let allow = vec!["SC-I0504".to_string()];
        let part = |starts: &[&str]| Shot {
            elements: starts.iter().map(ToString::to_string).collect(),
            allow: allow.clone(),
            ..probe(Some("TS-17C"), None, &[])
        };
        // TS-17C's letters alone, as drawn and routed, and its lines alone: each a picture of its own.
        let (letters, lines) = (stitch(&root, &part(&["ts17c:drawn:", "ts17c:routed:"])).unwrap(), stitch(&root, &part(&["ts17c:lines-"])).unwrap());
        assert!(letters.len() == 1 && lines.len() == 1 && letters != lines);
        let picked = |starts: &[&str]| part_of("TS-17C", &part(starts)).unwrap().stitches().count();
        assert!(picked(&["ts17c:drawn:"]) < picked(&["ts17c:drawn:", "ts17c:routed:"]));
        for (bad, why) in [
            (part(&["ts17c:nothing"]), "no element whose id starts with ts17c:nothing"),
            (Shot { elements: vec!["a".into()], ..probe(Some("TS-01"), None, &[]) }, "stitch by stitch"),
        ] {
            let refused = stitch(&root, &bad);
            assert!(refused.as_ref().is_err_and(|e| e.contains(why)), "{refused:?}");
        }
        let fixture = Shot { elements: vec!["e".into()], ..probe(None, Some("docs/fixtures/wave.svg"), &[]) };
        assert!(stitch(&root, &fixture).is_err_and(|e| e.contains("`elements` picks a test sheet's elements")));
    }

    /// The plan [`part`] gives of the sheet `id` for `shot`.
    fn part_of(id: &str, shot: &Shot) -> Result<StitchPlan, String> {
        part(testsheets::find(id).unwrap(), shot)
    }
}
