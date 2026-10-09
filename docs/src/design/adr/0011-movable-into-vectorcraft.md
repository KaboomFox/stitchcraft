# ADR-0011: StitchCraft can move into VectorCraft's repository

**Status:** Accepted · 2026-10-08

## Context

If VectorCraft's maintainers want machine embroidery in VectorCraft itself, they should be able to take
this repository into theirs without a rewrite. Much already lines up: the same licence
(MIT OR Apache-2.0), edition (2024), minimum Rust version (1.95), formatting and lint style
([ADR-0009](0009-adopt-vectorcraft-conventions.md)).

We tried it on 2026-10-08 with VectorCraft `main` at `a469568c`: a copy of this repository as the folder
`stitchcraft/`, and a VectorCraft crate depending on `stitchcraft-engine` by path. It failed in three
ways:

1. Cargo gave our crates VectorCraft's workspace settings, because the path dependency sits inside
   VectorCraft's workspace directory. Our crates inherited `publish`, which VectorCraft's workspace does
   not define, so Cargo refused to load them. Once VectorCraft's manifest excluded the folder
   (`exclude = ["stitchcraft"]`), the path dependency resolved against our own workspace and built.
2. Both repositories call their tooling package `xtask`, so the two cannot share one workspace.
3. Our tooling ran `cargo … --workspace` and read every package in `cargo metadata`. Inside a shared
   workspace it would build, lint and test VectorCraft's crates too, and the layering check would call
   them unknown.

## Decision

The repository stays movable **as one folder**, in either of two ways. Both are one command, and CI
rehearses both against VectorCraft's latest release and `main`.

**Nested.** Copy the folder in, with history if wanted
(`git subtree add --prefix=stitchcraft https://github.com/KaboomFox/stitchcraft main`), then run
`cargo xtask compat join --nested ..` from it. That adds the folder to VectorCraft's `exclude` list.
StitchCraft keeps its own workspace, lockfile, toolchain pin and gates (`cargo xtask ci` inside the
folder), and VectorCraft's crates may depend on StitchCraft's by path.

**Joined.** After the copy, run `cargo xtask compat join ..` instead (or after a nested join). It edits
VectorCraft's root `Cargo.toml` in place, keeping its comments and layout:
- our crates, apps and tooling become members;
- our `[workspace.dependencies]` are merged into VectorCraft's: VectorCraft's own entry is reused where
  it satisfies ours, and any features we need are added;
- our root `Cargo.toml` and `Cargo.lock` are removed, leaving one workspace and one lockfile.

It refuses, writing nothing, when the two cannot merge: a dependency in an incompatible version, a
setting our crates inherit that VectorCraft does not define, or a name clash. It prints the rows
VectorCraft's layering table needs. The tooling keeps working from the folder: `cargo xtask ci` there
builds, lints and tests StitchCraft's packages, with VectorCraft's lockfile and lints.

These rules keep it that way:

- **Every package is named `stitchcraft-*`**, the tooling included (`stitchcraft-xtask`, still run as
  `cargo xtask`). The layering table holds only prefixed names, and an unlisted package fails
  `cargo xtask layers`.
- **Crate manifests inherit only what VectorCraft's workspace defines**: `version`, `edition`, `license`,
  `rust-version`, `repository`, and lints. Each crate sets `publish = false` itself.
- **Tooling is relative to its own folder.** Files are found from the folder that holds `xtask/`, never
  from the Git root or the current directory. Cargo commands name StitchCraft's packages instead of
  `--workspace` (`util::packages`). Inside a larger workspace, `cargo-deny` is skipped, because it judges
  a whole workspace and VectorCraft's answers to VectorCraft's policy.
- **Configuration that should follow the code lives in the folder.** Clippy and rustfmt read the
  configuration file nearest each crate, so our determinism rules (`clippy.toml`) keep applying to our
  crates, and VectorCraft's to theirs.
- **Generated files never depend on where the folder is**: no absolute paths, no repository-relative
  paths outside it. `cargo xtask docs --check` and `shots --check` run in the rehearsal.

## What stays with VectorCraft's maintainers

- **Workflows.** GitHub reads them only at the repository root. Ours call `cargo xtask …`, so moving
  them is a matter of `working-directory: stitchcraft`.
- **Their own checks.** In the joined form, their layering table needs rows for our crates (printed by
  `compat join`), and their repository-wide checks will see our files.
- **The plug-in shim.** `apps/stitchcraft-vc-plugin` talks to VectorCraft through ABI v1. Inside
  VectorCraft, the engine can be called directly instead; that is their integration choice.

## Consequences

- Taking StitchCraft into VectorCraft is a folder copy plus one command, and the `move` job in
  `compat.yml` proves it daily, so a change that would break it fails here first.
- Small constraints for us: no `publish.workspace`, no `--workspace` in tooling, and a prefixed name
  for every package.

## Alternatives considered

- **Publish the crates to crates.io and let VectorCraft depend on them.** Possible later. It doesn't
  bring the docs, tests and gates along, and it isn't what "moving into their codebase" means.
- **Relative `path` dependencies instead of workspace dependencies.** That would make crates movable
  without merging tables, but it departs from VectorCraft's convention and repeats every version in
  every crate. `compat join` merges the tables instead.
