//! The character's look in the viewport: baked skin textures and eyes.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use oxy_io::material::{iris, SkinBaker};
use oxy_material::Image as Rgba;

use crate::editor::Editor;
use crate::scene::Character;
use crate::ui::UiState;

/// Texture size baked for the viewport (exports use their own size).
pub const VIEW_TEXTURE_SIZE: u32 = 1024;

#[derive(Resource)]
pub struct LookAssets {
    albedo: Handle<Image>,
    normal: Handle<Image>,
    orm: Handle<Image>,
    iris: Handle<Image>,
    baker: Option<SkinBaker>,
    /// What the textures were last baked from.
    baked: Option<(oxy_material::skin::SkinParams, [f32; 3])>,
    baked_eyes: Option<oxy_material::eye::EyeParams>,
}

#[derive(Component)]
pub struct EyeEntity(pub usize);

fn image(img: &Rgba, srgb: bool) -> Image {
    Image::new(
        Extent3d {
            width: img.width,
            height: img.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        img.rgba.clone(),
        if srgb {
            TextureFormat::Rgba8UnormSrgb
        } else {
            TextureFormat::Rgba8Unorm
        },
        RenderAssetUsages::default(),
    )
}

fn placeholder(rgba: [u8; 4], srgb: bool) -> Image {
    image(
        &Rgba {
            width: 1,
            height: 1,
            rgba: rgba.to_vec(),
        },
        srgb,
    )
}

pub fn setup_look(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    editor: Res<Editor>,
    character: Single<&MeshMaterial3d<StandardMaterial>, With<Character>>,
) {
    let textured = editor.asset.render.is_some();
    let look = LookAssets {
        albedo: images.add(placeholder([200, 160, 140, 255], true)),
        normal: images.add(placeholder([128, 128, 255, 255], false)),
        orm: images.add(placeholder([255, 140, 0, 255], false)),
        iris: images.add(placeholder([220, 215, 210, 255], true)),
        baker: None,
        baked: None,
        baked_eyes: None,
    };
    if textured {
        if let Some(mut m) = materials.get_mut(&character.0) {
            m.base_color = Color::WHITE;
            m.base_color_texture = Some(look.albedo.clone());
            m.normal_map_texture = Some(look.normal.clone());
            m.metallic_roughness_texture = Some(look.orm.clone());
            m.perceptual_roughness = 1.0;
            m.metallic = 0.0;
        }
    }
    // Eyes: fixed topology/UVs, positions updated with the body.
    let proto = oxy_material::eye::eyeball(0.012, 24, 32);
    let eye_mat = materials.add(StandardMaterial {
        base_color_texture: Some(look.iris.clone()),
        perceptual_roughness: 0.12,
        reflectance: 0.5,
        ..default()
    });
    for i in 0..2 {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            proto
                .positions
                .iter()
                .map(|p| p.to_array())
                .collect::<Vec<_>>(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            proto
                .normals
                .iter()
                .map(|p| p.to_array())
                .collect::<Vec<_>>(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, proto.uvs.clone());
        mesh.insert_indices(Indices::U32(proto.indices.clone()));
        commands.spawn((
            EyeEntity(i),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(eye_mat.clone()),
            Visibility::Hidden,
        ));
    }
    commands.insert_resource(look);
}

/// Re-bake textures when the look changed (not while a slider is dragged).
pub fn bake_textures(
    mut look: ResMut<LookAssets>,
    mut images: ResMut<Assets<Image>>,
    editor: Res<Editor>,
    ui: Res<UiState>,
) {
    if ui.pointer_busy {
        return;
    }
    let key = (editor.material.skin.clone(), editor.skin);
    if look.baked.as_ref() != Some(&key) && editor.asset.render.is_some() {
        if look.baker.is_none() {
            look.baker = SkinBaker::new(&editor.asset, VIEW_TEXTURE_SIZE);
        }
        if let Some(baker) = &look.baker {
            let t = baker.bake(editor.skin, &editor.material.skin);
            for (handle, img, srgb) in [
                (look.albedo.clone(), &t.albedo, true),
                (look.normal.clone(), &t.normal, false),
                (look.orm.clone(), &t.orm, false),
            ] {
                if let Some(mut slot) = images.get_mut(&handle) {
                    *slot = image(img, srgb);
                }
            }
        }
        look.baked = Some(key);
    }
    if look.baked_eyes.as_ref() != Some(&editor.material.eyes) {
        let handle = look.iris.clone();
        if let Some(mut slot) = images.get_mut(&handle) {
            *slot = image(&iris(&editor.material.eyes), true);
        }
        look.baked_eyes = Some(editor.material.eyes.clone());
    }
}

/// Push eye positions computed by the last body update.
pub fn sync_eyes(
    mut editor: ResMut<Editor>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut eyes: Query<(&EyeEntity, &Mesh3d, &mut Visibility)>,
) {
    if !editor.eyes_dirty {
        return;
    }
    editor.eyes_dirty = false;
    for (EyeEntity(i), m, mut vis) in &mut eyes {
        match editor.eyes.get(*i) {
            Some((pos, nrm)) => {
                if let Some(mut mesh) = meshes.get_mut(&m.0) {
                    mesh.insert_attribute(
                        Mesh::ATTRIBUTE_POSITION,
                        pos.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
                    );
                    mesh.insert_attribute(
                        Mesh::ATTRIBUTE_NORMAL,
                        nrm.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
                    );
                }
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}
