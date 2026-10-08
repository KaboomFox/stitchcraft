# Playbook: add a stitch type

A stitch type touches several registries, but each touch is small. Follow the steps in order; the
gates catch anything you miss.

1. **Design page first.** Add or extend the section in `docs/src/design/algorithms/` with: purpose,
   parameters (registry keys; Ink/Stitch names where they exist), the algorithm in your own words,
   properties, diagnostics, references. Clean room: public sources only.
2. **Requirements.** Add `REQ-…` entries to `conformance/requirements.toml` with `status = "planned"` and
   the milestone.
3. **Cases.** Add cases under `conformance/cases/<area>/`: typical shapes, the degenerate corpus entries
   that matter, and one case per diagnostic. They fail for now — that is the point.
4. **Module.** Create `crates/stitchcraft-engine/src/generators/<name>/` with `mod.rs` (the
   `Generator` impl), `params.rs` (the `params!` block, doc comments as help text) and `tests.rs`.
   Keep files under 800 lines; split by concern (sampling, routing, compensation).
5. **Register** the generator in the generator table and its params in the registry list (one line
   each).
6. **Make the cases pass.** Run `cargo xtask conformance --filter <area>`; switch the requirements to
   `active`.
7. **Docs.** `cargo xtask docs` regenerates the parameter pages; write the user-facing page in
   `docs/src/user/` (what it is for, when to use it, two or three declared shots); run `cargo xtask shots`.
8. **Gate.** `cargo xtask ci`, then open the PR with the roadmap step id.
9. **Sew-out.** If the type is new to the machine, list it for the next machine checkpoint (TS-11).
