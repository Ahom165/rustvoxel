// Deterministic hash-based value noise (2D/3D) + fBm. No tables, no crates.

/// splitmix-style 64-bit integer hash.
pub fn hash2i(x: i64, y: i64, seed: u64) -> u64 {
    let mut h = (x as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ seed;
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    h
}

pub fn hash3i(x: i64, y: i64, z: i64, seed: u64) -> u64 {
    hash2i(x, hash2i(y, z, seed ^ 0x1234_5678_9ABC_DEF0) as i64, seed)
}

/// Map a hash to [0, 1).
pub fn hash01(h: u64) -> f32 {
    ((h >> 32) as f32) / 4294967296.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

pub fn value_noise_2d(x: f32, y: f32, seed: u64) -> f32 {
    let xi = x.floor() as i64;
    let yi = y.floor() as i64;
    let xf = x - xi as f32;
    let yf = y - yi as f32;
    let u = smooth(xf);
    let v = smooth(yf);
    let a = hash01(hash2i(xi, yi, seed));
    let b = hash01(hash2i(xi + 1, yi, seed));
    let c = hash01(hash2i(xi, yi + 1, seed));
    let d = hash01(hash2i(xi + 1, yi + 1, seed));
    a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

pub fn value_noise_3d(x: f32, y: f32, z: f32, seed: u64) -> f32 {
    let xi = x.floor() as i64;
    let yi = y.floor() as i64;
    let zi = z.floor() as i64;
    let xf = x - xi as f32;
    let yf = y - yi as f32;
    let zf = z - zi as f32;
    let u = smooth(xf);
    let v = smooth(yf);
    let w = smooth(zf);
    let s = seed;
    let c000 = hash01(hash3i(xi, yi, zi, s));
    let c100 = hash01(hash3i(xi + 1, yi, zi, s));
    let c010 = hash01(hash3i(xi, yi + 1, zi, s));
    let c110 = hash01(hash3i(xi + 1, yi + 1, zi, s));
    let c001 = hash01(hash3i(xi, yi, zi + 1, s));
    let c101 = hash01(hash3i(xi + 1, yi, zi + 1, s));
    let c011 = hash01(hash3i(xi, yi + 1, zi + 1, s));
    let c111 = hash01(hash3i(xi + 1, yi + 1, zi + 1, s));
    let x00 = c000 + (c100 - c000) * u;
    let x10 = c010 + (c110 - c010) * u;
    let x01 = c001 + (c101 - c001) * u;
    let x11 = c011 + (c111 - c011) * u;
    let y0 = x00 + (x10 - x00) * v;
    let y1 = x01 + (x11 - x01) * v;
    y0 + (y1 - y0) * w
}

/// Fractal Brownian motion over value noise, output in [0, 1).
pub fn fbm2(x: f32, y: f32, seed: u64, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 1.0f32;
    let mut norm = 0.0f32;
    let mut fx = x;
    let mut fy = y;
    for o in 0..octaves {
        sum += value_noise_2d(fx, fy, seed.wrapping_add(0x51ED_2700 + o as u64)) * amp;
        norm += amp;
        amp *= 0.5;
        fx *= 2.0;
        fy *= 2.0;
    }
    if norm > 0.0 {
        sum / norm
    } else {
        0.0
    }
}

pub fn fbm3(x: f32, y: f32, z: f32, seed: u64, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 1.0f32;
    let mut norm = 0.0f32;
    let mut fx = x;
    let mut fy = y;
    let mut fz = z;
    for o in 0..octaves {
        sum += value_noise_3d(fx, fy, fz, seed.wrapping_add(0x9E37_79B9 + o as u64)) * amp;
        norm += amp;
        amp *= 0.5;
        fx *= 2.0;
        fy *= 2.0;
        fz *= 2.0;
    }
    if norm > 0.0 {
        sum / norm
    } else {
        0.0
    }
}

/// Random seed from the system clock.
pub fn rand_seed() -> u64 {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    hash2i(d.as_nanos() as i64, d.as_secs() as i64, 0xCAFE_BABE)
}

/// Thread-local xorshift PRNG in [0, 1).
pub fn rand01() -> f32 {
    use std::cell::Cell;
    thread_local! {
        static RNG: Cell<u64> = const { Cell::new(0x853C_49E6_748F_EA9B) };
    }
    RNG.with(|r| {
        let mut x = r.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        r.set(x);
        ((x >> 11) as f32) / 9007199254740992.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_in_range() {
        for i in 0..2000 {
            let x = (i as f32) * 0.137;
            let y = (i as f32) * 0.291;
            let n = fbm2(x, y, 42, 4);
            assert!((0.0..=1.0).contains(&n), "fbm2 out of range: {n}");
            assert_eq!(n, fbm2(x, y, 42, 4));
            let m = fbm3(x, y, x * 0.5, 42, 2);
            assert!((0.0..=1.0).contains(&m));
        }
    }

    #[test]
    fn hash_spread() {
        let mut sum = 0.0f32;
        for i in 0..1000 {
            sum += hash01(hash2i(i, i * 7 + 3, 1));
        }
        let avg = sum / 1000.0;
        assert!(avg > 0.4 && avg < 0.6, "hash average not centered: {avg}");
    }
}
