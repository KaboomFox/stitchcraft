//! The letters of the differential's pictures (M8.14), drawn as straight strokes.
//!
//! **Why strokes.** The repository keeps no font, and a font's outlines would bring curves, whose drawing
//! isn't the same on every platform. The thickness key draws its figures as a calculator's 7 bars for the same
//! reason (`stitchcraft-render`'s `key`). A picture's labels need letters too, so each character here is a
//! few straight lines on a grid 4 wide and 6 high, from its top left corner, which tiny-skia draws with arithmetic alone: a picture
//! is the same bytes wherever it is drawn, as `cargo xtask shots` needs. Only capitals are drawn, and a
//! character without strokes of its own is drawn as a question mark.

/// A character's width on the grid, and the room after it: a character takes 6 units.
pub const ADVANCE: f32 = 6.0;

/// The lines of `c`, each a run of points on the grid from its top left corner, `x` then `y`, a digit each.
fn strokes(c: char) -> &'static [&'static str] {
    match c.to_ascii_uppercase() {
        'A' => &["0602204246", "0444"],
        'B' => &["003041423303", "3344453606", "0006"],
        'C' => &["4130100105163645"],
        'D' => &["00304145360600"],
        'E' => &["40000646", "0333"],
        'F' => &["400006", "0333"],
        'G' => &["41301001051636454323"],
        'H' => &["0006", "4046", "0343"],
        'I' => &["1030", "2026", "1636"],
        'J' => &["40453606150504"],
        'K' => &["0006", "400346"],
        'L' => &["000646"],
        'M' => &["0600234046"],
        'N' => &["06004640"],
        'O' => &["1030414536160501 10"],
        'P' => &["06003041423303"],
        'Q' => &["1030414536160501 10", "2446"],
        'R' => &["06003041423303", "2346"],
        'S' => &["413010010213334445361605"],
        'T' => &["0040", "2026"],
        'U' => &["0005163645 40"],
        'V' => &["002640"],
        'W' => &["0016233640"],
        'X' => &["0046", "4006"],
        'Y' => &["002340", "2326"],
        'Z' => &["00400646"],
        '0' => &["1030414536160501 10", "0541"],
        '1' => &["112026", "1636"],
        '2' => &["011030414206 46"],
        '3' => &["01103041423323 33 44453616 05"],
        '4' => &["300444", "3036"],
        '5' => &["4000033344453616 05"],
        '6' => &["4130100105163645443303"],
        '7' => &["00402 6"],
        '8' => &["1030414233130201 10", "133344453616050413"],
        '9' => &["43130201103041453616 05"],
        '.' => &["1626"],
        ',' => &["2615"],
        ':' => &["2122", "2526"],
        '-' => &["0343"],
        '+' => &["0343", "2125"],
        '/' => &["0640"],
        '#' => &["1016", "3036", "0242", "0444"],
        '%' => &["0640", "0011", "3546"],
        '(' => &["30121436"],
        ')' => &["10323416"],
        '\'' => &["2021"],
        '!' => &["2024", "2626"],
        '_' => &["0646"],
        '=' => &["0242", "0444"],
        '×' => &["0145", "4105"],
        ' ' => &[],
        _ => &["0110304142322325", "2626"],
    }
}

/// The lines of `c` as segments on the grid, each from one point to the next.
pub fn segments(c: char) -> Vec<((f32, f32), (f32, f32))> {
    let mut out = Vec::new();
    for line in strokes(c) {
        let digits: Vec<f32> = line.chars().filter_map(|d| d.to_digit(10)).map(|d| d as f32).collect();
        let points: Vec<(f32, f32)> = digits.as_chunks::<2>().0.iter().map(|&[x, y]| (x, y)).collect();
        out.extend(points.windows(2).map(|pair| (pair[0], pair[1])));
    }
    out
}

/// How wide `text` is on the grid: [`ADVANCE`] a character, without the room after the last.
pub fn width(text: &str) -> f32 {
    let n = text.chars().count() as f32;
    if n == 0.0 { 0.0 } else { n * ADVANCE - 2.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_character_a_label_uses_is_drawn_within_its_grid() {
        let used = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,:-+/#%()'_=×!";
        for c in used.chars() {
            let lines = segments(c);
            assert!(!lines.is_empty(), "{c}");
            assert!(lines.iter().all(|&((x0, y0), (x1, y1))| [x0, x1].iter().all(|x| (0.0..=4.0).contains(x))
                && [y0, y1].iter().all(|y| (0.0..=6.0).contains(y))), "{c}: {lines:?}");
            // Each character's look is its own, and none is the stand-in for those without strokes.
            assert_ne!(lines, segments('\u{1F9F5}'), "{c}");
        }
        let distinct: std::collections::BTreeSet<String> = used.chars().map(|c| format!("{:?}", segments(c))).collect();
        assert_eq!(distinct.len(), used.chars().count());
        // Small letters are drawn as capitals, a space as nothing.
        assert_eq!(segments('a'), segments('A'));
        assert!(segments(' ').is_empty());
        // A line of points runs from each point to the next: the minus sign is one segment.
        assert_eq!(segments('-'), [((0.0, 3.0), (4.0, 3.0))]);
    }

    #[test]
    fn a_text_is_as_wide_as_its_characters_without_the_room_after_the_last() {
        assert_eq!((width(""), width("A"), width("AB"), width("A×B")), (0.0, 4.0, 10.0, 16.0));
    }
}
