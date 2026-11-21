# Glossary

| Term | Meaning |
|---|---|
| **Automatic digitizing** | StitchCraft choosing each shape's stitches from its geometry, for a shape that has no settings ([automatic digitizing](../stitches/automatic.md)) |
| **Base layer** | One open fill sewn first under a whole design digitized automatically, in place of each fill's own underlay |
| **Bean stitch** | A running stitch where each stitch is sewn back and forth (3, 5, … passes) for a bold line |
| **Bird's nest** | A tangle of thread under the fabric, usually from bobbin or tension problems |
| **Cap** | The most fills of thread automatic digitizing lets a spot sew, `auto_thick_cap_fills` ([how thick it sews](../stitches/automatic.md#how-thick-it-sews)) |
| **Collapse length** | Moves shorter than this are sewn instead of jumped (default 3 mm) |
| **Colour block** | A run of stitches in one thread colour; blocks are separated by colour changes |
| **Comfort zone** | The part of the hoop a machine profile may recommend keeping designs inside, with a warning for larger ones. No built-in profile has one yet |
| **Contour fill** | A fill whose rows follow the shape's outline inward |
| **Density** | How close rows or zigzags are (row spacing / zigzag spacing) |
| **Element** | One thing that sews: a shape with its stitch type and settings, or a part of a shape that automatic digitizing sews as one |
| **Expand** | Turning a design digitized automatically into ordinary shapes, each with its own stitch settings ([expand an automatic design](../how-to/expand-an-automatic-design.md)) |
| **Fill (of thread)** | A measure of how thick thread lies on a spot: one fill is what a tatami fill lays at its default row spacing ([embroidery basics](../explanation/embroidery-basics.md#how-thick-thread-lies)) |
| **Guide** | A path named `stitch:guide` whose line the rows of the fills it crosses follow ([guide the rows](../how-to/guide-the-rows.md)) |
| **Hoop** | The frame that holds fabric; its sewable field is a machine profile's hard limit |
| **Jump** | A move between stitches without sewing; the thread spans the gap until trimmed |
| **Lock stitch (tie-in, tie-off)** | Tiny stitches that secure the thread at the start or end of a section |
| **Mark** | A shape named `stitch:keep`, or an area `stitch plan --keep` gives: the parts of a picture inside it keep their own thread ([keep small parts](../how-to/keep-small-parts.md)) |
| **Needle penetration** | One drop of the needle; consecutive penetrations make a stitch |
| **PES / PEC** | Brother's design format; the PEC block inside a PES file holds the stitches |
| **Profile (machine profile)** | StitchCraft's description of a machine with one hoop, named by both, such as `brother-pe800-5x7`: hoop, comfort zone, file format, stitch limits, trims, palette ([machine profiles](profiles.md)) |
| **Pull compensation** | Widening shapes so they come out the drawn size after thread tension pulls the fabric |
| **Push** | Fabric moving outward across the stitch direction; columns grow at their ends |
| **Rail** | One of the two edges of a satin column |
| **Row spacing** | Distance between rows of a fill |
| **Rung** | A short line across a satin's rails that fixes which points correspond |
| **Running stitch** | A line of single stitches |
| **Satin column** | Stitches swinging between two rails; glossy bands for borders and lettering |
| **Stabilizer** | Backing material that keeps fabric from distorting while stitching |
| **Stagger** | The offset of needle points between neighbouring fill rows; *staggers* is how many rows before the pattern repeats |
| **Stitch plan** | The full ordered list of stitches, jumps, trims and colour changes for a design |
| **Style** | A named set of automatic digitizing's settings, `logo` or `photo`, given with `--style` ([automatic rules](auto-rules.md)) |
| **Tatami** | The standard fill: parallel rows with staggered needle points |
| **Trace** | Turning a picture into shapes of flat colour, as VectorCraft's Image Trace does ([from a picture to a machine file](../tutorials/picture-to-machine-file.md)) |
| **Travel** | Running stitches that move the needle inside a shape, hidden by later stitching |
| **Trim** | A thread cut performed by the machine |
| **Underlay** | A light foundation layer sewn before the top stitches |
| **Zigzag spacing** | Peak-to-peak distance of a satin's zigzag |
