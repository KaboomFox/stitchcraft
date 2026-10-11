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
| `conformance/requirements.toml` + latest report | requirement status pages | `cargo xtask docs` |
| `conformance/inkstitch-params.toml` + registry | [compatibility contract](inkstitch-compat-contract.md) with status | `cargo xtask docs` |
| Compatibility runs | supported VectorCraft versions | `cargo xtask compat report` |

Generated pages are committed (reviewers see user-facing changes in the diff) and marked with a header
comment; `cargo xtask docs --check` regenerates into a temporary directory and fails if anything
differs.

## Images: four kinds, each reproducible

Every image in the docs is declared in `docs/shots.toml` with an id, a kind and how to make it. Pages
reference `images/generated/<id>.png`. An image without a declaration fails the docs check.

| Kind | Made by | Compared by | Notes |
|---|---|---|---|
| `stitch` | `stitchcraft-render` ([rendering](rendering.md)) from a test sheet, or from an SVG design the engine plans | **exact bytes** (rendering is deterministic) | Stitch-type illustrations, parameter comparisons, diagnostics examples |
| `vectorcraft-render` | `vectorcraft-cli run --in fixture --cmd plugin.install … --cmd effect.apply … --export x.png` | exact bytes, or tight tolerance if VectorCraft's renderer changes | How a preview looks in VectorCraft's canvas, headless |
| `vectorcraft-ui` | VectorCraft (pinned stable) driven over its control channel: open fixture, install plug-ins, select, open the dialog, set fields, `ui.screenshot`, crop | perceptual tolerance (≤ 0.2 % of pixels differing by more than 8/255 per channel) | Dialogs, menus, the workflow; runs under Xvfb with Mesa software rendering in a pinned container |
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
and so on. The plan may report only the codes the shot lists in `allow`, and a shot fails on any other:
a picture never shows a design that the engine changed without the page saying so. The `alt` text is
mandatory (accessibility), and the docs check fails on images without it.

A page shows a shot between the lines `<!-- shot: running-length -->` and `<!-- /shot -->`. `cargo xtask
docs` writes the figure there. Without panels the figure is one image. With panels it is a table with a
column per panel, its caption on top and its image below it. A shot with `layout = "rows"` shows its
panels one under another instead, each caption in bold above an image as wide as the page: pictures side by
side shrink to share the page's width, and a wide picture's stitches then become too small to tell apart. Each image's alternative text is the shot's,
followed by the panel's caption. The page
cannot show an image under any other text, and a stale figure fails `--check` like a stale page.

## Command-line examples

Pages show what a command prints by including it from `docs/src/user/reference/generated/`. The commands
are declared in `docs/examples.toml`, and `cargo xtask docs` runs each one with the `stitch` binary in an
empty directory. A declaration can copy files of the repository into that directory and run commands
that are not shown first. The output is the binary's own, standard output then standard error, with the
exit status when it is not 0. When a command prints something new, `--check` fails until the output is
written again. Pages show what the command prints today.

The `vectorcraft-ui` kind depends on running VectorCraft's window in CI; spike **M0.8** proves it
(Xvfb + Mesa llvmpipe through wgpu's GL backend). Until it lands, UI images are limited to
`vectorcraft-render` shots, and VectorCraft's own approach — semantic headless frame tests — covers the
dialogs' contents.

## What runs on every pull request

`.github/workflows/docs.yml`:

| Job | Fails when |
|---|---|
| `book` | `mdbook build` fails, or any page is missing from `SUMMARY.md` |
| `check` | generated pages are stale; a relative link or `#anchor` is broken; a doc mentions a `cargo xtask` subcommand that does not exist (the drift we found in VectorCraft's own `AGENTS.md`); a `REQ-…` or `SC-…` id does not exist; an image lacks a declaration or alt text |
| `examples` | a Rust snippet does not compile (`cargo test --doc`). Command-line examples are generated pages, which `check` covers |
| `shots` | `cargo xtask shots --check`: a declared image is missing, has no alt text, or regenerates differently from the committed file — byte-exact for `stitch` shots, within tolerance for `vectorcraft-ui` and `vectorcraft-render` shots (generators in M6.7; a separate job with Xvfb and Mesa) |
| `spelling` | `typos` finds a misspelling (allow-list in `typos.toml`) |
| `links` | an internal link is broken (`lychee --offline`); external links are checked nightly, not per PR |

When a shot differs, `--check` writes the new image to `target/shots/` and the job keeps it as the
`stale-images` artifact, next to the committed one in the PR's diff. An image under
`docs/src/images/generated/` that no shot declares is an error too, so renamed shots leave nothing
behind.

### Refreshing images

Changing a stitch algorithm *should* change its pictures. The PR author runs `cargo xtask shots` locally,
or a maintainer adds the label **`docs:refresh-shots`**: `docs-refresh.yml` regenerates every shot and
every generated page, pushes a commit to the PR branch and removes the label (same-repository PRs; for
forks the run keeps the files as an artifact and its summary explains how to apply them). GitHub starts
no workflows for a push made with a workflow's token, so the refresh then dispatches `ci`, `docs` and
`goldens` for the new commit. The refreshed images go through review like any other change.

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
