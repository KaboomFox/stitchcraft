# Guardrails

Guardrails keep the code clean while many people (and agents) change it. Each one below is a check
that runs automatically — locally with `cargo xtask ci`, and on every pull request in GitHub Actions —
so the rules hold without anyone having to remember them. Most are adopted from VectorCraft; the
[ADR-0009 audit](adr/0009-adopt-vectorcraft-conventions.md) says which, and what we changed.

## The checks

| Guardrail | Enforces | Configured in | Runs |
|---|---|---|---|
| Format | One style, no bikeshedding | `rustfmt.toml` | `cargo xtask ci` · CI |
| Clippy, warnings denied | Idiomatic, bug-prone patterns flagged | workspace `[lints]`, `clippy.toml` | `cargo xtask ci` · CI |
| **No panics** | No `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`, `dbg!` outside tests; `indexing_slicing` denied in parsers | workspace `[lints.clippy]`, crate attributes | `cargo xtask ci` · CI |
| **No `unsafe`** | `unsafe_code` denied in the workspace and forbidden at every crate root, the compatibility probe's included, except the plug-in's: its ABI shim alone may use `unsafe`, with a `// SAFETY:` comment of its own above each block | workspace `[lints.rust]`, `cargo xtask unsafe-audit` | `cargo xtask ci` · CI |
| **Determinism** | No `HashMap`/`HashSet`, `RandomState`, platform `sin`/`cos`/`atan2`/`powi` and their kin, `mul_add`, `SystemTime` or `Instant`; no files, threads, environment or processes in library crates; `rand` and `getrandom` only through proptest | `clippy.toml` `disallowed-types` / `disallowed-methods`, `deny.toml` `[bans]` | `cargo xtask ci` · CI |
| **Layering** | Crates depend only on lower layers ([architecture](architecture.md)); dependencies like `clap` or `tiny-skia` only where allowed | `xtask/src/layers.rs` (append-only table) | `cargo xtask layers` · CI |
| **Module size** | Rust files warn above 800 lines, fail above 1,500 (generated files exempt) | `xtask/src/filesize.rs` | `cargo xtask filesize` · CI |
| WebAssembly | L0 to L3 crates and the plug-in build for `wasm32-unknown-unknown`, and `stitch`, built for WASI and run by wasmtime, makes every golden file of the conformance cases byte for byte | `xtask/src/wasm.rs`, `xtask/src/conformance/wasm.rs` | `cargo xtask wasm` · `cargo xtask wasm --goldens` · CI |
| **Movable into VectorCraft** | Every package is `stitchcraft-*`; crates inherit only workspace keys VectorCraft defines; tooling works from its own folder on StitchCraft's packages ([ADR-0011](adr/0011-movable-into-vectorcraft.md)) | `xtask/src/layers.rs`, `xtask/src/util.rs`, `xtask/src/join.rs` | `move` job in `compat.yml` · daily and on PRs touching manifests or tooling |
| **No GPL code** | No GPL/AGPL licence text, in any case, nor a module's short GPL notice, and no pasted Python source anywhere in the repository: every text file, documents and files without an extension included ([ADR-0012](adr/0012-read-dont-copy.md)) | `xtask/src/cleanroom.rs` | `cargo xtask cleanroom` · CI |
| Docs | Generated pages fresh and none left over; links, anchors (as mdBook makes them) and included files valid; mentioned `cargo xtask` commands exist; `REQ-` and `DEV-` ids exist, and a code is registered once its milestone is done; no long sentence on two pages; every repository path named exists; each stitch type the engine sews has a page with at least two pictures | `xtask/src/docs.rs`, `xtask/src/docs_audit.rs`, `xtask/src/markdown.rs`, `xtask/src/stitch_pages.rs` | `cargo xtask docs --check` · CI |
| **The published book** | The pages mdBook builds link only to files of the book and to ids on their pages. Their links to the repository's files on `main`, each page's edit link among them, name files of the checkout | `xtask/src/book.rs`, `docs/book.toml` | `cargo xtask book` · `cargo xtask ci` · CI |
| **Design pages follow the code** | Every source file of a crate or app is on a page's `implements` comment, and a branch that changes those files changes the page too or records a `Design-reviewed:` line | `xtask/src/design_map.rs`, `.claude/settings.json` (names the pages after each edit) | `cargo xtask docs --check` · CI |
| Docs images | Every image a page shows is one a shot in `docs/shots.toml` writes, with alt text, and regenerating to the committed file. VectorCraft's own pictures, which its pinned release makes, regenerate to within a tolerance of theirs, and its plug-in dialogs show a label for each parameter | `xtask/src/shots/`, `compat/vc-window/tests/shots.rs` | `cargo xtask shots --check` · CI; `cargo xtask shots --vectorcraft --check` · `shots-ui` job |
| Conformance | Requirement/case consistency; every fixture has its row in the fixtures index; all cases pass; changed goldens need the `golden-change` label and a changelog line | `xtask/src/conformance/`, `ci.yml`, `goldens.yml` | `cargo xtask conformance` · CI |
| Dependencies | Licence allow-list (GPL family denied), advisories, duplicates, sources, for the workspace, the fuzz targets' and the window probe's. An advisory accepted is listed with its reason | `deny.toml` | `cargo xtask ci` · CI (`cargo-deny`) |
| Spelling | Typos in code, docs, commit-facing text | `typos.toml` | CI |
| **Prose** | New Markdown lines follow the [writing style](../contributing/writing-style.md): the house rules and ai-tells, a style for phrasing that machine-written text overuses | `.vale.ini`, `.vale/styles/`, `xtask/src/prose.rs` (Vale) | `cargo xtask prose` · CI |
| MSRV | Builds on the declared `rust-version` | workspace `Cargo.toml` | CI |
| Cross-platform | Tests on Linux, macOS and Windows, among them the conformance suite's data cases, whose machine files must equal the golden files on each, and the same machine files made as WebAssembly ([determinism](determinism.md#cross-platform-check)) | `ci.yml` | CI |
| API docs | Rustdoc builds without warnings: no broken, ambiguous or private intra-doc links | `cargo xtask ci` (`RUSTDOCFLAGS=-D warnings`) | `cargo xtask ci` · CI |
| **Public API review** | Changes to a library crate's public API show in the PR diff, as a change to its `public-api.txt` | `xtask/src/api.rs` (cargo-public-api, pinned) | `cargo xtask api --check` · CI |
| **Coverage ratchet** | No crate's line coverage drops below its floor, and floors only go up. A crate below its floor has its untested lines named on the pull request, those only another build of a function or a closure on the line runs included | `conformance/coverage.toml`, `xtask/src/coverage.rs` (cargo-llvm-cov) | `cargo xtask coverage` · CI job |
| **Performance budget** | Planning the reference hoop's whole field (`REQ-FILL-TAT-009`) takes at most 2 % more instructions than recorded, and so does shrinking a region 150 mm across 300 times, as contour fill will. Records only go down. A count below half the record fails too: the counted function was lost. The times are reported against NFR-PERF-1's 50 ms and NFR-PERF-3's 200 ms, with a warning above them | `conformance/perf.toml`, `xtask/src/perf.rs` (valgrind's callgrind), the engine's `field` and `insets` examples | `cargo xtask perf` · CI job |
| **Release build** | The build `stitch` and the plug-ins are published as plans as the tests' debug build does: the engine's release tests pass in it too. A release build is compiled more aggressively, and Rust 1.97.0 got one wrong there | `crates/stitchcraft-engine/tests/release.rs`, `rust-toolchain.toml` | `perf` CI job |
| **Mutation testing** | Tests notice changed behaviour: every mutant in the lines a pull request changes is noticed or a listed equivalent; no crate gets more mutants no test notices than recorded, and records only go down. A weekly part whose unmutated tree failed its tests, or a recorded crate with no mutant tried, fails | `.cargo/mutants.toml`, `conformance/mutation.toml`, `xtask/src/mutants.rs` | `mutants.yml` · changed lines every PR, in 8 parts · everything weekly |
| **Fuzzing** | Readers never panic, respect their caps and terminate; anything read round-trips and previews | `fuzz/`, `stitchcraft-testkit::fuzz` ([conformance](conformance.md#fuzzing)) | bodies every PR · an hour nightly |

`cargo xtask ci` runs everything that does not need extra tools; tools that are not installed locally
are reported as skipped, while CI installs them and treats them as required. `cargo xtask` refuses an
option it does not know with exit status 2, before anything runs, so a misspelt `--check` cannot run a
command's other mode. In GitHub Actions every
problem a check finds is also an annotation, on the right line of the pull request's diff when it names
one, and a failed fuzz run puts the end of its output in one: the reason is visible without opening a
log.

The performance budget counts instructions, because they come out the same on any machine and in any
run, where milliseconds do not. Its check fails for a change that slows planning, on whichever runner it
runs. The time is still measured and reported, against the budget the requirement states in milliseconds.

Coverage counts each build of a function apart and takes the build that runs the most lines. The unit
tests and the integration tests use separate builds, a generic function has one per type it is used with,
and a closure is a function of its own. So a line can count as untested though the lcov report counts it
run. Only the other kind of test runs it, or only a closure on the line runs, as with a `?` after a
closure argument whose error the tests never bring. The coverage job names these lines and the function
they count against. Run them from the kind of test that runs most of the function.

`cargo xtask coverage` runs the job locally. It needs cargo-llvm-cov and rustup's `llvm-tools`. Without
the latter, `LLVM_COV` and `LLVM_PROFDATA` can name another LLVM's llvm-cov and llvm-profdata that read
the toolchain's profiles. Rust 1.99 writes raw profile format 11, which its own LLVM 23 reads and LLVM
20's does not.

The tests run in a debug build, and `stitch` and the plug-ins are published as a release build, which is
compiled more aggressively, with one code unit per crate and link-time optimization across crates. A
compiler can get that wrong where the debug build is right. Rust 1.97.0 did: its release build wrote past
the end of a vector while planning a satin column drawn as one closed path, and crashed or sewed stitches
from elsewhere. So the `perf` job, which builds the release profile anyway, also runs the engine's release
tests (`crates/stitchcraft-engine/tests/release.rs`) in it: shapes a release build once planned wrong,
whose plan they check.

## Design rules (reviewed, partly linted)

1. **Everything user-visible is data first.** A stitch type, parameter, diagnostic, profile or format is
   registered in its registry; UIs, docs and tests are generated from registries.
2. **Validate at the edge, trust inside.** Untrusted inputs are parsed into validated types once
   (`Mm`, `Region`, typed params); internal functions take those types and do not re-check.
3. **Total functions.** Every function returns for every input it accepts; impossible states are made
   unrepresentable by types rather than asserted.
4. **Bounded work.** Loops over data charge a `Budget`; recursion has an explicit depth bound.
5. **No silent fallbacks.** A degraded result always comes with a coded diagnostic.
6. **One crate, one error enum** (`thiserror`), with messages a user can act on.
7. **Docs on every public item** (`missing_docs` denied in library crates); module docs explain *why*.
8. **Tests next to code**, conformance cases in `conformance/`, integration tests in `tests/`.

## Contribution flow

- **One roadmap step per PR**, titled with its id: `M3.4: running stitch even spacing`. Small PRs
  (target under ~400 lines of non-generated change) review faster and revert cleanly.
- **Conventional commit scopes** (`feat(engine):`, `fix(formats):`, `docs:`) feed the changelog.
- **The PR template** asks for: the step id, requirements touched, cases added, docs updated, golden
  changes justified, screenshots refreshed if UI changed, whether a sew-out is needed.
- **Required checks** on `main` (branch protection): the `ci` and `docs` workflows. Merges are squash
  merges; `main` is always releasable.
- **Playbooks** make common changes mechanical: [new stitch type](../contributing/playbook-new-stitch-type.md),
  [new parameter](../contributing/playbook-new-param.md), [new format](../contributing/playbook-new-format.md),
  [new diagnostic](../contributing/playbook-new-diagnostic.md), and the [review checklist](../contributing/review-checklist.md).
- **Agents** (Claude Code and others) start at `AGENTS.md`, which links here; the same gates apply.
