# Browser acceptance cases

The web bundle must satisfy these cases before it is deployed:

- VectorCraft loads from both the site root and a nested PR directory, without requests to missing assets.
- Every bundled light StitchCraft plug-in installs through the page command API. A failed install is visible and offers a reload.
- A rectangle with a StitchCraft fill effect exports as nonempty PES and DST files in the browser.
- A failed module download never reports the embroidery tools as ready.
- Heavy filters are available as downloads. Opening one uses VectorCraft's own approval dialog.
- Publishing one PR preserves other PRs and the documentation. Closing it removes only that preview.

`node --test web/loader.test.mjs` checks the loader's success and failure paths.
`node web/smoke.mjs URL` checks the real WASM app, plug-ins and embroidery exports.

- An empty session opens the native starter template with its embroidery effects.
- Existing documents, pending opens and recovery dialogs prevent automatic opening.
- Opening the starter again creates a new document.
- The starter exports a DST file containing stitches.
- The preview comment embeds actual screenshots of the starter and both settings dialogs.
- Publication reads the credential includes used by checkout v7.

- Permanent documentation links to the published browser demo.
- A PR preview opens its own documentation with its matching demo nested beneath it.
- The demo’s Documentation link returns to the same PR preview.
- Replacing a preview removes stale app files from its former root.
- Updating the permanent docs or landing page preserves PR previews.
