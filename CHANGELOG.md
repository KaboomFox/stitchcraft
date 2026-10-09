# Changelog

All notable changes are listed here, newest first. Golden-file changes must be listed with their reason.

## Unreleased

### Added
- **Prose lint.** `cargo xtask prose` runs Vale on the Markdown lines that a branch adds. It checks the
  project's own style and ai-tells, a published style for phrasing that machine-written text overuses. The
  rules are in the new [writing style](docs/src/contributing/writing-style.md) page, with a glossary. CI
  builds Vale and runs the check, and locally it runs when Vale is installed.
- **Docs audit.** `cargo xtask docs --check` reports a sentence of 12 or more words that is on two pages,
  and a path of the repository in inline code that does not exist.
- **Design pages follow the code.** Each docs page that describes code names its source files in an
  `implements` comment. Every source file of a crate or app is on such a page. `cargo xtask docs for PATH`
  lists the pages for a file, and a Claude Code hook lists them after each edit. A branch that changes a
  page's files changes the page too, or a commit message records that it still holds.
- **SVG input page.** A new design page describes what the SVG adapter reads and what it reports.
- M3.6: manual stitch (`stitchcraft_engine::generators::manual`).
  - **Needle points.** A needle point goes on every node of the path, in order. A curve gives only its
    end node.
  - **Longest stitch.** Stitches longer than `max_stitch_length_mm` are split into equal parts, never
    shorter than the shortest stitch. As in Ink/Stitch, 0 or less means no maximum: an optional length
    of 0 or less now counts as empty, where it used to be clamped up with `SC-W0102`.
  - **Bean stitch** applies; repeats don't.
  - **Shortest stitch.** No hand-placed stitch is shorter than the shortest stitch. A point too close to
    the one before is left out, the last point is kept, and the new `SC-W0403` says how many. Ink/Stitch
    drops such points silently.
  - **Conformance.** `REQ-RUN-006` and the new `REQ-RUN-008` are active.
- `SC-I0605`: writing DST says at how many places its machines will cut the thread where the plan does
  not trim. `stitchcraft_formats::encode` returns the file as `Encoded`, its bytes with notes on
  what the format makes the machine do that the plan does not say; `stitch convert` and
  `stitch testsheet` print them.
- M3.5: repeats, bean stitch and random length for the running stitch.
  - **Repeats.** `repeats` sews a run several times, every other pass backwards. Each pass starts where
    the last one ended, so a turnaround is never a stitch in place.
  - **Bean stitch.** `bean_stitch_repeats` sews each stitch 2b + 1 times. A list such as `"1 0"` is taken
    in turn along the stitches and runs on across repeats, each turnaround taking one step, as in
    Ink/Stitch. With a two-value list, each stretch is sewn the same way on every pass.
  - **Random length.** `enable_random_stitch_length` draws each stitch from its length ±
    `random_stitch_length_jitter_percent` and starts each span at a random phase, still ending exactly on
    the corners. The element's own generator draws the lengths, seeded with `random_seed`, so the same
    element and seed always give the same stitches.
  - **Where it lives.** Repeats and bean stitch are in `generators::passes`, shared with manual stitch
    from M3.6.
  - **Conformance.** `REQ-RUN-004`, `REQ-RUN-005` and the new `REQ-RUN-007` are active.
- M3.4: the running stitch (`stitchcraft_engine::generators::running`).
  - **Placement.** Curves are flattened to within a tenth of `running_stitch_tolerance_mm`. Corners
    (turns of more than 30° between segments) always get a needle point. Stitches are spread evenly between
    corners, so none is longer than its length and none is a short leftover. A pattern of lengths (`"3 1"`)
    repeats along the path. A stitch that strays from the curve by more than the tolerance is split.
  - **Measured straight.** Stitch lengths are the straight line between needle points. Where a path bends
    back within less than the shortest stitch (a cusp, a tight loop, a small closed shape), needle points
    are dropped rather than sewing a stitch that short. When the rules disagree, the shortest stitch wins,
    then corners, then the tolerance.
  - **Diagnostics.** `SC-W0401` reports a part of a path too small to stitch. `SC-W0402` reports a stitch
    length below twice the shortest stitch, which is raised to it.
  - **Shared code.** Strokes are flattened by the engine itself, not with `kurbo`, whose flattening uses
    the platform's maths library. Lengths are compared with one shared slack,
    `stitchcraft_core::units::LENGTH_SLACK`, which the plan checker uses too.
  - **Conformance.** `REQ-RUN-001` to `REQ-RUN-003` are active.
- M3.3: the engine's input model and the SVG reader. A `Design` holds elements in stitching order, each a
  stroke or a fill with exact lines and curves in millimetres, a thread and its parameters; `Design::new`
  checks that ids are unique and every point lies within 10 m. `stitchcraft_svg::read` turns an SVG file
  into one: paths and the basic shapes, groups and `<switch>`, transforms, the root's size and viewBox,
  fill and stroke colours (`currentColor`, `paint-order`, gradients by their first colour) and everything
  that hides an element. Arcs become cubic curves within a micrometre, and all of it computes the same on
  every platform. What it leaves out or simplifies is reported: `SC-W0802` for SVG features it does not
  stitch (text, images, clones, style sheets, clipping, masks, filters, markers, patterns, Ink/Stitch's
  settings, which arrive in M8), `SC-W0804` for geometry it cannot use; `SC-E0801` refuses what is not
  SVG. `REQ-SVG-001` and `REQ-SVG-002` are active, and `read_svg` joins the nightly fuzzing.
- M3.2: every registered diagnostic code has a case. A test named `diag_sc_<code>_<what>` produces the
  code from real input and checks the exact text a user reads; `cargo xtask conformance` fails for a code
  without one and lists every code with its cases in the report. All 13 codes have one.
- M3.1: the parameter registry. Parameters are declared once, beside the code that uses them, with
  `params!`; the typed struct, validation (`SC-E0101` for a value that cannot be used, `SC-W0102` for one
  clamped into range, `SC-W0105` for a key StitchCraft does not know, which is kept), the reference pages
  (`docs/src/user/reference/params.md`), a JSON Schema and the Ink/Stitch contract's StitchCraft column
  are generated from the declaration. The first declaration holds the 14 settings every stitch type shares
  (locks, trims, stops, shortest stitch and jump) with Ink/Stitch's keys and defaults; they change the
  stitches from M3.7. `cargo xtask docs --check` cross-checks the registry with the Ink/Stitch contract and
  the deviations ledger, whose first entry (`DEV-LCK-001`) records that the lock shapes are StitchCraft's
  own. `REQ-PRM-001` and `REQ-PRM-002` are active.
- M2.8: quality baselines. Each library crate's public API is a committed snapshot
  (`crates/*/public-api.txt`, `cargo xtask api`), so an API change shows in the pull request's diff.
  Rustdoc builds without warnings. Line coverage per crate may not drop below its floor
  (`conformance/coverage.toml`, a CI job). Mutation testing runs weekly against recorded counts of
  mutants no test notices (`conformance/mutation.toml`). In GitHub Actions every `cargo xtask` finding
  is an annotation.
- M2.5: coverage-guided fuzzing of the readers (`fuzz/`: `read_pes`, `read_dst` and
  `read_write_preview`, which also writes, reads back and previews whatever it read), an hour every
  night with a corpus that grows from night to night; the target bodies run on every pull request
  (`stitchcraft-testkit::fuzz`). `REQ-FMT-006` is active.
- M2.7: `stitch preview FILE -o FILE.png` (`--style realistic|simple`, `--scale`) draws any PES, PEC or
  DST file as it will sew; `stitch convert FILE -o FILE` rewrites one in another format; diagnostic
  `SC-W0604` when a file stores no thread colours. Documentation images are generated by
  `cargo xtask shots` and compared byte for byte on every pull request: the test-sheets reference and the
  first-sew-out tutorial now show each sheet. The `docs:refresh-shots` label regenerates them on a pull
  request (`docs-refresh.yml`).
- M2.6: preview renderer (`stitchcraft-render`) in a realistic and a simple style, drawn from the
  positions a machine file makes (`REQ-RND-001`) and byte-identical on every platform (`REQ-RND-002`);
  diagnostic `SC-E0005` for previews larger than 4,096 pixels on a side. Writers and previews share one
  rounding function, `Point::to_tenths` in `stitchcraft-core`.
- M2.4: an independent reader, pinned pyembroidery, reads every golden machine file the way
  StitchCraft's reader does (conformance case `pyembroidery-oracle`, run in CI).
- M2.3: round-trip property tests for every writer/reader pair, judged by machine-visible behaviour
  (`stitchcraft-testkit::equivalence`), with fixed seeds per PR and fresh seeds nightly.
- M2.1–M2.2: PES/PEC and DST readers (any PES version's PEC block; trims inferred from DST jump runs)
  and `stitch inspect` for any machine file, with an optional profile check; diagnostic `SC-E0603`.
- M1 (machine checkpoint MC-1 pending): budgets and the diagnostics registry (`stitch explain`); the
  stitch plan, its invariant checker and the `brother-200x200` profile with hoop and comfort-zone
  diagnostics; the Brother PEC palette with CIEDE2000 matching; PES v1 and DST writers; test sheets
  TS-01, TS-02 and TS-10A/B/C (`stitch testsheet`); `stitch profiles`; the conformance runner and its
  report (`cargo xtask conformance`), with requirements named by Rust tests (`req_<area>_<nnn>_…`);
  reference pages generated from the code (command line, profiles, formats, test sheets, diagnostic
  codes) and the first-sew-out tutorial with real output.

### Changed
- The Ink/Stitch contract is checked against Ink/Stitch's source. `conformance/inkstitch/check_params.py`
  compares every row of `inkstitch-params.toml` with the parameter declarations in an Ink/Stitch
  checkout, along with the method and lock identifiers. Ink/Stitch is parsed as text; nothing from it
  is imported or copied. At `d59c9ab`, all 145 parameters agree in name, type, unit and default. The one
  gap was four conditions: the lock scales apply only to some lock shapes (`*_scale_mm` to
  back-and-forth and custom locks, `*_scale_percent` to the drawn shapes and custom, neither to the half
  stitch), and the data now says so.
- Reading Ink/Stitch's source is allowed; copying it is not (ADR-0012, replacing decision 2 of ADR-0001).
  Behaviour still goes into the design docs in our own words and code is written from them, so the
  details its documentation leaves out (parameter edge cases, how lists repeat, the font files) can be
  checked against what Ink/Stitch does instead of guessed. This is VectorCraft's own line: its `AGENTS.md`
  forbids copying GPL code, not reading it. `cargo xtask cleanroom` still fails on GPL licence text and
  pasted Python source, and no longer on Ink/Stitch file paths, so a design doc can link what it read.
- Mutation testing on pull requests runs only the mutants in the lines a pull request changes, which
  takes minutes, and fails for any that no test notices unless it is a listed equivalent
  (`[[equivalent]]` in `conformance/mutation.toml`). The full run, with the per-crate comparison, is
  weekly and on demand; with the SVG reader it had grown to about 2,350 mutants, a quarter of an hour
  per part on every pull request that touched the baseline.
- M0.9: StitchCraft can move into VectorCraft's repository (ADR-0011). `cargo xtask compat join DIR`
  makes this folder, copied into a VectorCraft checkout, part of VectorCraft's workspace (or, with
  `--nested`, a workspace of its own that VectorCraft's crates can depend on). It edits VectorCraft's
  `Cargo.toml` in place and refuses when the two cannot merge. The tooling package is now
  `stitchcraft-xtask` (still `cargo xtask`); crates set `publish = false` themselves; every `cargo xtask`
  step works on StitchCraft's packages only, so it means the same inside VectorCraft's workspace. The
  daily `move` job in `compat.yml` rehearses both forms against VectorCraft's latest release and `main`.
- M0.10: the clean room is tighter. The two pages that reviewed Ink/Stitch are gone: one described
  Ink/Stitch's source code, which the clean room forbids even second-hand, and their lessons already live
  in StitchCraft's own requirements and decisions, which now give their own reasons. Ink/Stitch is named
  only where file compatibility needs it (its SVG attribute names, the compatibility contract, the
  deviations ledger) and in the notices saying StitchCraft is independent. `cargo xtask cleanroom` now
  scans every text file, documents included.

### Fixed
- **PEC files without the origin field.** pyembroidery 1.4.32 to 1.5.1 write PEC stitch data without the
  4 bytes that Brother's software writes before the first record. The PES/PEC reader skipped those bytes
  in every file. A file from those versions lost its first record or failed to read. The reader now
  takes the bytes as the field only if the design after them fits the header's box, and otherwise warns
  that Brother machines expect the field (`REQ-FMT-009`). Found by reviewing pystitch's fixes.
- **Palette credits.** `NOTICE` credits pystitch for palette entries 62 and 63, the appliqué functions.
- The SVG reader no longer stitches Ink/Stitch's own objects (`REQ-SVG-003`), which a review against
  Ink/Stitch found.
  - **Commands and connectors.** Command symbols (`<use>` of an `inkstitch_*` symbol) were reported as
    clones, and the connector from each symbol to its object was sewn as a line in its own colour. Lines
    drawn with Inkscape's connector tool were sewn too. None is stitched now. A connector that ties no
    command is noted.
  - **Commands applied.** `trim` and `stop` commands set the shape's `trim_after` and `stop_after`.
  - **Leaving out.** The `ignore_object` and `ignore_layer` commands, and the `inkstitch:ignore_object`
    setting, leave out what they name, and the new `SC-I0805` lists each (`REQ-ASM-004`, now active).
  - **Not applied yet.** `origin` and `stop_position` are noted as not applied yet. A trim or stop on
    something that is not a stitched shape is noted, as is an `ignore_layer` outside every layer.
  - **Helper paths.** Paths that carry Ink/Stitch's guide-line, anchor-line or pattern start marker are
    helpers for other shapes. They are no longer stitched; each is noted, since StitchCraft does not
    apply them yet.
  - **Markers.** A marker property set to `none` no longer hides another one that names a marker, and
    each of the three inherits on its own.
  - **Work.** Reading charges two units of work per XML node: one to find ids and commands, one to read.
- DST is read as DST machines sew it (`REQ-FMT-008`). Machines count jump records in a row and cut the
  thread before three or more — a machine setting; three is the common one, and pyembroidery's reading —
  where something was sewn since the thread was last cut or changed, so a jump longer than 24.2 mm is a
  trim too. StitchCraft recognised only its own spelling of a trim (three small jumps back to where they
  started), so a DST file with a long untrimmed jump read back without the cut its machines make, and
  pyembroidery read such a file differently. Found by the pyembroidery oracle on M3.9's plan golden;
  reproduced on `main` with the new canonical plan `long-jumps`. Round trips through DST are compared
  with what DST machines do (`stitchcraft_testkit::equivalence::dst_events`), and the fuzz body now
  checks DST read-backs as well as PES.
- The compatibility contract check took the lock shape `zigzag`, listed for `lock_*_scale_percent`, for
  the satin method of the same name, and so thought those parameters applied to zigzag satins only. A
  stitch type is now named only by an id of the row's own family; the common settings have none. It
  showed once the contract check (which records the lock shapes each size is shown for) and the running
  stitch's matching by stitch type were both on `main`, and failed its CI.
- The Ink/Stitch contract page matches a registered parameter with the rows for the stitch types it
  applies to, not every row with its key. Ink/Stitch gives some keys to several elements with different
  defaults: registering the running stitch's `running_stitch_tolerance_mm` (0.2 mm) had marked the
  satin's and the fill's (0.1 mm) registered too. A declaration for a stitch type Ink/Stitch does not give
  the key is now reported.
- Mutation testing deals the mutants to its four parts round-robin. Cut into consecutive slices, one part
  held every mutant of the formats crate, whose tests are the slowest, and took 23 minutes while the
  others took 4 to 7.
- Test-sheet drawing charges the stitch budget. Mutation testing found that a sign error in the drawing
  code would make lines grow without bound and use up memory before any check ran.
- Broken and ambiguous links in the formats crate's API documentation.
- The test-sheets reference said "1 stops"; counts are now worded the way `stitch` prints them.
- DST reading: a long jump followed by a long move back could be read as a trim (their split pieces
  cancelled exactly); trims are now runs of jumps of at most 1 mm, and the writer no longer writes
  zero-length jumps.

### Golden files
- Added `conformance/golden/formats/long-jumps.pes` and `.dst`: untrimmed jumps of two and three DST
  records, from the start, between stitches and after a thread change. DST machines cut before the one
  between stitches only; the pyembroidery oracle reads both files.
- Added `conformance/golden/render/`: previews of a sampler design (simple and realistic) and of TS-01
  (realistic). They pin how previews look; a change to them is a change users will see.
- Added `conformance/golden/formats/` (canonical plans `every-command` and `one-stitch`, PES and DST)
  and `conformance/golden/testsheets/` (the exact MC-1 files: TS-01 PES and DST, TS-02, TS-10A/B/C PES).
- M0 bootstrap: design documents, roadmap and machine-testing protocol; layered workspace with
  `stitchcraft-core` foundations (millimetre units, deterministic math, SplitMix64); `cargo xtask` gates
  (`ci`, `layers`, `docs --check`, `conformance --check`, `cleanroom`, `unsafe-audit`, `filesize`,
  `wasm`); GitHub Actions for CI, docs, VectorCraft compatibility discovery and nightly checks.
