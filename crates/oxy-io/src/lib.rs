//! Import/export for OxyCharacter.
//!
//! * [`obj`]       — minimal Wavefront OBJ reader (positions, polygon faces, groups).
//! * [`target`]    — sparse text morph-target reader (`index dx dy dz` per line).
//! * [`makehuman`] — builds a [`CharacterAsset`] from the CC0 MakeHuman data set.
//! * [`bundle`]    — our own compact binary container for a [`CharacterAsset`].
//! * [`gltf`]      — glTF 2.0 binary (`.glb`) export.
//! * [`oxb`]       — OxyBrush project files (`.oxb`), read and write.
//! * [`oxybrush`]  — the OxyBrush bridge: send a character, bring sculpts back.

pub use glam;

pub mod bundle;
pub mod gltf;
pub mod makehuman;
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

    pub fn triangles(&self) -> Vec<[u32; 3]> {
        self.mesh.base.triangles().collect()
    }
}
