# Diagnostics

A diagnostic is a problem with the user's design or input, explained in the user's terms, with a
stable code. Diagnostics are values: the engine collects them and keeps planning whatever it can.
Errors in the Rust sense (`Result`) are reserved for failures to do the work at all
([architecture](architecture.md#errors-and-diagnostics)).

```rust,ignore
pub struct Diagnostic {
    pub code: Code,                  // SC-W0702
    pub severity: Severity,          // Error | Warning | Info
    pub element: Option<ElementId>,  // which object
    pub at: Option<Point>,           // where on the canvas (mm)
    pub message: String,             // one sentence, specific: "Design is 162 × 148 mm; …"
    pub fix: Option<Fix>,            // a hint, or a machine-applicable fix
}
```

## Codes

`SC-` + `E` (error: this element, or the design, cannot be stitched), `W` (warning: stitched, but
probably not what you want) or `I` (info) + four digits grouped by area:

| Range | Area |
|---|---|
| 0000–0099 | Input and budgets |
| 0100–0199 | Parameters |
| 0200–0299 | Satin |
| 0300–0399 | Fills |
| 0400–0499 | Strokes (running, bean, manual, zigzag, ripple) |
| 0500–0599 | Plan assembly (ties, travel, trims, colours) |
| 0600–0699 | Formats and threads |
| 0700–0799 | Machine profile (hoop, limits) |
| 0800–0899 | Hosts (SVG, VectorCraft) |

Codes are never reused. A retired code keeps its page with "retired in vX".

## The registry

All codes live in one table in `stitchcraft-core`: the `registry!` block in `src/diag.rs`, which
defines the `Code` enum. Each entry is one line — variant, id, severity, title — and its doc comment is
the long explanation (Markdown: what it means, why it matters for the sew-out, how to fix it), so the
text users read is also the code's rustdoc. Tests reject an entry whose id is malformed or duplicated,
whose letter disagrees with its severity, or whose explanation is missing. From that table:

- `cargo xtask docs` generates the **diagnostics index** (one page per code, like `rustc`'s error index);
- `stitch explain SC-W0702` prints the explanation in the terminal;
- the VectorCraft plug-in links each message to its page;
- a test fails when a code has no explanation, no triggering conformance case, or a case triggers a
  code that is not registered.

That last rule is what keeps error paths alive: Ink/Stitch shipped an error message reading "There
are d color changes" because the message was never rendered by a test
([finding F4](inkstitch-analysis.md#f4--a-verified-bug-in-an-error-path)).

## Writing good messages

- Say what is wrong *and* by how much: "Rows are 0.25 mm apart but the region is only 0.18 mm tall."
- Name the object and point at the place.
- Offer the fix in embroidery terms: "Use a running stitch for parts this thin, or widen the shape."
- No blame, no internal names (`LineString`, `GEOS`), no stack traces.

## Fixes

`Fix::Hint(String)` is advice. `Fix::Apply(Edit)` is a change the host can apply with one click and
undo (for example "rotate the design 90°" for `SC-W0702` when the rotated bounds fit, or "drop the
dangling rung" for `SC-W0203`). Applied fixes go through the host's command system so they are undoable.

## Initial codes

Codes referenced by the design docs; each is registered in the milestone that implements it.

| Code | Severity | Title | Milestone |
|---|---|---|---|
| `SC-E0004` | Error | Budget exhausted (work for one element, or stitches for the design) | M1 |
| `SC-E0009` | Error | Internal check failed (a StitchCraft bug; a bug-report bundle is written) | M1 |
| `SC-E0010` | Error | Nothing to stitch (no embroiderable elements) | M3 |
| `SC-E0101` | Error | Parameter has the wrong type or an unknown choice | M3 |
| `SC-W0102` | Warning | Parameter clamped to its allowed range | M3 |
| `SC-W0105` | Warning | Unknown parameter preserved but ignored | M3 |
| `SC-E0201` | Error | Satin needs exactly two rails | M4 |
| `SC-W0203` | Warning | Rung does not cross both rails (dangling rung) | M4 |
| `SC-E0204` | Error | Rails cross each other | M4 |
| `SC-W0206` | Warning | Satin narrower than the minimum width; underlay skipped | M4 |
| `SC-W0207` | Warning | Rung crosses a rail more than once; ignored | M4 |
| `SC-W0208` | Warning | Satin stitches skew more than 45° from the column; add a rung here | M4 |
| `SC-W0209` | Warning | Satin wider than 12 mm; long stitches may snag | M4 |
| `SC-W0303` | Warning | Tiny ring dropped from fill region | M5 |
| `SC-W0305` | Warning | Region too small for fill rows; outlined with running stitch instead | M5 |
| `SC-W0307` | Warning | Region split into parts that are not connected; parts joined with trims | M5 |
| `SC-W0311` | Warning | Spiral could not be connected in a narrow part; that part uses inner-to-outer contours | M7 |
| `SC-W0401` | Warning | Path shorter than the minimum stitch length; skipped | M3 |
| `SC-W0402` | Warning | Stitch length below the profile minimum; raised to the minimum | M3 |
| `SC-W0501` | Warning | Travel could not stay inside the region; used tie-off, trim and tie-in | M5 |
| `SC-E0601` | Error | Too many colour changes for the format | M1 |
| `SC-E0602` | Error | Design too large for the file format (coordinates or data exceed its fields) | M1 |
| `SC-E0603` | Error | Machine file could not be read (wrong format, truncated, or a record that makes no sense) | M2 |
| `SC-E0701` | Error | Design does not fit the hoop | M1 |
| `SC-W0702` | Warning | Design is larger than the profile's comfort zone | M1 |
| `SC-E0801` | Error | SVG could not be read (with the parser's position) | M3 |
| `SC-W0802` | Warning | SVG feature ignored (e.g. raster image, text not converted to paths) | M3 |
| `SC-W0803` | Warning | `.vectorcraft` file from a newer VectorCraft format version; read best-effort | M6 |
