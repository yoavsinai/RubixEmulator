use std::time::{SystemTime, UNIX_EPOCH};

/// A small, dependency-free xorshift64* generator — scrambling is the only place this
/// project needs randomness, so it isn't worth pulling in a whole RNG crate for it.
pub struct Rng(u64);

impl Default for Rng {
    fn default() -> Self {
        Rng::new()
    }
}

impl Rng {
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Rng(seed | 1) // xorshift needs a nonzero state
    }

    /// A pseudo-random value in `0..bound`.
    pub fn next(&mut self, bound: usize) -> usize {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) % bound as u64) as usize
    }
}
