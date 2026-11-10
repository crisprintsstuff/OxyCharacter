//! Carrying deltas authored on the base mesh onto a reshaped mesh.
//!
//! An expression (or any delta) is authored on the base face. On a strongly
//! morphed face, adding the same world-space vectors looks wrong: a smile
//! authored on a narrow face pulls a wide mouth too little and in the wrong
//! directions where the surface turned. For every vertex we fit the rotation
//! and uniform scale that best map its neighbourhood (a few edge rings, plus
//! the normal) from the base shape to the reshaped one, and apply that to the
//! delta. Regions that only moved get the delta unchanged.
//!
//! The fit is the orthogonal Procrustes problem (Schönemann 1966): the best
//! rotation is the orthogonal polar factor of the cross-covariance matrix,
//! found here with Higham's Newton iteration (Higham 1986, "Computing the
//! polar decomposition — with applications") instead of an SVD.

use glam::{Mat3, Vec3};

use crate::Mesh;

/// Per-vertex rotation and scale from one shape of a mesh to another.
#[derive(Debug, Clone)]
pub struct Deformation {
    rot: Vec<Mat3>,
    scale: Vec<f32>,
}

impl Deformation {
    /// Fit the local deformation from `from` to `to` (both shapes of `mesh`)
    /// at `vertices`, over `rings` edge rings. Other vertices get identity.
    pub fn fit(
        mesh: &Mesh,
        from: &[Vec3],
        to: &[Vec3],
        vertices: impl IntoIterator<Item = u32>,
        rings: usize,
    ) -> Self {
        let n = from.len();
        let mut rot = vec![Mat3::IDENTITY; n];
        let mut scale = vec![1.0; n];
        let adj = adjacency(mesh, n);
        let n_from = mesh.compute_normals(from);
        let n_to = mesh.compute_normals(to);
        let mut mark = vec![u32::MAX; n];
        let mut ring: Vec<u32> = Vec::new();
        for v in vertices {
            let vi = v as usize;
            if vi >= n {
                continue;
            }
            // Gather the neighbourhood by breadth-first rings.
            ring.clear();
            ring.push(v);
            mark[vi] = v;
            let mut start = 0;
            for _ in 0..rings {
                let end = ring.len();
                for k in start..end {
                    for &w in &adj[ring[k] as usize] {
                        if mark[w as usize] != v {
                            mark[w as usize] = v;
                            ring.push(w);
                        }
                    }
                }
                start = end;
            }
            let k = ring.len() as f32;
            let c0 = ring.iter().map(|&i| from[i as usize]).sum::<Vec3>() / k;
            let c1 = ring.iter().map(|&i| to[i as usize]).sum::<Vec3>() / k;
            let mut m = Mat3::ZERO;
            let (mut s0, mut s1) = (0.0f32, 0.0f32);
            for &i in &ring {
                let p = from[i as usize] - c0;
                let q = to[i as usize] - c1;
                m += outer(q, p);
                s0 += p.length_squared();
                s1 += q.length_squared();
            }
            let s = if s0 > 1e-20 { (s1 / s0).sqrt() } else { 1.0 };
            // The normal pins the rotation about in-surface axes for flat
            // neighbourhoods, weighted like an average neighbour.
            let w = s0 / k;
            m += outer(n_to[vi] * s, n_from[vi]) * w;
            rot[vi] = polar_rotation(m);
            scale[vi] = s;
        }
        Self { rot, scale }
    }

    /// Carry sparse `deltas` at `indices`.
    pub fn apply(&self, indices: &[u32], deltas: &[Vec3]) -> Vec<Vec3> {
        indices
            .iter()
            .zip(deltas)
            .map(|(&i, &d)| {
                let i = i as usize;
                match (self.rot.get(i), self.scale.get(i)) {
                    (Some(r), Some(s)) => *r * d * *s,
                    _ => d,
                }
            })
            .collect()
    }
}

fn outer(a: Vec3, b: Vec3) -> Mat3 {
    Mat3::from_cols(a * b.x, a * b.y, a * b.z)
}

fn adjacency(mesh: &Mesh, n: usize) -> Vec<Vec<u32>> {
    let mut adj = vec![Vec::new(); n];
    for &[a, b, c] in mesh.indices.as_chunks::<3>().0 {
        for (i, j) in [(a, b), (b, c), (c, a)] {
            if !adj[i as usize].contains(&j) {
                adj[i as usize].push(j);
            }
            if !adj[j as usize].contains(&i) {
                adj[j as usize].push(i);
            }
        }
    }
    adj
}

/// Rotation closest to `m` (its orthogonal polar factor), or identity when
/// `m` is degenerate or a reflection.
fn polar_rotation(m: Mat3) -> Mat3 {
    if m.determinant() <= 1e-30 {
        return Mat3::IDENTITY;
    }
    let mut x = m;
    for _ in 0..30 {
        let inv_t = x.inverse().transpose();
        // Scaling keeps the iteration fast for badly scaled inputs.
        let g = ((inv_t.determinant()).abs() / x.determinant().abs())
            .powf(1.0 / 6.0)
            .clamp(1e-3, 1e3);
        let next = (x * g + inv_t / g) * 0.5;
        let diff = (next - x).abs().to_cols_array().iter().sum::<f32>();
        x = next;
        if diff < 1e-6 {
            break;
        }
    }
    if x.is_finite() {
        x
    } else {
        Mat3::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;

    fn grid() -> Mesh {
        let mut p = Vec::new();
        for y in 0..4 {
            for x in 0..4 {
                p.push(Vec3::new(x as f32, y as f32, 0.0));
            }
        }
        let mut t = Vec::new();
        for y in 0..3u32 {
            for x in 0..3u32 {
                let i = y * 4 + x;
                t.extend([i, i + 1, i + 5, i, i + 5, i + 4]);
            }
        }
        Mesh::new(p, t).unwrap()
    }

    #[test]
    fn identity_rotation_and_scale_carry_over() {
        let m = grid();
        let idx = [5, 6];
        let d = [Vec3::new(0.1, 0.0, 0.3), Vec3::new(0.0, -0.2, 0.0)];
        // Same shape (moved): unchanged.
        let shifted: Vec<Vec3> = m.positions.iter().map(|p| *p + Vec3::ONE).collect();
        let same = Deformation::fit(&m, &m.positions, &shifted, idx, 2).apply(&idx, &d);
        assert!(same.iter().zip(&d).all(|(a, b)| a.distance(*b) < 1e-5));
        // Rotated and doubled: deltas rotate and double.
        let r = Quat::from_rotation_y(0.7) * Quat::from_rotation_x(0.3);
        let moved: Vec<Vec3> = m.positions.iter().map(|p| r * (*p * 2.0)).collect();
        let out = Deformation::fit(&m, &m.positions, &moved, idx, 2).apply(&idx, &d);
        for (o, d) in out.iter().zip(&d) {
            assert!(
                o.distance(r * (*d * 2.0)) < 1e-4,
                "{o:?} vs {:?}",
                r * (*d * 2.0)
            );
        }
    }

    #[test]
    fn polar_factor_of_a_stretch_is_the_rotation() {
        let r = Mat3::from_quat(Quat::from_rotation_z(0.4));
        let m = r * Mat3::from_diagonal(Vec3::new(3.0, 0.5, 1.2));
        let p = polar_rotation(m);
        assert!((p - r).abs().to_cols_array().iter().all(|v| *v < 1e-4));
        assert_eq!(polar_rotation(Mat3::ZERO), Mat3::IDENTITY);
    }
}
