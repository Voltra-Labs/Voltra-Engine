//! A small, seedable generator for particle spreads.
//!
//! xorshift64*: a few instructions per value, good enough that a fountain
//! does not visibly repeat, and deterministic from a seed so a test can pin
//! what an emitter does. Not for anything that needs to be unpredictable.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rng(u64);

impl Rng {
    /// A generator from `seed`. Zero, the one state xorshift cannot leave, is
    /// moved off.
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A value in `0..1`.
    pub fn unit(&mut self) -> f32 {
        // The top 24 bits: exactly what an f32's mantissa holds.
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A value between `a` and `b`, either order.
    pub fn between(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_stay_in_the_unit_range_and_vary() {
        let mut rng = Rng::new(7);
        let values: Vec<f32> = (0..1000).map(|_| rng.unit()).collect();
        assert!(values.iter().all(|v| (0.0..1.0).contains(v)));
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        assert!((0.4..0.6).contains(&mean), "{mean}");
    }

    #[test]
    fn one_seed_is_one_sequence() {
        let (mut a, mut b) = (Rng::new(3), Rng::new(3));
        assert_eq!(a.next_u64(), b.next_u64());
        assert_ne!(Rng::new(3).next_u64(), Rng::new(4).next_u64());
    }

    #[test]
    fn a_zero_seed_still_moves() {
        let mut rng = Rng::new(0);
        assert_ne!(rng.next_u64(), rng.next_u64());
    }
}
