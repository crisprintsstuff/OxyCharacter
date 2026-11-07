//! Procedural, layered materials baked to textures (no GPU).
//!
//! 1. [`surface::SurfaceMap`] rasterizes a mesh's UV layout once: each texel
//!    knows which triangle and barycentric point of the surface it shows.
//! 2. Per-vertex attributes (rest-pose position, normal, region masks) are
//!    interpolated to texels ([`surface::TexelAttributes`]).
//! 3. A stack of [`layers::Layer`]s (colour or 3D noise, masked by regions or
//!    noise, with a blend mode) is evaluated per texel into albedo, roughness
//!    and height; [`layers::encode`] turns those into RGBA8 albedo (sRGB),
//!    tangent-space normal and occlusion/roughness/metal maps.
//!
//! Noise is evaluated at the 3D surface point, so patterns are continuous
//! across UV seams and don't stretch with the UV layout.
//!
//! [`skin`] builds the skin stack from a few user parameters; [`eye`] makes
//! eyeball meshes and iris textures. All patterns and parameters are our own.

pub mod eye;
pub mod layers;
pub mod noise;
pub mod skin;
pub mod surface;

/// An RGBA8 image, row-major, top row first.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Linear → sRGB transfer for one channel.
pub fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}
