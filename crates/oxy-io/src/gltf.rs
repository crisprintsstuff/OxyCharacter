//! glTF 2.0 binary (`.glb`) export of an evaluated (static) mesh.
//!
//! Skinning, skeletons and morph targets are added in milestone M2.

use std::io::Write;

use glam::Vec3;
use serde_json::json;

use crate::Result;

pub struct ExportMesh<'a> {
    pub name: &'a str,
    pub positions: &'a [Vec3],
    pub normals: &'a [Vec3],
    pub indices: &'a [u32],
    /// Linear RGBA base color.
    pub base_color: [f32; 4],
}

const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;
const FLOAT: u32 = 5126;
const UNSIGNED_INT: u32 = 5125;

fn pad4(v: &mut Vec<u8>, byte: u8) {
    while !v.len().is_multiple_of(4) {
        v.push(byte);
    }
}

pub fn write_glb(mesh: &ExportMesh, mut w: impl Write) -> Result<()> {
    assert_eq!(mesh.positions.len(), mesh.normals.len());
    let mut bin = Vec::new();
    let f32s = |v: &[Vec3]| -> Vec<u8> {
        v.iter()
            .flat_map(|p| p.to_array())
            .flat_map(f32::to_le_bytes)
            .collect()
    };

    let pos_off = bin.len();
    bin.extend(f32s(mesh.positions));
    let nrm_off = bin.len();
    bin.extend(f32s(mesh.normals));
    let idx_off = bin.len();
    bin.extend(mesh.indices.iter().flat_map(|i| i.to_le_bytes()));
    let n = mesh.positions.len();

    let (lo, hi) = crate::makehuman::bounds(mesh.positions);
    let doc = json!({
        "asset": { "version": "2.0", "generator": "OxyCharacter" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "name": mesh.name, "mesh": 0 }],
        "meshes": [{
            "name": mesh.name,
            "primitives": [{
                "attributes": { "POSITION": 0, "NORMAL": 1 },
                "indices": 2,
                "material": 0
            }]
        }],
        "materials": [{
            "name": "skin",
            "pbrMetallicRoughness": { "baseColorFactor": mesh.base_color, "metallicFactor": 0.0, "roughnessFactor": 0.6 }
        }],
        "buffers": [{ "byteLength": bin.len() }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": pos_off, "byteLength": n * 12, "target": ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": nrm_off, "byteLength": n * 12, "target": ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": idx_off, "byteLength": mesh.indices.len() * 4, "target": ELEMENT_ARRAY_BUFFER }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": FLOAT, "count": n, "type": "VEC3", "min": lo.to_array(), "max": hi.to_array() },
            { "bufferView": 1, "componentType": FLOAT, "count": n, "type": "VEC3" },
            { "bufferView": 2, "componentType": UNSIGNED_INT, "count": mesh.indices.len(), "type": "SCALAR" }
        ]
    });
    let mut json = serde_json::to_vec(&doc)?;
    pad4(&mut json, b' ');
    pad4(&mut bin, 0);

    let total = 12 + 8 + json.len() + 8 + bin.len();
    w.write_all(b"glTF")?;
    w.write_all(&2u32.to_le_bytes())?;
    w.write_all(&(total as u32).to_le_bytes())?;
    w.write_all(&(json.len() as u32).to_le_bytes())?;
    w.write_all(b"JSON")?;
    w.write_all(&json)?;
    w.write_all(&(bin.len() as u32).to_le_bytes())?;
    w.write_all(b"BIN\0")?;
    w.write_all(&bin)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glb_layout_is_valid() {
        let p = [Vec3::ZERO, Vec3::X, Vec3::Y];
        let mut out = Vec::new();
        write_glb(
            &ExportMesh {
                name: "t",
                positions: &p,
                normals: &[Vec3::Z; 3],
                indices: &[0, 1, 2],
                base_color: [1.0; 4],
            },
            &mut out,
        )
        .unwrap();
        assert_eq!(&out[0..4], b"glTF");
        assert_eq!(
            u32::from_le_bytes(out[8..12].try_into().unwrap()) as usize,
            out.len()
        );
        let jlen = u32::from_le_bytes(out[12..16].try_into().unwrap()) as usize;
        assert_eq!(jlen % 4, 0);
        let doc: serde_json::Value = serde_json::from_slice(&out[20..20 + jlen]).unwrap();
        assert_eq!(doc["accessors"][0]["max"][0], 1.0);
        assert_eq!(&out[20 + jlen + 4..20 + jlen + 8], b"BIN\0");
    }
}
