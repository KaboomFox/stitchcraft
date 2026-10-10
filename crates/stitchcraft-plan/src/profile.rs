//! Machine profiles: the facts about one machine and hoop that decide what a plan may contain.
//!
//! A profile says how big the hoop is, where the machine sews best, which file format it reads, and the
//! stitch lengths it handles. These are *machine facts*: a value changes only with a sew-out report that
//! justifies it (`docs/src/plan/machine-testing.md`), and each profile names its evidence. The built-in
//! profiles are data in [`crate::profiles`].

use stitchcraft_core::{Code, Diagnostic, Edit, Fix, Mm, Rect, Size};

use crate::palette::{BROTHER_PEC, Palette};

/// The machine-file formats StitchCraft writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FormatId {
    /// Brother PES, version 1 (`#PES0001`) with its PEC block.
    PesV1,
    /// Tajima DST.
    Dst,
}

impl FormatId {
    /// Every format, in documentation order.
    pub const ALL: &'static [FormatId] = &[FormatId::PesV1, FormatId::Dst];

    /// The name people know it by.
    pub const fn name(self) -> &'static str {
        match self {
            FormatId::PesV1 => "PES v1",
            FormatId::Dst => "DST",
        }
    }

    /// The file extension, lower case and without a dot.
    pub const fn extension(self) -> &'static str {
        match self {
            FormatId::PesV1 => "pes",
            FormatId::Dst => "dst",
        }
    }

    /// The format a file extension stands for (any case, without a dot).
    pub fn from_extension(extension: &str) -> Option<FormatId> {
        FormatId::ALL.iter().copied().find(|f| f.extension().eq_ignore_ascii_case(extension))
    }

    /// The palette whose indices the format stores instead of colours, if any.
    pub const fn palette(self) -> Option<PaletteId> {
        match self {
            FormatId::PesV1 => Some(PaletteId::BrotherPec),
            FormatId::Dst => None,
        }
    }

    /// The most colour changes (stops included) the format can record. PES stores the number of colour
    /// entries minus one in a single byte (255); DST's header has a three-digit `CO:` field (999).
    pub const fn max_color_changes(self) -> usize {
        match self {
            FormatId::PesV1 => 255,
            FormatId::Dst => 999,
        }
    }
}

/// How a machine trims thread between parts of a design.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrimSupport {
    /// The machine obeys the format's trim command (for PES, trim-flagged jumps).
    Command,
    /// The machine has no trim command but cuts jumps longer than this by itself.
    LongJumps(Mm),
    /// The machine cannot trim; the operator cuts jump threads.
    None,
}

/// The thread palettes formats map colours to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PaletteId {
    /// The 64 Brother PEC colours.
    BrotherPec,
}

impl PaletteId {
    /// The palette's table.
    pub const fn palette(self) -> &'static Palette {
        match self {
            PaletteId::BrotherPec => &BROTHER_PEC,
        }
    }
}

/// The facts about one machine and hoop.
#[derive(Clone, Debug, PartialEq)]
pub struct MachineProfile {
    /// Stable id used on the command line, such as `brother-pe800-5x7`.
    pub id: &'static str,
    /// Human name, such as "Brother PE800 with its 5 × 7 in hoop", which messages put after "the".
    pub name: &'static str,
    /// The sewing field of the hoop.
    pub hoop: Size,
    /// The area where the machine sews most accurately; larger designs get a warning.
    pub comfort: Option<Size>,
    /// The file format the machine reads.
    pub format: FormatId,
    /// The longest stitch the machine sews cleanly.
    pub max_stitch: Mm,
    /// The shortest stitch the machine sews without thread breaks or nests (locks excepted).
    pub min_stitch: Mm,
    /// How the machine trims.
    pub trims: TrimSupport,
    /// The palette the format maps thread colours to.
    pub palette: PaletteId,
    /// Where these values come from: sew-out reports, the owner's manual, or what still needs testing.
    pub evidence: &'static str,
}

impl MachineProfile {
    /// The problems with this profile's values, empty when it is valid (REQ-PRF-001).
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let positive = |size: Size| size.width.get() > 0.0 && size.height.get() > 0.0;
        if !positive(self.hoop) {
            problems.push(format!("{}: the hoop must have a positive width and height", self.id));
        }
        if let Some(comfort) = self.comfort
            && (!positive(comfort) || !self.hoop.holds(comfort.width.get(), comfort.height.get()))
        {
            problems.push(format!("{}: the comfort zone must be positive and inside the hoop", self.id));
        }
        if !(self.min_stitch.get() > 0.0 && self.min_stitch.get() < self.max_stitch.get()) {
            problems.push(format!("{}: need 0 < min_stitch < max_stitch", self.id));
        }
        if let TrimSupport::LongJumps(length) = self.trims
            && length.get() <= 0.0
        {
            problems.push(format!("{}: the trim jump length must be positive", self.id));
        }
        let id_ok = !self.id.is_empty()
            && self.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !self.id.starts_with('-')
            && !self.id.ends_with('-');
        if !id_ok {
            problems.push(format!("`{}`: ids are lower-case words and digits joined by hyphens", self.id));
        }
        if self.name.is_empty() || self.evidence.is_empty() {
            problems.push(format!("{}: name and evidence are required", self.id));
        }
        problems
    }

    /// Whether a design with these bounds fits: `SC-E0701` when it does not fit the hoop, `SC-W0702`
    /// when it is larger than the comfort zone, `None` when it is fine. When turning the design 90
    /// degrees would fit, the diagnostic offers that as a fix (REQ-PRF-002).
    pub fn check_fit(&self, bounds: Rect) -> Option<Diagnostic> {
        let (width, height) = (bounds.width(), bounds.height());
        let design = format!("The design is {width:.1} × {height:.1} mm");
        if !self.hoop.holds(width, height) {
            let message = format!("{design}, but the {} sews at most {}.", self.name, describe(self.hoop));
            let fix = if self.hoop.rotated().holds(width, height) {
                Fix::Apply(Edit::RotateDesign90)
            } else {
                Fix::Hint("Scale the design down, or split it into parts sewn in separate hoopings.".to_string())
            };
            return Some(Diagnostic::new(Code::OutsideHoop, message).with_fix(fix));
        }
        let comfort = self.comfort?;
        if comfort.holds(width, height) {
            return None;
        }
        let message = format!("{design}, larger than the {} comfort zone of the {}.", describe(comfort), self.name);
        let fix = if comfort.rotated().holds(width, height) {
            Fix::Apply(Edit::RotateDesign90)
        } else {
            Fix::Hint("Use a firm stabilizer and hoop the fabric drum-tight, or scale the design down.".to_string())
        };
        Some(Diagnostic::new(Code::OutsideComfortZone, message).with_fix(fix))
    }
}

/// `130 × 180 mm`.
fn describe(size: Size) -> String {
    format!("{} × {} mm", size.width.get(), size.height.get())
}
