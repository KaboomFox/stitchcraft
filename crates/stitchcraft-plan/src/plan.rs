//! The stitch plan: everything the machine does, in order.
//!
//! A plan is a list of colour blocks, each sewn with one thread. Between two blocks the machine stops
//! for a thread change, and after the last block it ends. Both are implied by the structure, so a plan
//! cannot put a colour change in the wrong place or forget its end; encoders write exactly one colour
//! change between blocks and exactly one end (REQ-PLAN-003).
//!
//! Inside a block every entry is a [`Stitch`]:
//!
//! | Kind | The machine… |
//! |---|---|
//! | `Normal` | moves the frame so the needle is over `at`, and sews: the needle goes down there |
//! | `Jump` | moves the frame so the needle is over `at`, without sewing |
//! | `Trim` | cuts the thread where the needle is; `at` repeats that position |
//! | `Stop` | pauses for the operator where the needle is; `at` repeats that position |
//!
//! Commands carry the needle's position so that every entry has one: bounds, previews and reports never
//! special-case them, and the invariant checker verifies the position is the needle's.
//!
//! **Coordinates** are millimetres with y down, relative to the machine origin, the centre of the hoop.
//! The needle starts there, above the fabric; the first `Normal` stitch is the first time it goes down.
//! Encoders quantize positions to 0.1 mm once, when writing (`docs/src/design/determinism.md`).

use stitchcraft_core::{ElementId, Point, Rect};

use crate::thread::Thread;

/// What the machine does at one entry of the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StitchKind {
    /// Move and sew.
    Normal,
    /// Move without sewing.
    Jump,
    /// Cut the thread where the needle is.
    Trim,
    /// Pause for the operator where the needle is (encoders write it as a change to the same thread).
    Stop,
}

impl StitchKind {
    /// Whether this kind happens in place (trim, stop) rather than moving the needle.
    pub const fn is_command(self) -> bool {
        matches!(self, StitchKind::Trim | StitchKind::Stop)
    }
}

/// Why a stitch exists: the part of an element's stitching it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// The visible stitching of an element.
    Top,
    /// Stitches under the top stitching that stabilize the fabric.
    Underlay,
    /// Stitches that move the needle between parts and are meant to be covered.
    Travel,
    /// Tie-in and tie-off stitches that stop the thread from unravelling.
    Lock,
    /// A trim or a stop.
    Command,
}

/// An element of the plan, by its index in [`StitchPlan::elements`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementRef(u32);

impl ElementRef {
    /// The index into [`StitchPlan::elements`].
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// A reference to the element at `index`, if the index fits.
    pub fn from_index(index: usize) -> Option<Self> {
        u32::try_from(index).ok().map(ElementRef)
    }
}

/// Where a stitch came from: which element, and which part of its stitching.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Provenance {
    /// The element; `None` for plan-level entries.
    pub element: Option<ElementRef>,
    /// The part of the element's stitching.
    pub role: Role,
}

impl Provenance {
    /// Provenance of `element`'s stitches in `role`.
    pub const fn new(element: ElementRef, role: Role) -> Self {
        Provenance { element: Some(element), role }
    }

    /// Provenance of a plan-level entry.
    pub const fn plan(role: Role) -> Self {
        Provenance { element: None, role }
    }
}

/// One entry of a plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stitch {
    /// Where the needle is after this entry.
    pub at: Point,
    /// What the machine does.
    pub kind: StitchKind,
    /// Why it exists.
    pub origin: Provenance,
}

/// The stitches sewn with one thread.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorBlock {
    /// The thread.
    pub thread: Thread,
    /// The entries, in sewing order.
    pub stitches: Vec<Stitch>,
}

/// Everything the machine does, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StitchPlan {
    /// The colour blocks, in sewing order.
    pub blocks: Vec<ColorBlock>,
    /// The elements stitches refer to through [`ElementRef`].
    pub elements: Vec<ElementId>,
}

/// One thread the machine asks for: each block's thread, and the same thread again after each stop
/// (formats without a stop command record a stop as a change to the same thread).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorEntry<'a> {
    /// The thread.
    pub thread: &'a Thread,
    /// Whether this entry is a stop rather than a new block.
    pub stop: bool,
}

/// A stitch that lays thread between two needle holes: a `Normal` stitch whose previous movement was a
/// `Normal` stitch in the same block, with no trim since (a stop keeps the run going; a jump, a trim or a
/// thread change starts a new one). Its length is what the machine and the fabric feel, so stitch-length
/// rules, statistics and previews all use this one definition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SewnStitch {
    /// The colour block.
    pub block: usize,
    /// The entry within the block.
    pub index: usize,
    /// Where the thread comes from (the previous needle hole).
    pub from: Point,
    /// Where the needle goes down.
    pub to: Point,
    /// The stitch's role: the role of the entry where the needle goes down.
    pub role: Role,
    /// The role of the entry the thread comes from (the previous needle hole).
    pub from_role: Role,
}

impl SewnStitch {
    /// Whether it is a lock stitch: one into or out of a lock stitch's needle hole. A tie-in's last stitch
    /// lands on the stitching's first point and a tie-off's first leaves its last point, and both belong to
    /// the lock, whose stitches may be as short as 0.2 mm.
    pub fn is_lock(&self) -> bool {
        self.role == Role::Lock || self.from_role == Role::Lock
    }

    /// The stitch's length in millimetres.
    pub fn length(&self) -> f64 {
        self.from.distance(self.to)
    }
}

/// Counts that describe a plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanStats {
    /// `Normal` stitches.
    pub stitches: usize,
    /// `Jump` entries.
    pub jumps: usize,
    /// `Trim` commands.
    pub trims: usize,
    /// `Stop` commands.
    pub stops: usize,
    /// Thread changes between blocks.
    pub color_changes: usize,
}

impl PlanStats {
    /// Thread changes plus stops: what a format with no separate stop command (PES, DST) has to record
    /// as colour changes.
    pub const fn changes_including_stops(&self) -> usize {
        self.color_changes + self.stops
    }
}

impl StitchPlan {
    /// Every entry, in sewing order.
    pub fn stitches(&self) -> impl Iterator<Item = &Stitch> {
        self.blocks.iter().flat_map(|block| block.stitches.iter())
    }

    /// The bounds of every position in the plan, or `None` for a plan without entries.
    pub fn bounds(&self) -> Option<Rect> {
        Rect::around(self.stitches().map(|s| s.at))
    }

    /// The element `element` refers to.
    pub fn element(&self, element: ElementRef) -> Option<&ElementId> {
        self.elements.get(element.index())
    }

    /// The threads the machine asks for, in order: each block's, and again after each stop.
    pub fn color_entries(&self) -> Vec<ColorEntry<'_>> {
        let mut entries = Vec::new();
        for block in &self.blocks {
            entries.push(ColorEntry { thread: &block.thread, stop: false });
            let stops = block.stitches.iter().filter(|s| s.kind == StitchKind::Stop).count();
            entries.extend(core::iter::repeat_n(ColorEntry { thread: &block.thread, stop: true }, stops));
        }
        entries
    }

    /// Every stitch that lays thread between two needle holes, in sewing order (see [`SewnStitch`]).
    pub fn sewn_stitches(&self) -> Vec<SewnStitch> {
        let mut sewn = Vec::new();
        let (mut needle, mut needle_role) = (Point::ORIGIN, Role::Travel);
        for (block, b) in self.blocks.iter().enumerate() {
            let mut sewing = false;
            for (index, stitch) in b.stitches.iter().enumerate() {
                match stitch.kind {
                    StitchKind::Normal => {
                        if sewing {
                            sewn.push(SewnStitch { block, index, from: needle, to: stitch.at, role: stitch.origin.role, from_role: needle_role });
                        }
                        sewing = true;
                        (needle, needle_role) = (stitch.at, stitch.origin.role);
                    }
                    StitchKind::Jump => {
                        sewing = false;
                        needle = stitch.at;
                    }
                    StitchKind::Trim => sewing = false,
                    StitchKind::Stop => {}
                }
            }
        }
        sewn
    }

    /// Counts of stitches, jumps, commands and thread changes.
    pub fn stats(&self) -> PlanStats {
        let mut stats = PlanStats { color_changes: self.blocks.len().saturating_sub(1), ..PlanStats::default() };
        for stitch in self.stitches() {
            match stitch.kind {
                StitchKind::Normal => stats.stitches += 1,
                StitchKind::Jump => stats.jumps += 1,
                StitchKind::Trim => stats.trims += 1,
                StitchKind::Stop => stats.stops += 1,
            }
        }
        stats
    }
}
