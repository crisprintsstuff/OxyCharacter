//! Editor state and logic, independent of the UI toolkit.

use std::collections::BTreeMap;

use bevy::prelude::Resource;
use oxy_io::CharacterAsset;
use oxy_mesh::WEIGHT_EPSILON;
use oxy_morph::{ParamDef, Params};

/// Preferred top-level category order in the morph tree.
const TOP_ORDER: &[&str] = &["Body", "Head", "Limbs", "Custom", "Asymmetry"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub top: String,
    pub subs: Vec<String>,
}

#[derive(Resource)]
pub struct Editor {
    pub asset: CharacterAsset,
    pub params: Params,
    pub categories: Vec<Category>,
    /// Selected `(top, sub)` in the morph tree.
    pub selected: (String, String),
    pub search: String,
    pub symmetry: bool,
    pub modified_only: bool,
    /// Parameter under the cursor (or being dragged), for region highlighting.
    pub hovered: Option<String>,
    /// Highlight shown when nothing is hovered (set from the command line).
    pub pinned_hover: Option<String>,
    pub skin: [f32; 3],
    pub status: String,
    /// Set when the mesh must be re-evaluated.
    pub dirty: bool,
    /// Set when only the highlight colors changed.
    pub highlight_dirty: bool,
    pub active_targets: usize,
    pub rng: u64,
    pub bridge: crate::bridge::Bridge,
    pub accessories: Vec<crate::bridge::AccessoryItem>,
    /// Set when accessory meshes must be re-spawned.
    pub accessories_dirty: bool,
    /// Built-in garments authored on the default body. Filled on first use.
    pub library: Option<Vec<oxy_fit::AuthoredGarment>>,
    /// Garments and hair currently worn.
    pub worn: Vec<crate::clothing::WornGarment>,
    /// Set when garment meshes must be re-spawned.
    pub clothing_dirty: bool,
    /// Body triangles hidden under clothing (base-mesh triangle indices).
    pub body_hidden: Vec<u32>,
    /// Index into `oxy_rig::pose::PRESETS` (0 = rest pose).
    pub pose: usize,
    pub show_skeleton: bool,
    /// Posed bone segments (head, tail) for the overlay.
    pub skeleton_lines: Vec<([f32; 3], [f32; 3])>,
    /// Skinning matrices of the current pose (empty in the rest pose).
    pub skin_mats: Vec<oxy_io::glam::Mat4>,
    /// Last evaluated body in the rest pose (for accessory weight transfer).
    pub rest_body: Vec<oxy_io::glam::Vec3>,
    pub export: oxy_io::export::ExportOptions,
    /// Skin and eye settings (part of the document).
    pub material: oxy_io::material::MaterialParams,
    /// Posed eyeball positions/normals from the last body update.
    pub eyes: Vec<(Vec<oxy_io::glam::Vec3>, Vec<oxy_io::glam::Vec3>)>,
    pub eyes_dirty: bool,
    /// While a plugin runs: hold back undo snapshots and use this label for
    /// the single step its whole run becomes.
    pub batch_label: Option<String>,
    /// Document session (undo, file, autosave); set by `init_session`.
    pub session: Option<crate::document::Session>,
}

/// Category tree for a catalog, in the preferred top-level order.
pub fn categories_of(catalog: &oxy_morph::Catalog) -> Vec<Category> {
    let mut tops: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in &catalog.params {
        let (top, sub) = split_group(p);
        let subs = tops.entry(top).or_default();
        if !subs.contains(&sub) {
            subs.push(sub);
        }
    }
    let rank = |t: &str| {
        TOP_ORDER
            .iter()
            .position(|o| *o == t)
            .unwrap_or(TOP_ORDER.len())
    };
    let mut categories: Vec<Category> = tops
        .into_iter()
        .map(|(top, subs)| Category { top, subs })
        .collect();
    categories.sort_by_key(|c| rank(&c.top));
    categories
}

impl Editor {
    pub fn new(asset: CharacterAsset, user_dir: &std::path::Path) -> Self {
        let params = asset.catalog.defaults();
        let categories = categories_of(&asset.catalog);
        let selected = categories
            .first()
            .map(|c| (c.top.clone(), c.subs[0].clone()))
            .unwrap_or_default();
        Self {
            asset,
            params,
            categories,
            selected,
            search: String::new(),
            symmetry: true,
            modified_only: false,
            hovered: None,
            pinned_hover: None,
            skin: SKIN_TONES[2].1,
            status: "Ready".into(),
            dirty: true,
            highlight_dirty: true,
            active_targets: 0,
            rng: 0x9E37_79B9_7F4A_7C15,
            bridge: crate::bridge::Bridge::new(user_dir),
            accessories: Vec::new(),
            accessories_dirty: false,
            library: None,
            worn: Vec::new(),
            clothing_dirty: false,
            body_hidden: Vec::new(),
            pose: 0,
            show_skeleton: false,
            skeleton_lines: Vec::new(),
            skin_mats: Vec::new(),
            rest_body: Vec::new(),
            export: oxy_io::export::ExportOptions::default(),
            material: oxy_io::material::MaterialParams::default(),
            eyes: Vec::new(),
            eyes_dirty: false,
            batch_label: None,
            session: None,
        }
    }

    pub fn set_pose(&mut self, index: usize) {
        if index < oxy_rig::pose::PRESETS.len() && index != self.pose {
            self.pose = index;
            self.dirty = true;
            self.status = format!("Pose: {}", oxy_rig::pose::PRESETS[index].name);
        }
    }

    /// Pose the evaluated body; updates the skeleton overlay and skinning matrices.
    pub fn apply_pose(
        &mut self,
        ev: &oxy_io::Evaluated,
        normals: &[oxy_io::glam::Vec3],
    ) -> (Vec<oxy_io::glam::Vec3>, Vec<oxy_io::glam::Vec3>) {
        use oxy_io::glam::{Mat4, Vec3};
        self.skeleton_lines.clear();
        self.skin_mats.clear();
        let Some(rig) = &self.asset.rig else {
            return (ev.body.clone(), normals.to_vec());
        };
        let bind = rig.bind_matrices(&ev.joints);
        let local = oxy_rig::pose::solve(rig, &bind, &oxy_rig::pose::PRESETS[self.pose]);
        let posed = rig.pose_matrices(&bind, &local);
        for (i, b) in rig.bones.iter().enumerate() {
            let len = ev.joints[b.head].distance(ev.joints[b.tail]);
            let head = posed[i].w_axis.truncate();
            let tail = posed[i].transform_point3(Vec3::Y * len);
            self.skeleton_lines.push((head.to_array(), tail.to_array()));
        }
        if self.pose == 0 {
            return (ev.body.clone(), normals.to_vec());
        }
        self.skin_mats = posed
            .iter()
            .zip(&bind)
            .map(|(p, b)| *p * b.inverse())
            .collect::<Vec<Mat4>>();
        oxy_rig::skin_with(&rig.skin, &ev.body, normals, &self.skin_mats)
    }

    /// Place (and pose) the eyeballs for the evaluated body.
    pub fn update_eyes(&mut self, ev: &oxy_io::Evaluated) {
        let eyes = oxy_io::material::eyes(&self.asset, ev, &self.material.eyes);
        self.eyes = eyes
            .into_iter()
            .map(|e| match (e.bone, self.skin_mats.is_empty()) {
                (Some(b), false) => {
                    let m = self.skin_mats[b];
                    (
                        e.mesh
                            .positions
                            .iter()
                            .map(|p| m.transform_point3(*p))
                            .collect(),
                        e.mesh
                            .normals
                            .iter()
                            .map(|n| m.transform_vector3(*n).normalize())
                            .collect(),
                    )
                }
                _ => (e.mesh.positions, e.mesh.normals),
            })
            .collect();
        self.eyes_dirty = true;
    }

    pub fn value(&self, id: &str) -> f32 {
        self.params.get(id).copied().unwrap_or(0.0)
    }

    pub fn is_modified(&self, p: &ParamDef) -> bool {
        (self.value(&p.id) - p.default).abs() > WEIGHT_EPSILON
    }

    /// Set a parameter (and its mirror when symmetry is on).
    pub fn set(&mut self, id: &str, v: f32) {
        let Some(def) = self.asset.catalog.param(id) else {
            return;
        };
        let v = v.clamp(def.min, def.max);
        let mirror = def.mirror.clone();
        self.params.insert(id.to_owned(), v);
        if self.symmetry {
            if let Some(m) = mirror {
                if self.asset.catalog.param(&m).is_some() {
                    self.params.insert(m, v);
                }
            }
        }
        self.dirty = true;
    }

    pub fn reset_all(&mut self) {
        self.params = self.asset.catalog.defaults();
        self.dirty = true;
        self.status = "Reset to neutral".into();
    }

    pub fn reset_group(&mut self, top: &str, sub: &str) {
        let ids: Vec<(String, f32)> = self
            .params_in(top, sub)
            .map(|p| (p.id.clone(), p.default))
            .collect();
        for (id, d) in ids {
            self.params.insert(id, d);
        }
        self.dirty = true;
    }

    pub fn params_in<'a>(
        &'a self,
        top: &'a str,
        sub: &'a str,
    ) -> impl Iterator<Item = &'a ParamDef> + 'a {
        self.asset.catalog.params.iter().filter(move |p| {
            let (t, s) = split_group(p);
            t == top && s == sub
        })
    }

    pub fn modified_count(&self, top: &str, sub: Option<&str>) -> usize {
        self.asset
            .catalog
            .params
            .iter()
            .filter(|p| {
                let (t, s) = split_group(p);
                t == top && sub.is_none_or(|x| x == s) && self.is_modified(p)
            })
            .count()
    }

    /// Parameters matching the search box (label, id or category).
    pub fn search_results(&self) -> Vec<&ParamDef> {
        let q = self.search.to_lowercase();
        self.asset
            .catalog
            .params
            .iter()
            .filter(|p| {
                p.label.to_lowercase().contains(&q)
                    || p.id.to_lowercase().contains(&q)
                    || p.group.join(" ").to_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn apply_preset(&mut self, preset: &Preset) {
        self.reset_all();
        for (id, v) in preset.values {
            self.set(id, *v);
        }
        self.status = format!("Applied preset “{}”", preset.name);
    }

    fn next_f32(&mut self) -> f32 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        ((self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32) / (1u64 << 24) as f32
    }

    /// Random but plausible character: macro values plus small face variation.
    pub fn randomize(&mut self) {
        self.reset_all();
        let g = if self.next_f32() < 0.5 { 0.05 } else { 0.95 };
        let age = 0.42 + self.next_f32() * 0.4;
        let muscle = 0.3 + self.next_f32() * 0.5;
        let weight = 0.3 + self.next_f32() * 0.45;
        let height = self.next_f32() * 1.2 - 0.6;
        for (id, v) in [
            ("gender", g),
            ("age", age),
            ("muscle", muscle),
            ("weight", weight),
            ("height", height),
        ] {
            self.set(id, v);
        }
        let face: Vec<(String, f32, f32)> = self
            .asset
            .catalog
            .params
            .iter()
            .filter(|p| p.group.first().is_some_and(|t| t == "Head") && p.min < 0.0)
            .map(|p| (p.id.clone(), p.min, p.max))
            .collect();
        for (id, lo, hi) in face {
            if self.next_f32() < 0.35 {
                let t = self.next_f32() * 2.0 - 1.0;
                self.set(&id, (t * 0.45).clamp(lo, hi));
            }
        }
        self.status = "Randomized character".into();
    }

    /// Display value for a parameter (e.g. age in years).
    pub fn display_value(&self, p: &ParamDef) -> String {
        let v = self.value(&p.id);
        match p.id.as_str() {
            "age" => format!("{:.0} yrs", age_years(v)),
            _ if p.min < 0.0 => format!("{:+.0}", v * 100.0),
            _ => format!("{:.0}", v * 100.0),
        }
    }
}

pub fn split_group(p: &ParamDef) -> (String, String) {
    let top = p.group.first().cloned().unwrap_or_else(|| "Other".into());
    let sub = p.group.get(1).cloned().unwrap_or_else(|| "General".into());
    (top, sub)
}

/// Map the 0..1 age parameter to years (piecewise linear between grid stops).
pub fn age_years(v: f32) -> f32 {
    const STOPS: [(f32, f32); 4] = [(0.0, 1.0), (0.1875, 11.0), (0.5, 25.0), (1.0, 90.0)];
    for w in STOPS.windows(2) {
        let ((a, ya), (b, yb)) = (w[0], w[1]);
        if v <= b {
            return ya + (v.max(a) - a) / (b - a) * (yb - ya);
        }
    }
    STOPS[3].1
}

pub struct Preset {
    pub name: &'static str,
    pub kind: &'static str,
    pub values: &'static [(&'static str, f32)],
}

pub const PRESETS: &[Preset] = &[
    Preset {
        name: "Neutral",
        kind: "Adult",
        values: &[],
    },
    Preset {
        name: "Woman",
        kind: "Adult",
        values: &[("gender", 0.0), ("height", -0.2)],
    },
    Preset {
        name: "Man",
        kind: "Adult",
        values: &[("gender", 1.0), ("height", 0.2)],
    },
    Preset {
        name: "Athlete F",
        kind: "Adult",
        values: &[
            ("gender", 0.0),
            ("muscle", 0.9),
            ("weight", 0.4),
            ("height", 0.3),
        ],
    },
    Preset {
        name: "Athlete M",
        kind: "Adult",
        values: &[
            ("gender", 1.0),
            ("muscle", 1.0),
            ("weight", 0.55),
            ("height", 0.5),
        ],
    },
    Preset {
        name: "Heavy",
        kind: "Adult",
        values: &[("gender", 0.8), ("weight", 1.0), ("muscle", 0.4)],
    },
    Preset {
        name: "Slender",
        kind: "Adult",
        values: &[
            ("gender", 0.2),
            ("weight", 0.1),
            ("muscle", 0.3),
            ("height", 0.4),
        ],
    },
    Preset {
        name: "Teen",
        kind: "Young",
        values: &[("gender", 0.5), ("age", 0.32), ("height", -0.2)],
    },
    Preset {
        name: "Child",
        kind: "Young",
        values: &[("gender", 0.4), ("age", 0.18)],
    },
    Preset {
        name: "Elder F",
        kind: "Elder",
        values: &[
            ("gender", 0.0),
            ("age", 0.95),
            ("weight", 0.6),
            ("height", -0.3),
        ],
    },
    Preset {
        name: "Elder M",
        kind: "Elder",
        values: &[("gender", 1.0), ("age", 0.95), ("muscle", 0.3)],
    },
];

/// Linear RGB skin tone swatches.
pub const SKIN_TONES: &[(&str, [f32; 3])] = &[
    ("Porcelain", [0.86, 0.66, 0.56]),
    ("Light", [0.78, 0.55, 0.43]),
    ("Medium", [0.62, 0.40, 0.29]),
    ("Tan", [0.48, 0.29, 0.19]),
    ("Brown", [0.30, 0.17, 0.10]),
    ("Deep", [0.16, 0.09, 0.06]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_mapping_hits_stops() {
        assert_eq!(age_years(0.0), 1.0);
        assert_eq!(age_years(0.5), 25.0);
        assert_eq!(age_years(1.0), 90.0);
        assert!((age_years(0.75) - 57.5).abs() < 1e-4);
    }
}
