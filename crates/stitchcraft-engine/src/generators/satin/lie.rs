//! How a satin column's stitches lie (`docs/src/design/algorithms/satin.md` › Leaning and long stitches):
//! the 2 warnings every satin method gives, which Ink/Stitch does not.
//!
//! - **Leaning stitches** (`SC-W0208`). Rails without enough rungs pair their points by length alone, and
//!   where one rail runs on ahead of the other the stitches lean along the column. The pairs that cut the
//!   column, placed with no compensation and nothing random, show where. At each, the column runs halfway
//!   between its 2 rails' ways there, so that a stitch square to either rail where the other steps out
//!   never leans too far. A stitch that goes further along the column than across it, more than 45° from
//!   square to a tenth of a degree, leans too far: rounding alone never tips a 45° stitch over. Each
//!   stretch of such pairs in a row is named once, at its middle, where a rung would set it straight.
//! - **Long stitches** (`SC-W0209`). A top stitch longer than 12 mm floats loose and snags. The longest is
//!   named once, at its middle, measured on the top stitches as the column sews them: a stitch that the
//!   column's end cuts in 2 is 2 stitches.
//!
//! Both look at what the column has made already, the cuts and the top stitches, whose making charged the
//! budget for each of them, so the looks charge nothing more.

use stitchcraft_core::{Code, Diagnostic, Fix, Point, math};

use crate::generators::mm;
use crate::generators::satin::pairs::Pair;

/// A stitch that leans further than this from square to its column, in degrees to a tenth, leans too far
/// (`SC-W0208`).
pub(crate) const MOST_LEAN: f64 = 45.0;

/// A top stitch longer than this, in millimetres, floats loose and may snag (`SC-W0209`).
pub(crate) const LONG_STITCH: f64 = 12.0;

/// `SC-W0208` for each stretch of `cuts` in a row whose stitches lean more than [`MOST_LEAN`] from square
/// to the column, at the middle of the stretch's middle pair: the later of the 2 middle pairs of an even
/// stretch. At each pair the column runs as [`column`] says, between the pairs before and after it, or the
/// pair itself at the column's ends. A pair shorter than the shortest stitch `min_stitch` does not lean.
pub(crate) fn leaning(cuts: &[Pair], min_stitch: f64) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    let mut stretch: Vec<(Point, f64)> = Vec::new();
    for (i, pair) in cuts.iter().enumerate() {
        let before = i.checked_sub(1).and_then(|j| cuts.get(j)).unwrap_or(pair);
        let after = cuts.get(i + 1).unwrap_or(pair);
        let [a, b] = *pair;
        match lean(*pair, column(before, after), min_stitch) {
            Some(degrees) => stretch.push((a.lerp(b, 0.5), degrees)),
            None => found.extend(named(&std::mem::take(&mut stretch))),
        }
    }
    found.extend(named(&stretch));
    found
}

/// The way the column runs from the pair `before` to the pair `after`: halfway between its 2 rails' ways,
/// each the line from `before`'s point on that rail to `after`'s, taken with length 1. A rail whose 2
/// points coincide gives way to the other, and with neither the column has no way.
fn column(&[a0, b0]: &Pair, &[a1, b1]: &Pair) -> Option<(f64, f64)> {
    match (unit(a0, a1), unit(b0, b1)) {
        (Some((ax, ay)), Some((bx, by))) => Some((ax + bx, ay + by)),
        (Some(way), None) | (None, Some(way)) => Some(way),
        (None, None) => None,
    }
}

/// The line from `from` to `to` with length 1, or `None` when they coincide.
fn unit(from: Point, to: Point) -> Option<(f64, f64)> {
    let length = from.distance(to);
    (length > 0.0).then(|| ((to.x() - from.x()) / length, (to.y() - from.y()) / length))
}

/// How far the stitch of the pair `[a, b]` leans from square to a column that runs along `way`, in degrees
/// to a tenth, when that is more than [`MOST_LEAN`]. A pair shorter than `min_stitch`, or one whose column
/// has no way, does not lean, and neither does one whose rails run opposite ways.
fn lean([a, b]: Pair, way: Option<(f64, f64)>, min_stitch: f64) -> Option<f64> {
    let (dx, dy) = way?;
    let (sx, sy) = (b.x() - a.x(), b.y() - a.y());
    // Both measured against the way as long as it is, which compares them alike.
    let (along, across) = ((sx * dx + sy * dy).abs(), (sx * dy - sy * dx).abs());
    let tenths = (math::to_degrees(math::atan2(along, across)) * 10.0).round();
    (a.distance(b) >= min_stitch && tenths > MOST_LEAN * 10.0).then_some(tenths / 10.0)
}

/// `SC-W0208` for the `stretch` of leaning pairs, each its middle and how far it leans: at the middle of
/// its middle pair, with how many lean and the most any leans. None for an empty stretch.
fn named(stretch: &[(Point, f64)]) -> Option<Diagnostic> {
    let &(at, _) = stretch.get(stretch.len() / 2)?;
    let most = stretch.iter().map(|&(_, degrees)| degrees).fold(0.0, f64::max);
    let message = match stretch.len() {
        1 => format!("A stitch of this satin column leans {most:.1}° along it, more than {MOST_LEAN}° from square to the column."),
        count => format!("{count} stitches of this satin column lean along it, up to {most:.1}°, more than {MOST_LEAN}° from square to the column."),
    };
    let hint = "Add a rung across the column here, square to it, and the stitches follow the rung.";
    Some(Diagnostic::new(Code::SatinStitchesLean, message).located(at).with_fix(Fix::Hint(hint.to_string())))
}

/// `SC-W0209` for the longest stitch of the top stitches' `parts`, each a run of needle points as the column
/// sews it, when that is longer than [`LONG_STITCH`], at its middle: the first of several as long.
pub(crate) fn longest<'a>(parts: impl IntoIterator<Item = &'a [Point]>) -> Option<Diagnostic> {
    let (length, a, b) = parts.into_iter().flat_map(|part| part.iter().zip(part.iter().skip(1))).map(|(&a, &b)| (a.distance(b), a, b)).fold(
        None,
        |longest: Option<(f64, Point, Point)>, stitch| match longest {
            Some(kept) if kept.0 >= stitch.0 => Some(kept),
            _ => Some(stitch),
        },
    )?;
    (length > LONG_STITCH).then(|| {
        let message = format!("A top stitch of this satin column is {} mm long, longer than {} mm, and may snag.", mm(length), mm(LONG_STITCH));
        let hint = "Set `max_stitch_length_mm` to split long stitches, make the column narrower, or sew the shape as a fill.";
        Diagnostic::new(Code::SatinStitchLong, message).located(a.lerp(b, 0.5)).with_fix(Fix::Hint(hint.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn the_codes_titles_give_the_limits_set_here() {
        assert!(Code::SatinStitchesLean.title().contains(&format!("{MOST_LEAN}°")), "{}", Code::SatinStitchesLean.title());
        assert!(Code::SatinStitchLong.title().contains(&format!("{} mm", mm(LONG_STITCH))), "{}", Code::SatinStitchLong.title());
    }

    #[test]
    fn a_stitch_leans_when_it_goes_further_along_the_column_than_across_it() {
        let east = Some((2.0, 0.0));
        assert_eq!(lean([p(1.0, 0.0), p(1.0, 4.0)], east, 0.3), None, "square");
        assert_eq!(lean([p(0.0, 0.0), p(4.0, 4.0)], east, 0.3), None, "45° is not more than 45°");
        assert_eq!(lean([p(0.0, 0.0), p(4.006, 4.0)], east, 0.3), None, "45.04° is 45.0° to a tenth");
        assert_eq!(lean([p(0.0, 0.0), p(4.01, 4.0)], east, 0.3), Some(45.1), "45.07° is 45.1° to a tenth");
        assert_eq!(lean([p(0.0, 0.0), p(8.0, 4.0)], east, 0.3), Some(63.4));
        // Either way along the column.
        assert_eq!(lean([p(8.0, 0.0), p(0.0, 4.0)], east, 0.3), Some(63.4));
        assert_eq!(lean([p(0.0, 0.0), p(0.2, 0.1)], east, 0.3), None, "shorter than the shortest stitch");
        assert!(lean([p(0.0, 0.0), p(4.0, 3.0)], east, 5.0).is_some(), "as long as the shortest stitch");
        assert_eq!(lean([p(0.0, 0.0), p(8.0, 4.0)], None, 0.3), None, "no way to run along");
        // Anywhere and any way round: a column running from (3, 9) towards (5, 11), and a stitch from (4, 7)
        // to (9, 8) that goes 12 along it for 8 across, both measured against that way: 56.3°.
        assert_eq!(lean([p(4.0, 7.0), p(9.0, 8.0)], Some((2.0, 2.0)), 0.3), Some(56.3));
    }

    #[test]
    fn a_column_runs_halfway_between_its_rails() {
        let pair = |a: (f64, f64), b: (f64, f64)| [p(a.0, a.1), p(b.0, b.1)];
        // Rails that run east and north-east, from anywhere: halfway, with each taken with length 1.
        let (x, y) = column(&pair((3.0, 9.0), (5.0, 2.0)), &pair((5.0, 9.0), (8.0, 5.0))).unwrap();
        let half = 0.5_f64.sqrt();
        assert!((x - (1.0 + half)).abs() < 1e-12 && (y - half).abs() < 1e-12, "({x}, {y})");
        // A rail that stands still gives way to the other, and with both still there is no way.
        assert_eq!(column(&pair((0.0, 0.0), (1.0, 4.0)), &pair((0.0, 0.0), (1.0, 6.0))), Some((0.0, 1.0)));
        assert_eq!(column(&pair((0.0, 0.0), (1.0, 4.0)), &pair((0.0, 0.0), (1.0, 4.0))), None);
        // Where the second rail steps out square to the first, a stitch square to the first leans less than
        // 45°: 0.4 mm along the first rail and 4 mm out along the second.
        let step = column(&pair((10.0, 0.0), (10.0, 4.0)), &pair((10.4, 0.0), (10.4, 8.0)));
        assert_eq!(lean([p(10.2, 0.0), p(10.2, 6.0)], step, 0.3), None);
    }

    #[test]
    fn each_stretch_is_named_at_its_middle() {
        // Pairs every 1 mm along y = 2, square but for 2 stretches that lean 3 mm along for 1 mm across. Each
        // rail of a leaning pair is 1.5 mm off its square place, so the rails still run east.
        let square = |x: f64| [p(x, 1.5), p(x, 2.5)];
        let leaning_pair = |x: f64| [p(x - 1.5, 1.5), p(x + 1.5, 2.5)];
        let cuts: Vec<Pair> = (0..10).map(|i| if [2, 3, 4, 7].contains(&i) { leaning_pair(f64::from(i)) } else { square(f64::from(i)) }).collect();
        let found = leaning(&cuts, 0.3);
        let places: Vec<Point> = found.iter().filter_map(|d| d.at).collect();
        assert_eq!(places, [p(3.0, 2.0), p(7.0, 2.0)]);
        assert!(found[0].message.starts_with("3 stitches") && found[1].message.starts_with("A stitch"), "{found:?}");
        // An even stretch is named at the later of its 2 middle pairs, and one at the end is named too.
        let cuts: Vec<Pair> = (0..6).map(|i| if i >= 2 { leaning_pair(f64::from(i)) } else { square(f64::from(i)) }).collect();
        assert_eq!(leaning(&cuts, 0.3).iter().filter_map(|d| d.at).collect::<Vec<_>>(), [p(4.0, 2.0)]);
        // So is one at the start, its first pair measured along the way to the next pair.
        let cuts: Vec<Pair> = (0..6).map(|i| if i < 2 { leaning_pair(f64::from(i)) } else { square(f64::from(i)) }).collect();
        let found = leaning(&cuts, 0.3);
        assert_eq!((found.len(), found[0].at), (1, Some(p(1.0, 2.0))));
        assert!(found[0].message.starts_with("2 stitches"), "{}", found[0].message);
        assert!(leaning(&[], 0.3).is_empty());
    }

    #[test]
    fn only_a_stitch_longer_than_12_mm_is_named_the_first_of_the_longest() {
        let top = [p(0.0, 0.0), p(0.0, 13.0), p(1.0, 0.0), p(1.0, 13.0)];
        let found = longest([&top[..]]).unwrap();
        assert_eq!((found.code, found.at), (Code::SatinStitchLong, Some(p(0.5, 6.5))), "the stitch back is longer");
        assert!(found.message.contains("13.04 mm"), "{}", found.message);
        let tied = [p(0.0, 0.0), p(0.0, 13.0), p(1.0, 13.0), p(1.0, 0.0)];
        assert_eq!(longest([&tied[..]]).and_then(|d| d.at), Some(p(0.0, 6.5)));
        assert_eq!(longest([&[p(0.0, 0.0), p(0.0, 12.0)][..]]), None);
        assert_eq!(longest([&[p(0.0, 0.0)][..]]), None);
        // The parts are sewn apart: no stitch joins one to the next.
        assert_eq!(longest([&[p(0.0, 0.0), p(0.0, 6.0)][..], &[p(0.0, 20.0), p(0.0, 14.0)]]), None);
    }
}
