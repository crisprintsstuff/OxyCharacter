//! OxyCharacter projects: what the user made, saved as a small, versioned,
//! human-readable JSON file (`.oxyc`), plus a generic snapshot undo history.
//!
//! A project stores **parameters, not meshes**: slider values (only those
//! that differ from the default, so files stay small and diffs readable), skin
//! color, preview pose and export options. Things that are not part of the
//! base asset — sculpted morphs and accessory meshes — are embedded, so a
//! project file is self-contained.
//!
//! The base asset is identified by a fingerprint of its topology; morph data
//! is only applied to a base with the same fingerprint.

pub mod history;

use std::collections::BTreeMap;
use std::path::Path;

use oxy_fit::{builtin_name, FitSettings, GarmentBind, ItemKind};
use oxy_io::export::ExportOptions;
use oxy_io::expression::Expression;
use oxy_io::oxybrush::{Accessory, CustomMorph};
use oxy_io::CharacterAsset;
use serde::{Deserialize, Serialize};

pub use history::History;

pub const FORMAT: &str = "oxycharacter-project";
pub const VERSION: u32 = 1;
pub const EXTENSION: &str = "oxyc";

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not an OxyCharacter project: {0}")]
    NotAProject(String),
    #[error("project format version {0} is newer than this OxyCharacter supports ({VERSION})")]
    TooNew(u32),
    #[error("damaged project: {0}")]
    Damaged(String),
}

pub type Result<T> = std::result::Result<T, ProjectError>;

/// Identifies the base mesh a project was made for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseRef {
    pub name: String,
    pub vertices: usize,
    /// FNV-1a 64 of the triangle list, hex.
    pub topology: String,
}

impl BaseRef {
    pub fn of(asset: &CharacterAsset) -> Self {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &i in &asset.mesh.base.indices {
            for b in i.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        }
        Self {
            name: asset.meta.name.clone(),
            vertices: asset.mesh.base.vertex_count(),
            topology: format!("{h:016x}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessoryEntry {
    #[serde(flatten)]
    pub mesh: Accessory,
    pub visible: bool,
    /// How this accessory follows the body. Missing in older projects: rigid.
    #[serde(default)]
    pub settings: FitSettings,
    /// Present when `settings.method` is not rigid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<GarmentBind>,
}

/// A mesh embedded in a project (a garment that is not a built-in).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddedMesh {
    pub positions: Vec<glam::Vec3>,
    pub triangles: Vec<[u32; 3]>,
    #[serde(default)]
    pub uvs: Vec<[f32; 2]>,
}

/// One worn garment or hair item.
///
/// Built-ins store only their id and settings; the mesh is regenerated from
/// the base body. Other items embed the mesh and the binding it was authored
/// with. `hidden_tris` is the body triangles this item covered at save time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WardrobeEntry {
    pub id: String,
    pub name: String,
    pub kind: ItemKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builtin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<EmbeddedMesh>,
    pub settings: FitSettings,
    /// Linear RGB.
    pub color: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<GarmentBind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_tris: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub format: String,
    pub version: u32,
    /// Writer, e.g. `oxycharacter 0.0.1-alpha`.
    pub app: String,
    pub base: BaseRef,
    /// Parameter values that differ from the defaults.
    pub params: BTreeMap<String, f32>,
    /// Linear RGB.
    pub skin: [f32; 3],
    /// Preview pose preset name.
    pub pose: String,
    pub export: ExportOptions,
    /// Skin and eye settings (added in 0.1 without a version bump: optional).
    #[serde(default)]
    pub material: oxy_io::material::MaterialParams,
    pub custom_morphs: Vec<CustomMorph>,
    pub accessories: Vec<AccessoryEntry>,
    /// Worn garments and hair (added without a version bump: optional).
    #[serde(default)]
    pub wardrobe: Vec<WardrobeEntry>,
    /// Facial expression and gaze (added without a version bump: optional).
    #[serde(default, skip_serializing_if = "Expression::is_neutral")]
    pub expression: Expression,
}

impl Project {
    pub fn new(base: BaseRef) -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION,
            app: format!("oxycharacter {}", env!("CARGO_PKG_VERSION")),
            base,
            params: BTreeMap::new(),
            skin: [0.62, 0.40, 0.29],
            pose: "Rest (A-pose)".into(),
            export: ExportOptions::default(),
            material: Default::default(),
            custom_morphs: Vec::new(),
            accessories: Vec::new(),
            wardrobe: Vec::new(),
            expression: Expression::default(),
        }
    }

    /// Keep only values that differ from the catalog defaults (custom morphs
    /// are kept whenever non-zero, as they are not in the base catalog).
    pub fn set_params(&mut self, asset: &CharacterAsset, params: &oxy_morph::Params) {
        self.params = params
            .iter()
            .filter(|(id, v)| {
                asset
                    .catalog
                    .param(id)
                    .is_none_or(|p| (**v - p.default).abs() > oxy_mesh::WEIGHT_EPSILON)
            })
            .map(|(k, v)| (k.clone(), *v))
            .collect();
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).expect("project serializes")
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let v: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| ProjectError::NotAProject(e.to_string()))?;
        migrate(v)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        oxy_io::oxb::write_atomic(path, &self.to_json()).map_err(|e| match e {
            oxy_io::IoError::Io(e) => ProjectError::Io(e),
            other => ProjectError::Damaged(other.to_string()),
        })
    }

    pub fn load(path: &Path) -> Result<Self> {
        Self::from_json(&std::fs::read(path)?)
    }

    /// Check the project against the base it is opened with. Returns warnings
    /// and drops anything that can't apply (so opening never fails on them).
    pub fn reconcile(&mut self, asset: &CharacterAsset) -> Vec<String> {
        let mut warnings = Vec::new();
        let here = BaseRef::of(asset);
        let custom_ids: Vec<String> = self
            .custom_morphs
            .iter()
            .map(|m| format!("{}{}", oxy_io::oxybrush::CUSTOM_PREFIX, m.id))
            .collect();
        if here.topology != self.base.topology || here.vertices != self.base.vertices {
            if !self.custom_morphs.is_empty() {
                warnings.push(format!(
                    "{} sculpted morph(s) were made for a different base mesh ({}) and were not loaded.",
                    self.custom_morphs.len(),
                    self.base.name
                ));
                self.custom_morphs.clear();
            }
        } else {
            let n = here.vertices;
            let before = self.custom_morphs.len();
            self.custom_morphs.retain(|m| {
                m.indices.len() == m.deltas.len() && m.indices.iter().all(|&i| (i as usize) < n)
            });
            if self.custom_morphs.len() != before {
                warnings.push(format!(
                    "{} damaged sculpted morph(s) were skipped.",
                    before - self.custom_morphs.len()
                ));
            }
        }
        let unknown: Vec<String> = self
            .params
            .keys()
            .filter(|id| asset.catalog.param(id).is_none() && !custom_ids.contains(id))
            .cloned()
            .collect();
        if !unknown.is_empty() {
            warnings.push(format!(
                "{} slider(s) don't exist in this base and were ignored: {}",
                unknown.len(),
                unknown.join(", ")
            ));
            for id in unknown {
                self.params.remove(&id);
            }
        }
        let missing: Vec<&str> = self
            .expression
            .units
            .keys()
            .map(String::as_str)
            .filter(|id| oxy_io::expression::available(asset).all(|u| u.id != *id))
            .collect();
        if !missing.is_empty() {
            // Kept in the project (saved back unchanged); they just move nothing here.
            warnings.push(format!(
                "{} expression unit(s) are not in this base and have no effect: {}",
                missing.len(),
                missing.join(", ")
            ));
        }
        let before = self.accessories.len();
        self.accessories.retain(|a| {
            let n = a.mesh.positions.len();
            a.mesh.triangles.iter().flatten().all(|&i| (i as usize) < n)
                && (a.mesh.color.is_empty() || a.mesh.color.len() == n)
        });
        if self.accessories.len() != before {
            warnings.push(format!(
                "{} damaged accessory mesh(es) were skipped.",
                before - self.accessories.len()
            ));
        }
        let before = self.wardrobe.len();
        self.wardrobe.retain(|w| match &w.builtin {
            Some(id) => builtin_name(id).is_some(),
            None => w.mesh.as_ref().is_some_and(|m| {
                let n = m.positions.len();
                !m.triangles.is_empty()
                    && m.triangles.iter().flatten().all(|&i| (i as usize) < n)
                    && (m.uvs.is_empty() || m.uvs.len() == n)
            }),
        });
        if self.wardrobe.len() != before {
            warnings.push(format!(
                "{} clothing item(s) could not be loaded and were skipped.",
                before - self.wardrobe.len()
            ));
        }
        self.base = here;
        warnings
    }
}

/// Bring older project versions up to the current one.
fn migrate(v: serde_json::Value) -> Result<Project> {
    if v.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
        return Err(ProjectError::NotAProject("missing format marker".into()));
    }
    let version = v.get("version").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    match version {
        // Future: `0 => upgrade_v0_to_v1(v)`, etc.
        VERSION => serde_json::from_value(v).map_err(|e| ProjectError::Damaged(e.to_string())),
        n if n > VERSION => Err(ProjectError::TooNew(n)),
        n => Err(ProjectError::Damaged(format!(
            "unknown project version {n}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use oxy_io::AssetMeta;
    use oxy_mesh::{Mesh, MorphableMesh};
    use oxy_morph::{Catalog, ParamDef};

    fn asset() -> CharacterAsset {
        let base = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![0, 1, 2]).unwrap();
        let p = |id: &str, default: f32| ParamDef {
            id: id.into(),
            label: id.into(),
            group: vec![],
            min: 0.0,
            max: 1.0,
            default,
            mirror: None,
            ends: None,
        };
        CharacterAsset {
            meta: AssetMeta {
                name: "tri".into(),
                ..Default::default()
            },
            mesh: MorphableMesh {
                base,
                targets: vec![],
                aux: vec![],
            },
            catalog: Catalog {
                params: vec![p("gender", 0.5), p("nose", 0.0)],
                ..Default::default()
            },
            rig: None,
            render: None,
        }
    }

    fn sample(a: &CharacterAsset) -> Project {
        let mut pr = Project::new(BaseRef::of(a));
        let params = [("gender".to_string(), 0.5), ("nose".to_string(), 0.25)].into();
        pr.set_params(a, &params);
        pr.custom_morphs.push(CustomMorph {
            id: "brow".into(),
            label: "Brow".into(),
            indices: vec![2],
            deltas: vec![Vec3::Z * 0.01],
        });
        pr.params.insert("custom/brow".into(), 1.0);
        pr.material.skin.freckles = 0.4;
        pr.material.eyes.iris = [0.1, 0.3, 0.5];
        pr.accessories.push(AccessoryEntry {
            mesh: Accessory {
                name: "Hat".into(),
                positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
                triangles: vec![[0, 1, 2]],
                color: vec![],
            },
            visible: true,
            settings: Default::default(),
            binding: None,
        });
        pr.wardrobe.push(WardrobeEntry {
            id: "tee".into(),
            name: "T-shirt".into(),
            kind: oxy_fit::ItemKind::Garment,
            builtin: Some("tee".into()),
            mesh: None,
            settings: oxy_fit::FitSettings::surface(2),
            color: [0.2, 0.3, 0.5],
            binding: None,
            hidden_tris: vec![0, 2, 5],
        });
        pr
    }

    #[test]
    fn stores_only_changed_params_and_round_trips() {
        let a = asset();
        let pr = sample(&a);
        assert_eq!(
            pr.params.keys().collect::<Vec<_>>(),
            vec!["custom/brow", "nose"]
        );
        let back = Project::from_json(&pr.to_json()).unwrap();
        assert_eq!(back, pr);
        let text = String::from_utf8(pr.to_json()).unwrap();
        assert!(
            text.contains("\"format\": \"oxycharacter-project\""),
            "human-readable"
        );
    }

    #[test]
    fn projects_without_material_still_open() {
        let a = asset();
        let mut v: serde_json::Value = serde_json::from_slice(&sample(&a).to_json()).unwrap();
        let obj = v.as_object_mut().unwrap();
        obj.remove("material");
        obj.remove("wardrobe");
        let p = Project::from_json(v.to_string().as_bytes()).unwrap();
        assert_eq!(p.material, Default::default());
        assert!(p.wardrobe.is_empty());
    }

    #[test]
    fn wardrobe_round_trips_and_unknown_items_are_dropped() {
        let a = asset();
        let pr = sample(&a);
        assert_eq!(pr.wardrobe[0].hidden_tris, vec![0, 2, 5]);
        let back = Project::from_json(&pr.to_json()).unwrap();
        assert_eq!(back.wardrobe, pr.wardrobe);
        let mut pr = sample(&a);
        pr.wardrobe.push(WardrobeEntry {
            id: "nope".into(),
            name: "Nope".into(),
            kind: oxy_fit::ItemKind::Garment,
            builtin: Some("not-a-garment".into()),
            mesh: None,
            settings: Default::default(),
            color: [0.0; 3],
            binding: None,
            hidden_tris: vec![],
        });
        let w = pr.reconcile(&a);
        assert!(w.iter().any(|m| m.contains("clothing")));
        assert_eq!(pr.wardrobe.len(), 1);
    }

    #[test]
    fn rejects_other_files_and_newer_versions() {
        assert!(matches!(
            Project::from_json(b"{}"),
            Err(ProjectError::NotAProject(_))
        ));
        assert!(matches!(
            Project::from_json(b"not json"),
            Err(ProjectError::NotAProject(_))
        ));
        let mut v: serde_json::Value = serde_json::from_slice(&sample(&asset()).to_json()).unwrap();
        v["version"] = 99.into();
        assert!(matches!(
            Project::from_json(v.to_string().as_bytes()),
            Err(ProjectError::TooNew(99))
        ));
    }

    #[test]
    fn reconcile_drops_what_cannot_apply() {
        let a = asset();
        let mut pr = sample(&a);
        pr.params.insert("gone".into(), 1.0);
        let w = pr.reconcile(&a);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(!pr.params.contains_key("gone") && pr.params.contains_key("custom/brow"));

        // Different topology: morphs can't apply.
        let mut pr = sample(&a);
        pr.base.topology = "0".into();
        let w = pr.reconcile(&a);
        assert!(pr.custom_morphs.is_empty() && w.iter().any(|w| w.contains("different base mesh")));

        // Damaged accessory indices are skipped.
        let mut pr = sample(&a);
        pr.accessories[0].mesh.triangles[0][2] = 9;
        assert_eq!(pr.reconcile(&a).len(), 1);
        assert!(pr.accessories.is_empty());
    }

    #[test]
    fn saves_atomically() {
        let dir = std::env::temp_dir().join(format!("oxy-project-{}", std::process::id()));
        let path = dir.join("hero.oxyc");
        let pr = sample(&asset());
        pr.save(&path).unwrap();
        pr.save(&path).unwrap();
        assert_eq!(Project::load(&path).unwrap(), pr);
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            1,
            "no temp files left"
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
