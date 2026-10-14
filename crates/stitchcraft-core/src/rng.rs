//! The one random number generator StitchCraft uses.
//!
//! Embroidery uses randomness for texture — jittered stitch lengths, randomized split points — but the
//! same design must still produce the same file every time, on every machine. So randomness is a pure
//! function of a seed:
//!
//! - [`SplitMix64`] (Steele, Lea & Flood, "Fast splittable pseudorandom number generators", OOPSLA
//!   2014) is fully specified by a 64-bit state update. Its output can be frozen in tests and will never
//!   change with a dependency upgrade.
//! - [`SplitMix64::for_element`] derives a per-element generator from the element's `random_seed` when it
//!   has one, and otherwise from its stable id, so editing one element never changes another element's
//!   stitches. A seed replaces the id, as in Ink/Stitch: 2 elements given one seed draw the same numbers.
//! - [`SplitMix64::stream`] keys a generator for one part of an element, so editing one part never
//!   changes another part's numbers either: a running stitch draws each span's lengths from its own.

/// SplitMix64 pseudorandom number generator. Deterministic, fast, and statistically adequate for
/// jitter; not for cryptography.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
const FNV_OFFSET_BASIS: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

impl SplitMix64 {
    /// A generator starting from `seed`.
    pub const fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    /// The generator for one element: from its `random_seed` parameter, `seed`, when it has one, and
    /// otherwise from its stable id. A seed is mixed once, so that seeds a few apart draw unrelated numbers:
    /// the state is the first number a generator started from it draws. Without one, it is the id's hash
    /// (FNV-1a) joined by exclusive or with the first number a generator started from 0 draws.
    pub fn for_element(element_id: &str, seed: Option<u64>) -> Self {
        match seed {
            Some(seed) => SplitMix64::new(SplitMix64::new(seed).next_u64()),
            None => SplitMix64::new(fnv1a64(element_id.as_bytes()) ^ SplitMix64::new(0).next_u64()),
        }
    }

    /// Stream `key` of this generator: a generator whose numbers depend on this one's state and on `key`
    /// alone. Making one draws nothing from this generator, so streams keyed by the parts of an element,
    /// such as the spans of a running stitch, keep each part's numbers as they were when another part
    /// changes. The key is spread by the golden gamma, joined to the state by exclusive or, and mixed
    /// once, as [`SplitMix64::for_element`] mixes a seed.
    #[must_use]
    pub fn stream(&self, key: u64) -> Self {
        SplitMix64::new(SplitMix64::new(self.state ^ key.wrapping_mul(GOLDEN_GAMMA)).next_u64())
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniformly distributed number in `[0, 1)`, using the top 53 bits.
    pub fn next_f64(&mut self) -> f64 {
        const SCALE: f64 = 1.0 / (1u64 << 53) as f64;
        (self.next_u64() >> 11) as f64 * SCALE
    }

    /// A uniformly distributed number in `[low, high)`; `low` if the range is empty or not finite. Where
    /// `low` plus a share of the span rounds up to `high`, the number below `high` is given instead.
    pub fn next_in(&mut self, low: f64, high: f64) -> f64 {
        let span = high - low;
        if span.is_finite() && span > 0.0 { (low + self.next_f64() * span).min(high.next_down()) } else { low }
    }
}

/// FNV-1a, 64-bit: a tiny, fully specified hash for deriving seeds from ids.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV_OFFSET_BASIS, |hash, &b| (hash ^ u64::from(b)).wrapping_mul(FNV_PRIME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_matches_the_reference_sequence() {
        // First outputs for seed 0 of the reference implementation (Vigna, splitmix64.c).
        let mut rng = SplitMix64::new(0);
        assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(rng.next_u64(), 0x06C4_5D18_8009_454F);
    }

    #[test]
    fn fnv1a64_matches_published_vectors() {
        assert_eq!(fnv1a64(b""), 0xCBF2_9CE4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xAF63_DC4C_8601_EC8C);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_F739_67E8);
    }

    #[test]
    fn floats_stay_in_range() {
        let mut rng = SplitMix64::new(42);
        for _ in 0..10_000 {
            let x = rng.next_f64();
            assert!((0.0..1.0).contains(&x));
            let y = rng.next_in(-2.0, 3.0);
            assert!((-2.0..3.0).contains(&y));
        }
        assert_eq!(rng.next_in(1.0, 1.0), 1.0);
        assert_eq!(rng.next_in(0.0, f64::NAN), 0.0);
        // Where the floats lie 2 apart, low plus a share of the span rounds to high half the time: the
        // number below high is taken instead.
        assert!((0..2_000).all(|seed| SplitMix64::new(seed).next_in(1e16, 1e16 + 2.0) == 1e16));
    }

    #[test]
    fn a_float_is_the_next_number_s_top_53_bits() {
        // Seed 0's first number, 0xE220A8397B1DCDAF, as a share of 2^64 cut to 53 bits; from -2 to 3, -2
        // plus 5 times that.
        assert_eq!(SplitMix64::new(0).next_f64(), 0.883_310_808_213_642_6);
        assert_eq!(SplitMix64::new(0).next_in(-2.0, 3.0), 2.416_554_041_068_213_4);
        // An empty, reversed or endless range gives its low end and draws nothing: the next draw is the
        // generator's first.
        for (low, high) in [(1.0, 1.0), (5.0, 2.0), (0.0, f64::INFINITY), (0.0, f64::NAN)] {
            let mut rng = SplitMix64::new(0);
            assert_eq!(rng.next_in(low, high), low, "{low}..{high}");
            assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF, "{low}..{high}");
        }
    }

    #[test]
    fn element_generators_are_stable_and_independent() {
        let a1 = SplitMix64::for_element("vc:42", None).next_u64();
        let a2 = SplitMix64::for_element("vc:42", None).next_u64();
        let b = SplitMix64::for_element("vc:43", None).next_u64();
        assert_eq!(a1, a2, "same element: same stream");
        assert_ne!(a1, b, "different elements: different streams");
        // The id's FNV-1a hash and the first number from 0 joined by exclusive or: a vector worked out from
        // both published definitions.
        assert_eq!(a1, 0x4D48_729A_411D_5424);
    }

    #[test]
    fn a_seed_replaces_the_element_s_id() {
        // 2 elements with one seed draw the same numbers, and another seed others; 0 is a seed like any other.
        let seeded = |id: &str, seed: u64| SplitMix64::for_element(id, Some(seed)).next_u64();
        assert_eq!(seeded("vc:42", 7), seeded("vc:43", 7));
        assert_ne!(seeded("vc:42", 7), seeded("vc:42", 8));
        assert_ne!(seeded("vc:42", 0), SplitMix64::for_element("vc:42", None).next_u64());
        // The state is the first number from the seed: from 0, 0xE220A8397B1DCDAF, whose own first number
        // the reference implementation gives as the generator's first.
        assert_eq!(SplitMix64::for_element("any", Some(0)), SplitMix64::new(0xE220_A839_7B1D_CDAF));
    }

    #[test]
    fn a_stream_depends_on_the_state_and_its_key_alone() {
        // From state 0, stream 1 starts from the golden gamma mixed once: the second number seed 0 draws.
        // Its first number, and that of stream 3 of stream 2, are worked out from the published definition.
        let base = SplitMix64::new(0);
        assert_eq!(base.stream(1), SplitMix64::new(0x6E78_9E6A_A1B9_65F4));
        assert_eq!(base.stream(1).next_u64(), 0x46B7_3E79_F0C3_7C00);
        assert_eq!(base.stream(2).stream(3).next_u64(), 0x71CC_622D_271A_84F0);
        // Other keys, or another state, give other streams.
        assert_ne!(base.stream(1).next_u64(), base.stream(2).next_u64());
        assert_ne!(base.stream(1), SplitMix64::new(1).stream(1));
    }
}
