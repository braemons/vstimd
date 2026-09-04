//! The dot field's random number generator.
//!
//! In-tree, and deliberately so. An RDK is a *sample*: the config records a seed,
//! and replaying that config years later has to put every dot back where it was.
//! That makes the generator's output stream part of the file format, not an
//! implementation detail — so it is written here, frozen by the test vector at the
//! bottom of this file, rather than inherited from a dependency whose stream is
//! free to change when the dependency does.
//!
//! The algorithm is PCG-XSH-RR 64/32 (O'Neill 2014) with the reference multiplier
//! and increment, seeded through SplitMix64 so that adjacent seeds (`0`, `1`, `2` —
//! which is exactly how seeds get written by hand) do not produce correlated
//! streams. It is small, fast, and has no state to synchronise: `f32_01` and
//! `unit_vector` are the only two things the dot update asks of it.

/// PCG-XSH-RR 64/32. Not cryptographic; nothing here needs it to be.
#[derive(Clone)]
pub struct DotsRng {
    state: u64,
}

const PCG_MULT: u64 = 6364136223846793005;
const PCG_INC: u64 = 1442695040888963407;

impl DotsRng {
    /// The stream belonging to one dot of a field.
    ///
    /// Each dot draws from its own stream rather than from a shared one walked in
    /// index order, so that touching one dot cannot move any other dot's position
    /// in the stream. That is what makes dot `i`'s state a function of the field
    /// seed, `i`, and how long the dot has lived — and nothing else. A single
    /// shared stream would make it depend on *when* a `SetDotCount` arrived,
    /// because growing the field consumed draws that the surviving dots would
    /// otherwise have taken.
    ///
    /// The index is mixed through SplitMix64 before it meets the seed, so that
    /// adjacent dots get independent streams for the same reason adjacent seeds do.
    pub fn for_dot(seed: u64, index: usize) -> Self {
        Self::new(seed ^ splitmix64(index as u64).rotate_left(32))
    }

    /// Seed the generator, whitening through SplitMix64 first.
    pub fn new(seed: u64) -> Self {
        let mut rng = Self { state: splitmix64(seed) };
        // Discard one output: PCG's first output after seeding is a function of the
        // seed alone, so a run of small seeds would start with a run of small values.
        rng.next_u32();
        rng
    }

    pub fn next_u32(&mut self) -> u32 {
        let state = self.state;
        self.state = state.wrapping_mul(PCG_MULT).wrapping_add(PCG_INC);
        // XSH: xor high bits down, then RR: rotate by the top 5 bits.
        let xorshifted = (((state >> 18) ^ state) >> 27) as u32;
        let rot = (state >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform in `[0, 1)`, with 24 bits of mantissa — every value exactly
    /// representable in `f32`, so the result is the same on every platform.
    pub fn f32_01(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// Uniform in `[lo, hi)`.
    pub fn f32_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.f32_01() * (hi - lo)
    }

    /// A unit vector with uniformly distributed direction.
    ///
    /// Drawn from the angle rather than by rejection sampling in a disc: rejection
    /// consumes a variable number of outputs per call, which would make the stream
    /// position at frame N depend on the values drawn rather than on N alone.
    pub fn unit_vector(&mut self) -> [f32; 2] {
        let theta = self.f32_01() * std::f32::consts::TAU;
        [theta.cos(), theta.sin()]
    }

    /// True with probability `p`. `p <= 0` is never, `p >= 1` is always — but the
    /// draw happens either way, so the stream position does not depend on `p`.
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32_01() < p
    }
}

fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The output stream is part of the scene-config format.** A config records a
    /// seed and nothing else about the sample; if this vector ever changes, every
    /// config saved before the change replays a different stimulus, silently. Update
    /// it only alongside a config version bump that says so.
    #[test]
    fn stream_is_frozen() {
        let mut rng = DotsRng::new(0);
        let got: Vec<u32> = (0..8).map(|_| rng.next_u32()).collect();
        assert_eq!(
            got,
            vec![
                278790474, 1039822109, 1377468856, 2033553421, 812736149, 2537966385,
                2065831338, 1112633243
            ],
            "the RDK random stream changed — every saved config now replays differently"
        );
    }

    #[test]
    fn f32_01_stays_in_range() {
        let mut rng = DotsRng::new(12345);
        for _ in 0..10_000 {
            let v = rng.f32_01();
            assert!((0.0..1.0).contains(&v), "{v} out of [0, 1)");
        }
    }

    /// Adjacent seeds must not give correlated streams — seeds get typed by hand as
    /// 0, 1, 2, and two conditions differing only in seed must be independent samples.
    #[test]
    fn adjacent_seeds_diverge() {
        let first: Vec<u32> = (0..4u64)
            .map(|s| DotsRng::new(s).next_u32())
            .collect();
        let mut sorted = first.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), first.len(), "adjacent seeds collided");
        // And not merely distinct: they must not be clustered the way raw
        // consecutive PCG seeds are.
        let spread = sorted[sorted.len() - 1] - sorted[0];
        assert!(spread > u32::MAX / 8, "adjacent seeds gave a clustered stream");
    }

    /// Adjacent dots of the same field must not be correlated — dot 0 and dot 1 of
    /// seed 7 are two independent samples, not one sample and its neighbour.
    #[test]
    fn adjacent_dots_get_independent_streams() {
        let first: Vec<u32> = (0..8).map(|i| DotsRng::for_dot(7, i).next_u32()).collect();
        let mut sorted = first.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), first.len(), "two dots of one field collided");
        assert!(sorted[sorted.len() - 1] - sorted[0] > u32::MAX / 8, "clustered");
    }

    /// The same dot of the same field is the same stream, however the field got to
    /// that size. This is the property a `SetDotCount` must not disturb.
    #[test]
    fn a_dots_stream_depends_only_on_seed_and_index() {
        let a: Vec<u32> = (0..4).map(|_| DotsRng::for_dot(3, 99).next_u32()).collect();
        assert!(a.iter().all(|v| *v == a[0]));
        assert_ne!(DotsRng::for_dot(3, 99).next_u32(), DotsRng::for_dot(4, 99).next_u32());
        assert_ne!(DotsRng::for_dot(3, 99).next_u32(), DotsRng::for_dot(3, 98).next_u32());
    }

    #[test]
    fn unit_vectors_have_unit_length() {
        let mut rng = DotsRng::new(7);
        for _ in 0..1000 {
            let [x, y] = rng.unit_vector();
            assert!((x * x + y * y - 1.0).abs() < 1e-5);
        }
    }

    /// A stream reproduces exactly from the same seed — the property the whole
    /// stimulus rests on.
    #[test]
    fn same_seed_same_stream() {
        let a: Vec<u32> = { let mut r = DotsRng::new(99); (0..64).map(|_| r.next_u32()).collect() };
        let b: Vec<u32> = { let mut r = DotsRng::new(99); (0..64).map(|_| r.next_u32()).collect() };
        assert_eq!(a, b);
    }
}
