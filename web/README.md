# StitchCraft in VectorCraft, in the browser

The web build packages VectorCraft's existing Rust/WASM editor with StitchCraft's
WASM plug-ins. Artwork stays in the browser. Open or draw a design, apply a stitch
effect through Effect → Plug-ins, and use File → Export to download PES or DST.
Save the editable design before closing the tab.

## Build and check

Install Python 3.11 or newer, the Rust toolchain in `rust-toolchain.toml`, its
`wasm32-unknown-unknown` target, and Trunk 0.22.1. Then run from the repository root:

```sh
python3 web/build.py
node --test web/*.test.mjs
python3 -m http.server 8766 --directory target/web
# In a second terminal, after installing web/package-lock.json dependencies:
cd web && npm ci && npx playwright install chromium && cd ..
node web/smoke.mjs http://127.0.0.1:8766/
```

`web/config.json` pins the VectorCraft source revision. The build clones it under
`target/`, or accepts an existing matching checkout through `VECTORCRAFT_DIR`.
It does not modify VectorCraft's source. Trunk uses relative asset URLs, so the
same files work at `/app/` and `/pr/123/demo/`. `?webgl` forces WebGL2 if WebGPU fails.
The pinned editor is about 97 MB before transfer compression, so the first load can take time.

The loader uses VectorCraft's documented `window.vectorcraft.execute` API to
install the light plug-ins on each page load. The loader displays an error when a plug-in fails. Heavy
filters are separate downloads: open their WASM files in VectorCraft and approve
its budget prompt. Large designs and heavy filters can pause the browser because
VectorCraft's web build runs them on the main thread. The app runs on an ordinary static file server. The acceptance cases are in [requirements.md](requirements.md).

## GitHub Pages and previews

The Docs and demo workflow builds the editor and the book from the same revision.
A PR preview opens its documentation at `/pr/<number>/`. The demo link opens
`/pr/<number>/demo/`, which contains that PR's app and screenshots. Its Documentation
link returns to the same preview. Closing the PR removes the whole preview folder.
Fork PRs build and test with read-only credentials.

The permanent docs at `/dev/` link to the published app at `/app/` through a small
launcher page. Versioned books use that app too. The existing docs workflow publishes
the permanent book and site landing page. Main pushes in the Docs and demo workflow
publish the app.

Enable Actions and set Pages to **Deploy from a branch**, `gh-pages`, `/ (root)`.
The publisher preserves other previews and permanent pages when updating one folder.
It retries push conflicts from the latest branch.

Install the pinned mdBook version from `.github/actions/tools/action.yml` to build
combined previews locally. Run `python3 web/site.py --base-path /stitchcraft/pr/1/`
after the web build, then serve `target/site`. `node web/docs-smoke.mjs URL` follows
the docs link into the demo and back. `node web/smoke.mjs URL/demo/` tests embroidery.

## Starter patch and preview pictures

An empty session opens Alpine Night, a 50 mm mountain patch on a 60 mm artboard.
The native `.vctemplate` contains editable paths with StitchCraft Auto effects.
The Starter & help panel opens another copy in a new tab or downloads the template.
Existing documents and recovery dialogs prevent automatic opening.

The original artwork is `web/templates/alpine-night.svg`, licensed with this repository.
To regenerate the native template from it, serve the built app and run
`node web/generate-template.mjs http://127.0.0.1:8766/?webgl`.

The browser check captures the starter patch and the StitchCraft Auto dialog.
It also captures Design Settings, then checks that the patch exports stitches.
The preview comment embeds these PNG images from its demo’s `screenshots/` folder.
Each image URL includes the commit SHA to refresh GitHub's image cache.
