//! Eyeballs: a UV sphere whose texture is laid out by angle from the front
//! pole (v = polar angle / π, u = azimuth / 2π), so the iris is a band at the
//! top of the texture and needs no seam across it.

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

use crate::noise::{fbm, value};
use crate::{linear_to_srgb, to_u8, Image};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EyeParams {
    /// Linear RGB iris colour.
    pub iris: [f32; 3],
    /// Iris radius as an angle from the front (radians).
    pub iris_angle: f32,
    /// Pupil radius as a fraction of the iris.
    pub pupil: f32,
    /// Eyeball radius as a fraction of the distance between the eyes.
    pub size: f32,
}

impl Default for EyeParams {
    fn default() -> Self {
        Self {
            iris: [0.20, 0.11, 0.05],
            iris_angle: 0.55,
            pupil: 0.36,
            size: 0.19,
        }
    }
}

/// Eyeball mesh around the origin, front (+Z) at `v = 0`.
pub struct EyeMesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

pub fn eyeball(radius: f32, rings: u32, segments: u32) -> EyeMesh {
    let (mut positions, mut normals, mut uvs, mut indices) = (vec![], vec![], vec![], vec![]);
    for r in 0..=rings {
        let theta = std::f32::consts::PI * r as f32 / rings as f32;
        for s in 0..=segments {
            let phi = std::f32::consts::TAU * s as f32 / segments as f32;
            let n = Vec3::new(
                theta.sin() * phi.cos(),
                theta.sin() * phi.sin(),
                theta.cos(),
            );
            positions.push(n * radius);
            normals.push(n);
            uvs.push([s as f32 / segments as f32, r as f32 / rings as f32]);
        }
    }
    let row = segments + 1;
    for r in 0..rings {
        for s in 0..segments {
            let (a, b) = (r * row + s, (r + 1) * row + s);
            // Counter-clockwise seen from outside.
            // e_r × e_φ points outward, so go radially first, then around.
            if r != 0 {
                indices.extend([a, b, a + 1]);
            }
            if r != rings - 1 {
                indices.extend([a + 1, b, b + 1]);
            }
        }
    }
    EyeMesh {
        positions,
        normals,
        uvs,
        indices,
    }
}

/// Transform from eye space (+Z forward) to the world: centre and look direction.
pub fn eye_transform(center: Vec3, forward: Vec3) -> Mat4 {
    let z = forward.try_normalize().unwrap_or(Vec3::Z);
    let x = Vec3::Y.cross(z).try_normalize().unwrap_or(Vec3::X);
    let y = z.cross(x);
    Mat4::from_cols(
        x.extend(0.0),
        y.extend(0.0),
        z.extend(0.0),
        center.extend(1.0),
    )
}

/// Iris/sclera texture (`width × height`, sRGB) for the eyeball UV layout.
pub fn iris_texture(p: &EyeParams, width: u32, height: u32) -> Image {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    let iris = Vec3::from_array(p.iris);
    let pupil_angle = p.iris_angle * p.pupil;
    for y in 0..height {
        let theta = std::f32::consts::PI * (y as f32 + 0.5) / height as f32;
        for x in 0..width {
            let phi = std::f32::consts::TAU * (x as f32 + 0.5) / width as f32;
            let dir = Vec3::new(phi.cos(), phi.sin(), 0.0);
            let c = if theta < pupil_angle {
                Vec3::splat(0.01)
            } else if theta < p.iris_angle {
                // 0 at the pupil edge, 1 at the limbus.
                let t = (theta - pupil_angle) / (p.iris_angle - pupil_angle);
                let fibres = value(dir * 40.0 + Vec3::new(0.0, 0.0, t * 3.0), 5);
                let crypts = fbm(dir * 6.0 + Vec3::new(0.0, 0.0, t * 5.0), 3, 9);
                let collarette = (-((t - 0.3) * 9.0).powi(2)).exp() * 0.5;
                let limbal = ((t - 0.8) / 0.2).clamp(0.0, 1.0);
                let lum =
                    (0.75 + 0.35 * fibres + 0.25 * crypts + collarette) * (1.0 - 0.75 * limbal);
                (iris * lum).min(Vec3::ONE)
            } else {
                // Sclera: off-white, a little warmer and veinier towards the back.
                let back = ((theta - p.iris_angle) / 1.2).clamp(0.0, 1.0);
                let vein = (fbm(Vec3::new(phi.cos(), phi.sin(), theta) * 12.0, 3, 13) - 0.55)
                    .max(0.0)
                    * 1.6
                    * back;
                Vec3::new(0.82, 0.78, 0.74) * (1.0 - 0.15 * back)
                    + Vec3::new(0.25, -0.15, -0.15) * vein
            };
            rgba.extend([
                to_u8(linear_to_srgb(c.x)),
                to_u8(linear_to_srgb(c.y)),
                to_u8(linear_to_srgb(c.z)),
                255,
            ]);
        }
    }
    Image {
        width,
        height,
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eyeball_is_closed_and_outward() {
        let e = eyeball(0.012, 16, 24);
        assert_eq!(e.positions.len(), e.uvs.len());
        for t in e.indices.as_chunks::<3>().0 {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| e.positions[i as usize]);
            let n = (b - a).cross(c - a);
            if n.length() > 1e-12 {
                assert!(n.dot((a + b + c) / 3.0) > 0.0, "triangle faces outward");
            }
        }
        let tr = eye_transform(Vec3::new(0.03, 1.6, 0.1), Vec3::Z);
        assert!(
            (tr.transform_point3(Vec3::Z * 0.012) - Vec3::new(0.03, 1.6, 0.112)).length() < 1e-6
        );
    }

    #[test]
    fn iris_has_dark_pupil_and_light_sclera() {
        let img = iris_texture(&EyeParams::default(), 64, 64);
        let px = |x: u32, y: u32| img.rgba[((y * 64 + x) * 4) as usize];
        assert!(px(10, 0) < 40, "pupil");
        assert!(px(10, 40) > 180, "sclera");
    }
}
