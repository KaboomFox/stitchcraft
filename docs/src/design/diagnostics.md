# Diagnostics

<!-- implements: crates/stitchcraft-core/src/diag.rs, crates/stitchcraft-core/src/diag/registry.rs, crates/stitchcraft-core/src/diag/registry/auto.rs, crates/stitchcraft-core/src/text.rs, apps/stitchcraft-cli/src/commands/explain.rs -->

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
| 0900 to 0999 | Automatic digitizing ([image to embroidery](image-to-embroidery.md)) |
| 1000 to 1099 | Routing ([routing tools](routing.md)) |
| 1100 to 1199 | Fonts and lettering ([fonts](fonts.md#the-reader)) |
| 1200 to 1299 | Subject cut-out ([image to embroidery](image-to-embroidery.md#subject-cut-out)) |

Codes are never reused. A retired code keeps its page with "retired in vX".

## The registry

The codes are one table in `stitchcraft-core`, the `registry!` block in `src/diag/registry.rs`, which
defines the `Code` enum. Automatic digitizing's codes, `SC-x09xx`, are written in `src/diag/registry/auto.rs`,
which keeps the table's file under the size limit. `Code` lists them in the order of their codes.
Each entry is one line (variant, id, severity, title), and its doc comment is
the long explanation (Markdown: what it means, why it matters for the sew-out, how to fix it), so the
text users read is also the code's rustdoc. Tests reject an entry whose id is malformed or duplicated,
whose letter disagrees with its severity, or whose explanation is missing. From that table:

- `cargo xtask docs` generates the **diagnostics index** (one page per code, like `rustc`'s error index);
- `stitch explain SC-W0702` prints the explanation in the terminal;
- in VectorCraft, the plug-in shows each message after its code, for the user to look up in the index or
  with `stitch explain`;
- `cargo xtask conformance` fails when a code has no `diag_sc_<code>_<what>` test that produces it from
  real input and checks the text a user reads, or when such a test names a code that is not registered
  ([conformance](conformance.md#cases)); the registry's own tests fail when a code has no explanation.

That last rule is what keeps error paths alive: a message that no test ever renders can ship with a
typo in its formatting and nobody sees it until a user does.

## Writing good messages

- Say what is wrong *and* by how much: "Rows are 0.25 mm apart but the region is only 0.18 mm tall."
- Name the object and point at the place.
- Offer the fix in embroidery terms: "Use a running stitch for parts this thin, or widen the shape."
- No blame, no internal names (`LineString`, `GEOS`), no stack traces.

## As text

A diagnostic reads the same wherever StitchCraft prints it. Its `Display` is the severity, the code and
the message: `warning SC-W0702: Design is 190.0 × 150.0 mm; …`. `Diagnostic::headline` adds the object
it is about, as in `warning SC-W0307 (svg:patch:fill): The area falls into 2 parts.`, and the command
line prints that, with the fix on the line below. `Diagnostic::one_line` puts the fix after the
headline, for a host that shows each diagnostic as one line of text, as VectorCraft shows an export's
warnings.

## Fixes

`Fix::Hint(String)` is advice. `Fix::Apply(Edit)` is a change the host can apply with one click and
undo (for example "rotate the design 90°" for `SC-W0702` when the rotated bounds fit, or "drop the
dangling rung" for `SC-W0203`). Applied fixes go through the host's command system so they are undoable.

## Initial codes

Codes referenced by the design docs; each is registered in the milestone that implements it. A
registered code must keep its row here, with the same severity and a title that starts with the
registry's (`cargo xtask docs --check`), so a code allocated for one thing cannot be registered for
another.

| Code | Severity | Title | Milestone |
|---|---|---|---|
| `SC-E0004` | Error | Budget exhausted (work for one element, work joining the elements, or stitches for the design) | M1 |
| `SC-E0005` | Error | Preview too large (over 4,096 pixels on a side at the requested scale) | M2 |
| `SC-E0009` | Error | Internal check failed (a StitchCraft bug; a bug-report bundle is written) | M1 |
| `SC-E0010` | Error | Nothing to stitch (no embroiderable elements) | M1 |
| `SC-W0011` | Warning | Stitch type not sewn yet, element skipped (retired): M10.10 sews the last of them | M3 |
| `SC-E0012` | Error | Bug-report bundle could not be replayed | M3 |
| `SC-W0013` | Warning | Element outside its clip paths; left out (wholly outside them or a VectorCraft clipping object, or under clips that don't meet or have no area; Ink/Stitch sews one whose clip path is empty whole) | M8 |
| `SC-E0101` | Error | Parameter has the wrong type or an unknown choice | M3 |
| `SC-W0102` | Warning | Parameter clamped to its allowed range | M3 |
| `SC-W0105` | Warning | Unknown parameter preserved but ignored | M3 |
| `SC-I0106` | Info | Value only the node's other stitch type reads; default used here (as in Ink/Stitch) | M7 |
| `SC-E0201` | Error | Satin column has no rails (no subpath longer than a point) | M4 |
| `SC-W0202` | Warning | Satin rails taken as the two longest subpaths (where the subpaths meet does not say) | M4 |
| `SC-W0203` | Warning | Satin rung does not cross both rails (dangling rung); the rail's nearest point is used | M4 |
| `SC-W0205` | Warning | Satin subpath is one point; left out | M4 |
| `SC-W0206` | Warning | Satin contour underlay too short for its insets; it keeps its length | M4 |
| `SC-W0207` | Warning | Satin rung crosses a rail more than once; left out | M4 |
| `SC-W0208` | Warning | Satin stitches lean more than 45° from square to the column; add a rung here | M7 |
| `SC-W0209` | Warning | Satin stitch longer than 12 mm; it may snag | M7 |
| `SC-W0210` | Warning | Satin rails without rungs have different numbers of nodes (some pair with none) | M4 |
| `SC-W0211` | Warning | Satin push compensation too long for a rail; that rail keeps its length | M4 |
| `SC-W0212` | Warning | Satin column drawn as one path too narrow; sewn as a stroke | M4 |
| `SC-W0213` | Warning | Parts of a satin column drawn as one path left out (still crossing after 20 cuts in half, or turning too tightly for its width) | M4 |
| `SC-E0214` | Error | Satin column drawn as one path could not be made into rails (too short, or too tight for its width) | M4 |
| `SC-W0215` | Warning | Satin column too small for the shortest stitch; left out (all within it of its first needle point, or a zigzag satin with no stitch across, underlays and all; Ink/Stitch sews a speck) | M4 |
| `SC-E0216` | Error | Satin column has no section to sew (its rungs or push compensation take each section's length off a rail) | M4 |
| `SC-W0217` | Warning | Satin column is clipped; sewn whole (as in Ink/Stitch) | M8 |
| `SC-W0303` | Warning | Parts of a fill too small to sew left out (3 square CSS pixels or less, as in Ink/Stitch); a fill of no area sews nothing | M5 |
| `SC-W0304` | Warning | Fill smaller than 1.4 mm² (20 square CSS pixels); a running stitch or a satin column may sew it better | M5 |
| `SC-W0305` | Warning | Fill part too thin for rows; sewn round its outline (as in Ink/Stitch) | M5 |
| `SC-I0306` | Info | Nonzero fill rule fills a part inside that the even-odd rule leaves empty (Ink/Stitch leaves it empty) | M5 |
| `SC-W0307` | Warning | Fill in parts that are apart; each sewn on its own, joined by jumps | M5 |
| `SC-I0308` | Info | Fill underlay too thin for rows; sewn round the outline (as in Ink/Stitch) | M5 |
| `SC-W0309` | Warning | Fill's expand shrinks a part away; sewn at its own size (as in Ink/Stitch) | M5 |
| `SC-W0310` | Warning | Fill's underlay inset shrinks a part away; underlay over all of it (as in Ink/Stitch) | M5 |
| `SC-W0311` | Warning | Contour fill leaves out rings it does not reach, where its rings part or lie too far apart (as in Ink/Stitch, which says nothing) | M7 |
| `SC-W0312` | Warning | Fill's pull compensation below 0; read as 0 (as in Ink/Stitch) | M5 |
| `SC-I0313` | Info | Fill drawn twice along its edge; Ink/Stitch fills the part left empty (its repair merges the stretch) | M5 |
| `SC-W0314` | Warning | Legacy fill sewn as a tatami fill without underlay (Ink/Stitch's legacy fill is not built) | M7 |
| `SC-I0315` | Info | Contour fill's part too narrow for its spiral; sewn inner to outer (Ink/Stitch fails or sews nothing) | M7 |
| `SC-W0316` | Warning | Meander pattern is one of Ink/Stitch's tiles; sewn with StitchCraft's default tile | M7 |
| `SC-W0317` | Warning | Meander fill part too small for its tile; sewn round its outline (Ink/Stitch stops with an error) | M7 |
| `SC-W0318` | Warning | Circular fill part within its row spacing of its middle; sewn as one circle (as Ink/Stitch sews it) | M7 |
| `SC-W0319` | Warning | Cross stitch part covers no cell of its grid enough; sews nothing (Ink/Stitch warns too) | M10 |
| `SC-W0320` | Warning | Cross stitch with an odd thread count; an even count sewn (1 sews 2, where Ink/Stitch sews no cross) | M10 |
| `SC-W0321` | Warning | Cross stitch grid too fine for the shortest stitch; needle points left out | M10 |
| `SC-W0322` | Warning | Half cross with an even thread count; the odd count above sewn (as Ink/Stitch sews it) | M10 |
| `SC-E0323` | Error | Tartan fill's sett can't be read; skipped (not JSON, no palette, or a threadcount or colour letters before M10.7) | M10 |
| `SC-W0324` | Warning | Tartan's herringbone sewn as straight rows (retired): M10.7 sews herringbones | M10 |
| `SC-W0325` | Warning | Tartan fill without a sett; Ink/Stitch's default sett sewn (as Ink/Stitch sews it) | M10 |
| `SC-W0326` | Warning | Tartan sett with no stripe to sew; only the underlay sewn (as in Ink/Stitch) | M10 |
| `SC-W0327` | Warning | Tartan sett too fine for the shortest stitch; needle points left out | M10 |
| `SC-W0328` | Warning | Tartan stripe's colour can't be read; left out (as Ink/Stitch leaves it) | M10 |
| `SC-W0329` | Warning | Linear gradient fill without a linear gradient; its colour fades out | M10 |
| `SC-W0330` | Warning | Gradient stop within 2 rows of the blend before; passed over | M10 |
| `SC-W0331` | Warning | Guided fill without a guide line; sewn as a tatami fill (also for a guide of no length, or a copied one that starts and ends at one point, with the fill's own settings, as Ink/Stitch sews one without a guide) | M10 |
| `SC-W0332` | Warning | Guided fill's rows along its guide meet no part; sewn as a tatami fill (that part, at the angle of the guide's ends, the fallback Ink/Stitch has for a part its rows meet and the cut leaves empty, while it stops with an error where copies or offset curves miss the part, `DEV-FILL-034`) | M10 |
| `SC-W0333` | Warning | Guided fill's rows apart, the needle jumps between them (rows that only meet a hole's edge, with no row from that hole to the outline, as Ink/Stitch sews the rows it can't join on their own) | M10 |
| `SC-W0401` | Warning | Path too small for the shortest stitch; skipped | M3 |
| `SC-W0402` | Warning | Stitch length below twice the shortest stitch; raised to it, so even spacing never sews a stitch shorter than the shortest | M3 |
| `SC-W0403` | Warning | Hand-placed stitch shorter than the shortest stitch; point left out | M3 |
| `SC-W0404` | Warning | Hand-placed stitch longer than max_stitch_length_mm; kept for the shortest stitch | M3 |
| `SC-W0405` | Warning | Zigzag stitch shorter than the shortest stitch; the zigzag sews narrower | M7 |
| `SC-W0406` | Warning | Ripple stitch setting not applied; sewn without it (the satin column's `push_compensation_mm` along a column) | M10 |
| `SC-W0407` | Warning | Ripple stitch of more than 1,000 lines; cut to 1,000, or 999 to keep the parity of `line_count` | M10 |
| `SC-W0408` | Warning | Ripple stitch's skips would leave out every line; every line sewn | M10 |
| `SC-W0409` | Warning | Ripple stitch's guide can't place its copies; sewn without it | M10 |
| `SC-W0502` | Warning | Lock stitch shorter than 0.2 mm; lengthened | M3 |
| `SC-W0503` | Warning | Custom lock cannot be sewn as written | M3 |
| `SC-I0504` | Info | Stitches shorter than the shortest stitch merged | M3 |
| `SC-W0505` | Warning | Trim or stop after an element that sews nothing; left out | M3 |
| `SC-I0506` | Info | Underlays first joins no run; the element sews as without it (it would make a run of one, or it is sewn in several threads) | M10 |
| `SC-E0601` | Error | Too many colour changes for the file format | M1 |
| `SC-E0602` | Error | Design too large for the file format (coordinates or data exceed its fields) | M1 |
| `SC-E0603` | Error | Machine file could not be read (wrong format, truncated, or a record that makes no sense) | M2 |
| `SC-W0604` | Warning | Thread colours unknown (the file stores none, as DST never does; threads are placeholders) | M2 |
| `SC-I0605` | Info | Long jumps cut in DST (machines cut the thread before three or more jump records in a row) | M3 |
| `SC-W0606` | Warning | Threads share a Brother colour (more different threads than a PES file's 61 thread colours) | M3 |
| `SC-E0701` | Error | Design does not fit the hoop | M1 |
| `SC-W0702` | Warning | Design is larger than the comfort zone (the profile's most accurate area) | M1 |
| `SC-I0703` | Info | Stitches longer than the machine's longest stitch; split | M3 |
| `SC-E0801` | Error | SVG could not be read (with the parser's position) | M3 |
| `SC-W0802` | Warning | SVG feature ignored (e.g. raster image, text not converted to paths, an Ink/Stitch file Ink/Stitch would update or warn about, read as it is) | M3 |
| `SC-W0803` | Warning | VectorCraft file from a newer format version; read best-effort (a version newer than the newest the adapter reads) | M6 |
| `SC-W0804` | Warning | Element geometry invalid or out of range (path data error, invalid transform, draws nothing, beyond 10 m, a stroke's width, join or miter limit that does not read, a clone that shows itself); the usable part is stitched | M3 |
| `SC-I0805` | Info | Object left out, as the file asks (Ink/Stitch's ignore commands and `ignore_object` setting, or a clone whose `clone` setting is off) | M3 |
| `SC-E0806` | Error | VectorCraft file could not be read (not a VectorCraft document, or larger than the adapter reads). Its JSON counts, and so do the copies of names, settings and paths its elements would hold | M6 |
| `SC-W0807` | Warning | VectorCraft object or feature not stitched (text, images, symbols, live objects, opacity masks, VectorCraft's own effects, brushes, gradients, patterns, a group's own paints), what adds nothing to a clip, StitchCraft effects from more than one level or on a clipping object, a start or end marker, or a `stitch:` name StitchCraft doesn't read | M6 |
| `SC-W0808` | Warning | VectorCraft geometry unusable (a number that does not read or a point beyond 10 m; a shape that draws nothing; a clipping path, which adds nothing to its clip) | M6 |
| `SC-I0809` | Info | VectorCraft layer or guide left out, as it does not print (template layers, layers whose printing is off, guides) | M6 |
| `SC-I0810` | Info | VectorCraft live effect leaves the object as it is (it sews no stitch, its preview level needs more work than VectorCraft lets a live effect do, it is a satin column drawn as one path or a zigzag stroke and VectorCraft gives the effect no stroke, or the effect is on one of the object's fills or strokes) | M6 |
| `SC-I0811` | Info | VectorCraft object left out, as its name asks (an object or a layer named `stitch:ignore`) | M6 |
| `SC-W0812` | Warning | VectorCraft design setting ignored (a length that isn't a number of 0 or more, or a machine StitchCraft doesn't have, which the Design Settings filter keeps with the document) | M6 |
| `SC-E0813` | Error | VectorCraft too old for the filter (the Design Settings filter run by a VectorCraft before 0.16, which doesn't keep a filter's data, Stroke to Satin by one before 0.15, which doesn't keep a filter's effect records, and Test Sheet by one before 0.16, which doesn't say it keeps them) | M6 |
| `SC-E0814` | Error | VectorCraft line not made into a satin column (Stroke to Satin refused a selection because it is empty or a path has no stroke, or because a line drawn with its rails and rungs would sew otherwise) | M6 |
| `SC-E0815` | Error | VectorCraft's command line could not be started (`stitch plan` traces a picture with it: neither on the PATH nor where `--vectorcraft-cli` says) | M13 |
| `SC-E0816` | Error | VectorCraft could not trace the picture (its command line failed or didn't save the document) | M13 |
| `SC-E0817` | Error | VectorCraft test sheet not drawn (in the middle of the artboard, parts of the sheet would lie more than 10 m from the document's origin) | M6 |
| `SC-I0818` | Info | Ink/Stitch file from an older version, read as Ink/Stitch updates it (the shapes given a default or a setting whose meaning has changed since, such as a fill's 3 mm stitches before its file version 4, counted for each change) | M3 |
| `SC-E0819` | Error | More than one origin or stop position (Ink/Stitch's `origin` command, or more than one object named `stitch:origin`, 2 or more times, or its `stop_position` command or `stitch:stop-position` in a design that stops: no machine file is written, as Ink/Stitch writes none) | M8 |
| `SC-E0820` | Error | VectorCraft selection not routed (no selected path has an effect of its own that the filter routes, satin for Auto-Route Satin Columns and stroke for Auto-Route Running Stitch, or 2 objects selected mark the route's start, or 2 its end: nothing changes) | M10 |
| `SC-E0821` | Error | VectorCraft tartan's sett not set (StitchCraft Tartan's code can't be read, names letters the register uses for no colour, lacks a stripe or makes a setting longer than the fill's Sett holds, or no selected path takes a sett: nothing changes) | M10 |
| `SC-W0822` | Warning | Kept thread left out of the picture's trace (no part of the picture is nearer the colour `--keep-colour` kept than the other threads') | M13 |
| `SC-I0901` | Info | Shapes digitized automatically (`--auto`, a VectorCraft document by default, or VectorCraft's Auto effect), counted by their stitches (running stitches, satin columns or tatami fills) | M13 |
| `SC-I0902` | Info | Satin column can't be made; filled (a part of an automatic line that turns more tightly than it is wide, is shorter than a CSS pixel or is a dot its cap draws, or a part of an automatic area whose triangles don't make a column round one of its lines or whose column would be read back otherwise) | M13 |
| `SC-W0903` | Warning | Line too wide to fill; left out (an automatic line whose filled area would reach more than 10 m from the origin) | M13 |
| `SC-I0904` | Info | Areas reach under the shapes sewn after them (automatic areas grown under the shapes sewn after them that they touch, or cut away under the areas and parts' lines and columns sewn after them that cover them, by an overlap of 0.4 mm unless an area sets another) | M13 |
| `SC-I0905` | Info | Speck merged into a neighbour (an automatic area or a part of one smaller than its smallest area, 1 mm² unless it sets another, merged into the area it shares the longest edge with, or left out) | M13 |
| `SC-W0906` | Warning | Background left out (the lowest shape, an automatic area that covers the whole design under the other shapes, unless it switches `auto_ignore_background` off; what shows of it inside the picture is sewn) | M13 |
| `SC-I0907` | Info | Elements sewn in an order of their own (automatic elements, and with `--sort-colours` every element, sewn in the order StitchCraft chooses, with the thread changes of each order; an element or an order that needs more work than the budget allows keeps paint order) | M13 |
| `SC-I0908` | Info | Layers flattened (each automatic area sews what shows of it under the automatic areas drawn over it, and an area wider than a satin column somewhere is digitized whole under the areas that fill its holes, which sew after it) | M13 |
| `SC-I0909` | Info | Base layer sewn under the design (one underlay under the outer shape of the automatic shapes, with its area, pieces and thread) | M13 |
| `SC-W0910` | Warning | Base layer left out (its shape needs more work than the budget allows, and every fill sews the underlay of its settings) | M13 |
| `SC-I0911` | Info | Base layer turned off (the design's lowest automatic shape turns `auto_base_layer` off, so nothing lies under a place the top thread leaves bare, and fabric may show there) | M13 |
| `SC-I0912` | Info | Rule that chose a shape's stitches (for each part of an automatic shape, the rule of the table it sewed by and the measures that decided it) | M13 |
| `SC-W0913` | Warning | Design sews thick (a design digitized automatically has 1 mm² or more with 3 fills of thread or more over it, or 15 needle points or more, and is written all the same) | M13 |
| `SC-I0914` | Info | Jumps not looked at for a run under later shapes (finding the runs that take the place of jumps between automatic elements needed more work than the budget allows, and the moves from the element named on are as they were) | M13 |
| `SC-I0915` | Info | Thin line merged into a neighbour (a ragged part of an automatic area narrower than the narrowest satin stroke merges into the automatic area it shares the longest edge with, or keeps its lines when finding them needs more work than the budget allows) | M13 |
| `SC-I0916` | Info | Parts kept by a mark (each mark named `stitch:keep` or given by `--keep`, with the parts that lie wholly inside it and how many of them sew that would have merged as specks or thin lines) | M13 |
| `SC-W0917` | Warning | Mark not used (a mark that has no part of an automatic shape wholly inside it, as a mark without an area has none, and the design then sews as without it) | M13 |
| `SC-I0918` | Info | Parts that follow a guide (each guide named `stitch:guide` or given by `--guide`, with the areas and the filled parts that sew a guided fill along it, and the parts it crosses that keep their stitches) | M13 |
| `SC-W0919` | Warning | Guide not used (a guide that crosses no filled part of an automatic shape, whose line has no length, or in a design with no automatic shape, and the design then sews as without it) | M13 |
| `SC-W0920` | Warning | Own Auto settings replace the group's (a path or a compound path with StitchCraft's Auto effect inside a group or a layer with one sews by its own settings alone: the message names it and the group's settings it doesn't take) | M13 |
| `SC-I0921` | Info | Outlines smoothed (how many points the smoothed automatic areas had and have, and the areas whose smoothing needed more than the work budget, which keep their outlines) | M13 |
| `SC-W0922` | Warning | What Expand couldn't keep (lettering that became its columns, a shape left out for its budget, and a part a VectorCraft document has without its clips, each named) | M13 |
| `SC-I0923` | Info | Style that changes how a design sews (the thin lines merged or kept and the parts filled or sewn as satin columns that the other style would sew another way, with that style's name) | M13 |
| `SC-I0924` | Info | Shapes digitized by default (a VectorCraft document planned without `--auto` or `--no-auto`: how many shapes without StitchCraft effects are digitized automatically and how many sew as their effects say, once for the design) | M13 |
| `SC-E1001` | Error | Nothing to route (no element the tool can take among those given, a satin column for Auto-Route Satin Columns or a stroke for Auto-Route Running Stitch: they stay as they were) | M10 |
| `SC-I1002` | Info | Element left out of the route (a fill, a shape digitized automatically, settings that don't read, rails that can't be made, no length, a stroke whose first subpath ends where it starts, or a satin column among running stitches: kept as it is) | M10 |
| `SC-W1003` | Warning | Piece of a routed satin column may sew otherwise (read back, it has other rails or pairs other points: it is kept) | M10 |
| `SC-W1004` | Warning | Too many lines to cut where they cross (more than 1,000 lines routed as running stitches: none is cut by another, as with Add nodes at intersections off) | M10 |
| `SC-E1101` | Error | Font could not be read (a `font.json` that is missing or isn't an object of settings, or glyph files that are missing or aren't SVG StitchCraft reads: nothing of the font is sewn) | M14 |
| `SC-W1102` | Warning | Font setting not read, its default kept (a value in `font.json` of another type than its key takes, which keeps Ink/Stitch's default, or an entry of a list or a table of another type, which is left out) | M14 |
| `SC-W1103` | Warning | Font file not read (a glyph file compressed with xz, as in Ink/Stitch's packages: its glyphs are left out) | M14 |
| `SC-W1104` | Warning | Glyph left out (a glyph whose paths give it no box, or every glyph of a file whose baseline guide doesn't give a place, as Ink/Stitch's lettering leaves them out) | M14 |
| `SC-I1105` | Info | Font read as Ink/Stitch reads it (a key the format doesn't have, a glyph drawn twice in a direction, of which the later counts, a file without a baseline guide, or a font without glyphs for its `default_variant`, which sews its first direction) | M14 |
| `SC-W1106` | Warning | Scale outside the font's range (lettering sets the text at the nearest scale between the font's `min_scale` and `max_scale`, and names the range in percent and in millimetres) | M14 |
| `SC-W1107` | Warning | Character not in the font (it sews as the font's default glyph, or is left as a space in a font without that glyph) | M14 |
| `SC-E1108` | Error | Text could not be set (a text none of whose characters sews, or one with a glyph that can't be placed. Along a path, also a path without length) | M14 |
| `SC-W1109` | Warning | Text longer than its path (the line goes on straight past an open path's end, round a closed path again, or is pushed together when stretched) | M14 |
| `SC-W1110` | Warning | Lettering setting not applied (lines asked to sew back and forth in a font drawn for one direction, or colours asked to be sorted in a font that doesn't sort) | M14 |
| `SC-W1111` | Warning | Text not sewn in its embroidery font (a VectorCraft text object with the Lettering effect whose font isn't found or doesn't read, or that is area type, type on a path or vertical type, and a text object under automatic digitizing without a font or with a line that can't be set) | M14 |
| `SC-I1112` | Info | Traced letters replaced by lettering (what lay inside a text marker of a picture, with both heights, the text and the font) | M14 |
| `SC-W1113` | Warning | Text marker not used (without text, without an area, with every shape partly outside it, or with a font that is missing or unreadable), or one of its settings left unread | M14 |
| `SC-W1114` | Warning | Shape crosses a text marker's edge (more than half inside it, and sewn as traced beside the lettering) | M14 |
| `SC-W1115` | Warning | Lettering not the traced letters' size (set at the nearest height the font sews, or over a tenth of their height from their width) | M14 |
| `SC-I1116` | Info | Text sewn in an embroidery font (a text object under automatic digitizing, whose own typeface can't be read. The message has the font used, the height with its reason, and the longest line's length) | M14 |
| `SC-W1117` | Warning | Lettering longer than its text's place (a line of a text object under automatic digitizing that is longer than the length its text most likely had, at the smallest height the font sews at) | M14 |
| `SC-I1118` | Info | Font chosen by the letters' weight (a text marker without a font, set in the font nearest in weight to its traced letters among those that sew at their height, or a bold text object sewn in the bold font where it can. The message names the font and says why) | M14 |
| `SC-I1119` | Info | Thread taken from thick spots (by the last pass over a design digitized automatically and the cap after it, where a spot still sewed above the cap: how much of each kind, and what is left) | M13 |
| `SC-E1201` | Error | Subject cut-out's models not ready (a model that couldn't be fetched, or isn't the file pinned: the message names every file and the folder they belong in) | M13 |
| `SC-E1202` | Error | No subject found to cut out (no part of U²-Net-p's map reaches a half and no box is given, or MobileSAM didn't keep a pixel) | M13 |
| `SC-E1203` | Error | Picture could not be read for the cut-out (not a PNG or JPEG file it reads, or larger than its limits) | M13 |
| `SC-E1204` | Error | Cut-out model could not be run (tract refused a model that matches its pin) | M13 |
| `SC-E1205` | Error | Cut-out model has a random operator (refused before it runs, as tract would seed it from the computer) | M13 |
| `SC-I1206` | Info | Subject cut out (the box it came from, the share of the picture kept and the cut-out's size) | M13 |
| `SC-E1207` | Error | Subject cut-out left out of this build (a `stitch` built without its `cutout` feature) | M13 |
