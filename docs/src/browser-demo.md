# Try StitchCraft in your browser

[Open the browser demo](demo/index.html) to use StitchCraft inside VectorCraft without installing either.
The page runs VectorCraft's editor in your browser with StitchCraft's plug-ins already installed.

It is an unofficial build, which the StitchCraft project makes from VectorCraft's source. VectorCraft's
makers neither publish it nor support it.

## What opens

A session without a document opens Alpine Night as a plain drawing: a round mountain badge 50 mm wide on an
artboard of 60 mm. None of its shapes has a stitch effect yet. Turning them into stitches is yours to do.

The **Start here** panel is open at the lower right of the page. A session that has a document of its own
keeps it, and the panel starts closed.

## From the drawing to a machine file

1. Press **Turn the drawing into stitches** in the panel. Each shape gets StitchCraft's Auto effect, and
   the editor draws the stitches.
2. Select a shape and change its effect in the Appearance panel.
3. Set the machine and the hoop in **Object › Plug-ins › StitchCraft Design Settings**.
4. Write a machine file, in PES or DST, with **File › Export › Export As…**.

The button does what the menus do. To do the first step by hand, choose **Select › All**, then
**Effect › Plug-ins › StitchCraft Auto**, and confirm the dialog. The other entries of that menu give a
shape another stitch type.

The button works on the document in front, a drawing of your own included. A shape that has a StitchCraft
effect keeps it, and pressing the button again doesn't change the drawing.

## The finished example

**Open the finished example** in the panel opens the same badge with the Auto effect on every shape, in a
new document. **Download the finished example as a template** saves it as a VectorCraft template. The
panel's **Documentation** link comes back to these pages.

Save your design to a file before you close the tab. The plain drawing comes from an SVG file: choose
VectorCraft's own format when you save, which keeps each shape's effect.

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
the same editor with that pull request's plug-ins and starter drawing.
