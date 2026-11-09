//! Wardrobe packages: a folder with `manifest.json` and mesh files.
//!
//! ```text
//! my-clothes/
//!   manifest.json
//!   shirt.obj
//! ```
//!
//! The manifest is our own JSON format (`oxycharacter-wardrobe`, version 1).
//! Mesh files are Wavefront OBJ; the caller parses them and builds a
//! [`crate::GarmentMesh`].

use serde::{Deserialize, Serialize};

use crate::{FitSettings, ItemKind};

pub const FORMAT: &str = "oxycharacter-wardrobe";
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub items: Vec<ManifestItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestItem {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: ItemKind,
    /// Path of the mesh relative to the manifest's folder.
    pub file: String,
    #[serde(default = "surface_settings")]
    pub settings: FitSettings,
}

fn surface_settings() -> FitSettings {
    FitSettings::surface(1)
}

impl Manifest {
    pub fn new(items: Vec<ManifestItem>) -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION,
            items,
        }
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let m: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if m.format != FORMAT {
            return Err(format!("not a wardrobe package (format {:?})", m.format));
        }
        if m.version > VERSION {
            return Err(format!(
                "wardrobe package version {} is newer than this OxyCharacter supports ({VERSION})",
                m.version
            ));
        }
        if m.version == 0 {
            return Err("wardrobe package is missing a version".into());
        }
        for item in &m.items {
            if item.id.is_empty() || item.file.is_empty() {
                return Err(format!(
                    "wardrobe item {:?} needs an id and a file",
                    item.name
                ));
            }
        }
        Ok(m)
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).expect("manifest serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FitMethod;

    #[test]
    fn manifest_round_trips_and_rejects_other_files() {
        let m = Manifest::new(vec![ManifestItem {
            id: "tee".into(),
            name: "T-shirt".into(),
            kind: ItemKind::Garment,
            file: "tee.obj".into(),
            settings: FitSettings {
                method: FitMethod::Surface,
                tightness: 0.4,
                ..FitSettings::surface(2)
            },
        }]);
        let back = Manifest::from_json(&m.to_json()).unwrap();
        assert_eq!(back, m);
        assert!(Manifest::from_json(b"{}").is_err());
        let mut v: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        v["version"] = 9.into();
        assert!(Manifest::from_json(v.to_string().as_bytes()).is_err());
    }
}
