//! Random numbers by counter: every draw is a function of what it is for (the seed, the step, the
//! transition, the cell, the draw's index), not of the draws before it. The same cell draws the same numbers
//! in any tile, on any worker, in any order: the simulation does not depend on how the map is cut.

/// SplitMix64's finaliser: a bijection of 64 bits with full avalanche.
#[inline]
pub fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A 64-bit hash of a tuple of words.
#[inline]
pub fn hash(parts: &[u64]) -> u64 {
    let mut h = 0x243F_6A88_85A3_08D3u64;
    for &p in parts {
        h = mix(h.wrapping_add(0x9E37_79B9_7F4A_7C15) ^ mix(p.wrapping_add(0x9E37_79B9_7F4A_7C15)));
    }
    h
}

/// A uniform number in (0, 1) from a hash (53 bits; never 0 or 1).
#[inline]
pub fn unit(h: u64) -> f64 {
    ((h >> 11) as f64 + 0.5) * (1.0 / (1u64 << 53) as f64)
}

/// The uniform of a tuple.
#[inline]
pub fn uniform(parts: &[u64]) -> f64 {
    unit(hash(parts))
}

/// A stream of numbers of its own (e.g. one patch's draws): `key` names it, each draw has its index.
#[derive(Debug, Clone)]
pub struct Stream {
    key: u64,
    n: u64,
}

impl Stream {
    pub fn new(parts: &[u64]) -> Self {
        Stream { key: hash(parts), n: 0 }
    }

    pub fn uniform(&mut self) -> f64 {
        self.n += 1;
        unit(hash(&[self.key, self.n]))
    }

    /// A standard normal (Box-Muller).
    pub fn normal(&mut self) -> f64 {
        let (u, v) = (self.uniform(), self.uniform());
        (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniforms_are_uniform() {
        let n = 200_000;
        let mut bins = [0usize; 10];
        let mut sum = 0.0;
        for i in 0..n {
            let u = uniform(&[7, i]);
            assert!(u > 0.0 && u < 1.0);
            bins[(u * 10.0) as usize] += 1;
            sum += u;
        }
        assert!((sum / n as f64 - 0.5).abs() < 0.005);
        for b in bins {
            assert!((b as f64 - n as f64 / 10.0).abs() < 0.03 * n as f64 / 10.0, "{bins:?}");
        }
    }

    #[test]
    fn the_same_parts_give_the_same_numbers() {
        assert_eq!(uniform(&[1, 2, 3]), uniform(&[1, 2, 3]));
        assert_ne!(uniform(&[1, 2, 3]), uniform(&[1, 3, 2]));
        let (mut a, mut b) = (Stream::new(&[5, 9]), Stream::new(&[5, 9]));
        assert_eq!((a.uniform(), a.normal()), (b.uniform(), b.normal()));
    }

    #[test]
    fn normals_are_normal() {
        let mut s = Stream::new(&[42]);
        let n = 100_000;
        let xs: Vec<f64> = (0..n).map(|_| s.normal()).collect();
        let m = xs.iter().sum::<f64>() / n as f64;
        let v = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / n as f64;
        assert!(m.abs() < 0.02 && (v - 1.0).abs() < 0.02, "{m} {v}");
    }
}
