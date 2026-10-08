//! REQ-RND-001: a preview shows what sews — the holes a machine file makes, not the positions the plan
//! meant — checked on off-grid plans and by reading designs back from the PES files they become.
//!
//! REQ-RND-002: previews are byte-identical on every platform, checked against committed golden PNG
//! files on Linux, macOS and Windows. A changed golden file is a changed picture: re-bless it on purpose,
//! with `STITCHCRAFT_BLESS=1 cargo test -p stitchcraft-render --test preview`, in a PR that explains why
//! (`docs/src/design/conformance.md`).

// Test code may unwrap (clippy.toml allows it inside #[test] functions; these helpers are test code too).
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use proptest::prelude::*;
use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::testsheets;
use stitchcraft_formats::{decode, encode};
use stitchcraft_plan::palette::BROTHER_PEC;
use stitchcraft_plan::{FormatId, PlanBuilder, Provenance, Rgb, Role, StitchPlan, Thread};
use stitchcraft_render::{Scene, Settings, Style, preview};
use stitchcraft_testkit::strategies;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A small design with everything a preview draws: lock stitches, stitches in every direction, a trim,
/// a jump thread left uncut, a stop, a thread change and a trim at the end.
fn sampler(offset: f64) -> StitchPlan {
    let (top, lock, travel) = (Provenance::plan(Role::Top), Provenance::plan(Role::Lock), Provenance::plan(Role::Travel));
    let at = |x: f64, y: f64| p(x + offset, y + offset);
    let mut b = PlanBuilder::new(Thread::named(Rgb::from_hex(0xED171F), "Red"));
    b.jump(at(-15.0, -8.0), travel);
    for (x, y) in [(-15.0, -8.0), (-14.7, -8.0), (-15.0, -8.0)] {
        b.stitch(at(x, y), lock);
    }
    for i in 1..=8 {
        let x = -15.0 + 2.5 * f64::from(i);
        b.stitch(at(x, if i % 2 == 0 { -8.0 } else { -3.0 }), top);
    }
    b.stitch(at(4.7, -8.0), lock);
    b.stitch(at(5.0, -8.0), lock);
    b.trim(None);
    b.jump(at(10.0, -8.0), travel);
    for (x, y) in [(10.0, -8.0), (12.0, -5.0), (14.0, -8.0)] {
        b.stitch(at(x, y), top);
    }
    // A jump without a trim: the thread stays, loose, from (14, -8) to (14, 2).
    b.jump(at(16.0, -2.0), travel);
    b.stitch(at(14.0, 2.0), top);
    b.stitch(at(11.0, 2.0), top);
    b.stop(None);
    b.stitch(at(8.0, 2.0), top);
    b.change_thread(Thread::named(Rgb::from_hex(0x0A55A3), "Blue"));
    b.jump(at(-12.0, 2.0), travel);
    for (x, y) in [(-12.0, 2.0), (-4.0, 2.0), (-4.0, 9.0), (-12.0, 9.0), (-12.0, 2.0), (-4.0, 9.0)] {
        b.stitch(at(x, y), top);
    }
    b.trim(None);
    b.finish()
}

/// `plan` with every position moved to the machine grid.
fn on_grid(plan: &StitchPlan) -> StitchPlan {
    let mut plan = plan.clone();
    for stitch in plan.blocks.iter_mut().flat_map(|b| b.stitches.iter_mut()) {
        let (x, y) = stitch.at.to_tenths().unwrap();
        stitch.at = Point::from_tenths(x, y);
    }
    plan
}

fn scene(plan: &StitchPlan) -> Scene {
    Scene::of(plan, &mut Budget::DEFAULT.meter()).unwrap()
}

/// `plan` written as a PES file and read back.
fn through_pes(plan: &StitchPlan) -> StitchPlan {
    decode(&encode(plan, FormatId::PesV1, "preview").unwrap()).unwrap().plan
}

/// What a PES file can say of `scene`: colours become Brother palette colours and roles are gone.
fn as_pes_says(mut scene: Scene) -> Scene {
    for hole in &mut scene.holes {
        hole.color = BROTHER_PEC.nearest(hole.color).unwrap().color;
        hole.lock = false;
    }
    scene
}

#[test]
fn req_rnd_001_previews_show_the_holes_a_machine_file_makes() {
    let meant = sampler(0.04);
    assert_ne!(meant, on_grid(&meant), "the sampler must be off the grid for this test to mean anything");
    for style in Style::ALL {
        let settings = Settings::new(*style, 10.0).unwrap();
        let shown = preview(&meant, settings, &mut Budget::DEFAULT.meter()).unwrap();
        let sewn = preview(&on_grid(&meant), settings, &mut Budget::DEFAULT.meter()).unwrap();
        assert!(shown == sewn, "{}: the preview of the plan differs from the preview of what sews", style.name());
    }
}

#[test]
fn req_rnd_001_test_sheets_look_the_same_read_back_from_their_pes_files() {
    for sheet in testsheets::SHEETS {
        let plan = sheet.plan().unwrap();
        assert_eq!(scene(&through_pes(&plan)), as_pes_says(scene(&plan)), "{}", sheet.id);
    }
    let plan = sampler(0.04);
    assert_eq!(scene(&through_pes(&plan)), as_pes_says(scene(&plan)), "sampler");
}

proptest! {
    #![proptest_config(strategies::config(256))]

    /// Random, untidy plans (positions on a 0.01 mm grid, commands anywhere), small enough that PES
    /// never splits a stitch.
    #[test]
    fn req_rnd_001_random_plans_look_the_same_read_back_from_pes(plan in strategies::plan(120, 70.0)) {
        // PES cannot tell two blocks whose threads are the same Brother colour from one block with a
        // stop (`stitchcraft-formats/src/pes/read.rs`), so such plans read back differently.
        let pec = |b: &stitchcraft_plan::ColorBlock| BROTHER_PEC.nearest(b.thread.color).unwrap().index;
        prop_assume!(plan.blocks.windows(2).all(|w| pec(&w[0]) != pec(&w[1])));
        prop_assert_eq!(scene(&through_pes(&plan)), as_pes_says(scene(&plan)));
    }
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/golden/render")
}

#[test]
fn req_rnd_002_previews_match_their_golden_files() {
    let bless = std::env::var_os("STITCHCRAFT_BLESS").is_some();
    let ts01 = testsheets::find("TS-01").unwrap().plan().unwrap();
    let cases =
        [("sampler", sampler(0.0), Style::Simple, 10.0), ("sampler", sampler(0.0), Style::Realistic, 10.0), ("TS-01", ts01, Style::Realistic, 4.0)];
    for (name, plan, style, scale) in cases {
        let image = preview(&plan, Settings::new(style, scale).unwrap(), &mut Budget::DEFAULT.meter()).unwrap();
        let path = golden_dir().join(format!("{name}-{}.png", style.name()));
        if bless {
            std::fs::create_dir_all(golden_dir()).unwrap();
            std::fs::write(&path, &image.png).unwrap();
            continue;
        }
        let golden = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e} (bless with STITCHCRAFT_BLESS=1)", path.display()));
        if golden != image.png {
            // Keep the new image next to the build output, so a failure on CI can be looked at.
            let new = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{}.png", style.name()));
            let _ = std::fs::write(&new, &image.png);
            panic!("{}: the preview differs from the golden file; the new one is {}", path.display(), new.display());
        }
    }
}
