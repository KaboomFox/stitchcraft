# Conformance testing

Ink/Stitch shows what happens when behaviour is specified only by its implementation: fixes for one
input break another, and nobody can say what "correct" means ([finding F1](inkstitch-analysis.md#f1--tests-cover-almost-nothing-of-the-stitch-engine)).
StitchCraft specifies behaviour as **requirements**, proves each with **cases**, and reports the result
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
rationale = "Ink/Stitch #3395: compensation by buffering closed deliberate gaps."
milestone = "M5"
status = "planned"        # planned | active | retired
```

Rules, enforced in CI:

- ids are unique and never reused; retired requirements stay in the file (`--check`);
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

**Data cases**, `conformance/cases/<area>/<name>.toml`: an input, a profile and expectations, run by the
suite in-process. Unknown fields are errors, so a typo cannot silently switch a check off. M1 has the
`testsheet` kind:

```toml
id = "ts-10b"
requirements = ["REQ-PLAN-001", "REQ-PLAN-002", "REQ-PLAN-003", "REQ-PLAN-005", "REQ-PLAN-006", "REQ-PRF-002", "REQ-FMT-001"]
kind = "testsheet"
sheet = "TS-10B"
profile = "brother-200x200"

[expect]
size_mm = [190.0, 150.0]
diagnostics = ["SC-W0702"]                       # exactly these codes, no others
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
without it the case is skipped locally (⏭ in the report) and fails in CI, like every optional tool.

From M3, `design` cases take an SVG or JSON design and parameters, and check generator properties:

```toml
id = "fill-tatami-gap-preserved"
requirements = ["REQ-FILL-TAT-006", "REQ-FILL-TAT-007"]
input = "fixtures/fill/c-shape-gap-1mm.svg"     # or an inline design
profile = "brother-200x200"

[params]
fill_method = "tatami_fill"
row_spacing_mm = 0.4
pull_compensation_mm = 0.6                       # wider than half the gap

[expect]
invariants = "all"                               # every L0 plan invariant
holes = "same-as-input"
components = "same-as-input"
inside_region_tolerance_mm = 0.65
diagnostics = []                                 # exactly these codes, no others
golden = "golden/fill-tatami-gap-preserved.csv"  # stitch list after quantization
```

Inline designs are allowed for small cases; fixtures are SVG or JSON designs under
`conformance/fixtures/` (small, hand-written or generated, with a row in the fixtures index stating their
origin and licence). Large real-world corpora live in a separate repository pinned by commit and
SHA-256, downloaded by `cargo xtask corpus` — the same policy VectorCraft follows for its corpora.

## Levels

### L0 — Plan invariants (every PR)

The [plan invariants](data-model.md#plan-invariants) run on *every* plan the
suite produces, whatever the case is about. A case never needs to ask for them.

### L1 — Formats (every PR; fuzzing nightly)

Golden bytes for canonical plans; round trips on random plans (`proptest`); every command per format;
empty and degenerate files; the **pyembroidery oracle** job (pinned version, in its own CI job) decodes
our files and must agree on every stitch; `cargo-fuzz` targets run nightly with a persisted corpus
([formats](formats.md#conformance)).

#### Fuzzing

`fuzz/` holds one target per reader (`read_pes`, `read_dst`) and one for a file's whole journey
(`read_write_preview`: read, write in every format, read the PES file back, preview). Each target is
one line; its body is in `stitchcraft-testkit::fuzz`, so every pull request runs the bodies on the golden
files, every shortening of them and random bytes, on stable Rust and all three operating systems. The
properties (`REQ-FMT-006`): no panic; at most 2,000,000 entries and no position beyond ±10 m from any
reader; a plan read from anything is written in every format, and its PES file reads back to a plan the
machine sews the same way; a preview never panics.

Every night `nightly.yml` fuzzes each target for 20 minutes with libFuzzer (`-timeout=10`,
`-rss_limit_mb=2048`). The corpus starts from the golden machine files and is kept, minimised, in the
Actions cache, so each night continues where the last stopped. A crash fails the job and keeps the input
as an artifact. To work on one locally:

```sh
cargo +nightly fuzz run read_pes                        # until stopped; corpus in fuzz/corpus/read_pes
cargo +nightly fuzz run read_pes fuzz/artifacts/read_pes/crash-…   # reproduce a crash
```

A crash becomes a regression test next to the code it found a bug in, before the fix.

### L2 — Generator properties (every PR)

Property tests per generator ([algorithms](algorithms/README.md)), run over:

- hand-written cases (the shapes people actually draw: letters, circles, leaves, rings, stars);
- the **degenerate corpus** (zero-area, hair-thin, self-touching, collinear, duplicate points, single
  points, near-coincident rings, huge coordinates, 10⁶ vertices);
- random shapes from `stitchcraft-testkit` strategies, with fixed seeds in PR CI (fast, reproducible)
  and fresh seeds nightly (with failing seeds saved as new cases).

Metrics are defined once in `stitchcraft-testkit::metrics` so every case measures the same way:

| Metric | Definition |
|---|---|
| Coverage | Rasterize the region and the stitch lines (thread width 0.4 mm) at 0.05 mm; covered fraction of region pixels |
| Containment | Maximum distance of any stitch point outside the region (0 if inside) |
| Row spacing | Median and spread of perpendicular distances between consecutive rows |
| Stitch length | Min, max and distribution of `Normal` stitch lengths by role |
| Furrows | Longest run of rows whose needle points align within 0.1 mm |
| Topology | Holes and components of the region vs. of the stitched area |

### L3 — Ink/Stitch differential (nightly)

A pinned Ink/Stitch release runs as a black box on a corpus of Ink/Stitch-authored SVGs (our own or
CC0), in a container, producing stitch files. We compare **metrics, not stitches**: stitch count
(±10 %), bounds (±0.5 mm), colour sequence (exact), trims and jumps (±1), coverage IoU (≥ 0.9). Running
Ink/Stitch as an oracle uses it, it does not copy it; implementers see metric reports, not Ink/Stitch
code. Differences we intend are recorded in the **deviations ledger** (`conformance/deviations.toml`),
each with a reason and a link to the requirement that motivates it (for example row-end compensation,
L4 of the issues review). Starts in M8.

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
- `hashes.json` — one SHA-256 per output, for the [cross-platform determinism](determinism.md#cross-platform-check) job;
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
| Every PR | L0, L1 (no fuzzing), L2 with fixed seeds, `--check` rules, cross-platform hashes, coverage floors |
| Nightly | L1 fuzzing, L2 fresh seeds, L3 differential |
| Weekly | Mutation testing of every shipped crate, against the recorded counts |
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
