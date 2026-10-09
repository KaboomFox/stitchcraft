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
| **No `unsafe`** | `unsafe_code = "forbid"` everywhere except the plug-in ABI shim, where every block needs a `// SAFETY:` comment | workspace `[lints.rust]`, `cargo xtask unsafe-audit` | `cargo xtask ci` · CI |
| **Determinism** | No `HashMap`/`HashSet`, platform `sin`/`cos`/`atan2`, `SystemTime`, `Instant` in library crates | `clippy.toml` `disallowed-types` / `disallowed-methods` | `cargo xtask ci` · CI |
| **Layering** | Crates depend only on lower layers ([architecture](architecture.md)); dependencies like `clap` or `tiny-skia` only where allowed | `xtask/src/layers.rs` (append-only table) | `cargo xtask layers` · CI |
| **Module size** | Rust files warn above 800 lines, fail above 1,500 (generated files exempt) | `xtask/src/filesize.rs` | `cargo xtask filesize` · CI |
| WebAssembly | L0–L3 crates and the plug-in build for `wasm32-unknown-unknown` | `xtask/src/wasm.rs` | `cargo xtask wasm` · CI |
| **Clean room** | No GPL/AGPL licence text or Ink/Stitch source paths in code, fixtures or tests | `xtask/src/cleanroom.rs` | `cargo xtask cleanroom` · CI |
| Docs | Generated pages fresh, links and anchors valid, mentioned `cargo xtask` commands exist, ids exist | `xtask/src/docs.rs` | `cargo xtask docs --check` · CI |
| Docs images | Every image declared in `docs/shots.toml`, with alt text, and regenerating to the committed file | `xtask/src/shots.rs` | `cargo xtask shots --check` · CI |
| Conformance | Requirement/case consistency; all cases pass; changed goldens need the `golden-change` label and a changelog line | `xtask/src/conformance/`, `ci.yml`, `goldens.yml` | `cargo xtask conformance` · CI |
| Dependencies | Licence allow-list (GPL family denied), advisories, duplicates, sources | `deny.toml` | CI (`cargo-deny`) |
| Spelling | Typos in code, docs, commit-facing text | `typos.toml` | CI |
| MSRV | Builds on the declared `rust-version` | workspace `Cargo.toml` | CI |
| Cross-platform | Tests on Linux, macOS, Windows; identical conformance hashes | `ci.yml` | CI |
| API docs | Rustdoc builds without warnings: no broken, ambiguous or private intra-doc links | `cargo xtask ci` (`RUSTDOCFLAGS=-D warnings`) | `cargo xtask ci` · CI |
| **Public API review** | Changes to a library crate's public API show in the PR diff, as a change to its `public-api.txt` | `xtask/src/api.rs` (cargo-public-api, pinned) | `cargo xtask api --check` · CI |
| **Coverage ratchet** | No crate's line coverage drops below its floor; floors only go up | `conformance/coverage.toml`, `xtask/src/coverage.rs` (cargo-llvm-cov) | `cargo xtask coverage` · CI job |
| **Mutation testing** | Tests notice changed behaviour: no crate gets more mutants no test notices than recorded; records only go down | `.cargo/mutants.toml`, `conformance/mutation.toml`, `xtask/src/mutants.rs` | `mutants.yml` · weekly |
| **Fuzzing** | Readers never panic, respect their caps and terminate; anything read round-trips and previews | `fuzz/`, `stitchcraft-testkit::fuzz` ([conformance](conformance.md#fuzzing)) | bodies every PR · an hour nightly |

`cargo xtask ci` runs everything that does not need extra tools; tools that are not installed locally
are reported as skipped, while CI installs them and treats them as required. In GitHub Actions every
problem a check finds is also an annotation, on the right line of the pull request's diff when it names
one, and a failed fuzz run puts the end of its output in one: the reason is visible without opening a
log.

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
