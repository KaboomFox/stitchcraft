# The browser demo's acceptance cases

Each case names its check. `web.yml` runs them all before anything is published.

| Case | Check |
|---|---|
| The editor starts from `/app/` and from a preview, and asks for no file that is missing | `smoke.mjs` |
| Each light plug-in installs through the page's calls, and a heavy one is offered as a file | `smoke.mjs` |
| A download that fails, or a module VectorCraft refuses, is never reported as installed | `loader.test.mjs` |
| A session without a document opens the plain drawing, with no StitchCraft effect on a shape | `smoke.mjs`, `starter.test.mjs` |
| The panel's button gives every shape the Auto effect, and a second press doesn't add one | `smoke.mjs`, `starter.test.mjs` |
| The finished example opens in a new document, with the Auto effect on every shape | `smoke.mjs`, `starter.test.mjs` |
| A document, a file being opened or a recovery dialog stops the starter from opening by itself | `starter.test.mjs` |
| The drawing after the button and the finished example export to DST with stitches, and a filled rectangle to PES too | `smoke.mjs` |
| The book's link opens the editor, a preview's with that preview's loader, and the panel links back | `docs-smoke.mjs` |
| Only a pull request's number names a preview to the editor | `start.test.mjs` |
| A preview contains its book and its plug-ins, and not the editor | `test_site.py` |
| The demo's release is recorded beside the gate's and is never older | `test_site.py` |
| A change to a script, the drawing or the finished example names another build | `test_site.py` |
| Publishing one folder keeps every other, also when 2 runs push at the same moment | `test_publish.py` |
| Closing a pull request removes its preview alone, and the branch is one commit afterwards | `test_publish.py` |
| The comment names the book and the demo, and shows the pictures of the build | `preview-comment.test.mjs` |
| The sheet marks a number only when more of it is worse and it rose, and ranks the worst first | `test_sheet.py` |
| A picture that stops planning is marked, and the others are still shown | `test_sheet.py` |
| The sheet names main's commit, and says when it and the branch differ by more than the branch's commits | `test_sheet.py` |
| Without a sheet of main's, a pull request's sheet shows its own pictures and says so | `test_sheet.py` |
| The editor in Chromium and `stitch` write the same PES and DST files of a document, and a difference names its first byte | `sheet.mjs`, `parity.test.mjs` |
| The Auto effect's time fails the run only above a limit that is set | `sheet.mjs`, `parity.test.mjs` |
| The panel's Export PES, Export DST and Show the real stitches make a file and a picture of the drawing, and say how long the engine worked | `smoke.mjs`, `exports.test.mjs` |
| The exporter's module, run by the page, writes a traced photo that VectorCraft's export is stopped on | `engine.test.mjs` |
| The panel's PES, DST and picture of the starter drawing and the 4 traced dogs are `stitch`'s, byte for byte | `sheet.mjs`, `test_sheet.py` |
| The comment's sheet section is the build's lines with the sheet's address | `preview-comment.test.mjs` |
