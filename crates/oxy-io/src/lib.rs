//! Import/export for OxyCharacter.
//!
//! * [`obj`]       — minimal Wavefront OBJ reader (positions, polygon faces, groups).
//! * [`target`]    — sparse text morph-target reader (`index dx dy dz` per line).
//! * [`makehuman`] — builds a [`CharacterAsset`] from the CC0 MakeHuman data set.
//! * [`bundle`]    — our own compact binary container for a [`CharacterAsset`].
//! * [`gltf`]      — glTF 2.0 binary (`.glb`) export.

pub mod bundle;
pub mod gltf;
pub mod makehuman;
pub mod obj;
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
