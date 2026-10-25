# VectorCraft compatibility gate

**Goal:** know within a day when a VectorCraft change would break the StitchCraft plug-in — ideally
before VectorCraft ships it — and keep a public, generated table of which VectorCraft versions work.

## Tracks

| Track | What we test against | How it is discovered | When | Effect of a failure |
|---|---|---|---|---|
| `stable` | Latest published, non-pre-release VectorCraft release | SemVer tags from `git ls-remote` | Daily, and on every PR touching the plug-in, the adapter or the gate | **Blocks** our release; its issue is labelled `upstream-stable` |
| `prerelease` | Latest published alpha/beta/RC (`-alpha`, `-beta`, `-rc` SemVer tags) | SemVer tags from `git ls-remote`, when newer than the latest release | Daily | Its issue is labelled `upstream-prerelease`; does not block |
| `release-branch` | Head of VectorCraft's `release` branch, which their pipeline builds into the next (still private, draft) release | `git ls-remote … refs/heads/release` | Daily | Its issue is labelled `upstream-canary` |
| `main` | Head of VectorCraft's `main` | `git ls-remote … refs/heads/main` | Daily | Its issue is labelled `upstream-canary` |

Each track has one issue at a time, commented on while the track fails and closed when it passes again.

Draft releases are not visible to the public API, so the `release` branch is the earliest public signal
of an upcoming release (VectorCraft's release workflow runs on pushes to `release`,
`.github/workflows/release.yml`); pre-release tags catch alphas, betas and release candidates as soon as
they are published.

`compat/vectorcraft.toml` records the versions currently pinned per track (the branch for branch tracks,
and the SHA-256 of the stable release's Linux archive), so that pull requests are checked against a fixed
release and every change of pin is a reviewable PR.

## Two levels of test

### Level A — ABI contract (fast, minutes)

`compat/vc-contract` is a small crate **outside the workspace** that depends on VectorCraft's
`vectorcraft-plugins` crate at the track's ref (a git dependency rewritten per track by
`cargo xtask compat`). It loads our freshly built `.wasm` files into VectorCraft's real host code and
checks:

1. Each module installs (`vectorcraft_plugins::registry::install_bytes`), and its manifest is accepted
   (id, kind, parameter count and types).
2. `vc_abi_version` is a version the host supports.
3. Live effects run through `vectorcraft_plugins::effect::apply` on fixture geometry, finish within the
   host's limits, and return the expected preview geometry: paths of stitches for strokes and satin
   columns and closed ribbons for fills. When an effect can't show stitches, its last error says why.
   Which level a shape shows, and the geometry of each, the plug-in's native tests check.
4. Filters run and return valid objects. Exporters run through `vectorcraft_plugins::export::run` on
   documents written for the test, return a file of their format, with the export's warnings, plan for
   the machine their dialog names, and decline with the first error when there is one (`REQ-VC-011`).
5. Error paths: invalid parameters are rejected by the host with the expected message.
6. Budget: the *hello* plug-in's cost for each byte of JSON it reads and writes stays within a quarter
   above the costs [recorded for previews](vectorcraft-integration.md#live-effect-previews-within-v1-budgets).
   Every preview level finishes on shapes of growing size, within VectorCraft's limits (`REQ-VC-003`), and
   an export of a fill of the reference hoop's whole field finishes within an export's (`REQ-VC-011`).
   Each run prints the costs.

`vectorcraft-plugins` depends only on `geom`, `color`, `doc` and `wasmi`, so this builds quickly.

### Level B — end to end (slower, tens of minutes)

Uses VectorCraft's own command line, `vectorcraft-cli run`. For `stable` and `prerelease` it is the
binary in the release's Linux archive (VectorCraft's `packaging/linux/package.sh` puts it there). The
archive is checked against the SHA-256 the pins file records for the pinned release, or against the one
the release's `SHA256SUMS.txt` lists for a newer release. For the branch tracks it is built from the
branch, with a cached target directory. `cargo xtask compat track <track> --ref <tag|sha>` runs both
levels and writes the track's result for the report, and `cargo xtask compat e2e` runs Level B alone.

The scenarios are data files in `compat/scenarios/*.toml`, each one or more runs of `vectorcraft-cli`. A
run starts from a new document, or from one an earlier run saved, and draws its shapes with VectorCraft's
commands. So every file VectorCraft opens is one it wrote, and StitchCraft reads only what the version
under test saved. Steps work on the selection, which drawing a shape sets: VectorCraft 0.7.0 can't give
one command another's result. VectorCraft runs a live effect once for an object and its parameters, and
remembers only a module's last failure, which the next success clears. So each module a run installs is
asked for its last error after every PNG export, which runs the live effects, and it must have none. A
scenario draws each object it gives an effect before it gives another the same family's, and
`cargo xtask compat` refuses an `apply` step no drawing shows. Exporting a PES or a DST file draws
nothing: an exporter is given the document's JSON. The checks are StitchCraft's, since VectorCraft's
pictures change from version to version and what sews must not:

| Scenario | Steps | Checks |
|---|---|---|
| `install-and-list` | install the 5 modules, then `plugin.list` | each family's live effect and each format's exporter is listed (`REQ-VC-002`, `REQ-VC-011`) |
| `apply` | a square filled red and a line, given the fill and stroke effects with parameters, then saved | as sent, with a fill in red and a stroke (`REQ-VC-004`, `REQ-PRM-003`) |
| `compound-satin` | 2 rails and a rung, Make Compound Path, then the satin effect | as sent, and one satin column (`REQ-VC-007`) |
| `graphic-style` | a group of 2 squares, and a gold square with a fill effect, drawn, whose new graphic style is then applied to the group | as sent, and 3 fills in gold at the effect's 90° (`REQ-VC-006`) |
| `missing-plugin` | a fill saved and drawn, then opened and saved again without the plug-ins | both sew alike, as sent: the records are kept (`REQ-PRM-003`) |
| `export` | a square filled red and a line far below it, given the fill and stroke effects, saved, then exported to PES and DST by StitchCraft's exporters | each file is the one `stitch export` writes of the saved document, and the export's warnings are what it says, the DST's cut on the long jump among them (`REQ-VC-011`) |

*As sent*: the saved file sews as it does with the StitchCraft records of the objects the effects were
applied to put back as the steps sent them. So VectorCraft stored what the dialog set, on the objects it
was applied to, in a form StitchCraft reads. An object `effect.apply` named that the file doesn't hold
fails the check. A file is sewn as `stitch plan` sews it, for the reference machine, and 2 are compared
byte for byte, as is a machine file exported with the one `stitch export` writes of its document, by the
format and the name of the file. Elements are counted by kind, thread and settings. A paint's fill is a fill too, and has no
settings. The live-effect budget of each preview level is Level A's (`REQ-VC-003`).

## Automation

```text
schedule (daily) ──▶ discover tracks ──▶ for each track: build plug-ins ─▶ Level A ─▶ Level B ─▶ its result
                                                                                                  │
              ┌───────────────────────────────────────────────────────────────────────────────────┘
              ▼
   results ──▶ `cargo xtask compat report` ──▶ an issue per track: opened, commented on, or closed
              │
              └─ a new stable or pre-release passes ─▶ bot PR "compat: VectorCraft vX.Y.Z":
                                                       pins, compat/supported.toml and the generated page
```

- `cargo xtask compat report` decides, and the workflow acts on what it prints. The rules are Rust with
  tests. Filing an issue is too (`cargo xtask compat issue`), which comments on an open issue with the
  same title rather than open another.
- Each track's issue is titled "VectorCraft compatibility: <track> fails". It lists what failed at each
  level, with links to the run and to VectorCraft's changes since the last release that passed. The issue
  is closed when the track passes again. A track fails when its job ends without writing a result.
  VectorCraft always has a release and a `main` branch, so a run that discovers no version of either
  counts that track as failing. When discovery fails, the other tracks' issues are left as they are. The
  move rehearsal has an issue of its own, which a run without the rehearsal leaves alone.
- A new stable release or pre-release that passes both levels is adopted. The report writes its pins (for
  a stable release, the archive's SHA-256 the run checked, and the contract crate's tag) and its row in
  `compat/supported.toml`, and `cargo xtask docs` regenerates the page. The bot pushes the branch
  `compat/vectorcraft-<tag>` and opens the pull request. It then dispatches ci, docs, goldens and compat
  on the branch itself, because GitHub runs workflows for a push made with the workflow's own token only
  when they are dispatched. A new VectorCraft version is adopted only through a reviewed pull request
  with the full CI green.
- GitHub Actions can open a pull request only if the repository's settings allow it (Settings › Actions ›
  General › Workflow permissions). Otherwise the run opens an issue, or comments on it, titled
  `VectorCraft compatibility: a new release waits for its pull request`, which names the branch. The first
  run with nothing to adopt closes it.
- The report job alone writes: `contents`, `pull-requests`, `issues` and `actions` with the default
  `GITHUB_TOKEN`. It runs on schedule, or when the workflow is started by hand on `main`, and never for a
  cancelled run, whose tracks may have stopped before writing their results.

## Moving into VectorCraft

StitchCraft must stay movable into VectorCraft's own repository ([ADR-0011](adr/0011-movable-into-vectorcraft.md)).
The `move` job rehearses it against the latest stable release and `main` (pull requests that touch
manifests or tooling: the pinned stable release):

1. Check out VectorCraft and put this repository into it as the folder `stitchcraft/`.
2. **Nested:** `cargo xtask compat join --nested ..`, then build a throwaway VectorCraft crate that depends
   on `stitchcraft-engine` by path.
3. **Joined:** `cargo xtask compat join ..`, then run `cargo xtask ci` from the folder: StitchCraft's
   gates, inside VectorCraft's workspace, with its lockfile and lints.

A failure means a change here, or one in VectorCraft, has made the move harder. The scheduled run reports
it in an issue of its own, labelled `upstream-canary`.

## VectorCraft's window

The `window` job runs the app VectorCraft releases, from the Linux archive of the pinned stable release,
in a desktop without a screen. `cargo xtask compat window` takes screenshots of the window and drives it
through its accessibility tree with xa11y, as the docs' `vectorcraft-ui` pictures will be made
([docs pipeline](docs-pipeline.md#vectorcrafts-window-in-ci)). The archive's SHA-256 is pinned beside the
stable tag in `compat/vectorcraft.toml`, so the bot PR that moves the pin records the new archive's sum
too, the one the release's `SHA256SUMS.txt` lists and the run checked. The same archive holds
`vectorcraft-cli`, for Level B.

## The supported-versions table

The user guide's [VectorCraft compatibility](../user/reference/compatibility.md) page is generated by
`cargo xtask docs` from `compat/supported.toml`, never hand-edited. The record lists the releases that
passed both levels, with the day each passed. Only the pull request that adopts a release writes to it. A
branch's result changes daily and is reported in its issue, never in the record. A row of the page looks
like this:

| VectorCraft | Track | Passed the gate |
|---|---|---|
| 0.12.0 | stable | 2026-10-18 |

It also names the release the gate pins, which pull requests are checked against.

## Going upstream

The strongest gate is in VectorCraft itself: we will offer VectorCraft a tiny third-party contract test
(our minimal plug-in built as a fixture, like their `desaturate.wasm`) so `cargo xtask ci` catches ABI
changes before release. VectorCraft runs its gates locally rather than on every PR in GitHub Actions
(observed in `.github/workflows/` at `89ea46c`), so a test in their suite is worth more than a test in
ours.
