//! Character export: the morphed body (+ eyes and accessories) as a rigged,
//! textured glTF.
//!
//! Shared by the editor and the `oxy` CLI so both produce identical files.

use std::io::Write;

use glam::{Mat4, Vec3};
use oxy_fit::without_triangles;
use oxy_material::hair::strand_textures;
use oxy_morph::Params;
use oxy_rig::{pose, skin_with, transfer_nearest, Influence};

use crate::expression::{Expression, FaceShapes, PRESETS, VISEMES};
use crate::gltf::{
    write_glb_scene, ExportImage, ExportMesh, ExportMorph, ExportSkeleton, MeshTextures,
};
use crate::material::{eyes, iris, png_bytes, MaterialParams, SkinBaker};
use crate::oxybrush::{Accessory, CUSTOM_PREFIX};
use crate::{CharacterAsset, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum BindPose {
    /// As modelled (an A-pose for the MakeHuman base).
    #[default]
    Rest,
    /// Arms straight out, the usual bind pose for engine retargeting.
    TPose,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ExportOptions {
    /// Include the skeleton and skin weights (when the asset has a rig).
    pub skeleton: bool,
    pub bind_pose: BindPose,
    /// Export sculpted (custom) morphs as blendshapes instead of baking them.
    pub blendshapes: bool,
    /// Bake and embed skin/eye textures (needs UVs in the asset).
    pub textures: bool,
    /// Skin texture size in pixels (square).
    pub texture_size: u32,
    /// Include eyeballs.
    pub eyes: bool,
    /// Export facial expressions as blendshapes (`expr.*` units at their
    /// current weight, `face.*` presets and `viseme.*` mouth shapes at 0).
    /// Without it the current expression is baked into the body.
    pub expressions: bool,
    /// Linear RGB skin color (set by the caller; projects store skin separately).
    #[serde(skip)]
    pub skin_color: [f32; 3],
    /// Current facial expression (set by the caller; projects store it separately).
    #[serde(skip)]
    pub expression: Expression,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            skeleton: true,
            bind_pose: BindPose::Rest,
            blendshapes: false,
            textures: true,
            texture_size: 2048,
            eyes: true,
            expressions: true,
            skin_color: [0.62, 0.40, 0.29],
            expression: Expression::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportStats {
    pub meshes: usize,
    pub bones: usize,
    pub blendshapes: usize,
    pub textures: usize,
}

/// A garment or hair mesh already fitted to the exported body (rest pose).
pub struct WornMesh {
    pub name: String,
    pub positions: Vec<Vec3>,
    pub indices: Vec<u32>,
    pub uvs: Vec<[f32; 2]>,
    pub skin: Option<Vec<Influence>>,
    /// Linear RGBA. Hair with textures uses this as the strand colour.
    pub base_color: [f32; 4],
    pub hair: bool,
    pub double_sided: bool,
}

/// Clay color for unpainted accessories (linear).
pub const ACCESSORY_CLAY: [f32; 3] = [0.42, 0.44, 0.48];

/// Linear RGBA vertex colors of a painted accessory.
pub fn accessory_colors(a: &Accessory) -> Option<Vec<[f32; 4]>> {
    (a.color.len() == a.positions.len() && !a.color.is_empty()).then(|| {
        a.color
            .iter()
            .map(|c| {
                [
                    oxy_material::srgb_to_linear(c.x),
                    oxy_material::srgb_to_linear(c.y),
                    oxy_material::srgb_to_linear(c.z),
                    1.0,
                ]
            })
            .collect()
    })
}

struct Part {
    name: String,
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    indices: Vec<u32>,
    uvs: Option<Vec<[f32; 2]>>,
    colors: Option<Vec<[f32; 4]>>,
    base_color: [f32; 3],
    skin: Option<Vec<Influence>>,
    morphs: Vec<(String, Vec<Vec3>, f32)>,
    textures: MeshTextures,
    alpha_mask: bool,
    double_sided: bool,
    anisotropy: Option<f32>,
}

/// Named expression blendshapes on the base body: every unit (`expr.<id>`,
/// at its current weight), then presets (`face.<id>`) and visemes
/// (`viseme.<id>`) at weight 0.
fn expression_morphs<'a>(
    face: &'a FaceShapes,
    current: &'a Expression,
    n: usize,
) -> impl Iterator<Item = (String, Vec<Vec3>, f32)> + 'a {
    let units = face.available().map(move |u| {
        (
            format!("expr.{}", u.id),
            face.dense(n, [(u.id, 1.0)]),
            current.get(u.id),
        )
    });
    let mixes = PRESETS
        .iter()
        .map(|m| ("face", m))
        .chain(VISEMES.iter().map(|m| ("viseme", m)))
        .filter(|(_, m)| !m.units.is_empty())
        .map(move |(kind, m)| {
            (
                format!("{kind}.{}", m.id),
                face.dense(n, m.units.iter().copied()),
                0.0,
            )
        });
    units.chain(mixes)
}

fn normals_of(positions: &[Vec3], indices: &[u32]) -> Vec<Vec3> {
    let mut n = oxy_mesh::Mesh::new(positions.to_vec(), indices.to_vec()).map_or_else(
        |_| vec![Vec3::Y; positions.len()],
        |m| m.compute_normals(positions),
    );
    // A vertex on only degenerate faces gets a zero normal, which the glTF
    // validator rejects. A unit fallback keeps the file valid.
    for v in &mut n {
        if v.length_squared() < 1e-8 {
            *v = Vec3::Y;
        }
    }
    n
}

#[allow(clippy::too_many_arguments)]
pub fn character_glb(
    asset: &CharacterAsset,
    params: &Params,
    accessories: &[Accessory],
    material: &MaterialParams,
    opts: &ExportOptions,
    worn: &[WornMesh],
    hidden_body: &[u32],
    w: impl Write,
) -> Result<ExportStats> {
    // With blendshapes, sculpted morphs are left out of the base shape and
    // exported as targets whose default weight is their current value.
    let mut base_params = params.clone();
    let mut shapes: Vec<(String, String, f32)> = Vec::new();
    if opts.blendshapes {
        for p in asset
            .catalog
            .params
            .iter()
            .filter(|p| p.id.starts_with(CUSTOM_PREFIX))
        {
            shapes.push((
                p.id.clone(),
                p.label.clone(),
                params.get(&p.id).copied().unwrap_or(0.0),
            ));
            base_params.insert(p.id.clone(), 0.0);
        }
    }
    let ev = asset.evaluate_full(&base_params);
    let n = ev.body.len();
    let rig = asset.rig.as_ref().filter(|_| opts.skeleton);
    let render = asset.render.as_ref();
    // Per base vertex data, gathered through the render mesh when there is one.
    let gather = |v: Vec<Vec3>| render.map_or_else(|| v.clone(), |r| r.gather(&v));

    // Textures.
    let mut pngs: Vec<(String, Vec<u8>)> = Vec::new();
    let mut body_tex = MeshTextures {
        roughness: Some(material.skin.roughness),
        ..Default::default()
    };
    let mut eye_tex = MeshTextures {
        roughness: Some(0.12),
        ..Default::default()
    };
    if opts.textures {
        if let Some(baker) = SkinBaker::new(asset, opts.texture_size.clamp(64, 8192)) {
            let t = baker.bake(opts.skin_color, &material.skin);
            for (slot, name, img) in [
                (0, "skin_albedo", &t.albedo),
                (1, "skin_normal", &t.normal),
                (2, "skin_roughness", &t.orm),
            ] {
                let i = pngs.len();
                pngs.push((name.into(), png_bytes(img)?));
                match slot {
                    0 => body_tex.base_color = Some(i),
                    1 => body_tex.normal = Some(i),
                    _ => body_tex.metallic_roughness = Some(i),
                }
            }
            body_tex.roughness = None;
        }
        if opts.eyes {
            eye_tex.base_color = Some(pngs.len());
            pngs.push(("iris".into(), png_bytes(&iris(&material.eyes))?));
        }
    }
    let textured_body = body_tex.base_color.is_some();

    let mut body_morphs: Vec<(String, Vec<Vec3>, f32)> = shapes
        .iter()
        .filter_map(|(id, label, weight)| {
            let t = &asset.mesh.targets[asset.mesh.target_index(id)?];
            let mut dense = vec![Vec3::ZERO; n];
            for (&i, &d) in t.indices.iter().zip(&t.deltas) {
                dense[i as usize] = d;
            }
            Some((label.clone(), gather(dense), *weight))
        })
        .collect();
    // Facial expressions, carried onto this shape. The base stays neutral
    // when they are blendshapes; otherwise the current expression is baked.
    let face = FaceShapes::new(asset, &ev.body);
    let mut body_pos = ev.body.clone();
    if !face.is_empty() {
        if opts.expressions {
            body_morphs.extend(
                expression_morphs(&face, &opts.expression, n)
                    .map(|(name, d, w)| (name, gather(d), w)),
            );
        } else {
            face.apply_expression(&opts.expression, &mut body_pos);
        }
    }
    let body = Part {
        name: "body".into(),
        normals: gather(asset.mesh.base.compute_normals(&body_pos)),
        positions: gather(body_pos),
        indices: without_triangles(
            &render.map_or_else(|| asset.mesh.base.indices.clone(), |r| r.indices.clone()),
            hidden_body,
        ),
        uvs: render.map(|r| r.uvs.clone()),
        colors: None,
        base_color: if textured_body {
            [1.0; 3]
        } else {
            opts.skin_color
        },
        skin: rig.map(|r| render.map_or_else(|| r.skin.clone(), |rm| rm.gather(&r.skin))),
        morphs: body_morphs,
        textures: body_tex,
        alpha_mask: false,
        double_sided: false,
        anisotropy: None,
    };
    let mut parts = vec![body];

    if opts.eyes {
        for e in eyes(asset, &ev, &material.eyes) {
            let skin = rig.zip(e.bone).map(|(_, b)| {
                vec![
                    Influence {
                        bones: [b as u16, 0, 0, 0],
                        weights: [1.0, 0.0, 0.0, 0.0]
                    };
                    e.mesh.positions.len()
                ]
            });
            parts.push(Part {
                name: e.name.into(),
                positions: e.mesh.positions,
                normals: e.mesh.normals,
                indices: e.mesh.indices,
                uvs: eye_tex.base_color.is_some().then_some(e.mesh.uvs),
                colors: None,
                base_color: if eye_tex.base_color.is_some() {
                    [1.0; 3]
                } else {
                    [0.85, 0.82, 0.78]
                },
                skin,
                morphs: vec![],
                textures: eye_tex,
                alpha_mask: false,
                double_sided: false,
                anisotropy: None,
            });
        }
    }

    for a in accessories {
        let idx: Vec<u32> = a.triangles.iter().flatten().copied().collect();
        parts.push(Part {
            name: a.name.clone(),
            normals: normals_of(&a.positions, &idx),
            positions: a.positions.clone(),
            indices: idx,
            uvs: None,
            base_color: if accessory_colors(a).is_some() {
                [1.0; 3]
            } else {
                ACCESSORY_CLAY
            },
            colors: accessory_colors(a),
            skin: rig.map(|r| transfer_nearest(&ev.body, &r.skin, &a.positions)),
            morphs: vec![],
            textures: MeshTextures::default(),
            alpha_mask: false,
            double_sided: true,
            anisotropy: None,
        });
    }

    for wmesh in worn {
        let mut textures = MeshTextures::default();
        let mut base_color = [
            wmesh.base_color[0],
            wmesh.base_color[1],
            wmesh.base_color[2],
        ];
        let mut alpha_mask = false;
        let mut anisotropy = None;
        if wmesh.hair && opts.textures {
            let maps = strand_textures(
                [
                    wmesh.base_color[0],
                    wmesh.base_color[1],
                    wmesh.base_color[2],
                ],
                256,
            );
            let albedo = pngs.len();
            pngs.push((format!("{}_albedo", wmesh.name), png_bytes(&maps.albedo)?));
            let normal = pngs.len();
            pngs.push((format!("{}_normal", wmesh.name), png_bytes(&maps.normal)?));
            textures.base_color = Some(albedo);
            textures.normal = Some(normal);
            textures.roughness = Some(0.45);
            base_color = [1.0; 3];
            alpha_mask = true;
            anisotropy = Some(0.85);
        }
        let uvs = (wmesh.uvs.len() == wmesh.positions.len()).then(|| wmesh.uvs.clone());
        parts.push(Part {
            name: wmesh.name.clone(),
            normals: normals_of(&wmesh.positions, &wmesh.indices),
            positions: wmesh.positions.clone(),
            indices: wmesh.indices.clone(),
            uvs,
            colors: None,
            base_color,
            skin: wmesh.skin.clone().filter(|_| rig.is_some()),
            morphs: vec![],
            textures,
            alpha_mask,
            double_sided: wmesh.double_sided,
            anisotropy,
        });
    }

    let mut skeleton = None;
    if let Some(rig) = rig {
        let mut bind = rig.bind_matrices(&ev.joints);
        if opts.bind_pose == BindPose::TPose {
            // Pose everything into the T-pose and make that the bind pose.
            let local = pose::solve(rig, &bind, pose::preset("T-pose").expect("built-in preset"));
            let posed = rig.pose_matrices(&bind, &local);
            let mats: Vec<Mat4> = posed
                .iter()
                .zip(&bind)
                .map(|(p, b)| *p * b.inverse())
                .collect();
            for part in &mut parts {
                let skin = part.skin.as_ref().expect("skinned");
                let (p, nrm) = skin_with(skin, &part.positions, &part.normals, &mats);
                for (_, deltas, _) in &mut part.morphs {
                    let moved: Vec<Vec3> = part
                        .positions
                        .iter()
                        .zip(deltas.iter())
                        .map(|(a, d)| *a + *d)
                        .collect();
                    let (pm, _) = skin_with(skin, &moved, &part.normals, &mats);
                    *deltas = pm.iter().zip(&p).map(|(a, b)| *a - *b).collect();
                }
                part.positions = p;
                part.normals = nrm;
            }
            bind = posed;
        }
        skeleton = Some(ExportSkeleton {
            name: &rig.name,
            joint_names: rig.bones.iter().map(|b| b.name.as_str()).collect(),
            parents: rig.bones.iter().map(|b| b.parent).collect(),
            local: rig.local_bind(&bind),
            inverse_bind: bind
                .iter()
                .map(|m| Mat4::from(glam::Affine3A::from_mat4(*m).inverse()))
                .collect(),
        });
    }

    let meshes: Vec<ExportMesh> = parts
        .iter()
        .map(|p| ExportMesh {
            name: &p.name,
            positions: &p.positions,
            normals: &p.normals,
            indices: &p.indices,
            base_color: [p.base_color[0], p.base_color[1], p.base_color[2], 1.0],
            colors: p.colors.as_deref(),
            skin: p.skin.as_deref(),
            morphs: p
                .morphs
                .iter()
                .map(|(name, d, w)| ExportMorph {
                    name,
                    deltas: d.clone(),
                    weight: *w,
                })
                .collect(),
            uvs: p.uvs.as_deref(),
            textures: p.textures,
            alpha_cutoff: p.alpha_mask.then_some(0.5),
            double_sided: p.double_sided,
            anisotropy: p.anisotropy,
        })
        .collect();
    let images: Vec<ExportImage> = pngs
        .iter()
        .map(|(name, png)| ExportImage { name, png })
        .collect();
    write_glb_scene(&meshes, skeleton.as_ref(), &images, w)?;
    Ok(ExportStats {
        meshes: meshes.len(),
        bones: skeleton.as_ref().map_or(0, |s| s.joint_names.len()),
        blendshapes: parts[0].morphs.len(),
        textures: images.len(),
    })
}
