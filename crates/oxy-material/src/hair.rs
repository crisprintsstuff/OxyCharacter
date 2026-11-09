//! Procedural hair-card textures: alpha-tested strands and a round normal.
//!
//! The pattern is original. Strands run vertically (along the card's v axis)
//! so an anisotropic highlight can follow them. RGB is sRGB; alpha is linear.

use crate::{linear_to_srgb, to_u8, Image};

/// Albedo (with strand alpha) and tangent-space normal for hair cards.
pub struct HairTextures {
    pub albedo: Image,
    pub normal: Image,
}

/// `color` is linear RGB. `size` is clamped to 32..=1024.
pub fn strand_textures(color: [f32; 3], size: u32) -> HairTextures {
    let size = size.clamp(32, 1024);
    let n = (size * size) as usize;
    let mut albedo = Vec::with_capacity(n * 4);
    let mut normal = Vec::with_capacity(n * 4);
    let period = 6.0f32;
    for y in 0..size {
        for x in 0..size {
            let (xf, yf) = (x as f32, y as f32);
            let wobble = (yf * 0.085).sin() * 1.4 + (yf * 0.021 + 1.7).sin() * 0.6;
            let sx = xf + wobble;
            let strand = (sx / period).floor() as i32;
            let local = sx.rem_euclid(period);
            let center = period * 0.5;
            let d = (local - center).abs();
            let radius = 2.55 + hash(strand, 3) * 0.15;
            // Wide, mostly opaque strands with narrow gaps, so a card reads as
            // hair instead of a scatter of dots.
            let mut alpha = if d >= radius {
                0.0
            } else {
                (1.0 - d / radius).powf(0.4)
            };
            let along = (yf * 0.08 + hash(strand, 2) * 20.0).rem_euclid(14.0);
            if along < 0.3 {
                alpha *= 0.2;
            }
            let shade = 0.72 + 0.28 * hash(strand, 1);
            let hi = hash(strand, 4);
            let rgb = [
                color[0] * shade * (0.9 + 0.1 * hi),
                color[1] * shade,
                color[2] * shade * (0.85 + 0.15 * hi),
            ];
            albedo.extend([
                to_u8(linear_to_srgb(rgb[0])),
                to_u8(linear_to_srgb(rgb[1])),
                to_u8(linear_to_srgb(rgb[2])),
                to_u8(alpha),
            ]);
            // Cylinder: bend the normal across the strand (tangent / +u).
            let nx = ((local - center) / radius).clamp(-1.0, 1.0) * alpha.clamp(0.25, 1.0);
            let nz = (1.0 - nx * nx).clamp(0.0, 1.0).sqrt();
            normal.extend([
                to_u8(nx * 0.5 + 0.5),
                to_u8(0.5),
                to_u8(nz * 0.5 + 0.5),
                255,
            ]);
        }
    }
    HairTextures {
        albedo: Image {
            width: size,
            height: size,
            rgba: albedo,
        },
        normal: Image {
            width: size,
            height: size,
            rgba: normal,
        },
    }
}

fn hash(a: i32, b: i32) -> f32 {
    let mut x = (a as u32)
        .wrapping_mul(374761393)
        .wrapping_add((b as u32).wrapping_mul(668265263));
    x = (x ^ (x >> 13)).wrapping_mul(1274126177);
    (x >> 16) as f32 / 65535.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strands_are_neither_solid_nor_empty() {
        let t = strand_textures([0.15, 0.08, 0.04], 64);
        assert_eq!(t.albedo.rgba.len(), 64 * 64 * 4);
        let alphas: Vec<u8> = t.albedo.rgba.iter().skip(3).step_by(4).copied().collect();
        let opaque = alphas.iter().filter(|a| **a > 200).count();
        let clear = alphas.iter().filter(|a| **a < 20).count();
        assert!(opaque > 100, "opaque texels {opaque}");
        assert!(clear > 100, "clear texels {clear}");
        // The normal map is not flat: some texel bends off +Z.
        let bent = t
            .normal
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| (p[0] as i16 - 128).abs() > 20)
            .count();
        assert!(bent > 50, "bent normals {bent}");
    }
}
