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
node --test web/loader.test.mjs
python3 -m http.server 8766 --directory target/web
# In a second terminal, after installing web/package-lock.json dependencies:
cd web && npm ci && npx playwright install chromium && cd ..
node web/smoke.mjs http://127.0.0.1:8766/
```

`web/config.json` pins the VectorCraft source revision. The build clones it under
`target/`, or accepts an existing matching checkout through `VECTORCRAFT_DIR`.
It does not modify VectorCraft's source. Trunk uses relative asset URLs, so the
same files work at `/app/` and `/pr/123/`. `?webgl` forces WebGL2 if WebGPU fails.
The pinned editor is about 97 MB before transfer compression, so the first load can take time.

The loader uses VectorCraft's documented `window.vectorcraft.execute` API to
install the light plug-ins on each page load. The loader displays an error when a plug-in fails. Heavy
filters are separate downloads: open their WASM files in VectorCraft and approve
its budget prompt. Large designs and heavy filters can pause the browser because
VectorCraft's web build runs them on the main thread. The app runs on an ordinary static file server. The acceptance cases are in [requirements.md](requirements.md).

## GitHub Pages and previews

The Web workflow builds on pushes to `main` and trusted, same-repository PRs.
It publishes the app at `/app/` and previews at `/pr/<number>/`.
The workflow adds the preview URL to a PR comment. Closing a PR removes its preview. Fork PRs build and test with read-only credentials.

Enable Actions in the fork. In Settings → Pages, choose **Deploy from a branch**,
`gh-pages`, `/ (root)`. The existing documentation remains at `/dev/`. The app
publisher preserves it and all other previews. The site root remains the docs
landing page. The site uses the repository’s github.io address.

Publishing writes only generated files to `gh-pages`. Push conflicts are retried
from the latest branch so simultaneous documentation and app deployments preserve
each other's files. The PR preview uses the PR head revision, recorded in the
comment.

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
These PNGs are published under `screenshots/` and embedded in the preview comment.
Each image URL includes the commit SHA to refresh GitHub's image cache.
