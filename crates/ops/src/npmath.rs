//! numpy's float32 exp, sin and cos, lane by lane: numpy computes them with its own SIMD approximations (the
//! AVX2 + FMA3 kernels of numpy 2.x, `loops_exponent_log.dispatch.c.src` and `loops_trigonometric.dispatch.cpp`),
//! not the C library's, so the InVEST models that landschaft runs through numpy get these bits. Other float32
//! functions numpy takes from the C library (atanf, powf, log10f): Rust's are the same calls.

/// numpy's float32 exp (Cody-Waite reduction, a 5/2 rational minimax approximation; at most 2.52 ULP).
pub fn exp(x: f32) -> f32 {
    const XMAX: f32 = f32::from_bits(0x42b1_7218);
    const XMIN: f32 = f32::from_bits(0xc2cf_f1b5);
    const C1: f32 = f32::from_bits(0xbf31_7200);
    const C2: f32 = f32::from_bits(0xb5bf_be8e);
    const P: [u32; 6] = [0x3f80_0000, 0x3f39_cbd5, 0x3e7d_4c58, 0x3d51_7d8c, 0x3bdd_7159, 0x3a05_3dd8];
    const Q: [u32; 3] = [0x3f80_0000, 0xbe8c_6857, 0x3cb0_e832];
    let (p, q_) = (P.map(f32::from_bits), Q.map(f32::from_bits));
    const MAGIC: f32 = 12_582_912.0; // 0x1.8p23
    if x.is_nan() {
        return f32::NAN;
    }
    if x >= XMAX {
        return f32::INFINITY;
    }
    if x <= XMIN {
        return 0.0;
    }
    let q = (x * std::f32::consts::LOG2_E + MAGIC) - MAGIC;
    let mut r = q.mul_add(C1, x);
    r = q.mul_add(C2, r);
    r = q.mul_add(0.0, r);
    let mut num = p[5].mul_add(r, p[4]);
    num = num.mul_add(r, p[3]);
    num = num.mul_add(r, p[2]);
    num = num.mul_add(r, p[1]);
    num = num.mul_add(r, p[0]);
    let mut den = q_[2].mul_add(r, q_[1]);
    den = den.mul_add(r, q_[0]);
    let poly = num / den;
    // poly x 2^q by the exponent's bits; below 2^-125 through a division (a denormal)
    if q <= -125.0 {
        let two = (1i32 << (-(q + 125.0)).round_ties_even() as i32) as f32;
        let p = f32::from_bits((poly.to_bits() as i32).wrapping_add(-125 << 23) as u32);
        return p / two;
    }
    f32::from_bits((poly.to_bits() as i32).wrapping_add((q.round_ties_even() as i32) << 23) as u32)
}

/// numpy's power of an array by a scalar exponent: its fast paths (0.5 a square root, 2 a square, -1 a
/// reciprocal), the C library's pow otherwise.
pub fn pow(x: f64, e: f64) -> f64 {
    match e {
        0.5 => x.sqrt(),
        2.0 => x * x,
        -1.0 => 1.0 / x,
        _ => x.powf(e),
    }
}

/// [`pow`] in float32.
pub fn powf(x: f32, e: f32) -> f32 {
    match e {
        0.5 => x.sqrt(),
        2.0 => x * x,
        -1.0 => 1.0 / x,
        _ => x.powf(e),
    }
}

/// numpy's float32 sine.
pub fn sin(x: f32) -> f32 {
    sincos(x, false)
}

/// numpy's float32 cosine.
pub fn cos(x: f32) -> f32 {
    sincos(x, true)
}

fn sincos(x: f32, cosine: bool) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    // beyond Cody-Waite's range, the C library's
    let max = if cosine { f32::from_bits(0x478b_9a08) } else { f32::from_bits(0x47e5_5dff) };
    if x.abs() > max {
        return if cosine { x.cos() } else { x.sin() };
    }
    const TWO_OVER_PI: f32 = f32::from_bits(0x3f22_f983); // 0x1.45f306p-1
    const PIO2_HIGH: f32 = f32::from_bits(0xbfc9_0fd8); // -0x1.921fb0p+00
    const PIO2_MED: f32 = f32::from_bits(0xb4a8_885a); // -0x1.5110b4p-22
    const PIO2_LOW: f32 = f32::from_bits(0xa7c2_34c4); // -0x1.846988p-48
    const MAGIC: f32 = 12_582_912.0;
    let q = (x * TWO_OVER_PI + MAGIC) - MAGIC;
    let mut r = q.mul_add(PIO2_HIGH, x);
    r = q.mul_add(PIO2_MED, r);
    r = q.mul_add(PIO2_LOW, r);
    let r2 = r * r;
    let mut c = f32::from_bits(0x37cc_730b).mul_add(r2, f32::from_bits(0xbab6_036e)); // 0x1.98e616p-16, -0x1.6c06dcp-10
    c = c.mul_add(r2, f32::from_bits(0x3d2a_aa9e)); // 0x1.55553cp-05
    c = c.mul_add(r2, -0.5);
    c = c.mul_add(r2, 1.0);
    let mut s = f32::from_bits(0x363e_9dde).mul_add(r2, f32::from_bits(0xb950_35dd)); // 0x1.7d3bbcp-19, -0x1.a06bbap-13
    s = s.mul_add(r2, f32::from_bits(0x3c08_88cd)); // 0x1.11119ap-07
    s = s.mul_add(r2, f32::from_bits(0xbe2a_aaab)); // -0x1.555556p-03
    s = s.mul_add(r2, 0.0);
    s = s.mul_add(r, r);
    let mut k = q.round_ties_even() as i32;
    if cosine {
        k += 1;
    }
    let v = if k & 1 == 0 { s } else { c };
    if k & 2 == 2 { 0.0 - v } else { v }
}

#[cfg(test)]
mod tests {
    // numpy 2.5's float32 results (np.exp, np.sin, np.cos on an AVX2 machine)
    const X: [u32; 86] = [
        0x41704e35, 0x428ae2b1, 0x42348c6e, 0xc281eac9, 0xc247ddf2, 0x42816bdf, 0xc2d9e4d5, 0x4258fb95, 0x4245a7d2,
        0xc1834dd8, 0xc24592f6, 0xc259426f, 0xc26c1ab5, 0xc1a7e0bf, 0xc1117211, 0x3f33107e, 0x42b2333b, 0x42422129,
        0x4166f93a, 0x42af9585, 0xc285e064, 0xc29bea4a, 0x41482072, 0xc2ca6c57, 0xc2cdba57, 0xc0e0b628, 0xc1861203,
        0x4292ddfb, 0x417d8626, 0xc0e5a5a6, 0xc12a0148, 0xc271fcf2, 0xc2d7484b, 0xc28f0a05, 0x41e3405b, 0xc28bc1df,
        0xc2105ef7, 0xc2da819d, 0x426009c6, 0xc29e372f, 0xc06dfa73, 0x40c2bae5, 0x3e2069a2, 0x40b1bdad, 0x400f1204,
        0x407792ce, 0xc0d1277d, 0x3f28866c, 0x3dfeae3e, 0x40be2032, 0xc00e10cb, 0x3fc914ba, 0xc0e1a9c5, 0xbfe6214c,
        0xc03535f6, 0xc0b31905, 0x40a1f711, 0xbff6e4ed, 0x40f51e71, 0x3fb84d91, 0x3fd727bc, 0x400d4efa, 0x4034af5f,
        0xc0b2cbea, 0xbf7479de, 0xc08557df, 0xbfc7aef9, 0xc0ce7ccd, 0x40ef8729, 0xc091eafe, 0x402fe335, 0xc04c5eae,
        0x40bf8706, 0x40261b9f, 0xc0bc9cda, 0x40b0ad95, 0x40e3d03f, 0x40cece2e, 0x3f8ec8ea, 0xc0b58646, 0xc2fb0000,
        0xc2fd6666, 0xc3020000, 0x80000000, 0x116c2, 0x48435000,
    ];
    const EXP: [u32; 86] = [
        0x4a4b5f08, 0x71917c01, 0x600b048e, 0x109be9d2, 0x1b710e70, 0x6e2406cb, 0x0, 0x6699467b, 0x631c686e,
        0x339fe694, 0x1bd5d09e, 0x18477e4e, 0x14e5a95a, 0x3053a20b, 0x38ec738d, 0x4000cfdb, 0x7f800000, 0x62818e40,
        0x49e2ff02, 0x7ec9d922, 0xf2c4447, 0x738f244, 0x488410e9, 0x8, 0x2, 0x3a69c9a6, 0x33624fe5, 0x7475f36d,
        0x4ae84d9e, 0x3a485f61, 0x37cbc951, 0x13d305ca, 0x0, 0xbe1d39e, 0x53fccaba, 0xd11a7b0, 0x2573bccc, 0x0,
        0x67df9640, 0x66a3336, 0x3cc6d721, 0x43dbadcf, 0x3f95b51d, 0x43812fba, 0x41159d5d, 0x423f7357, 0x3abe13d4,
        0x3ff73ba8, 0x3f90f319, 0x43be3d45, 0x3dde7b44, 0x4099f54b, 0x3a62f00b, 0x3e299f97, 0x3d715fc0, 0x3b731e72,
        0x431dd013, 0x3e14cd16, 0x45049af7, 0x40870b59, 0x40abda38, 0x41118d56, 0x4186a53f, 0x3b756b05, 0x3ec506e1,
        0x3c7defbc, 0x3e572c6a, 0x3ace98b4, 0x44deb22b, 0x3c2b6be2, 0x4179d7b8, 0x3d281684, 0x43c6c1e7, 0x41567058,
        0x3b34960f, 0x4379ee9c, 0x449a6d9d, 0x44203131, 0x4043456f, 0x3b615d06, 0x0, 0x0, 0x0, 0x3f800000, 0x3f800000,
        0x7f800000,
    ];
    const SIN: [u32; 86] = [
        0x3f22baf3, 0x3ea4cded, 0x3f6a2bcc, 0xbf597371, 0x3e96b07e, 0x3f73f345, 0xbf58a04a, 0xbf3e641a, 0xbf4098db,
        0x3f25e7dc, 0x3f43fe0b, 0x3f49c45d, 0xbf1dcd40, 0xbf584c74, 0xbea80dd5, 0x3f24d0e6, 0x3f681f7d, 0xbf7ca2b5,
        0x3f74aa74, 0xbe2fd46f, 0x3f526afa, 0xbf0cc4ef, 0xbd6f45be, 0xbf211900, 0xbf392c25, 0xbf2c702a, 0x3f5e2a8b,
        0xbf6c65eb, 0xbe0c243b, 0xbf47756c, 0x3f6ea6ce, 0x3f38d812, 0xbf3c6641, 0xbf2c0fbc, 0xbe06ddea, 0xbf3108a3,
        0x3f7fd688, 0xbf256d3b, 0xbf036d21, 0x3f09b4ed, 0x3f0b9cb6, 0xbe494ce2, 0x3e1fc1e1, 0xbf2a7c6e, 0x3f498063,
        0xbf2a18a8, 0xbe8019fd, 0x3f1c9d36, 0x3dfe0655, 0xbeab97a4, 0xbf4bf4a1, 0x3f800000, 0xbf31fce3, 0xbf796d56,
        0xbe9c4662, 0x3f223ceb, 0xbf7090b6, 0xbf6fc37e, 0x3f7b3269, 0x3f7dcf14, 0x3f7e7329, 0x3f4dc584, 0x3ea0465c,
        0x3f24183a, 0xbf50f95a, 0x3f5adb9a, 0xbf7ffc33, 0xbe2cc9a7, 0x3f6ecafe, 0x3f7d07e1, 0x3ec43de6, 0x3d539cb6,
        0xbe964e04, 0x3f04f7db, 0x3ec23375, 0xbf30bbe3, 0x3f3df0ed, 0x3e36ce37, 0x3f65ec15, 0x3f12c4af, 0x3e26e328,
        0xbf5c4b14, 0x3f6e1b6d, 0x80000000, 0x116c2, 0xbd92555f,
    ];
    const COS: [u32; 86] = [
        0xbf459f7f, 0x3f726020, 0x3ecee172, 0xbf0718ba, 0x3f74a94e, 0xbe9b3b74, 0xbf086a4e, 0xbf2b229d, 0x3f28a60c,
        0xbf42f702, 0x3f24b0cc, 0xbf1d9019, 0xbf49948c, 0xbf08ef14, 0xbf71d127, 0x3f43e30d, 0x3ed7ea75, 0xbe257840,
        0xbe96a907, 0x3f7c32a5, 0xbf11cf26, 0xbf55d288, 0x3f7f9016, 0x3f46f4c6, 0xbf30c4c9, 0x3f3d3636, 0xbefe6312,
        0xbec47d0e, 0xbf7d975c, 0x3f20799c, 0xbeb9459c, 0xbf311cb0, 0x3f2d5346, 0xbf3d8dea, 0xbf7dc51b, 0x3f38eb48,
        0xbd11b0b4, 0xbf435f19, 0x3f5bb04f, 0xbf57ceb5, 0xbf569467, 0x3f7b0138, 0x3f7cdd83, 0x3f3ef8f7, 0xbf1de6fe,
        0xbf3f51db, 0x3f77db9a, 0x3f4a811f, 0x3f7e05eb, 0x3f7131ea, 0xbf1ab831, 0xb91bebbc, 0x3f38003b, 0xbe668d2b,
        0xbf73c8a7, 0x3f460703, 0x3eaf17ad, 0xbeb36fd3, 0x3e456d8e, 0x3e05b053, 0xbde10986, 0xbf184bc7, 0xbf73223a,
        0x3f447dda, 0x3f13defe, 0xbf04ce2a, 0x3c306ff1, 0x3f7c5446, 0x3eb88abe, 0xbe1b821e, 0xbf6c7308, 0xbf7fa87b,
        0x3f74b872, 0xbf5ac248, 0x3f6cdec6, 0x3f3934a3, 0x3f2ba268, 0x3f7be33c, 0x3ee12356, 0x3f51c001, 0x3f7c93c9,
        0x3f026910, 0xbebc0d9c, 0x3f800000, 0x3f800000, 0x3f7f587e,
    ];

    #[test]
    fn the_functions_are_numpys() {
        for (k, &x) in X.iter().enumerate() {
            let x = f32::from_bits(x);
            assert_eq!(super::exp(x).to_bits(), EXP[k], "exp({x})");
            assert_eq!(super::sin(x).to_bits(), SIN[k], "sin({x})");
            assert_eq!(super::cos(x).to_bits(), COS[k], "cos({x})");
        }
    }
}
