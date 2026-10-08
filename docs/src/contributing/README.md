# Contributing

Welcome. StitchCraft is built to be easy to change safely: the rules are checked by machines, common
changes follow playbooks, and every behaviour has a requirement and a test.

## Start here

1. Read `AGENTS.md` at the repository root (the short contract for humans and agents) and the
   [architecture](../design/architecture.md) map.
2. Pick a step from the [roadmap](../plan/roadmap.md) or an issue labelled `good first issue`.
3. Set up: install Rust (the toolchain in `rust-toolchain.toml` installs itself), then
   `rustup target add wasm32-unknown-unknown`.
4. Before every push: `cargo xtask ci`.

## Playbooks

| Change | Playbook |
|---|---|
| Add a stitch type | [New stitch type](playbook-new-stitch-type.md) |
| Add or change a parameter | [New parameter](playbook-new-param.md) |
| Add a machine format | [New format](playbook-new-format.md) |
| Add a diagnostic | [New diagnostic](playbook-new-diagnostic.md) |
| Review a PR | [Review checklist](review-checklist.md) |

## Ground rules

- **Clean room:** never read or copy Ink/Stitch source code (GPL-3.0). Work from the design docs,
  public documentation and papers ([ADR-0001](../design/adr/0001-license-and-clean-room.md)).
- **No panics, no `unsafe`, deterministic output** — the lints will tell you
  ([guardrails](../design/guardrails.md)).
- **One roadmap step per PR**, its id in the title; requirements and cases first.
- **Docs in the same PR** as the behaviour change; never hand-write a parameter's reference.
- **Be kind.** Reviews are about code, not people.
