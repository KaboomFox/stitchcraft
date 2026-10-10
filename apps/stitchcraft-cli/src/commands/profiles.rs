//! `stitch profiles`: the built-in machine profiles and the facts behind them (REQ-PRF-001).

use std::fmt::Write as _;

use stitchcraft_plan::profiles::BUILTIN;
use stitchcraft_plan::{MachineProfile, TrimSupport};

use super::Outcome;

/// Lists the built-in profiles.
pub fn run() -> Outcome {
    let mut out = String::new();
    for profile in BUILTIN {
        describe(&mut out, profile);
    }
    Outcome::done(out)
}

fn describe(out: &mut String, p: &MachineProfile) {
    let size = |s: stitchcraft_core::Size| format!("{} × {} mm", s.width.get(), s.height.get());
    let _ = writeln!(out, "{} — {}", p.id, p.name);
    let _ = writeln!(out, "  hoop          {}", size(p.hoop));
    if let Some(comfort) = p.comfort {
        let _ = writeln!(out, "  comfort zone  {} (larger designs get warning SC-W0702)", size(comfort));
    }
    let _ = writeln!(out, "  format        {}", p.format.name());
    let _ = writeln!(out, "  stitches      {} to {} mm", p.min_stitch.get(), p.max_stitch.get());
    let trims = match p.trims {
        TrimSupport::Command => "the format's trim command".to_string(),
        TrimSupport::LongJumps(length) => format!("cuts jumps longer than {} mm by itself", length.get()),
        TrimSupport::None => "none (cut jump threads by hand)".to_string(),
    };
    let _ = writeln!(out, "  trims         {trims}");
    let _ = writeln!(out, "  evidence      {}", p.evidence);
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::{Mm, Size};
    use stitchcraft_plan::profiles::REFERENCE;

    use super::*;

    #[test]
    fn lists_every_profile_with_its_facts() {
        let out = run().stdout;
        assert!(out.starts_with("brother-pe800-5x7 — Brother PE800 with its 5 × 7 in hoop\n  hoop          130 × 180 mm\n  format "), "{out}");
        assert!(out.contains("\nbrother-pe800-4x4 — Brother PE800 with its 4 × 4 in hoop\n  hoop          100 × 100 mm\n"), "{out}");
        assert!(out.contains("\nbrother-pe800-small — Brother PE800 with its small hoop\n  hoop          20 × 60 mm\n"), "{out}");
        assert!(out.contains("stitches      0.3 to 12 mm"));
        assert!(!out.contains("comfort zone"), "no built-in profile has one");
    }

    #[test]
    fn a_comfort_zone_is_listed_after_the_hoop() {
        let mut out = String::new();
        let square = Size::new(Mm::from_tenths(1000), Mm::from_tenths(1000));
        describe(&mut out, &MachineProfile { comfort: Some(square), ..REFERENCE.clone() });
        assert!(out.contains("  hoop          130 × 180 mm\n  comfort zone  100 × 100 mm (larger designs get warning SC-W0702)\n"), "{out}");
    }
}
