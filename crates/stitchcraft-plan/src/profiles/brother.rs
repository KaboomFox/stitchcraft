//! Brother home embroidery machines.

use stitchcraft_core::{Mm, Size};

use crate::profile::{FormatId, MachineProfile, PaletteId, TrimSupport};

/// The reference machine: a Brother home embroidery machine with its 200 × 200 mm (8 × 8 in) hoop,
/// designs normally kept within about 150 mm (6 in).
pub static BROTHER_200X200: MachineProfile = MachineProfile {
    id: "brother-200x200",
    name: "Brother, 200 × 200 mm hoop",
    hoop: Size::new(Mm::from_tenths(2000), Mm::from_tenths(2000)),
    comfort: Some(Size::new(Mm::from_tenths(1500), Mm::from_tenths(1500))),
    format: FormatId::PesV1,
    max_stitch: Mm::from_tenths(120),
    min_stitch: Mm::from_tenths(3),
    trims: TrimSupport::Command,
    palette: PaletteId::BrotherPec,
    evidence: "Hoop and comfort zone: the owner of the reference machine (2026-10-08). Stitch limits and trim \
               support: common Brother home-machine values, to be confirmed by machine checkpoint MC-1 \
               (test sheets TS-01, TS-02, TS-10).",
};
