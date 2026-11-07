//! Layer stack evaluated per texel, and encoding to textures.

use glam::Vec3;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::noise::fbm;
use crate::surface::TexelAttributes;
use crate::{linear_to_srgb, to_u8, Image};

/// Which map a layer writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Channel {
    /// Linear RGB colour.
    Albedo,
    /// Roughness (uses the red component of the source).
    Roughness,
    /// Height for the normal map (red component, 0..1 around 0.5).
    Height,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Source {
    Color([f32; 3]),
    /// 3D fractal noise at `frequency` (cycles per metre), mapped `low → high`.
    Noise {
        frequency: f32,
        octaves: u32,
        seed: u32,
        low: [f32; 3],
        high: [f32; 3],
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Mask {
    Full,
    /// Region weight, remapped by smoothstep(`lo`, `hi`).
    Region {
        region: usize,
        lo: f32,
        hi: f32,
    },
    /// Noise above a threshold (soft edge `softness`).
    Noise {
        frequency: f32,
        seed: u32,
        threshold: f32,
        softness: f32,
    },
    /// Product of two masks.
    Both(Box<Mask>, Box<Mask>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Blend {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Add,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub channel: Channel,
    pub source: Source,
    pub mask: Mask,
    pub blend: Blend,
    pub opacity: f32,
}

fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn eval_mask(m: &Mask, at: &TexelAttributes, i: usize) -> f32 {
    match m {
        Mask::Full => 1.0,
        Mask::Region { region, lo, hi } => at
            .regions
            .get(*region)
            .map_or(0.0, |r| smoothstep(*lo, *hi, r[i])),
        Mask::Noise {
            frequency,
            seed,
            threshold,
            softness,
        } => {
            let v = fbm(at.position[i] * *frequency, 3, *seed);
            smoothstep(*threshold - softness, *threshold + softness, v)
        }
        Mask::Both(a, b) => eval_mask(a, at, i) * eval_mask(b, at, i),
    }
}

fn eval_source(s: &Source, at: &TexelAttributes, i: usize) -> Vec3 {
    match s {
        Source::Color(c) => Vec3::from_array(*c),
        Source::Noise {
            frequency,
            octaves,
            seed,
            low,
            high,
        } => {
            let t = fbm(at.position[i] * *frequency, *octaves, *seed);
            Vec3::from_array(*low).lerp(Vec3::from_array(*high), t)
        }
    }
}

fn blend(mode: Blend, base: Vec3, top: Vec3) -> Vec3 {
    match mode {
        Blend::Normal => top,
        Blend::Multiply => base * top,
        Blend::Screen => Vec3::ONE - (Vec3::ONE - base) * (Vec3::ONE - top),
        Blend::Add => base + top,
        Blend::Overlay => {
            let f = |b: f32, t: f32| {
                if b < 0.5 {
                    2.0 * b * t
                } else {
                    1.0 - 2.0 * (1.0 - b) * (1.0 - t)
                }
            };
            Vec3::new(f(base.x, top.x), f(base.y, top.y), f(base.z, top.z))
        }
    }
}

/// Per-texel results in linear float.
#[derive(Debug, Clone)]
pub struct Maps {
    pub size: u32,
    pub albedo: Vec<Vec3>,
    pub roughness: Vec<f32>,
    pub height: Vec<f32>,
}

/// Evaluate `layers` (bottom first) over every texel.
pub fn bake(at: &TexelAttributes, layers: &[Layer]) -> Maps {
    let n = at.position.len();
    let px: Vec<(Vec3, f32, f32)> = (0..n)
        .into_par_iter()
        .map(|i| {
            let (mut albedo, mut rough, mut height) = (Vec3::splat(0.5), 0.5f32, 0.5f32);
            if at.empty[i] {
                return (albedo, rough, height);
            }
            for l in layers {
                let k = (eval_mask(&l.mask, at, i) * l.opacity).clamp(0.0, 1.0);
                if k <= 0.0 {
                    continue;
                }
                let src = eval_source(&l.source, at, i);
                match l.channel {
                    Channel::Albedo => albedo = albedo.lerp(blend(l.blend, albedo, src), k),
                    Channel::Roughness => {
                        rough += (blend(l.blend, Vec3::splat(rough), src).x - rough) * k
                    }
                    Channel::Height => {
                        height += (blend(l.blend, Vec3::splat(height), src).x - height) * k
                    }
                }
            }
            (
                albedo.clamp(Vec3::ZERO, Vec3::ONE),
                rough.clamp(0.02, 1.0),
                height,
            )
        })
        .collect();
    Maps {
        size: at.size,
        albedo: px.iter().map(|p| p.0).collect(),
        roughness: px.iter().map(|p| p.1).collect(),
        height: px.iter().map(|p| p.2).collect(),
    }
}

/// Encoded textures (glTF conventions).
#[derive(Debug, Clone)]
pub struct Textures {
    /// sRGB colour, alpha 255.
    pub albedo: Image,
    /// Tangent space, +X along +u, +Y up the image (glTF), 0.5 = flat.
    pub normal: Image,
    /// R occlusion (1), G roughness, B metallic (0) — glTF metallicRoughness layout.
    pub orm: Image,
}

/// Encode maps; `normal_strength` scales height differences into slopes.
pub fn encode(maps: &Maps, empty: &[bool], normal_strength: f32) -> Textures {
    let s = maps.size as usize;
    let mut albedo = Vec::with_capacity(s * s * 4);
    let mut orm = Vec::with_capacity(s * s * 4);
    for i in 0..s * s {
        let a = maps.albedo[i];
        albedo.extend([
            to_u8(linear_to_srgb(a.x)),
            to_u8(linear_to_srgb(a.y)),
            to_u8(linear_to_srgb(a.z)),
            255,
        ]);
        orm.extend([255, to_u8(maps.roughness[i]), 0, 255]);
    }
    let h = |x: usize, y: usize| maps.height[y * s + x];
    let valid = |x: isize, y: isize| {
        x >= 0
            && y >= 0
            && (x as usize) < s
            && (y as usize) < s
            && !empty[y as usize * s + x as usize]
    };
    let mut normal = Vec::with_capacity(s * s * 4);
    for y in 0..s {
        for x in 0..s {
            // Central differences where both neighbours are on the surface, else one-sided.
            let d = |dx: isize, dy: isize| {
                let (xi, yi) = (x as isize, y as isize);
                let (fwd, back) = (valid(xi + dx, yi + dy), valid(xi - dx, yi - dy));
                let here = h(x, y);
                match (fwd, back) {
                    (true, true) => {
                        (h((xi + dx) as usize, (yi + dy) as usize)
                            - h((xi - dx) as usize, (yi - dy) as usize))
                            * 0.5
                    }
                    (true, false) => h((xi + dx) as usize, (yi + dy) as usize) - here,
                    (false, true) => here - h((xi - dx) as usize, (yi - dy) as usize),
                    _ => 0.0,
                }
            };
            let (du, dv) = (d(1, 0), d(0, 1));
            // Image y runs down (+v); tangent-space +Y points up the image.
            let n = Vec3::new(-du * normal_strength, dv * normal_strength, 1.0).normalize();
            normal.extend([
                to_u8(n.x * 0.5 + 0.5),
                to_u8(n.y * 0.5 + 0.5),
                to_u8(n.z * 0.5 + 0.5),
                255,
            ]);
        }
    }
    let img = |rgba| Image {
        width: maps.size,
        height: maps.size,
        rgba,
    };
    Textures {
        albedo: img(albedo),
        normal: img(normal),
        orm: img(orm),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(n: usize) -> TexelAttributes {
        TexelAttributes {
            size: n as u32,
            position: (0..n * n)
                .map(|i| Vec3::new((i % n) as f32 / n as f32, (i / n) as f32 / n as f32, 0.0))
                .collect(),
            regions: vec![(0..n * n)
                .map(|i| if i % n < n / 2 { 1.0 } else { 0.0 })
                .collect()],
            empty: vec![false; n * n],
        }
    }

    #[test]
    fn region_masks_and_blends() {
        let at = attrs(8);
        let layers = vec![
            Layer {
                channel: Channel::Albedo,
                source: Source::Color([0.5, 0.4, 0.3]),
                mask: Mask::Full,
                blend: Blend::Normal,
                opacity: 1.0,
            },
            Layer {
                channel: Channel::Albedo,
                source: Source::Color([1.0, 0.0, 0.0]),
                mask: Mask::Region {
                    region: 0,
                    lo: 0.0,
                    hi: 1.0,
                },
                blend: Blend::Multiply,
                opacity: 1.0,
            },
            Layer {
                channel: Channel::Roughness,
                source: Source::Color([0.3; 3]),
                mask: Mask::Full,
                blend: Blend::Normal,
                opacity: 1.0,
            },
        ];
        let m = bake(&at, &layers);
        assert_eq!(
            m.albedo[0],
            Vec3::new(0.5, 0.0, 0.0),
            "masked region multiplied"
        );
        assert_eq!(
            m.albedo[7],
            Vec3::new(0.5, 0.4, 0.3),
            "outside the region untouched"
        );
        assert!((m.roughness[3] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn flat_height_encodes_flat_normals_and_slopes_tilt() {
        let at = attrs(16);
        let mut m = bake(&at, &[]);
        let t = encode(&m, &at.empty, 4.0);
        assert_eq!(&t.normal.rgba[0..3], &[128, 128, 255]);
        assert_eq!(t.albedo.rgba.len(), 16 * 16 * 4);
        // Height rising with +u tilts normals towards -X.
        m.height = (0..256).map(|i| (i % 16) as f32 / 16.0).collect();
        let t = encode(&m, &at.empty, 4.0);
        let px = 5 * 16 + 5;
        assert!(
            t.normal.rgba[px * 4] < 120,
            "x tilt {}",
            t.normal.rgba[px * 4]
        );
    }
}
