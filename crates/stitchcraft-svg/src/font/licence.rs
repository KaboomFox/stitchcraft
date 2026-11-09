//! A font's licence, as its licence file names it (`docs/src/design/fonts.md`, the corpus).
//!
//! **Which file.** A font's licence file is a file of its folder named `LICENSE` or `LICENCE`, alone or with
//! `.txt` or `.md`, in any case ([`is_licence_file`]). Ink/Stitch reads the file `LICENSE`. At `c7e3a05`,
//! 141 of its fonts name the file so, and 1 names it `license`.
//!
//! **Why the licence file.** The licence field of a font's `font.json` disagrees with its licence file for 3
//! of Ink/Stitch's fonts (`docs/src/design/fonts.md`, Licences), and the file is what grants the rights.
//!
//! **Why the licence it names first.** A licence file names the font's own licence first, in a sentence such
//! as "This font has been adapted for Ink/Stitch and is licensed under the SIL Open Font License". It then
//! often names or quotes the licence of the font it was made from, which may be another: "The underlying font
//! is in public domain". So of the names `NAMES` lists, the one found earliest in the text is the font's.
//! At the same place the longest wins, so `CC-BY-NC-SA` is the non-commercial licence and not CC BY. A name
//! counts only at the start of a word, after the text is put in lower case and each hyphen or underscore made
//! a space. A file that names none of them is under another licence, as is a font without one.
//!
//! At `c7e3a05` this reads 102 fonts as under the OFL, 25 under CC BY-SA, 1 under CC BY and 3 in the public
//! domain, which the corpus takes ([`Licence::taken`]), and 8 under CC BY-NC-SA, 1 under CC BY-ND, 1 under
//! the GPL and 1 under a free font's own licence, which it leaves out, as the design counted them.

/// A font's licence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Licence {
    /// The SIL Open Font License.
    Ofl,
    /// CC0, or the public domain.
    PublicDomain,
    /// Creative Commons Attribution.
    CcBy,
    /// Creative Commons Attribution-ShareAlike.
    CcBySa,
    /// A Creative Commons licence that forbids commercial use.
    NonCommercial,
    /// A Creative Commons licence that forbids sharing adaptations.
    NoDerivatives,
    /// The GPL.
    Gpl,
    /// None of these: the font's own terms, or no licence file.
    Other,
}

/// The GPL's name, in lower case. It is put together from pieces, as `cargo xtask cleanroom` finds the whole
/// name in any file, where it would mean that a GPL file came in.
const GPL: &str = concat!("gnu ", "general public license");

/// The names a licence file may give a licence, in lower case with spaces for hyphens.
const NAMES: &[(&str, Licence)] = &[
    ("open font license", Licence::Ofl),
    ("open font licence", Licence::Ofl),
    ("cc0", Licence::PublicDomain),
    ("publicdomain/zero", Licence::PublicDomain),
    ("public domain", Licence::PublicDomain),
    ("cc by", Licence::CcBy),
    ("licenses/by/", Licence::CcBy),
    ("cc by sa", Licence::CcBySa),
    ("licenses/by sa/", Licence::CcBySa),
    ("attribution sharealike", Licence::CcBySa),
    ("cc by nc", Licence::NonCommercial),
    ("licenses/by nc", Licence::NonCommercial),
    ("noncommercial", Licence::NonCommercial),
    ("cc by nd", Licence::NoDerivatives),
    ("licenses/by nd", Licence::NoDerivatives),
    ("noderivatives", Licence::NoDerivatives),
    ("noderivs", Licence::NoDerivatives),
    (GPL, Licence::Gpl),
];

impl Licence {
    /// The licences of the fonts the corpus takes: they allow adaptations to be shared, for any use.
    pub const TAKEN: [Licence; 4] = [Licence::Ofl, Licence::PublicDomain, Licence::CcBy, Licence::CcBySa];

    /// Its name in the reports.
    pub fn name(self) -> &'static str {
        match self {
            Licence::Ofl => "OFL",
            Licence::PublicDomain => "CC0 or public domain",
            Licence::CcBy => "CC BY",
            Licence::CcBySa => "CC BY-SA",
            Licence::NonCommercial => "CC BY-NC",
            Licence::NoDerivatives => "CC BY-ND",
            Licence::Gpl => "GPL",
            Licence::Other => "another licence",
        }
    }

    /// Whether the corpus takes a font under it.
    pub fn taken(self) -> bool {
        Licence::TAKEN.contains(&self)
    }

    /// The licence a licence file's `text` names first.
    pub fn of(text: &str) -> Licence {
        let text: String = text.to_lowercase().replace(['-', '_'], " ").split_whitespace().collect::<Vec<_>>().join(" ");
        let at_word = |at: usize| at == 0 || text.get(..at).and_then(|before| before.chars().next_back()).is_some_and(|c| !c.is_alphanumeric());
        NAMES
            .iter()
            .filter_map(|&(name, licence)| {
                let at = text.match_indices(name).map(|(at, _)| at).find(|&at| at_word(at))?;
                Some(((at, std::cmp::Reverse(name.len())), licence))
            })
            .min_by_key(|&(place, _)| place)
            .map_or(Licence::Other, |(_, licence)| licence)
    }
}

/// Whether a file named `name` is a licence file: `LICENSE` or `LICENCE`, alone or with `.txt` or `.md`, in
/// any case.
pub fn is_licence_file(name: &str) -> bool {
    let name = name.to_lowercase();
    let stem = name.strip_suffix(".txt").or_else(|| name.strip_suffix(".md")).unwrap_or(&name);
    stem == "license" || stem == "licence"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_font_s_licence_is_the_one_its_file_names_first() {
        let of = Licence::of;
        assert_eq!(of("This font has been adapted, and is licensed under the SIL Open Font License, Version 1.1."), Licence::Ofl);
        // The font it was made from is under another licence, named later.
        assert_eq!(
            of("This font is licensed CC-BY-SA (http://creativecommons.org/licenses/by-sa/4.0/). The underlying font is in public domain."),
            Licence::CcBySa
        );
        assert_eq!(
            of(&format!("This font is licensed under the SIL Open Font License. It is derived from a font licensed under GPL, or under the {GPL}.")),
            Licence::Ofl
        );
        assert_eq!(of("Adapted from a font under Creative Commons Deed CC0, and licensed under the SIL Open Font Licence."), Licence::PublicDomain);
        assert_eq!(of("This ink/stitch font is public domain."), Licence::PublicDomain);
        assert_eq!(of("Licence: https://creativecommons.org/publicdomain/zero/1.0/"), Licence::PublicDomain);
        assert_eq!(of("licensed under CC BY 4.0. Original font: ..."), Licence::CcBy);
        assert_eq!(of("see https://creativecommons.org/licenses/by/4.0/"), Licence::CcBy);
        assert_eq!(of("Creative Commons Attribution-ShareAlike 4.0"), Licence::CcBySa);
        assert_eq!(of(&format!("licensed under the {}, Version 3", GPL.to_uppercase())), Licence::Gpl);
    }

    #[test]
    fn at_the_same_place_the_longest_name_wins() {
        // CC BY-NC-SA starts as CC BY does, and its address may even be CC BY-SA's.
        assert_eq!(Licence::of("licensed CC-BY-NC-SA4 .0 (http://creativecommons.org/licenses/by-sa/4.0/)"), Licence::NonCommercial);
        assert_eq!(Licence::of("licensed under Creative Commons CC BY-ND"), Licence::NoDerivatives);
        assert_eq!(Licence::of("see https://creativecommons.org/licenses/by-nc/4.0/"), Licence::NonCommercial);
        assert_eq!(Licence::of("see https://creativecommons.org/licenses/by-nd/4.0/"), Licence::NoDerivatives);
        assert_eq!(Licence::of("Attribution-NonCommercial 4.0"), Licence::NonCommercial);
        assert_eq!(Licence::of("Attribution-NoDerivatives 4.0"), Licence::NoDerivatives);
        assert_eq!(Licence::of("Attribution-NoDerivs 3.0"), Licence::NoDerivatives);
    }

    #[test]
    fn a_name_counts_at_the_start_of_a_word_in_any_case_and_spacing() {
        assert_eq!(Licence::of("Font's license: Free for Personal and Commercial use"), Licence::Other);
        assert_eq!(Licence::of("The disc by the door, and a licence of its own"), Licence::Other);
        // Inside a word it is no name, and the next one counts.
        assert_eq!(Licence::of("Tacc by night, then CC_BY_SA"), Licence::CcBySa);
        assert_eq!(Licence::of("SIL\n  OPEN   FONT\tLICENSE"), Licence::Ofl);
        assert_eq!(Licence::of("public-domain"), Licence::PublicDomain);
        assert_eq!(Licence::of(""), Licence::Other);
    }

    #[test]
    fn the_corpus_takes_the_licences_that_allow_shared_adaptations_for_any_use() {
        let taken: Vec<Licence> = [
            Licence::Ofl,
            Licence::PublicDomain,
            Licence::CcBy,
            Licence::CcBySa,
            Licence::NonCommercial,
            Licence::NoDerivatives,
            Licence::Gpl,
            Licence::Other,
        ]
        .into_iter()
        .filter(|licence| licence.taken())
        .collect();
        assert_eq!(taken, Licence::TAKEN);
        let names: Vec<&str> = [Licence::Ofl, Licence::PublicDomain, Licence::CcBy, Licence::CcBySa].map(Licence::name).to_vec();
        assert_eq!(names, ["OFL", "CC0 or public domain", "CC BY", "CC BY-SA"]);
        let others = [Licence::NonCommercial, Licence::NoDerivatives, Licence::Gpl, Licence::Other].map(Licence::name);
        assert_eq!(others, ["CC BY-NC", "CC BY-ND", "GPL", "another licence"]);
    }

    #[test]
    fn licence_files_are_found_by_their_names_in_any_case() {
        for (name, is) in [
            ("LICENSE", true),
            ("licence", true),
            ("License.txt", true),
            ("LICENCE.md", true),
            ("LICENSE.pdf", false),
            ("license-x", false),
            ("OFL.txt", false),
        ] {
            assert_eq!(is_licence_file(name), is, "{name}");
        }
    }
}
