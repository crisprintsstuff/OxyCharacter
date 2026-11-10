//! Mesh representation and sparse morph-target blending.
//!
//! Topology never changes during character editing; only vertex positions do.
//! Morph targets store just the vertices they move, so hundreds of targets
//! stay cheap to keep in memory and to evaluate.

pub mod correspond;
pub mod transport;

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
        self.indices.as_chunks::<3>().0.iter().copied()
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
///
/// `aux_*` move the mesh's auxiliary points (see [`MorphableMesh::aux`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MorphTarget {
    pub name: String,
    pub indices: Vec<u32>,
    pub deltas: Vec<Vec3>,
    #[serde(default)]
    pub aux_indices: Vec<u32>,
    #[serde(default)]
    pub aux_deltas: Vec<Vec3>,
}

impl MorphTarget {
    pub fn new(name: impl Into<String>, indices: Vec<u32>, deltas: Vec<Vec3>) -> Self {
        Self {
            name: name.into(),
            indices,
            deltas,
            ..Default::default()
        }
    }

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
        Self::new(name, indices, deltas)
    }
}

/// A base mesh plus its library of morph targets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphableMesh {
    pub base: Mesh,
    pub targets: Vec<MorphTarget>,
    /// Auxiliary points that morph with the mesh but are not part of it,
    /// e.g. joint locator points inside the body.
    #[serde(default)]
    pub aux: Vec<Vec3>,
}

/// The mesh as drawn and exported: vertices split where texture coordinates
/// differ (UV seams), each mapped back to a base vertex.
///
/// Morphs, skinning and everything indexed by the base topology stay on the
/// welded base mesh; per-vertex data is gathered through [`RenderMesh::map`]
/// after evaluation. Triangle `t` here is triangle `t` of the base mesh.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RenderMesh {
    /// Render vertex → base vertex.
    pub map: Vec<u32>,
    /// Texture coordinates in glTF convention (v down, origin top-left).
    pub uvs: Vec<[f32; 2]>,
    /// Triangle list over render vertices, in base triangle order.
    pub indices: Vec<u32>,
}

impl RenderMesh {
    /// Build from per-corner `(base vertex, uv)` pairs of a triangle list.
    /// Corners with the same base vertex and the same UV share a render vertex.
    pub fn from_corners(corners: impl IntoIterator<Item = (u32, [f32; 2])>) -> Self {
        let mut out = Self::default();
        let mut seen: std::collections::HashMap<(u32, [u32; 2]), u32> =
            std::collections::HashMap::new();
        for (v, uv) in corners {
            let key = (v, uv.map(f32::to_bits));
            let i = *seen.entry(key).or_insert_with(|| {
                out.map.push(v);
                out.uvs.push(uv);
                out.map.len() as u32 - 1
            });
            out.indices.push(i);
        }
        out
    }

    pub fn vertex_count(&self) -> usize {
        self.map.len()
    }

    /// Per-render-vertex copy of per-base-vertex data.
    pub fn gather<T: Copy>(&self, per_base: &[T]) -> Vec<T> {
        self.map.iter().map(|&b| per_base[b as usize]).collect()
    }

    /// Check consistency with `base` (same triangles, valid indices).
    pub fn matches(&self, base: &Mesh) -> bool {
        self.map.len() == self.uvs.len()
            && self.indices.len() == base.indices.len()
            && self.map.iter().all(|&b| (b as usize) < base.vertex_count())
            && self
                .indices
                .iter()
                .zip(&base.indices)
                .all(|(&r, &b)| (r as usize) < self.map.len() && self.map[r as usize] == b)
    }
}

/// Per-vertex tangents `[x, y, z, w]` for normal mapping (glTF convention):
/// `xyz` follows increasing u; `w` is the handedness such that
/// `cross(normal, xyz) * w` points up the texture (decreasing v, as glTF
/// normal maps use +Y up the image with v running down).
pub fn compute_tangents(
    positions: &[Vec3],
    normals: &[Vec3],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Vec<[f32; 4]> {
    let n = positions.len();
    let (mut tan, mut bit) = (vec![Vec3::ZERO; n], vec![Vec3::ZERO; n]);
    for t in indices.as_chunks::<3>().0 {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| i as usize);
        let (e1, e2) = (positions[b] - positions[a], positions[c] - positions[a]);
        let (d1, d2) = (
            [uvs[b][0] - uvs[a][0], uvs[b][1] - uvs[a][1]],
            [uvs[c][0] - uvs[a][0], uvs[c][1] - uvs[a][1]],
        );
        let det = d1[0] * d2[1] - d2[0] * d1[1];
        if det.abs() < 1e-12 {
            continue;
        }
        let r = 1.0 / det;
        let du = (e1 * d2[1] - e2 * d1[1]) * r; // dP/du
        let dv = (e2 * d1[0] - e1 * d2[0]) * r; // dP/dv
        for i in [a, b, c] {
            tan[i] += du;
            bit[i] += dv;
        }
    }
    (0..n)
        .map(|i| {
            let nrm = normals[i];
            let t = (tan[i] - nrm * nrm.dot(tan[i]))
                .try_normalize()
                .unwrap_or_else(|| nrm.any_orthonormal_vector());
            let up = -bit[i]; // up the image
            let w = if nrm.cross(t).dot(up) < 0.0 {
                -1.0
            } else {
                1.0
            };
            [t.x, t.y, t.z, w]
        })
        .collect()
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

    /// Evaluate the auxiliary points for the same weights.
    pub fn evaluate_aux(&self, weights: &[f32]) -> Vec<Vec3> {
        let mut out = self.aux.clone();
        for (target, &w) in self.targets.iter().zip(weights) {
            if w.abs() < WEIGHT_EPSILON {
                continue;
            }
            for (&i, &d) in target.aux_indices.iter().zip(&target.aux_deltas) {
                out[i as usize] += d * w;
            }
        }
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
    fn render_mesh_splits_seams_and_maps_back() {
        let base = quad();
        // Same corners, but vertex 0 has a different UV in the second triangle (a seam).
        let uv = |x: f32, y: f32| [x, y];
        let corners = [
            (0, uv(0.0, 0.0)),
            (1, uv(1.0, 0.0)),
            (2, uv(1.0, 1.0)),
            (0, uv(0.5, 0.5)),
            (2, uv(1.0, 1.0)),
            (3, uv(0.0, 1.0)),
        ];
        let r = RenderMesh::from_corners(corners);
        assert_eq!(r.vertex_count(), 5, "vertex 0 is split, vertex 2 shared");
        assert!(r.matches(&base));
        assert_eq!(r.gather(&[10, 11, 12, 13]), vec![10, 11, 12, 10, 13]);
        let mut bad = r.clone();
        bad.indices.swap(0, 1);
        assert!(!bad.matches(&base));
    }

    #[test]
    fn tangents_follow_u_and_handedness_points_up_the_image() {
        let m = quad();
        // u along +X, v running down the image = along -Y of the quad.
        let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        let n = m.compute_normals(&m.positions);
        let t = compute_tangents(&m.positions, &n, &uvs, &m.indices);
        for tv in t {
            assert!((Vec3::new(tv[0], tv[1], tv[2]) - Vec3::X).length() < 1e-5);
            // cross(+Z, +X) = +Y, which is "up the image" here, so w = +1.
            assert_eq!(tv[3], 1.0);
        }
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
            aux: vec![],
            base,
            targets: vec![t],
        };
        assert_eq!(mm.evaluate(&[0.5])[2].z, 1.0);
        assert_eq!(mm.evaluate(&[-1.0])[2].z, -2.0);
        assert_eq!(mm.evaluate(&[])[2].z, 0.0);
    }
}
