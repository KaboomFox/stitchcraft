# Ink/Stitch analysis: what to keep, what to fix

Ink/Stitch is the reference for *what* embroiderers expect. This page records what we measured in its
code, tests and documentation, so our design decisions rest on evidence rather than impressions.
The issue tracker is reviewed separately in [Ink/Stitch issues review](inkstitch-issues-review.md).

> **Clean-room note.** This page cites Ink/Stitch files and lines as *evidence*. Implementers must not
> open those files: the behaviour we need is specified in our own design pages
> ([ADR-0001](adr/0001-license-and-clean-room.md)).

**Snapshot reviewed:** `inkstitch/inkstitch` `main` at `d59c9ab` (2026-09-17), docs branch
`gh-pages` at `9f7b13c` (2026-08-28), `inkstitch/pystitch` at `b72b557` (2026-09-08). Numbers below
were produced by commands that are reproducible against those commits.

## Shape of the project

| Fact | Value | How measured |
|---|---|---|
| Licence | GPL-3.0 | `LICENSE` |
| Language / host | Python, Inkscape extension (`inkex`) with wxPython dialogs | `requirements.txt`, `lib/extensions/base.py` |
| Library size | 45,344 lines of Python in `lib/` | `find lib -name '*.py' \| xargs wc -l` |
| Embroidery parameters | 145 `@param` declarations on 5 element classes | AST scan of `lib/elements` |
| Stitch types | running, bean, manual, zigzag, ripple, satin (+ E, S, zigzag), tatami, legacy fill, contour, guided, meander, circular, linear gradient, tartan, cross stitch | `lib/stitches/`, `lib/elements/` |
| Extensions (menu tools) | 80 modules in `lib/extensions/` | `ls lib/extensions/*.py`, excluding `__init__` |
| File formats | 51 readers, 24 writers via `pystitch` (MIT) | `pystitch/src/pystitch/*Reader.py`, `*Writer.py` |
| Geometry and graphs | shapely/GEOS, networkx, trimesh, numpy | `requirements.txt` |

### What it does well (keep, or match)

- **The parameter vocabulary.** Names like `row_spacing_mm`, `pull_compensation_mm`,
  `zigzag_underlay_inset_mm` are what users and existing SVG files speak. We keep them as our
  registry keys ([ADR-0001](adr/0001-license-and-clean-room.md), [compatibility contract](inkstitch-compat-contract.md)).
- **Breadth of stitch types** and per-object control (every object carries its own parameters).
- **Commands as first-class objects**: start/end points, trim, stop, ignore, origin, stop position.
- **Validation messages with a position and steps to fix** (`lib/elements/validation.py`; about 30
  warning/error classes such as dangling rungs or self-intersecting fills).
- **Documentation breadth**: a page per stitch type with galleries and downloadable samples, a
  parameter dataset, tutorials, a troubleshooting tool, three maintained languages (en, de, fr).

## Quality findings

### F1 — Tests cover almost nothing of the stitch engine

| Measure | Value |
|---|---|
| Test code | 1,142 lines in `tests/` (674 of them clone tests) |
| Test functions | 52 (`grep -c 'def test_'`) |
| Stitch algorithms with direct tests | 0 — two output tests sew a 10 × 10 rectangle to JEF and check determinism and trims (`tests/test_output.py`) |
| CI | `pytest`, then mypy with `continue-on-error: true`, then a style check (`.github/workflows/test.yml`) |
| Non-merge commits since 2024-01-01 | 1,146, of which 22 touched `tests/` |
| Commits since 2024 whose subject reports an error, crash, invalid or empty case | 62, of which **0** touched `tests/` |

Issue #245 "automated testing" has been open since July 2018; issue #3832 "Improve CI" (June 2025)
lists coverage, multi-version testing and enforced type checks, all unchecked.

**Design response:** conformance-first development, property tests over random and degenerate
shapes, fuzzing, coverage and mutation baselines ([conformance](conformance.md)).

### F2 — Failures surface as exceptions in production

Commit subjects since 2024 name `ZeroDivisionError` 3 times, `AttributeError` 2, `IndexError`,
`KeyError` and `RecursionError` once each, GEOS/topology exceptions 4 times and `NoneType` errors 4 times,
and mention "empty" inputs 11 times, for example:

| Commit | Fix |
|---|---|
| `dbef888e` (2025-08-24) | crash with a tiny satin (#3934) |
| `ed3283d2` (2026-03-14) | skip clamping when buffering a polygon raises `GEOSException` (#4221) |
| `3a359bc1` (2025-04-27) | possible `RecursionError` for relative lock stitches (#3695) |
| `d1e84fc2` (2025-07-12) | `ZeroDivisionError` in zigzag-to-satin (#3858) |
| `a40b8107` (2025-04-18) | `NoneType` error in auto-fill travel (#3659) |

Each fix guards one input; nothing stops the next variant.

**Design response:** no-panic lints, total functions over validated inputs, budgets, and generators
property-tested on the same degenerate classes (tiny, zero-width, self-touching, collinear) that
produced these fixes ([guardrails](guardrails.md)).

### F3 — Library code ends the process

`sys.exit(1)`/`exit(1)` is called from library modules: `lib/stitch_plan/stitch_plan.py:30` (no
stitchable elements), `lib/output.py:113` and `:124` (write errors), and
`lib/stitch_plan/generate_stitch_plan.py:77` (missing file). A host embedding this code cannot recover.

**Design response:** libraries return `Result` or diagnostics; only apps exit.

### F4 — A verified bug in an error path

`lib/output.py:115` extracts the number of colour changes with the regular expression `"d+"` instead
of `"\d+"`. Against pystitch's message (`'Too many color changes, (300 out of bounds (0, 255)'`) it
matches the letter `d` in "bounds", so users are told *"There are d color changes in your design."*
Verified by running both expressions on that message. Error paths that are never executed by tests
rot.

**Design response:** every diagnostic code has a conformance case that triggers it, so its message is
rendered in CI ([diagnostics](diagnostics.md)).

### F5 — Silent degradation

- When a tatami fill has no rows or no graph (small shapes), it falls back to a running stitch around
  the first outline without telling the user (`lib/stitches/tatami_fill.py:99–117`, `:387–399`).
- When no travel path exists inside the shape, travel becomes a straight line between the two points,
  which can cross outside it (`lib/stitches/tatami_fill.py:891–893`).

**Design response:** every fallback emits a coded warning; travel that cannot stay inside the region
becomes tie-off + trim + tie-in, never an unmarked straight stitch.

### F6 — Parameters described twice, by hand

Parameter labels and tooltips live in code (`@param`); the website documents them again in a
hand-maintained multilingual dataset, `_data/params.yml` (102 parameter entries, each with
English, German and French text). Six code parameters have no entry there: `guided_fill_angle`,
`stitch_position_method`, `cross_thread_count`, `cross_rotation`, `zigzag_underlay_inset_mm`,
`zigzag_underlay_inset_percent` (checked by searching labels and names in `_data/params.yml` and all
English pages). Nothing checks the two against each other.

**Design response:** one registry generates labels, help, UI schemas and docs ([ADR-0003](adr/0003-parameter-registry.md)).

### F7 — Documentation is hand-made and unversioned

| Measure | Value |
|---|---|
| English doc pages | 51 (+ 9 tutorial pages); de 51, fr 50, ru 6, da 3 |
| Images | 1,341 files, 395 MB in `assets/images/`, all committed by hand |
| Docs CI | none (no workflows on the `gh-pages` branch) |
| Versioning | one site that tracks development; the satin page notes some parameters are not in the current release |
| Self-assessment | the docs README says the site still needs a lot of work to be complete documentation |

**Design response:** generated reference, CI-regenerated images, versioned docs, tested examples
([docs pipeline](docs-pipeline.md)).

### F8 — Platform coupling

Ink/Stitch depends on Inkscape's extension runtime, a bundled Python, wxPython (a custom wheel in CI)
and GEOS. Many open issues are installer, antivirus or OS-version problems (see the
[issues review](inkstitch-issues-review.md#not-applicable-to-us)). Running inside VectorCraft as a
sandboxed WebAssembly module, and as a single static CLI binary, removes that class of problem.

## Behaviours worth matching (functional summary)

These are behaviours users rely on; our own designs specify how we achieve them.

| Area | Behaviour users expect | Our design |
|---|---|---|
| Element type | Filled shapes become fills, strokes become running stitch, a path flagged as satin becomes a satin | [SVG adapter](architecture.md#hosts-ports-and-adapters), [VectorCraft plug-ins](vectorcraft-integration.md) |
| Satin | Two rails plus optional rungs; underlays; pull compensation per side; short stitches on curves; split long stitches | [satin](algorithms/satin.md) |
| Tatami | Rows at an angle and spacing, staggered stitch grid, underlay at a cross angle, travel hidden under later rows | [fills](algorithms/fills.md#tatami-fill) |
| Ties | Lock stitches at start/end with selectable shapes and scale | [plan assembly](engine-pipeline.md#4-plan-assembly) |
| Connections | Jumps shorter than the collapse length (default 3 mm) are sewn; longer ones tie off and jump; trims and stops by command | [plan assembly](engine-pipeline.md#4-plan-assembly) |
| Units | Parameters in mm; documents assume 96 px per inch | [data model](data-model.md#units-and-coordinates-stitchcraft-core) |
