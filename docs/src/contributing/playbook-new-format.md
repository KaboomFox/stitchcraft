# Playbook: add a machine format

1. **Collect the facts** from public specifications or MIT/Apache-licensed implementations, and record
   the sources at the top of the module and, if data is derived, in `NOTICE`.
2. **Write the format page section** in `docs/src/design/formats.md`: structure, units, axis, command
   encoding, per-record limits, colours, gotchas.
3. **Requirements and cases:** golden files for canonical plans, round trip, commands, empty files,
   limits (`REQ-FMT-*` per format).
4. **Module:** `crates/stitchcraft-formats/src/<format>/` with `write.rs`, `read.rs`, `consts.rs`,
   `tests.rs`. Readers: check every length and offset before use, cap counts, return typed errors;
   `indexing_slicing` is denied here.
5. **Fuzz target** in `fuzz/fuzz_targets/read_<format>.rs`; run it for 10 minutes locally.
6. **Oracle:** add the format to the pyembroidery oracle job if pyembroidery supports it.
7. **Profiles:** if a machine uses it, add or update a profile — and schedule a machine test before
   calling it supported.
