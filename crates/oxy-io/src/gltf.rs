//! glTF 2.0 binary (`.glb`) export: meshes, optional skeleton + skin, and
//! morph targets (blendshapes).

use std::io::Write;

use glam::{Mat4, Vec3};
use oxy_rig::Influence;
use serde_json::{json, Value};

use crate::Result;

/// A morph target with dense per-vertex position deltas.
pub struct ExportMorph<'a> {
    pub name: &'a str,
    pub deltas: Vec<Vec3>,
    /// Default weight in the exported file.
    pub weight: f32,
}

#[derive(Default)]
pub struct ExportMesh<'a> {
    pub name: &'a str,
    pub positions: &'a [Vec3],
    pub normals: &'a [Vec3],
    pub indices: &'a [u32],
    /// Linear RGBA base color.
    pub base_color: [f32; 4],
    /// Optional linear RGBA vertex colors (`COLOR_0`).
    pub colors: Option<&'a [[f32; 4]]>,
    /// Per-vertex bone influences; requires a skeleton in the scene.
    pub skin: Option<&'a [Influence]>,
    pub morphs: Vec<ExportMorph<'a>>,
    /// Texture coordinates (`TEXCOORD_0`, glTF convention); required for textures.
    pub uvs: Option<&'a [[f32; 2]]>,
    /// Textures by index into the scene's images.
    pub textures: MeshTextures,
}

/// Texture slots of a mesh's material (indices into the image list).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MeshTextures {
    pub base_color: Option<usize>,
    pub normal: Option<usize>,
    /// G = roughness, B = metallic (glTF layout).
    pub metallic_roughness: Option<usize>,
    /// Roughness factor when there is no metallic-roughness texture.
    pub roughness: Option<f32>,
}

/// An embedded PNG image.
pub struct ExportImage<'a> {
    pub name: &'a str,
    pub png: &'a [u8],
}

/// Joint hierarchy for the scene's single skin.
pub struct ExportSkeleton<'a> {
    pub name: &'a str,
    pub joint_names: Vec<&'a str>,
    /// Parent joint, parents first.
    pub parents: Vec<Option<usize>>,
    /// Bind transform of each joint relative to its parent.
    pub local: Vec<Mat4>,
    /// Inverse of each joint's world bind transform.
    pub inverse_bind: Vec<Mat4>,
}

const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;
const FLOAT: u32 = 5126;
const UNSIGNED_SHORT: u32 = 5123;
const UNSIGNED_INT: u32 = 5125;

fn pad4(v: &mut Vec<u8>, byte: u8) {
    while !v.len().is_multiple_of(4) {
        v.push(byte);
    }
}

fn bounds(p: &[Vec3]) -> (Vec3, Vec3) {
    crate::makehuman::bounds(p)
}

struct Builder {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Builder {
    fn view(&mut self, bytes: Vec<u8>, target: Option<u32>) -> usize {
        let off = self.bin.len();
        let len = bytes.len();
        self.bin.extend(bytes);
        pad4(&mut self.bin, 0);
        let mut v = json!({ "buffer": 0, "byteOffset": off, "byteLength": len });
        if let Some(t) = target {
            v["target"] = json!(t);
        }
        self.views.push(v);
        self.views.len() - 1
    }
    fn accessor(
        &mut self,
        view: usize,
        component: u32,
        count: usize,
        ty: &str,
        minmax: Option<(Vec3, Vec3)>,
    ) -> usize {
        let mut a =
            json!({ "bufferView": view, "componentType": component, "count": count, "type": ty });
        if let Some((lo, hi)) = minmax {
            a["min"] = json!(lo.to_array());
            a["max"] = json!(hi.to_array());
        }
        self.accessors.push(a);
        self.accessors.len() - 1
    }
    fn vec3s(&mut self, v: &[Vec3], minmax: bool) -> usize {
        let bytes = v
            .iter()
            .flat_map(|p| p.to_array())
            .flat_map(f32::to_le_bytes)
            .collect();
        let view = self.view(bytes, Some(ARRAY_BUFFER));
        self.accessor(view, FLOAT, v.len(), "VEC3", minmax.then(|| bounds(v)))
    }
}

/// Write one mesh as a `.glb`.
pub fn write_glb(mesh: &ExportMesh, w: impl Write) -> Result<()> {
    write_glb_scene(std::slice::from_ref(mesh), None, &[], w)
}

/// Write several meshes (separate nodes) and an optional skeleton as one `.glb`.
pub fn write_glb_scene(
    meshes: &[ExportMesh],
    skeleton: Option<&ExportSkeleton>,
    images: &[ExportImage],
    mut w: impl Write,
) -> Result<()> {
    let mut b = Builder {
        bin: Vec::new(),
        views: Vec::new(),
        accessors: Vec::new(),
    };
    let (mut gl_meshes, mut nodes, mut materials) = (vec![], vec![], vec![]);
    let joint_base = meshes.len();

    for (i, mesh) in meshes.iter().enumerate() {
        let n = mesh.positions.len();
        assert_eq!(mesh.normals.len(), n);
        let pos = b.vec3s(mesh.positions, true);
        let nrm = b.vec3s(mesh.normals, false);
        let mut attributes = json!({ "POSITION": pos, "NORMAL": nrm });
        if let (Some(uv), Some(_)) = (mesh.uvs, mesh.textures.normal) {
            // Normal-mapped: provide tangents (validators want them; consistent with the bake).
            let t = oxy_mesh::compute_tangents(mesh.positions, mesh.normals, uv, mesh.indices);
            let v = b.view(
                t.iter().flatten().flat_map(|x| x.to_le_bytes()).collect(),
                Some(ARRAY_BUFFER),
            );
            attributes["TANGENT"] = json!(b.accessor(v, FLOAT, n, "VEC4", None));
        }
        if let Some(uv) = mesh.uvs {
            assert_eq!(uv.len(), n);
            let v = b.view(
                uv.iter().flatten().flat_map(|x| x.to_le_bytes()).collect(),
                Some(ARRAY_BUFFER),
            );
            attributes["TEXCOORD_0"] = json!(b.accessor(v, FLOAT, n, "VEC2", None));
        }
        if let Some(c) = mesh.colors {
            assert_eq!(c.len(), n);
            let v = b.view(
                c.iter().flatten().flat_map(|x| x.to_le_bytes()).collect(),
                Some(ARRAY_BUFFER),
            );
            attributes["COLOR_0"] = json!(b.accessor(v, FLOAT, n, "VEC4", None));
        }
        let skinned = mesh.skin.is_some() && skeleton.is_some();
        if let (Some(skin), Some(_)) = (mesh.skin, skeleton) {
            assert_eq!(skin.len(), n);
            // Unweighted vertices follow the first joint (glTF requires weights to sum to 1).
            let fixed: Vec<Influence> = skin
                .iter()
                .map(|s| {
                    if s.weights.iter().sum::<f32>() > 0.0 {
                        *s
                    } else {
                        Influence {
                            bones: [0; 4],
                            weights: [1.0, 0.0, 0.0, 0.0],
                        }
                    }
                })
                .collect();
            let jb = fixed
                .iter()
                .flat_map(|s| s.bones)
                .flat_map(u16::to_le_bytes)
                .collect();
            let jv = b.view(jb, Some(ARRAY_BUFFER));
            attributes["JOINTS_0"] = json!(b.accessor(jv, UNSIGNED_SHORT, n, "VEC4", None));
            let wb = fixed
                .iter()
                .flat_map(|s| s.weights)
                .flat_map(f32::to_le_bytes)
                .collect();
            let wv = b.view(wb, Some(ARRAY_BUFFER));
            attributes["WEIGHTS_0"] = json!(b.accessor(wv, FLOAT, n, "VEC4", None));
        }
        let iv = b.view(
            mesh.indices.iter().flat_map(|i| i.to_le_bytes()).collect(),
            Some(ELEMENT_ARRAY_BUFFER),
        );
        let idx = b.accessor(iv, UNSIGNED_INT, mesh.indices.len(), "SCALAR", None);
        let mut primitive = json!({ "attributes": attributes, "indices": idx, "material": i });
        let mut gl_mesh = json!({ "name": mesh.name });
        if !mesh.morphs.is_empty() {
            let targets: Vec<Value> = mesh
                .morphs
                .iter()
                .map(|m| {
                    assert_eq!(m.deltas.len(), n);
                    json!({ "POSITION": b.vec3s(&m.deltas, true) })
                })
                .collect();
            primitive["targets"] = json!(targets);
            gl_mesh["weights"] = json!(mesh.morphs.iter().map(|m| m.weight).collect::<Vec<_>>());
            gl_mesh["extras"] =
                json!({ "targetNames": mesh.morphs.iter().map(|m| m.name).collect::<Vec<_>>() });
        }
        gl_mesh["primitives"] = json!([primitive]);
        gl_meshes.push(gl_mesh);
        let t = mesh.textures;
        let textured = mesh.uvs.is_some();
        let tex = |i: Option<usize>| {
            i.filter(|&i| textured && i < images.len())
                .map(|i| json!({ "index": i }))
        };
        let mut pbr = json!({ "baseColorFactor": mesh.base_color, "metallicFactor": 0.0, "roughnessFactor": t.roughness.unwrap_or(0.6) });
        if let Some(bc) = tex(t.base_color) {
            pbr["baseColorTexture"] = bc;
        }
        if let Some(mr) = tex(t.metallic_roughness) {
            pbr["metallicRoughnessTexture"] = mr;
            pbr["roughnessFactor"] = json!(1.0);
        }
        let mut material =
            json!({ "name": format!("{}-material", mesh.name), "pbrMetallicRoughness": pbr });
        if let Some(nt) = tex(t.normal) {
            material["normalTexture"] = nt;
        }
        materials.push(material);
        let mut node = json!({ "name": mesh.name, "mesh": i });
        if skinned {
            node["skin"] = json!(0);
        }
        nodes.push(node);
    }

    let mut scene_nodes: Vec<usize> = (0..meshes.len()).collect();
    let mut skins = Vec::new();
    if let Some(s) = skeleton {
        let count = s.joint_names.len();
        assert!(
            s.parents.len() == count && s.local.len() == count && s.inverse_bind.len() == count
        );
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); count];
        for (j, p) in s.parents.iter().enumerate() {
            match p {
                Some(p) => children[*p].push(joint_base + j),
                None => scene_nodes.push(joint_base + j),
            }
        }
        for (j, kids) in children.iter().enumerate() {
            let (scale, rot, trans) = s.local[j].to_scale_rotation_translation();
            let mut node = json!({ "name": s.joint_names[j], "translation": trans.to_array(), "rotation": rot.normalize().to_array() });
            if (scale - Vec3::ONE).abs().max_element() > 1e-5 {
                node["scale"] = json!(scale.to_array());
            }
            if !kids.is_empty() {
                node["children"] = json!(kids);
            }
            nodes.push(node);
        }
        let ibm = s
            .inverse_bind
            .iter()
            .flat_map(|m| m.to_cols_array())
            .flat_map(f32::to_le_bytes)
            .collect();
        let v = b.view(ibm, None);
        let acc = b.accessor(v, FLOAT, count, "MAT4", None);
        let root = s.parents.iter().position(|p| p.is_none()).unwrap_or(0);
        skins.push(json!({
            "name": s.name,
            "inverseBindMatrices": acc,
            "joints": (0..count).map(|j| joint_base + j).collect::<Vec<_>>(),
            "skeleton": joint_base + root,
        }));
    }

    // Images go into the binary buffer before the document is assembled.
    let gl_images: Vec<Value> = images
        .iter()
        .map(|img| {
            let v = b.view(img.png.to_vec(), None);
            json!({ "name": img.name, "bufferView": v, "mimeType": "image/png" })
        })
        .collect();

    let mut doc = json!({
        "asset": { "version": "2.0", "generator": "OxyCharacter" },
        "scene": 0,
        "scenes": [{ "nodes": scene_nodes }],
        "nodes": nodes,
        "meshes": gl_meshes,
        "materials": materials,
        "buffers": [{ "byteLength": b.bin.len() }],
        "bufferViews": b.views,
        "accessors": b.accessors,
    });
    if !skins.is_empty() {
        doc["skins"] = json!(skins);
    }
    if !gl_images.is_empty() {
        doc["textures"] = json!((0..gl_images.len())
            .map(|i| json!({ "source": i, "sampler": 0 }))
            .collect::<Vec<_>>());
        doc["images"] = json!(gl_images);
        doc["samplers"] =
            json!([{ "magFilter": 9729, "minFilter": 9987, "wrapS": 33071, "wrapT": 33071 }]);
    }
    let mut json = serde_json::to_vec(&doc)?;
    pad4(&mut json, b' ');
    let mut bin = b.bin;
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
        let mesh = ExportMesh {
            name: "t",
            positions: &p,
            normals: &[Vec3::Z; 3],
            indices: &[0, 1, 2],
            base_color: [1.0; 4],
            ..Default::default()
        };
        write_glb(&mesh, &mut out).unwrap();
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
