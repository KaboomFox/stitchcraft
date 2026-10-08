# Machine formats

`stitchcraft-formats` encodes a `StitchPlan` into machine files and decodes machine files into plans.
PES (Brother) and DST (Tajima) come first because the first target machine is a Brother and DST is the
lingua franca every machine and digitizer reads.

## Principles

- **Quantize once.** Absolute positions are rounded to 0.1 mm (round half to even) at the start of
  encoding; deltas are differences of quantized positions, so error never accumulates
  ([determinism](determinism.md)).
- **Format limits live in the encoder.** Each writer knows its maximum displacement per record and
  splits long moves into several jump records; the engine never needs to know.
- **Commands are explicit.** Each writer documents how it expresses `Jump`, `Trim`, `Stop`,
  `ColorChange` and `End`, and conformance checks every one of them per format (`REQ-FMT-003`).
- **Readers are hostile-input parsers.** Every length, count and offset is checked before use; total
  stitches are capped (default 2,000,000); allocation is proportional to verified sizes; the reader
  returns the plan or a typed error, never a panic (fuzzed, `NFR-SEC-1`).
- **Never write an empty file** (`SC-E0010`), and accept empty or zero-stitch files when reading
  (`REQ-FMT-004`).
- **Facts are cited.** Format knowledge comes from public descriptions and MIT-licensed pyembroidery /
  pystitch as a reference implementation; where we rely on them, the source is noted in the module and
  in `NOTICE`.

## PES and PEC

PES is Brother's design format. A PES file is a PES header (and, for newer versions, design-editor
data) followed by a **PEC block**, which is what the machine actually sews from.

### What we write (v1)

| Part | Content |
|---|---|
| Signature | `#PES0001` |
| PEC offset | 32-bit little-endian offset of the PEC block |
| PES section | Minimal v1 design section (hoop indicator, a single block object with the stitch extents) |
| PEC header | `LA:` + label (padded) + carriage return; colour count − 1; one palette index per colour block (PEC palette of Brother threads); padding |
| PEC graphics header | Stitch-data length, design width/height, thumbnail size |
| PEC stitches | Relative moves in 0.1 mm (see below) |
| Thumbnails | 48 × 38 one-bit images: the whole design, then one per colour |

**Stitch encoding.** A move is encoded per axis: a short form (one byte, 7-bit two's complement) for
small displacements, and a long form (two bytes, 12-bit two's complement with flags) for larger ones up
to ±2047 units (204.7 mm). Long-form flags mark the move as a **jump (`0x10`)** or a **trim (`0x20`)**.
A colour change is `0xFE 0xB0` followed by a byte that alternates between `0x02` and `0x01`; the end is
`0xFF`. PEC has no separate trim or stop command: a trim is a trim-flagged jump, and a stop is encoded as a
colour change to the same thread.

**Colours.** The PEC header stores palette *indices*, not RGB, so every thread is mapped to the
nearest Brother PEC colour (CIEDE2000). Catalogue numbers from other brands cannot survive PES v1; a
PES v6 writer (below) can carry a thread list with RGB and codes.

### Risks found in the field, and how we test them

From the [issues review](inkstitch-issues-review.md#l2--machines-disagree-about-files):

1. **Trims ignored** on some Brother machines (Ink/Stitch #689). Test sheet TS-02 sews both encodings:
   trim-flagged jumps, and a long-jump-only variant for machines that trim on jump length. The profile
   records which one the machine honours.
2. **Large designs hidden** by an older Brother-family machine for PES v1 files beyond about 130 ×
   180 mm (#1853, root cause never found). The PES v1 section carries a hoop indication, and
   Ink/Stitch's writer (pystitch) sets it to the 130 × 180 mm class — our unconfirmed hypothesis for
   that report. For our 200 × 200 mm hoop, MC-1 sews TS-10 at **150 mm and 190 mm** widths from a PES v1
   file. If the machine refuses either, M1 adds a PES v6 writer with explicit hoop dimensions and repeats
   the test.
3. **Colour expectations** (#2668): documented behaviour — what you see on the machine is the PEC
   palette colour; the thread chart in the report lists the source colour and the PEC match.

### Later versions

A PES v6 writer (`#PES0060`) is planned behind the profile's `format` field, only if a machine
checkpoint needs it (hoop dimensions, thread lists). Its header contains fields whose meaning is only
partly known publicly; every field we write is either understood or copied from the v1 values that
machines accept, and covered by a golden file.

## DST (Tajima)

| Part | Content |
|---|---|
| Header | 512 bytes of ASCII fields — `LA:` label, `ST:` stitch count, `CO:` colour changes, `+X`, `-X`, `+Y`, `-Y` extents, `AX`, `AY`, `MX`, `MY`, `PD` — padded with spaces and terminated by `0x1A` |
| Records | 3 bytes each; x and y displacements in balanced-ternary bit patterns up to ±121 units (12.1 mm) per record |
| Axis | y is up: positive y in the plan (down) is written negative |

The third byte always has bits 0 and 1 set; bit 7 marks a jump (`0x83` with no other bits); `0xC3`
is a colour change (and, by common convention, a stop); `0xF3` with zero displacement ends the design.
DST has no trim command: trims are expressed as a short sequence of jumps that the machine interprets
as a trim; the sequence length is a profile setting validated on a machine.

Moves longer than 12.1 mm are split into several jump records by the encoder.

## Later formats

| Format | Machines | Milestone | Notes |
|---|---|---|---|
| EXP | Melco, Bernina | M11 | 2-byte records, `0x80` escape codes for commands |
| JEF | Janome | M11 | Janome thread table indices; per-palette mapping (Ink/Stitch #3137) |
| VP3 | Husqvarna Viking, Pfaff | M11 | Nested length-prefixed sections; thread descriptions as strings |
| XXX | Singer | M11 | |
| U01 | Barudan | M11 | |
| CSV / JSON | Debugging and interchange | M2 | Human-readable plan dumps; the conformance suite's golden format for stitch lists |
| HUS, VIP | Husqvarna (older) | — | Compressed; read-only candidates after 1.0 |

## Conformance

| Requirement | What it checks |
|---|---|
| `REQ-FMT-001` | Golden bytes: canonical plans encode to committed files byte for byte |
| `REQ-FMT-002` | Round trip: `decode(encode(plan)) == quantize(plan)` for every writer/reader pair, on random plans |
| `REQ-FMT-003` | Every command kind survives every writer/reader pair (and is sewn in TS-02) |
| `REQ-FMT-004` | Empty and zero-stitch files read cleanly; empty plans are never written |
| `REQ-FMT-005` | Independent oracle: pyembroidery reads our files to the same stitches (CI job with a pinned version) |
| `REQ-FMT-006` | Fuzzed readers never panic, never allocate beyond caps, always terminate |
| `REQ-FMT-007` | Long moves are split within the format's per-record limit |

The pyembroidery oracle runs in a separate CI job with a pinned version and only compares decoded
stitch lists; no pyembroidery code is vendored.
