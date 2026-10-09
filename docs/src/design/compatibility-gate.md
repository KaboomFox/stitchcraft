# VectorCraft compatibility gate

**Goal:** know within a day when a VectorCraft change would break the StitchCraft plug-in — ideally
before VectorCraft ships it — and keep a public, generated table of which VectorCraft versions work.

## Tracks

| Track | What we test against | How it is discovered | When | Effect of a failure |
|---|---|---|---|---|
| `stable` | Latest published, non-pre-release VectorCraft release | GitHub Releases API (`prerelease: false`) | Daily, and on every PR touching the plug-in or adapters | **Blocks** our release; opens an issue |
| `prerelease` | Latest published alpha/beta/RC (`-alpha`, `-beta`, `-rc` SemVer tags) | GitHub Releases API (`prerelease: true`) | Daily | Opens an issue labelled `upstream-prerelease`; does not block |
| `release-branch` | Head of VectorCraft's `release` branch, which their pipeline builds into the next (still private, draft) release | `git ls-remote … refs/heads/release` | Daily | Opens an issue labelled `upstream-canary` |
| `main` | Head of VectorCraft's `main` | `git ls-remote … refs/heads/main` | Nightly | Opens or updates one rolling issue labelled `upstream-canary` |

Draft releases are not visible to the public API, so the `release` branch is the earliest public signal
of an upcoming release (VectorCraft's release workflow runs on pushes to `release`,
`.github/workflows/release.yml`); pre-release tags catch alphas, betas and release candidates as soon as
they are published.

`compat/vectorcraft.toml` records the versions currently pinned per track (and the commit SHA for branch
tracks) so every run is reproducible and every change of pin is a reviewable PR.

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
   host's limits, and return the expected preview geometry (golden JSON, tolerance 1 µm).
4. Filters run and return valid objects.
5. Error paths: invalid parameters are rejected by the host with the expected message.

`vectorcraft-plugins` depends only on `geom`, `color`, `doc` and `wasmi`, so this builds quickly.

### Level B — end to end (slower, tens of minutes)

Uses the real `vectorcraft-cli`: the binary from the release's Linux package for `stable` and
`prerelease` (the Linux packages ship `vectorcraft-cli`, `packaging/linux/package.sh`), or built from source
with a cached target directory for branch tracks. Scenarios are data files in `compat/scenarios/*.toml`:

| Scenario | Steps | Checks |
|---|---|---|
| `install-and-list` | `--cmd plugin.install`, `--cmd plugin.list` | our plug-ins listed with the right kinds |
| `apply-running` / `-satin` / `-fill` | open fixture, `effect.apply {"effect": "plugin.<ns>.<x>", "ids": […], "params": {…}}`, `--export out.png` | PNG matches golden within tolerance; no `lastError` |
| `save-reopen` | apply, `app.save`, reopen, `effect.list` | parameters identical (`REQ-PRM-003`) |
| `export-pes` | VectorCraft saves `.vectorcraft`; `stitch export` reads it | our CLI reads the version's files; PES golden matches |
| `missing-plugin` | open a document with our effect records without the plug-in installed | document opens; records preserved on save |
| `limits` | largest fixture at each preview level | completes inside the live-effect budget |

## Automation

```text
schedule (daily) ──▶ discover tracks ──▶ for each track: build plug-ins ─▶ Level A ─▶ Level B
                                                                              │
              ┌───────────────────────────────────────────────────────────────┘
              ▼
   results.json ──▶ `cargo xtask compat report` ──▶ docs/src/user/reference/compatibility.md (generated)
              │
              ├─ new stable/pre-release and green ─▶ bot PR "compat: VectorCraft vX.Y.Z" updating pins + table
              └─ red ─▶ issue with the failing scenario, logs, and the VectorCraft commit range since last green
```

- The bot PR runs the full CI, so a new VectorCraft version is adopted only through a reviewed,
  green PR.
- Failures on branch tracks produce one rolling issue per track, updated instead of duplicated.
- The workflow uses the default `GITHUB_TOKEN` with `contents: write`, `pull-requests: write`,
  `issues: write` only on the scheduled job.

## Moving into VectorCraft

StitchCraft must stay movable into VectorCraft's own repository ([ADR-0011](adr/0011-movable-into-vectorcraft.md)).
The `move` job rehearses it against the latest stable release and `main` (pull requests that touch
manifests or tooling: the pinned stable release):

1. Check out VectorCraft and put this repository into it as the folder `stitchcraft/`.
2. **Nested:** `cargo xtask compat join --nested ..`, then build a throwaway VectorCraft crate that depends
   on `stitchcraft-engine` by path.
3. **Joined:** `cargo xtask compat join ..`, then run `cargo xtask ci` from the folder: StitchCraft's
   gates, inside VectorCraft's workspace, with its lockfile and lints.

A failure means a change here, or one in VectorCraft, has made the move harder; the scheduled run reports
it in the same `upstream-canary` issue as the contract tests.

## The supported-versions table

Generated, never hand-edited:

| VectorCraft | Track | ABI | Contract | End to end | Checked |
|---|---|---|---|---|---|
| 0.6.0 | stable | v1 | ✅ | ✅ | 2026-10-09 |

The plug-in's own docs state the minimum VectorCraft version (the oldest green `stable` row we still test).

## Going upstream

The strongest gate is in VectorCraft itself: we will offer VectorCraft a tiny third-party contract test
(our minimal plug-in built as a fixture, like their `desaturate.wasm`) so `cargo xtask ci` catches ABI
changes before release. VectorCraft runs its gates locally rather than on every PR in GitHub Actions
(observed in `.github/workflows/` at `89ea46c`), so a test in their suite is worth more than a test in
ours.
