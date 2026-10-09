//! `cargo xtask shots [--check]`: every documentation image is declared, reproducible and current
//! (`docs/src/design/docs-pipeline.md`).
//!
//! Each `[[shot]]` in `docs/shots.toml` says how its image is made. Without `--check` the images are
//! regenerated into `docs/src/images/generated/`; with `--check` they are regenerated in memory and
//! compared byte for byte with the committed files, so a pull request that changes how something looks
//! must also update its pictures. A stale image's new version is written to `target/shots/` (CI keeps it),
//! and a generated file no shot declares is an error. Generators arrive with the roadmap. `stitch` renders
//! with `stitchcraft-render` either a test sheet, or an SVG design that the engine plans as `stitch plan`
//! plans it, once per panel with the panel's parameters set on every element. `vectorcraft-render` and
//! `vectorcraft-ui` come with the VectorCraft pipeline (M0.8, M6.7). `photo` shots are never
//! regenerated; they must exist and link their sew-out report.
//!
//! A shot of a design with panels makes one image per panel, `<id>-1.png` and so on, and pages show them
//! side by side in a figure that `cargo xtask docs` writes ([`crate::figures`]).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use stitchcraft_core::Budget;
use stitchcraft_engine::design::{Design, Element};
use stitchcraft_engine::testsheets;
use stitchcraft_plan::StitchPlan;
use stitchcraft_plan::profiles::BROTHER_200X200;
use stitchcraft_render::{Settings, Style};

use crate::util::{self, Findings};

const SHOTS: &str = "docs/shots.toml";
/// Where generated images live; pages link `images/generated/<id>.png`.
pub const GENERATED: &str = "docs/src/images/generated";
const PHOTOS: &str = "docs/src/images/photos";
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
    /// How it is made: `stitch`, `vectorcraft-render`, `vectorcraft-ui` or `photo`.
    pub kind: String,
    /// The alternative text; pages that show the image use it.
    #[serde(default)]
    pub alt: String,
    /// `stitch` shots: the test sheet to draw.
    #[serde(default)]
    pub sheet: Option<String>,
    /// `stitch` shots: or else an SVG file of the repository, planned by the engine for the reference
    /// machine.
    #[serde(default)]
    pub fixture: Option<String>,
    /// `stitch` shots of a design: one image per panel. Without panels, the design is drawn once, as it is.
    #[serde(default)]
    pub panels: Vec<Panel>,
    /// `stitch` shots of a design: the diagnostic codes the plan may report. Any other is an error, so a
    /// picture never shows a design the engine changed without saying so on the page.
    #[serde(default)]
    pub allow: Vec<String>,
    /// `stitch` shots: `realistic` (the default) or `simple`.
    #[serde(default)]
    pub style: Option<String>,
    /// `stitch` shots: pixels per millimetre (the renderer's default when absent).
    #[serde(default)]
    pub scale: Option<f32>,
    /// `photo` shots: the sew-out report.
    #[serde(default)]
    pub report: String,
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
}

/// The declared shots, in file order.
pub fn load(root: &Path) -> Result<Vec<Shot>, String> {
    let file: ShotsFile = toml::from_str(&util::read(&root.join(SHOTS))?).map_err(|e| format!("{SHOTS}: {e}"))?;
    Ok(file.shot)
}

/// `cargo xtask shots`.
pub fn run(check_only: bool) -> Result<(), String> {
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
        match shot.kind.as_str() {
            "photo" => {
                let found = ["jpg", "jpeg", "png"].iter().any(|ext| root.join(PHOTOS).join(format!("{}.{ext}", shot.id)).is_file());
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
                        keep(&root, name, png, check_only, &mut findings);
                    }
                }
                Err(e) => findings.error(format!("shot `{}`: {e}", shot.id)),
            },
            "vectorcraft-render" | "vectorcraft-ui" => {
                findings.error(format!("shot `{}`: VectorCraft shots arrive with roadmap steps M0.8 and M6.7", shot.id));
            }
            other => findings.error(format!("shot `{}`: unknown kind `{other}`", shot.id)),
        }
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

/// The PNGs of a `stitch` shot, one per image it makes ([`Shot::images`]).
pub fn stitch(root: &Path, shot: &Shot) -> Result<Vec<Vec<u8>>, String> {
    let name = shot.style.as_deref().unwrap_or(Style::Realistic.name());
    let style = Style::from_name(name).ok_or_else(|| format!("unknown style `{name}` (known: realistic, simple)"))?;
    let scale = shot.scale.unwrap_or(Settings::DEFAULT_SCALE);
    let settings = Settings::new(style, scale)
        .ok_or_else(|| format!("scale {scale} is outside {} to {} pixels per millimetre", Settings::MIN_SCALE, Settings::MAX_SCALE))?;
    let plans = match (shot.sheet.as_deref(), shot.fixture.as_deref()) {
        (Some(id), None) => {
            let sheet = testsheets::find(id).ok_or_else(|| format!("there is no test sheet `{id}`"))?;
            vec![sheet.plan().map_err(|e| format!("{id} could not be drawn: {e}"))?]
        }
        (None, Some(fixture)) => designs(root, fixture, shot)?,
        _ => return Err("a stitch shot names a test sheet (`sheet = \"TS-01\"`) or an SVG design (`fixture = \"…\"`)".to_string()),
    };
    plans
        .iter()
        .map(|plan| stitchcraft_render::preview(plan, settings, &mut Budget::DEFAULT.meter()).map(|image| image.png).map_err(|e| e.to_string()))
        .collect()
}

/// The plans of the design in `fixture`: one per panel of `shot`, each with its panel's parameters set on
/// every element, or the design as it is without panels.
fn designs(root: &Path, fixture: &str, shot: &Shot) -> Result<Vec<StitchPlan>, String> {
    let svg = stitchcraft_svg::read(&util::read_bytes(&root.join(fixture))?, &Budget::DEFAULT).map_err(|d| format!("{fixture}: {d}"))?;
    let unchanged = [Panel { caption: String::new(), params: BTreeMap::new() }];
    let panels = if shot.panels.is_empty() { &unchanged[..] } else { &shot.panels[..] };
    panels
        .iter()
        .map(|panel| {
            let elements: Vec<Element> = svg
                .design
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
            let design = Design::new(elements, svg.design.settings).map_err(|d| format!("{fixture}: {d}"))?;
            let outcome = stitchcraft_engine::plan(&design, &BROTHER_200X200, &Budget::DEFAULT);
            let said: Vec<String> = svg
                .warnings
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

/// Writes `png` as shot `id`, or with `check_only` compares it with the committed file.
fn keep(root: &Path, id: &str, png: &[u8], check_only: bool, findings: &mut Findings) {
    let path = root.join(GENERATED).join(format!("{id}.png"));
    let file = util::rel(&path);
    let current = std::fs::read(&path).ok();
    if current.as_deref() == Some(png) {
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
    findings.error(format!("{file} {what}{new}: run `cargo xtask shots`, or add the `docs:refresh-shots` label to the pull request"));
}
