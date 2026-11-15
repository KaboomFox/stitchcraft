# The browser demo's acceptance cases

Each case names its check. `web.yml` runs them all before anything is published.

| Case | Check |
|---|---|
| The editor starts from `/app/` and from a preview, and asks for no file that is missing | `smoke.mjs` |
| Each light plug-in installs through the page's calls, and a heavy one is offered as a file | `smoke.mjs` |
| A download that fails, or a module VectorCraft refuses, is never reported as installed | `loader.test.mjs` |
| A session without a document opens the starter patch | `smoke.mjs`, `starter.test.mjs` |
| A document, a file being opened or a recovery dialog stops the starter from opening by itself | `starter.test.mjs` |
| The starter patch and a filled rectangle export to PES and DST with stitches | `smoke.mjs` |
| The book's link opens the editor, a preview's with that preview's loader, and the panel links back | `docs-smoke.mjs` |
| Only a pull request's number names a preview to the editor | `start.test.mjs` |
| A preview contains its book and its plug-ins, and not the editor | `test_site.py` |
| The demo's release is recorded beside the gate's and is never older | `test_site.py` |
| Publishing one folder keeps every other, also when 2 runs push at the same moment | `test_publish.py` |
| Closing a pull request removes its preview alone, and the branch is one commit afterwards | `test_publish.py` |
| The comment names the book and the demo, and shows the pictures of the build | `preview-comment.test.mjs` |
