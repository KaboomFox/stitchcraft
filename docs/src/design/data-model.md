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
    pub blocks: Vec<ColorBlock>,
    pub profile_id: ProfileId,
}

pub struct ColorBlock {
    pub thread: Thread,
    pub stitches: Vec<Stitch>,
}

pub struct Stitch {
    pub at: Point,
    pub kind: StitchKind,
    pub origin: Provenance,
}

pub enum StitchKind { Normal, Jump, Trim, Stop, ColorChange, End }

pub struct Provenance {
    pub element: Option<ElementId>,  // None for plan-level stitches (e.g. final End)
    pub role: Role,                  // Underlay, Top, Travel, Lock, Command
}
```

`Trim`, `Stop`, `ColorChange` and `End` are *commands at a position*; encoders decide how a format
expresses them (PEC has trim-flagged jumps; DST expresses trims as jump sequences).

### Plan invariants

Conformance level L0 checks these on every plan the suite produces (requirements `REQ-PLAN-001` to
`REQ-PLAN-007`, in this order).

1. All coordinates finite and inside the profile's hoop.
2. Every `Normal` stitch is ≤ `profile.max_stitch` and ≥ `settings.min_stitch_len`, except lock
   stitches, which are ≥ 0.2 mm.
3. Colour blocks are non-empty; a `ColorChange` separates every pair of blocks; the plan ends with
   exactly one `End`.
4. A `Trim` is preceded by a tie-off and followed (before the next `Normal`) by a tie-in, when the
   element's lock settings ask for them.
5. No two consecutive identical positions among `Normal` stitches.
6. Colour count ≤ the format's maximum (PEC: 255 colour changes).
7. Provenance: every non-command stitch names its element.

## Threads and palettes

```rust,ignore
pub struct Thread {
    pub color: Rgb8,
    pub name: Option<String>,
    pub catalog: Option<CatalogRef>,  // e.g. Brother 001 "White"
}
```

Palettes are static tables (Brother PEC palette first). Nearest-colour matching uses CIEDE2000 in
CIELAB with a fixed white point, implemented with `libm` so it is deterministic. Tables carry their
source in a comment and a row in `NOTICE` where they derive from a third-party list (for example the
PEC palette as published in MIT-licensed pyembroidery).

## Machine profiles

```rust,ignore
pub struct MachineProfile {
    pub id: ProfileId,               // "brother-200x200"
    pub name: String,                // "Brother, 200 × 200 mm hoop"
    pub hoop: Hoop,                  // { width: 200 mm, height: 200 mm }
    pub comfort: Option<Hoop>,       // { 150 mm, 150 mm } — warning only
    pub format: FormatId,            // Pes { version: 1 }
    pub max_stitch: Mm,              // 12.0
    pub min_stitch: Mm,              // 0.3
    pub max_jump: Option<Mm>,        // None: the encoder splits long jumps per format limits
    pub trims: TrimSupport,          // Command | JumpThreshold(Mm) | Unsupported
    pub palette: PaletteId,          // BrotherPec
    pub max_colors: u16,
}
```

Profiles are data in `stitchcraft-plan/src/profiles/`, listed by `stitch profiles` and documented by
generated reference pages. A profile's values are *machine facts*: changing one requires a
machine-testing record that justifies it ([machine testing](../plan/machine-testing.md)).

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
