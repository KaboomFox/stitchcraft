# The before and after sheet

A change to the engine moves what the pictures sew, and the tests say so only where a case pins a number.
M13.21 raised the thread changes of a traced dog from 18 to 28, and a table made by hand was what showed it.
Every pull request's run now makes that table. The sheet is a page in the pull request's preview. It shows
the standard pictures as the pull request sews them beside main's, and marks the numbers that differ. A
reviewer opens it from the preview's comment and builds nothing.

The sheet reports. A number that got worse is marked and never fails the run, because a step may trade one
number for another on purpose, and the cases that pin a number already fail when it moves. The 2 things that
fail are in [the browser's checks](#the-browsers-checks).

## The standard pictures

The testkit's example `sheet` (`crates/stitchcraft-testkit/examples/sheet.rs`) holds the list:

- the owner's dog, traced from 2 photos in 6 and in 9 threads (`docs/fixtures/README.md` › The traced dogs),
  which are the documents the release cases measure
- the traced pear the docs show
- a lettered patch drawn in code, the word of TS-13B on a field with a border
- the digitized half of every test sheet that has one, which today are the 4 sheets of TS-13

Each picture is digitized automatically and planned for the reference machine with its stitches where it
draws its shapes, as the cases plan it. The example draws the plan with the renderer at 5 pixels to the
millimetre and writes the numbers of all the pictures to one file. A picture that does not plan is written
with what was said of it in the place of its numbers, and the others are still measured. On the sheet such
a picture is marked as worse than any number.

A new picture is one more entry in the example's list, and the scripts don't name a picture.

## The numbers

The example computes no number of its own. Each comes from the function a conformance case asserts with:

| Number | From |
|---|---|
| Stitches, thread changes and jumps | the plan's own counts, `StitchPlan::stats` |
| Jump threads left to cut, on a machine with one needle that cuts none | `stitchcraft_testkit::fabric::astray` |
| Stitches astray | `stitchcraft_testkit::fabric::astray` |
| Slivers, and the share of the design left bare | `stitchcraft_testkit::fabric::cover` |
| The thickest spot in fills, the area at 3 fills or more, and the most needle points per mm² | `stitchcraft_testkit::fabric::thickness` |

[Image to embroidery](image-to-embroidery.md#covering-the-fabric) defines each one, and
[the thickness](image-to-embroidery.md#how-thick-a-design-sews) beside them, so that thread taken away
is weighed against fabric shown. The example
also says of each number whether more of it is worse. More stitches are neither better nor worse, so a
change in them is shown and not marked.

## Main's side

A second build of main in every pull request's run would double the time the sheet takes. So main's own run
makes main's sheet, and the publisher puts it at `/sheet/` on the site, with its numbers in `sheet.json`
([publishing](docs-pipeline.md#publishing)). A pull request's run reads that file and copies main's
pictures into its own sheet, so the sheet in a preview stays as it was made when main moves on.

Main's published sheet is of one commit, which the page names. It is not always the commit the branch
started from, and `web/sheet.py` asks GitHub how the 2 differ:

- Main's sheet has commits the branch lacks: main moved on after the branch was made. Merging main into
  the branch removes them from the comparison.
- The branch has commits of main from after main's sheet was made: main's run had not ended, or had
  failed. Main's next run catches up.

In both cases the page and the comment say how many commits, and that some differences may be theirs.
When main hasn't published a sheet, the page shows the pull request's pictures alone and says so.

## The browser's checks

The run builds the real editor for the demo, and `web/sheet.mjs` uses it in Chromium for what only a
browser shows. It runs the pull request's plug-ins in the editor built in the same run.

**The same files as the command line.** The editor opens a design, gives every shape the Auto effect and
exports it to PES and to DST. It also saves the document, and `stitch plan` writes both files of the saved
document, under the name the editor gave its exporter. The script does this for 2 designs, the demo's starter drawing and a traced dog, and each pair of files
must be the same byte for byte. A pair that differs fails the run, which names the first byte
that differs and what each program has there. The workflow keeps both programs' files as an artifact.
VectorCraft stops an exporter that runs more instructions than it allows an export. The editor then
doesn't write a file, and there is nothing to compare. The sheet and the comment say so, and the run goes on. Any
other refusal fails the run.

**The panel's exports.** The demo's panel exports with StitchCraft's engine in a worker, which no budget
stops ([an export that finishes](vectorcraft-integration.md#an-export-that-finishes)). For the starter
drawing and each of the 4 traced dogs with the Auto effect, the script has the panel make the PES file,
the DST file and the picture of the stitches. It writes the document the panel took from the editor to a
file, and `stitch plan` makes the same 3 of it under the same name. Each must be the same byte for byte.
Here a refusal fails the run, as does a file that differs or an export that takes more than 60 seconds.
The sheet and the comment give the range of the times.

The `stitch` compared is built without link-time optimization, which saves minutes
([determinism](determinism.md) is why its files are those of the published build).

**The time of the Auto effect.** The script gives the traced dog the Auto effect 3 times, each
time in a new document, and exports each to PES. The traced dog is one group, and the effect on the group
digitizes every path in it. It records 2 times of each run, on the page's main thread:
from the call that applies the effect until the editor doesn't list an effect left to draw, and the
export. The sheet shows the middle one of the 3 for the pull request and for main.

**What a person sees.** After the last run the script takes a picture of the editor with the dog's
stitches drawn, and the sheet shows it beside main's.

### The time's limit

Chromium in CI has no graphics card and draws in software, and the editor runs a call on its next
frame. So the times include frames drawn far more slowly than on a person's computer, and they say more
about a change than about how fast the editor is for a visitor. The limit is several times the measured time for that reason, and a
run fails only when the middle time is above it. The limits are `LIMIT_MS` in `web/sheet.mjs`.

Measured on 2026-11-15 in pull request #274's run, on GitHub's Linux runner, the 3 runs took 5.9 s, 2.5 s
and 2.4 s from the call until the editor had drawn the dog. The first run also loads the plug-in's module.
The limit is 30 s, 5 times the slowest run, so a slower runner passes and a change that makes the effect
several times slower fails.

The export has no limit yet. VectorCraft 0.24.1 stopped the dog's PES and DST exports after 27 s for
the instruction budget it gives an export
([exporters](vectorcraft-integration.md#exporters-file--export-writes-pes-and-dst)). `stitch` plans the
same document. So the sheet reports that a traced dog doesn't export with the editor's File › Export, whose
files are compared for the starter drawing alone until it does. The dogs' files are compared as the panel
writes them.

## The time the sheet adds

The sheet's steps come after the demo's checks in the `web` workflow's build job. They build `stitch` and
the example, measure the pictures and run the editor once more.

In pull request #274's run they took 5 minutes 53 seconds of the job's 25 minutes: 2 minutes 13 seconds
to build `stitch`, 2 minutes 50 seconds in the browser and 50 seconds to build the example and measure
the 10 pictures. A minute of the browser's time was the dog's export stopped twice more, which the
script no longer tries after the first run.

## Not in this step

Every effect exercised in the browser, and pictures of each effect's dialog on the current VectorCraft,
are a later step ([roadmap](../plan/roadmap.md)).
