# Data model

<!-- implements: crates/stitchcraft-core/src/lib.rs, crates/stitchcraft-core/src/units.rs, crates/stitchcraft-core/src/element.rs, crates/stitchcraft-core/src/rect.rs, crates/stitchcraft-core/src/budget.rs, crates/stitchcraft-plan/src/*.rs, crates/stitchcraft-plan/src/palette/**, crates/stitchcraft-plan/src/profiles/**, crates/stitchcraft-engine/src/design.rs, crates/stitchcraft-engine/src/design/turn.rs -->

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
pub struct Design {                  // checked by Design::new
    elements: Vec<Element>,
    pub settings: DesignSettings,
    pub sort_colours: bool,          // since M13.3: the elements with settings sew in the order StitchCraft chooses too
    pub marks: Vec<Mark>,            // since M13.39: areas whose parts automatic digitizing keeps, never sewn
    pub guides: Vec<Mark>,           // since M13.24: lines whose rows the filled parts they cross follow, never sewn
}

pub struct Element {
    pub id: ElementId,               // stable host id: "svg:path123:fill", "vc:42:fill"
    pub name: Option<String>,        // the name the user gave it (Inkscape's label)
    pub shape: Shape,
    pub thread: Thread,
    pub params: ParamSet,            // as the host stores them; validated against the registry when planned
    pub automatic: bool,             // since M13.1: a shape without settings, read with --auto, whose stitches StitchCraft chooses
    pub lettered: Option<Lettered>,  // since M13.50: lettering set in a font (Set) or placed by digitizing (Digitized); no file holds it
    pub hints: Hints,                // since M8.4: where its design says it starts and ends, the point it aims at, its guide
    pub clips: Vec<Clip>,            // since M8.5: it sews only what lies inside every one (REQ-GEN-005)
}

pub struct Clip {                    // since M8.5: an SVG clip path, and from M8.7 a VectorCraft clipping object
    pub shapes: Arc<[ClipShape]>,    // their areas together, shared by the elements it cuts; none leaves nothing
}

pub struct ClipShape {
    pub path: Path,                  // in the design's millimetres
    pub rule: FillRule,              // which parts of it are inside, as a fill's
}

pub struct Hints {                   // since M8.4: Ink/Stitch's start, end and target commands, VectorCraft's markers
    pub start: Option<Point>,        // its start position, in place of the needle before it (REQ-GEN-001)
    pub end: Option<Point>,          // its end position, in place of the next element's offer
    pub target: Option<Point>,       // what a circular fill turns round and a ripple aims at (REQ-FILL-CIR-008, REQ-RIP-002)
    pub guide: Option<Guide>,        // since M10.11: the line a ripple's copies follow (REQ-RIP-012, REQ-RIP-013)
    pub anchor: Option<Path>,        // since M10.11: the anchor line. Its first subpath's ends are where a satin
                                     // guide's copies land (REQ-RIP-014), and from M10.10 a guided fill's
                                     // closed rows start where any of it crosses them (REQ-FILL-GDD-012)
}

pub struct Guide {                   // since M10.11: Ink/Stitch's guide line, VectorCraft's stitch:guide marker
    pub path: Path,                  // in the design's millimetres: a line, or a satin guide's rails and rungs
    pub width: Mm,                   // its stroke's width, for a satin guide drawn as one path
    pub join: Join,                  // how its stroke turns its corners
    pub params: ParamSet,            // its own settings, which say whether it is a satin column and how to read it
}

pub struct Reading {                 // since M13.1: what every host's reader is asked
    pub auto: bool,                  // --auto, or a VectorCraft document without --no-auto
    pub sort_colours: bool,          // since M13.3: --sort-colours, which the reader passes on (Design::read_as)
}

pub struct DesignSettings {          // since M3.8; read from an Ink/Stitch file since M4.8, its commands from M8,
                                     // and from a VectorCraft document since M6.11
    pub collapse_len: Mm,            // 3 mm: same-thread moves no longer than this are sewn on, not jumped
    pub min_stitch_len: Option<Mm>,  // the design's shortest stitch, for elements that set none
    pub origin: Option<Point>,       // where the hoop's centre goes; None: the centre of the stitches
    pub stop_position: Option<Point>,// where the frame moves before each stop; None: it stays
    pub min_satin_stroke_width: Mm,  // 1 mm: a satin column drawn as one path no wider is sewn as a stroke (M4.8)
    pub rotate_on_export: Turn,      // the plan turned about the hoop's centre as it is written (M8.9); none by default
    pub cap: Option<auto::Held>,     // what the cap holds the parts digitizing made to, which Expand keeps (M13.26)
}

pub enum Shape {
    Stroke { path: Path, width: Mm, join: Join, cap: Cap },  // an outline: running stitch and the other stroke methods
    Fill { path: Path, rule: FillRule, gradient: Option<Gradient> },  // an area: its boundary, which parts are inside,
                                                                      // and since M10.12 its linear gradient
}

pub struct Gradient {                // since M10.12: a fill's linear gradient, placed by its host
    pub start: Point,                // where offset 0 lies, in the design's millimetres, every transform applied
    pub end: Point,                  // where offset 1 lies
    pub stops: Vec<GradientStop>,    // 2 or more, in order
}

pub struct GradientStop {
    pub offset: f64,                 // from 0 to 1, never before the stop before
    pub colour: Rgb,
    pub opacity: f64,                // from 0 to 1; 0 is transparent
}

pub enum Join { Miter { limit: f64 }, Round, Bevel }   // a stroke's corners; Join::UNSET is a miter limited at 5
pub enum Cap { Butt, Round, Square }                  // since M13.1: how a stroke ends where an open subpath ends

pub struct Path { pub subpaths: Vec<Subpath> }   // in millimetres, y down
pub struct Subpath { pub start: Point, pub segments: Vec<Segment>, pub closed: bool }
pub enum Segment { Line(Point), Quad(Point, Point), Cubic(Point, Point, Point) }
```

Elements are in the host's paint order, bottom first. They sew in that order unless the design has
automatic elements or `sort_colours` is set: since M13.3 automatic digitizing then chooses the order
([image to embroidery](image-to-embroidery.md#sewing-order)). From M10.15 a run of elements that set
`underlays_first` sews in that order twice, its underlays first and its top stitches after
([engine pipeline](engine-pipeline.md#underlays-first)). Curves keep their control points, and the
generators flatten them with the tolerance their parameters give.

A design's marks (`design::mark::Mark`, an id and a shape) are no elements: a reader takes the shapes named
`stitch:keep` out of its elements and gives them to the design, and automatic digitizing keeps the parts
wholly inside them ([marks that keep parts](image-to-embroidery.md#marks-that-keep-parts)).
Its guides are no elements either: the shapes named `stitch:guide`, taken out beside the marks, whose
lines the filled parts of automatic areas they cross follow ([a guide on one shape](image-to-embroidery.md#a-guide-on-one-shape)).
`Design::with_elements` gives a design other elements and keeps its settings, its marks and its guides.

A host reads 3 of the settings by Ink/Stitch's names, `collapse_len_mm`, `min_stitch_len_mm` and
`min_satin_stroke_width_mm`: an SVG file's metadata, and since M6.11 the data a VectorCraft document keeps
for StitchCraft's Design Settings filter. `SETTINGS` lists them with the label and help that filter's
dialog shows, and `Setting::get` and `Setting::set` read and write each in millimetres. A shortest stitch
of 0 is `None`, which leaves it to the machine.

From M13.26 a design also keeps what the cap at the end of planning holds the elements digitizing made to:
the limits of its thickness and the overlap of its areas, by the names of the Auto effect's settings
(`auto::Held::KEYS`). Expand writes them from the lowest automatic shape, and both hosts read them where
they read the other settings ([Expand](image-to-embroidery.md#expand)). `None` leaves them to the lowest
automatic shape, or to the Auto effect's defaults.

A stroke's width and join, since M4.8, are as the host draws them: in SVG, `stroke-width` scaled by the
transforms, and `stroke-linejoin` with `stroke-miterlimit` ([SVG input](svg-input.md#stroke-width-and-join)).
Only a satin column drawn as one path uses them, as its width and its corners. `Shape::stroke(path)`
makes a stroke that says nothing of either: 1 CSS pixel wide, with `Join::UNSET`. Its cap, since M13.1, is
SVG's `stroke-linecap` or VectorCraft's cap, `Butt` when nothing sets it. Ink/Stitch sews no cap, and
neither does an element with settings. An automatic element's stitches follow it
([image to embroidery](image-to-embroidery.md#stitch-types-by-width)).

An automatic element is a shape without settings that the user asked StitchCraft to digitize: the readers
mark it when `Reading::auto` is set, and the engine turns it into ordinary elements with the settings its
width calls for before anything is generated (`stitchcraft_engine::auto`).

Documents repeat ids, so adapters make theirs unique with `stitchcraft_core::Labels`, which gives a
label repeated in a document `~2`, `~3` and so on, in every adapter alike.

Invariants, checked by `Design::new`: element ids are unique, and every point is finite and within
±10,000 mm, as far as machine-file coordinates go, a gradient's start and end among them. No stroke is
narrower than 0, a miter limit is a finite number of at least 1, and the settings' lengths are not negative.
A gradient has 2 stops or more, its offsets and opacities from 0 to 1, each offset at or after the one before. Adapters drop what they cannot
represent with a diagnostic of their own, so a failed check is a bug in the adapter (`SC-E0009`).
`Design::moved` gives the design moved by an offset, every point of its elements, their gradients' among
them, and its origin and stop position, checked the same way. VectorCraft's Test Sheet filter draws a test sheet in the middle of the
artboard with it (`REQ-VC-015`).

**Gradients.** A fill painted with a linear gradient keeps it in its shape (M10.12), as the gradient's
start and end with its stops, each an offset, a colour and an opacity. A linear gradient fill sews its colours
([gradient fill](algorithms/gradient.md)), and every other method sews the element's thread, which the host
reads as the first colour a viewer shows of it. Before it gives the 2 points in the design's millimetres, as it
gives the path, the host applies the gradient's own units and transforms and the shape's. A host's paint
rules stay in its adapter, and the points move with the path. Whatever builds a fill anew from another, a design
moved or a sheet's clone of a fill, must say what becomes of its gradient, and the type makes it say so.
These other places were weighed and left:

- A field of the element beside its thread. A stroke has no use for it, and code that copies an element
  with `..element.clone()` and moves its path would keep its gradient where it was, without a word.
- A kind of shape of its own beside `Fill`. Every reader of a fill's area would then have 2 kinds to read.
- A setting among the element's parameters. The registry, the dialogs and the files' settings would hold a
  paint, and no host keeps one there.
- The gradient as its host writes it, with its units and its transforms, placed by the engine. Each host
  has rules of its own for that, as SVG's bounding box units and VectorCraft's placement fitted to a box,
  and the adapters already apply them to paths.

**Commands.** Ink/Stitch's start, end and target commands, and VectorCraft's start, end and target markers,
are an element's hints (M8.4, the target marker M10.8), its points checked and moved with the rest. So are
Ink/Stitch's guide lines and anchor lines, and VectorCraft's `stitch:guide` and `stitch:anchor` markers
(M10.11), a guide's and an anchor's every point included. Clip
paths are an element's clips (M8.5), and so are VectorCraft's clipping objects (M8.7), checked and moved
with it too, which the engine cuts it to ([engine pipeline](engine-pipeline.md)). A clip's shapes are shared by the elements it cuts, so
that a clip group of many objects holds its clipping object once. Trims and stops are the element
parameters `trim_after` and `stop_after`, and the origin and stop position are design settings (M3.8), which
Ink/Stitch's `origin` and `stop_position` commands and VectorCraft's `stitch:origin` and `stitch:stop-position`
markers give (M8.8).

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
  it. Encoders decide how a format expresses them. PEC puts a trim flag on every jump after the first
  move, and records a stop as a change to the same thread. DST spells a trim as a run of jumps. See
  [formats](formats.md).
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
change starts a new run. A length or a position within `LENGTH_SLACK` (10⁻⁹ mm) of a limit counts as at
it, at the hoop's edge as for a stitch's length, so floating-point rounding cannot push a stitch placed
exactly at a limit past it. Generators place stitches by the same test, and so does the check that a design
fits the hoop. The rules hold for the plan. A machine file rounds its positions to 0.1 mm, and its
stitches are within 0.14 mm of the plan's ([formats](formats.md), `REQ-FMT-002`). The colour changes a plan may
have are those of the format it is written in: `MachineProfile::writing` gives the machine's profile for
another format than its own (`REQ-PLAN-006`).

| Requirement | Rule | Since |
|---|---|---|
| `REQ-PLAN-001` | Every position is finite and inside the profile's hoop, centred on the origin. | M1 |
| `REQ-PLAN-002` | While sewing, every stitch is between the profile's `min_stitch` (lock stitches, into or out of a lock point: 0.2 mm) and `max_stitch`. | M1 |
| `REQ-PLAN-003` | Every block sews at least one stitch; trims and stops happen where the needle is. | M1 |
| `REQ-PLAN-005` | While sewing, no stitch lands where the needle already is. | M1 |
| `REQ-PLAN-006` | Thread changes plus stops fit the format (PES: 255; DST: 999). | M1 |
| `REQ-PLAN-007` | Every `Normal` and `Jump` entry names an element the plan lists. A trim or a stop need not name one. | M3 |

`REQ-PLAN-004`, a tie-off before a trim and a tie-in after it, is retired. Whether they belong there
depends on the elements' lock settings, which a plan does not hold. Assembly's cases check it with the
settings (`REQ-ASM-003`, `REQ-LCK-001`).

### The thread's state

The invariants follow the needle. A machine reading a plan is also in a state of thread, and that state
decides where locks belong. M13.56 wrote it down, from the owner's question on 2026-11-21, and checks its
rules, `REQ-PLAN-008` to `REQ-PLAN-010`, in every plan the engine returns.

| Entry | Needle | Sewing | Thread, `Command` | Thread, `LongJumps(l)` | Thread, `None` (PE800) |
|---|---|---|---|---|---|
| The plan's start | at the origin, up | no | cut: a new end | cut | cut |
| `Normal` | moves to `at`, down | yes, from the second in a row | attached | attached | attached |
| `Jump` | moves to `at`, up | no | attached: a jump thread | cut when the next needle point is farther than `l` from the last | attached: a jump thread the operator snips |
| `Trim` | stays | no | cut | attached (no trim command) | attached (MC-1: the PE800 cuts no jump, trim-flagged or not) |
| `Stop` | stays | no | cut, the same thread again | cut | cut (MC-1) |
| A thread change | stays | no | cut, a new thread | cut | cut |
| The plan's end | stays | no | cut | cut | cut |

The thread is either attached to the fabric or cut. Each time it is cut, or may be (an operator snips the
jump threads the PE800 leaves, `plan/machine-testing.md`), the stitching before it needs a tie-off and the
stitching after it a tie-in, where the elements' `ties` set them. That is why plan assembly ties off and in
round every jump, whatever the machine: the locks hold where a machine or a person cuts.

The model is one type, `stitchcraft_plan::ThreadState`. It reads a plan entry by entry, keeps where the
needle is, whether a run is open and whether the thread has sewn since it was last cut, and says what each
entry did, from a stitch sewn from the hole before it to a stop. A thread change
ends the run and cuts. `StitchPlan::sewn_stitches`, `invariants::check` and the engine's lock check read
the plan through it, and the preview's scene (`stitchcraft-render`) reads `sewn_stitches`. The scene
keeps the thread's state by `TrimSupport` on top, and draws each hole as reached by a stitch, by a jump
thread or across a cut. The lock check reads an element's lock settings by the rule that plan assembly
places locks by (`REQ-LCK-001`).

Before M13.56 the model was in 4 places, which disagreed in 2:

- **A stop.** `sewn_stitches` kept a run going across a stop. The scene cut there, as the PE800 does,
  and encoders write a stop as a change to the same thread. Assembly always jumps after a stop, so only a
  sheet drawn stitch by stitch shows it: TS-02 sews on after its stop. A stop now ends the run
  (`REQ-PLAN-010`). The stitch after TS-02's stop is no longer a sewn stitch for the length rules and the
  thickness measure. Its machine files are byte for byte as before, and so is its preview, where the
  scene already drew that hole as reached across a cut.
- **The cap.** Finalize checks the invariants (`REQ-FIN-003`). Then an automatic design is thinned and
  capped ([how thick a design sews](image-to-embroidery.md#how-thick-a-design-sews)), and those passes take
  needle points out, lock stitches among them, and the plan went out unchecked after them. The
  [check](engine-pipeline.md#6-check) now runs on the plan that is returned, after the cap.

**What M13.56 measured.** A walk of every finished plan with this model, the elements' lock settings
read from the design: the 26 test sheets, every conformance fixture and repository document planned plain,
automatic, and in the photo and logo styles (240 plans), and 18 local traced photos in the photo style,
9 subjects at 6 and 12 threads. Of their 5,333 runs with settings, 0 lack a lock that their element's
`ties` sets. Every trim follows a stitch sewn since the last cut, and every returned plan passes the
invariants after the cap too. On automatic designs the thinning pass leaves a lock one back and forth
(`REQ-AUTO-074`): 2 needle points where assembly sewed 4, at 2,230 of the 5,333 tie-ins. The only stops
right after a trim are TS-11A's, an element trimmed and then stopped after, and both cut. The 5 sheets
drawn stitch by stitch, TS-01, TS-02 and TS-10A to TS-10C, have no locks and no settings, so the check
wants no lock of them.

**The checks.** `REQ-PLAN-009` and `REQ-PLAN-010` are plan rules, in `invariants::check` and
`sewn_stitches`. `REQ-PLAN-008` needs the lock settings, which a plan doesn't hold, so it is the engine's
(`stitchcraft-engine`'s `check` module). It walks the plan the engine returns once the cap is done, with
each element's lock settings from its generated parameters, and reports a missing lock as `SC-E0009`, as a
broken invariant is, and never repairs the plan. A run of one needle point lays no thread, so the check
skips it, as assembly sews it without locks (`REQ-ASM-007`). The walk is linear in the plan's length.

The check found one real gap, in a random degenerate design of `degenerate.rs` and in none of the 284 plans
of the set. A guided fill sewed its last group, of one needle point, straight on from the group before
it. Its tie-off was made from that group alone, which has no stitch to lock, so the run ended with
no tie-off. From M13.56 a tie-off is made from the run's own points
([lock stitches](algorithms/locks.md#placement)).

### How thick a plan sews

`stitchcraft_plan::thickness` measures a plan's thread, in fills, and its needle points per mm² in the
square millimetre round each spot. It lives beside the plan, so that the engine's warning and the
renderer's heat map are one measure, which the test kit reads too. [Image to embroidery](image-to-embroidery.md#how-thick-a-design-sews)
defines the 2 measures and their limits.

## Threads and palettes

```rust,ignore
pub struct Thread {
    pub color: Rgb,                  // what the user chose
    pub name: Option<String>,        // shown to the operator
}
```

An element has one thread of its own, `Element::thread`, which its host reads from its paint. From M10.5 its
generator may sew some of its groups in threads of their own, as tartan and linear gradient fills do. The
element's stitches then lie in several colour blocks of the plan, each block in one thread, and each stitch
still names its element ([engine pipeline](engine-pipeline.md#several-threads-in-one-element)).

Palettes are static tables (the Brother PEC palette first) of `PaletteEntry { index, name, color,
matchable }`. Nearest-colour matching uses CIEDE2000 in CIELAB (D65), through `stitchcraft_core::math`
so it is deterministic, with ties going to the lower index; it is tested against the 34 reference pairs
of Sharma, Wu & Dalal (2005). Entries that are not threads — Brother's applique steps 62–64 — are never
matched. A format that names threads by entries gives a design's threads theirs together
(`Palette::assign`, REQ-THREAD-002). Different threads get different entries while they last, the nearest
pairs first, and a thread read from such a file gets its own ([formats](formats.md), PES). Each table names
its source in its module docs and in a row of `NOTICE`. The PEC palette is the one
MIT-licensed pyembroidery publishes, with entries 62 and 63 from its fork pystitch. A palette's threads,
the entries matching may choose, are also written as a GIMP palette (`Palette::gpl`, `stitch palette`,
REQ-CLI-011), which VectorCraft loads as a swatch library for Image Trace to trace a picture in
([image to embroidery](image-to-embroidery.md)). Thread catalogues (brand and number) arrive with M11.

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
`trims` decides where a preview cuts the thread and when a report says the machine makes none of a file's
trims ([rendering](rendering.md)). The PE800's is `None`, from machine checkpoint MC-1.

A profile is a machine with one hoop, so a machine with 3 hoops has 3 profiles. `profiles::REFERENCE` is
the reference machine's, the Brother PE800 with its 5 × 7 in hoop
([ADR 0013](adr/0013-brother-pe800-reference-machine.md)). Code that means the machine the checkpoints
run on imports it rather than naming a hoop: the command line plans for it unless `--profile` names
another or a VectorCraft document keeps one
([integration](vectorcraft-integration.md#design-settings-and-machine-kept-with-the-document)). Each
test sheet names the profile of the hoop it is sewn in.

`check_fit(bounds)` turns the design's size into `SC-E0701` (larger than the hoop) or `SC-W0702` (larger
than the comfort zone), with "rotate 90°" as a one-click fix when turning the design would fit
(`REQ-PRF-002`).

## Budgets

```rust,ignore
pub struct Budget {
    pub max_stitches: u32,           // per design, default 2,000,000
    pub max_work: u64,               // abstract work units per element, and once more for assembly
}
```

Work units are counted per inner loop iteration (scanline crossings, graph edge visits, offset
steps) so a budget behaves identically on every machine. Each element has the work budget to itself,
and so does working out what it offers the element before it to end near. Exhaustion there yields
`SC-E0004` for the element, and other elements still plan. Assembly and finalize share the budget once
more, and running out there plans nothing. `SC-E0004` then says so for the whole design.

Work bounds time, not memory: a unit can make a point, and 500 million points are gigabytes. A few
settings multiply an element's needle points. 10 mm of zigzag at the smallest spacing, sewn 100 times
with bean stitch 10, is 4.2 million. So the stitch limit bounds what generators hold as well
(`REQ-GEN-004`):

- A generator spends a stitch for each needle point as it makes it, and for each one a pass after the
  first or bean stitch sews again.
- What it lays out before sewing it must fit in the stitches left, though it spends none on it: a
  satin column's pairs, say, or a fill's row segments.
- Once done, it settles at the needle points it gives. Some it made were dropped on the way, such as
  points too close together, and some were copied, such as an E-stitch's way back down its spike.

So a generator never holds much more than the stitches left, and it may stop a little short of the limit
where it made many more points than it keeps. Each element is given the stitches the elements before it
left. The first that needs more ends planning there, with `SC-E0004` naming it: the design cannot be sewn
within its limit, and making the rest would only fill memory.
