//! The built-in machine profiles. Each is data in its own file; `BUILTIN` lists them all, and the
//! profile reference page and `stitch profiles` are generated from that list.

mod brother;

pub use brother::{BROTHER_PE800_4X4, BROTHER_PE800_5X7, BROTHER_PE800_SMALL};

use crate::profile::MachineProfile;

/// The reference machine's profile: the machine the checkpoints run on, which test sheets fit and the
/// command line plans for unless told otherwise.
pub static REFERENCE: &MachineProfile = &BROTHER_PE800_5X7;

/// Every built-in profile, in documentation order: the reference first.
pub static BUILTIN: &[&MachineProfile] = &[&BROTHER_PE800_5X7, &BROTHER_PE800_4X4, &BROTHER_PE800_SMALL];

/// The built-in profile with id `id`.
pub fn find(id: &str) -> Option<&'static MachineProfile> {
    BUILTIN.iter().copied().find(|profile| profile.id == id)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::{Point, Rect};

    use super::*;
    use crate::profile::FormatId;
    use stitchcraft_core::{Code, Edit, Fix};

    fn bounds(width: f64, height: f64) -> Rect {
        Rect::around([Point::new(-width / 2.0, -height / 2.0).unwrap(), Point::new(width / 2.0, height / 2.0).unwrap()]).unwrap()
    }

    #[test]
    fn req_prf_001_builtin_profiles_are_valid_and_unique() {
        let mut ids = std::collections::BTreeSet::new();
        for profile in BUILTIN {
            assert_eq!(profile.problems(), Vec::<String>::new());
            assert!(ids.insert(profile.id), "duplicate profile id {}", profile.id);
            assert_eq!(find(profile.id).map(|p| p.id), Some(profile.id));
        }
        assert!(find("nope").is_none());
    }

    #[test]
    fn the_pe800_profiles_record_its_hoops() {
        let field = |id: &str| find(id).map(|p| (p.hoop.width.get(), p.hoop.height.get(), p.comfort, p.format));
        assert_eq!(field("brother-pe800-5x7"), Some((130.0, 180.0, None, FormatId::PesV1)));
        assert_eq!(field("brother-pe800-4x4"), Some((100.0, 100.0, None, FormatId::PesV1)));
        assert_eq!(field("brother-pe800-small"), Some((20.0, 60.0, None, FormatId::PesV1)));
        assert_eq!(REFERENCE.id, "brother-pe800-5x7");
        // Each names its hoop, and its evidence gives the hoop's field.
        for (p, (hoop, field)) in BUILTIN.iter().zip([("5 × 7 in", "130 × 180 mm"), ("4 × 4 in", "100 × 100 mm"), ("small", "2 × 6 cm")]) {
            assert_eq!(p.name, format!("Brother PE800 with its {hoop} hoop"));
            assert!(p.evidence.contains(&format!("Field: Brother's {hoop}")) && p.evidence.contains(field), "{}", p.evidence);
        }
        // The hoops differ, and nothing else.
        for p in [&BROTHER_PE800_4X4, &BROTHER_PE800_SMALL] {
            assert_eq!(
                MachineProfile { id: REFERENCE.id, name: REFERENCE.name, hoop: REFERENCE.hoop, evidence: REFERENCE.evidence, ..(*p).clone() },
                *REFERENCE
            );
        }
    }

    #[test]
    fn req_prf_001_invalid_profiles_are_reported() {
        let mut p = BROTHER_PE800_5X7.clone();
        p.min_stitch = p.max_stitch;
        p.id = "Brother PE800";
        p.comfort = Some(stitchcraft_core::Size::new(stitchcraft_core::Mm::from_tenths(2500), p.hoop.height));
        assert_eq!(p.problems().len(), 3);
    }

    #[test]
    fn req_prf_002_designs_are_checked_against_hoop_and_comfort_zone() {
        // A comfort zone of 100 × 100 mm inside the 5 × 7 in hoop: no built-in profile has one.
        let mut p = BROTHER_PE800_5X7.clone();
        p.comfort = Some(stitchcraft_core::Size::new(stitchcraft_core::Mm::from_tenths(1000), stitchcraft_core::Mm::from_tenths(1000)));
        assert_eq!(p.check_fit(bounds(100.0, 100.0)), None, "the comfort zone itself is fine");
        let warning = p.check_fit(bounds(120.0, 100.0)).unwrap();
        assert_eq!(warning.code, Code::OutsideComfortZone);
        assert_eq!(
            warning.message,
            "The design is 120.0 × 100.0 mm, larger than the 100 × 100 mm comfort zone of the Brother PE800 with its 5 × 7 in hoop."
        );
        assert!(matches!(warning.fix, Some(Fix::Hint(_))), "turning does not help a square comfort zone");
        let error = p.check_fit(bounds(131.0, 181.0)).unwrap();
        assert_eq!(error.code, Code::OutsideHoop);
        assert_eq!(error.message, "The design is 131.0 × 181.0 mm, but the Brother PE800 with its 5 × 7 in hoop sews at most 130 × 180 mm.");
        assert!(matches!(error.fix, Some(Fix::Hint(_))));
    }

    #[test]
    fn req_prf_002_rotation_is_offered_when_it_fits() {
        let p = &BROTHER_PE800_5X7;
        let error = p.check_fit(bounds(170.0, 120.0)).unwrap();
        assert_eq!(error.code, Code::OutsideHoop);
        assert_eq!(error.fix, Some(Fix::Apply(Edit::RotateDesign90)));
        assert_eq!(p.check_fit(bounds(120.0, 170.0)), None);
        assert_eq!(p.check_fit(bounds(130.0, 180.0)), None, "the field itself is fine");
    }
}
