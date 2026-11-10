//! A guided fill's rows over one piece with the parallel offset strategy: offset curves of its guide line, a
//! row spacing apart, as Ink/Stitch offsets its guide (read at `d59c9ab`, `REQ-FILL-GDD-009`).
//!
//! **Why offset.** A copy of a curved guide moved square to the line between its ends lies closer to the
//! guide where the guide runs across that way, and the rows crowd there. An offset curve stays the same
//! distance from the guide all along it, so the rows keep their spacing round every bend. On the inside of
//! a bend tighter than the distance, the curve skips the loop a copy would make, and the rows turn there
//! with a sharp corner.
//!
//! **The guide.** Its first subpath, flattened. A closed guide that doesn't cross itself is a ring, and is
//! offset as it is. Any other guide is lengthened straight on at both ends as a copy's is, and then made one
//! line where it crosses itself, as a copied guide is, but not opened. A ring's offset is the offset of its
//! points taken as a line that ends where it starts, as shapely's is: shrunk, it closes where the ring's
//! first and last sides meet, and grown, it leaves out the round corner at the ring's start.
//!
//! **The rows.** Row `k` is the guide's offset curve at `k` row spacings, to the guide's left with y up, which
//! is its right as the drawing shows it, square to the line between its ends as a copy moves
//! ([`super::copies`]). Row 0 lies a ten-thousandth of a CSS pixel off the guide, as Ink/Stitch's does. The
//! rows start from the row nearest the piece's centroid, as the copies do, and go outward one way and then
//! the other while they meet the piece's bounds. `guided_fill_angle` moves only copies. Where an offset
//! falls into pieces, pieces that meet end to end are joined, and each piece that meets the piece is a row.
//!
//! **Cost.** Each offset curve costs as the offset module's curves do ([`crate::normalize::offset`]), and
//! each test of a row against the piece looks only at the sides of the rings near it.

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Exhausted, Meter, Point};

use super::Guided;
use super::closed::closed;
use super::copies::{Bounds, lengthened, meets, square_to, start, unit};
use super::line::{one_line, simple};
use crate::design::Join;
use crate::generators::tatami::rings::Rings;
use crate::normalize::offset::{Offset, offset};
use crate::normalize::region::Polygon;

/// How far off the guide row 0 lies, in millimetres: a ten-thousandth of a CSS pixel, as Ink/Stitch moves
/// it so that the offset is never 0.
pub(super) const ROW_0: f64 = 1e-4 * MM_PER_SVG_PX;

/// The rows the guide line `guide`, flattened, gives over `piece`, whose rings are `rings`, as `guided`
/// spaces them: its offset curves that meet the piece, in the order found, as the module says. Costs as the
/// module says.
pub(super) fn rows(piece: &Polygon, rings: &Rings<'_>, guide: &[Point], guided: &Guided, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let bounds = Bounds::of(&piece.outline);
    let ring = closed(guide) && simple(guide, meter)?;
    let line = if ring { guide.to_vec() } else { lengthened(guide, &bounds, meter)? };
    let line = if simple(&line, meter)? { line } else { one_line(&line, meter)? };
    let (Some(&first), Some(&last)) = (line.first(), line.last()) else { return Ok(Vec::new()) };
    // A ring's ends are one point, and its rows start from the row nearest the centroid on the guide's left.
    let step = square_to(unit(first, last));
    let start = start(piece, rings, &line, step, guided.spacing, meter)?;
    let mut rows = Vec::new();
    for way in [1_i64, -1] {
        let mut k = if way == 1 { start } else { start - 1 };
        loop {
            meter.charge(1)?;
            #[allow(clippy::cast_precision_loss)] // Row numbers are far below 2⁵².
            let by = k as f64 * guided.spacing;
            let by = if by == 0.0 { ROW_0 } else { by };
            let curves = line_offsets(&line, by, meter)?;
            let mut met = false;
            for curve in &curves {
                met |= bounds.met_by(curve, meter)?;
            }
            if !met {
                break;
            }
            for curve in curves {
                if meets(&curve, rings, meter)? {
                    rows.push(curve);
                }
            }
            k += way;
        }
    }
    Ok(rows)
}

/// The offset curves of `line` at `by`, to its left with y up, with round corners of 16 segments to a
/// quarter circle, as shapely's `offset_curve` gives Ink/Stitch: none, one, or the pieces it falls into.
/// Pieces never meet end to end, a stretch the stroke covers lying between each 2, so the merge Ink/Stitch
/// gives them joins none.
fn line_offsets(line: &[Point], by: f64, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    Ok(match offset(line, by, Join::Round, meter)? {
        Offset::Empty => Vec::new(),
        Offset::Line(curve) => vec![curve],
        Offset::Apart(pieces) => pieces,
    })
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::tatami::fixture::p;

    fn xy(points: &[Point]) -> Vec<(f64, f64)> {
        points.iter().map(|q| ((q.x() * 1e6).round() / 1e6, (q.y() * 1e6).round() / 1e6)).collect()
    }

    #[test]
    fn req_fill_gdd_009_a_closed_guide_is_offset_as_a_line_that_ends_where_it_starts() {
        // A square and an L, counter-clockwise with y up, as shapely 2.2 on GEOS 3.14 offsets them as rings: grown,
        // from the first point moved off the first side round to the last side's end, without the round corner
        // at the start (3 quarter circles of 16 segments); shrunk, closed where the first and last sides meet.
        let meter = &mut Budget::DEFAULT.meter();
        let square = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0), p(0.0, 0.0)];
        let grown = line_offsets(&square, -1.0, meter).unwrap();
        assert_eq!((grown.len(), grown[0].len()), (1, 53));
        assert_eq!((xy(&grown[0][..2]), xy(&grown[0][52..])), (vec![(0.0, -1.0), (10.0, -1.0)], vec![(-1.0, 0.0)]));
        assert_eq!(xy(&line_offsets(&square, 1.0, meter).unwrap()[0]), [(1.0, 1.0), (9.0, 1.0), (9.0, 9.0), (1.0, 9.0), (1.0, 1.0)]);
        let l = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 4.0), p(4.0, 4.0), p(4.0, 10.0), p(0.0, 10.0), p(0.0, 0.0)];
        assert_eq!(line_offsets(&l, -1.0, meter).unwrap()[0].len(), 71);
        let shrunk = line_offsets(&l, 1.5, meter).unwrap();
        assert_eq!((shrunk[0].len(), shrunk[0][0], shrunk[0][22]), (23, p(1.5, 1.5), p(1.5, 1.5)));
    }

    #[test]
    fn req_fill_gdd_009_an_offset_may_fall_into_pieces_each_a_row() {
        let meter = &mut Budget::DEFAULT.meter();
        // Along x, round a box, back down beside itself to 0.5 mm above, and up: the stroke 2 mm wide covers the
        // offset's middle, which falls into a line from the start to the top, and a ring inside the box.
        let hook = [p(0.0, 0.0), p(30.0, 0.0), p(30.0, 10.0), p(15.0, 10.0), p(15.0, 0.5), p(14.0, 0.5), p(14.0, 20.0)];
        let pieces = line_offsets(&hook, 2.0, meter).unwrap();
        let ends: Vec<_> = pieces.iter().map(|piece| (xy(&piece[..1]), xy(&piece[piece.len() - 1..]))).collect();
        assert_eq!(ends, [(vec![(0.0, 2.0)], vec![(12.0, 20.0)]), (vec![(17.0, 2.0)], vec![(17.0, 2.0)])]);
        assert!(closed(&pieces[1]) && !closed(&pieces[0]));
        // No length: no curve. A segment: moved.
        assert!(line_offsets(&[p(0.0, 0.0), p(0.0, 0.0)], 1.0, meter).unwrap().is_empty());
        assert_eq!(xy(&line_offsets(&[p(0.0, 0.0), p(5.0, 0.0)], 1.0, meter).unwrap()[0]), [(0.0, 1.0), (5.0, 1.0)]);
    }
}
