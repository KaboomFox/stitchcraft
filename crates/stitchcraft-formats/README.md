# stitchcraft-formats

Layer **L2**. Encodes stitch plans into machine files and decodes machine files into plans. PES (v1 header +
PEC block) for Brother and DST for everything else come first.

**Status:** writers in M1, readers in M2. Design: `docs/src/design/formats.md`.

## Invariants

- Quantize absolute positions once (round half to even, 0.1 mm); deltas never accumulate error.
- Each writer splits moves beyond its per-record limit and documents how every command is encoded.
- Readers treat every byte as hostile: lengths and offsets checked, counts capped, typed errors, fuzzed;
  `indexing_slicing` is denied in this crate.
- Never write an empty plan; read empty files cleanly.
- Format facts cite their sources (public specifications, MIT-licensed pyembroidery) here and in `NOTICE`.
