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

Rules, enforced by `cargo xtask conformance --check` in CI:

- ids are unique and never reused; retired requirements stay in the file;
- an `active` requirement must have at least one passing case;
- a milestone cannot be closed while one of its requirements is `planned`;
- docs that mention a `REQ-…` id must refer to an existing one (part of `cargo xtask docs --check`).

## Cases

`conformance/cases/<area>/<name>.toml`:

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
our files and must agree on every stitch; `cargo-fuzz` targets per reader run nightly with a persisted
corpus ([formats](formats.md#conformance)).

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

`cargo xtask conformance` (also `stitch conformance run` for users who want to validate a machine
profile) runs all cases and writes `target/conformance/`:

- `report.md` — the requirement × case matrix, failures first; posted to the PR as the job summary;
- `report.json` — machine-readable results, consumed by the docs (requirement pages show their status);
- `hashes.json` — output hashes for the [cross-platform determinism](determinism.md#cross-platform-check) job;
- `diffs/` — for failing golden cases, before/after stitch renders and a side-by-side PNG.

`cargo xtask conformance --bless <case>` rewrites a golden file; CI refuses changed goldens unless the
PR also carries the `golden-change` label and a line in `CHANGELOG.md`.

## Gates

| When | What must pass |
|---|---|
| Every PR | L0, L1 (no fuzzing), L2 with fixed seeds, `--check` rules, cross-platform hashes |
| Nightly | L1 fuzzing, L2 fresh seeds, L3 differential, mutation testing on changed crates |
| Milestone close | All requirements of the milestone `active` and green; its machine checkpoint signed off |
| Release | Everything above, plus the compatibility gate green on VectorCraft `stable` |

## Unit, snapshot and doc tests still exist

Conformance cases describe behaviour users see. Unit tests (in modules), snapshot tests (`insta`, for
intermediate structures such as cell decompositions) and CLI examples in the docs (`trycmd`) still cover
internals and documentation; mutation testing (`cargo-mutants`) and a coverage ratchet (`cargo-llvm-cov`)
measure how well all of them together bite.
