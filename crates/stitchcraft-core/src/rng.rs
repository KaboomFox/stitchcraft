//! The one random number generator StitchCraft uses.
//!
//! Embroidery uses randomness for texture — jittered stitch lengths, randomized split points — but the
//! same design must still produce the same file every time, on every machine. So randomness is a pure
//! function of a seed:
//!
//! - [`SplitMix64`] (Steele, Lea & Flood, "Fast splittable pseudorandom number generators", OOPSLA
//!   2014) is fully specified by a 64-bit state update. Its output can be frozen in tests and will never
//!   change with a dependency upgrade.
//! - [`SplitMix64::for_element`] derives a per-element generator from the element's stable id and the
//!   user's `random_seed` parameter, so editing one element never changes another element's stitches.

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

    /// The generator for one element: seeded from the element's stable id (FNV-1a) mixed with the
    /// user's `random_seed` parameter (0 when the user left it empty).
    pub fn for_element(element_id: &str, user_seed: u64) -> Self {
        let id_hash = fnv1a64(element_id.as_bytes());
        let mixed = SplitMix64::new(user_seed).next_u64();
        SplitMix64::new(id_hash ^ mixed)
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

    /// A uniformly distributed number in `[low, high)`; `low` if the range is empty or not finite.
    pub fn next_in(&mut self, low: f64, high: f64) -> f64 {
        let span = high - low;
        if span.is_finite() && span > 0.0 { low + self.next_f64() * span } else { low }
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
    }

    #[test]
    fn element_generators_are_stable_and_independent() {
        let a1 = SplitMix64::for_element("vc:42", 0).next_u64();
        let a2 = SplitMix64::for_element("vc:42", 0).next_u64();
        let b = SplitMix64::for_element("vc:43", 0).next_u64();
        let a_seeded = SplitMix64::for_element("vc:42", 7).next_u64();
        assert_eq!(a1, a2, "same element, same seed: same stream");
        assert_ne!(a1, b, "different elements: different streams");
        assert_ne!(a1, a_seeded, "the user's seed changes the stream");
    }
}
