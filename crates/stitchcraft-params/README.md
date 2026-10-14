# stitchcraft-params

Layer **L0** (may use `stitchcraft-core`). The parameter registry's building blocks: every embroidery
parameter is declared once, next to the code that uses it, with the `params!` macro, and typed structs,
validation, the reference docs and the JSON Schema are generated from that declaration — later also
VectorCraft manifests, CLI help, SVG attribute mapping and property-test strategies.

Design: `docs/src/design/params.md`, `docs/src/design/adr/0003-parameter-registry.md`.

## What is here

- `params!` (`macros.rs`): the declaration; the field name is the key, the doc comment the help text.
- `ParamSpec`, `Kind`, `ParamGroup` (`spec.rs`): what the registry knows about a parameter.
- `ParamSet` (`set.rs`) holds an element's parameters as its design stores them, keys and text. Each
  declaration's `from_set` reads them into a typed view, and `from_set_as` reads them as an element of one
  stitch type does, leaving out the parameters that stitch type does not use.
- `Value` and `Kind::parse` (`value.rs`): the one place text becomes a value, for every host.
- `audit` (`audit.rs`): what REQ-PRM-001 asks of every declaration.
- `StitchType` (`stitch_type.rs`): the stitch types, by their Ink/Stitch method ids.

The registry itself — the list of every declaration — is `stitchcraft_engine::registry::PARAMETERS`.

## Invariants

- Registry keys equal Ink/Stitch attribute names where the meaning matches, and defaults equal Ink/Stitch's
  (the interoperability contract in `conformance/inkstitch-params.toml`, cross-checked by
  `cargo xtask docs --check`); a deliberate difference names its entry in `conformance/deviations.toml`.
- Invalid values never fall back silently to defaults. Each produces `SC-E0101` or `SC-W0102`, and an
  unknown key `SC-W0105`, naming the key, the value and what is accepted. Empty or blank text is not
  set, and takes the default, as in Ink/Stitch.
- A value is stripped of the white space round it before it is read, whatever its kind, as Ink/Stitch
  strips it, and a value longer than 4,096 bytes does not read, so reading one costs no more than that.
- Spaces separate the values of a list or a pair, and a comma separates nothing. A unit follows its number
  with no space.
- A toggle reads Ink/Stitch's words for on (`yes`, `y`, `true`, `t`, `1`), their opposites and `off`.
  Other text does not read (`toggle`, which the SVG reader shares).
- Knows no stitch algorithm.

## Dependencies

`stitchcraft-core` only (diagnostics, `Mm`, unit constants).
