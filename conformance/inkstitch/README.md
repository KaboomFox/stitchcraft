# Ink/Stitch, as the conformance suite uses it

StitchCraft sews Ink/Stitch's files as Ink/Stitch sews them. The suite checks that in 2 ways, neither of
which copies Ink/Stitch's code ([ADR-0001](../../docs/src/design/adr/0001-license-and-clean-room.md)):

- `check_params.py` compares the compatibility contract with Ink/Stitch's parameter declarations
  (`conformance/inkstitch-params.toml`). Its docstring says how to run it.
- The **differential** (conformance level L3, [conformance](../../docs/src/design/conformance.md#l3--inkstitch-differential-nightly))
  runs Ink/Stitch as a black box on the designs of the `inkstitch` cases (`conformance/cases/inkstitch/`)
  and compares its machine files with StitchCraft's by measures. Its `centre-lines` cases compare the
  lines Ink/Stitch's Fill to Stroke draws for narrow shapes with the lines automatic digitizing sews them
  along. Its `fonts` case sews the glyphs of Ink/Stitch's fonts, each as a design of its own
  ([below](#inkstitchs-fonts)). CI runs it every night, and on a pull request that changes it or automatic
  digitizing (`.github/workflows/differential.yml`). Every Sunday it sews every glyph.

## What is here

| File | What it is |
|---|---|
| `pin.toml` | The Ink/Stitch the differential is measured against: its repository and commit, the one the compatibility contract was read at |
| `fonts.toml` | Ink/Stitch's fonts, the corpus of the glyph differential: their repository and commit, with the SHA-256 of the archive GitHub makes of it |
| `glyphs.toml` | The glyph differential's excuses: the measures the deviations ledger explains, and for which glyphs |
| `requirements.txt` | The Python packages that Ink/Stitch runs with, pinned by version and hash |
| `stubs/wx/` | A stand-in for wxPython, which Ink/Stitch's extensions import as they load and writing a file never uses |

## Running the differential by hand

Fetch Ink/Stitch at the pinned commit outside the repository, give it a Python of its own, and point the
suite at both, as the `inkstitch` action does in CI. Run from a checkout, Ink/Stitch logs at its debug level,
and a glyph took it 10 seconds, against 1.6 with its `DEBUG.toml` turning the log off. `cargo xtask corpus`
fetches the fonts:

```sh
commit=$(sed -n 's/^commit = "\(.*\)"$/\1/p' conformance/inkstitch/pin.toml)
git init -q ../inkstitch-pinned
git -C ../inkstitch-pinned fetch -q --depth 1 https://github.com/inkstitch/inkstitch "$commit"
git -C ../inkstitch-pinned checkout -q FETCH_HEAD
printf '[LOGGING]\ndisable_logging = true\n' > ../inkstitch-pinned/DEBUG.toml
python3 -m venv target/inkstitch-venv
target/inkstitch-venv/bin/pip install --no-deps --require-hashes -r conformance/inkstitch/requirements.txt
cargo xtask corpus
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

## Ink/Stitch's fonts

`cargo xtask corpus` fetches the archive of inkstitch/embroidery-fonts at the commit `fonts.toml` pins, checks
its SHA-256 and unpacks it in a cache outside the repository: the folder `STITCHCRAFT_CORPUS` names, or
`stitchcraft/corpus` in the user's cache folder. It lists the fonts it takes, those whose licence file is the
OFL, CC0, CC BY or CC BY-SA, and the others. No font file is ever committed.

The `inkstitch-fonts` case (`conformance/cases/inkstitch/fonts.toml`) cuts each glyph out of its glyph file
as a design of its own, its group shown and the other glyphs left out, which both programs sew. A run sews
the case's `sample` of glyphs of each font, or every glyph with `--every-glyph`, and `--shard 2/6` sews the
second of 6 parts of them:

```sh
cargo xtask conformance --differential --filter inkstitch-fonts                  # the nightly sample
cargo xtask conformance --differential --filter inkstitch-fonts --every-glyph    # every glyph, for hours
```

The report is `target/conformance/fonts.md`, with a row for each font, the excuses with the glyphs each
excused, the stitch types StitchCraft doesn't sew yet with their glyphs, and each glyph that differs with its
measures. Beside it, `fonts.json` gives every glyph's verdict for tools, and `fonts/` keeps the design of
each glyph that differs or didn't sew, by font and file, with Ink/Stitch's PES file of it.

`glyphs.toml` excuses a difference StitchCraft makes on purpose by the ledger's entry that explains it. Each
excuse is for the glyphs where its cause is, picked by the diagnostics StitchCraft gives them (`codes` and
`saying`), by their stitch types (`types` and `only_types`), or by their settings and fonts (`settings` and
`fonts`). With `within_mm`, an excuse counts only where each program's thread lies that near the other's:

```toml
[[excuse]]
measures = ["stitches"]
deviation = "DEV-FIN-001"
codes = ["SC-I0703", "SC-I0504"]
```

A difference the weekly run finds that no one has explained yet goes in as `[[known]]`, with a `follow_up`
saying what is known of it in place of the deviation. The case then passes, and the report shows the glyph
apart, until the difference is explained or reproduced as a bug by a case of its own.

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
program, with the tool `tool` names and the options it sets by Ink/Stitch's names, and compares the 2
routes, each sewn by its own program (`REQ-ROUTE-008`, `REQ-ROUTE-013`). The tool is `satins`, Auto-Route
Satin Columns, unless `tool = "runs"` names Auto-Route Running Stitch. Ink/Stitch's tool runs as
`--extension=auto_satin` or `--extension=auto_run`, on the elements painted by `style` attributes, as
Inkscape paints them: Auto-Route Running Stitch gives the elements it makes the style of their strokes and no
other paint. It writes the SVG file it routed, `route-<case>.routed.svg`, beside StitchCraft's route as an SVG
file, `route-<case>.stitchcraft.svg`:

```toml
id = "inkstitch-route-ts17c-lines-in-order"
requirements = ["REQ-ROUTE-013"]
kind = "inkstitch"
sheet = "TS-17C"

[route]
elements = "ts17c:lines-drawn:"
tool = "runs"
preserve_order = true
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
