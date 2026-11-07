//! The skin material: a few user parameters → a layer stack.
//!
//! Face regions (lips, eyelids, cheeks, brows) are derived from skin weights
//! of the rig's face bones, so they follow any face shape and need no painted
//! masks.

use glam::Vec3;
use oxy_rig::Rig;
use serde::{Deserialize, Serialize};

use crate::layers::{Blend, Channel, Layer, Mask, Source};

/// Region index constants for [`REGIONS`].
pub const LIPS: usize = 0;
pub const EYELIDS: usize = 1;
pub const CHEEKS: usize = 2;
pub const BROWS: usize = 3;

/// Region → bones whose summed skin weight defines it.
pub const REGIONS: &[(&str, &[&str])] = &[
    (
        "lips",
        &[
            "oris01", "oris03.L", "oris03.R", "oris05", "oris07.L", "oris07.R",
        ],
    ),
    (
        "eyelids",
        &[
            "orbicularis03.L",
            "orbicularis03.R",
            "orbicularis04.L",
            "orbicularis04.R",
        ],
    ),
    (
        "cheeks",
        &["risorius03.L", "risorius03.R", "levator06.L", "levator06.R"],
    ),
    ("brows", &["oculi01.L", "oculi01.R"]),
];

/// Per region, per body vertex weight in `0..1` (zeros for bones the rig lacks).
pub fn region_weights(rig: &Rig) -> Vec<Vec<f32>> {
    REGIONS
        .iter()
        .map(|(_, bones)| {
            let ids: Vec<u16> = bones
                .iter()
                .filter_map(|b| rig.bone_index(b))
                .map(|i| i as u16)
                .collect();
            rig.skin
                .iter()
                .map(|inf| {
                    (0..4)
                        .filter(|&k| ids.contains(&inf.bones[k]))
                        .map(|k| inf.weights[k])
                        .sum::<f32>()
                        .min(1.0)
                })
                .collect()
        })
        .collect()
}

/// User-facing skin settings. Colours are linear RGB.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkinParams {
    /// Blotchy tone variation, 0..1.
    pub variation: f32,
    /// Fine surface detail (normal map + roughness variation), 0..1.
    pub detail: f32,
    /// Base roughness.
    pub roughness: f32,
    pub freckles: f32,
    pub freckle_color: [f32; 3],
    pub blush: f32,
    pub blush_color: [f32; 3],
    pub eye_shadow: f32,
    pub eye_shadow_color: [f32; 3],
    pub lips: f32,
    pub lip_color: [f32; 3],
    pub seed: u32,
}

impl Default for SkinParams {
    fn default() -> Self {
        Self {
            variation: 0.35,
            detail: 0.5,
            roughness: 0.55,
            freckles: 0.0,
            freckle_color: [0.30, 0.16, 0.09],
            blush: 0.15,
            blush_color: [0.75, 0.25, 0.25],
            eye_shadow: 0.0,
            eye_shadow_color: [0.25, 0.18, 0.30],
            lips: 0.35,
            lip_color: [0.55, 0.20, 0.22],
            seed: 1,
        }
    }
}

/// The layer stack for skin of linear tone `tone`.
pub fn skin_layers(tone: [f32; 3], p: &SkinParams) -> Vec<Layer> {
    let t = Vec3::from_array(tone);
    let seed = p.seed;
    let layer = |channel, source, mask, blend, opacity: f32| Layer {
        channel,
        source,
        mask,
        blend,
        opacity,
    };
    let region = |region, lo, hi| Mask::Region { region, lo, hi };
    vec![
        layer(
            Channel::Albedo,
            Source::Color(tone),
            Mask::Full,
            Blend::Normal,
            1.0,
        ),
        // Large, soft blotches (warm/cool shifts), then finer mottling.
        layer(
            Channel::Albedo,
            Source::Noise {
                frequency: 5.0,
                octaves: 3,
                seed,
                low: [0.86, 0.84, 0.86],
                high: [1.10, 1.06, 1.02],
            },
            Mask::Full,
            Blend::Multiply,
            p.variation,
        ),
        layer(
            Channel::Albedo,
            Source::Noise {
                frequency: 45.0,
                octaves: 2,
                seed: seed + 1,
                low: [0.93, 0.92, 0.93],
                high: [1.04, 1.03, 1.02],
            },
            Mask::Full,
            Blend::Multiply,
            p.variation * 0.7,
        ),
        layer(
            Channel::Albedo,
            Source::Color(
                (t * Vec3::from_array(p.freckle_color) * 2.2)
                    .min(Vec3::ONE)
                    .to_array(),
            ),
            Mask::Noise {
                frequency: 160.0,
                seed: seed + 2,
                threshold: 0.74 - 0.12 * p.freckles,
                softness: 0.025,
            },
            Blend::Normal,
            p.freckles,
        ),
        layer(
            Channel::Albedo,
            Source::Color(p.blush_color),
            region(CHEEKS, 0.05, 0.7),
            Blend::Overlay,
            p.blush,
        ),
        layer(
            Channel::Albedo,
            Source::Color(p.eye_shadow_color),
            region(EYELIDS, 0.1, 0.8),
            Blend::Multiply,
            p.eye_shadow,
        ),
        layer(
            Channel::Albedo,
            Source::Color(p.lip_color),
            region(LIPS, 0.3, 0.8),
            Blend::Overlay,
            p.lips,
        ),
        // Roughness: base, fine variation, lips smoother.
        layer(
            Channel::Roughness,
            Source::Color([p.roughness; 3]),
            Mask::Full,
            Blend::Normal,
            1.0,
        ),
        layer(
            Channel::Roughness,
            Source::Noise {
                frequency: 120.0,
                octaves: 2,
                seed: seed + 3,
                low: [p.roughness - 0.1; 3],
                high: [p.roughness + 0.1; 3],
            },
            Mask::Full,
            Blend::Normal,
            p.detail,
        ),
        layer(
            Channel::Roughness,
            Source::Color([0.35; 3]),
            region(LIPS, 0.4, 0.9),
            Blend::Normal,
            0.8,
        ),
        // Height: soft bumps plus finer grain.
        layer(
            Channel::Height,
            Source::Noise {
                frequency: 70.0,
                octaves: 3,
                seed: seed + 4,
                low: [0.4; 3],
                high: [0.6; 3],
            },
            Mask::Full,
            Blend::Normal,
            p.detail,
        ),
        layer(
            Channel::Height,
            Source::Noise {
                frequency: 260.0,
                octaves: 2,
                seed: seed + 5,
                low: [-0.04; 3],
                high: [0.04; 3],
            },
            Mask::Full,
            Blend::Add,
            p.detail,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxy_rig::{Bone, Influence, Joint};

    #[test]
    fn regions_come_from_bone_weights() {
        let joint = Joint {
            name: "j".into(),
            anchor: vec![(0, 1.0)],
        };
        let bone = |n: &str| Bone {
            name: n.into(),
            parent: None,
            head: 0,
            tail: 0,
            roll_plane: None,
        };
        let rig = Rig {
            name: "r".into(),
            joints: vec![joint],
            bones: vec![bone("head"), bone("oris01"), bone("oris05")],
            skin: vec![
                Influence::from_pairs(vec![(0, 1.0)]),
                Influence::from_pairs(vec![(0, 0.5), (1, 0.25), (2, 0.25)]),
            ],
            aux_from_body: vec![],
        };
        let r = region_weights(&rig);
        assert_eq!(r.len(), REGIONS.len());
        assert_eq!(r[LIPS], vec![0.0, 0.5]);
        assert_eq!(r[CHEEKS], vec![0.0, 0.0], "missing bones give zero");
    }

    #[test]
    fn stack_scales_with_parameters() {
        let quiet = SkinParams {
            variation: 0.0,
            detail: 0.0,
            freckles: 0.0,
            blush: 0.0,
            lips: 0.0,
            eye_shadow: 0.0,
            ..Default::default()
        };
        let l = skin_layers([0.5, 0.4, 0.3], &quiet);
        assert!(l
            .iter()
            .skip(1)
            .filter(|l| l.channel == Channel::Albedo)
            .all(|l| l.opacity == 0.0));
    }
}
