# Documentation pipeline

The bar: **documentation that cannot silently drift from the code.** Hand-made screenshots, parameter
lists kept apart from the code and an unversioned site all go stale without anyone noticing. Ours are
generated where they can be, tested where they cannot, and regenerated on every pull request.

## Structure

The site is an [mdBook](https://rust-lang.github.io/mdBook/) in `docs/`, organised by the
[Diátaxis](https://diataxis.fr/) framework so each page has one job:

| Section | Job | Examples |
|---|---|---|
| Tutorials | Learn by doing, start to finish | "Your first sew-out on a Brother", "A patch in VectorCraft" |
| How-to guides | Solve one task | "Stop a fill from puckering", "Export for a different machine" |
| Reference | Look things up (mostly generated) | parameters, diagnostics, CLI, machine profiles, formats, compatibility |
| Explanation | Understand why | embroidery basics, pull and push, underlay, how routing works |
| Design / Plan / Contributing | Build StitchCraft | this page, the roadmap, playbooks |

The `print.html` page mdBook builds is the printable manual, and mdBook's built-in search covers
everything.

## Sources of truth → generated pages

| Source (code or data) | Generated page(s) | Generator |
|---|---|---|
| Parameter registry | one page per stitch type with every parameter, its range, default, units and Ink/Stitch name; JSON Schema | `cargo xtask docs` |
| Diagnostics registry | diagnostics index, one page per code | `cargo xtask docs` |
| `clap` CLI definition | CLI reference | `cargo xtask docs` |
| Machine profiles | machine profile reference | `cargo xtask docs` |
| Format modules (limits, commands) | format reference | `cargo xtask docs` |
| `conformance/inkstitch-params.toml` + registry | [compatibility contract](inkstitch-compat-contract.md) with status | `cargo xtask docs` |
| Compatibility runs | supported VectorCraft versions | `cargo xtask compat report` (M6.6) |

Generated pages are committed (reviewers see user-facing changes in the diff) and marked with a header
comment; `cargo xtask docs --check` regenerates them in memory and fails if anything differs, or if a
file in a generated directory has lost its generator. The header is for people. The checks know
a generated page by the generator that writes it, so a page that only says it is generated is checked as
one written by hand. The conformance report is not a page of the book: `cargo xtask conformance` writes it
to `target/conformance/`, and CI shows it in the job summary.

## Images: four kinds, each reproducible

Every image in the docs is declared in `docs/shots.toml` with an id, a kind and how to make it. Pages
reference `images/generated/<id>.png`, or a photo in `images/photos/`. An image at any other path fails the
docs check, whatever its file name.

| Kind | Made by | Compared by | Notes |
|---|---|---|---|
| `stitch` | `stitchcraft-render` ([rendering](rendering.md)) from a test sheet, or from a design the engine plans, an SVG file or a VectorCraft document | **exact bytes** (rendering is deterministic) | Stitch-type illustrations, parameter comparisons, diagnostics examples |
| `vectorcraft-render` | `vectorcraft-cli` of the pinned stable release, with the plug-in's modules installed: the canvas after the shot's `steps`, exported as PNG at `scale` pixels per point | perceptual tolerance (≤ 0.2 % of pixels differing by more than 8/255 in a channel, and ≤ 8 of them in any square of 16 pixels a side) | How the previews look in VectorCraft's canvas, without its window |
| `vectorcraft-ui` | VectorCraft's window, of the same release, with the document `vectorcraft-cli` drew. The shot's menu path and the family's row are pressed through the accessibility tree, and the screenshot is cropped to the menus or the dialog | the same tolerance | Menus and dialogs, in a desktop without a screen ([VectorCraft's window in CI](#vectorcrafts-window-in-ci)) |
| `photo` | a person, of a real sew-out | presence + metadata record (machine, fabric, commit, checkpoint) | Never regenerated; listed with their sew-out report |

A shot declaration:

```toml
[[shot]]
id = "running-length"
kind = "stitch"
fixture = "docs/fixtures/wave.svg"   # planned by the engine for the reference machine, as `stitch plan` plans it
style = "simple"
scale = 8.0                          # pixels per millimetre
alt = "A wave in running stitch"
panels = [
  { caption = "1.5 mm", params = { running_stitch_length_mm = "1.5" } },
  { caption = "4 mm", params = { running_stitch_length_mm = "4" } },
]
```

Each panel sets its parameters on every element of the design and becomes one image, `running-length-1.png`
and so on. A test sheet is drawn as it is, so a shot of one has no panels. The plan may report only the codes the shot lists in `allow`, and a shot fails on any other:
a picture never shows a design that the engine changed without the page saying so. The `alt` text is
mandatory (accessibility), and the docs check fails on images without it.

A VectorCraft shot draws its document with VectorCraft's own commands, as a Level B scenario does
([compatibility gate](compatibility-gate.md)), in points:

```toml
[[shot]]
id = "vectorcraft-fill-dialog"
kind = "vectorcraft-ui"
plugins = ["stroke", "satin", "fill"]   # the modules VectorCraft installs
steps = [
  { cmd = "file.new", params = { width = 144, height = 144 } },
  { cmd = "shape.ellipse", params = { x = 12, y = 12, width = 120, height = 120 } },
]
menu = ["Effect", "Plug-ins"]           # the menu bar's title, then rows
dialog = "fill"                         # the picture shows that family's dialog
alt = "The StitchCraft Fill dialog in VectorCraft…"
```

`cargo xtask shots --vectorcraft` makes these shots. `vectorcraft-cli` runs the steps and ends the run
with a drawing, which runs the live effects. Each module is asked for its last error after it, and a shot
with one fails. VectorCraft keeps a module's last error only until its next success: a shot applies each
stitch family once at most, and one that applies a family twice is an error. A render shot's picture is
that drawing. A UI shot opens the document in VectorCraft's window and needs a `menu`, and with a
`dialog` the picture shows the dialog of that family's effect.

A render shot can keep its document as a file of the repository, `document`, for the pages that sew it:
the tutorial's patch is sewn from `docs/fixtures/round-patch.vectorcraft`, which VectorCraft draws from
the shot's steps. The generator writes the file while the committed one sews otherwise than the drawing,
and `--check` fails then. Another release may save the same drawing otherwise: the file stays while it
sews the same.

A page shows a shot between the lines `<!-- shot: running-length -->` and `<!-- /shot -->`. `cargo xtask
docs` writes the figure there. Without panels the figure is one image. With panels it is a table with a
column per panel, its caption on top and its image below it. A shot with `layout = "rows"` shows its
panels one under another instead, each caption in bold above an image as wide as the page: pictures side by
side shrink to share the page's width, and a wide picture's stitches then become too small to tell apart. Each image's alternative text is the shot's,
followed by the panel's caption. The page
cannot show an image under any other text, and a stale figure fails `--check` like a stale page.

### VectorCraft's window in CI

Spike M0.8 ran VectorCraft's window in CI, as the `vectorcraft-ui` kind needs, and drove it through its
accessibility tree. Both work, and M6.7 built that kind's generator this way.

`cargo xtask compat window` runs the window probe, `compat/vc-window`, against the app VectorCraft
releases: `bin/vectorcraft` from the Linux x86_64 archive of the stable release that
`compat/vectorcraft.toml` pins, checked against the SHA-256 recorded there. The probe's tests run in a
desktop of their own, with an X server (Xvfb) and a D-Bus session that nothing else uses:

- Mesa draws the window in software, through OpenGL (`WGPU_BACKEND=gl`), with its OpenGL renderer,
  llvmpipe. Left to choose, the app would take Mesa's Vulkan renderer wherever that one is installed, and
  the pixels would depend on the runner.
- The accessibility bus is told that a screen reader is on, since AccessKit, VectorCraft's accessibility
  layer, builds the tree only then. The flag stays in the session's memory (`GSETTINGS_BACKEND=memory`).
  Through GSettings it would reach the person's own settings, and GNOME would start a screen reader at
  their next login.

The compat workflow's `window` job runs the command daily, and on pull requests that touch the plug-in,
`compat/`, the tooling or a manifest. It keeps the screenshots, the trees and the app's logs.

**Screenshots: the same pixels, but for the frame timings.** The window opens at 1440 × 900 pixels,
drawn by llvmpipe, and a screenshot takes about 0.65 s. Above the status bar, screenshots of one window
match to the pixel. The screenshots of two starts of the app match there too, and so did two runs on
GitHub's runners. The status bar differs, since it shows the frame timings ("render 4.4 ms · ui 1.4 ms ·
24 fps"), which change with every frame. VectorCraft's own visual fixtures record where that text is, as
text that changes. So the generator crops each picture to the menu or dialog it shows. A picture of the
whole window leaves the status bar out. The tolerance in the table stays: the runner's Mesa comes with
Ubuntu's updates, and a new one may move an anti-aliased edge.

**xa11y: AT-SPI's press and clicks both work.** xa11y finds the app by its process. AT-SPI's press, as a
screen reader uses it, opens View and chooses its row Outline. Clicks at the middle of the bounds the tree
gives View and its row Preview, as a person makes them, bring the drawing back. The control channel sees
each change. A row's name is its label after the room for a check mark, and its shortcut
(`     Outline Ctrl+Y`). With no window manager, the window opens at the screen's corner, so the tree's
bounds are the screenshot's pixels.

**The generator: VectorCraft driven to each screen.** `tests/shots.rs` of the probe takes the UI shots
that `cargo xtask shots --vectorcraft` lists in a job file, in the same desktop. The tree is used as a
person would use the app. AT-SPI's press opens the menu bar's title and each row of the shot's path,
then the row of the family's effect, and the dialog is the largest element named after the plug-in. The
window opens the document it starts with. The control channel installs the modules, which a person does
with file dialogs. It also selects everything and sizes the window, and it reads the app's state. A
menus' picture is cropped to the title and the rows that opened, with 10 pixels around them for the
menus' frames. A dialog's picture is cropped to the dialog.

A dialog must show a row for each parameter of its module's manifest that shows at the defaults, under
the label the manifest gives it (`REQ-VC-012`). Its control must be the one VectorCraft gives the
parameter's type. A number gets a text field and a toggle a checkbox, and a choice gets a drop-down list.
At the window's size when it opens, 1440 × 900, the fill's and the satin's dialogs are too tall for it:
the stroke's is about 850 pixels tall, the fill's about 1,500 and the satin's about 2,050. From 0.14
VectorCraft then scrolls the rows, and the picture would show only some of them. The probe fails on a
dialog with a scroll bar. A dialog's picture is taken in a window 2,400 pixels tall instead, on a screen
of 3000 × 2500.

## Command-line examples

Pages show what a command prints by including it from `docs/src/user/reference/generated/`. The commands
are declared in `docs/examples.toml`, and `cargo xtask docs` runs each one with the `stitch` binary in an
empty directory. A declaration can copy files of the repository into that directory and run commands
that are not shown first. The output is the binary's own, standard output then standard error, with the
exit status when it is not 0. When a command prints something new, `--check` fails until the output is
written again. Pages show what the command prints today.

## What runs on every pull request

`.github/workflows/docs.yml` runs two jobs. The steps of the first, `check`:

- `cargo xtask docs --check` fails for a generated page that is stale or left over, and a page missing
  from `SUMMARY.md`. A broken relative link, `#anchor` or included file fails it, with ids made as mdBook
  makes them, smart punctuation included. A doc that names a `cargo xtask` subcommand that does not exist
  fails it too: we found that drift in VectorCraft's own `AGENTS.md`. Ids must be in their files, `REQ-…`
  and `DEV-…` alike, and an `SC-…` code registered once its milestone is done. An image must be one a shot
  writes, with alt text.
- `cargo xtask conformance --check` fails when requirements and cases disagree, or a fixture has no row
  in the fixtures index.
- `cargo xtask shots --check` fails for a declared image that is missing or has no alt text. It also
  regenerates each `stitch` image and compares it with the committed file, which must come out byte for
  byte the same. It checks VectorCraft's shots' declarations without running VectorCraft.
- `mdbook build docs` fails when the book does not build.
- `cargo test --doc` fails when a Rust snippet does not compile. Command-line examples are generated
  pages, which `docs --check` covers.
- `typos` fails on a misspelling (allow-list in `typos.toml`).

External links are checked nightly by lychee, not on pull requests.

The workflow's second job, `shots-ui`, runs `cargo xtask shots --vectorcraft --check` on Ubuntu, with
the desktop the window needs (`.github/actions/desktop`). It makes VectorCraft's pictures with the pinned
release and compares each with the committed file within the tolerance.

When a shot differs, `--check` writes the new image to `target/shots/` and the job keeps it as the
`stale-images` artifact, next to the committed one in the PR's diff. `shots-ui` keeps VectorCraft's as
`stale-vectorcraft-images`, with the documents, the window's screenshots and its records. An image under
`docs/src/images/generated/` that no shot declares is an error too, so renamed shots leave nothing
behind.

### Refreshing images

Changing a stitch algorithm *should* change its pictures. The PR author runs `cargo xtask shots` locally,
and `cargo xtask shots --vectorcraft` on Linux, with the packages `.github/actions/desktop` installs and
Rust's `wasm32-unknown-unknown` target, which downloads the pinned release. Or a maintainer adds the label
**`docs:refresh-shots`**, and `docs-refresh.yml` regenerates every shot, VectorCraft's first, and every
generated page. For a pull request from a branch of this repository, it pushes a commit to the branch and
removes the label. For a fork, the run keeps the files as an artifact, and its summary explains how to
apply them. When a picture can't be made, the others are still committed, and the job fails after them.
GitHub doesn't start workflows for a push made with a workflow's token, and the refresh then dispatches
`ci`, `docs` and `goldens` for the new commit itself. The refreshed images go through review like any
other change.

Pages show a shot with the shot's own alt text: the test-sheets reference is generated with one picture
per `stitch` shot of each sheet, so a new sheet gets its picture by declaring a shot.

## Publishing

- Push to `main` → build and publish to `/dev/` on GitHub Pages.
- Tag `vX.Y.Z` → publish to `/vX.Y/` and point `/latest/` at it. Older versions stay online.
- Every page shows the version it documents and links to the same page in `/dev/`.

## Writing rules

- Every user-visible change updates the docs in the same PR (PR template checkbox; reviewers enforce;
  generated pages update themselves).
- Plain language; define embroidery terms on first use and link the glossary.
- Show, then tell: a picture or a runnable example before the paragraph.
- Never document a parameter by hand: improve its doc comment in the registry instead.
- Explanations cite sources (papers, standards, public references) where they make claims.

## Translations (M12)

UI strings use Fluent message files generated from registry labels and help; the docs use
`mdbook-i18n-helpers` (gettext catalogues). Translations are community-maintained; a translated page shows
when its source changed after the translation, so stale translations are visible instead of misleading.
