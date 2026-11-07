//! UV rasterization: which surface point each texel shows.

use glam::{Vec2, Vec3};
use rayon::prelude::*;

/// A texel's surface point: triangle index and barycentric weights.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Texel {
    pub tri: u32,
    pub bary: [f32; 3],
}

impl Texel {
    pub const EMPTY: Texel = Texel {
        tri: u32::MAX,
        bary: [0.0; 3],
    };
    pub fn is_empty(&self) -> bool {
        self.tri == u32::MAX
    }
}

/// `size × size` texels covering UV space `[0, 1]²` (glTF convention, v down).
#[derive(Debug, Clone)]
pub struct SurfaceMap {
    pub size: u32,
    pub texels: Vec<Texel>,
    /// Texels covered by a triangle (before dilation).
    pub inside: Vec<bool>,
}

impl SurfaceMap {
    /// Rasterize triangles (`indices` over `uvs`), then grow islands by
    /// `dilate` texels so texture filtering doesn't bleed background in.
    pub fn rasterize(uvs: &[[f32; 2]], indices: &[u32], size: u32, dilate: u32) -> Self {
        let n = size as usize;
        let mut texels = vec![Texel::EMPTY; n * n];
        let s = size as f32;
        for (t, tri) in indices.as_chunks::<3>().0.iter().enumerate() {
            let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| Vec2::from(uvs[i as usize]) * s);
            let area = (b - a).perp_dot(c - a);
            if area.abs() < 1e-12 {
                continue;
            }
            let lo = a.min(b).min(c).floor().max(Vec2::ZERO);
            let hi = a.max(b).max(c).ceil().min(Vec2::splat(s - 1.0));
            for y in lo.y as usize..=hi.y as usize {
                for x in lo.x as usize..=hi.x as usize {
                    let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = (b - p).perp_dot(c - p) / area;
                    let w1 = (c - p).perp_dot(a - p) / area;
                    let w2 = 1.0 - w0 - w1;
                    const E: f32 = -1e-4;
                    if w0 >= E && w1 >= E && w2 >= E {
                        texels[y * n + x] = Texel {
                            tri: t as u32,
                            bary: [w0, w1, w2],
                        };
                    }
                }
            }
        }
        let inside: Vec<bool> = texels.iter().map(|t| !t.is_empty()).collect();
        // Dilation: copy a filled neighbour into empty texels, ring by ring.
        for _ in 0..dilate {
            let prev = texels.clone();
            for y in 0..n {
                for x in 0..n {
                    if !prev[y * n + x].is_empty() {
                        continue;
                    }
                    let mut found = None;
                    for (dx, dy) in [
                        (-1i32, 0i32),
                        (1, 0),
                        (0, -1),
                        (0, 1),
                        (-1, -1),
                        (1, 1),
                        (-1, 1),
                        (1, -1),
                    ] {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx >= 0
                            && ny >= 0
                            && (nx as usize) < n
                            && (ny as usize) < n
                            && !prev[ny as usize * n + nx as usize].is_empty()
                        {
                            found = Some(prev[ny as usize * n + nx as usize]);
                            break;
                        }
                    }
                    if let Some(f) = found {
                        texels[y * n + x] = f;
                    }
                }
            }
        }
        Self {
            size,
            texels,
            inside,
        }
    }

    /// Fraction of texels covered by the layout (before dilation).
    pub fn coverage(&self) -> f32 {
        self.inside.iter().filter(|&&b| b).count() as f32 / self.inside.len() as f32
    }

    /// Interpolate per-vertex data to texels (empty texels get `T::default()`).
    pub fn interpolate<T>(&self, indices: &[u32], per_vertex: &[T]) -> Vec<T>
    where
        T: Copy
            + Default
            + Send
            + Sync
            + std::ops::Mul<f32, Output = T>
            + std::ops::Add<Output = T>,
    {
        self.texels
            .par_iter()
            .map(|t| {
                if t.is_empty() {
                    return T::default();
                }
                let i = t.tri as usize * 3;
                let v = |k: usize| per_vertex[indices[i + k] as usize];
                v(0) * t.bary[0] + v(1) * t.bary[1] + v(2) * t.bary[2]
            })
            .collect()
    }
}

/// What layers can read at each texel.
#[derive(Debug, Clone)]
pub struct TexelAttributes {
    pub size: u32,
    /// Rest-pose surface position (metres) — drives 3D noise.
    pub position: Vec<Vec3>,
    /// Region weights per texel (`regions[r][texel]`, 0..1).
    pub regions: Vec<Vec<f32>>,
    pub empty: Vec<bool>,
}

impl TexelAttributes {
    pub fn new(
        map: &SurfaceMap,
        indices: &[u32],
        positions: &[Vec3],
        region_weights: &[Vec<f32>],
    ) -> Self {
        Self {
            size: map.size,
            position: map.interpolate(indices, positions),
            regions: region_weights
                .iter()
                .map(|w| map.interpolate(indices, w))
                .collect(),
            empty: map.texels.iter().map(|t| t.is_empty()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterizes_and_interpolates_a_quad() {
        // Two triangles covering the left half of UV space.
        let uvs = [[0.0, 0.0], [0.5, 0.0], [0.5, 1.0], [0.0, 1.0]];
        let idx = [0, 1, 2, 0, 2, 3];
        let m = SurfaceMap::rasterize(&uvs, &idx, 64, 0);
        assert!((m.coverage() - 0.5).abs() < 0.02, "{}", m.coverage());
        // x position interpolates across texels.
        let pos = [Vec3::ZERO, Vec3::X, Vec3::X, Vec3::ZERO];
        let p = m.interpolate(&idx, &pos);
        let at = |x: usize, y: usize| p[y * 64 + x].x;
        assert!(at(1, 10) < 0.1 && at(30, 10) > 0.9);
        // Dilation fills a ring around the island.
        let d = SurfaceMap::rasterize(&uvs, &idx, 64, 2);
        assert!(!d.texels[10 * 64 + 33].is_empty() && d.texels[10 * 64 + 40].is_empty());
    }
}
