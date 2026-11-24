//! A small seeded generator: simulated loss has to replay the same way in a
//! failing test, and nothing here needs a cryptographic source.

/// SplitMix64 (Steele, Lea and Flood): one add and three mixes a number,
/// every 64-bit state reached, good enough for dice.
#[derive(Debug, Clone)]
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub(crate) fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// True with probability `p`, clamped to `[0, 1]`; NaN is never.
    pub(crate) fn chance(&mut self, p: f32) -> bool {
        p > 0.0 && self.unit() < f64::from(p.min(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_numbers() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn chances_land_near_their_probability() {
        let mut rng = Rng::new(1);
        let hits = (0..10_000).filter(|_| rng.chance(0.25)).count();
        assert!((2_200..2_800).contains(&hits), "{hits}");
        assert!(!rng.chance(0.0));
        assert!(!rng.chance(f32::NAN));
        assert!(rng.chance(1.0));
    }
}
