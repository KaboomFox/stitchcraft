# Try StitchCraft in your browser

[Open the browser demo](demo/index.html) to use StitchCraft inside VectorCraft without installing either.
The page runs VectorCraft's editor in your browser with StitchCraft's plug-ins already installed.

It is an unofficial build, which the StitchCraft project makes from VectorCraft's source. VectorCraft's
makers neither publish it nor support it.

## What opens

A session without a document opens the starter patch, Alpine Night: a round mountain badge 50 mm wide on an
artboard of 60 mm. Each of its shapes has StitchCraft's Auto effect, and the editor shows the stitches.

## What to try

- Select a shape and change its Auto effect in the Appearance panel.
- Give a shape another stitch type from **Effect › Plug-ins**.
- Set the machine and the hoop in **Object › Plug-ins › StitchCraft Design Settings**.
- Write a machine file, in PES or DST, with **File › Export › Export As…**.

The **Starter & help** panel is at the lower right of the page. It opens another copy of the patch, or
downloads the patch as a VectorCraft template. Its **Documentation** link comes back to these pages.

Save your design to a file before you close the tab.

## Loading and limits

- The first visit downloads the whole editor, and a slow connection needs minutes for it.
- The editor draws with WebGPU where the browser has it and with WebGL2 otherwise, and `?webgl` at the end
  of the page's address makes it use WebGL2 when it doesn't appear.
- A large design can stop the page for a moment. The editor's web build runs a plug-in on the page's own
  thread, and the live preview draws large areas more simply to stay quick.
- Stroke to Satin and the 2 routing filters are not installed. Each does more work than VectorCraft lets a
  plug-in do unasked, and the panel offers them as files. Open one in VectorCraft and allow it when
  VectorCraft asks.

## In a pull request's preview

A pull request from a branch of the repository gets its own copy of these pages. The link above then opens
the same editor with that pull request's plug-ins and starter patch.
