# Conformance testing

<!-- implements: crates/stitchcraft-testkit/src/**, crates/stitchcraft-engine/src/normalize/fixture.rs -->

When behaviour is specified only by its implementation, fixes for one input break another and nobody
can say what "correct" means. StitchCraft specifies behaviour as **requirements**, proves each with **cases**, and reports the result
as a **matrix** on every pull request. Code is written to make cases pass, not the other way round
([ADR-0008](adr/0008-conformance-first.md)).

## Vocabulary

| Term | Meaning |
|---|---|
| Requirement | A numbered, testable statement: `REQ-FILL-TAT-006: pull compensation preserves region topology` |
| Case | Input + parameters + profile + expectations, as a data file, linked to ≥ 1 requirement |
| Check | A reusable assertion (an invariant, a metric bound, a golden comparison, an expected diagnostic) |
| Level | Where the case's truth comes from (L0 invariants … L4 a real machine) |
| Bless | Accept a changed golden output, in a PR, with a reason |
| Deviation | A documented, intended difference from Ink/Stitch's behaviour |

## Requirements

`conformance/requirements.toml`:

```toml
[[req]]
id = "REQ-FILL-TAT-006"
area = "fill/tatami"
level = "L2"
statement = "Pull compensation preserves the number of holes and connected components of the region."
rationale = "Compensating by growing the whole shape closes gaps the designer left on purpose."
milestone = "M5"
status = "planned"        # planned | active | retired
```

Rules, enforced in CI:

- ids are unique (`--check`), and never reused: a retired requirement stays in the file as `retired`.
  The checks read the file as it is, so review keeps the second rule. `--check` helps, failing for a case
  that names an id the file lacks, and `docs --check` fails for a page that names one;
- every case names existing requirements, and an `active` requirement has at least one case (`--check`)
  that passes (the run);
- a milestone cannot be closed while one of its requirements is `planned` (review);
- docs that mention a `REQ-…` id must refer to an existing one (part of `cargo xtask docs --check`).

## Cases

Cases come in two forms, and the report treats them alike.

**Rust tests named after a requirement.** A test function named `req_<area>_<nnn>_<what>` is a case for
`REQ-<AREA>-<NNN>`: `req_plan_002_stitch_lengths` proves `REQ-PLAN-002`, `req_fill_tat_006_gap_preserved`
would prove `REQ-FILL-TAT-006`. The name is the link, so there is no list to keep in sync; `--check` finds
them by scanning the source and rejects a name that points at no requirement. Use them for rules best
shown with small hand-built inputs — an invariant's violating and passing examples, a palette's spot
checks.

**Rust tests named after a diagnostic code.** A test function named `diag_sc_<e|w|i><nnnn>_<what>` is a
case for the code `SC-<E|W|I><NNNN>`: it produces the code from real input through the public API and
checks the full text a user reads, message and fix (`diag_sc_e0701_a_design_larger_than_the_hoop_names_both_sizes`).
Every registered code needs one: `--check` fails for a code without a `diag_` case and for a `diag_` name
that points at no registered code, and the run fails when a code's cases do not pass. The report lists
the codes in a table of their own. This is what keeps error paths alive
([diagnostics](diagnostics.md#the-registry)).

**Data cases**, `conformance/cases/<area>/<name>.toml`: an input, a profile and expectations, run by the
suite in-process. Unknown fields are errors, so a typo cannot silently switch a check off. M1 has the
`testsheet` kind:

```toml
id = "ts-10b"
requirements = ["REQ-PLAN-001", "REQ-PLAN-002", "REQ-PLAN-003", "REQ-PLAN-005", "REQ-PLAN-006", "REQ-PRF-002", "REQ-FMT-001"]
kind = "testsheet"
sheet = "TS-10B"
profile = "brother-pe800-5x7"

[expect]
size_mm = [130.0, 180.0]
diagnostics = []                                 # exactly these codes, here none
golden = ["golden/testsheets/TS-10B.pes"]        # byte for byte; the format comes from the extension
```

The plan invariants run on the sheet's plan before anything else. The golden files are the very bytes
sewn at the machine checkpoint, so a change to how a sheet sews cannot slip through unnoticed.

The `oracle` kind reads machine files with an independent reader — a pinned pyembroidery, through
`conformance/oracle/decode.py` — and with StitchCraft's, and requires the machine to do the same thing
either way (`REQ-FMT-005`):

```toml
id = "pyembroidery-oracle"
requirements = ["REQ-FMT-005"]
kind = "oracle"
files = ["golden/formats/every-command.pes", "golden/testsheets/TS-01.dst"]   # …
```

It needs Python with pyembroidery (`STITCHCRAFT_PYTHON`, see `conformance/oracle/requirements.txt`):
without it the case is skipped locally (⏭ in the report) and fails in CI, like every optional tool. A
requirement whose cases were all skipped is skipped too, and fails the run only in CI.

Geometry that Ink/Stitch leaves to shapely is checked against shapely's own answers. Scripts in
`conformance/oracle/` ask a pinned shapely, through its public API alone, about random shapes:

- the nearest points of rails (`nearest.py`)
- offset curves (`offset.py`)
- the measures that make a satin column's rails from its centre line (`centre_line.py`)
- polygons shrunk and grown, for a fill's inset underlay and its expanded rows (`buffer.py`)
- the pieces of rows that polygons cut, for a fill's rows (`rows.py`)

Their answers are fixtures under `conformance/fixtures/geometry/`, one question to a line of text, and the
engine's unit tests read them with a shared reader (`normalize::fixture`) and answer every question the
same way. Regenerating a fixture needs shapely, and checking it does not. The same module draws, from
their corners, the parts that the unit tests of centre lines and columns take.

The `plan` kind takes a design, an SVG file or a VectorCraft document, the whole way to machine files, as
`stitch plan` does for a profile. It checks the plan, and compares the files written from it with golden
files. The `strokes` case:

```toml
{{#include ../../../conformance/cases/plans/strokes.toml}}
```

What a generator keeps of a shape, such as the holes of a fill, is checked by Rust tests named after their
requirement. Fixtures are small files under `conformance/fixtures/`, hand-written or generated, each with
a row in the fixtures index stating its origin and licence (`--check`). Large real-world corpora live in
repositories of their own, as VectorCraft's do, and are never committed. `cargo xtask corpus` fetches
Ink/Stitch's fonts at a commit and with a SHA-256 this repository pins ([fonts](fonts.md#the-corpus)).

The `font` kind reads one font of that corpus with the font reader and plans each of its glyphs, as the
glyph differential plans a glyph (`REQ-FONT-008`, `xtask/src/conformance/font.rs`). It names the font's
folder and the glyphs it has in each direction:

```toml
{{#include ../../../conformance/cases/fonts/small-font.toml}}
```

Without the corpus the case is skipped, and says so. In CI every job that runs `cargo xtask ci` fetches the
corpus first (`.github/actions/fonts`), and the case runs.

The engine's cases build their small designs with `stitchcraft_testkit::designs`. It also reads a plan's
shape at a glance: `J L4 S5 L4` is a jump, 4 lock stitches, 5 needle points of stitching and 4 more lock
stitches. Its strokes are as wide as an SVG stroke whose style says nothing, and `widened` gives one a
width, `capped` a cap, `clipped` one clip path more, and `automatic` makes any element one whose stitches
StitchCraft chooses, by the settings of the rules that `rules` gives at their defaults. Its fills are
closed rings filled by the nonzero rule (`fill`, through `rings`), so a hole is drawn the other way round
from its outline. No stitch type gives its groups threads of their own before M10.6. Until then `threaded`
puts the groups of each element in the threads a case gives them, `block` and `within`, and assembles and
finalizes the elements as `plan` does. `three_threads` is one element sewn in red, green and blue, which the
cases of the writers, the previews and the report sew (`REQ-THREAD-003`). The plug-in has no dev-dependencies,
and its own `testing` module makes the same element for the cases of its exporters and live effects. The satin cases sew their columns with `stitchcraft_testkit::satins`, as an element is
sewn on the reference machine, with the stitch lengths the engine gives it there (`satin_lengths`), and
read the needle points back in pairs across the column. A column is sewn alone (`sewn_satin`) or between
the neighbours a case gives it (`sewn_between`), and `satin_diagnostics` keeps its warnings' places for
the cases that check where a warning points. The cases of a satin method sew a straight column by that
method (`sewn_ladder`) and compare its needle points with the ones they expect, but for rounding
(`near`). The cases of automatic digitizing sew the test pictures, which
`stitchcraft_engine::testsheets::pictures` draws in code and the TS-13 sheets sew too, each with the
edges where its shapes meet. `stitchcraft_testkit::pictures` places them where the cases measure them.
There are 3 stripes side by side, and a badge, a row of badges, an emblem and a button, each traced
abutting or stacked ([image to embroidery](image-to-embroidery.md#testing)). The cases sew the pear that the
docs show too, a photo that VectorCraft traced into ragged shapes (`stitchcraft_testkit::pictures::pear`).

## Levels

### L0 — Plan invariants (every PR)

The [plan invariants](data-model.md#plan-invariants) run on *every* plan the
suite produces, whatever the case is about. A case never needs to ask for them.

### L1 — Formats (every PR; fuzzing nightly)

Golden bytes for canonical plans; round trips on random plans (`proptest`); every command per format;
empty and degenerate files; the **pyembroidery oracle** job (pinned version, in its own CI job) decodes
our files and must agree on every stitch; `cargo-fuzz` targets run nightly with a persisted corpus
([formats](formats.md#conformance)).
A file read back must do what its format tells the machine, which can mean more cuts than the plan has.
DST machines cut before long runs of jumps (`REQ-FMT-008`), and a PES file flags every jump as a trim
(`REQ-FMT-010`). `stitchcraft_testkit::equivalence::format_events` gives both, and drops a cut with
nothing sewn since the last one, which changes nothing on the fabric.

#### Fuzzing

`fuzz/` has a target for each reader (`read_pes`, `read_dst`, `read_svg` and `read_vectorcraft`), and
one for a file's whole journey (`read_write_preview`: read, write in every format, read the files back,
preview). Each target is one line. Its body is in `stitchcraft-testkit::fuzz`, where every pull request
runs it on the seed files, every shortening of them and random or damaged bytes, on stable Rust and all
three operating systems. The properties:

- **machine files** (`REQ-FMT-006`). No reader panics or gives more than 2,000,000 entries or a position
  beyond ±10 m. Each format writes a plan read from anything or refuses it. PES refuses a design more
  than 204.7 mm from its start (`REQ-FMT-011`). Each file reads back to a plan the machine sews the same
  way, for DST the way DST machines sew it (`REQ-FMT-008`). Previews never panic.
- **SVG** (`REQ-SVG-002`): no panic; the reader refuses only with its own codes (`SC-E0801`,
  `SC-E0004`); what it reads is a valid design, with no point beyond ±10 m, and everything it says about
  the file is a warning.
- **VectorCraft** (`REQ-VC-001`), as for SVG, with `SC-E0806` for a file that isn't a VectorCraft
  document. The same bytes compressed with gzip read the same, unless unpacking them spends the budget.

Every night `nightly.yml` fuzzes each target for 20 minutes with libFuzzer (`-timeout=10`,
`-rss_limit_mb=2048`). The corpus starts from the seeds, the golden machine files or the SVG and
VectorCraft fixtures (with dictionaries of their keywords, `fuzz/svg.dict` and `fuzz/vectorcraft.dict`).
It is kept, minimised, in the Actions cache, and each night continues where the last stopped. A crash
fails the job and keeps the input as an artifact. To work on one locally:

```sh
cargo +nightly fuzz run read_pes                        # until stopped; corpus in fuzz/corpus/read_pes
cargo +nightly fuzz run read_pes fuzz/artifacts/read_pes/crash-…   # reproduce a crash
```

A crash becomes a regression test next to the code it found a bug in, before the fix.

### L2 — Generator properties (every PR)

Property tests per generator ([algorithms](algorithms/)), run over:

- hand-written cases (the shapes people actually draw: letters, circles, leaves, rings, stars);
- the **degenerate corpus** (zero-area, hair-thin, self-touching, collinear, duplicate points, single
  points, near-coincident rings, huge coordinates, 10⁶ vertices) in `stitchcraft_testkit::degenerate`;
- random shapes from `stitchcraft-testkit` strategies, with fixed seeds in PR CI (fast, reproducible)
  and fresh seeds nightly (with failing seeds saved as new cases).

Every stitch type sews the degenerate corpus, and random paths whose points lie on a coarse grid so that
they coincide, lie in line and cross often (`tests/degenerate.rs`): 64 on every PR, and 2,000 with a fresh
seed every night. Each shape is a fill by each fill method under each fill rule, a stroke by each stroke
method 0.5 and 3 mm wide, and a satin column by each satin method. None may report a bug (`SC-E0009`),
and each gives a plan or an error that says why it doesn't. Each sews the same planned again, and its plan keeps the machine's invariants. A method not built yet says so
(`SC-W0011`) and is run as soon as it is built. The path of 10⁶ points is sewn within a thousandth of the
default budget, so that every way of sewing it shows that it runs out of work only by saying so
(`SC-E0004`).

Metrics are defined once in `stitchcraft-testkit::metrics` so every case measures the same way:

| Metric | Definition |
|---|---|
| Coverage | Rasterize the region and the stitch lines (thread width 0.4 mm) at 0.05 mm; covered fraction of region pixels |
| Edge coverage | Points every 0.05 mm along an edge where 2 shapes meet. The fraction within 0.2 mm of a top stitch, with each stitch pulled in where the thread turns |
| Containment | Maximum distance of any stitch point outside the region (0 if inside) |
| Row spacing | Median and spread of perpendicular distances between consecutive rows |
| Stitch length | Min, max and distribution of `Normal` stitch lengths by role |
| Furrows | Longest run of rows whose needle points align within 0.1 mm |
| Topology | Holes and components of the region vs. of the stitched area |
| Bare fabric | The design painted at 0.05 mm as a viewer paints it, its background left out: the largest patch the top stitches leave bare, once opened with a disc as wide as the thread (`stitchcraft_testkit::fabric`) |
| Thread astray | The longest stretch of a stitch that shows on another colour or on bare fabric, or of an underlay or travel stitch that shows at all. Moves between elements and jump threads are counted apart |

Coverage is defined since M5.9 and edge coverage since M13.2, and the other metrics arrive with the cases that
need them. `stitchcraft-testkit::stitches` reads back what a test sheet's stitches show against its
drawing: where each element's needle goes in and where a satin column's rungs lie. It also finds the jump
threads the PE800 leaves, as it cuts none, and the later stitches that sew them down. A pixel is the region's when its centre lies inside the region's rings, by the even-odd rule, and
covered when its centre lies within 0.2 mm of a stitch the plan sews, whatever its role: the thread is the
stitch widened to 0.4 mm and rounded at its ends. Each stitch is drawn a row of pixels at a time, along the
stretch where the row crosses that shape. The work grows with the pixels the thread covers, not with the
stitches times the pixels.

Edge coverage pulls each top stitch in where the thread turns, as thread pulls a row in at its ends. The
needle holes along a straight run stay where they are. Underlay and locks are left out, as they are
meant to be covered (`stitchcraft_testkit::metrics`).

**Transformations.** A design mirrored or turned half round must sew its stitches mirrored or turned with
it (`stitchcraft-testkit::transformed`, `tests/transformed.rs`), and a shape drawn again the other way
round, from another point or with points added along its edges must sew the same thread. Since the checks
don't need an expected answer, they run on every element of every test sheet drawn as a design, and on
random letters digitized automatically. They compare the share of each element's top stitches and
underlay that lies within 0.3 mm of the other plan's, which overlooks needle points placed a little apart
but still tells a column from a line sewn along its middle. A fill lays out its rows from the point (0,
0). So a design is mirrored and turned about that point, its fill angles and its elements' start, end and
target positions and clips with it, and a move is left out of the changes. The design's turn on export
turns the other way in a mirror image, whose plan is then the mirror image of the design's. A rule that
depends on a tie broken by coordinates fails the checks: TS-13B's letter A 6 mm tall sewed one leg as a
satin column and the other as a line, and its mirror image the other way round. Each element of the
sheets keeps 0.993 or more of its thread where the change takes it, against a bound of 0.98. A zigzag
stroke and an E-stitch are laid by design from where their shape is drawn, as Ink/Stitch lays them, and
are not measured where that changes. A zigzag stroke moves its first needle point to the right of its
way, so mirrored or drawn from its end it zigzags from the other side. An E-stitch's spikes lie a spacing
apart from its start, so drawn from its end they lie elsewhere along it. TS-11A's sparse zigzags and
E-stitches keep as little as 0.42 of their thread so.

### L3 — Ink/Stitch differential (nightly)

A pinned Ink/Stitch runs as a black box on a corpus of SVG files written for StitchCraft, and writes
machine files. Running Ink/Stitch as an oracle uses it and doesn't copy it
([ADR-0001](adr/0001-license-and-clean-room.md), decision 6). The job fetches it at the commit
`conformance/inkstitch/pin.toml` names, and implementers see measures instead of Ink/Stitch's code.
`cargo xtask conformance --differential` runs the `inkstitch` cases (`conformance/cases/inkstitch/`):
for each, Ink/Stitch and StitchCraft, planning on the reference machine, each write a PES file, and
StitchCraft's reader reads both back, so that both are measured on PES's 0.1 mm grid
(`xtask/src/conformance/differential.rs`). We compare **measures, not stitches**:

| Measure | Tolerance |
|---|---|
| stitches, the needle-downs | within 10 % |
| size, the bounds of the needle points | each side within 0.5 mm |
| threads, in order | the same |
| trims, and jumps | each within 1 |
| overlap of the thread's footprints, once their centres are made one (`stitchcraft_testkit::metrics`) | 0.9 at least |

Differences we intend are recorded in the **deviations ledger** (`conformance/deviations.toml`), each
with a reason and a link to the requirement that motivates it (`DEV-LCK-001`, say: the lock shapes
are StitchCraft's own). A case excuses a measure with the entry that explains it, and the report shows
the difference as excused. An excuse for a measure that agrees fails the case, so that every reason in
the ledger is still true. Where the differential can undo a deviation in Ink/Stitch's file, it measures
the file again without it. The excuse then counts only if the measure agrees. Ink/Stitch's jumps that go
nowhere (`DEV-FMT-001`) excuse as many trims and jumps as there are of them, and no more. Within a colour
block, pystitch writes such a jump after lock stitches an element forces as it writes one after a trim of
the plan. Only as many are left out there as Ink/Stitch's file has trims more than StitchCraft's. Where both files
change thread at the same colour blocks, Ink/Stitch's threads, named by other entries of Brother's palette
(`DEV-THREAD-001`), take StitchCraft's names. StitchCraft fits each stitch to the machine (`DEV-FIN-001`),
so to undo that, Ink/Stitch's stitches longer than the machine's longest are split and its needle points
nearer the one before than the machine's shortest stitch left out. A needle point Ink/Stitch sews alone,
where a path's move-to draws nothing after it, is left out with the jumps and the trim that lead to it
(`DEV-SVG-011`).

`REQ-SVG-008`, `REQ-AUTO-021`, `REQ-AUTO-022`, `REQ-RUN-013`, `REQ-SAT-029`, `REQ-FILL-CON-008`,
`REQ-FILL-CIR-007`, `REQ-FILL-CRS-007`, `REQ-FILL-CRS-012`, `REQ-FONT-001`, `REQ-ROUTE-008`,
`REQ-ROUTE-013`, `REQ-RIP-011` and `REQ-RIP-019` are the requirements of level L3. The other runs of the suite leave them and their cases to this one. The corpus is
designs the SVG reader sews in full, with their settings (`REQ-SVG-009`), trim and stop commands and the file
version. A case may name a test sheet drawn as a design instead (`sheet`). With `elements`, the measures
judge only the sheet's elements whose ids start with it. Both programs sew an SVG file
of the elements StitchCraft sews the sheet with, written with their parameters, their start, end and
target positions as Ink/Stitch's commands and their clips as clip paths, the design's origin and stop
position as commands too, and its turn on export (`stitchcraft_testkit::inkstitch`), which StitchCraft's
reader reads back unchanged (`REQ-PRM-003`). An element's guide line and anchor line are paths beside it
in its own group, each started with Ink/Stitch's marker, the guide with its stroke and its settings. The file leaves out an element that sets a setting of
StitchCraft's own that its stitch type reads, which Ink/Stitch would pass over and sew otherwise
(`sewn_alike`). So TS-17C's file has no crossing columns with their underlays first. An element a sheet
draws as a clone of another is written as a `<use>` of it, and each command in a group with its
connector, as Ink/Stitch's Attach Commands writes them, since Ink/Stitch moves a command its clone copies
only from a group. Automatic digitizing chooses ordinary elements with Ink/Stitch's parameters, so
Ink/Stitch can sew what StitchCraft chose, and the TS-13 sheets check StitchCraft's stitches of those
choices against Ink/Stitch's (`REQ-AUTO-021`). TS-11B checks the contour fills so, each strategy and join style among them (`REQ-FILL-CON-008`), and its
circular fills (`REQ-FILL-CIR-007`). TS-11A checks the zigzag strokes (`REQ-RUN-013`), and the
E-stitches, S-stitches and zigzag satins (`REQ-SAT-029`), along straights and round curves. TS-17A checks
the simple crosses of cross stitch, on grids of 2 and 3 mm with 2 and 4 threads, flipped, and over a
letter whose slanted edges cut their cells in half (`REQ-FILL-CRS-007`). It checks the other methods too,
upright, dense upright, double, Smyrna and half crosses, and simple crosses on a grid turned 30°. The
`cross-methods` case adds each full cross's flipped variant, grids turned on the canvas and from a part's
own corner, and half crosses round a hole, through cells that meet at a corner and in 2 pieces of one
part (`REQ-FILL-CRS-012`). TS-17B checks the tartans, with setts mirrored and repeated and a warp and a
weft apart. Its narrow stripes sew as runs, one sett is turned and scaled, and one is a herringbone. The
`tartan-herringbone` case adds 4 herringbones turned both ways. One of them lies round a hole, its warp and
weft apart and 3 rows a thread, and another has stripes sewn as runs. The `tartan-marks`, `tartan-hole`,
`tartan-parts` and `tartan-stop` cases add stripes left unsewn or sewn in the colour before them, a sett
moved by its offsets, a disc round a hole in stitches 3 mm long, a fill in 2 parts with its rows at 30° and one row a thread, and
a fill that stops at its end with its runs sewn as bean stitches (`REQ-FILL-TRT-011`). Every stripe is one of
Brother's threads, so both files name each thread as itself. Below its tartans, TS-17B checks linear gradients
of 2 stops and of 3, one of them transparent, each at a slant of its own. The `gradient-` cases add a gradient
of no length, one that a `gradientTransform` turns, a fill in 2 parts that stops at its end, and a fill without a
gradient (`REQ-FILL-GRD-009`). Their gradients placed in the shape's box and by the gradient their `href`
names, and stops that all lie before the fill's first row, sew otherwise in Ink/Stitch, as the ledger explains
(`DEV-SVG-027`, `DEV-FILL-032`). The `inkstitch-ts17b-guided` case judges TS-17B's guided
leaf alone, its rows copies of an S-shaped guide from tip to tip over its underlay (`REQ-FILL-GDD-008`).
The `ts17c-ripple-` cases check each of TS-17C's
ripple stitches on its own, the one along a column and the 2 that shrink to a target point (`REQ-RIP-011`).
The `ts17c-guided-` cases check its guided ripples so (`REQ-RIP-019`). A wave and a zigzag cut to a circle
follow guide lines. A circle and a diamond stand between satin guides' rails, the diamond at its guide's
rungs with an anchor line, and a column has a grid across it.
TS-02B to TS-09, the sheets of MC-2 to MC-4, and TS-11C, the meander fills' sheet, are cases of
`REQ-SVG-008` (M8.2). The `drawn-locks` case and TS-16 check custom locks drawn as
paths, of each kind of command, which both programs sew at the same nodes (`REQ-LCK-005`).
`conformance/inkstitch/README.md` says how to run it by hand and how to add a case.

**Routes.** A route is a design of its own, so sewing StitchCraft's route in both programs would compare 2
sewings of one route. A case with a `[route]` table instead routes elements of a sheet in each program
(`REQ-ROUTE-008`, `REQ-ROUTE-013`, `xtask/src/conformance/routing.rs`). The table's `tool` names
Ink/Stitch's tool, Auto-Route Satin Columns or Auto-Route Running Stitch, which routes the SVG file of the
elements with the table's options and writes its route for Ink/Stitch to sew. `stitchcraft_engine::route`
routes the same elements with the same options, and StitchCraft sews its route. The 2 PES files are
measured as any case's. The `route-ts17c` cases route TS-17C's letters as drawn, once with the options at
their defaults and once with Preserve order on and trims off. The `route-ts17c-lines` cases route its
network of running stitches as drawn, once with the options at their defaults and once each with Trim jump
stitches on, Preserve order on and Add nodes at intersections off. `ts17c` sews the sheet's file, whose
lower letters and right-hand lines StitchCraft routed, in both programs.

**Centre lines.** Sewing what StitchCraft chose can't find a choice that is wrong: TS-13B's small T sewed
a Γ, the left arm of its bar left out of its centre line, and the differential agreed by every measure.
The `centre-lines` cases compare a step before the stitches (`REQ-AUTO-022`,
`xtask/src/conformance/centre_lines.rs`). Each shape that automatic digitizing sews wholly as running
stitches is digitized alone, and its lines are compared with the lines Ink/Stitch's Fill to Stroke draws
for it, with its threshold at the shape's width. Fill to Stroke finds them another way, from a Voronoi
diagram of the outline. Where one program leaves a branch out, or puts a line off the middle, the 2
differ. With `reach` the width and 0.1 mm:

| Measure | Tolerance |
|---|---|
| each end of Ink/Stitch's lines | an end of StitchCraft's within `reach` |
| each end of StitchCraft's lines | within `reach` of Ink/Stitch's lines |
| each program's lines, farther than `reach` from their ends | within a quarter of the width of the other's |

The ends differ by design. StitchCraft's line stops short of a round or square end (`REQ-AUTO-014`), and
Fill to Stroke's runs to the end's middle. Fill to Stroke leaves out a branch shorter than its threshold,
such as the stub up into the top of an A, which StitchCraft sews to cover the top. A case names a test
sheet, an SVG file of shapes, or letters that `stitchcraft_testkit::designs::letter` draws (`[letters]`).
The report has a row for each shape, and `target/conformance/differential/` the shapes as written for
Ink/Stitch and a picture of both programs' lines over them. Run on the code from before the T was fixed
(`03f0ab69`), the letters case finds a branch missing in 3 letters 3 mm tall: the Ts in 0.5 and 0.7 mm
lines, and the E in 0.7 mm lines.

**Ink/Stitch's fonts.** The `fonts` case sews each glyph of Ink/Stitch's fonts as a design of its own
(`REQ-FONT-001`, `xtask/src/conformance/fonts/`), from the 131 fonts whose licence file allows it
([fonts](fonts.md#the-corpus)). Each glyph is cut out of its glyph file, its group shown and the other
glyphs left out, and both programs sew that file, compared by the measures above. StitchCraft plans it for
the reference machine with a hoop 400 mm a side, as a glyph file is drawn at its font's height and a glyph
may be wider than the reference hoop. Every night the case sews 4 glyphs of each font, and every Sunday all
21,179, in 6 parts. A glyph whose stitch types StitchCraft doesn't sew yet is listed, and joins the
comparison with M10's steps. An element whose setting StitchCraft can't read is left out with `SC-E0101`,
where Ink/Stitch sews the setting's default: its glyph is excused by `DEV-PRM-001` and not compared. A glyph
neither program writes a file of agrees. Ink/Stitch writes none when it finds nothing to sew, an element it
can't sew or elements that sew no stitch, as of a glyph of one empty path, and StitchCraft none when an
element is left out with an error, as `stitch plan` does. `--font` sews the glyphs of the fonts it names
alone, so that the fonts a weekly run found differing sew again in minutes.

The corpus can't have an excuse for each of its glyphs. `conformance/inkstitch/glyphs.toml` excuses a
measure by an entry of the ledger for the glyphs where its cause is. A diagnostic picks them, such as
`SC-I0703` where StitchCraft splits stitches longer than the machine's longest, and so do a stitch type, a
setting and a font. A cause that only the drawing of a few glyphs shows, such as a rung that rounding leaves
a hair short of a rail, is named by its glyphs, and an excuse that names `sewn` explains a glyph that only one
program sews. The excuses the differential can check are checked as a case's are. StitchCraft's
running stitches place their needle points otherwise (`DEV-RUN-002`), and that excuse counts only where each
program's thread lies within 0.3 mm of the other's, for 0.9 of it. The report, `target/conformance/fonts.md`,
has a row for each font, the glyphs each excuse excused, and each glyph that differs with its measures.

### L4 — Physical (milestone gates)

Machine checkpoints sew generated test sheets on the target machine and record the results through the
**Sew-out report** issue form ([machine testing](../plan/machine-testing.md)). A checkpoint result is
linked from the requirements it validates (`physical = ["MC-1"]`), and a profile value can change only
with such a record.

## The runner and its report

`cargo xtask conformance` runs every data case in-process and every Rust-test case through one `cargo
test`, then writes `target/conformance/`:

- `report.md` — the requirement × case matrix, failures first, requirements nobody tests yet folded
  away; in GitHub Actions it is appended to the job summary, so every pull request shows it;
- `report.json` — machine-readable results, for the docs (requirement pages will show their status);
- `diffs/` — for a machine-file golden that differs, previews (simple style) of the golden file and of
  the new output, so a reviewer sees what sews differently; CI keeps them as a workflow artifact when a
  run fails.

It fails when a case fails or an active requirement has no passing case. `--filter <text>` runs only the
cases whose id, test name or requirements contain the text (and skips the "every active requirement"
rule, since the run is partial). `cargo xtask ci` runs the whole suite; the docs workflow runs `--check`.

`cargo xtask conformance --bless <case>` rewrites a data case's golden files. CI refuses a pull request
that changes files under `conformance/golden/` unless it carries the `golden-change` label and a line in
`CHANGELOG.md`. For the canonical-plan goldens of the format tests, bless with
`STITCHCRAFT_BLESS=1 cargo test -p stitchcraft-formats --test golden`; for the preview goldens
(`REQ-RND-002`), with `STITCHCRAFT_BLESS=1 cargo test -p stitchcraft-render --test preview`. Later, `stitch conformance run`
will let users validate a machine profile with the same cases.

## Gates

| When | What must pass |
|---|---|
| Every PR | L0, L1 (no fuzzing), L2 with fixed seeds, `--check` rules, the data cases' golden files on Linux, macOS and Windows ([cross-platform check](determinism.md#cross-platform-check)), coverage floors, mutants in the changed lines |
| Nightly | L1 fuzzing, L2 fresh seeds, L3 differential (also on a pull request that changes it) |
| Weekly | Mutation testing of every shipped crate, against the recorded counts |
| Weekly | Every glyph of Ink/Stitch's fonts in the L3 differential |
| Milestone close | All requirements of the milestone `active` and green; its machine checkpoint signed off |
| Release | Everything above, plus the compatibility gate green on VectorCraft `stable` |

## Unit, snapshot and doc tests still exist

Conformance cases describe behaviour users see. Unit tests (in modules), snapshot tests (`insta`, for
intermediate structures such as cell decompositions) and CLI examples in the docs (`trycmd`) still cover
internals and documentation; mutation testing (`cargo-mutants`) and a coverage ratchet (`cargo-llvm-cov`)
measure how well all of them together bite ([guardrails](guardrails.md)).

Both are ratchets. `conformance/coverage.toml` holds a line-coverage floor per crate, which a pull
request may not fall below and `cargo xtask coverage --record` only raises. `conformance/mutation.toml`
holds, per crate, how many mutants no test notices: the weekly run fails when a crate has more, and
`cargo xtask mutants --record` only lowers the counts. The weekly job summary lists every missed mutant,
which is the to-do list for better tests. Each mutant is judged by its own crate's tests
(`.cargo/mutants.toml`), so each crate answers for its own code.

New code answers for itself at once: every pull request runs the mutants in the lines it changes
(`cargo mutants --in-diff`, minutes rather than the full run's quarter of an hour per part), and
`cargo xtask mutants --changed` fails for any that no test notices. The exception is an *equivalent*
mutant, which changes nothing a test could observe; it is listed under `[[equivalent]]` in
`conformance/mutation.toml` with its file, its description, the source line it changes and why, so a
listed equivalent stops counting as one as soon as its line changes.

A mutant can also break the condition that ends a loop, as when a walk over a graph never takes away the
edges it has used. The loop then runs until the work budget ends or cargo-mutants' timeout stops it,
which counts apart. A loop that keeps 40 bytes for each unit of work wants 20 GB by the end of the
default budget, more than a CI runner has, and the job ends without a result when the runner runs out of
memory. So in the mutants jobs each test binary runs with at most 4 GiB of address space, through
`prlimit` as Cargo's runner: such a mutant aborts at the cap, which counts as noticed.

Each mutant builds its crate again and runs the crate's tests, and the tests take most of the time. So
the mutants jobs build without debug information, and with the workspace's own code optimized a little
(`opt-level` 1, where its dependencies are at 2 in every build). On 2 cores an engine mutant then takes
157 s against 267 s: its build 114 s against 47 s, and its tests 43 s against 220 s. The results stay the
same. Rust keeps floating-point arithmetic as written at every level, and debug assertions and overflow
checks stay on.

Mutation testing runs the tests hundreds of times for one pull request. A test that writes files puts
them in a folder of its own (`stitchcraft_testkit::temp::TempDir`), which goes with everything in it
when the test ends, whether it passed or failed. Until October 2026 the command line's tests left their
folders in the system's temporary folder: 14 folders and about 65 MB on each run of the workspace's
tests.
