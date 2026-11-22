//! The settings of automatic digitizing's rules (`REQ-AUTO-020`): StitchCraft's own parameters, registered
//! as any other (`docs/src/design/params.md`), which no stitch type reads.
//!
//! Each rule goes by a value (`docs/src/design/image-to-embroidery.md` › Settings), in 3 tiers. A shape says
//! which stitch it sews as, or leaves the choice to the rules ([`STITCH`]). A design leans the rules towards
//! satin columns or towards fills, fills its uneven parts or not, and gives its fills one angle, beside
//! the overlap, the pull compensation, the background and the base layer. The bounds themselves are the
//! advanced settings ([`ADVANCED`]): the widest satin column, the narrowest triple run, the least slenderness,
//! the most a width varies, the farthest a column reaches from its line, the narrowest piece of a fill, the smallest area, and the 2 limits of what the top
//! thread may leave to the base layer. MC-8 confirms the values on the machine or changes them, and a picture
//! can need others, so each is a parameter of the shape it digitizes, read when the shape is digitized
//! ([`AutoParams`]). A shape that sets none takes the defaults, as every shape does with `--auto`, and the
//! requirements give the defaults as their numbers. VectorCraft's Auto effect (M13.7) sets them from its
//! dialog, for the object it is on and the paths inside the group or the layer it is on, but for the advanced
//! ones, which `stitch plan --set` sets on every automatic shape.
//!
//! They apply to no stitch type ([`AUTOMATIC`]): an automatic element reads them before it is sewn as any,
//! and the elements it becomes don't, so no stitch family's dialog shows them. The registry's audit keeps
//! them StitchCraft's own, since Ink/Stitch digitizes nothing automatically.

use stitchcraft_params::{AUTOMATIC, Origin, Stability, params};

use crate::design::DesignSettings;

/// The key of the stitch a shape asks for.
pub const STITCH: &str = "auto_stitch";
/// The key of the choice between satin columns and fills.
pub const PREFER: &str = "auto_prefer";
/// The key of the switch that fills uneven parts.
pub const FILL_UNEVEN: &str = "auto_fill_uneven";
/// The key of the switch that sews an outline round each fill.
pub const FILL_OUTLINE: &str = "auto_fill_outline";
/// The key of the switch that sews the details last.
pub const DETAILS_LAST: &str = "auto_details_last";
/// The key of the fills' angle.
pub const FILL_ANGLE: &str = "auto_fill_angle";
/// The key of the widest satin column.
pub const MAX_SATIN_WIDTH: &str = "auto_max_satin_width_mm";
/// The key of the widest split satin column.
pub const MAX_SPLIT_SATIN_WIDTH: &str = "auto_max_split_satin_width_mm";
/// The key of the narrowest triple run.
pub const MIN_TRIPLE_RUN_WIDTH: &str = "auto_min_triple_run_width_mm";
/// The key of the least slenderness of a column.
pub const MIN_SLENDERNESS: &str = "auto_min_slenderness";
/// The key of the most a column's width varies.
pub const MAX_UNEVENNESS: &str = "auto_max_unevenness";
/// The key of the least a thin line's width varies for it to merge into its neighbour.
pub const MIN_LINE_UNEVENNESS: &str = "auto_min_line_unevenness";
/// The key of the least likeness of the picture that merging thin lines keeps.
pub const MIN_LIKENESS: &str = "auto_min_likeness";
/// The key of the farthest a column reaches from its centre line.
pub const MAX_REACH: &str = "auto_max_reach";
/// The key of the narrowest piece that stays in a fill.
pub const MIN_FILL_WIDTH: &str = "auto_min_fill_width_mm";
/// The key of the overlap.
pub const OVERLAP: &str = "auto_overlap_mm";
/// The key of the overlap under a part's satin column.
pub const COLUMN_OVERLAP: &str = "auto_column_overlap_mm";
/// The key of the smallest area.
pub const MIN_AREA: &str = "auto_min_area_mm2";
/// The key of the choice of where a satin column's crowded needle points move in.
pub const SHORT_STITCHES: &str = "auto_short_stitches";
/// The value of [`SHORT_STITCHES`] that moves a column's crowded points in as it is made (`REQ-AUTO-173`).
pub const CROWDED: &str = "crowded";
/// The key of the switch that leaves the background out.
pub const IGNORE_BACKGROUND: &str = "auto_ignore_background";

/// The section of the settings that no dialog shows: the bounds of the rules, set on the command line
/// (`stitch plan --set`) or in a document. The Auto effect's dialog leaves its parameters out.
pub const ADVANCED: &str = "Advanced";

/// The longest satin stitch a machine sews well, in millimetres, and so the widest satin column a shape can
/// ask for: longer satin stitches lie loose and snag. It is the machine's limit, the same for every machine
/// profile so far, so a design may set its widest satin column lower and never higher. MC-8 confirms it.
pub const LONGEST_SATIN_STITCH: f64 = 7.0;

/// The widest a person may set a split satin column, in millimetres (`auto_max_split_satin_width_mm`). The
/// trade sews satin up to about 10 mm wide with split stitches, and the reference profile's longest stitch is
/// 12 mm: a column any wider is a fill.
pub const WIDEST_SPLIT_SATIN: f64 = 12.0;

params! {
    /// Automatic digitizing: the settings of the rules that choose the stitches of a shape without settings
    /// from its geometry. Each rule goes by the settings of the shape it digitizes, and a shape that sets none
    /// takes the defaults, as every shape does with `--auto`. The machine checkpoint MC-8 confirms the
    /// defaults or changes them. The settings of the last section, Advanced, are the rules' bounds: no dialog
    /// shows them, and `stitch plan --set` sets them.
    pub struct AutoParams for AUTOMATIC;

    "Stitches by shape" {
        /// The stitch the shape sews as. Left at `auto`, each part of it sews by the first rule that it
        /// matches, from its geometry. `run` sews every part as a running stitch along its centre lines,
        /// `satin` sews each part no wider than the widest satin column as satin columns along them, and
        /// `fill` fills the shape whole. A part that can't sew as asked goes by the rules: a speck, a part
        /// without a centre line, and for `satin` a part too narrow or too wide for a column.
        auto_stitch: Choice = "auto", label "Stitch",
            options ["auto" => "Chosen by the rules", "run" => "Running stitch", "satin" => "Satin columns", "fill" => "Tatami fill"],
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// Which way the rules lean where a part could sew as satin columns or as a fill. `satin` moves 3
        /// bounds together by half, so that more parts sew as columns: the least slenderness is divided by
        /// 1.5, and the most a width varies and the narrowest piece that stays in a fill are multiplied by
        /// it. `fill` moves them the other way, and `balanced` leaves them as they are set.
        auto_prefer: Choice = "balanced", label "Satin or fill",
            options ["satin" => "Prefer satin columns", "balanced" => "Balanced", "fill" => "Prefer fills"],
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// Whether a part of several centre lines whose width varies along them is filled. On, such a part
        /// is a tatami fill: the ragged patches of a traced photo sew flat, and a ragged thin line merges
        /// into the area beside it. Off, it sews a satin column along each line, and a thin line runs along
        /// its middle, as a drawing made of strokes of changing width may want.
        auto_fill_uneven: Toggle = "true", label "Fill uneven parts",
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// The angle of the rows of every fill that automatic digitizing makes, as a fill's `angle` gives
        /// it. One angle for the whole design lets a small filled part lie flat beside the fills round it.
        auto_fill_angle: Angle = "0", label "Fill angle",
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// Whether each fill that automatic digitizing makes sews an outline: a running stitch round its
        /// edge, after its rows, in its own thread. The outline draws the edge of a small shape as one
        /// line, where the ends of its rows would leave its points blunt. It sews only where the edge
        /// shows: not where a shape sewn after the fill covers the edge. The design goes by the setting of
        /// its lowest automatic shape.
        auto_fill_outline: Toggle = "true", label "Outline round fills",
            origin Origin::StitchCraft, stability Stability::Experimental;
    }

    "Neighbours" {
        /// How far an area reaches under each shape sewn after it that it touches, and how far inside the
        /// edge of an area, or of a part sewn as a line or a column, that is sewn after it and covers it the
        /// area is cut away. The thread pulls stitches in
        /// from the edges of their shape, and without the overlap fabric shows between shapes that only
        /// meet. A satin column reaches as far past a rail that a shape sewn after it runs along.
        auto_overlap_mm: Length = "1", label "Overlap", range (0.0, 2.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// How far a satin column sews past a rail with no shape sewn after the column beside it: its pull
        /// compensation there. The thread pulls a column in from its rails, and a column that sews this far
        /// past them is as wide as drawn. Along a rail where a shape sewn after the column runs for more
        /// than half the rail's length, the column sews past the rail by the overlap instead.
        auto_pull_compensation_mm: Length = "0.4", label "Pull compensation", range (0.0, 2.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// Whether the lowest shape is left out when it is the picture's background: an area under the
        /// other shapes that covers the whole design. A background is usually the fabric's own colour, and
        /// sewing it doubles the stitches and stiffens the design. What shows of it inside the picture sews
        /// still, as a white disc on a badge traced with its colours stacked does. Off, all of it that shows
        /// sews, as of any area.
        auto_ignore_background: Toggle = "true", label "Leave out the background",
            origin Origin::StitchCraft, stability Stability::Experimental;
    }

    "Base layer" {
        /// Whether one base layer sews first under the whole design: an open fill under what the
        /// automatic shapes paint together, every stitch of it underlay. On fabric of another
        /// colour than the design, thread of one colour then lies under a slit between 2 shapes. The fills
        /// on it sew no underlay of their own. Off, each fill sews the underlay of its settings. The design
        /// goes by the settings its lowest automatic shape has for the base layer.
        auto_base_layer: Toggle = "true", label "Base layer",
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// How long the base layer's stitches are. Its rows lie this length times 0.866 apart, so that
        /// its needle points make triangles with sides this long, as the stitches of Ink/Stitch's
        /// Knockdown Fill do. Shorter stitches hide more of the fabric and sew longer.
        auto_base_stitch_length_mm: Length = "3", label "Base layer's stitch length", range (1.0, 6.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// Whether the base layer keeps the holes of the design's outer shape. Off, it sews across them.
        auto_base_keep_holes: Toggle = "true", label "Base layer keeps holes",
            origin Origin::StitchCraft, stability Stability::Experimental;
    }

    "Advanced" {
        /// The widest a line or a part of an area sews as a satin column, and the longest stitch such a
        /// column sews between its rails: satin stitches any longer lie loose. A line up to this wide is a
        /// satin column as wide as the line, and a wider one fills the area it paints. A part of an area
        /// wider than this somewhere is filled. As a traced picture's layers are flattened, an area that
        /// is filled is filled whole under the areas that fill its holes. The value is the machine's
        /// longest satin stitch, so a shape may set it lower and never higher.
        auto_max_satin_width_mm: Length = "7", label "Widest satin column",
            range (DesignSettings::MIN_SATIN_STROKE_WIDTH.get(), LONGEST_SATIN_STITCH),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// The widest a part of an area, or a line, sews as one satin column whose stitches are split, when it
        /// is wider than the widest satin column. Such a part has one centre line, and its width varies by no
        /// more than `auto_max_unevenness`: a border, a ring or a bar. Its stitches split into pieces no
        /// longer than the widest satin column, at random places, so that their needle holes don't line up.
        /// At its default, the widest satin column, no part is split, and every part that wide is filled.
        /// The trade sews satin up to about 10 mm wide so. A split column keeps satin's sheen where a fill
        /// reads flat, but it sews thicker than a fill and with about half as much thread again. Off by
        /// default until a sew-out confirms it.
        auto_max_split_satin_width_mm: Length = "7", label "Widest split satin column",
            range (LONGEST_SATIN_STITCH, WIDEST_SPLIT_SATIN),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// The narrowest running stitch sewn as a triple run, each stitch sewn 3 times (bean stitch 1). A
        /// line no wider than the design's narrowest satin stroke runs, and from this width it sews bolder
        /// than one stitch, which is as fine as the thread. A narrow part of an area is as wide as the
        /// circles that fit inside it along its centre line.
        auto_min_triple_run_width_mm: Length = "0.5", label "Narrowest triple run", range (0.0, 10.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// How long for its width a part is, at the least, to sew as satin columns: its area divided by
        /// its width squared. A part less slender is compact, a blob with no way to run, and is filled. A
        /// rectangle is as slender as it is long for its width, and a disc measures 0.8.
        auto_min_slenderness: Number = "3", label "Least slenderness of a column", range (0.0, 20.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// How much the width of a part of several centre lines varies along them, at the most, for it to
        /// sew as satin columns: the standard deviation of its width divided by its width. A part more
        /// uneven is filled, unless `auto_fill_uneven` is off. A part of one centre line isn't held to it:
        /// a leaf and a tapering stroke sew well as one column.
        auto_max_unevenness: Number = "0.3", label "Most a column's width varies", range (0.0, 5.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// How much the width of a thin line varies along it, at the least, for it to merge into the shape
        /// beside it (`auto_fill_uneven`): the standard deviation of its width divided by its width. A thin
        /// line is a part narrower than the narrowest satin stroke everywhere. The ragged strands that soft
        /// shading and fur trace into vary by 0.09 to 0.46, and merge. A line drawn with an even pen varies
        /// by less, and sews as a line.
        auto_min_line_unevenness: Number = "0.05", label "Least a merged line's width varies", range (0.0, 5.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// The least likeness of the picture that merging thin lines keeps. A part is a piece of one colour of
        /// 1 mm² or more in which a disc a thread across fits, and the likeness is the mean over the colours
        /// of the share of their parts kept. A thin line that touches no other piece of its colour is a part
        /// of its own, and merged into a neighbour of another colour it is a part lost: a twig, the rim of a
        /// petal, a shadow. Such lines merge, those of the colours with the most parts and the smallest first,
        /// while the picture keeps this likeness, and the others sew as lines. At 0 every thin line merges,
        /// and at 1 none that is a part does. Keeping lines adds running stitches and jump threads, and it
        /// changes the order the blocks sew in: the cap (`auto_thick_cap_fills`) keeps the spots that order
        /// piles up within it. Lower it for fewer jump threads where the detail matters less.
        auto_min_likeness: Number = "0.85", label "Least likeness kept when lines merge", range (0.0, 1.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// How far a part reaches from one of its centre lines on both sides of the line, at the most, for
        /// it to sew as satin columns: the distance, divided by the radius of the widest circle that fits
        /// inside the part centred on its lines. A column's edges lie as far from its line as that circle
        /// reaches, so it measures 1, on one side or the other. A star measures 1.6 to 2.6: its centre line
        /// runs from one point to another, and its other points lie far from the line on both sides. One
        /// column along that line would sew them as bumps, so a part that reaches farther is filled. A
        /// sharp corner reaches far on one side only, and stays a column.
        auto_max_reach: Number = "1.4", label "Farthest a column reaches from its line", range (1.0, 20.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// The narrowest piece of an area that stays in its fill. A piece narrower than this leaves the
        /// fill of an area wider than a satin column and sews by the rules, as a line or as columns. A
        /// piece as wide or wider stays, so that a wing 6 mm wide isn't cut from the fill of its body.
        auto_min_fill_width_mm: Length = "4", label "Narrowest piece of a fill",
            range (DesignSettings::MIN_SATIN_STROKE_WIDTH.get(), LONGEST_SATIN_STITCH),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// The smallest area sewn. An area, or a part of one, smaller than this is a speck, which would sew a
        /// knot: it is merged into the area beside it that it shares the longest edge with, or left out
        /// when it shares none.
        auto_min_area_mm2: Area = "1", label "Smallest area", range (0.0, 10.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// How far outside the design's outer shape the base layer's edge lies: inside it when negative, so
        /// that no base stitch shows beside a shape once the thread has pulled the shape's edge in. By
        /// default it lies as far inside as a satin column's pull compensation reaches outside.
        auto_base_offset_mm: Length = "-0.4", label "Base layer's offset", range (-3.0, 3.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// Which way the base layer's rows run. `across` sews one layer, its rows across those of the
        /// fills on it, `auto_fill_angle` turned by 90°: the underlay a fill is usually given, once under
        /// the whole design. `lattice` sews 3 layers 60° apart, as Ink/Stitch's Knockdown Fill does. That
        /// holds down the pile of a towel or a fleece, and lays 3 times the thread under the design.
        auto_base_rows: Choice = "across", label "Base layer's rows",
            options ["across" => "Across the fills' rows", "lattice" => "In 3 directions"],
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// The largest gap in the top thread that is left to the base layer: a sliver. Where thin shapes
        /// meet, the top thread can leave a place this small bare, and the base layer's thread shows there
        /// in place of the fabric. Nothing sews differently by this value. It is a limit of the check of a
        /// design's cover: a larger gap fails the check, and so does a gap with no base layer under it.
        auto_base_sliver_mm2: Area = "0.4", label "Largest sliver", range (0.0, 2.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// The most of the design's area that the top thread may leave bare over the base layer, the
        /// slivers and the places narrower than a thread together. Nothing sews differently by this value.
        /// It is the other limit of the check of a design's cover. A design without a base layer is held to
        /// 0.15 % and to no gap at all.
        auto_base_bare_percent: Percent = "0.5", label "Most left to the base layer", range (0.0, 5.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// How many fills of thread over one spot make the design sew thick. A fill is what rows 0.25 mm
        /// apart lay, the default spacing of a fill's rows. Nothing sews differently by this value: planning
        /// warns of a place of 1 mm² or more with as much thread, which a person should look at. One layer of satin
        /// over a fill lays 2 to 3 fills and sews well, and 2 dense layers on a fill, 4, risk broken needles.
        /// It is a starting value for a 75/11 needle and 40-weight polyester thread, which a sew-out has to
        /// confirm. The design goes by the settings its
        /// lowest automatic shape has for the 4 limits of its thickness.
        auto_thick_warn_fills: Number = "4", label "Thread that warns, in fills", range (0.5, 50.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// How many fills of thread over one spot fail: the warning gives the area with as much, and the
        /// heat map draws it in its last colour. A value under the warning's limit reads as that limit.
        auto_thick_fail_fills: Number = "6", label "Thread that fails, in fills", range (0.5, 50.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// How many needle points to a square millimetre make the design sew thick, the locks left out.
        /// Planning warns of a place of 1 mm² or more with as many. The rows of a fill end at its edge, 5 to
        /// a square millimetre there, so this is what the edges of 3 fills on each other make. A starting
        /// value, as the limit of thread.
        auto_thick_warn_points: Number = "15", label "Needle points that warn, per mm²", range (0.5, 200.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// How many needle points to a square millimetre fail, what the edges of 6 fills on each other
        /// make: the warning gives the area with as many. A value under the warning's limit reads as that
        /// limit.
        auto_thick_fail_points: Number = "30", label "Needle points that fail, per mm²", range (0.5, 200.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// The most fills of thread a spot of a millimetre sews: what planning aims at. Where a spot sews
        /// more once the design is planned, a last pass and then the cap take thread there that other
        /// stitches cover: travel, the base layer, a shape's thread under later shapes, and a shape's own top
        /// stitches where they are doubled. The cap leaves a satin column's top stitches as they are, adds no
        /// jump thread, and what shows keeps its colour. A note says what is left above the cap, and the
        /// warning comes only at its own limit. A starting value for a 75/11 needle and 40-weight polyester
        /// thread, which a sew-out has to confirm. The design goes by the setting of its lowest automatic
        /// shape.
        auto_thick_cap_fills: Number = "3.25", label "Most thread on a spot, in fills", range (1.0, 50.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// Where a satin column's crowded needle points move in. `above_limit`, the default: each stitch
        /// runs from rail to rail as it is made, a bend keeps its spacing on the inside and opens its
        /// outside by a quarter at the most (`bend_open_percent` 25), and only where a spot would sew above
        /// the warning's limit does the cap pull crowded points in. `crowded`: as before, the outside keeps
        /// the spacing, and every point closer than 0.35 mm to the one before it on its rail moves in, by a
        /// quarter of its stitch and then by half. A shape that sets its own short-stitch settings keeps
        /// them either way.
        auto_short_stitches: Choice = "above_limit", label "Short stitches",
            options ["above_limit" => "Only above the limit", "crowded" => "Where points crowd"],
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// Whether a detail sews after the larger areas round it, where that costs no thread change: an
        /// automatic area that the shapes round it close in on, smaller than an area it meets, as an eye is in
        /// the fur round it. Sewn last, the detail keeps the later fills' rows off its edge. Off by default:
        /// a fill that moves for a detail can wait for a later batch of its thread, which adds jump threads
        /// to cut by hand, and on traced photos it shows no more of the picture. Turn it on where an eye or a
        /// mark must keep its edge clear. The design goes by the setting of its lowest automatic shape.
        auto_details_last: Toggle = "false", label "Details sew last",
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// How far a point of an area's outline may move when the outline is smoothed, before anything is
        /// digitized. A traced outline wobbles by less than a thread is wide, and the wobble sews short
        /// stitches and ragged edges. Corners stay, an edge 2 areas share is smoothed once so that no gap
        /// opens, and a ring's area changes by a tenth at most. 0, the default, sews every outline as drawn.
        /// 0.2 mm, half a thread, cut short stitches on every traced picture measured, but it also changes
        /// which parts sew as satin columns, and not always for the better.
        auto_smooth_mm: Length = "0", label "Smooth outlines", range (0.0, 1.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// How far an area stays under a satin column that digitizing makes of a part of another shape and
        /// sews after it: the area is cut away that far inside the column's edge, and reaches that far under
        /// it where they meet. Left empty, the default, it is the overlap, as everywhere else. A column's
        /// thread pulls its stitches in by about its pull compensation, 0.4 mm, so less than that may show
        /// fabric between the fill and the column. Less overlap sews thinner where a column edges a fill, but
        /// only a sew-out shows how little is enough. A value above the overlap reads as the overlap. Under
        /// an area, a running line or a satin column drawn as a stroke, lettering's too, the overlap stays.
        auto_column_overlap_mm: OptionalLengthWithZero = "", label "Overlap under a column", range (0.0, 2.0),
            origin Origin::StitchCraft, stability Stability::Experimental;
    }
}

#[cfg(test)]
impl AutoParams {
    /// The settings of a shape that sets none of the rules', each its default.
    pub(crate) fn defaults() -> AutoParams {
        AutoParams::from_set(&stitchcraft_params::ParamSet::new()).unwrap().params
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_key_names_a_setting_in_the_order_they_are_declared() {
        let keys: Vec<&str> = AutoParams::SPECS.iter().map(|spec| spec.key).collect();
        let shape = [STITCH, PREFER, FILL_UNEVEN, FILL_ANGLE, FILL_OUTLINE];
        let neighbours = [OVERLAP, "auto_pull_compensation_mm", IGNORE_BACKGROUND];
        let base = ["auto_base_layer", "auto_base_stitch_length_mm", "auto_base_keep_holes"];
        let bounds = [
            MAX_SATIN_WIDTH,
            MAX_SPLIT_SATIN_WIDTH,
            MIN_TRIPLE_RUN_WIDTH,
            MIN_SLENDERNESS,
            MAX_UNEVENNESS,
            MIN_LINE_UNEVENNESS,
            MIN_LIKENESS,
            MAX_REACH,
            MIN_FILL_WIDTH,
            MIN_AREA,
        ];
        let cover = ["auto_base_offset_mm", "auto_base_rows", "auto_base_sliver_mm2", "auto_base_bare_percent"];
        let thick = ["auto_thick_warn_fills", "auto_thick_fail_fills", "auto_thick_warn_points", "auto_thick_fail_points", "auto_thick_cap_fills", SHORT_STITCHES];
        assert_eq!(
            keys,
            [&shape[..], &neighbours[..], &base[..], &bounds[..], &cover[..], &thick[..], &[DETAILS_LAST, "auto_smooth_mm", COLUMN_OVERLAP]].concat()
        );
        // The bounds, the base layer's offset and rows, the 2 limits of the cover's check, the 4 limits of the thickness and its cap, the switch of the details and the smoothing and the overlap under a column
        // are the advanced settings, which no dialog shows.
        let advanced: Vec<&str> = AutoParams::SPECS.iter().filter(|spec| spec.group == ADVANCED).map(|spec| spec.key).collect();
        assert_eq!(advanced, [&bounds[..], &cover[..], &thick[..], &[DETAILS_LAST, "auto_smooth_mm", COLUMN_OVERLAP]].concat());
    }

    #[test]
    fn the_widest_satin_column_is_the_machines_longest_satin_stitch_at_the_most() {
        // A shape may set it lower, never higher: a higher value reads as the longest satin stitch.
        let read = |text: &str| AutoParams::from_set(&[(MAX_SATIN_WIDTH, text)].into_iter().collect()).unwrap();
        assert_eq!(read("5").params.auto_max_satin_width_mm.get(), 5.0);
        let raised = read("9");
        assert_eq!((raised.params.auto_max_satin_width_mm.get(), raised.warnings.len()), (LONGEST_SATIN_STITCH, 1));
        assert_eq!(AutoParams::defaults().auto_max_satin_width_mm.get(), LONGEST_SATIN_STITCH);
    }
}
