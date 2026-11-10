//! Facial expressions: units, presets, visemes, and how they reach a shaped face.
//!
//! An expression is a set of *unit* weights (0..1). Each unit is a sparse
//! target authored on the neutral base face (`expr/<unit>` in the mesh,
//! imported from the CC0 MakeHuman expression units). Presets and visemes are
//! OxyCharacter's own mixes of units.
//!
//! Units are authored on the base shape, so on a morphed face they are carried
//! over with [`oxy_mesh::transport`]: each delta is turned and scaled like
//! the surface around its vertex was, from the base shape to the shaped one.
//! Expressions move body vertices only; joints stay where the shape put them,
//! as they do when an engine plays the exported blendshapes.

use std::collections::BTreeMap;

use glam::Vec3;
use oxy_mesh::transport::Deformation;
use serde::{Deserialize, Serialize};

use crate::CharacterAsset;

/// Target-name prefix of expression units in the mesh.
pub const PREFIX: &str = "expr/";

/// One expression unit: a single facial movement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unit {
    /// Stable id (file name of the CC0 target, also the export name suffix).
    pub id: &'static str,
    pub label: &'static str,
    pub group: &'static str,
}

const fn u(id: &'static str, label: &'static str, group: &'static str) -> Unit {
    Unit { id, label, group }
}

/// Unit groups in display order.
pub const GROUPS: [&str; 5] = ["Brows", "Eyes", "Nose", "Mouth", "Neck"];

/// Every unit OxyCharacter knows, by group. Ids are stable: they name
/// project fields, plugin arguments and exported morph targets.
pub const UNITS: &[Unit] = &[
    u("eyebrows-left-up", "Left brow raise", "Brows"),
    u("eyebrows-right-up", "Right brow raise", "Brows"),
    u("eyebrows-left-inner-up", "Left inner brow raise", "Brows"),
    u("eyebrows-right-inner-up", "Right inner brow raise", "Brows"),
    u("eyebrows-left-extern-up", "Left outer brow raise", "Brows"),
    u(
        "eyebrows-right-extern-up",
        "Right outer brow raise",
        "Brows",
    ),
    u("eyebrows-left-down", "Left brow lower", "Brows"),
    u("eyebrows-right-down", "Right brow lower", "Brows"),
    u("eye-left-closure", "Left eye close", "Eyes"),
    u("eye-right-closure", "Right eye close", "Eyes"),
    u("eye-left-slit", "Left eye squint", "Eyes"),
    u("eye-right-slit", "Right eye squint", "Eyes"),
    u("eye-left-opened-up", "Left eye widen", "Eyes"),
    u("eye-right-opened-up", "Right eye widen", "Eyes"),
    u("nose-compression", "Nose wrinkle", "Nose"),
    u("nose-depression", "Nose tip down", "Nose"),
    u("nose-left-dilatation", "Left nostril flare", "Nose"),
    u("nose-right-dilatation", "Right nostril flare", "Nose"),
    u("nose-left-elevation", "Left nostril raise", "Nose"),
    u("nose-right-elevation", "Right nostril raise", "Nose"),
    u("mouth-open", "Jaw open", "Mouth"),
    u("mouth-corner-puller", "Corners up", "Mouth"),
    u("mouth-depression", "Corners down", "Mouth"),
    u("mouth-retraction", "Lips stretch", "Mouth"),
    u("mouth-upward-retraction", "Lips stretch up", "Mouth"),
    u("mouth-depression-retraction", "Lower lip down", "Mouth"),
    u("mouth-elevation", "Upper lip raise", "Mouth"),
    u("mouth-compression", "Lips press", "Mouth"),
    u("mouth-pursing", "Lips pucker", "Mouth"),
    u("mouth-protusion", "Lips push out", "Mouth"),
    u("mouth-eversion", "Lips roll out", "Mouth"),
    u("mouth-parling", "Upper lip lift", "Mouth"),
    u("mouth-part-later", "Lips part", "Mouth"),
    u("neck-platysma", "Neck tense", "Neck"),
];

pub fn unit(id: &str) -> Option<&'static Unit> {
    UNITS.iter().find(|u| u.id == id)
}

/// Units the character has targets for, in [`UNITS`] order.
pub fn available(asset: &CharacterAsset) -> impl Iterator<Item = &'static Unit> + '_ {
    UNITS.iter().filter(|u| {
        asset
            .mesh
            .target_index(&format!("{PREFIX}{}", u.id))
            .is_some()
    })
}

/// A named mix of units.
#[derive(Debug, Clone, Copy)]
pub struct Mix {
    pub id: &'static str,
    pub label: &'static str,
    pub units: &'static [(&'static str, f32)],
}

/// Expression presets (OxyCharacter's own mixes).
pub const PRESETS: &[Mix] = &[
    Mix {
        id: "smile",
        label: "Smile",
        units: &[
            ("mouth-corner-puller", 0.8),
            ("eye-left-slit", 0.25),
            ("eye-right-slit", 0.25),
        ],
    },
    Mix {
        id: "big-smile",
        label: "Big smile",
        units: &[
            ("mouth-corner-puller", 1.0),
            ("mouth-open", 0.3),
            ("mouth-upward-retraction", 0.4),
            ("eye-left-slit", 0.45),
            ("eye-right-slit", 0.45),
            ("eyebrows-left-up", 0.15),
            ("eyebrows-right-up", 0.15),
        ],
    },
    Mix {
        id: "sad",
        label: "Sad",
        units: &[
            ("mouth-depression", 0.8),
            ("eyebrows-left-inner-up", 0.8),
            ("eyebrows-right-inner-up", 0.8),
            ("eye-left-slit", 0.15),
            ("eye-right-slit", 0.15),
        ],
    },
    Mix {
        id: "angry",
        label: "Angry",
        units: &[
            ("eyebrows-left-down", 0.9),
            ("eyebrows-right-down", 0.9),
            ("mouth-compression", 0.6),
            ("nose-compression", 0.3),
            ("nose-left-dilatation", 0.4),
            ("nose-right-dilatation", 0.4),
            ("eye-left-slit", 0.3),
            ("eye-right-slit", 0.3),
        ],
    },
    Mix {
        id: "surprise",
        label: "Surprise",
        units: &[
            ("eyebrows-left-up", 1.0),
            ("eyebrows-right-up", 1.0),
            ("eye-left-opened-up", 0.8),
            ("eye-right-opened-up", 0.8),
            ("mouth-open", 0.55),
        ],
    },
    Mix {
        id: "fear",
        label: "Fear",
        units: &[
            ("eyebrows-left-inner-up", 0.8),
            ("eyebrows-right-inner-up", 0.8),
            ("eyebrows-left-up", 0.4),
            ("eyebrows-right-up", 0.4),
            ("eye-left-opened-up", 0.7),
            ("eye-right-opened-up", 0.7),
            ("mouth-retraction", 0.6),
            ("mouth-depression-retraction", 0.4),
            ("mouth-open", 0.2),
            ("neck-platysma", 0.5),
        ],
    },
    Mix {
        id: "disgust",
        label: "Disgust",
        units: &[
            ("nose-compression", 0.7),
            ("nose-left-elevation", 0.6),
            ("nose-right-elevation", 0.6),
            ("mouth-elevation", 0.6),
            ("mouth-depression", 0.3),
            ("eyebrows-left-down", 0.4),
            ("eyebrows-right-down", 0.4),
            ("eye-left-slit", 0.3),
            ("eye-right-slit", 0.3),
        ],
    },
    Mix {
        id: "wink-left",
        label: "Wink (left)",
        units: &[
            ("eye-left-closure", 1.0),
            ("mouth-corner-puller", 0.35),
            ("eye-right-slit", 0.1),
        ],
    },
    Mix {
        id: "blink",
        label: "Blink",
        units: &[("eye-left-closure", 1.0), ("eye-right-closure", 1.0)],
    },
    Mix {
        id: "kiss",
        label: "Kiss",
        units: &[("mouth-pursing", 1.0), ("mouth-protusion", 0.6)],
    },
];

/// Mouth shapes for speech (OxyCharacter's own set and mixes).
pub const VISEMES: &[Mix] = &[
    Mix {
        id: "rest",
        label: "Rest",
        units: &[],
    },
    Mix {
        id: "aa",
        label: "A (father)",
        units: &[("mouth-open", 0.7), ("mouth-part-later", 0.2)],
    },
    Mix {
        id: "ee",
        label: "E (bed)",
        units: &[("mouth-open", 0.35), ("mouth-retraction", 0.45)],
    },
    Mix {
        id: "ih",
        label: "I (see)",
        units: &[
            ("mouth-open", 0.15),
            ("mouth-retraction", 0.7),
            ("mouth-part-later", 0.2),
        ],
    },
    Mix {
        id: "oh",
        label: "O (go)",
        units: &[("mouth-open", 0.45), ("mouth-pursing", 0.55)],
    },
    Mix {
        id: "ou",
        label: "U (blue)",
        units: &[
            ("mouth-open", 0.1),
            ("mouth-pursing", 0.85),
            ("mouth-protusion", 0.5),
        ],
    },
    Mix {
        id: "mbp",
        label: "M B P",
        units: &[("mouth-compression", 0.8)],
    },
    Mix {
        id: "fv",
        label: "F V",
        units: &[
            ("mouth-elevation", 0.3),
            ("mouth-compression", 0.3),
            ("mouth-part-later", 0.15),
        ],
    },
    Mix {
        id: "l",
        label: "L",
        units: &[("mouth-open", 0.3), ("mouth-part-later", 0.3)],
    },
    Mix {
        id: "s",
        label: "S Z",
        units: &[
            ("mouth-open", 0.05),
            ("mouth-retraction", 0.5),
            ("mouth-part-later", 0.3),
        ],
    },
    Mix {
        id: "ch",
        label: "CH SH",
        units: &[
            ("mouth-open", 0.1),
            ("mouth-protusion", 0.5),
            ("mouth-eversion", 0.5),
            ("mouth-part-later", 0.3),
        ],
    },
    Mix {
        id: "th",
        label: "TH",
        units: &[("mouth-open", 0.2), ("mouth-part-later", 0.4)],
    },
];

pub fn preset(id: &str) -> Option<&'static Mix> {
    PRESETS.iter().find(|m| m.id == id)
}

pub fn viseme(id: &str) -> Option<&'static Mix> {
    VISEMES.iter().find(|m| m.id == id)
}

/// A face's expression: unit weights and eye gaze.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Expression {
    /// Unit id → weight (0..1). Units at 0 are left out.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub units: BTreeMap<String, f32>,
    /// Gaze `[yaw, pitch]` in degrees: + yaw looks to the character's left,
    /// + pitch looks up.
    #[serde(default, skip_serializing_if = "is_zero2")]
    pub gaze: [f32; 2],
}

fn is_zero2(g: &[f32; 2]) -> bool {
    *g == [0.0, 0.0]
}

/// How far the eyes may turn, in degrees (yaw, pitch).
pub const GAZE_LIMIT: [f32; 2] = [35.0, 25.0];

impl Expression {
    pub fn is_neutral(&self) -> bool {
        self.units.is_empty() && is_zero2(&self.gaze)
    }

    /// The expression of `mix` (gaze kept at zero).
    pub fn from_mix(mix: &Mix) -> Self {
        let mut e = Self::default();
        for &(id, w) in mix.units {
            e.set(id, w);
        }
        e
    }

    pub fn get(&self, id: &str) -> f32 {
        self.units.get(id).copied().unwrap_or(0.0)
    }

    /// Set a unit weight, clamped to 0..1; 0 removes it.
    pub fn set(&mut self, id: &str, w: f32) {
        let w = w.clamp(0.0, 1.0);
        if w == 0.0 || !w.is_finite() {
            self.units.remove(id);
        } else {
            self.units.insert(id.to_owned(), w);
        }
    }

    pub fn set_gaze(&mut self, yaw: f32, pitch: f32) {
        let c = |v: f32, l: f32| if v.is_finite() { v.clamp(-l, l) } else { 0.0 };
        self.gaze = [c(yaw, GAZE_LIMIT[0]), c(pitch, GAZE_LIMIT[1])];
    }

    /// Parse a comma-separated spec: preset or viseme ids (`smile`,
    /// `viseme.oh` or `oh`) and `unit:weight` pairs, applied left to right
    /// (a later unit weight replaces an earlier one).
    pub fn parse(spec: &str) -> Result<Self, String> {
        let mut e = Self::default();
        for item in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            if let Some((id, w)) = item.split_once(':') {
                let w: f32 = w.parse().map_err(|_| format!("bad weight in `{item}`"))?;
                if unit(id).is_none() {
                    return Err(format!("unknown expression unit `{id}`"));
                }
                e.set(id, w);
            } else {
                let id = item
                    .strip_prefix("face.")
                    .or_else(|| item.strip_prefix("viseme."))
                    .unwrap_or(item);
                let mix = preset(id)
                    .or_else(|| viseme(id))
                    .ok_or_else(|| format!("unknown expression `{item}`"))?;
                for &(u, w) in mix.units {
                    e.set(u, e.get(u).max(w));
                }
            }
        }
        Ok(e)
    }

    /// Unknown unit ids (kept, but they move nothing).
    pub fn unknown_units(&self) -> Vec<&str> {
        self.units
            .keys()
            .filter(|k| unit(k).is_none())
            .map(String::as_str)
            .collect()
    }
}

/// Edge rings in each vertex's fitted neighbourhood (about 1–2 cm on the face).
const RINGS: usize = 3;

/// Expression units of a character carried onto one shape of its body.
pub struct FaceShapes {
    /// Unit id → (indices, deltas) on the shaped body.
    units: BTreeMap<&'static str, (Vec<u32>, Vec<Vec3>)>,
}

impl FaceShapes {
    /// Transport every unit `asset` has onto `shaped` (body positions).
    pub fn new(asset: &CharacterAsset, shaped: &[Vec3]) -> Self {
        let mesh = &asset.mesh;
        let mut units = BTreeMap::new();
        let present: Vec<(&'static Unit, usize)> = UNITS
            .iter()
            .filter_map(|u| Some((u, mesh.target_index(&format!("{PREFIX}{}", u.id))?)))
            .collect();
        if present.is_empty() {
            return Self { units };
        }
        let mut touched: Vec<u32> = present
            .iter()
            .flat_map(|&(_, i)| mesh.targets[i].indices.iter().copied())
            .collect();
        touched.sort_unstable();
        touched.dedup();
        let field = Deformation::fit(&mesh.base, &mesh.base.positions, shaped, touched, RINGS);
        for (u, i) in present {
            let t = &mesh.targets[i];
            units.insert(
                u.id,
                (t.indices.clone(), field.apply(&t.indices, &t.deltas)),
            );
        }
        Self { units }
    }

    /// Units available (in [`UNITS`] order).
    pub fn available(&self) -> impl Iterator<Item = &'static Unit> + '_ {
        UNITS.iter().filter(|u| self.units.contains_key(u.id))
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// Add `Σ weight · unit` to `body`.
    pub fn apply<'a>(&self, weights: impl IntoIterator<Item = (&'a str, f32)>, body: &mut [Vec3]) {
        for (id, w) in weights {
            let Some((idx, d)) = self.units.get(id) else {
                continue;
            };
            if w == 0.0 {
                continue;
            }
            for (&i, &d) in idx.iter().zip(d) {
                body[i as usize] += d * w;
            }
        }
    }

    /// Apply an expression's units.
    pub fn apply_expression(&self, e: &Expression, body: &mut [Vec3]) {
        self.apply(e.units.iter().map(|(k, &w)| (k.as_str(), w)), body);
    }

    /// Dense body deltas of a weighted mix (for export).
    pub fn dense<'a>(
        &self,
        n: usize,
        weights: impl IntoIterator<Item = (&'a str, f32)>,
    ) -> Vec<Vec3> {
        let mut out = vec![Vec3::ZERO; n];
        self.apply(weights, &mut out);
        out
    }
}

/// Local rotation for an eye bone looking along `gaze` (degrees), as a
/// world-space rotation (the character faces +Z, its left is +X, Y up).
pub fn gaze_rotation(gaze: [f32; 2]) -> glam::Quat {
    let [yaw, pitch] = gaze.map(f32::to_radians);
    glam::Quat::from_rotation_y(yaw) * glam::Quat::from_rotation_x(-pitch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_name_known_units_and_ids_are_unique() {
        for m in PRESETS.iter().chain(VISEMES) {
            for (id, w) in m.units {
                assert!(unit(id).is_some(), "{} uses unknown unit {id}", m.id);
                assert!((0.0..=1.0).contains(w));
            }
        }
        let mut ids: Vec<&str> = UNITS.iter().map(|u| u.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), UNITS.len());
        assert!(UNITS.iter().all(|u| GROUPS.contains(&u.group)));
    }

    #[test]
    fn expression_set_clamps_and_serializes_sparse() {
        let mut e = Expression::from_mix(preset("smile").unwrap());
        assert!(e.get("mouth-corner-puller") > 0.5);
        e.set("mouth-open", 3.0);
        assert_eq!(e.get("mouth-open"), 1.0);
        e.set("mouth-open", 0.0);
        assert!(!e.units.contains_key("mouth-open"));
        e.set_gaze(90.0, -5.0);
        assert_eq!(e.gaze, [35.0, -5.0]);
        let json = serde_json::to_string(&Expression::default()).unwrap();
        assert_eq!(json, "{}");
        let back: Expression = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn parse_mixes_and_units() {
        let e = Expression::parse("smile, viseme.oh ,mouth-open:0.9").unwrap();
        assert_eq!(e.get("mouth-open"), 0.9);
        assert_eq!(e.get("mouth-corner-puller"), 0.8);
        assert!(e.get("mouth-pursing") > 0.5);
        assert!(Expression::parse("grin").is_err());
        assert!(Expression::parse("mouth-open:x").is_err());
        assert!(Expression::parse("nope:1").is_err());
        assert!(Expression::parse("").unwrap().is_neutral());
    }

    #[test]
    fn gaze_yaw_turns_forward_towards_left() {
        let d = gaze_rotation([30.0, 0.0]) * Vec3::Z;
        assert!(d.x > 0.4 && d.z > 0.8);
        let up = gaze_rotation([0.0, 20.0]) * Vec3::Z;
        assert!(up.y > 0.3);
    }
}
