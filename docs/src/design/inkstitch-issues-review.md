# Ink/Stitch issues review: lessons we build in

Ink/Stitch's issue tracker is a free, years-long field study of what goes wrong when people digitize
embroidery. We read it (all open issue titles — about 230 on 2026-10-08 — and the full discussion of
the issues marked ✔ below) and turned each recurring problem into a design decision, a requirement
or a machine test. Closed bugs were reviewed through the fix commits in the
[Ink/Stitch analysis](inkstitch-analysis.md#f2--failures-surface-as-exceptions-in-production).

## How to read this page (the grain of salt)

- **An issue is a report, not a verdict.** Some are user error, machine configuration or a
  misunderstanding; some were never reproduced. We record what was reported and, for ✔ issues, what
  the discussion concluded.
- **Many problems belong to Ink/Stitch's platform** (Inkscape, Python packaging, wxPython,
  antivirus) and do not apply to a Rust engine running in VectorCraft or as a static CLI. They are
  listed at the end so nobody spends time on them.
- **Feature requests are signals, not commitments.** They inform the roadmap's later phases.
- ✔ = issue page read in full; others are cited by title only.

## Lessons and design responses

### L1 — Edge-case geometry and inputs raise exceptions

| Issue | Report |
|---|---|
| #4552 ✔ | `TypeError` in the satin centre-line code for a satin made by "Fill to Satin" whose rung does not intersect the rails |
| #3342 ✔ | `ZeroDivisionError` while converting an SVG arc to a path (from Ink/Stitch's SVG parsing dependency) |
| #3947 ✔ | `TypeError` in even running stitch on a ~110 cm design with ~460,000 stitches; not reproducible by maintainers |
| #4039 ✔ | `ValueError: min() arg is an empty sequence` opening a DST file (empty bounding box) |
| #3041 ✔ | `TypeError` in the PEC reader when opening a PES saved from a design with nothing embroiderable |
| #3905, #3748, #4297, #4545, #4546, #4550 | GEOS exception on PES import; `KeyError` parsing an Illustrator file; several "unexpected error" reports |

**Responses**

- *Validate, then generate*: a generator never runs on a shape that failed validation; an invalid
  satin is `SC-E02xx` with the rung highlighted, not an exception ([satin](algorithms/satin.md)).
- SVG path data is parsed by our adapter with degenerate arcs handled as the SVG spec says (zero radius
  → straight line), and fuzzed (`REQ-SVG-002`).
- We never write an empty machine file (`SC-E0010` "nothing to stitch"), and readers accept empty and
  zero-stitch files without failing (`REQ-FMT-004`).
- Hoop limits stop absurd sizes before planning (`SC-E0701`), and budgets bound work (`SC-E0004`).
- **Reproducibility built in:** `stitch bug-report` writes a self-contained bundle (normalized
  design, parameters, profile, version, diagnostics) — determinism means the maintainer sees exactly
  what the user saw. Ink/Stitch maintainers ask for SVG files in nearly every bug thread.

### L2 — Machines disagree about files

| Issue | Report |
|---|---|
| #689 ✔ | **Trims ignored in PES on Brother machines** (Innov-ís V3 Limited Edition, PR-655); a related pyembroidery issue is cited; no root cause |
| #1853 ✔ | An older Brother-family machine (Baby Lock Ellageo, 160 × 260 mm hoop) does not list Ink/Stitch PES (version 1) designs larger than about 130 × 180 mm, while larger PES files from other software (newer PES versions) load; unresolved |
| #2668 ✔ | PES colours: thread catalogue numbers are dropped and colours snap to the nearest Brother PEC thread; some machines show only PEC colours |
| #3137 ✔ | JEF colour numbers wrong on a Janome MC500E: the writer uses its own thread table, not the chosen palette |
| #3544 ✔ | STOP commands missing from DST files for a Tajima multi-needle |
| #4260, #2673, #2388, #1296, #4187, #4508, #4519 | DST not read on specific machines; PMV deformation; DST export issue; PES save error |

**Responses**

- Machine **profiles** are data, and every profile value is backed by a machine-testing record.
- **For our Brother 200 × 200 mm machine, MC-1 tests exactly these failure modes**: trims (both
  trim-flagged jumps and the jump-threshold behaviour), and designs at 150 mm and 190 mm wide. Our
  hypothesis for #1853 (unconfirmed by its thread) is the PES v1 header's hoop indication, which
  Ink/Stitch's writer sets to the 130 × 180 mm class — a 150 mm-wide design is already beyond it. If
  PES v1 fails, we test PES v6 with explicit hoop dimensions
  ([machine testing](../plan/machine-testing.md), [formats](formats.md#pes-and-pec)).
- Every command (trim, stop, colour change, jump) is round-tripped per format by conformance level L1
  and sewn in test sheet TS-02 (`REQ-FMT-003`).
- Colour mapping is per format and per palette, with the palette's catalogue codes carried where the
  format can store them (PES v6 thread list) and spot-checked against published tables (`REQ-THREAD-001`).

### L3 — The preview does not match the sew-out

| Issue | Report |
|---|---|
| #2066 ✔ | The simulator shows unrounded coordinates; files round to 0.1 mm, so stitches "move" after saving |
| #3853, #4468 | Show lock stitches, trims and jumps in the simulation |

**Responses:** previews render the plan *after* quantization (decode(encode(plan))), so what you see
is what sews (`REQ-RND-001`); commands and lock stitches are drawn with distinct markers.

### L4 — Algorithms that break the shape

| Issue | Report |
|---|---|
| #3395 ✔ | Pull compensation or "expand" closes a deliberate gap; the fill then stops following the path or fails with "border crosses itself"; maintainers: not fixable without a rewrite |
| #3278 ✔ | Gap-fill rows repeat the last row along the stitch angle and stitch outside the shape |
| #1778 ✔ | Single-spiral contour fill fails in narrowing areas; inherent to the algorithm as implemented |
| #4380, #3176 | Satin adds overlap stitching at gaps; patterns don't remove stitches underneath |

**Responses**

- Fill pull compensation **extends rows at their ends** instead of buffering the whole shape, so
  holes and gaps keep their topology (`REQ-FILL-TAT-006`: compensation never changes the number of
  holes or components).
- Gap-fill rows come from the same scanline machinery as normal rows and are clipped to the region
  (`REQ-FILL-TAT-007`: containment).
- Spiral fills detect necks and split the region before spiralling, or fall back to inner-to-outer
  with `SC-W03xx` ([fills](algorithms/fills.md#contour-fill)).

### L5 — Settings lost or changed by a save

| Issue | Report |
|---|---|
| #3723 ✔ | Fill settings reverted to defaults after save and reopen in a beta; not reproducible on demand |
| #3867, #4542 | Unversioned SVG detection stuck; re-stacking objects also moves them |

**Responses:** parameter round-trip is a property test through every adapter (SVG attributes,
VectorCraft effect records) (`REQ-PRM-003`); the compatibility gate runs *save → reopen → compare*
against each VectorCraft version; parameter schema changes ship with tested migrations.

### L6 — Large designs are slow

| Issue | Report |
|---|---|
| #4482 ✔ | The simulator takes 10 s or more to open on large projects because all stitches are planned first; fills depend on neighbours' stitches, which blocks parallel planning |
| #2151, #836, #3789 | Autoroute running stitch performance; use multiple cores; Windows 11 freezing |

**Responses:** each element is generated independently from geometry-based entry/exit hints (not from
neighbours' stitches), so results are cached per element and can be planned in parallel by the CLI;
assembly only connects them ([engine pipeline](engine-pipeline.md)). Performance budgets run in CI.

### L7 — Users want control over order and layers

| Issue | Report |
|---|---|
| #1894 ✔ | Sew a satin's underlay now and its top stitching later; maintainers plan to let users choose layers and order |
| #1355, #4153, #2576, #3084, #1903, #3013, #3531, #378, #3168 | Underlay order; honour start/stop commands on strokes; fill and meander start/end; contour start/end; routing for all types; reorder and sort by colour |

**Responses:** every stitch carries a role (underlay, top, travel, lock), so splitting an element into
separately ordered layers is a plan-assembly feature, not a rewrite (phase P3). Every generator
accepts start/end hints from day one (`REQ-GEN-001`).

### L8 — Parameters are hard to manage

| Issue | Report |
|---|---|
| #3640, #1069, #3024, #2953, #3172 | Presets, saving custom defaults, a reset button, grouping parameters, named parameter styles |
| #1070, #497 | Comma as decimal separator; inches |

**Responses:** the registry defines groups, defaults and presets once; in VectorCraft, embroidery
settings live in the object's appearance as live effects, which VectorCraft's Graphic Styles are
designed to save and re-apply (verified in M6.4). Units are presented in the user's choice; values are
stored in mm.

### L9 — Hoop size awareness

| Issue | Report |
|---|---|
| #2702 ✔ | A hoop size setting that warns or rotates the design to fit was floated, not built |
| #182 | Automatically split designs for small machines |

**Responses:** profiles give a hard hoop limit and a comfort zone; the diagnostic suggests "rotate 90°
to fit" when that would work. Design splitting is a post-1.0 backlog item.

### L10 — People need to learn embroidery, not just the tool

| Issue | Report |
|---|---|
| #4109, #2480, #3247, #4509, #2590 | A glossary; getting-started docs on embroidery itself; a PDF manual; Italian translation; screen-reader use |

**Responses:** a glossary and "embroidery basics" explanation pages from the first release, a printable
docs build, translations planned in M12, and accessible VectorCraft panels (egui exposes AccessKit).

### L11 — Testing and CI were never prioritized

| Issue | Report |
|---|---|
| #245 ✔ | "Automated testing", open since July 2018; the thread proposes crash tests on sample SVGs and random generated shapes |
| #3832 ✔ | "Improve CI": coverage, linting, multiple Python versions, enforced type checks — all unchecked |

**Response:** the whole of [conformance](conformance.md) and [guardrails](guardrails.md). The random-
shapes idea from #245 is exactly our property-test strategy.

### L12 — Thread palettes drift

| Issue | Report |
|---|---|
| #3274, #3257 | Isacord and Madeira palette mismatches |

**Response:** palette tables carry their source and are tested with spot checks against the
published values; changing a table needs a source link in the PR.

## Not applicable to us

These are real problems for Ink/Stitch users, but they come from its platform, not from embroidery:

- **Installers and packaging:** #4553 (macOS Sequoia), #4534 and #4533 (AUR), #4287 (winget), #4147,
  #3848 (Linux bundle size), #3885 (Windows symbol libraries).
- **Python and native libraries:** #4161 (`iconv` on macOS), #3839 (migrating from wxPython).
- **Inkscape coupling:** #3173 (extensions folder not found), #4252, #4245 (dialogs behind Inkscape),
  #4407 (Inkscape shape transforms).
- **Antivirus false positives:** #3895, #4456.
- **Community and admin:** #4471, #3872, #4288, #1084.

We still keep one lesson from them: a single static binary and a sandboxed plug-in are a feature, so
we do not add native dependencies to the engine.

## Requirements created by this review

| Requirement | Statement | Lesson |
|---|---|---|
| `REQ-GEN-001` | Every generator honours start and end hints when given | L7 |
| `REQ-SVG-002` | SVG path data, including degenerate arcs, never fails the adapter; fuzzed | L1 |
| `REQ-FMT-003` | Every command kind round-trips through every writer/reader pair | L2 |
| `REQ-FMT-004` | Readers accept empty and zero-stitch files; writers refuse to write empty plans | L1 |
| `REQ-THREAD-001` | Palette tables match their published source at spot-checked entries | L2, L12 |
| `REQ-RND-001` | Previews render the quantized plan | L3 |
| `REQ-FILL-TAT-006` | Pull compensation preserves region topology | L4 |
| `REQ-FILL-TAT-007` | All fill rows, including gap-fill rows, lie inside the region (± tolerance) | L4 |
| `REQ-PRM-003` | Parameters round-trip through every adapter unchanged | L5 |
| `REQ-PRF-002` | A design larger than the hoop is an error; larger than the comfort zone, a warning with a rotate hint | L9 |

They are listed in `conformance/requirements.toml` and scheduled in the [roadmap](../plan/roadmap.md).
