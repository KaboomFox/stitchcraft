# Data model

<!-- implements: crates/stitchcraft-core/src/lib.rs, crates/stitchcraft-core/src/units.rs, crates/stitchcraft-core/src/element.rs, crates/stitchcraft-core/src/rect.rs, crates/stitchcraft-core/src/budget.rs, crates/stitchcraft-plan/src/*.rs, crates/stitchcraft-plan/src/palette/**, crates/stitchcraft-plan/src/profiles/**, crates/stitchcraft-engine/src/design.rs -->

The types every crate shares. Code sketches are illustrative (`ignore`); the crates' rustdoc is the
API reference once the types exist. Invariants listed here are checked in code (constructors return
`Result`) and by conformance level L0.

## Units and coordinates (`stitchcraft-core`)

```rust,ignore
/// A length in millimetres. Always finite; construction from untrusted input is checked.
pub struct Mm(f64);

/// A point in millimetres, y pointing down (SVG and VectorCraft convention).
pub struct Point { pub x: f64, pub y: f64 }
```

- **Internal unit: millimetres.** Embroidery parameters are specified in mm and machine files use
  0.1 mm, so mm keeps the numbers people read and the numbers we compute the same.
- **Host units are converted at the adapter:** VectorCraft points ×25.4/72; SVG user units through
  the root `viewBox`/`width`/`height`, with CSS pixels at 96 per inch.
- **Quantization happens once, in the encoder**, on absolute positions: `round_half_even(x * 10)`.
  Deltas are differences of quantized absolutes, so rounding never accumulates.
- **Axis conventions per format** are the encoder's job (DST is y-up). Test sheet TS-01 (an
  asymmetric "F" with a ruler) proves orientation and scale on the machine.

## Engine input: `Design` (`stitchcraft-engine`)

Hosts translate their documents into a `Design` (`stitchcraft_engine::design`), so the engine never sees a
host document and every host gets the same stitches for the same design. Since M3.3:

```rust,ignore
pub struct Design { elements: Vec<Element>, pub settings: DesignSettings }   // checked by Design::new

pub struct Element {
    pub id: ElementId,               // stable host id: "svg:path123:fill", "vc:42"
    pub name: Option<String>,        // the name the user gave it (Inkscape's label)
    pub shape: Shape,
    pub thread: Thread,
    pub params: ParamSet,            // as the host stores them; validated against the registry when planned
}

pub struct DesignSettings {          // since M3.8; read from an Ink/Stitch file since M4.8, its commands from M8
    pub collapse_len: Mm,            // 3 mm: same-thread moves no longer than this are sewn on, not jumped
    pub min_stitch_len: Option<Mm>,  // the design's shortest stitch, for elements that set none
    pub origin: Option<Point>,       // where the hoop's centre goes; None: the centre of the stitches
    pub stop_position: Option<Point>,// where the frame moves before each stop; None: it stays
    pub min_satin_stroke_width: Mm,  // 1 mm: a satin column drawn as one path no wider is sewn as a stroke (M4.8)
}

pub enum Shape {
    Stroke { path: Path, width: Mm, join: Join },  // an outline: running stitch and the other stroke methods
    Fill { path: Path, rule: FillRule },           // an area: its boundary, and which parts are inside
}

pub enum Join { Miter { limit: f64 }, Round, Bevel }   // a stroke's corners; Join::UNSET is a miter limited at 5

pub struct Path { pub subpaths: Vec<Subpath> }   // in millimetres, y down
pub struct Subpath { pub start: Point, pub segments: Vec<Segment>, pub closed: bool }
pub enum Segment { Line(Point), Quad(Point, Point), Cubic(Point, Point, Point) }
```

Elements are in stitching order: the host's paint order, bottom first. Geometry stays exact, curves with
their control points, and the generators flatten it with the tolerance their parameters give.

A stroke's width and join, since M4.8, are as the host draws them: in SVG, `stroke-width` scaled by the
transforms, and `stroke-linejoin` with `stroke-miterlimit` ([SVG input](svg-input.md#stroke-width-and-join)).
Only a satin column drawn as one path uses them, as its width and its corners. `Shape::stroke(path)`
makes a stroke that says nothing of either: 1 CSS pixel wide, with `Join::UNSET`.

Invariants, checked by `Design::new`: element ids are unique, and every point is finite and within
±10,000 mm, as far as machine-file coordinates go. No stroke is narrower than 0, a miter limit is a
finite number of at least 1, and the settings' lengths are not negative. Adapters drop what they cannot
represent with a diagnostic of their own, so a failed check is a bug in the adapter (`SC-E0009`).

**Commands** attached to elements come with M8, for Ink/Stitch's command symbols such as start and end
points. Trims and stops are the element parameters `trim_after` and `stop_after`, and the origin and
stop position are design settings (M3.8).

### Region

A fill's region is not part of the design: the normalizer builds it from the fill's path and fill rule,
since M5.1 (`stitchcraft_engine::normalize::region`):

```rust,ignore
pub struct Region { pub parts: Vec<Polygon> }   // in the order the drawing reaches them
pub struct Polygon { pub outline: Vec<Point>, pub holes: Vec<Vec<Point>> }   // closed rings, in millimetres
```

Each ring starts at its point that comes first in the drawing, and repeats it at its end. Outlines turn
clockwise and holes counter-clockwise in y-up axes, and in those axes the region lies on the right of
every ring. Parts of 3 square CSS pixels (0.21 mm²) or less are left out, with `SC-W0303`. The
[fills design](algorithms/fills.md#region) builds the region step by step.

### SatinShape

A satin column's path is recognized, since M4.1, as one of 2 shapes (`stitchcraft_engine::normalize::satin`):

```rust,ignore
pub enum Shape {
    Rails(Satin),   // 2 rails as polylines, and what pairs their points: rungs, or the rails' nodes
    CentreLine { line: Vec<Point>, closed: bool },  // one subpath, flattened; closed when the path ends with Z
}
```

A centre line is made into a `Satin` of its own, its rails offset from the line by the stroke's width and
join (`stitchcraft_engine::normalize::centre_line`, since M4.9). How rails and rungs are recognised from
a host path, and made from a centre line, is part of the [satin design](algorithms/satin.md).

## Parameters

`ParamSet` maps registry keys to typed values; generators receive *typed views*
(`TatamiParams { row_spacing: Mm, angle: Deg, … }`) produced by the registry. See
[parameter registry](params.md).

## Engine output: `StitchPlan` (`stitchcraft-plan`)

```rust,ignore
pub struct StitchPlan {
    pub blocks: Vec<ColorBlock>,       // sewing order; a thread change between blocks, the end after the last
    pub elements: Vec<ElementId>,      // what `ElementRef`s point at
}

pub struct ColorBlock {
    pub thread: Thread,
    pub stitches: Vec<Stitch>,
}

pub struct Stitch {
    pub at: Point,                     // where the needle is after this entry (mm, y down, hoop centre = 0,0)
    pub kind: StitchKind,
    pub origin: Provenance,
}

pub enum StitchKind { Normal, Jump, Trim, Stop }

pub struct Provenance {
    pub element: Option<ElementRef>,   // index into `elements`; None for plan-level entries
    pub role: Role,                    // Top, Underlay, Travel, Lock, Command
}
```

- **Colour changes and the end are structure, not entries.** The machine changes thread between two
  blocks and ends after the last one, so a plan cannot misplace either; encoders write exactly one
  colour change between blocks and one end.
- **`Trim` and `Stop` happen where the needle is;** their `at` repeats the needle position so every entry
  has one (bounds, previews and reports never special-case commands), and the invariant checker verifies
  it. Encoders decide how a format expresses them (PEC: trim-flagged jumps, and a stop as a change to the
  same thread; DST: trims as a jump sequence) — see [formats](formats.md).
- **The origin is the centre of the hoop**, where the needle starts, above the fabric. The first
  `Normal` stitch is the first time it goes down.
- `PlanBuilder` appends entries with the needle tracked; `PlanStats` counts stitches, jumps, trims, stops
  and thread changes for reports.
- A plan does not name a profile: it is checked against one, `invariants::check(&plan, &profile)`.

### Plan invariants

Conformance level L0 checks these on every plan the suite produces. A violation is a bug in whatever
built the plan: the command line reports it as `SC-E0009` and writes nothing. "While sewing" means the
previous movement was a `Normal` stitch with no trim since — the stitch lays thread between two holes,
and its length is what the machine and fabric feel; the first stitch after a jump, a trim or a thread
change starts a new run.

| Requirement | Rule | Since |
|---|---|---|
| `REQ-PLAN-001` | Every position is finite and inside the profile's hoop, centred on the origin. | M1 |
| `REQ-PLAN-002` | While sewing, every stitch is between the profile's `min_stitch` (lock stitches, into or out of a lock point: 0.2 mm) and `max_stitch`. | M1 |
| `REQ-PLAN-003` | Every block sews at least one stitch; trims and stops happen where the needle is. | M1 |
| `REQ-PLAN-004` | A `Trim` is preceded by a tie-off and followed by a tie-in, when the element's lock settings ask for them. | M3 |
| `REQ-PLAN-005` | While sewing, no stitch lands where the needle already is. | M1 |
| `REQ-PLAN-006` | Thread changes plus stops fit the format (PES: 255; DST: 999). | M1 |
| `REQ-PLAN-007` | Every `Normal` and `Jump` entry names its element. | M3 |

## Threads and palettes

```rust,ignore
pub struct Thread {
    pub color: Rgb,                  // what the user chose
    pub name: Option<String>,        // shown to the operator
}
```

Palettes are static tables (the Brother PEC palette first) of `PaletteEntry { index, name, color,
matchable }`. Nearest-colour matching uses CIEDE2000 in CIELAB (D65), through `stitchcraft_core::math`
so it is deterministic, with ties going to the lower index; it is tested against the 34 reference pairs
of Sharma, Wu & Dalal (2005). Entries that are not threads — Brother's applique steps 62–64 — are never
matched. Each table names its source in its module docs and in a row of `NOTICE`. The PEC palette is the one
MIT-licensed pyembroidery publishes, with the names of entries 62 and 63 from its fork pystitch. Thread
catalogues (brand and number) arrive with M11.

## Machine profiles

```rust,ignore
pub struct MachineProfile {
    pub id: &'static str,            // "brother-pe800-5x7"
    pub name: &'static str,          // "Brother PE800 with its 5 × 7 in hoop"
    pub hoop: Size,                  // 130 × 180 mm
    pub comfort: Option<Size>,       // None — a warning beyond it, not an error
    pub format: FormatId,            // PesV1 | Dst
    pub max_stitch: Mm,              // 12.0
    pub min_stitch: Mm,              // 0.3
    pub trims: TrimSupport,          // Command | LongJumps(Mm) | None
    pub palette: PaletteId,          // BrotherPec
    pub evidence: &'static str,      // where the values come from
}
```

Profiles are data in `stitchcraft-plan/src/profiles/`, listed by `stitch profiles` and documented by
generated reference pages. A profile's values are *machine facts*: changing one requires a
machine-testing record that justifies it ([machine testing](../plan/machine-testing.md)), and `evidence`
says where each value comes from. Format limits that are not machine facts (the most colour changes a
format records) live on `FormatId`, so the invariant checker and the encoders read the same number.

A profile is a machine with one hoop, so a machine with 3 hoops has 3 profiles. `profiles::REFERENCE` is
the reference machine's, the Brother PE800 with its 5 × 7 in hoop
([ADR 0013](adr/0013-brother-pe800-reference-machine.md)). Code that means the machine the checkpoints
run on imports it rather than naming a hoop: the command line plans for it unless told otherwise. Each
test sheet names the profile of the hoop it is sewn in.

`check_fit(bounds)` turns the design's size into `SC-E0701` (larger than the hoop) or `SC-W0702` (larger
than the comfort zone), with "rotate 90°" as a one-click fix when turning the design would fit
(`REQ-PRF-002`).

## Budgets

```rust,ignore
pub struct Budget {
    pub max_stitches: u32,           // per design, default 2,000,000
    pub max_work: u64,               // abstract work units, decremented by generators
}
```

Work units are counted per inner loop iteration (scanline crossings, graph edge visits, offset
steps) so a budget behaves identically on every machine. Exhaustion yields `SC-E0004` for the
element; other elements still plan.
