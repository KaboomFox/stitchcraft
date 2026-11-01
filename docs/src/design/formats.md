# Machine formats

<!-- implements: crates/stitchcraft-formats/src/**, apps/stitchcraft-cli/src/commands/convert.rs, apps/stitchcraft-cli/src/commands/inspect.rs -->

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
| PEC offset | 32-bit little-endian offset of the PEC block: 22 |
| PES section | A hoop indicator (0 = 100 × 100 mm, 1 = 130 × 180 mm; we write 1 for designs larger than 100 × 100 mm) and no design-editor objects. Machines sew from the PEC block; the section is for PE-Design. |
| PEC header | `LA:` + label (16 characters, padded) + carriage return; thumbnail size; colour entries − 1; one Brother palette index per colour entry (each block, and each stop); padding to 512 bytes |
| PEC graphics header | Bytes to the thumbnails, the design's width and height in 0.1 mm, and the start as seen from the design's top-left corner. That field holds −204.8 to 204.7 mm on each axis, and a design whose corner lies farther from the start is refused (`REQ-FMT-011`, `SC-E0602`) |
| PEC stitches | Relative moves in 0.1 mm (see below) |
| Thumbnails | 48 × 38 one-bit pictures with a rounded frame, of the whole design and then of each colour entry, at one scale so that they line up. No thread is drawn before the needle first goes down. Stitches stay inside the frame, and the byte after the stitch data's end mark is `00` (`REQ-FMT-012`) |

The byte-level layout is documented in the writer modules (`crates/stitchcraft-formats/src/pes/`), next
to the code that writes it, with the spec vectors that test it.

**Stitch encoding.** A move is encoded per axis: a short form (one byte, 7-bit two's complement) for small
displacements, and a long form (two bytes, 12-bit two's complement with flags) for larger ones up to ±2047
units (204.7 mm). Each axis is short or long on its own; we use the short form for −63…62. Long-form flags
mark the move as a **jump (`0x10`)** or a **trim (`0x20`)**. A colour change is `0xFE 0xB0` followed by a
byte that alternates between `0x02` and `0x01`, starting with `0x02`; the end is `0xFF`. PEC has no separate
stop command: a stop is a colour change to the same thread, which readers recognise as a stop.

**Trims ride on the next move.** PEC cannot cut in place, so a trim flags the next jump; a sewn move after a
trim becomes a trim-flagged jump to its target and a zero-length stitch there, so the needle still goes
down where the plan says. A trim right before the end needs no record.

**Jumps are trim-flagged, as pystitch writes them** (REQ-FMT-010). A machine treats a StitchCraft file
as it treats one from Ink/Stitch. Only the stitch data's first record, the move from the origin,
keeps the plain jump flag. StitchCraft first flagged only the jumps that follow a `Trim` in the plan and
wrote the others as plain jumps, so the plan decided. Whether a trim-flagged jump is cut is the machine's
choice. At machine checkpoint MC-1 the reference PE800 cut no jump of either flag and sewed test sheet
TS-02 the same with both, and it cut the thread only at thread changes and stops. Machines that trim
jumps cut those longer than their setting. A jump that goes nowhere has no record, since a record of it
would be trim-flagged.
The pieces of a sewn move longer than 204.7 mm keep the plain flag, because a cut there would cut the
stitch's own thread. No profile allows a stitch that long.

**Checked against a reference.** For the canonical plans, whose threads are palette colours, every PEC
header byte equals pystitch's output for the same stitches, the palette indices and the origin fields
among them. A pinned pyembroidery reads our files to the same needle-downs, cuts and pauses as
StitchCraft's own reader (REQ-FMT-005, run as a pinned CI job from M2.4). Threads of other colours can get
other entries than pystitch gives them (`DEV-THREAD-001`).

**Colours** (`REQ-THREAD-002`). The PEC header stores palette *indices*, not RGB, and a machine knows
threads only by them. Threads written as one entry are one thread to it, shown in one colour, and a change
between them reads as a stop. So the threads of a design get their entries together. A thread read from a
PES file is an entry, by its colour and name, and is written back as it, Brother's applique steps 62 to 64
included. The other threads are given the 61 thread colours, one each while they last, the nearest pairs
first by CIEDE2000. A thread gets its nearest entry unless a thread nearer to it took it. Equal threads
share one. With more different threads than that, the rest share their nearest entries, and `SC-W0606`
says how many. `stitch plan`, `convert` and `inspect` name each thread by the entry the file has.
Catalogue numbers from other brands cannot survive PES v1. A PES v6 writer (below) could store a thread
list with RGB and codes.

### Risks found in the field, and how we test them

Embroiderers report these with files from other software:

1. **Trims ignored** on some Brother machines. Test sheet TS-02 sews jumps with and without a trim
   before them. At MC-1 the PE800 cut none of them: on that machine the operator cuts the jump
   threads. Its profiles' trim support is none, from MC-1's sew-out, and previews draw those threads.
2. **Large designs hidden** by an older Brother-family machine for PES v1 files beyond about 130 ×
   180 mm (root cause unknown). The PES v1 section carries a hoop indication, and pystitch sets it to
   the 130 × 180 mm class, which is our unconfirmed hypothesis for that report. The reference PE800's
   largest field is that class, 130 × 180 mm. MC-1 sews TS-10 at the whole field of each of its hoops from
   a PES v1 file. If one of them does not load, a PES v6 writer with explicit hoop dimensions follows, and
   the test is repeated.
3. **Colour expectations:** documented behaviour — what you see on the machine is the PEC
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

**DST has no trim command.** Machines count jump records in a row, and when there are as many as their
setting, they cut the thread before the jumps — if something was sewn since it was last cut or changed
(`REQ-FMT-008`). The setting is commonly three (it can be as high as five; Brother's PR machines take 1 to
8, matched by PE-DESIGN's "number of jumps for trim"), and three is how pyembroidery reads DST too. So:

- a **trim** is three small jumps that cancel out — (+2, −2), (−4, +4), (+2, −2) units, the sequence
  pystitch writes — so the frame goes nowhere while the machine counts three;
- a **jump longer than 24.2 mm** takes three or more records, so it is a trim too, whether the plan asks
  for one or not. The writer says so: `SC-I0605` counts the places where its machines will cut the
  thread and the plan does not trim. With a tie-off before the jump the cut is usually welcome — there is no jump thread
  to clip; without one (an element whose `ties` are off), the stitching may unravel.

A machine set to another count cuts before other jumps, and does not cut at a three-jump trim if set
higher. A machine profile that writes DST records its count, validated on that machine.

Moves longer than 12.1 mm are split evenly into several records by the encoder: all jumps for a jump, and
jumps then the final stitch for a sewn move (the thread lies the same way: the needle only goes down at the
end). Header extents are measured in DST axes (y up) over every position the frame visits; `ST` counts the
records before the end record.

## Reading

`stitch inspect` and the round-trip tests read machine files back. Readers treat every file as possibly
damaged or hostile: every offset and length is checked before use, the record count is capped at the stitch
budget (2,000,000), positions must stay within ±10 m, and a problem is a typed error (`SC-E0603`) — never a
panic and never a silent guess. Readers are literal: each record becomes one plan entry, so `inspect` shows
what is in the file. Unusual but readable things (a thread index outside the palette, a thumbnail offset
that points nowhere) become warnings in the result. `stitch inspect` gives lengths to a hundredth of a
millimetre and counts a stitch that does not move on its own. A file does not mark its lock stitches, so of
the stitches shorter than a profile's shortest it also counts those 0.2 mm or longer, as a lock stitch may
be. `stitch plan`, `testsheet` and `convert` read the file they wrote back the same way, and give its counts
and size (`REQ-CLI-008`).

| Reader | Reads | What it has to infer |
|---|---|---|
| PES / PEC | the PEC block of any PES version (`#PES0001` … `#PES0060`), and bare `#PEC0001` files; the block must start with `LA:` | A colour change to the same palette entry is a **stop**, as PEC writes stops. StitchCraft's writer gives different threads different entries, so only threads beyond the palette's 61 colours read back as a stop. Whether the stitch data starts with the **origin field** (below). |
| DST | the header's label and every record | Three or more jumps in a row are a **trim** before them where something was sewn since the thread was last cut or changed, as DST machines read them (`REQ-FMT-008`); a trim's own spelling at the start of the run — up to 8 jumps of at most 1 mm that end where they started — moves the frame nowhere and is not kept as jumps. Colours: none — each block gets a placeholder thread, and a stop reads as a colour change. |

**The PEC origin field** (`REQ-FMT-009`). Brother's software, pystitch and StitchCraft write 4 bytes
before the first record, shaped like a long jump on both axes. They hold the offset from the corner of
the box that the header's width and height give to the start of the design. pyembroidery 1.4.32 to
1.5.1 leave the field out, and their first record is often a long jump that looks the same. The reader
takes the 4 bytes as the field only if the design read after them fits that box, and otherwise reads them
as the first record, with a warning that Brother machines expect the field. Files from pyembroidery
1.5.1 itself test both cases (`conformance/oracle/write_pes.py`).

Every reader survives every truncation and single-byte change of the golden files (a deterministic test
that runs on every PR), and an hour of coverage-guided fuzzing every night (`REQ-FMT-006`;
[fuzzing](conformance.md#fuzzing)).

**Round trips.** A file cannot say everything a plan says (a trim is a flag in PEC and three jumps in DST;
long moves become several records), so a plan read back is not the same plan — it makes the machine do
the same thing. `stitchcraft-testkit::equivalence` defines that: where the needle goes down, where the
thread is cut, where the machine pauses. Every writer/reader pair keeps it on 256 random plans per PR (fixed
seed) and 10,000 more each night (fresh seed). DST is held to what its machines do with the plan
(`equivalence::dst_events`): they cut the thread before long jumps the plan does not trim. Fresh seeds
paid off before this was even merged: they found a DST design whose split jump pieces cancelled exactly,
which is why only jumps of at most 1 mm make up a trim's spelling.
PES is held to what its file says (`equivalence::pes_events`): a trim flag on every jump.

**Reading back.** `read_back` reads a file just written, with its plan's threads, as a machine reads it.
The command line describes and draws what it wrote from it (`REQ-CLI-008`). The command line and the
VectorCraft exporters also call it before they hand a file out. A file StitchCraft wrote that doesn't read
back is a bug in it (`SC-E0009`), and then nothing is written.

**Converting.** `stitch convert` is a read followed by a write, so the converted file makes the machine do
the same thing as the original: the round-trip guarantee above — except that a DST file cuts the thread
before long jumps where a PES machine leaves a jump thread, which `SC-I0605` reports. A PES file from
StitchCraft has a trim flag on every jump already, so only PES files with plain jumps get those cuts.
Records carry over
literally (a DST file's 12 mm jump pieces stay separate jumps in a PES file), and what the new format
cannot say is reported instead of guessed. A DST file's threads are placeholders, all black (`SC-W0604`).
A PES file made from one names each by the Brother colour nearest black that is still free.

## Later formats

| Format | Machines | Milestone | Notes |
|---|---|---|---|
| EXP | Melco, Bernina | M11 | 2-byte records, `0x80` escape codes for commands |
| JEF | Janome | M11 | Janome thread table indices; per-palette mapping |
| VP3 | Husqvarna Viking, Pfaff | M11 | Nested length-prefixed sections; thread descriptions as strings |
| XXX | Singer | M11 | |
| U01 | Barudan | M11 | |
| CSV / JSON | Debugging and interchange | M2 | Human-readable plan dumps; the conformance suite's golden format for stitch lists |
| HUS, VIP | Husqvarna (older) | — | Compressed; read-only candidates after 1.0 |

## Conformance

| Requirement | What it checks |
|---|---|
| `REQ-FMT-001` | Golden bytes: canonical plans encode to committed files byte for byte |
| `REQ-FMT-002` | Round trip: `decode(encode(plan))` makes the machine do what the plan does — needle-downs, cuts and pauses at the same 0.1 mm positions — for every writer/reader pair, on random plans |
| `REQ-FMT-003` | Every command kind survives every writer/reader pair (and is sewn in TS-02) |
| `REQ-FMT-004` | Empty and zero-stitch files read cleanly; empty plans are never written |
| `REQ-FMT-005` | Independent oracle: pyembroidery reads our files to the same stitches (CI job with a pinned version) |
| `REQ-FMT-006` | Fuzzed readers never panic, never allocate beyond caps, always terminate |
| `REQ-FMT-007` | Long moves are split within the format's per-record limit |
| `REQ-FMT-008` | DST is read as its machines sew it (three or more jumps in a row after sewing cut the thread), and writing DST reports where they will cut and the plan does not (`SC-I0605`) |
| `REQ-FMT-011` | A PES design whose top-left corner lies more than 204.7 mm from the start is refused, not written with its origin field wrapped round |
| `REQ-FMT-012` | PEC thumbnails draw stitches only inside their frames, so the stitch data's end mark is followed by `00` |
| `REQ-THREAD-002` | Different threads get different palette entries, the nearest pairs first, and a thread read from a file keeps its entry |

The pyembroidery oracle is a conformance case (`conformance/cases/formats/pyembroidery-oracle.toml`): a
pinned pyembroidery, installed by hash in CI, reads every golden machine file through
`conformance/oracle/decode.py`, and its reading must make the machine do what our reader's does. Only its
output is compared; no pyembroidery code is vendored. pyembroidery places a PES design's top-left corner at
its origin where StitchCraft keeps the machine origin, so both readings are compared from their first
needle-down.
