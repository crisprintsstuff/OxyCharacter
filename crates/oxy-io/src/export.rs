//! Character export: the morphed body (+ accessories) as a rigged glTF.
//!
//! Shared by the editor and the `oxy` CLI so both produce identical files.

use std::io::Write;

use glam::{Mat4, Vec3};
use oxy_morph::Params;
use oxy_rig::{pose, skin_with, transfer_nearest, Influence};

use crate::gltf::{write_glb_scene, ExportMesh, ExportMorph, ExportSkeleton};
use crate::oxybrush::{Accessory, CUSTOM_PREFIX};
use crate::{CharacterAsset, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindPose {
    /// As modelled (an A-pose for the MakeHuman base).
    #[default]
    Rest,
    /// Arms straight out, the usual bind pose for engine retargeting.
    TPose,
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Include the skeleton and skin weights (when the asset has a rig).
    pub skeleton: bool,
    pub bind_pose: BindPose,
    /// Export sculpted (custom) morphs as blendshapes instead of baking them.
    pub blendshapes: bool,
    /// Linear RGB skin color.
    pub skin_color: [f32; 3],
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            skeleton: true,
            bind_pose: BindPose::Rest,
            blendshapes: false,
            skin_color: [0.8, 0.62, 0.52],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportStats {
    pub meshes: usize,
    pub bones: usize,
    pub blendshapes: usize,
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
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
                    srgb_to_linear(c.x),
                    srgb_to_linear(c.y),
                    srgb_to_linear(c.z),
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
    colors: Option<Vec<[f32; 4]>>,
    base_color: [f32; 3],
    skin: Option<Vec<Influence>>,
    morphs: Vec<(String, Vec<Vec3>, f32)>,
}

fn normals_of(positions: &[Vec3], indices: &[u32]) -> Vec<Vec3> {
    oxy_mesh::Mesh::new(positions.to_vec(), indices.to_vec()).map_or_else(
        |_| vec![Vec3::Y; positions.len()],
        |m| m.compute_normals(positions),
    )
}

pub fn character_glb(
    asset: &CharacterAsset,
    params: &Params,
    accessories: &[Accessory],
    opts: &ExportOptions,
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
    let indices = asset.mesh.base.indices.clone();
    let n = ev.body.len();
    let mut body = Part {
        name: "body".into(),
        normals: asset.mesh.base.compute_normals(&ev.body),
        positions: ev.body.clone(),
        indices,
        colors: None,
        base_color: opts.skin_color,
        skin: None,
        morphs: shapes
            .iter()
            .filter_map(|(id, label, weight)| {
                let t = &asset.mesh.targets[asset.mesh.target_index(id)?];
                let mut dense = vec![Vec3::ZERO; n];
                for (&i, &d) in t.indices.iter().zip(&t.deltas) {
                    dense[i as usize] = d;
                }
                Some((label.clone(), dense, *weight))
            })
            .collect(),
    };
    let mut parts = vec![];
    let rig = asset.rig.as_ref().filter(|_| opts.skeleton);
    for a in accessories {
        let idx: Vec<u32> = a.triangles.iter().flatten().copied().collect();
        parts.push(Part {
            name: a.name.clone(),
            normals: normals_of(&a.positions, &idx),
            positions: a.positions.clone(),
            indices: idx,
            base_color: if accessory_colors(a).is_some() {
                [1.0; 3]
            } else {
                ACCESSORY_CLAY
            },
            colors: accessory_colors(a),
            skin: rig.map(|r| transfer_nearest(&ev.body, &r.skin, &a.positions)),
            morphs: vec![],
        });
    }

    let mut skeleton = None;
    if let Some(rig) = rig {
        body.skin = Some(rig.skin.clone());
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
            for part in std::iter::once(&mut body).chain(parts.iter_mut()) {
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

    parts.insert(0, body);
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
        })
        .collect();
    write_glb_scene(&meshes, skeleton.as_ref(), w)?;
    Ok(ExportStats {
        meshes: meshes.len(),
        bones: skeleton.as_ref().map_or(0, |s| s.joint_names.len()),
        blendshapes: parts[0].morphs.len(),
    })
}
