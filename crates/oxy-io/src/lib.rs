//! Import/export for OxyCharacter.
//!
//! * [`obj`]       — minimal Wavefront OBJ reader (positions, polygon faces, groups).
//! * [`target`]    — sparse text morph-target reader (`index dx dy dz` per line).
//! * [`makehuman`] — builds a [`CharacterAsset`] from the CC0 MakeHuman data set.
//! * [`bundle`]    — our own compact binary container for a [`CharacterAsset`].
//! * [`gltf`]      — glTF 2.0 binary (`.glb`) export.
//! * [`export`]    — the character (+ accessories) as a rigged glTF.
//! * [`material`]  — character skin/eye textures and eye meshes.
//! * [`oxb`]       — OxyBrush project files (`.oxb`), read and write.
//! * [`oxybrush`]  — the OxyBrush bridge: send a character, bring sculpts back.

pub use glam;

pub mod bundle;
pub mod export;
pub mod gltf;
pub mod makehuman;
pub mod material;
pub mod obj;
pub mod oxb;
pub mod oxybrush;
pub mod target;

use oxy_mesh::MorphableMesh;
use oxy_morph::Catalog;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error in {file} line {line}: {msg}")]
    Parse {
        file: String,
        line: usize,
        msg: String,
    },
    #[error("invalid data: {0}")]
    Invalid(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, IoError>;

/// Provenance of an asset, kept alongside it (see `assets/LICENSES.md`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssetMeta {
    pub name: String,
    pub source: String,
    pub license: String,
}

/// Everything the editor needs for one base character.
#[derive(Debug, Clone)]
pub struct CharacterAsset {
    pub meta: AssetMeta,
    pub mesh: MorphableMesh,
    pub catalog: Catalog,
    pub rig: Option<oxy_rig::Rig>,
    /// UV-seamed mesh for drawing/export (None for bundles without UVs).
    pub render: Option<oxy_mesh::RenderMesh>,
}

/// A fully evaluated, floor-standing character.
#[derive(Debug, Clone)]
pub struct Evaluated {
    pub body: Vec<glam::Vec3>,
    pub aux: Vec<glam::Vec3>,
    /// Joint positions (empty without a rig).
    pub joints: Vec<glam::Vec3>,
    pub ground_offset: f32,
}

impl CharacterAsset {
    /// Evaluate the body for `params`, shifted so its lowest point is at y = 0.
    /// Returns the positions and the vertical offset that was added.
    pub fn evaluate_grounded(&self, params: &oxy_morph::Params) -> (Vec<glam::Vec3>, f32) {
        let (dense, _) = oxy_morph::resolve(&self.mesh, &self.catalog.evaluate(params));
        let mut pos = self.mesh.evaluate(&dense);
        let offset = -pos.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        pos.iter_mut().for_each(|p| p.y += offset);
        (pos, offset)
    }

    /// Body, aux points and joints for `params`, standing on the floor.
    pub fn evaluate_full(&self, params: &oxy_morph::Params) -> Evaluated {
        let (dense, _) = oxy_morph::resolve(&self.mesh, &self.catalog.evaluate(params));
        let mut body = self.mesh.evaluate(&dense);
        let mut aux = self.mesh.evaluate_aux(&dense);
        let ground_offset = -body.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        body.iter_mut()
            .chain(aux.iter_mut())
            .for_each(|p| p.y += ground_offset);
        let joints = self
            .rig
            .as_ref()
            .map_or_else(Vec::new, |r| r.joint_positions(&body, &aux));
        Evaluated {
            body,
            aux,
            joints,
            ground_offset,
        }
    }

    /// Add (or replace) a sculpted morph as a 0..1 slider under Custom › Sculpted.
    /// Returns its param id. Joint locators are moved along with the body so
    /// the skeleton follows the sculpt.
    pub fn add_custom_morph(&mut self, m: &oxybrush::CustomMorph) -> String {
        let id = format!("{}{}", oxybrush::CUSTOM_PREFIX, m.id);
        self.remove_param(&id);
        let mut target =
            oxy_mesh::MorphTarget::new(id.clone(), m.indices.clone(), m.deltas.clone());
        if let Some(rig) = &self.rig {
            (target.aux_indices, target.aux_deltas) =
                rig.derive_aux_deltas(self.mesh.base.vertex_count(), &m.indices, &m.deltas);
        }
        self.mesh.targets.push(target);
        self.catalog.sliders.push(oxy_morph::Slider::Unipolar {
            id: id.clone(),
            target: id.clone(),
        });
        self.catalog.params.push(oxy_morph::ParamDef {
            id: id.clone(),
            label: m.label.clone(),
            group: vec!["Custom".into(), "Sculpted".into()],
            min: 0.0,
            max: 1.0,
            default: 0.0,
            mirror: None,
            ends: None,
        });
        id
    }

    /// Remove a parameter together with its slider and (same-named) target.
    pub fn remove_param(&mut self, id: &str) {
        self.mesh.targets.retain(|t| t.name != id);
        self.catalog.sliders.retain(|s| s.id() != id);
        self.catalog.params.retain(|p| p.id != id);
    }

    pub fn triangles(&self) -> Vec<[u32; 3]> {
        self.mesh.base.triangles().collect()
    }
}
