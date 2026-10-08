# Rendering previews

`stitchcraft-render` draws a stitch plan as a PNG image: for `stitch preview`, for the images in these
docs, and for conformance diffs. Two promises shape it:

- **What is shown is what sews** (`REQ-RND-001`). Ink/Stitch's simulator draws the positions the plan
  meant, but files store positions rounded to 0.1 mm, so stitches "move" after saving
  ([Ink/Stitch #2066](https://github.com/inkstitch/inkstitch/issues/2066)). StitchCraft rounds every
  position with the writers' own function (`Point::to_tenths`) before drawing anything.
- **The same bytes everywhere** (`REQ-RND-002`). A preview is a pure function of the plan and the
  settings, so documentation images can be compared byte for byte on every pull request.

## Two steps: scene, then pixels

```text
StitchPlan ──Scene::of──▶ Scene (machine units, plain data) ──Scene::render──▶ PNG
```

The **scene** is what the fabric will show, on the 0.1 mm grid. Tests compare scenes, which says *what*
differs; only the golden files compare pixels.

| Scene item | Meaning |
|---|---|
| Hole | A place where the needle goes down, in sewing order, with the thread's colour |
| Arrival `Sewn` | A stitch laid from the previous hole (`StitchPlan::sewn_stitches`, the single definition of "sewing") |
| Arrival `Loose` | The frame moved without sewing and the thread was not cut: a jump thread that lies loose until cut by hand |
| Arrival `Cut` | The thread was cut since the previous hole (a trim or a thread change), or this is the first hole |
| Mark `Trim`, `Stop` | Where the needle is when the machine cuts or pauses |

Jumps are not in the scene. The thread runs straight from hole to hole however the frame travelled, so a
place where a jump paused changes nothing on the fabric, and machine files split long jumps into pieces
anyway. Marks describe what the machine does, not how a file spells it, in the same way as the round-trip
oracle (`stitchcraft_testkit::equivalence`): two trims in a row cut once, the order of commands at one
spot does not matter, and a trim after the last hole cuts nothing that stays. So marks are a sorted set
and trailing trims are left out. The test for `REQ-RND-001` relies on this: for every test sheet and for
random plans, the scene of a plan equals the scene of its PES file read back. The only differences
allowed are the ones PES cannot record: palette colours instead of chosen colours, and no stitch roles.

## Styles

| | Realistic | Simple |
|---|---|---|
| Purpose | The finished embroidery | Checking a design |
| Background | Unbleached cotton `#f4efe6` | White |
| Stitch | Thread 0.4 mm wide (40 wt): dark edge, body, light crest, round ends | Thin line in the thread's colour |
| Needle hole | (the stitch ends show it) | Dot |
| Loose jump thread | Thinner thread, on top of what is under it | Dashed, thread colour |
| Travel after a cut | Nothing | Dashed, grey |
| Lock stitch | (like any stitch) | Orange ring |
| Trim, stop | Nothing | Red cross, blue square |

Everything is drawn in sewing order, so later stitches lie on earlier ones as they do on the fabric. The
realistic style draws loose jump threads because they *are* on the fabric when the hoop comes off the
machine; a design that should have trims shows them, which is what test sheet TS-02 is about. (A machine
that cuts long jumps by itself would remove some of them; previews do not model that yet.)

## Size and scale

The scale is in pixels per millimetre (default 8, about 200 dpi; 0.1 to 50), so a design looks the same
at every scale, only sharper. The image is the design's extent plus a 2 mm margin on each side. Images
are at most 4,096 pixels on a side (64 MiB of pixels); a larger request is refused with `SC-E0801`, whose
message gives the largest scale that fits. Drawing charges the caller's work budget: one unit per element
plus one per pixel of its length, so `SC-E0004` stops a pathological request instead of a long wait.

## Determinism

| Risk | Answer |
|---|---|
| SIMD code paths giving different rounding | tiny-skia is built without its `simd` feature: the same scalar pipeline on every CPU |
| Platform trigonometry | Only lines and circles are drawn; tiny-skia flattens circles and round caps (conics) with arithmetic and square roots, never reaching its cubic root-finding or rotations; `f32::sin` & co. are disallowed in `clippy.toml` |
| Coordinates | Grid positions are integers below 2^24, converted to `f32` exactly |
| PNG encoding | png with miniz_oxide: pure Rust, no timestamps, versions pinned by `Cargo.lock` |

The golden PNG files in `conformance/golden/render/` are compared by `cargo test` on Linux, macOS and
Windows. Like every golden file, they change only on purpose: re-bless with `STITCHCRAFT_BLESS=1 cargo
test -p stitchcraft-render --test preview` in a pull request labelled `golden-change` that says why in
`CHANGELOG.md`.

## Later

- The hoop and the comfort zone drawn around the design, from a machine profile.
- Thread direction and sheen for fills and satins, and a density map, once those stitch types exist (M3–M5).
- Inside VectorCraft the plug-in returns stitch geometry and VectorCraft draws it on its own canvas;
  this renderer is for files, the command line and the docs.
