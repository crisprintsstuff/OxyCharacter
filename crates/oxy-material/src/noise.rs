//! Hash-based 3D value noise and fractal sums (our own, deterministic).

use glam::Vec3;

fn hash(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f)
        ^ seed.wrapping_mul(0x9e37_79b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

fn smooth(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Value noise in `[0, 1)`, smooth (C²) between lattice points.
pub fn value(p: Vec3, seed: u32) -> f32 {
    let f = p.floor();
    let (x, y, z) = (f.x as i32, f.y as i32, f.z as i32);
    let t = p - f;
    let (u, v, w) = (smooth(t.x), smooth(t.y), smooth(t.z));
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let c = |dx, dy, dz| hash(x + dx, y + dy, z + dz, seed);
    lerp(
        lerp(
            lerp(c(0, 0, 0), c(1, 0, 0), u),
            lerp(c(0, 1, 0), c(1, 1, 0), u),
            v,
        ),
        lerp(
            lerp(c(0, 0, 1), c(1, 0, 1), u),
            lerp(c(0, 1, 1), c(1, 1, 1), u),
            v,
        ),
        w,
    )
}

/// Fractal sum of `octaves` value-noise layers, normalised to `[0, 1)`.
pub fn fbm(p: Vec3, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut q) = (0.0, 1.0, 0.0, p);
    for o in 0..octaves.max(1) {
        sum += value(q, seed.wrapping_add(o * 101)) * amp;
        norm += amp;
        amp *= 0.5;
        q = q * 2.03 + Vec3::splat(17.17);
    }
    sum / norm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_bounded_and_continuous() {
        let p = Vec3::new(1.3, -2.7, 0.4);
        assert_eq!(value(p, 1), value(p, 1));
        assert_ne!(value(p, 1), value(p, 2));
        for i in 0..1000 {
            let q = Vec3::new(i as f32 * 0.37, i as f32 * -0.11, i as f32 * 0.07);
            let v = fbm(q, 4, 7);
            assert!((0.0..1.0).contains(&v));
        }
        // Continuity: nearby points give nearby values.
        let d = (value(p, 3) - value(p + Vec3::splat(1e-4), 3)).abs();
        assert!(d < 1e-3, "{d}");
    }
}
