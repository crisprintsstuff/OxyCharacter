//! Named pose presets, expressed as bone aiming rules.
//!
//! A rule aims a bone (by name) along a world direction. Rules are applied
//! parents-first, so chains straighten naturally. Bones a rig doesn't have are
//! skipped, which keeps presets usable across skeletons with similar naming.

use glam::{Mat4, Quat, Vec3};

use crate::Rig;

pub struct Preset {
    pub name: &'static str,
    /// `(bone name, world direction)`; the character faces +Z, its left is +X.
    pub rules: &'static [(&'static str, [f32; 3])],
}

const LEFT_ARM: [&str; 5] = [
    "upperarm01.L",
    "upperarm02.L",
    "lowerarm01.L",
    "lowerarm02.L",
    "wrist.L",
];
const RIGHT_ARM: [&str; 5] = [
    "upperarm01.R",
    "upperarm02.R",
    "lowerarm01.R",
    "lowerarm02.R",
    "wrist.R",
];

macro_rules! arm {
    ($side:expr, $upper:expr, $lower:expr) => {
        [
            ($side[0], $upper),
            ($side[1], $upper),
            ($side[2], $lower),
            ($side[3], $lower),
            ($side[4], $lower),
        ]
    };
}

const T_POSE: [(&str, [f32; 3]); 10] = {
    let l = arm!(LEFT_ARM, [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    let r = arm!(RIGHT_ARM, [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]);
    [l[0], l[1], l[2], l[3], l[4], r[0], r[1], r[2], r[3], r[4]]
};
const RELAXED: [(&str, [f32; 3]); 10] = {
    let l = arm!(LEFT_ARM, [0.2, -1.0, 0.02], [0.12, -1.0, 0.12]);
    let r = arm!(RIGHT_ARM, [-0.2, -1.0, 0.02], [-0.12, -1.0, 0.12]);
    [l[0], l[1], l[2], l[3], l[4], r[0], r[1], r[2], r[3], r[4]]
};
const WAVE: [(&str, [f32; 3]); 10] = {
    let l = arm!(LEFT_ARM, [0.2, -1.0, 0.02], [0.12, -1.0, 0.12]);
    let r = arm!(RIGHT_ARM, [-1.0, 0.35, 0.1], [-0.15, 1.0, 0.1]);
    [l[0], l[1], l[2], l[3], l[4], r[0], r[1], r[2], r[3], r[4]]
};
const STRIDE: [(&str, [f32; 3]); 18] = {
    let l = arm!(LEFT_ARM, [0.15, -1.0, -0.35], [0.1, -1.0, 0.05]);
    let r = arm!(RIGHT_ARM, [-0.15, -1.0, 0.35], [-0.1, -0.8, 0.6]);
    [
        l[0],
        l[1],
        l[2],
        l[3],
        l[4],
        r[0],
        r[1],
        r[2],
        r[3],
        r[4],
        ("upperleg01.L", [0.03, -1.0, 0.42]),
        ("upperleg02.L", [0.03, -1.0, 0.42]),
        ("lowerleg01.L", [0.0, -1.0, 0.05]),
        ("lowerleg02.L", [0.0, -1.0, 0.05]),
        ("upperleg01.R", [-0.03, -1.0, -0.3]),
        ("upperleg02.R", [-0.03, -1.0, -0.3]),
        ("lowerleg01.R", [0.0, -1.0, -0.6]),
        ("lowerleg02.R", [0.0, -1.0, -0.6]),
    ]
};

/// The rest pose (as modelled, an A-pose for the MakeHuman base) is the empty preset.
pub const PRESETS: &[Preset] = &[
    Preset {
        name: "Rest (A-pose)",
        rules: &[],
    },
    Preset {
        name: "T-pose",
        rules: &T_POSE,
    },
    Preset {
        name: "Relaxed",
        rules: &RELAXED,
    },
    Preset {
        name: "Wave",
        rules: &WAVE,
    },
    Preset {
        name: "Stride",
        rules: &STRIDE,
    },
];

pub fn preset(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

/// Local bone rotations for `preset` on a rig in bind pose `bind`.
pub fn solve(rig: &Rig, bind: &[Mat4], preset: &Preset) -> Vec<Quat> {
    let mut local = vec![Quat::IDENTITY; rig.bones.len()];
    let mut rules: Vec<(usize, Vec3)> = preset
        .rules
        .iter()
        .filter_map(|(name, d)| rig.bone_index(name).map(|i| (i, Vec3::from_array(*d))))
        .collect();
    rules.sort_by_key(|r| r.0); // bones are stored parents-first
    for (bone, dir) in rules {
        rig.aim(bind, &mut local, bone, dir);
    }
    local
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bone, Influence, Joint};

    #[test]
    fn t_pose_points_arm_sideways_and_skips_missing_bones() {
        // A left upper arm hanging down and slightly out.
        let rig = Rig {
            name: "t".into(),
            joints: vec![
                Joint {
                    name: "s".into(),
                    anchor: vec![(0, 1.0)],
                },
                Joint {
                    name: "e".into(),
                    anchor: vec![(1, 1.0)],
                },
            ],
            bones: vec![Bone {
                name: "upperarm01.L".into(),
                parent: None,
                head: 0,
                tail: 1,
                roll_plane: None,
            }],
            skin: vec![Influence::default(); 2],
            aux_from_body: vec![],
        };
        let bind = rig.bind_matrices(&[Vec3::new(0.2, 1.5, 0.0), Vec3::new(0.3, 1.2, 0.0)]);
        let local = solve(&rig, &bind, preset("T-pose").unwrap());
        let posed = rig.pose_matrices(&bind, &local);
        assert!((posed[0].transform_vector3(Vec3::Y) - Vec3::X).length() < 1e-5);
        assert_eq!(
            solve(&rig, &bind, preset("Rest (A-pose)").unwrap()),
            vec![Quat::IDENTITY]
        );
    }
}
