# Data model

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
  the root `viewBox`/`width`/`height` (Ink/Stitch documents assume 96 px per inch, `PIXELS_PER_MM =
  96 / 25.4`).
- **Quantization happens once, in the encoder**, on absolute positions: `round_half_even(x * 10)`.
  Deltas are differences of quantized absolutes, so rounding never accumulates.
- **Axis conventions per format** are the encoder's job (DST is y-up). Test sheet TS-01 (an
  asymmetric "F" with a ruler) proves orientation and scale on the machine.

## Engine input: `Design` (`stitchcraft-engine`)

```rust,ignore
pub struct Design {
    pub elements: Vec<Element>,       // stitching order (host paint order, bottom first)
    pub settings: DesignSettings,
    pub profile: MachineProfile,
}

pub struct Element {
    pub id: ElementId,               // stable host id: "svg:path123", "vc:42"
    pub name: Option<String>,
    pub shape: Shape,
    pub thread: Thread,
    pub params: ParamSet,            // validated against the registry for `shape`'s stitch types
    pub commands: Commands,
}

pub enum Shape {
    Region(Region),                  // fills: one or more polygons with holes, fill rule resolved
    Path(StrokePath),                // running/bean/manual/zigzag/ripple
    Satin(SatinShape),               // two rails + rungs, or a centre line + width
}

pub struct Commands {
    pub start: Option<Point>,         // "starting point" command
    pub end: Option<Point>,           // "ending point" command
    pub target: Option<Point>,        // ripple/circular target
    pub trim_after: bool,
    pub stop_after: bool,
    pub ignore: bool,
}

pub struct DesignSettings {
    pub collapse_len: Mm,             // jumps shorter than this become stitches (default 3.0 mm)
    pub min_stitch_len: Mm,           // shorter stitches are merged (default 0.1 mm; profile may raise it)
    pub origin: Option<Point>,        // machine origin; default: centre of the design's bounds
    pub stop_position: Option<Point>,
}
```

Invariants: element ids are unique; regions are valid (no self-intersections, rings closed,
orientation normalized: outer counter-clockwise in y-down screen space is *not* assumed — the
normalizer orients explicitly); every coordinate is finite and within ±10,000 mm.

### Region

`Region` is a set of `Polygon { exterior: Ring, holes: Vec<Ring> }` produced by the normalizer from
the host's paths and fill rule (even-odd or non-zero). Curves are flattened with the element's
tolerance before booleans. Rings with area below `min_region_area` (default 0.01 mm²) are dropped with
`SC-W0303`.

### SatinShape

```rust,ignore
pub enum SatinShape {
    Rails { a: Polyline, b: Polyline, rungs: Vec<Segment> },
    Centerline { path: Polyline, width: Mm },   // single-path satin
}
```

How rails and rungs are recognised from a host path is part of the [satin design](algorithms/satin.md).

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
| `REQ-PLAN-002` | While sewing, every stitch is between the profile's `min_stitch` (locks: 0.2 mm) and `max_stitch`. | M1 |
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
matched. Tables carry their source in the module docs and a row in `NOTICE` (the PEC palette as published
in MIT-licensed pyembroidery). Thread catalogues (brand and number) arrive with M11.

## Machine profiles

```rust,ignore
pub struct MachineProfile {
    pub id: &'static str,            // "brother-200x200"
    pub name: &'static str,          // "Brother, 200 × 200 mm hoop"
    pub hoop: Size,                  // 200 × 200 mm
    pub comfort: Option<Size>,       // 150 × 150 mm — a warning beyond it, not an error
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
