# The browser demo

This folder builds and checks the browser demo: VectorCraft's web editor with StitchCraft's plug-ins, on
the project's Pages site. The design is in `docs/src/design/vectorcraft-integration.md`, under Browser
distribution, and the publishing in `docs/src/design/docs-pipeline.md`. The acceptance cases are in
[requirements.md](requirements.md).

| File | What it does |
|---|---|
| `build.py` | builds the editor from the release `compat/vectorcraft.toml` records, and each plug-in module |
| `site.py` | puts a pull request's preview together: its book with the demo folder |
| `start.mjs` | runs on the editor's page and chooses the loader: the app's own, or a preview's |
| `boot.mjs`, `loader.mjs`, `starter.mjs` | install the plug-ins, open the starter patch and show the help panel |
| `smoke.mjs`, `docs-smoke.mjs` | run the built demo in Chromium |
| `generate-template.mjs` | has the demo write the starter's template again from its SVG |
| `templates/` | the starter patch, Alpine Night, as an SVG and as VectorCraft's template |

## Build and check

The build needs Python 3.11 or newer, the Rust toolchain of `rust-toolchain.toml` with its
`wasm32-unknown-unknown` target, and the versions of Trunk, wasm-bindgen and wasm-opt that
`.github/actions/tools/action.yml` names, on the `PATH`. Trunk runs without the network and stops when
one of them is missing. From the repository's root:

```sh
python3 web/build.py                      # target/web/app and target/web/demo
node --test web/*.test.mjs
python3 -m unittest discover -s web -p 'test_*.py'
```

The build clones VectorCraft under `target/`, or uses the checkout `VECTORCRAFT_DIR` names when it is at
the recorded commit. It never changes VectorCraft's source. The editor's build takes most of an hour.

The browser checks need the site's layout, with the editor at `/app/`. Make the served folder new each
time: `cp -a` into a folder that is already there copies beside the page, and the check then finds a
folder's listing where the editor should be.

```sh
npm ci --prefix web && (cd web && npx playwright install chromium)
rm -rf target/serve && mkdir -p target/serve && cp -a target/web/app target/serve/app
python3 -m http.server 8766 --directory target/serve &
node web/smoke.mjs http://127.0.0.1:8766/app/
```

For a preview, build the book with the version of mdBook the tools action names, copy it beside the app,
and open the editor through it:

```sh
python3 web/site.py --base-path /stitchcraft/pr/123/
rm -rf target/serve/pr && mkdir -p target/serve/pr && cp -a target/site target/serve/pr/123
node web/docs-smoke.mjs http://127.0.0.1:8766/pr/123/
node web/smoke.mjs 'http://127.0.0.1:8766/app/?preview=123'
```

## The preview's comment

The workflow writes one comment on a pull request, which names the preview and shows the pictures
`smoke.mjs` took. Each picture's address ends with the commit, because GitHub keeps a copy of a picture by
its address. `.github/scripts/preview-comment.cjs` makes the comment from sections.

## The Pages site

GitHub Pages serves the `gh-pages` branch from its root (Settings › Pages › Deploy from a branch).
