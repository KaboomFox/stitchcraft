//! The diagnostic codes of automatic digitizing, `SC-x09xx`: a part of the registry ([`super`]) in a file of
//! its own, since automatic digitizing adds a code with most of its steps. An entry is added here, in the order
//! of its code, and nowhere else.

/// Gives `$before`, the entries before automatic digitizing's, and these to `later_codes`, which adds the
/// entries after them and gives all of them to the macro `$registry`.
macro_rules! auto_codes {
    ($registry:ident, { $($before:tt)* }) => {
        later_codes! { $registry, {
        $($before)*

        /// `--auto`, or StitchCraft's Auto effect in VectorCraft, digitized the design's shapes that have no
        /// settings: StitchCraft chose each one's stitches by its width, measured to the micrometre. An area is a
        /// tatami fill; a part of it narrower than the design's narrowest satin stroke
        /// (`min_satin_stroke_width_mm`, 1 mm unless the design sets it) everywhere is a running stitch along its
        /// centre line, and a part nowhere wider than the widest satin column sews satin columns along its centre
        /// lines. A line no wider than that narrowest stroke is a running stitch, a line up to the widest satin
        /// column a satin column as wide as the line, and a wider line a tatami fill of the area it paints. A
        /// running stitch is tripled from the narrowest triple run. The widest satin column is 7 mm and the
        /// narrowest triple run 0.5 mm, unless the shape sets others (`auto_max_satin_width_mm`,
        /// `auto_min_triple_run_width_mm`). A line whose every part a satin column cannot follow is filled too
        /// (`SC-I0902`), and counted as a fill. An area whose every part runs is counted as a running stitch, one
        /// none of which is filled and one of whose parts sews a satin column as satin columns, and any other as a
        /// fill. A narrower part is filled too when it is compact, a blob short for its width, or uneven, a
        /// branching patch whose width varies. Each shape says which rule chose its stitches (`SC-I0912`).
        ///
        /// The message counts the shapes that sew by the stitches they sew as: an area that the areas sewn after
        /// it cover whole sews nothing (`SC-I0904`), and isn't counted. To sew a shape another way, give it
        /// settings: in VectorCraft, apply StitchCraft's Fill, Stroke or Satin effect to it. A shape with settings
        /// is not digitized automatically.
        AutoDigitized = "SC-I0901", Info, "Shapes digitized automatically";

        /// `--auto` sews a line up to the widest satin column as a satin column along its path, and a part of an
        /// area nowhere wider than that as satin columns along its centre lines, but it can't make a satin column
        /// of this one. The widest satin column is 7 mm unless the shape sets another (`auto_max_satin_width_mm`).
        /// A column cannot follow a part of a line that turns more tightly than the line is wide, so the
        /// column's edges would fold over themselves, or that is shorter than a CSS pixel, or that is a dot the
        /// line's round or square cap draws. The area it paints is filled instead, as a wider line is. A part of
        /// an area doesn't make columns where the triangles round one of its centre lines don't make a column
        /// with 2 sides, or where one of its columns, drawn with its rails and rungs, would be read back
        /// otherwise, or when a column would sew a stitch between its rails longer than the widest satin column,
        /// as at the corners of a square ring: such a stitch lies loose. The part stays in the area's fill.
        ///
        /// The message says which. To sew it as satin columns, draw the line with gentler turns or narrower, or
        /// the part with smoother edges, or draw the columns with 2 rails each and give them settings.
        AutoSatinFilled = "SC-I0902", Info, "Satin column can't be made; filled";

        /// `--auto` fills the area a line paints when the line is wider than the widest satin column, 7 mm unless
        /// it sets another (`auto_max_satin_width_mm`), or when a satin column cannot follow it, and the area this
        /// line paints would reach more than 10 metres from the design's origin, beyond what machine files can
        /// record. It is left out. A stroke that wide is usually a mistake in the file.
        ///
        /// The message gives the line's width. Check the line's stroke width.
        AutoAreaTooLarge = "SC-W0903", Warning, "Line too wide to fill; left out";

        /// `--auto` makes each automatic area reach its overlap, 1 mm unless it sets another
        /// (`auto_overlap_mm`), under the shapes sewn after it, from where it touches them. The thread pulls
        /// stitches in from the edges of their shape, and without the overlap fabric shows between shapes that
        /// only meet. Where an area sewn after it covers part of it, as the details on an area digitized whole
        /// under them do (SC-I0908), the covered part is cut away, its overlap inside the covering area's edge,
        /// and sewn once. The running stitch or the satin column along a narrow part of an area covers that part
        /// as an area does: the areas of other shapes sewn before it are cut away under it, and when the shapes
        /// sewn after it cover the part whole it sews nothing. A satin column or a line digitized from a stroke
        /// and sewn after an area goes over it, and the area is sewn whole under it.
        ///
        /// The message counts the areas that reach under others and those cut away. An area that the areas drawn
        /// or sewn after it cover whole sews nothing, and the message names it, as it does a narrow part's line.
        /// It also
        /// names an area whose neighbours need more work to find than the budget allows it, which neither reaches
        /// under them nor is cut away, and a satin column whose area needs more, which the areas before it don't
        /// reach under, and which keeps its place in paint order beside the shapes near it. To sew a shape as it
        /// is drawn, give it settings: in VectorCraft, apply StitchCraft's Fill, Stroke or Satin effect to it.
        AutoUnder = "SC-I0904", Info, "Areas reach under the shapes sewn after them";

        /// `--auto` takes an automatic area smaller than its smallest area, 1 mm² unless it sets another
        /// (`auto_min_area_mm2`), or a part of one that small, for a speck the trace left. It is too small to
        /// sew, and would sew a knot. It is merged into the automatic area it shares the longest edge with,
        /// which may be the rest of its own area, and no hole shows where it was. A speck that shares no edge
        /// with an automatic area is left out.
        ///
        /// The message gives the speck's area and says where it went: into a neighbour, back to the rest of its
        /// area, or nowhere, as when finding its neighbours needs more work than the budget allows it. To sew it
        /// as it is, give it settings: in VectorCraft, apply StitchCraft's Fill, Stroke or Satin effect to it. A
        /// larger Noise in VectorCraft's Image Trace leaves such specks out of the trace.
        AutoSpeck = "SC-I0905", Info, "Speck merged into a neighbour";

        /// `--auto` takes the lowest shape of the design for the picture's background when it is an automatic
        /// area, a shape on another path than its own is drawn on it, and it covers the whole design to within
        /// 0.5 mm of each side, its holes aside, and the notches aside that the shapes drawn on it fill where
        /// they touch the picture's edge. It is left out, unless it switches `auto_ignore_background` off, but
        /// for what shows of it inside the picture once the layers are flattened (SC-I0908): its parts that come
        /// no nearer than 0.5 mm to the design's sides, such as a white disc on a badge traced with its colours
        /// stacked, which sew, and which the message counts. A background is usually the fabric's own colour, and
        /// sewing it doubles the stitches and stiffens the design. A background that needs more work to find than
        /// the budget allows is sewn, and the message says so.
        ///
        /// To sew it, give it settings: in VectorCraft, apply StitchCraft's Fill, Stroke or Satin effect to it.
        /// Ignore White in VectorCraft's Image Trace leaves a white background out of the trace.
        AutoBackground = "SC-W0906", Warning, "Background left out";

        /// `--auto` sews the automatic elements in an order of its own, and `--sort-colours` every element.
        /// Shapes that overlap as drawn keep their paint order, the lower one first, and a satin column or a
        /// line that borders a fill sews after it, over its edge. Within those rules each thread sews as long
        /// as an element of it is free to sew, and the next element is the nearest. Elements with settings keep
        /// their order among themselves unless `--sort-colours` is given, and nothing moves across a stop.
        ///
        /// The message says how many thread changes the order has, and how many paint order has. It also names
        /// an element whose neighbours need more work to find than the budget allows, which keeps its place in
        /// paint order as an element with a stop after it does, and a line whose paint can't be found, which
        /// keeps its place beside the shapes near it. SC-I0904 names a satin column whose paint can't be found.
        /// And it names the elements left in paint order when choosing the order needs more work than the budget
        /// allows. To keep shapes in their paint order among themselves, give them settings and leave
        /// `--sort-colours` off: in VectorCraft, apply StitchCraft's Fill, Stroke or Satin effect to each.
        AutoOrder = "SC-I0907", Info, "Elements sewn in an order of their own";

        /// `--auto` flattens the layers of a traced picture before it digitizes them. Each automatic area sews
        /// only what shows of it: the automatic areas drawn over it are taken from it, a hairline that rounding
        /// leaves along their edges shows nothing, and one they cover whole sews nothing (SC-I0904). An automatic
        /// area that is filled (wider than a satin column somewhere, 7 mm unless `auto_max_satin_width_mm` says
        /// another, or compact, or uneven: `SC-I0912`) is digitized whole under its details, the automatic areas
        /// that fill its holes: its stitches are chosen with those holes closed, it sews before its details, and it
        /// is cut away under them, its overlap inside their edges. So a picture traced with its colours stacked
        /// sews as one traced with its colours abutting, and a broad area with details on it is one fill, not
        /// narrow columns between the details. An area that sews as satin columns, such as a letter or an
        /// outline, keeps its holes, and the shapes in them sew beside it.
        ///
        /// The message counts the areas that sew only what shows of them and those digitized whole under their
        /// details. It also names an area whose layers need more work to find than the budget allows: one whose
        /// part that shows can't be found is sewn as it is drawn, in its place, or left out whole when it is the
        /// background; one whose holes can't be found keeps them open; one that can't be moved keeps its place;
        /// and one whose region can't be given is sewn as it is drawn. To sew a shape as it is drawn, give it
        /// settings: in VectorCraft, apply StitchCraft's Fill, Stroke or Satin effect to it.
        AutoLayers = "SC-I0908", Info, "Layers flattened";

        /// `--auto` sews one base layer under the whole design before anything else, so that fabric of any
        /// colour shows less where 2 shapes leave a slit between them. Its shape is what the automatic shapes
        /// paint together, with the slits narrower than a thread closed, offset by -0.4 mm, which is inwards,
        /// unless the design sets another offset (`auto_base_offset_mm`). It is an open fill of stitches 3 mm
        /// long in 3 directions (`auto_base_stitch_length_mm`), in the thread that paints the largest area, and
        /// every stitch of it is underlay. The fills on it sew no underlay of their own. A piece of it that has
        /// no room for a row of those stitches is left out, as under a satin column that stands alone, which
        /// sews the underlay of its width.
        ///
        /// The message gives its area, how many pieces it has and its thread. A design that is nowhere wide
        /// enough for one has none, and the message says so. A design whose shapes all sew as lines and
        /// columns has none either, and nothing is said. To sew the design without a base layer, turn
        /// `auto_base_layer` off: each fill then sews the underlay of its settings (SC-I0911).
        ///
        /// Where thin shapes meet, the top thread can leave a sliver bare, and the base layer's thread shows
        /// there in place of the fabric. The check that StitchCraft's test pictures pass allows a sliver of up
        /// to 0.4 mm² with the base layer under it (`auto_base_sliver_mm2`), and up to 0.5 % of the design left
        /// to the base layer in all (`auto_base_bare_percent`). A larger gap fails it.
        AutoBase = "SC-I0909", Info, "Base layer sewn under the design";

        /// `--auto` could not find the shape of the design's base layer within the work budget, so the design
        /// sews without one, and every fill sews the underlay of its settings. On dark fabric the slits between
        /// shapes then show the fabric. Raise the work budget, or simplify the picture: fewer colours and less
        /// detail in VectorCraft's Image Trace give fewer shapes.
        AutoBaseLeftOut = "SC-W0910", Warning, "Base layer left out";

        /// `--auto` sews no base layer, because the design's lowest automatic shape turns `auto_base_layer`
        /// off. Each fill sews the underlay of its settings, and nothing lies under a place that the top thread
        /// leaves bare between 2 shapes. On fabric of another colour than the design, the fabric may show
        /// there. With the base layer, its thread shows in such a place (SC-I0909). This is information and no
        /// warning, because the design sews as its settings ask. To sew the base layer, turn `auto_base_layer`
        /// on, or leave it unset.
        AutoBaseOff = "SC-I0911", Info, "Base layer turned off";

        /// `--auto` chooses the stitches of each part of a shape from the part's geometry, by the first rule
        /// of one table that the part matches. The message names that rule for each part, with the measures
        /// that decided it: the part's width along its centre lines, its slenderness (its area over its width
        /// squared), how much its width varies along those lines, and how many of them it has. A shape of many
        /// parts gives the measures of its first 3 and counts the rest by rule.
        ///
        /// To sew the shape another way, set `auto_stitch` on it to `run`, `satin` or `fill`: in VectorCraft,
        /// in the dialog of StitchCraft's Auto effect. For the whole design, lean the rules with `auto_prefer`,
        /// turn `auto_fill_uneven` off, or set a bound itself with `stitch plan --set`.
        AutoRule = "SC-I0912", Info, "Rule that chose a shape's stitches";

        /// A design digitized with `--auto` puts thread on thread. Each area reaches under the shapes sewn
        /// after it, and a base layer lies under all of them. An outline lies on a fill's edge. Where too much
        /// thread lies over one spot, a needle deflects and the thread shreds or breaks. Planning measures the
        /// design in 2 ways, in the window of 1 mm² round each spot. The first is the thread, in fills: the
        /// length of thread over the spot, as a share of what a fill with rows 0.4 mm apart lays. The second is
        /// the needle points in a square millimetre, the locks left out. The warning is given when a place of
        /// 1 mm² or more has 3 fills or more (`auto_thick_warn_fills`), or 15 needle points or more to a
        /// square millimetre (`auto_thick_warn_points`). The message gives the most of each measure and where,
        /// from the design's centre, the shapes whose thread is stacked there, and the area at each limit
        /// and at the failing limits, 6 fills (`auto_thick_fail_fills`) and 30 needle points
        /// (`auto_thick_fail_points`).
        ///
        /// The design is written all the same. The limits are starting values for a 75/11 needle and
        /// 40-weight polyester thread, from the public advice of machine and software makers, and no sew-out
        /// has confirmed them yet: sew a test piece of the thickest part before the whole design.
        /// `stitch preview --style thickness` draws where the thread is thick. Where the thickest spot is above
        /// the cap, 4 fills (`auto_thick_cap_fills`), the message says so: no more thread there may go. A design too large to measure,
        /// or one whose measuring needs more work than the budget allows, gets this warning too, and its
        /// message says that the design was not measured.
        AutoThick = "SC-W0913", Warning, "Design sews thick";

        /// Between 2 elements of one thread that `--auto` made, the needle sews a run under the shapes sewn
        /// later where they cover a way from one to the next, and leaves no jump thread to cut. Finding what
        /// covers, and the ways, has the work budget once for the whole design. This design needed more, so
        /// from the element named on its moves are as they were before: sewn on within the collapse length,
        /// and jumps beyond it. The design sews, with more jump threads than it need have. To have the runs,
        /// plan it with a larger work budget, or digitize fewer shapes: trace the picture in fewer colours,
        /// or raise `auto_min_area_mm2`.
        AutoRunsLeft = "SC-I0914", Info, "Jumps not looked at for a run under later shapes";

        /// `--auto` takes a part of an automatic area narrower than the design's narrowest satin stroke
        /// everywhere, a thin line such as soft shading and the strands of fur trace into, and merges it into
        /// the automatic area it shares the longest edge with, which may be the rest of its own area. Sewn on
        /// its own, such a line is a running stitch between 2 fills, and the fills beside it end their rows on
        /// it. Merged, it sews as part of the fill beside it. An area whose every part is such a line, merged,
        /// sews nothing of its own. A thin part that shares no edge with an automatic area sews as a line.
        ///
        /// A line that touches no other piece of its colour is a part of the picture of its own, such as a
        /// twig or the rim of a petal, and merged it would take its colour away. Such lines merge only while
        /// the picture keeps the likeness `auto_min_likeness` sets, and the others sew as lines, which this
        /// says too.
        ///
        /// The message gives the line's area and says where it went. To sew such lines as running stitches,
        /// turn `auto_fill_uneven` off, as the `logo` style does, or give the shape settings. To merge more of
        /// the lines that are parts of their own, lower `auto_min_likeness`.
        AutoLineMerged = "SC-I0915", Info, "Thin line merged into a neighbour";

        /// A mark, a shape named `stitch:keep` or an area that `stitch plan --keep` gives, holds parts of the
        /// design's automatic areas wholly inside it. `--auto` sews each of them by its width, where it would
        /// merge a speck or a thin line into a neighbour: a speck narrower than the narrowest satin stroke is
        /// a short running stitch, and a thin line a running stitch along its middle, each in its own thread.
        /// Nothing else changes, though the rest of the design may sew in another order.
        ///
        /// The message names the mark, how many parts lie wholly inside it, and how many of them sew that
        /// would have merged, as specks and as thin lines. A mark round a part of a traced photo takes in
        /// the specks of the trace's noise too, each a short stitch and maybe a jump thread: to merge them,
        /// draw the mark tighter round the part.
        AutoKept = "SC-I0916", Info, "Parts kept by a mark";

        /// A mark keeps nothing, and the design sews as it would without it. The mark encloses no area, or no
        /// part of an automatic area lies wholly inside it, or the design has no automatic shape. A part
        /// that crosses the mark's edge is decided as if there were no mark, and the message says which of
        /// these it is. Draw the mark round the whole part, a little larger than it, or remove it.
        AutoMarkUnused = "SC-W0917", Warning, "Mark not used";

        /// A guide, a line named `stitch:guide` or one that `stitch plan --guide` gives, crosses filled parts
        /// of the design's automatic areas. `--auto` sews each part it crosses as a guided fill whose rows
        /// follow the line, the parts of one area that follow one guide as one element. A part that 2 guides
        /// cross follows the one drawn last. Parts sewn along their middles, as running stitches or satin
        /// columns, keep their stitches. The overlaps, the base layer and the cap stay as they are, though
        /// the rest of the design may sew in another order.
        ///
        /// The message names the guide, how many filled parts of how many areas follow it, and how many
        /// parts sewn along their middles it crosses. A guide that crosses only a corner of a large part
        /// steers all of it: look at the preview, and draw the guide again where the rows go wrong.
        AutoGuided = "SC-I0918", Info, "Parts that follow a guide";

        /// A guide steers nothing, and the design sews as it would without it. It crosses no filled part of
        /// an automatic area, or its line has no length, or the design has no automatic shape, and the
        /// message says which of these it is. Draw the guide as a line across the filled shapes whose rows
        /// should follow it, or remove it.
        AutoGuideUnused = "SC-W0919", Warning, "Guide not used";

        /// A path or a compound path has StitchCraft's Auto effect of its own, inside a group or a layer
        /// whose Auto effect makes everything in it automatic. The nearest effects say how a path sews, so it
        /// sews by its own effect's settings alone, and none of the group's reach it. VectorCraft stores an
        /// effect with every setting, its defaults too, so the path's own effect can't leave a setting to
        /// the group.
        ///
        /// The message names the path, as the Layers panel shows it, and each setting the group changes from
        /// its default that the path's own effect sets otherwise, with both values. To sew the path by the
        /// group's settings, remove its own Auto effect. To keep it, set those settings on it too.
        AutoOwnReplaces = "SC-W0920", Warning, "Own Auto settings replace the group's";

        /// The outlines of automatic areas were smoothed before they were digitized, as their setting
        /// `auto_smooth_mm` asks. Each point moved no farther than the setting, the corners stayed, an edge 2
        /// areas share was smoothed once, and no ring's area changed by more than a tenth. Smoothing cuts short
        /// stitches at ragged edges, but it also changes which parts sew as satin columns.
        ///
        /// The message gives how many areas were smoothed, within which setting, and how many outline points
        /// they had and have, and names any area whose smoothing needed more work than the budget allows,
        /// which keeps its outline. To sew the outlines as drawn, set `auto_smooth_mm` to 0.
        AutoSmoothed = "SC-I0921", Info, "Outlines smoothed";
        } }
    };
}
