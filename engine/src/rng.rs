/// Seedable xoshiro256** PRNG + Box-Muller normal transform.
///
/// We implement our own RNG to avoid external dependencies and ensure
/// identical sequences across WASM and native builds.

/// xoshiro256** pseudo-random number generator.
///
/// Fast, high-quality 64-bit PRNG. Period: 2^256 - 1.
/// Reference: https://prng.di.unimi.it/xoshiro256starstar.c
#[derive(Debug, Clone)]
pub struct Xoshiro256StarStar {
    s: [u64; 4],
}

impl Xoshiro256StarStar {
    /// Create a new PRNG from a 64-bit seed.
    /// Uses SplitMix64 to expand the seed into the full state.
    pub fn new(seed: u64) -> Self {
        let mut sm = SplitMix64(seed);
        Xoshiro256StarStar {
            s: [sm.next(), sm.next(), sm.next(), sm.next()],
        }
    }

    /// Generate the next random u64.
    pub fn next_u64(&mut self) -> u64 {
        let result = (self.s[1].wrapping_mul(5)).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;

        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];

        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);

        result
    }

    /// Generate a uniform f64 in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        // Use top 53 bits for a uniform double in [0, 1)
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Generate a standard normal sample using Box-Muller transform.
    pub fn next_normal(&mut self) -> f64 {
        // Box-Muller: generate two uniforms, return one normal.
        // We discard the second to keep the API simple.
        loop {
            let u1 = self.next_f64();
            let u2 = self.next_f64();
            if u1 > 0.0 {
                let r = (-2.0 * u1.ln()).sqrt();
                let theta = 2.0 * std::f64::consts::PI * u2;
                return r * theta.cos();
            }
            // u1 == 0.0 is astronomically rare, but retry if it happens
        }
    }

    /// Fill a slice with standard normal samples.
    pub fn fill_normal(&mut self, out: &mut [f64]) {
        // Box-Muller generates pairs; use both values for efficiency.
        let mut i = 0;
        while i + 1 < out.len() {
            loop {
                let u1 = self.next_f64();
                let u2 = self.next_f64();
                if u1 > 0.0 {
                    let r = (-2.0 * u1.ln()).sqrt();
                    let theta = 2.0 * std::f64::consts::PI * u2;
                    out[i] = r * theta.cos();
                    out[i + 1] = r * theta.sin();
                    break;
                }
            }
            i += 2;
        }
        // Handle odd length
        if i < out.len() {
            out[i] = self.next_normal();
        }
    }
}

/// SplitMix64 — used only for seed expansion.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic() {
        let mut rng1 = Xoshiro256StarStar::new(42);
        let mut rng2 = Xoshiro256StarStar::new(42);
        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn test_different_seeds() {
        let mut rng1 = Xoshiro256StarStar::new(42);
        let mut rng2 = Xoshiro256StarStar::new(99);
        // Very unlikely to be equal
        let v1: Vec<u64> = (0..10).map(|_| rng1.next_u64()).collect();
        let v2: Vec<u64> = (0..10).map(|_| rng2.next_u64()).collect();
        assert_ne!(v1, v2);
    }

    #[test]
    fn test_f64_range() {
        let mut rng = Xoshiro256StarStar::new(42);
        for _ in 0..10000 {
            let v = rng.next_f64();
            assert!(v >= 0.0 && v < 1.0);
        }
    }

    #[test]
    fn test_normal_distribution() {
        let mut rng = Xoshiro256StarStar::new(42);
        let n = 100_000;
        let samples: Vec<f64> = (0..n).map(|_| rng.next_normal()).collect();

        let mean = samples.iter().sum::<f64>() / n as f64;
        let variance =
            samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;

        // Mean should be ~0, variance ~1 (with some tolerance)
        assert!(mean.abs() < 0.02, "mean = {}", mean);
        assert!((variance - 1.0).abs() < 0.05, "variance = {}", variance);
    }

    #[test]
    fn test_fill_normal() {
        let mut rng = Xoshiro256StarStar::new(42);
        let mut buf = vec![0.0; 11]; // odd length
        rng.fill_normal(&mut buf);
        // All values should be finite
        for v in &buf {
            assert!(v.is_finite());
        }
    }

    #[test]
    fn test_fill_normal_deterministic() {
        let mut rng1 = Xoshiro256StarStar::new(42);
        let mut rng2 = Xoshiro256StarStar::new(42);
        let mut buf1 = vec![0.0; 10];
        let mut buf2 = vec![0.0; 10];
        rng1.fill_normal(&mut buf1);
        rng2.fill_normal(&mut buf2);
        assert_eq!(buf1, buf2);
    }
}
