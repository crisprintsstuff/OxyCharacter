//! Skeleton model and morph-driven joint refitting.
//!
//! Each joint has an *anchor*: weighted vertices on the base mesh (e.g. a ring
//! of vertices around the knee). After the body is morphed, the joint is moved
//! to the weighted centroid of its anchor. Topology is fixed, so skin weights
//! stay valid and only the bind pose needs regenerating.

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum RigError {
    #[error("joint {0} has parent {1}, which is not an earlier joint")]
    BadParent(usize, usize),
    #[error("joint {0} has an empty or zero-weight anchor")]
    EmptyAnchor(usize),
    #[error("joint {0} anchor references vertex {1}, mesh has {2}")]
    AnchorOutOfRange(usize, u32, usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Joint {
    pub name: String,
    /// Parent index; parents always precede children.
    pub parent: Option<usize>,
    /// `(vertex index, weight)` pairs on the base mesh.
    pub anchor: Vec<(u32, f32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skeleton {
    pub joints: Vec<Joint>,
}

impl Skeleton {
    pub fn validate(&self, vertex_count: usize) -> Result<(), RigError> {
        for (i, j) in self.joints.iter().enumerate() {
            if let Some(p) = j.parent {
                if p >= i {
                    return Err(RigError::BadParent(i, p));
                }
            }
            if j.anchor.iter().map(|a| a.1).sum::<f32>() <= 0.0 {
                return Err(RigError::EmptyAnchor(i));
            }
            if let Some(&(v, _)) = j.anchor.iter().find(|a| a.0 as usize >= vertex_count) {
                return Err(RigError::AnchorOutOfRange(i, v, vertex_count));
            }
        }
        Ok(())
    }

    /// World-space joint positions for the given (morphed) vertex positions.
    pub fn fit(&self, positions: &[Vec3]) -> Vec<Vec3> {
        self.joints
            .iter()
            .map(|j| {
                let total: f32 = j.anchor.iter().map(|a| a.1).sum();
                j.anchor
                    .iter()
                    .map(|&(v, w)| positions[v as usize] * w)
                    .sum::<Vec3>()
                    / total
            })
            .collect()
    }

    /// Joint positions relative to their parent (local translations for export).
    pub fn local_translations(&self, world: &[Vec3]) -> Vec<Vec3> {
        self.joints
            .iter()
            .enumerate()
            .map(|(i, j)| j.parent.map_or(world[i], |p| world[i] - world[p]))
            .collect()
    }

    /// Inverse bind matrices for an unrotated bind pose at `world` positions.
    /// (Joint orientation solving is planned for milestone M2.)
    pub fn inverse_bind_matrices(&self, world: &[Vec3]) -> Vec<Mat4> {
        world.iter().map(|&p| Mat4::from_translation(-p)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leg() -> Skeleton {
        Skeleton {
            joints: vec![
                Joint {
                    name: "hip".into(),
                    parent: None,
                    anchor: vec![(0, 1.0), (1, 1.0)],
                },
                Joint {
                    name: "knee".into(),
                    parent: Some(0),
                    anchor: vec![(2, 1.0), (3, 3.0)],
                },
            ],
        }
    }

    #[test]
    fn validation_catches_errors() {
        let mut s = leg();
        assert_eq!(s.validate(4), Ok(()));
        assert_eq!(s.validate(3), Err(RigError::AnchorOutOfRange(1, 3, 3)));
        s.joints[0].parent = Some(1);
        assert_eq!(s.validate(4), Err(RigError::BadParent(0, 1)));
    }

    #[test]
    fn joints_follow_morphed_vertices() {
        let s = leg();
        let mut p = vec![
            Vec3::new(-1.0, 10.0, 0.0),
            Vec3::new(1.0, 10.0, 0.0),
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(0.0, 5.0, 0.0),
        ];
        assert_eq!(
            s.fit(&p),
            vec![Vec3::new(0.0, 10.0, 0.0), Vec3::new(0.0, 5.0, 0.0)]
        );

        // A "taller" morph moves the hip vertices up; the knee stays put.
        p[0].y = 12.0;
        p[1].y = 12.0;
        let w = s.fit(&p);
        assert_eq!(s.local_translations(&w)[1], Vec3::new(0.0, -7.0, 0.0));
    }
}
