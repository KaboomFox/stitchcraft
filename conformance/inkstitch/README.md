# Ink/Stitch, as the conformance suite uses it

StitchCraft sews Ink/Stitch's files as Ink/Stitch sews them. The suite checks that in 2 ways, neither of
which copies Ink/Stitch's code ([ADR-0001](../../docs/src/design/adr/0001-license-and-clean-room.md)):

- `check_params.py` compares the compatibility contract with Ink/Stitch's parameter declarations
  (`conformance/inkstitch-params.toml`). Its docstring says how to run it.
- The **differential** (conformance level L3, [conformance](../../docs/src/design/conformance.md#l3--inkstitch-differential-nightly))
  runs Ink/Stitch as a black box on the designs of the `inkstitch` cases (`conformance/cases/inkstitch/`)
  and compares its machine files with StitchCraft's by measures. Its `centre-lines` cases compare the
  lines Ink/Stitch's Fill to Stroke draws for narrow shapes with the lines automatic digitizing sews them
  along. CI runs it every night, and on a pull request that changes it or automatic digitizing
  (`.github/workflows/differential.yml`).

## What is here

| File | What it is |
|---|---|
| `pin.toml` | The Ink/Stitch the differential is measured against: its repository and commit, the one the compatibility contract was read at |
| `requirements.txt` | The Python packages that Ink/Stitch runs with, pinned by version and hash |
| `stubs/wx/` | A stand-in for wxPython, which Ink/Stitch's extensions import as they load and writing a file never uses |

## Running the differential by hand

Fetch Ink/Stitch at the pinned commit outside the repository, give it a Python of its own, and point the
suite at both, as the `inkstitch` action does in CI:

```sh
commit=$(sed -n 's/^commit = "\(.*\)"$/\1/p' conformance/inkstitch/pin.toml)
git init -q ../inkstitch-pinned
git -C ../inkstitch-pinned fetch -q --depth 1 https://github.com/inkstitch/inkstitch "$commit"
git -C ../inkstitch-pinned checkout -q FETCH_HEAD
python3 -m venv target/inkstitch-venv
target/inkstitch-venv/bin/pip install --no-deps --require-hashes -r conformance/inkstitch/requirements.txt
STITCHCRAFT_INKSTITCH="$PWD/../inkstitch-pinned" STITCHCRAFT_INKSTITCH_PYTHON=target/inkstitch-venv/bin/python \
  cargo xtask conformance --differential
```

The report is `target/conformance/differential.md`, with a table for each case. An `inkstitch` case's
rows are the measures of both machine files, and a `centre-lines` case's are its shapes. The SVG files written for test sheets are in
`target/conformance/differential/`, each with Ink/Stitch's PES file of it (`ts-16.inkstitch.pes`), which a
machine checkpoint sews beside `stitch testsheet`'s, and each `centre-lines` case's shapes with a picture
of both programs' lines over them, Ink/Stitch's red and StitchCraft's blue. `--filter centre-lines` runs those
cases alone. The suite refuses a checkout at another commit, since the cases' excuses were written against
this one. It runs Ink/Stitch with its stitch-plan cache off, from a settings file of its own in
`target/conformance/inkstitch-config/`: the cache keeps a clone's stitches across files whose commands
differ, and gave the stitches of an earlier run.

## Adding a case

Each case is a design in `conformance/fixtures/inkstitch/`, with its row in the fixtures index, and a file
in `conformance/cases/inkstitch/` naming the design and the requirements it covers. The SVG reader reads
each shape's `inkstitch:` settings (`REQ-SVG-009`), so a design may set them. A design leaves out what the
reader names as not applied (`SC-W0802`), such as text, masks and markers.
Where StitchCraft sews otherwise on purpose, the case excuses the measure with the deviation that says why:

```toml
id = "inkstitch-fill-rect"
requirements = ["REQ-SVG-008"]
kind = "inkstitch"
design = "fixtures/inkstitch/fill-rect.svg"

[excused]
threads = "DEV-THREAD-001"
```

A case may name a test sheet drawn as a design, `sheet = "TS-13A"`, in place of `design`. Both programs
sew an SVG file of the elements StitchCraft sews the sheet with, automatic digitizing's for its automatic
shapes, each with its parameters (`stitchcraft_testkit::inkstitch`), which StitchCraft's SVG reader reads
as they were written (`REQ-SVG-009`).

A sheet's case with a `[route]` table routes the sheet's elements whose ids start with `elements` in each
program, with the options it sets by Ink/Stitch's names, and compares the 2 routes, each sewn by its own
program (`REQ-ROUTE-008`). Ink/Stitch's tool runs as `--extension=auto_satin`, and writes the SVG file it
routed, `route-<case>.routed.svg`, beside StitchCraft's route as an SVG file, `route-<case>.stitchcraft.svg`:

```toml
id = "inkstitch-route-ts17c-in-order"
requirements = ["REQ-ROUTE-008"]
kind = "inkstitch"
sheet = "TS-17C"

[route]
elements = "ts17c:drawn:"
preserve_order = true
trim = false
```

A `centre-lines` case names its shapes with `sheet`, `design` or a `[letters]` table, and has no
excuses:

```toml
id = "inkstitch-centre-lines-letters"
requirements = ["REQ-AUTO-022"]
kind = "centre-lines"

[letters]
names = "THE"
heights_mm = [3, 4, 5, 6, 8, 10]
widths_mm = [0.3, 0.5, 0.7, 0.9]
```

Only the shapes that automatic digitizing sews wholly as running stitches are compared, and a case that
has none fails.

The [design](../../docs/src/design/conformance.md#l3--inkstitch-differential-nightly) gives the measures,
their tolerances and what an excuse may do.
