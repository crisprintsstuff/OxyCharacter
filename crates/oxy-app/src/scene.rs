//! 3D scene: character mesh, lights, orbit camera, live morph updates.

use bevy::{
    asset::RenderAssetUsages,
    camera::{visibility::RenderLayers, CameraOutputMode, Viewport},
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::BlendState,
    window::PrimaryWindow,
};
use bevy_egui::PrimaryEguiContext;

use crate::editor::Editor;

/// Region highlight color (linear RGB) — OxyCharacter's accent.
pub const HIGHLIGHT: [f32; 3] = [0.10, 0.55, 1.0];

#[derive(Component)]
pub struct Character;

#[derive(Component)]
pub struct ViewportCamera;

/// Orbit camera state: target point, yaw/pitch (radians), distance (m).
#[derive(Resource)]
pub struct Orbit {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Orbit {
    pub fn full_body(height: f32) -> Self {
        Self {
            target: Vec3::new(0.0, height * 0.52, 0.0),
            yaw: 0.35,
            pitch: 0.05,
            distance: height * 1.9,
        }
    }
    pub fn head(height: f32) -> Self {
        Self {
            target: Vec3::new(0.0, height * 0.92, 0.0),
            yaw: 0.6,
            pitch: 0.02,
            distance: height * 0.5,
        }
    }
    fn transform(&self) -> Transform {
        let rot = Quat::from_euler(EulerRot::YXZ, self.yaw, -self.pitch, 0.0);
        let eye = self.target + rot * Vec3::new(0.0, 0.0, self.distance);
        Transform::from_translation(eye).looking_at(self.target, Vec3::Y)
    }
}

/// Central (3D) region of the window in logical pixels, reported by the UI.
#[derive(Resource, Default, Clone, Copy)]
pub struct ViewportRect {
    pub min: Vec2,
    pub max: Vec2,
}

/// Current character height (m), updated after each evaluation.
#[derive(Resource)]
pub struct CharacterHeight(pub f32);

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    editor: Res<Editor>,
    mut egui_settings: ResMut<bevy_egui::EguiGlobalSettings>,
) {
    // We spawn the egui camera ourselves (see below).
    egui_settings.auto_create_primary_context = false;
    let base = &editor.asset.mesh.base;
    let n = base.vertex_count();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; n]);
    mesh.insert_indices(Indices::U32(base.indices.clone()));

    let [r, g, b] = editor.skin;
    commands.spawn((
        Character,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::linear_rgb(r, g, b),
            perceptual_roughness: 0.55,
            reflectance: 0.35,
            ..default()
        })),
    ));

    // Floor disc.
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(0.75).mesh().resolution(96))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.025, 0.027, 0.03),
            perceptual_roughness: 1.0,
            reflectance: 0.1,
            ..default()
        })),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    // Three-point lighting.
    for (dir, lux, color) in [
        (
            Vec3::new(-0.6, -0.5, -0.7),
            9000.0,
            Color::srgb(1.0, 0.97, 0.92),
        ), // key
        (
            Vec3::new(0.8, -0.2, -0.4),
            3000.0,
            Color::srgb(0.85, 0.9, 1.0),
        ), // fill
        (
            Vec3::new(0.2, -0.3, 1.0),
            6000.0,
            Color::srgb(0.9, 0.95, 1.0),
        ), // rim
    ] {
        commands.spawn((
            DirectionalLight {
                illuminance: lux,
                color,
                ..default()
            },
            Transform::default().looking_to(dir, Vec3::Y),
        ));
    }
    commands.insert_resource(GlobalAmbientLight {
        brightness: 250.0,
        ..default()
    });

    let orbit = Orbit::full_body(1.7);
    commands.spawn((ViewportCamera, Camera3d::default(), orbit.transform()));
    commands.insert_resource(orbit);

    // UI camera drawn on top of the whole window.
    commands.spawn((
        PrimaryEguiContext,
        Camera2d,
        RenderLayers::none(),
        Camera {
            order: 1,
            output_mode: CameraOutputMode::Write {
                blend_state: Some(BlendState::ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            },
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
    ));
}

/// Re-evaluate morphs and push positions/normals/highlight into the GPU mesh.
pub fn update_character(
    mut editor: ResMut<Editor>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut height: ResMut<CharacterHeight>,
    query: Single<(&Mesh3d, &MeshMaterial3d<StandardMaterial>), With<Character>>,
) {
    let (mesh_handle, mat_handle) = *query;
    if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
        let [r, g, b] = editor.skin;
        let c = Color::linear_rgb(r, g, b);
        if mat.base_color != c {
            mat.base_color = c;
        }
    }
    if !editor.dirty && !editor.highlight_dirty {
        return;
    }
    let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) else {
        return;
    };
    let ed = &mut *editor;

    if ed.dirty {
        let weights = ed.asset.catalog.evaluate(&ed.params);
        let (dense, _) = oxy_morph::resolve(&ed.asset.mesh, &weights);
        ed.active_targets = dense
            .iter()
            .filter(|w| w.abs() >= oxy_mesh::WEIGHT_EPSILON)
            .count();
        let mut pos = ed.asset.mesh.evaluate(&dense);
        // Stand on the floor.
        let (lo, hi) = oxy_io::makehuman::bounds(&pos);
        for p in &mut pos {
            p.y -= lo.y;
        }
        height.0 = hi.y - lo.y;
        let normals = ed.asset.mesh.base.compute_normals(&pos);
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            pos.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            normals.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
        );
        ed.dirty = false;
    }

    if ed.highlight_dirty {
        let n = ed.asset.mesh.base.vertex_count();
        let mut amount = vec![0.0f32; n];
        if let Some(id) = &ed.hovered {
            for name in ed.asset.catalog.targets_for(id) {
                if let Some(t) = ed
                    .asset
                    .mesh
                    .target_index(name)
                    .map(|i| &ed.asset.mesh.targets[i])
                {
                    for (&i, d) in t.indices.iter().zip(&t.deltas) {
                        amount[i as usize] += d.length();
                    }
                }
            }
        }
        let max = amount.iter().copied().fold(0.0f32, f32::max);
        let colors: Vec<[f32; 4]> = amount
            .iter()
            .map(|&a| {
                let t = if max > 0.0 {
                    (a / max).sqrt().min(1.0) * 0.85
                } else {
                    0.0
                };
                let c = Vec3::ONE.lerp(Vec3::from_array(HIGHLIGHT) * 1.6, t);
                [c.x, c.y, c.z, 1.0]
            })
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        ed.highlight_dirty = false;
    }
}

/// Orbit (left drag), pan (right/middle drag), zoom (wheel) — only over the viewport.
pub fn orbit_controls(
    mut orbit: ResMut<Orbit>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    rect: Res<ViewportRect>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let inside = window.cursor_position().is_some_and(|c| {
        c.x >= rect.min.x && c.y >= rect.min.y && c.x <= rect.max.x && c.y <= rect.max.y
    });
    if !inside {
        return;
    }
    let d = motion.delta;
    if buttons.pressed(MouseButton::Left) {
        orbit.yaw -= d.x * 0.008;
        orbit.pitch = (orbit.pitch + d.y * 0.006).clamp(-1.4, 1.4);
    } else if buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Middle) {
        let rot = Quat::from_euler(EulerRot::YXZ, orbit.yaw, -orbit.pitch, 0.0);
        let k = orbit.distance * 0.0015;
        orbit.target += rot * Vec3::new(-d.x * k, d.y * k, 0.0);
    }
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    if lines != 0.0 {
        orbit.distance = (orbit.distance * (1.0 - lines * 0.1)).clamp(0.15, 8.0);
    }
}

/// Apply the orbit transform and restrict the 3D camera to the central region.
pub fn apply_camera(
    orbit: Res<Orbit>,
    rect: Res<ViewportRect>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut cam: Single<(&mut Transform, &mut Camera), With<ViewportCamera>>,
) {
    *cam.0 = orbit.transform();
    let s = window.scale_factor();
    let size = (rect.max - rect.min) * s;
    if size.x < 8.0 || size.y < 8.0 {
        return;
    }
    let pos = (rect.min * s).as_uvec2();
    let phys = UVec2::new(window.physical_width(), window.physical_height());
    let size = size.as_uvec2().min(phys.saturating_sub(pos));
    cam.1.viewport = Some(Viewport {
        physical_position: pos,
        physical_size: size,
        ..default()
    });
}
