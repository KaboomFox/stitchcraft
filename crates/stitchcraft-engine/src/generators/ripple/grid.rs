//! A ripple's grid (milestone M10.11, `REQ-RIP-015`): lines across the ripple's own, as Ink/Stitch sews its
//! ripple stitch's grid (`docs/src/design/algorithms/strokes.md` › Ripple stitch › The grid).
//!
//! **Why.** A ripple's lines run one way. A grid adds lines across them, so the ripple sews a net, such as
//! the scales of a fish or a basket's weave.
//!
//! **Its lines.** Point `j` of every line of the ripple lies on one line across it: the outline's point
//! `j` copied, or a column's pair `j`. The grid sews that line through the ripple's lines kept after the
//! skips, in their order. With `grid_size_mm` the outline's needle points, or the column's pairs, lie that
//! far apart, so the grid's lines do too. Without `flip_copies`, the grid's every other line runs the other
//! way where the ripple's run one way and join flat. Then the grid turns to start at its corner nearest the
//! ripple's last needle point: its first or last line, from its start or its end, the first of them when
//! several are as near.

use stitchcraft_core::Point;

/// The grid's lines across `lines`, a ripple's lines as they are sewn, each line `j` through point `j` of
/// each, every other one reversed when `reverse_odd`, then turned to start nearest `last`.
pub(super) fn lines(lines: &[Vec<Point>], reverse_odd: bool, last: Point) -> Vec<Vec<Point>> {
    let across = lines.first().map_or(0, Vec::len);
    let mut grid: Vec<Vec<Point>> = (0..across).map(|j| lines.iter().filter_map(|line| line.get(j).copied()).collect()).collect();
    if reverse_odd {
        grid.iter_mut().skip(1).step_by(2).for_each(|line| line.reverse());
    }
    oriented(grid, last)
}

/// `grid` turned to start at its corner nearest `last`: the first line's start, the first line's end, the
/// last line's start or the last line's end, the first of them when several are as near. From an end, each
/// line is reversed, and from the last line, their order is.
fn oriented(mut grid: Vec<Vec<Point>>, last: Point) -> Vec<Vec<Point>> {
    let (Some(first), Some(final_line)) = (grid.first(), grid.last()) else { return grid };
    let corners = [first.first(), first.last(), final_line.first(), final_line.last()];
    let nearest = corners
        .iter()
        .enumerate()
        .filter_map(|(k, corner)| Some((k, corner.as_ref()?.distance(last))))
        .fold(None, |best: Option<(usize, f64)>, (k, d)| if best.is_some_and(|(_, b)| b <= d) { best } else { Some((k, d)) });
    let Some((corner, _)) = nearest else { return grid };
    if corner % 2 == 1 {
        grid.iter_mut().for_each(|line| line.reverse());
    }
    if corner >= 2 {
        grid.reverse();
    }
    grid
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// The points `(x, y)` of each of `lines`.
    fn points(lines: &[&[(f64, f64)]]) -> Vec<Vec<Point>> {
        lines.iter().map(|line| line.iter().map(|&(x, y)| p(x, y)).collect()).collect()
    }

    #[test]
    fn req_rip_015_grid_line_j_joins_point_j_of_each_line_every_other_one_reversed() {
        // 3 lines across, at y = 0, 1 and 2, of 2 points each: the grid's lines run down x = 0 and x = 1.
        let ripple = points(&[&[(0.0, 0.0), (1.0, 0.0)], &[(0.0, 1.0), (1.0, 1.0)], &[(0.0, 2.0), (1.0, 2.0)]]);
        let from_start = p(-1.0, -1.0);
        assert_eq!(lines(&ripple, false, from_start), points(&[&[(0.0, 0.0), (0.0, 1.0), (0.0, 2.0)], &[(1.0, 0.0), (1.0, 1.0), (1.0, 2.0)]]));
        assert_eq!(lines(&ripple, true, from_start), points(&[&[(0.0, 0.0), (0.0, 1.0), (0.0, 2.0)], &[(1.0, 2.0), (1.0, 1.0), (1.0, 0.0)]]));
        // A line shorter than the first leaves the grid's lines through its missing points shorter.
        let uneven = points(&[&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)], &[(0.0, 1.0), (1.0, 1.0)]]);
        assert_eq!(lines(&uneven, false, from_start), points(&[&[(0.0, 0.0), (0.0, 1.0)], &[(1.0, 0.0), (1.0, 1.0)], &[(2.0, 0.0)]]));
        assert_eq!(lines(&[], true, from_start), Vec::<Vec<Point>>::new());
    }

    #[test]
    fn req_rip_015_the_grid_starts_at_its_corner_nearest_the_ripple_s_last_needle_point() {
        let grid = points(&[&[(0.0, 0.0), (0.0, 2.0)], &[(1.0, 0.0), (1.0, 2.0)]]);
        let from = |x: f64, y: f64| oriented(grid.clone(), p(x, y));
        // Nearest the first line's start, its end, the last line's start and its end.
        assert_eq!(from(-0.1, -0.1), grid);
        assert_eq!(from(-0.1, 2.1), points(&[&[(0.0, 2.0), (0.0, 0.0)], &[(1.0, 2.0), (1.0, 0.0)]]));
        assert_eq!(from(1.1, -0.1), points(&[&[(1.0, 0.0), (1.0, 2.0)], &[(0.0, 0.0), (0.0, 2.0)]]));
        assert_eq!(from(1.1, 2.1), points(&[&[(1.0, 2.0), (1.0, 0.0)], &[(0.0, 2.0), (0.0, 0.0)]]));
        // As near all 4, the first corner wins, and as near the last 2, the first of them.
        assert_eq!(from(0.5, 1.0), grid);
        assert_eq!(from(5.0, 1.0), points(&[&[(1.0, 0.0), (1.0, 2.0)], &[(0.0, 0.0), (0.0, 2.0)]]));
        // A grid of no line, or of empty lines, stays as it is.
        assert_eq!(oriented(Vec::new(), p(0.0, 0.0)), Vec::<Vec<Point>>::new());
        assert_eq!(oriented(vec![Vec::new()], p(0.0, 0.0)), vec![Vec::<Point>::new()]);
    }
}
