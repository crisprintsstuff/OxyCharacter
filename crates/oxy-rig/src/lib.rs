//! Skeleton model, morph-driven refitting, posing and skinning.
//!
//! * **Joints** are named points defined by an *anchor*: weighted points of
//!   the character (body vertices, then auxiliary points — see
//!   `oxy_mesh::MorphableMesh::aux`). After morphing, a joint sits at the
//!   weighted centroid of its anchor, so the skeleton follows any body shape.
//! * **Bones** run from a head joint to a tail joint. In the bind pose a
//!   bone's local +Y points along the bone; its roll (local X) is taken from
//!   an optional *roll plane* of three joints, else from world axes.
//! * **Skinning** is linear blend skinning with up to four influences per
//!   vertex. The topology never changes, so weights stay valid for every shape.

pub mod pose;

use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum RigError {
    #[error("bone {0} has parent {1}, which is not an earlier bone")]
    BadParent(usize, usize),
    #[error("joint {0} has an empty or zero-weight anchor")]
    EmptyAnchor(usize),
    #[error("joint {0} anchor references point {1}, only {2} exist")]
    AnchorOutOfRange(usize, u32, usize),
    #[error("bone {0} references joint {1}, only {2} exist")]
    BadJoint(usize, usize, usize),
    #[error("skin has {0} vertices, mesh has {1}")]
    SkinSize(usize, usize),
    #[error("skin references bone {0}, only {1} exist")]
    BadInfluence(u16, usize),
}

/// A named point that follows the body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Joint {
    pub name: String,
    /// `(point index, weight)`; points are body vertices then aux points.
    pub anchor: Vec<(u32, f32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bone {
    pub name: String,
    /// Parent bone; parents always precede children.
    pub parent: Option<usize>,
    pub head: usize,
    pub tail: usize,
    /// Three joints spanning the plane whose normal gives the bone's roll.
    pub roll_plane: Option<[usize; 3]>,
}

/// Up to four bone influences per vertex; weights sum to 1 (or all zero).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct Influence {
    pub bones: [u16; 4],
    pub weights: [f32; 4],
}

impl Influence {
    /// Keep the four largest of `pairs` and normalise them.
    pub fn from_pairs(mut pairs: Vec<(u16, f32)>) -> Self {
        pairs.retain(|p| p.1 > 0.0);
        pairs.sort_by(|a, b| b.1.total_cmp(&a.1));
        pairs.truncate(4);
        let sum: f32 = pairs.iter().map(|p| p.1).sum();
        let mut out = Self::default();
        for (k, (b, w)) in pairs.into_iter().enumerate() {
            out.bones[k] = b;
            out.weights[k] = if sum > 0.0 { w / sum } else { 0.0 };
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Rig {
    pub name: String,
    pub joints: Vec<Joint>,
    pub bones: Vec<Bone>,
    /// One entry per body vertex.
    pub skin: Vec<Influence>,
    /// For each aux point, body vertices whose motion approximates it. Used to
    /// derive aux deltas for morphs that don't provide them (e.g. sculpts).
    #[serde(default)]
    pub aux_from_body: Vec<Vec<(u32, f32)>>,
}

impl Rig {
    pub fn bone_index(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }

    pub fn validate(&self, body_vertices: usize, aux_points: usize) -> Result<(), RigError> {
        let points = body_vertices + aux_points;
        for (i, j) in self.joints.iter().enumerate() {
            if j.anchor.iter().map(|a| a.1).sum::<f32>() <= 0.0 {
                return Err(RigError::EmptyAnchor(i));
            }
            if let Some(&(v, _)) = j.anchor.iter().find(|a| a.0 as usize >= points) {
                return Err(RigError::AnchorOutOfRange(i, v, points));
            }
        }
        for (i, b) in self.bones.iter().enumerate() {
            if let Some(p) = b.parent {
                if p >= i {
                    return Err(RigError::BadParent(i, p));
                }
            }
            for j in [b.head, b.tail]
                .into_iter()
                .chain(b.roll_plane.into_iter().flatten())
            {
                if j >= self.joints.len() {
                    return Err(RigError::BadJoint(i, j, self.joints.len()));
                }
            }
        }
        if self.skin.len() != body_vertices {
            return Err(RigError::SkinSize(self.skin.len(), body_vertices));
        }
        for inf in &self.skin {
            for (k, &b) in inf.bones.iter().enumerate() {
                if inf.weights[k] > 0.0 && b as usize >= self.bones.len() {
                    return Err(RigError::BadInfluence(b, self.bones.len()));
                }
            }
        }
        Ok(())
    }

    /// Joint positions for body positions + aux points (in that order).
    pub fn joint_positions(&self, body: &[Vec3], aux: &[Vec3]) -> Vec<Vec3> {
        let point = |i: u32| {
            let i = i as usize;
            if i < body.len() {
                body[i]
            } else {
                aux[i - body.len()]
            }
        };
        self.joints
            .iter()
            .map(|j| {
                let total: f32 = j.anchor.iter().map(|a| a.1).sum();
                j.anchor.iter().map(|&(v, w)| point(v) * w).sum::<Vec3>() / total
            })
            .collect()
    }

    /// World-space bind matrices: translation at the bone head, +Y along the bone.
    pub fn bind_matrices(&self, joints: &[Vec3]) -> Vec<Mat4> {
        self.bones
            .iter()
            .map(|b| {
                let head = joints[b.head];
                let y = (joints[b.tail] - head).try_normalize().unwrap_or(Vec3::Y);
                // Roll reference: roll-plane normal, else world X (or Z for bones along X).
                let reference = b
                    .roll_plane
                    .and_then(|[p0, p1, p2]| {
                        (joints[p1] - joints[p0])
                            .cross(joints[p2] - joints[p0])
                            .try_normalize()
                    })
                    .filter(|n| n.cross(y).length() > 0.1)
                    .unwrap_or(if y.x.abs() < 0.9 { Vec3::X } else { Vec3::Z });
                let x = (reference - y * reference.dot(y)).normalize();
                let z = x.cross(y);
                Mat4::from_cols(
                    x.extend(0.0),
                    y.extend(0.0),
                    z.extend(0.0),
                    head.extend(1.0),
                )
            })
            .collect()
    }

    /// Bind transforms relative to each bone's parent (glTF node TRS).
    pub fn local_bind(&self, bind: &[Mat4]) -> Vec<Mat4> {
        self.bones
            .iter()
            .enumerate()
            .map(|(i, b)| b.parent.map_or(bind[i], |p| bind[p].inverse() * bind[i]))
            .collect()
    }

    /// World matrices after rotating each bone by `local[b]` (in its own frame).
    pub fn pose_matrices(&self, bind: &[Mat4], local: &[Quat]) -> Vec<Mat4> {
        let rel = self.local_bind(bind);
        let mut out: Vec<Mat4> = Vec::with_capacity(self.bones.len());
        for (i, b) in self.bones.iter().enumerate() {
            let parent = b.parent.map_or(Mat4::IDENTITY, |p| out[p]);
            out.push(
                parent * rel[i] * Mat4::from_quat(local.get(i).copied().unwrap_or(Quat::IDENTITY)),
            );
        }
        out
    }

    /// Set `local[bone]` so the bone points along `world_dir`, given the
    /// already-posed parents (`local` is updated in bone order by callers).
    pub fn aim(&self, bind: &[Mat4], local: &mut [Quat], bone: usize, world_dir: Vec3) {
        let rel = self.local_bind(bind);
        let posed = self.pose_matrices(bind, local);
        let parent = self.bones[bone].parent.map_or(Mat4::IDENTITY, |p| posed[p]);
        let (_, w0, _) = (parent * rel[bone]).to_scale_rotation_translation();
        let current = w0 * Vec3::Y;
        let Some(d) = world_dir.try_normalize() else {
            return;
        };
        let arc = Quat::from_rotation_arc(current, d);
        local[bone] = (w0.inverse() * arc * w0).normalize();
    }

    /// Linear blend skinning of positions and normals.
    pub fn skin_points(
        &self,
        positions: &[Vec3],
        normals: &[Vec3],
        bind: &[Mat4],
        posed: &[Mat4],
    ) -> (Vec<Vec3>, Vec<Vec3>) {
        let mats: Vec<Mat4> = posed
            .iter()
            .zip(bind)
            .map(|(p, b)| *p * b.inverse())
            .collect();
        skin_with(&self.skin, positions, normals, &mats)
    }

    /// Aux deltas implied by body deltas (for morphs without their own).
    pub fn derive_aux_deltas(
        &self,
        body_count: usize,
        indices: &[u32],
        deltas: &[Vec3],
    ) -> (Vec<u32>, Vec<Vec3>) {
        let mut dense = vec![Vec3::ZERO; body_count];
        for (&i, &d) in indices.iter().zip(deltas) {
            dense[i as usize] = d;
        }
        let (mut ai, mut ad) = (Vec::new(), Vec::new());
        for (k, src) in self.aux_from_body.iter().enumerate() {
            let total: f32 = src.iter().map(|s| s.1).sum();
            if total <= 0.0 {
                continue;
            }
            let d = src
                .iter()
                .map(|&(v, w)| dense[v as usize] * w)
                .sum::<Vec3>()
                / total;
            if d.length_squared() > 0.0 {
                ai.push(k as u32);
                ad.push(d);
            }
        }
        (ai, ad)
    }
}

/// Skin with precomputed skinning matrices (`posed * bind⁻¹`).
pub fn skin_with(
    skin: &[Influence],
    positions: &[Vec3],
    normals: &[Vec3],
    mats: &[Mat4],
) -> (Vec<Vec3>, Vec<Vec3>) {
    let mut p = Vec::with_capacity(positions.len());
    let mut n = Vec::with_capacity(positions.len());
    for (i, inf) in skin.iter().enumerate() {
        let (mut sp, mut sn, mut used) = (Vec3::ZERO, Vec3::ZERO, 0.0);
        for k in 0..4 {
            let w = inf.weights[k];
            if w > 0.0 {
                let m = &mats[inf.bones[k] as usize];
                sp += m.transform_point3(positions[i]) * w;
                sn += m.transform_vector3(normals[i]) * w;
                used += w;
            }
        }
        if used == 0.0 {
            p.push(positions[i]);
            n.push(normals[i]);
        } else {
            p.push(sp / used);
            n.push(sn.normalize_or_zero());
        }
    }
    (p, n)
}

/// Copy influences from the nearest body vertex to each point (simple weight
/// transfer for accessories). Uses a uniform grid, so it is fast enough for
/// accessory meshes with tens of thousands of vertices.
pub fn transfer_nearest(body: &[Vec3], skin: &[Influence], points: &[Vec3]) -> Vec<Influence> {
    if body.is_empty() {
        return vec![Influence::default(); points.len()];
    }
    let (lo, hi) = body.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(l, h), &p| (l.min(p), h.max(p)),
    );
    let cell = ((hi - lo).max_element() / 64.0).max(1e-4);
    let key = |p: Vec3| ((p - lo) / cell).floor().as_ivec3();
    let mut grid: std::collections::HashMap<glam::IVec3, Vec<u32>> =
        std::collections::HashMap::new();
    for (i, &p) in body.iter().enumerate() {
        grid.entry(key(p)).or_default().push(i as u32);
    }
    points
        .iter()
        .map(|&p| {
            let c = key(p);
            let mut best = (f32::MAX, 0u32);
            // Grow the search shell until something is found and the shell is
            // farther than the best hit.
            for r in 0..=8i32 {
                for x in -r..=r {
                    for y in -r..=r {
                        for z in -r..=r {
                            if x.abs().max(y.abs()).max(z.abs()) != r {
                                continue;
                            }
                            if let Some(list) = grid.get(&(c + glam::IVec3::new(x, y, z))) {
                                for &i in list {
                                    let d = body[i as usize].distance_squared(p);
                                    if d < best.0 {
                                        best = (d, i);
                                    }
                                }
                            }
                        }
                    }
                }
                if best.0 < f32::MAX && (r as f32 * cell).powi(2) >= best.0 {
                    break;
                }
            }
            // Far from the body (or the shell search was inconclusive): scan everything.
            if best.0 == f32::MAX || best.0 > (8.0 * cell).powi(2) {
                for (i, b) in body.iter().enumerate() {
                    let d = b.distance_squared(p);
                    if d < best.0 {
                        best = (d, i as u32);
                    }
                }
            }
            skin[best.1 as usize]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A vertical two-bone "arm" with vertices along it.
    fn arm() -> (Rig, Vec<Vec3>) {
        let body: Vec<Vec3> = (0..=4)
            .map(|i| Vec3::new(0.0, i as f32 * 0.5, 0.0))
            .collect();
        let joint = |name: &str, v: u32| Joint {
            name: name.into(),
            anchor: vec![(v, 1.0)],
        };
        let rig = Rig {
            name: "arm".into(),
            joints: vec![joint("shoulder", 0), joint("elbow", 2), joint("hand", 4)],
            bones: vec![
                Bone {
                    name: "upper".into(),
                    parent: None,
                    head: 0,
                    tail: 1,
                    roll_plane: None,
                },
                Bone {
                    name: "lower".into(),
                    parent: Some(0),
                    head: 1,
                    tail: 2,
                    roll_plane: None,
                },
            ],
            skin: vec![
                Influence::from_pairs(vec![(0, 1.0)]),
                Influence::from_pairs(vec![(0, 1.0)]),
                Influence::from_pairs(vec![(0, 0.5), (1, 0.5)]),
                Influence::from_pairs(vec![(1, 1.0)]),
                Influence::from_pairs(vec![(1, 1.0)]),
            ],
            aux_from_body: vec![],
        };
        (rig, body)
    }

    #[test]
    fn validates_and_reports_problems() {
        let (mut rig, body) = arm();
        assert_eq!(rig.validate(body.len(), 0), Ok(()));
        assert_eq!(rig.validate(4, 0), Err(RigError::AnchorOutOfRange(2, 4, 4)));
        rig.bones[0].parent = Some(1);
        assert_eq!(rig.validate(body.len(), 0), Err(RigError::BadParent(0, 1)));
    }

    #[test]
    fn influences_keep_top_four_normalised() {
        let i = Influence::from_pairs(vec![
            (1, 0.1),
            (2, 0.4),
            (3, 0.2),
            (4, 0.2),
            (5, 0.1),
            (6, 0.0),
        ]);
        assert_eq!(i.bones[0], 2);
        assert!((i.weights.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn bind_pose_is_identity_skinning_and_follows_morphs() {
        let (rig, body) = arm();
        let joints = rig.joint_positions(&body, &[]);
        assert_eq!(joints[1], Vec3::new(0.0, 1.0, 0.0));
        let bind = rig.bind_matrices(&joints);
        // +Y along the bone, origin at the head.
        assert!((bind[1].transform_vector3(Vec3::Y) - Vec3::Y).length() < 1e-6);
        let normals = vec![Vec3::X; body.len()];
        let posed = rig.pose_matrices(&bind, &[Quat::IDENTITY; 2]);
        let (p, _) = rig.skin_points(&body, &normals, &bind, &posed);
        for (a, b) in p.iter().zip(&body) {
            assert!(a.distance(*b) < 1e-5);
        }
        // A "longer arm" morph moves the elbow joint with its anchor.
        let mut longer = body.clone();
        longer.iter_mut().for_each(|v| v.y *= 1.5);
        assert_eq!(rig.joint_positions(&longer, &[])[1].y, 1.5);
    }

    #[test]
    fn aim_bends_the_forearm() {
        let (rig, body) = arm();
        let bind = rig.bind_matrices(&rig.joint_positions(&body, &[]));
        let mut local = vec![Quat::IDENTITY; 2];
        rig.aim(&bind, &mut local, 1, Vec3::X);
        let posed = rig.pose_matrices(&bind, &local);
        // The forearm now points along +X; the hand vertex follows it.
        assert!((posed[1].transform_vector3(Vec3::Y) - Vec3::X).length() < 1e-5);
        let (p, _) = rig.skin_points(&body, &[Vec3::Z; 5], &bind, &posed);
        assert!(
            (p[4] - Vec3::new(1.0, 1.0, 0.0)).length() < 1e-5,
            "{:?}",
            p[4]
        );
        assert!((p[0] - body[0]).length() < 1e-6, "shoulder stays");
    }

    #[test]
    fn aux_deltas_and_weight_transfer() {
        let (mut rig, body) = arm();
        rig.aux_from_body = vec![vec![(1, 1.0), (3, 1.0)]];
        let (ai, ad) = rig.derive_aux_deltas(body.len(), &[1, 3], &[Vec3::X, Vec3::X * 3.0]);
        assert_eq!(ai, vec![0]);
        assert_eq!(ad, vec![Vec3::X * 2.0]);
        let inf = transfer_nearest(
            &body,
            &rig.skin,
            &[Vec3::new(0.1, 1.9, 0.0), Vec3::new(5.0, -3.0, 0.0)],
        );
        assert_eq!(inf[0], rig.skin[4]);
        assert_eq!(inf[1], rig.skin[0]);
    }
}
