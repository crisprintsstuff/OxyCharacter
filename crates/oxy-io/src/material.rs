//! Character materials: skin textures baked over the render mesh's UVs, and
//! eyeballs placed on the rig's eye bones.

use glam::{Mat4, Vec3};
use oxy_material::eye::{eye_transform, eyeball, iris_texture, EyeMesh, EyeParams};
use oxy_material::layers::{bake, encode, Textures};
use oxy_material::skin::{region_weights, skin_layers, SkinParams};
use oxy_material::surface::{SurfaceMap, TexelAttributes};
use oxy_material::Image;
use serde::{Deserialize, Serialize};

use crate::{CharacterAsset, Evaluated, IoError, Result};

/// Everything about the character's look that isn't shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MaterialParams {
    pub skin: SkinParams,
    pub eyes: EyeParams,
}

/// Normal-map strength used for skin detail.
pub const SKIN_NORMAL_STRENGTH: f32 = 6.0;

/// Precomputed UV rasterization for one texture size (reuse across bakes).
pub struct SkinBaker {
    pub map: SurfaceMap,
    pub attrs: TexelAttributes,
}

impl SkinBaker {
    /// `None` if the asset has no UVs.
    pub fn new(asset: &CharacterAsset, size: u32) -> Option<Self> {
        let r = asset.render.as_ref()?;
        let map = SurfaceMap::rasterize(&r.uvs, &r.indices, size, 4);
        // Rest (unmorphed) positions keep patterns fixed to the skin.
        let pos = r.gather(&asset.mesh.base.positions);
        let regions: Vec<Vec<f32>> = asset.rig.as_ref().map_or_else(Vec::new, |rig| {
            region_weights(rig).iter().map(|w| r.gather(w)).collect()
        });
        let attrs = TexelAttributes::new(&map, &r.indices, &pos, &regions);
        Some(Self { map, attrs })
    }

    pub fn bake(&self, tone: [f32; 3], skin: &SkinParams) -> Textures {
        let maps = bake(&self.attrs, &skin_layers(tone, skin));
        encode(&maps, &self.attrs.empty, SKIN_NORMAL_STRENGTH)
    }
}

/// One posed or rest eyeball in world space.
pub struct Eye {
    pub name: &'static str,
    pub bone: Option<usize>,
    pub mesh: EyeMesh,
}

/// Eyeballs centred on the `eye.L` / `eye.R` bone heads, looking along the
/// bones, sized from the distance between the eyes. Empty without a rig.
pub fn eyes(asset: &CharacterAsset, ev: &Evaluated, p: &EyeParams) -> Vec<Eye> {
    let Some(rig) = &asset.rig else { return vec![] };
    let (Some(l), Some(r)) = (rig.bone_index("eye.L"), rig.bone_index("eye.R")) else {
        return vec![];
    };
    let head = |b: usize| ev.joints[rig.bones[b].head];
    let tail = |b: usize| ev.joints[rig.bones[b].tail];
    let radius = (head(l).distance(head(r)) * p.size).clamp(0.004, 0.03);
    [("eye.L", l), ("eye.R", r)]
        .into_iter()
        .map(|(name, b)| {
            let forward = (tail(b) - head(b)).try_normalize().unwrap_or(Vec3::Z);
            let m: Mat4 = eye_transform(head(b), forward);
            let mut mesh = eyeball(radius, 24, 32);
            mesh.positions
                .iter_mut()
                .for_each(|v| *v = m.transform_point3(*v));
            mesh.normals
                .iter_mut()
                .for_each(|n| *n = m.transform_vector3(*n).normalize());
            Eye {
                name,
                bone: Some(b),
                mesh,
            }
        })
        .collect()
}

pub fn iris(p: &EyeParams) -> Image {
    iris_texture(p, 256, 128)
}

/// Encode an RGBA8 image as PNG.
pub fn png_bytes(img: &Image) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.width, img.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc
            .write_header()
            .map_err(|e| IoError::Invalid(format!("png: {e}")))?;
        w.write_image_data(&img.rgba)
            .map_err(|e| IoError::Invalid(format!("png: {e}")))?;
    }
    Ok(out)
}
