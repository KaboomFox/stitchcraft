# Documentation pipeline

The bar: **documentation that cannot silently drift from the code.** Ink/Stitch's docs are generous
but hand-made — 1,341 committed images, a parameter dataset maintained separately from the code, no
docs CI, one unversioned site ([finding F7](inkstitch-analysis.md#f7--documentation-is-hand-made-and-unversioned)).
Ours are generated where they can be, tested where they cannot, and regenerated on every pull request.

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
| `stitch` | `stitchcraft-render` from a conformance fixture + parameters | **exact bytes** (rendering is deterministic) | Stitch-type illustrations, parameter "before/after" pairs, diagnostics examples |
| `vectorcraft-render` | `vectorcraft-cli run --in fixture --cmd plugin.install … --cmd effect.apply … --export x.png` | exact bytes, or tight tolerance if VectorCraft's renderer changes | How a preview looks in VectorCraft's canvas, headless |
| `vectorcraft-ui` | VectorCraft (pinned stable) driven over its control channel: open fixture, install plug-ins, select, open the dialog, set fields, `ui.screenshot`, crop | perceptual tolerance (≤ 0.2 % of pixels differing by more than 8/255 per channel) | Dialogs, menus, the workflow; runs under Xvfb with Mesa software rendering in a pinned container |
| `photo` | a person, of a real sew-out | presence + metadata record (machine, fabric, commit, checkpoint) | Never regenerated; listed with their sew-out report |

A shot declaration:

```toml
[[shot]]
id = "tatami-staggers-1-vs-4"
kind = "stitch"
fixture = "conformance/fixtures/fill/square-40mm.svg"
params = [{ staggers = 1 }, { staggers = 4 }]   # two panels side by side
render = { style = "realistic", width = 960, background = "#f4efe6" }
alt = "Two 40 mm tatami squares: with one stagger the needle points form straight furrows; with four they disappear."
```

The `alt` text is mandatory (accessibility), and the docs check fails on images without it.

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
| `examples` | a CLI example in the docs (`trycmd`, from M1.8) prints something different from what the page shows; a Rust snippet does not compile (`cargo test --doc`) |
| `shots` | `cargo xtask shots --check`: a declared image is missing, has no alt text, or regenerates differently from the committed file — byte-exact for `stitch` shots (generator in M2.7), within tolerance for `vectorcraft-ui` and `vectorcraft-render` shots (generators in M6.7; a separate job with Xvfb and Mesa) |
| `spelling` | `typos` finds a misspelling (allow-list in `typos.toml`) |
| `links` | an internal link is broken (`lychee --offline`); external links are checked nightly, not per PR |

When a shot differs, the job uploads an artifact with the new image, the old one and a diff image, and
writes a summary table to the PR's job summary.

### Refreshing images

Changing a stitch algorithm *should* change its pictures. The PR author runs `cargo xtask shots` locally,
or a maintainer adds the label **`docs:refresh-shots`**: `docs-refresh.yml` regenerates every shot and
pushes a commit to the PR branch (same-repository PRs; for forks the artifact contains the files and the
PR summary explains how to apply them). The refreshed images go through review like any other change.

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
