//! `cargo xtask shots [--check]`: every documentation image is declared, reproducible and current
//! (`docs/src/design/docs-pipeline.md`).
//!
//! Each `[[shot]]` in `docs/shots.toml` says how its image is made. Without `--check` the images are
//! regenerated; with `--check` they are regenerated in memory and compared with the committed files, so a
//! pull request that changes how something looks must also update its pictures. Generators arrive with the
//! roadmap: `stitch` renders with the renderer (M2.6–M2.7), `vectorcraft-render` and `vectorcraft-ui` with
//! the VectorCraft pipeline (M0.8, M6.7). `photo` shots are never regenerated; they must exist and link
//! their sew-out report.

use serde::Deserialize;

use crate::util::{self, Findings};

const SHOTS: &str = "docs/shots.toml";
const IMAGES: &str = "docs/src/images";

#[derive(Deserialize, Default)]
struct Shots {
    #[serde(default)]
    shot: Vec<Shot>,
}

#[derive(Deserialize)]
struct Shot {
    id: String,
    kind: String,
    #[serde(default)]
    alt: String,
    /// Sew-out report URL (photos only).
    #[serde(default)]
    report: String,
}

/// `cargo xtask shots`.
pub fn run(check_only: bool) -> Result<(), String> {
    let root = util::root();
    let shots: Shots = toml::from_str(&util::read(&root.join(SHOTS))?).map_err(|e| format!("{SHOTS}: {e}"))?;
    let mut findings = Findings::default();
    let mut ids = std::collections::BTreeSet::new();
    for shot in &shots.shot {
        if !ids.insert(shot.id.as_str()) {
            findings.error(format!("{SHOTS}: duplicate shot id `{}`", shot.id));
        }
        if shot.alt.trim().is_empty() {
            findings.error(format!("{SHOTS}: shot `{}` needs alt text", shot.id));
        }
        match shot.kind.as_str() {
            "photo" => {
                let found = ["jpg", "jpeg", "png"].iter().any(|ext| root.join(IMAGES).join("photos").join(format!("{}.{ext}", shot.id)).is_file());
                if !found {
                    findings.error(format!("photo `{}` is missing from {IMAGES}/photos/", shot.id));
                }
                if !shot.report.starts_with("https://") {
                    findings.error(format!("photo `{}` must link its sew-out report (`report = \"https://…\"`)", shot.id));
                }
            }
            "stitch" => findings.error(format!("shot `{}`: stitch renders arrive with the renderer (roadmap M2.6–M2.7)", shot.id)),
            "vectorcraft-render" | "vectorcraft-ui" => {
                findings.error(format!("shot `{}`: VectorCraft shots arrive with roadmap steps M0.8 and M6.7", shot.id));
            }
            other => findings.error(format!("shot `{}`: unknown kind `{other}`", shot.id)),
        }
    }
    let verb = if check_only { "current" } else { "regenerated" };
    findings.finish("shots", &format!("{} shots declared, all {verb}", shots.shot.len()))
}
