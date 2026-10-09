# stitchcraft-formats

Layer **L2**. Encodes stitch plans into machine files and decodes machine files into plans. PES (v1 header +
PEC block) for Brother and DST for everything else come first.

**Status:** writers since M1, readers since M2. Design: `docs/src/design/formats.md`.

| Module | Purpose |
|---|---|
| `lower` (private) | What every writer needs once: quantize, moves, colour changes, end, command checks; `split` for per-record limits |
| `quantize` | `quantize` (mm → 0.1 mm, half to even), `Units`, `Delta` |
| `pes` | PES v1 container; `pes::pec` the PEC block (header, stitch data); `pes::thumbnail` the 48 × 38 pictures |
| `dst` | Tajima DST: header, balanced-ternary records, trims as jump sequences |
| `decode` | `decode` (recognises PES, PEC, DST), `Decoded`, and the `Recorder` every reader builds plans with (limits included) |
| `pes::read`, `dst::read` | The readers: literal, bounded, typed errors |
| `label` | Design names as machines show them (16 safe ASCII characters) |
| `error` | `EncodeError`, each mapped to a registered diagnostic code |

## Invariants

- Quantize absolute positions once (round half to even, 0.1 mm); deltas never accumulate error.
- Each writer splits moves beyond its per-record limit evenly and documents how every command is encoded
  (byte tables in the module docs, spec vectors in their tests).
- Never write an empty plan; refuse what does not fit a format's fields instead of truncating it.
- Canonical plans (`stitchcraft-testkit::plans`) encode to the golden files in
  `conformance/golden/formats/` byte for byte (`tests/golden.rs`; bless with `STITCHCRAFT_BLESS=1`).
- Readers treat every byte as hostile: lengths and offsets checked, records capped at the stitch budget,
  typed errors (`DecodeError`, `SC-E0603`); every truncation and byte flip of the golden files is tested;
  `indexing_slicing` is denied in this crate.
- Format facts cite their sources (public descriptions; pystitch output observed as a black box) in the
  module docs and in `NOTICE`.

## Dependencies

`stitchcraft-core`, `stitchcraft-plan`; `thiserror`. Dev: `stitchcraft-testkit`.
