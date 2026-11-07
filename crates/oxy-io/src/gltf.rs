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
    /// Optional linear RGBA vertex colors (`COLOR_0`).
    pub colors: Option<&'a [[f32; 4]]>,
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

/// Write one mesh as a `.glb`.
pub fn write_glb(mesh: &ExportMesh, w: impl Write) -> Result<()> {
    write_glb_scene(std::slice::from_ref(mesh), w)
}

/// Write several meshes as separate nodes of one `.glb` scene.
pub fn write_glb_scene(meshes: &[ExportMesh], mut w: impl Write) -> Result<()> {
    let mut bin: Vec<u8> = Vec::new();
    let (mut views, mut accessors, mut gl_meshes, mut nodes, mut materials) =
        (vec![], vec![], vec![], vec![], vec![]);
    let mut view = |bin: &mut Vec<u8>, bytes: Vec<u8>, target: u32| {
        let off = bin.len();
        let len = bytes.len();
        bin.extend(bytes);
        pad4(bin, 0);
        views.push(json!({ "buffer": 0, "byteOffset": off, "byteLength": len, "target": target }));
        views.len() - 1
    };
    let f32s = |v: &[Vec3]| -> Vec<u8> {
        v.iter()
            .flat_map(|p| p.to_array())
            .flat_map(f32::to_le_bytes)
            .collect()
    };

    for (i, mesh) in meshes.iter().enumerate() {
        assert_eq!(mesh.positions.len(), mesh.normals.len());
        let n = mesh.positions.len();
        let (lo, hi) = crate::makehuman::bounds(mesh.positions);
        let v = view(&mut bin, f32s(mesh.positions), ARRAY_BUFFER);
        accessors.push(json!({ "bufferView": v, "componentType": FLOAT, "count": n, "type": "VEC3", "min": lo.to_array(), "max": hi.to_array() }));
        let pos = accessors.len() - 1;
        let v = view(&mut bin, f32s(mesh.normals), ARRAY_BUFFER);
        accessors
            .push(json!({ "bufferView": v, "componentType": FLOAT, "count": n, "type": "VEC3" }));
        let nrm = accessors.len() - 1;
        let mut attributes = json!({ "POSITION": pos, "NORMAL": nrm });
        if let Some(c) = mesh.colors {
            assert_eq!(c.len(), n);
            let v = view(
                &mut bin,
                c.iter().flatten().flat_map(|x| x.to_le_bytes()).collect(),
                ARRAY_BUFFER,
            );
            accessors.push(
                json!({ "bufferView": v, "componentType": FLOAT, "count": n, "type": "VEC4" }),
            );
            attributes["COLOR_0"] = json!(accessors.len() - 1);
        }
        let v = view(
            &mut bin,
            mesh.indices.iter().flat_map(|i| i.to_le_bytes()).collect(),
            ELEMENT_ARRAY_BUFFER,
        );
        accessors.push(json!({ "bufferView": v, "componentType": UNSIGNED_INT, "count": mesh.indices.len(), "type": "SCALAR" }));
        let idx = accessors.len() - 1;
        materials.push(json!({
            "name": format!("{}-material", mesh.name),
            "pbrMetallicRoughness": { "baseColorFactor": mesh.base_color, "metallicFactor": 0.0, "roughnessFactor": 0.6 }
        }));
        gl_meshes.push(json!({ "name": mesh.name, "primitives": [{ "attributes": attributes, "indices": idx, "material": i }] }));
        nodes.push(json!({ "name": mesh.name, "mesh": i }));
    }
    let doc = json!({
        "asset": { "version": "2.0", "generator": "OxyCharacter" },
        "scene": 0,
        "scenes": [{ "nodes": (0..meshes.len()).collect::<Vec<_>>() }],
        "nodes": nodes,
        "meshes": gl_meshes,
        "materials": materials,
        "buffers": [{ "byteLength": bin.len() }],
        "bufferViews": views,
        "accessors": accessors,
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
                colors: None,
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
