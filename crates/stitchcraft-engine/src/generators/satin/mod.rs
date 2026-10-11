//! Satin columns (milestone M4): a band of stitches that swing from one rail to the other, the look of
//! lettering and borders (`docs/src/design/algorithms/satin.md`).
//!
//! An element is a satin column when its `satin_column` setting is on, as in Ink/Stitch, which shows the
//! one setting on both its satin and its stroke tabs. Its path is first recognized as rails and rungs
//! ([`crate::normalize::satin`]): a path that cannot be a satin column gets an error and no stitches, and
//! what recognition took by length, stood in for or left out is named. Then the rails are turned the way
//! they are sewn, shortened or lengthened by push compensation and cut into sections at the rungs (the
//! `column` module), and needle points are placed in pairs across the column along the sections (`pairs`),
//! each pair widened by pull compensation (`compensation`). Needle points that crowd together on a rail
//! are inset (`short`), and the pairs are sewn rail to rail, with long stitches split (`split`).
//!
//! These are the top stitches as Ink/Stitch places them. Before them come the underlays the element turns
//! on, sewn along the same sections (`underlay`): a centre walk, a contour and a zigzag, in that order,
//! with the needle travelling straight from each to the next, all in one run. The column starts near
//! where the elements before it left the needle, and ends near where the next element starts (`ends`).
//! A path of 1 subpath is the column's centre line, and its rails and rungs are made from it as Ink/Stitch
//! makes them (`normalize::centre_line`), then recognized like any others.

mod column;
mod compensation;
mod ends;
mod pairs;
mod short;
mod split;
mod underlay;

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Mm, Point};
use stitchcraft_params::{ChoiceOption, Origin, StitchType, params};

use crate::design::{Join, Path};
use crate::generators::satin::column::Section;
pub(crate) use crate::generators::satin::column::{first_point, sewn_rails};
use crate::generators::satin::compensation::Processor;
use crate::generators::satin::ends::Guides;
use crate::generators::satin::pairs::Pair;
use crate::generators::satin::split::Splitter;
use crate::generators::{Neighbours, Stitched, method};
use crate::normalize::centre_line::{Made, rails_and_rungs};
use crate::normalize::satin::{Pairing, Recognition, Satin, Shape, recognize};

/// The satin methods `satin_method` offers, in Ink/Stitch's order, which its files count on: Ink/Stitch
/// gives the parameter's default as the first one's place in this list.
pub const SATIN_METHODS: &[ChoiceOption] =
    &[method(StitchType::SatinColumn), method(StitchType::EStitch), method(StitchType::SStitch), method(StitchType::SatinZigzag)];

/// The stitch types of satin columns, one per method.
const SATINS: &[StitchType] = &[StitchType::SatinColumn, StitchType::EStitch, StitchType::SStitch, StitchType::SatinZigzag];

/// The stitch types the `satin_column` setting chooses between: those of strokes, with it off, and those
/// of satin columns, with it on.
const STROKES_AND_SATINS: &[StitchType] = &[
    StitchType::RunningStitch,
    StitchType::ManualStitch,
    StitchType::ZigzagStitch,
    StitchType::RippleStitch,
    StitchType::SatinColumn,
    StitchType::EStitch,
    StitchType::SStitch,
    StitchType::SatinZigzag,
];

params! {
    /// Satin column: a band of stitches between two rails.
    pub struct SatinParams for SATINS;

    "Satin column" {
        /// Sew the path as a satin column. Two of its subpaths are the rails, the column's edges, and the
        /// others are rungs across both, which say which point of one rail goes with which of the other.
        /// Without rungs, the rails' nodes pair up instead. A path of 1 subpath is the column's centre
        /// line: the column is as wide as the stroke, and turns its corners as the stroke's join does.
        /// Off, the path is sewn as a stroke, by its `stroke_method`.
        satin_column: Toggle = "false", label "Satin column", applies STROKES_AND_SATINS;

        /// How the column is sewn. A satin column sews stitches straight across it, from one rail to the
        /// other and back. E, S and zigzag stitches arrive in later versions, and until then an element
        /// set to one is skipped (`SC-W0011`).
        satin_method: Choice = "satin_column", label "Method", choices SATIN_METHODS;

        /// The distance from one stitch across the column to the next that goes the same way: from one
        /// rail to the other and back again is one spacing. It is measured across the column at its
        /// outside edge, and on a curve the stitches fan out from the inside edge.
        zigzag_spacing_mm: Length = "0.4", label "Zigzag spacing", range (0.01, 10.0);
    }

    "Rails" {
        /// Which rails are sewn against the way they are drawn, so that both run the same way. Automatic
        /// reverses the second rail when it runs against the first.
        reverse_rails: Choice = "automatic", label "Reverse rails",
            options ["automatic" => "Automatic", "none" => "Neither", "first" => "The first", "second" => "The second", "both" => "Both"];

        /// Make the second rail the first. The column starts on its first rail, and each stitch across it
        /// goes from the first rail to the second.
        swap_satin_rails: Toggle = "false", label "Swap rails";
    }

    "Start and end" {
        /// Start near where the elements before left the needle: on the line between the rails, or on the
        /// column's edge when only that is close, then along the line under the column to its start. It
        /// saves a jump and a trim.
        start_at_nearest_point: Toggle = "true", label "Start at nearest point";

        /// End near where the next element starts: the column is sewn up to the end point, along the line
        /// under it to its end, and back, and ends on its edge nearest the next element.
        end_at_nearest_point: Toggle = "true", label "End at nearest point";

        /// Where the line the needle follows to the start and to the end runs, in percent of the way from
        /// the first rail to the second: 50 is the middle.
        running_stitch_position: Percent = "50", label "Running stitch position", range (0.0, 100.0);
    }

    "Compensation" {
        /// How far each end of every stitch reaches past its rail. The thread pulls the fabric in across the
        /// column as it sews, so a satin comes out narrower than drawn, and this makes up for it. Negative
        /// values make the column narrower. 2 values set the first rail's side, then the second's.
        pull_compensation_mm: LengthPair = "0", label "Pull compensation", range (-10.0, 10.0);

        /// More pull compensation, in percent of the column's width at each stitch, added to the length
        /// above: wide parts of a column reach out further than narrow ones. 2 values set the first rail's
        /// side, then the second's.
        pull_compensation_percent: PercentPair = "0", label "Pull compensation (% of width)", range (-100.0, 100.0);

        /// How much shorter the column is made at its start and its end. Satin stitches push the fabric out
        /// along the column, so it comes out longer than drawn, and this makes up for it. Negative values
        /// lengthen the column. 2 values set the start, then the end.
        push_compensation_mm: LengthPair = "0", label "Push compensation", range (-10.0, 10.0);
    }

    "Random variation" {
        /// How much narrower than the compensated column a stitch may come out on each side, chosen at
        /// random for each stitch, in percent of the column's width there. A ragged edge looks like fur or
        /// grass. 2 values set the first rail's side, then the second's.
        random_width_decrease_percent: PercentPair = "0", label "Random width decrease", range (0.0, 100.0);

        /// How much wider than the compensated column a stitch may come out on each side, chosen at random
        /// for each stitch, in percent of the column's width there. 2 values set the first rail's side, then
        /// the second's.
        random_width_increase_percent: PercentPair = "0", label "Random width increase", range (0.0, 100.0);

        /// How much the distance to each stitch may differ from the zigzag spacing, chosen at random, in
        /// percent of the spacing, longer or shorter.
        random_zigzag_spacing_percent: Percent = "0", label "Random zigzag spacing", range (0.0, 100.0);
    }

    "Short stitches" {
        /// How far a crowded needle point is moved in along its stitch, in percent of the stitch's width.
        /// On the inside of a tight curve the needle points of a rail crowd together, and the thread piles
        /// up there. Moving some of them in spreads them out. Points that crowd one after another take
        /// turns with several values separated by spaces.
        short_stitch_inset: PercentList = "15", label "Short stitch inset", range (0.0, 50.0);

        /// How close a needle point may come to the last one left in place on its rail before it is moved
        /// in. 0 moves none.
        short_stitch_distance_mm: Length = "0.25", label "Short stitch distance", range (0.0, 5.0);
    }

    "Split stitches" {
        /// How stitches longer than the longest stitch (`max_stitch_length_mm`) are split. Default splits
        /// each into the fewest equal parts no longer than it. Simple splits at whole multiples of it from
        /// the stitch's start. Staggered moves those splits along from one stitch to the next, so the needle
        /// holes of neighbouring stitches do not line up in a row.
        split_method: Choice = "default", label "Split method",
            options ["default" => "Default", "simple" => "Simple", "staggered" => "Staggered"];

        /// How far each split may move at random, in percent of a part, either way. With a random split
        /// phase, how much each part's length may vary instead.
        random_split_jitter_percent: Percent = "0", label "Split jitter", range (0.0, 100.0), when split_method == "default";

        /// Start each stitch's splits at a random distance from its start, and space them by the longest
        /// stitch, instead of dividing the stitch evenly. The needle holes of neighbouring stitches then
        /// fall apart, at the cost of a few more stitches.
        random_split_phase: Toggle = "false", label "Random split phase", when split_method == "default";

        /// With a random split phase, also split stitches longer than this but no longer than the longest
        /// stitch. Empty: the longest stitch.
        min_random_split_length_mm: OptionalLength = "", label "Shortest split stitch", range (0.1, 25.0), when split_method == "default";

        /// How many stitches the staggered splits take to come back to where they started. A fraction draws
        /// diagonals that show less than whole numbers do.
        split_staggers: Number = "4", label "Staggers", range (0.01, 100.0), when split_method == "staggered";
    }

    "Centre walk underlay" {
        /// Sew a running stitch along the middle of the column first, there and back. It holds the fabric
        /// still along the column, and suits narrow columns. The other underlays and the top stitches
        /// follow it.
        center_walk_underlay: Toggle = "false", label "Centre walk underlay";

        /// How long each stitch of the centre walk is. Between corners its stitches are spread evenly, so
        /// each one is at most this long.
        center_walk_underlay_stitch_length_mm: Length = "3", label "Centre walk stitch length", range (0.1, 25.0);

        /// How far a centre walk stitch may stray from the middle of the column on a curve. A smaller
        /// tolerance follows curves more closely, with more and shorter stitches.
        center_walk_underlay_stitch_tolerance_mm: Length = "0.2", label "Centre walk tolerance", range (0.01, 5.0),
            origin Origin::InkStitchDeviates { deviation: "DEV-SAT-004" };

        /// How many times the centre walk is sewn: 2 goes there and back, 3 there, back and there again.
        /// An odd number ends at the column's end, and the rest of the column is then sewn from its end.
        center_walk_underlay_repeats: Count = "2", label "Centre walk repeats", range (1, 100);

        /// Where the centre walk runs, in percent of the way from the first rail to the second: 50 is the
        /// middle.
        center_walk_underlay_position: Percent = "50", label "Centre walk position", range (0.0, 100.0);
    }

    "Contour underlay" {
        /// Sew a running stitch along each edge of the column, a little inside it, after any centre walk.
        /// It holds the edges still and keeps them crisp.
        contour_underlay: Toggle = "false", label "Contour underlay";

        /// How long each stitch of the contour is. Between corners its stitches are spread evenly, so each
        /// one is at most this long.
        contour_underlay_stitch_length_mm: Length = "3", label "Contour stitch length", range (0.1, 25.0);

        /// How far a contour stitch may stray from its line on a curve. A smaller tolerance follows curves
        /// more closely, with more and shorter stitches.
        contour_underlay_stitch_tolerance_mm: Length = "0.2", label "Contour tolerance", range (0.01, 5.0);

        /// How far inside each rail the contour runs, so that it does not show past the top stitches. It
        /// also stops short of the column's start by the first rail's value and of its end by the
        /// second's. 2 values set the first rail's side, then the second's.
        contour_underlay_inset_mm: LengthPair = "0.4", label "Contour inset", range (-10.0, 10.0);

        /// More inset, in percent of the column's width at each point, added to the length above. 2 values
        /// set the first rail's side, then the second's.
        contour_underlay_inset_percent: PercentPair = "0", label "Contour inset (% of width)", range (-100.0, 100.0);
    }

    "Zigzag underlay" {
        /// Sew a sparse zigzag across the column, to its end and back, before the top stitches. It lifts
        /// them off the fabric and makes a wide column look fuller.
        zigzag_underlay: Toggle = "false", label "Zigzag underlay";

        /// How far apart the zigzag's points on each rail are, each way.
        zigzag_underlay_spacing_mm: Length = "3", label "Zigzag underlay spacing", range (0.01, 10.0);

        /// How far inside each rail the zigzag's points are. Empty: half the contour's inset, which puts
        /// them between the contour and the edge. 2 values set the first rail's side, then the second's.
        zigzag_underlay_inset_mm: OptionalLengthPair = "", label "Zigzag underlay inset", range (-10.0, 10.0);

        /// More inset, in percent of the column's width at each point, added to the length above. Empty:
        /// half the contour's. 2 values set the first rail's side, then the second's.
        zigzag_underlay_inset_percent: OptionalPercentPair = "", label "Zigzag underlay inset (% of width)", range (-100.0, 100.0);

        /// Split zigzag stitches longer than this into equal parts. Empty: none is split.
        zigzag_underlay_max_stitch_length_mm: OptionalLength = "", label "Zigzag underlay longest stitch", range (0.1, 25.0);
    }
}

/// A satin column's rails and what pairs them: as its `path` draws them, or made from its centre line,
/// `width` wide with `join` at its corners (`REQ-SAT-016`). What recognition took by length, stood in for
/// or left out is added to `diagnostics`; `None`, with the reason added, when the column has no rails.
pub fn rails(path: &Path, width: Mm, join: Join, diagnostics: &mut Vec<Diagnostic>, meter: &mut Meter) -> Result<Option<Satin>, Exhausted> {
    let Recognition { shape, warnings } = recognize(path, meter)?;
    diagnostics.extend(warnings);
    match shape {
        Ok(Shape::Rails(satin)) => Ok(Some(satin)),
        Ok(Shape::CentreLine { line, closed }) => from_centre_line(&line, closed, width, join, diagnostics, meter),
        Err(error) => {
            diagnostics.push(error);
            Ok(None)
        }
    }
}

/// The rails and rungs made from the centre line `line`, with `SC-W0213` for the stretches left out;
/// `None`, with `SC-E0214`, when no part of it can be made. A column with no rungs pairs its rails' points
/// in order, as one drawn with 2 rails does.
fn from_centre_line(
    line: &[Point],
    closed: bool,
    width: Mm,
    join: Join,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Satin>, Exhausted> {
    let Some(Made { rails, rungs, left_out }) = rails_and_rungs(line, closed, width.get(), join, meter)? else {
        diagnostics.push(
            Diagnostic::new(
                Code::SatinCentreLineFailed,
                "This satin column is drawn as one path, and its rails cannot be made from it: the line is too short or turns too \
                 tightly for the stroke's width.",
            )
            .with_fix(Fix::Hint("Draw the column with 2 rails, or make the stroke narrower.".to_string())),
        );
        return Ok(None);
    };
    if left_out > 0 {
        let message = if left_out == 1 {
            "1 part of this satin column drawn as one path crosses itself or turns more tightly than the column is wide, so it is left \
             out."
                .to_string()
        } else {
            format!(
                "{left_out} parts of this satin column drawn as one path cross themselves or turn more tightly than the column is wide, \
                 so they are left out."
            )
        };
        diagnostics.push(Diagnostic::new(Code::SatinCentreLinePartsLeftOut, message).with_fix(Fix::Hint(
            "Draw the column with 2 rails where its line crosses itself or turns more tightly than the column is wide.".to_string(),
        )));
    }
    let pairing = if rungs.is_empty() { Pairing::Nodes(rails.clone()) } else { Pairing::Rungs(rungs) };
    Ok(Some(Satin { rails, pairing }))
}

/// The lengths a satin column's stitches keep to that come from settings other than its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SatinLengths {
    /// The element's shortest stitch: the machine's, or the element's or the design's when longer.
    pub min_stitch: Mm,
    /// The element's longest stitch, when it sets one: longer top stitches are split.
    pub max_stitch: Option<Mm>,
    /// The longest stitch of the needle's travel between the underlays and on to the top stitches, and
    /// of the line it follows to the start and to the end: the first of the running stitch's lengths
    /// (`running_stitch_length_mm`).
    pub travel: Mm,
    /// How far the line to the start and to the end may stray from its pairs: the running stitch's
    /// tolerance (`running_stitch_tolerance_mm`).
    pub tolerance: Mm,
    /// The element's jump length: its `min_jump_stitch_length_mm`, or the design's collapse length when it
    /// sets none or 0. A column starts on its outline only when that is nearer the needle than this.
    pub jump: Mm,
}

/// The satin column `satin` sewn as `params` say, with an element's stitch `lengths`, and its random
/// variation drawn from the element's `rng`: one run of needle points, its underlays first, then its top
/// stitches a pair across the column at a time, from the rails' starts to their ends, or from their ends
/// after an odd centre walk. With its `neighbours`, it starts near where the needle was left and ends near
/// the next element, as its settings say (the `ends` module).
pub fn satin_stitch(
    satin: &Satin,
    params: &SatinParams,
    lengths: SatinLengths,
    neighbours: &Neighbours,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    let mut warnings = Vec::new();
    let sections = column::sections(satin, params, &mut warnings, meter)?;
    let (placed, mut top) = top_stitches(&sections, params, lengths, rng, meter)?;
    let odd = underlay::ends_at_end(params);
    if odd {
        top.reverse();
    }
    let layers = underlay::underlays(&sections, params, lengths.min_stitch.get(), &mut warnings, meter)?;
    let needle = neighbours.needle.filter(|_| params.start_at_nearest_point);
    let next = neighbours.next.as_ref().filter(|_| params.end_at_nearest_point);
    let pieces = if needle.is_none() && next.is_none() {
        ends::sewn(layers, top, meter)?
    } else {
        let guides = Guides::new(&sections, params, lengths, &mut warnings, meter)?;
        let edges: [Vec<Point>; 2] = [placed.iter().map(|[a, _]| *a).collect(), placed.iter().map(|[_, b]| *b).collect()];
        let outline = [edges[0].as_slice(), edges[1].as_slice()];
        let start = match needle {
            Some(needle) => guides.start(needle, outline, lengths.jump.get(), meter)?,
            None => None,
        };
        let end = match next {
            Some(next) => {
                let [first, second] = column::sewn_rails(satin, params, meter)?;
                guides.end(next, [&first, &second], outline, meter)?
            }
            None => None,
        };
        guides.pieces(layers, top, start, end, odd, meter)?
    };
    let run = ends::join(pieces, lengths.travel.get(), lengths.min_stitch.get(), meter)?;
    Ok(Stitched { runs: vec![run], warnings })
}

/// The top stitches along `sections`, from the rails' starts to their ends: placed, compensated and varied
/// at random by the element's `rng`, inset where they crowd, and split where they are long. With them,
/// the pairs as placed and compensated, before any inset: the ends of each pair lie on the column's
/// compensated outline.
fn top_stitches(
    sections: &[Section],
    params: &SatinParams,
    lengths: SatinLengths,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<(Vec<Pair>, Vec<Point>), Exhausted> {
    let placed = pairs::pairs(sections, params.zigzag_spacing_mm.get(), &mut Processor::new(params, rng), meter)?;
    let mut splitter = Splitter::new(params, lengths.max_stitch.map(Mm::get), lengths.min_stitch.get(), rng);
    let insets: Vec<f64> = params.short_stitch_inset.iter().map(|percent| percent / 100.0).collect();
    let short = short::inset(&placed, params.short_stitch_distance_mm.get(), &insets, splitter.inset_limit());
    let stitches = splitter.sew(&placed, &short, meter)?;
    Ok((placed, stitches))
}

#[cfg(test)]
mod tests {
    use stitchcraft_params::Family;

    use super::*;

    #[test]
    fn the_setting_applies_to_every_stroke_and_satin_stitch_type() {
        let of = |families: &[Family]| -> Vec<StitchType> { StitchType::ALL.iter().copied().filter(|t| families.contains(&t.family())).collect() };
        assert_eq!(STROKES_AND_SATINS, of(&[Family::Stroke, Family::Satin]));
        assert_eq!(SATINS, of(&[Family::Satin]));
    }
}
