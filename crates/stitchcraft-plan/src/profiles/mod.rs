//! The built-in machine profiles. Each is data in its own file; `BUILTIN` lists them all, and the
//! profile reference page and `stitch profiles` are generated from that list.

mod brother;

pub use brother::BROTHER_200X200;

use crate::profile::MachineProfile;

/// Every built-in profile, in documentation order.
pub static BUILTIN: &[&MachineProfile] = &[&BROTHER_200X200];

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
    fn the_brother_profile_records_the_owner_facts() {
        let p = find("brother-200x200").unwrap();
        assert_eq!((p.hoop.width.get(), p.hoop.height.get()), (200.0, 200.0));
        assert_eq!(p.comfort.map(|c| (c.width.get(), c.height.get())), Some((150.0, 150.0)));
        assert_eq!(p.format, FormatId::PesV1);
    }

    #[test]
    fn req_prf_001_invalid_profiles_are_reported() {
        let mut p = BROTHER_200X200.clone();
        p.min_stitch = p.max_stitch;
        p.id = "Brother 200";
        p.comfort = Some(stitchcraft_core::Size::new(stitchcraft_core::Mm::from_tenths(2500), p.hoop.height));
        assert_eq!(p.problems().len(), 3);
    }

    #[test]
    fn req_prf_002_designs_are_checked_against_hoop_and_comfort_zone() {
        let p = &BROTHER_200X200;
        assert_eq!(p.check_fit(bounds(150.0, 150.0)), None, "the comfort zone itself is fine");
        let warning = p.check_fit(bounds(190.0, 150.0)).unwrap();
        assert_eq!(warning.code, Code::OutsideComfortZone);
        assert_eq!(warning.message, "The design is 190.0 × 150.0 mm, larger than the 150 × 150 mm comfort zone of Brother, 200 × 200 mm hoop.");
        assert!(matches!(warning.fix, Some(Fix::Hint(_))), "turning does not help a square comfort zone");
        let error = p.check_fit(bounds(201.0, 120.0)).unwrap();
        assert_eq!(error.code, Code::OutsideHoop);
        assert!(matches!(error.fix, Some(Fix::Hint(_))));
    }

    #[test]
    fn req_prf_002_rotation_is_offered_when_it_fits() {
        let mut p = BROTHER_200X200.clone();
        p.hoop = stitchcraft_core::Size::new(stitchcraft_core::Mm::from_tenths(1300), stitchcraft_core::Mm::from_tenths(1800));
        p.comfort = None;
        let error = p.check_fit(bounds(170.0, 120.0)).unwrap();
        assert_eq!(error.code, Code::OutsideHoop);
        assert_eq!(error.fix, Some(Fix::Apply(Edit::RotateDesign90)));
        assert_eq!(p.check_fit(bounds(120.0, 170.0)), None);
    }
}
