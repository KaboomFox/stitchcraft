//! Style sheets: the rules of `<style>` elements, which set the properties of every element they select,
//! as its `style` attribute does (`REQ-SVG-006`).
//!
//! Illustrator and CorelDRAW write a drawing's colours as classes in a style sheet (`.cls-1{fill:none}`).
//! Read without it, a file would sew black fills where the drawing shows outlines, and sew what the sheet
//! hides. So the reader applies the rules a viewer applies, in the order CSS's cascade gives them
//! ([`crate::style::Declared`]):
//!
//! 1. A declaration marked `!important` wins over one that is not.
//! 2. Of 2 as important, one in the element's `style` attribute wins over one in a rule.
//! 3. Of 2 rules, the more specific wins: more ids, then more classes, then an element name. Of 2 as
//!    specific, the later in the file wins.
//! 4. Every rule wins over a presentation attribute (`fill="red"`), even one for every element (`*`), as
//!    SVG 2 says.
//!
//! A declaration that does not read gives way to the next, as in CSS.
//!
//! **What is read.** Rules whose selectors are an element name or `*`, then classes and ids, and lists of
//! them: `rect`, `.cls-1`, `#logo`, `path.st0.st1`, `.a, .b`, the kinds Illustrator and CorelDRAW write.
//! Other selectors (descendants, children, attributes, pseudo-classes) and at-rules (`@media`, `@import`, …)
//! are named in one `SC-W0802` note, and what they set is not used. `@charset` and `@font-face` change
//! nothing that is sewn, and are passed over quietly, as is a `<style>` element of another type than CSS,
//! which a viewer does not read either. Comments are skipped, and CDATA sections are read as text. Names
//! match exactly: SVG is XML, whose element names, classes and ids are case-sensitive.
//!
//! **Cost.** The reader charges a sheet's text as it charges a transform, a unit per 8 bytes. A rule is
//! tested only against the elements that could match it, those with its id, one of its classes or its
//! element name (every element, for a rule of `*` alone): a unit per class an element has and per rule
//! tested, and a unit per declaration the element then takes from its rules and its `style` attribute
//! ([`Declared::of`]). A document's sheets may have at most [`MAX_ITEMS`] selectors, declarations and
//! at-rules, counted by what separates them; a file with more is refused (`SC-E0801`), as one with too
//! many XML nodes is: each costs memory, and a small file could hold millions.

use std::collections::{BTreeMap, BTreeSet};

use roxmltree::Node;
use stitchcraft_core::{Diagnostic, Exhausted, Meter};

use crate::style::{Declared, Written, comment_end, css_trim, declarations, split_points, string_end};
use crate::text::unreadable;

/// At-rules that change nothing sewn: the sheet's character set, and fonts, which only text uses.
const QUIET_AT_RULES: [&str; 2] = ["charset", "font-face"];

/// The most selectors, declarations and at-rules a document's style sheets may have together, far beyond
/// any drawing's.
pub const MAX_ITEMS: usize = 1_000_000;

/// A document's style sheets have more than [`MAX_ITEMS`] selectors, declarations and at-rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooMany;

impl TooMany {
    /// What to say when it happens (`SC-E0801`): the file is not read.
    pub fn diagnostic(self) -> Diagnostic {
        unreadable(format!("The file's style sheets have more than {MAX_ITEMS} selectors and declarations, more than StitchCraft reads."))
    }
}

/// How many of the selectors and at-rules not read the note names; it counts the rest.
const NAMED: usize = 3;

/// The longest selector the note quotes in full, in characters.
const QUOTED: usize = 40;

/// The rules of a document's style sheets, in the order they come in the file.
#[derive(Debug, Default)]
pub struct Sheet<'a> {
    /// Each rule's declarations.
    rules: Vec<Vec<Written<'a>>>,
    /// Every selector that is read, with its rule.
    selectors: Vec<(usize, Compound<'a>)>,
    /// The selectors to test against an element, by what an element must have to match: an id, a class,
    /// an element name, or nothing.
    by_id: BTreeMap<&'a str, Vec<usize>>,
    by_class: BTreeMap<&'a str, Vec<usize>>,
    by_name: BTreeMap<&'a str, Vec<usize>>,
    any: Vec<usize>,
    /// The selectors and at-rules not read, as written, in order.
    unread: Vec<&'a str>,
    /// The selectors, declarations and at-rules read, at most: what separates them, counted.
    items: usize,
}

/// A compound selector: an element name (or any element), and the classes and ids an element must all
/// have.
#[derive(Debug)]
struct Compound<'a> {
    name: Option<&'a str>,
    classes: Vec<&'a str>,
    ids: Vec<&'a str>,
}

impl<'a> Compound<'a> {
    /// The selector `text` is, if it is one the reader matches.
    fn parse(text: &'a str) -> Option<Compound<'a>> {
        let mut compound = Compound { name: None, classes: Vec::new(), ids: Vec::new() };
        let mut rest = match text.strip_prefix('*') {
            Some(rest) => rest,
            None => match ident(text) {
                Some((name, rest)) => {
                    compound.name = Some(name);
                    rest
                }
                None => text,
            },
        };
        if rest.len() == text.len() && !text.starts_with(['.', '#']) {
            return None;
        }
        while !rest.is_empty() {
            let (list, after) = match (rest.strip_prefix('.'), rest.strip_prefix('#')) {
                (Some(after), _) => (&mut compound.classes, after),
                (_, Some(after)) => (&mut compound.ids, after),
                _ => return None,
            };
            let (name, after) = ident(after)?;
            list.push(name);
            rest = after;
        }
        Some(compound)
    }

    /// How specific it is: its ids, its classes and its element name, compared in that order.
    fn specificity(&self) -> (usize, usize, usize) {
        (self.ids.len(), self.classes.len(), usize::from(self.name.is_some()))
    }

    /// Whether an element named `name`, with `id` and `classes`, matches it.
    fn matches(&self, name: &str, id: Option<&str>, classes: &BTreeSet<&str>) -> bool {
        self.name.is_none_or(|n| n == name) && self.ids.iter().all(|i| Some(*i) == id) && self.classes.iter().all(|c| classes.contains(c))
    }
}

/// The CSS name at the start of `text`, and what follows it: letters, digits, `-`, `_` and characters
/// beyond ASCII, not starting with a digit or with `-` and a digit. An escape ends it.
fn ident(text: &str) -> Option<(&str, &str)> {
    let end = text.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii())).unwrap_or(text.len());
    let (name, rest) = text.split_at_checked(end)?;
    let starts = |s: &str| s.starts_with(|c: char| c.is_ascii_digit());
    let valid = !name.is_empty() && name != "-" && !starts(name) && !name.strip_prefix('-').is_some_and(starts);
    valid.then_some((name, rest))
}

impl<'a> Sheet<'a> {
    /// Adds the rules of the style sheet `text`, after those already read; `TooMany` when the sheets
    /// would have more than [`MAX_ITEMS`] selectors, declarations and at-rules. The caller charges for it.
    pub fn read(&mut self, text: &'a str) -> Result<(), TooMany> {
        let bytes = text.as_bytes();
        let mut at = 0;
        loop {
            at = after_space(bytes, at);
            let Some(&first) = bytes.get(at) else { return Ok(()) };
            if first == b'@' {
                self.grow(1)?;
                let name = text.get(at + 1..).and_then(ident).map_or("", |(name, _)| name);
                if !QUIET_AT_RULES.iter().any(|quiet| name.eq_ignore_ascii_case(quiet)) {
                    self.unread.push(text.get(at..at + 1 + name.len()).unwrap_or_default());
                }
                at = statement_end(bytes, at) + 1;
                continue;
            }
            // A rule: its selectors up to `{`, and its declarations up to the `}` that closes it. Text
            // with no block after it is not a rule.
            let Some(open) = block_start(bytes, at) else { return Ok(()) };
            let close = block_end(bytes, open);
            self.rule(text.get(at..open).unwrap_or_default(), text.get(open + 1..close).unwrap_or_default())?;
            at = close + 1;
        }
    }

    /// Counts `items` more, before they are read: at most as many as the commas and semicolons that
    /// separate them, and one.
    fn grow(&mut self, items: usize) -> Result<(), TooMany> {
        self.items = self.items.saturating_add(items);
        if self.items > MAX_ITEMS { Err(TooMany) } else { Ok(()) }
    }

    /// Adds the rule of the selector list `selectors` and the declarations `block`. A list with an empty
    /// selector is not valid, and CSS drops the rule.
    fn rule(&mut self, selectors: &'a str, block: &'a str) -> Result<(), TooMany> {
        let separated = |text: &str, by: u8| text.bytes().filter(|&byte| byte == by).count() + 1;
        self.grow(separated(selectors, b',').saturating_add(separated(block, b';')))?;
        let mut list = Vec::new();
        let mut start = 0;
        for at in split_points(selectors, b',').into_iter().chain([selectors.len()]) {
            list.push(css_trim(selectors.get(start..at).unwrap_or_default()));
            start = at + 1;
        }
        if list.iter().any(|selector| selector.is_empty()) {
            return Ok(());
        }
        let rule = self.rules.len();
        self.rules.push(declarations(block));
        for selector in list {
            let Some(compound) = Compound::parse(selector) else {
                self.unread.push(selector);
                continue;
            };
            let index = self.selectors.len();
            let key = match (compound.ids.first(), compound.classes.first(), compound.name) {
                (Some(id), _, _) => Some((&mut self.by_id, *id)),
                (None, Some(class), _) => Some((&mut self.by_class, *class)),
                (None, None, Some(name)) => Some((&mut self.by_name, name)),
                (None, None, None) => None,
            };
            match key {
                Some((map, key)) => map.entry(key).or_default().push(index),
                None => self.any.push(index),
            }
            self.selectors.push((rule, compound));
        }
        Ok(())
    }

    /// What `node` declares, with the rules that select it. One unit of `meter` per class it has, per rule
    /// tested and per declaration ([`Declared::of`]).
    pub fn declared<'input>(&self, node: Node<'a, 'input>, meter: &mut Meter) -> Result<Declared<'a, 'input>, Exhausted> {
        if self.selectors.is_empty() {
            return Declared::of(node, &[], meter);
        }
        let name = node.tag_name().name();
        let id = node.attribute("id");
        let words = node.attribute("class").unwrap_or_default().split_ascii_whitespace();
        meter.charge(u64::try_from(words.clone().count()).unwrap_or(u64::MAX))?;
        let classes: BTreeSet<&str> = words.collect();
        let candidates = id
            .and_then(|id| self.by_id.get(id))
            .into_iter()
            .chain(classes.iter().filter_map(|c| self.by_class.get(c)))
            .chain(self.by_name.get(name))
            .chain([&self.any]);
        // Each rule once, at its most specific selector that matches.
        let mut matched: BTreeMap<usize, (usize, usize, usize)> = BTreeMap::new();
        for &index in candidates.flatten() {
            meter.charge(1)?;
            let Some((rule, compound)) = self.selectors.get(index) else { continue };
            if compound.matches(name, id, &classes) {
                let specificity = compound.specificity();
                let best = matched.entry(*rule).or_insert(specificity);
                *best = specificity.max(*best);
            }
        }
        let mut order: Vec<(&(usize, usize, usize), &usize)> = matched.iter().map(|(rule, specificity)| (specificity, rule)).collect();
        order.sort();
        let rules: Vec<&[Written<'a>]> = order.iter().filter_map(|&(_, &rule)| self.rules.get(rule).map(Vec::as_slice)).collect();
        Declared::of(node, &rules, meter)
    }

    /// The `SC-W0802` note for the selectors and at-rules that are not read, if any: the first few named,
    /// and the rest counted.
    pub fn unread_note(&self) -> Option<String> {
        let quoted: Vec<String> = self.unread.iter().take(NAMED).map(|text| format!("`{}`", short(text))).collect();
        let more = self.unread.len() - quoted.len();
        let list = match (quoted.as_slice(), more) {
            ([], _) => return None,
            ([one], 0) => one.clone(),
            ([first @ .., last], 0) => format!("{} and {last}", first.join(", ")),
            (all, more) => format!("{} and {more} more", all.join(", ")),
        };
        let what = if self.unread.len() == 1 { "what it sets is" } else { "what they set is" };
        Some(format!("StitchCraft does not read {list} in the file's style sheet; {what} not used."))
    }
}

/// Whether the `<style>` element `node` holds CSS: its `type` is missing, empty or `text/css`, whatever its
/// parameters. A viewer reads no other kind.
pub fn is_css(node: Node<'_, '_>) -> bool {
    node.attribute("type").is_none_or(|kind| {
        let media = kind.split(';').next().unwrap_or_default().trim();
        media.is_empty() || media.eq_ignore_ascii_case("text/css")
    })
}

/// What the note says to do about a style sheet it cannot read in full.
pub const UNREAD_HINT: &str = "Use only type, class and id selectors, or save the file with inline styles or presentation attributes.";

/// `text`, cut to [`QUOTED`] characters.
fn short(text: &str) -> String {
    match text.char_indices().nth(QUOTED) {
        Some((cut, _)) => format!("{}…", text.get(..cut).unwrap_or_default()),
        None => text.to_string(),
    }
}

/// The offset of the first byte from `at` that is neither a space nor in a comment, nor CSS's `<!--` or
/// `-->`, which a sheet may have around its rules.
fn after_space(bytes: &[u8], mut at: usize) -> usize {
    loop {
        match bytes.get(at..) {
            Some([b' ' | b'\t' | b'\n' | b'\r' | b'\x0c', ..]) => at += 1,
            Some([b'/', b'*', ..]) => at = comment_end(bytes, at) + 1,
            Some([b'<', b'!', b'-', b'-', ..]) => at += 4,
            Some([b'-', b'-', b'>', ..]) => at += 3,
            _ => return at,
        }
    }
}

/// The offset of the `{` that opens the block of the rule starting at `at`, outside strings, comments,
/// parentheses and brackets; `None` when no block opens.
fn block_start(bytes: &[u8], at: usize) -> Option<usize> {
    let (mut depth, mut i) = (0_u32, at);
    while let Some(&byte) = bytes.get(i) {
        match byte {
            b'"' | b'\'' => i = string_end(bytes, i),
            b'/' if bytes.get(i + 1) == Some(&b'*') => i = comment_end(bytes, i),
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth = depth.saturating_sub(1),
            b'{' if depth == 0 => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// The offset of the `}` that closes the block opening at `open`, outside strings and comments, or the
/// text's end: CSS closes what the sheet leaves open.
fn block_end(bytes: &[u8], open: usize) -> usize {
    let (mut depth, mut i) = (0_u32, open);
    while let Some(&byte) = bytes.get(i) {
        match byte {
            b'"' | b'\'' => i = string_end(bytes, i),
            b'/' if bytes.get(i + 1) == Some(&b'*') => i = comment_end(bytes, i),
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    bytes.len()
}

/// The offset of the last byte of the at-rule starting at `at`: its `;`, the `}` that closes its block, or
/// the text's end.
fn statement_end(bytes: &[u8], at: usize) -> usize {
    let mut i = at;
    while let Some(&byte) = bytes.get(i) {
        match byte {
            b'"' | b'\'' => i = string_end(bytes, i),
            b'/' if bytes.get(i + 1) == Some(&b'*') => i = comment_end(bytes, i),
            b';' => return i,
            b'{' => return block_end(bytes, i),
            _ => {}
        }
        i += 1;
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use roxmltree::Document;
    use stitchcraft_core::Budget;

    use super::*;

    #[test]
    fn selectors_the_reader_matches() {
        let parsed = |text| Compound::parse(text).map(|c| (c.name, c.classes, c.ids));
        assert_eq!(parsed("rect"), Some((Some("rect"), vec![], vec![])));
        assert_eq!(parsed("*"), Some((None, vec![], vec![])));
        assert_eq!(parsed("path.st0.st-1#a_b"), Some((Some("path"), vec!["st0", "st-1"], vec!["a_b"])));
        assert_eq!(parsed(".é"), Some((None, vec!["é"], vec![])));
        assert_eq!(parsed("*.a"), Some((None, vec!["a"], vec![])));
        for other in ["g rect", "g>rect", "a:hover", "[id]", "svg|rect", ".1a", "#-1", ".", "#", ".a.", "r\\ect", ".-", "**"] {
            assert!(parsed(other).is_none(), "{other}");
        }
        assert_eq!(Compound::parse(".a#b.c").map(|c| c.specificity()), Some((1, 2, 0)));
        assert_eq!(Compound::parse("rect").map(|c| c.specificity()), Some((0, 0, 1)));
        assert_eq!(ident("--x y"), Some(("--x", " y")));
        assert_eq!(ident("-x"), Some(("-x", "")));
    }

    #[test]
    fn rules_at_rules_comments_and_broken_text() {
        let mut sheet = Sheet::default();
        sheet
            .read(
                "<!-- /* a; {x} */ .a{fill:red} @media print { .b { fill: blue } } @import 'x;y'; .c, , .d {fill:red} \
                 .e /* , g h */ {stroke : blue /* ; */ ; fill:red} @charset \"x\"; .f{fill:lime --> .g",
            )
            .unwrap();
        assert_eq!(sheet.unread, ["@media", "@import"]);
        assert_eq!(sheet.rules.len(), 3, "the list with an empty selector is dropped");
        let names: Vec<Vec<(&str, &str)>> = sheet.rules.iter().map(|r| r.iter().map(|w| (w.name, w.value)).collect()).collect();
        assert_eq!(names[1], [("stroke", "blue"), ("fill", "red")]);
        assert_eq!(names[2], [("fill", "lime --> .g")], "a block left open runs to the end");
        // A `}` in a string closes nothing, nor does a `{` in a comment, a string or brackets open anything,
        // and a `/` alone starts no comment.
        let mut quoted = Sheet::default();
        let text = r#"--> .q { font-family: "a}b"; fill: red } @media /* { */ print { } .r{fill:blue} /* @font-face */ @import url(a/b.css);
                      .s /* { */ { /* } */ fill: lime } [title="{"], .t[x={], .u { fill: navy } .v, .w/x { fill: teal }
                      [title="["], .y { fill: gray } .z { font: 12px/2 serif } .zz { fill: olive }"#;
        quoted.read(text).unwrap();
        let values: Vec<Vec<&str>> = quoted.rules.iter().map(|r| r.iter().map(|w| w.value).collect()).collect();
        let expected =
            [vec![r#""a}b""#, "red"], vec!["blue"], vec!["lime"], vec!["navy"], vec!["teal"], vec!["gray"], vec!["12px/2 serif"], vec!["olive"]];
        assert_eq!(values, expected);
        assert_eq!(quoted.unread, ["@media", "@import", r#"[title="{"]"#, ".t[x={]", ".w/x", r#"[title="["]"#]);
        assert_eq!(quoted.selectors.len(), 8, ".q, .r, .s, .u, .v, .y, .z and .zz");
        let mut broken = Sheet::default();
        for text in [".a{fill:red} .b", "@media", "/* open"] {
            broken.read(text).unwrap();
        }
        assert_eq!((broken.rules.len(), broken.unread.as_slice()), (1, ["@media"].as_slice()));
    }

    #[test]
    fn each_rule_counts_once_at_its_most_specific_selector() {
        let doc = Document::parse(r#"<svg><rect id="x" class="a b" style="stroke:green"/></svg>"#).unwrap();
        let rect = doc.descendants().find(|n| n.has_tag_name("rect")).unwrap();
        let mut sheet = Sheet::default();
        // The first rule matches by its id: more specific than the second, though written first.
        sheet.read("#x, .a {fill:red} .a.b {fill:blue} * {stroke:red} rect.zz {fill:black}").unwrap();
        let mut meter = Budget::DEFAULT.meter();
        let declared = sheet.declared(rect, &mut meter).unwrap();
        assert_eq!((declared.get("fill"), declared.get("stroke")), (Some("red"), Some("green")));
        // 2 classes; tested `#x`, `.a`, `.a.b` (by `a`; `b` keys nothing) and `*`, and `rect.zz` not, keyed
        // by its class; 3 declarations from the rules and 1 from the style attribute.
        assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 2 + 4 + 4);
    }

    #[test]
    fn the_sheets_hold_at_most_a_million_items() {
        // Counted by their separators: here 2 selectors and 3 declarations, and an at-rule.
        let mut sheet = Sheet::default();
        sheet.read(".a, .b {fill:red; ; stroke:blue} @media x;").unwrap();
        assert_eq!(sheet.items, 6);
        let mut full = Sheet { items: MAX_ITEMS - 2, ..Sheet::default() };
        assert_eq!(full.read(".a{fill:red}"), Ok(()));
        assert_eq!(full.read("@media x;"), Err(TooMany));
        let mut full = Sheet { items: MAX_ITEMS - 2, ..Sheet::default() };
        assert_eq!(full.read(".a, .b{fill:red}"), Err(TooMany));
        assert!(full.rules.is_empty(), "nothing is read past the limit");
    }

    #[test]
    fn the_note_names_a_few_and_counts_the_rest() {
        let note = |unread: &[&'static str]| Sheet { unread: unread.to_vec(), ..Sheet::default() }.unread_note();
        assert_eq!(note(&[]), None);
        let long = "g rect rect rect rect rect rect rect rect rect";
        assert_eq!(
            note(&[long]).unwrap(),
            "StitchCraft does not read `g rect rect rect rect rect rect rect rec…` in the file's style sheet; what it sets is not used."
        );
        assert_eq!(short(&"é".repeat(41)), format!("{}…", "é".repeat(40)), "cut at a character");
        assert_eq!(short(&"é".repeat(40)), "é".repeat(40));
        assert_eq!(
            note(&["a b", "@media", "c:d", "e f"]).unwrap(),
            "StitchCraft does not read `a b`, `@media`, `c:d` and 1 more in the file's style sheet; what they set is not used."
        );
    }
}
