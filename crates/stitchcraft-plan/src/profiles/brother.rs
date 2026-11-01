//! Brother home embroidery machines: the reference machine, a Brother PE800, with each of its hoops
//! (`docs/src/design/adr/0013-brother-pe800-reference-machine.md`).
//!
//! The PE800 sews at most 130 × 180 mm, the field of its 5 × 7 in hoop: 130 mm across and 180 mm from
//! back to front, as PES v1's hoop indicator also says. The 4 × 4 in hoop's field is 100 × 100 mm. The
//! small hoop is sold as 1 × 2.5 in, and Brother gives its field as 2 × 6 cm, the smaller and safer figure:
//! a design that fits it fits the hoop. Its field stands upright here, as the 5 × 7 in's does, until test
//! sheet TS-10C shows which way the machine holds it. Each hoop is the limit, with no smaller comfort zone.
//! The stitch limits are common Brother home-machine values, which the machine checkpoints confirm. The
//! trims are machine checkpoint MC-1's: the PE800 cuts the thread at thread changes and stops, and at no
//! jump, trim-flagged or not, so the operator cuts the jump threads.

use stitchcraft_core::{Mm, Size};

use crate::profile::{FormatId, MachineProfile, PaletteId, TrimSupport};

/// The PE800 with its 5 × 7 in hoop, which the other hoops' profiles start from.
const PE800: MachineProfile = MachineProfile {
    id: "brother-pe800-5x7",
    name: "Brother PE800 with its 5 × 7 in hoop",
    hoop: Size::new(Mm::from_tenths(1300), Mm::from_tenths(1800)),
    comfort: None,
    format: FormatId::PesV1,
    max_stitch: Mm::from_tenths(120),
    min_stitch: Mm::from_tenths(3),
    trims: TrimSupport::None,
    palette: PaletteId::BrotherPec,
    evidence: "Machine: the owner of the reference machine (2026-10-10). Field: the 5 × 7 in hoop Brother makes for the \
               PE800, 130 × 180 mm. Trims: from machine checkpoint MC-1's sew-out, which the owner reported (2026-10-31). The \
               machine cut the thread between colours. It cut no jump between places in one colour, whether flagged as \
               a trim or not. Stitch limits: common Brother home-machine values, to be confirmed by machine checkpoints \
               MC-1 and MC-2 (test sheets TS-01 to TS-04 and TS-10).",
};

/// The reference machine: a Brother PE800 with its 5 × 7 in hoop, the largest it takes.
pub static BROTHER_PE800_5X7: MachineProfile = PE800;

/// The Brother PE800 with its 4 × 4 in hoop.
pub static BROTHER_PE800_4X4: MachineProfile = MachineProfile {
    id: "brother-pe800-4x4",
    name: "Brother PE800 with its 4 × 4 in hoop",
    hoop: Size::new(Mm::from_tenths(1000), Mm::from_tenths(1000)),
    evidence: "Hoop: the owner's (2026-10-10). Field: the 4 × 4 in hoop Brother makes, 100 × 100 mm. The rest as \
               for brother-pe800-5x7.",
    ..PE800
};

/// The Brother PE800 with its small hoop, for monograms, cuffs and small patches.
pub static BROTHER_PE800_SMALL: MachineProfile = MachineProfile {
    id: "brother-pe800-small",
    name: "Brother PE800 with its small hoop",
    hoop: Size::new(Mm::from_tenths(200), Mm::from_tenths(600)),
    evidence: "Hoop: the owner's, sold as 1 × 2.5 in (2026-10-10). Field: the small hoop Brother makes, 2 × 6 cm. It \
               stands upright here, as the 5 × 7 in's does, until test sheet TS-10C shows which way the machine holds \
               it. The rest as for brother-pe800-5x7.",
    ..PE800
};
