# stitchcraft-params

Layer **L0** (may use `stitchcraft-core`). The parameter registry's building blocks: every embroidery
parameter is declared once, next to the code that uses it, with the `params!` macro, and typed structs,
validation, the reference docs and the JSON Schema are generated from that declaration — later also
VectorCraft manifests, CLI help, SVG attribute mapping and property-test strategies.

Design: `docs/src/design/params.md`, `docs/src/design/adr/0003-parameter-registry.md`.

## What is here

- `params!` (`macros.rs`): the declaration; the field name is the key, the doc comment the help text.
- `ParamSpec`, `Kind`, `ParamGroup` (`spec.rs`): what the registry knows about a parameter.
- `ParamSet` (`set.rs`): an element's parameters as its design stores them, keys and text; read into a
  typed view by each declaration's `from_set`.
- `Value` and `Kind::parse` (`value.rs`): the one place text becomes a value, for every host.
- `audit` (`audit.rs`): what REQ-PRM-001 asks of every declaration.
- `StitchType` (`stitch_type.rs`): the stitch types, by their Ink/Stitch method ids.

The registry itself — the list of every declaration — is `stitchcraft_engine::registry::PARAMETERS`.

## Invariants

- Registry keys equal Ink/Stitch attribute names where the meaning matches, and defaults equal Ink/Stitch's
  (the interoperability contract in `conformance/inkstitch-params.toml`, cross-checked by
  `cargo xtask docs --check`); a deliberate difference names its entry in `conformance/deviations.toml`.
- Invalid values never fall back silently to defaults: they produce `SC-E0101` or `SC-W0102`, and unknown
  keys `SC-W0105`, each naming the key, the value and what is accepted.
- Knows no stitch algorithm.

## Dependencies

`stitchcraft-core` only (diagnostics, `Mm`, unit constants).
