//! Mesh representation and sparse morph-target blending.
//!
//! Topology never changes during character editing; only vertex positions do.
//! Morph targets store just the vertices they move, so hundreds of targets
//! stay cheap to keep in memory and to evaluate.

use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum MeshError {
    #[error("triangle index {0} out of range for {1} vertices")]
    IndexOutOfRange(u32, usize),
    #[error("index buffer length {0} is not a multiple of 3")]
    BadIndexCount(usize),
}

/// A triangle mesh with fixed topology.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    /// Triangle list, three indices per triangle.
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn new(positions: Vec<Vec3>, indices: Vec<u32>) -> Result<Self, MeshError> {
        if !indices.len().is_multiple_of(3) {
            return Err(MeshError::BadIndexCount(indices.len()));
        }
        if let Some(&bad) = indices.iter().find(|&&i| i as usize >= positions.len()) {
            return Err(MeshError::IndexOutOfRange(bad, positions.len()));
        }
        Ok(Self { positions, indices })
    }

    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    pub fn triangles(&self) -> impl Iterator<Item = [u32; 3]> + '_ {
        self.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]])
    }

    /// Area-weighted vertex normals for the given positions (same topology).
    pub fn compute_normals(&self, positions: &[Vec3]) -> Vec<Vec3> {
        let mut normals = vec![Vec3::ZERO; positions.len()];
        for [a, b, c] in self.triangles() {
            let (pa, pb, pc) = (
                positions[a as usize],
                positions[b as usize],
                positions[c as usize],
            );
            // Unnormalized cross product is proportional to triangle area.
            let n = (pb - pa).cross(pc - pa);
            for i in [a, b, c] {
                normals[i as usize] += n;
            }
        }
        normals.iter_mut().for_each(|n| *n = n.normalize_or_zero());
        normals
    }
}

/// A sparse morph target: only the moved vertices are stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphTarget {
    pub name: String,
    pub indices: Vec<u32>,
    pub deltas: Vec<Vec3>,
}

impl MorphTarget {
    /// Build a sparse target by diffing a sculpted mesh against the base.
    /// Deltas with length below `epsilon` are dropped.
    pub fn from_diff(
        name: impl Into<String>,
        base: &[Vec3],
        sculpt: &[Vec3],
        epsilon: f32,
    ) -> Self {
        assert_eq!(base.len(), sculpt.len(), "topology mismatch");
        let (mut indices, mut deltas) = (Vec::new(), Vec::new());
        for (i, (b, s)) in base.iter().zip(sculpt).enumerate() {
            let d = *s - *b;
            if d.length_squared() > epsilon * epsilon {
                indices.push(i as u32);
                deltas.push(d);
            }
        }
        Self {
            name: name.into(),
            indices,
            deltas,
        }
    }
}

/// A base mesh plus its library of morph targets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphableMesh {
    pub base: Mesh,
    pub targets: Vec<MorphTarget>,
}

/// Weights below this magnitude are skipped during evaluation.
pub const WEIGHT_EPSILON: f32 = 1e-5;

impl MorphableMesh {
    pub fn target_index(&self, name: &str) -> Option<usize> {
        self.targets.iter().position(|t| t.name == name)
    }

    /// Evaluate `P = base + Σ wᵢ Δᵢ` into `out` (resized as needed).
    /// `weights` is indexed like `targets`; missing entries count as zero.
    pub fn evaluate_into(&self, weights: &[f32], out: &mut Vec<Vec3>) {
        out.clear();
        out.extend_from_slice(&self.base.positions);
        for (target, &w) in self.targets.iter().zip(weights) {
            if w.abs() < WEIGHT_EPSILON {
                continue;
            }
            for (&i, &d) in target.indices.iter().zip(&target.deltas) {
                out[i as usize] += d * w;
            }
        }
    }

    pub fn evaluate(&self, weights: &[f32]) -> Vec<Vec3> {
        let mut out = Vec::with_capacity(self.base.vertex_count());
        self.evaluate_into(weights, &mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad() -> Mesh {
        Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0), Vec3::Y],
            vec![0, 1, 2, 0, 2, 3],
        )
        .unwrap()
    }

    #[test]
    fn rejects_bad_indices() {
        assert_eq!(
            Mesh::new(vec![Vec3::ZERO], vec![0, 0, 5]).unwrap_err(),
            MeshError::IndexOutOfRange(5, 1)
        );
        assert_eq!(
            Mesh::new(vec![], vec![0]).unwrap_err(),
            MeshError::BadIndexCount(1)
        );
    }

    #[test]
    fn flat_quad_normals_point_up_z() {
        let m = quad();
        for n in m.compute_normals(&m.positions) {
            assert!((n - Vec3::Z).length() < 1e-6);
        }
    }

    #[test]
    fn diff_is_sparse_and_blending_is_linear() {
        let base = quad();
        let mut sculpt = base.positions.clone();
        sculpt[2].z += 2.0;
        let t = MorphTarget::from_diff("lift", &base.positions, &sculpt, 1e-6);
        assert_eq!(t.indices, vec![2]);

        let mm = MorphableMesh {
            base,
            targets: vec![t],
        };
        assert_eq!(mm.evaluate(&[0.5])[2].z, 1.0);
        assert_eq!(mm.evaluate(&[-1.0])[2].z, -2.0);
        assert_eq!(mm.evaluate(&[])[2].z, 0.0);
    }
}
