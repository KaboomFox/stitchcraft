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
install the light plug-ins on each page load. A failure stays visible. Heavy
filters are separate downloads: open their WASM files in VectorCraft and approve
its budget prompt. Large designs and heavy filters can pause the browser because
VectorCraft's web build runs them on the main thread. No server or thread headers
are required. The acceptance cases are in [requirements.md](requirements.md).

## GitHub Pages and previews

The Web workflow builds on pushes to `main` and trusted, same-repository PRs.
It publishes the app at `/app/`, previews at `/pr/<number>/`, and updates one PR
comment with the preview URL. Closing a PR removes its preview. Fork PRs only
build and test; their code does not receive a publishing token.

Enable Actions in the fork. In Settings → Pages, choose **Deploy from a branch**,
`gh-pages`, `/ (root)`. The existing documentation remains at `/dev/`; the app
publisher preserves it and all other previews. The site root remains the docs
landing page. No custom domain is required.

Publishing writes only generated files to `gh-pages`. Push conflicts are retried
from the latest branch so simultaneous documentation and app deployments preserve
each other's files. The PR preview uses the PR head revision, recorded in the
comment, rather than the default branch.
