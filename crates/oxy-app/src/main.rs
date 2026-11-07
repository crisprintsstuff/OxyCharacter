//! OxyCharacter editor.
//!
//! ```text
//! oxycharacter [--bundle PATH] [--set id=value ...] [--select "Top/Sub"]
//!              [--hover PARAM_ID] [--head] [--size WxH]
//!              [--import-oxb FILE.oxb [--apply-import]] [--send-oxybrush]
//!              [--pose NAME] [--skeleton] [--pose-tab] [--open FILE.oxyc]
//!              [--save-as FILE.oxyc] [--close]
//!              [--screenshot out.png [--frames N]]
//! ```

mod bridge;
mod document;
mod editor;
mod scene;
mod ui;

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::process::ExitCode;

use bevy::{
    prelude::*,
    render::view::screenshot::{save_to_disk, Screenshot},
    window::WindowResolution,
};
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

use editor::Editor;
use scene::{CharacterHeight, Orbit, ViewportRect};
use ui::UiState;

const DEFAULT_BUNDLE: &str = "assets/generated/makehuman.oxyb";

#[derive(Default)]
struct Args {
    bundle: Option<PathBuf>,
    sets: Vec<(String, f32)>,
    select: Option<(String, String)>,
    hover: Option<String>,
    head: bool,
    size: (u32, u32),
    screenshot: Option<PathBuf>,
    frames: u32,
    import_oxb: Option<PathBuf>,
    apply_import: bool,
    send_oxybrush: bool,
    pose: Option<String>,
    skeleton: bool,
    pose_tab: bool,
    open: Option<PathBuf>,
    save_as: Option<PathBuf>,
    close: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        size: (1600, 900),
        frames: 30,
        ..Default::default()
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--bundle" => a.bundle = Some(val()?.into()),
            "--set" => {
                let kv = val()?;
                let (k, v) = kv.split_once('=').ok_or("--set expects id=value")?;
                a.sets.push((k.into(), v.parse().map_err(|_| format!("bad value in {kv}"))?));
            }
            "--select" => {
                let s = val()?;
                let (t, sub) = s.split_once('/').ok_or("--select expects Top/Sub")?;
                a.select = Some((t.into(), sub.into()));
            }
            "--hover" => a.hover = Some(val()?),
            "--head" => a.head = true,
            "--size" => {
                let s = val()?;
                let (w, h) = s.split_once('x').ok_or("--size expects WxH")?;
                a.size = (w.parse().map_err(|_| "bad width")?, h.parse().map_err(|_| "bad height")?);
            }
            "--screenshot" => a.screenshot = Some(val()?.into()),
            "--frames" => a.frames = val()?.parse().map_err(|_| "bad frame count")?,
            "--import-oxb" => a.import_oxb = Some(val()?.into()),
            "--apply-import" => a.apply_import = true,
            "--send-oxybrush" => a.send_oxybrush = true,
            "--pose" => a.pose = Some(val()?),
            "--skeleton" => a.skeleton = true,
            "--pose-tab" => a.pose_tab = true,
            "--open" => a.open = Some(val()?.into()),
            "--save-as" => a.save_as = Some(val()?.into()),
            "--close" => a.close = true,
            "-h" | "--help" => return Err("see crate docs: oxycharacter [--bundle PATH] [--set id=v] [--select Top/Sub] [--hover ID] [--head] [--size WxH] [--import-oxb FILE [--apply-import]] [--screenshot out.png [--frames N]]".into()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(a)
}

/// Scripted screenshot capture (used for docs and CI smoke tests).
#[derive(Resource)]
struct Automation {
    screenshot: Option<PathBuf>,
    frames: u32,
    frame: u32,
    head: bool,
    save_as: Option<PathBuf>,
    close: bool,
}

fn automation(
    mut commands: Commands,
    mut auto: ResMut<Automation>,
    mut orbit: ResMut<Orbit>,
    height: Res<CharacterHeight>,
    mut exit: MessageWriter<AppExit>,
    mut editor: ResMut<Editor>,
) {
    auto.frame += 1;
    if auto.frame == 5 {
        if let Some(p) = auto.save_as.take() {
            editor.save_project(&p);
            println!("{}", editor.status);
        }
        if auto.close {
            // Same path as the window's close button.
            editor.request(document::After::Quit);
        }
    }
    if auto.frame == 3 {
        *orbit = if auto.head {
            Orbit::head(height.0)
        } else {
            Orbit::full_body(height.0)
        };
    }
    let Some(path) = auto.screenshot.clone() else {
        return;
    };
    if auto.frame == auto.frames {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    if auto.frame == auto.frames + 15 {
        exit.write(AppExit::Success);
    }
}

fn export(mut state: ResMut<UiState>, mut editor: ResMut<Editor>) {
    if !std::mem::take(&mut state.export_requested) {
        return;
    }
    let ed = &mut *editor;
    let accessories: Vec<oxy_io::oxybrush::Accessory> = ed
        .accessories
        .iter()
        .filter(|a| a.visible)
        .map(|a| a.acc.clone())
        .collect();
    let opts = oxy_io::export::ExportOptions {
        skin_color: ed.skin,
        ..ed.export.clone()
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let path = PathBuf::from(format!("exports/character-{stamp}.glb"));
    let result = std::fs::create_dir_all("exports")
        .map_err(oxy_io::IoError::from)
        .and_then(|_| {
            oxy_io::export::character_glb(
                &ed.asset,
                &ed.params,
                &accessories,
                &opts,
                BufWriter::new(File::create(&path)?),
            )
        });
    ed.status = match result {
        Ok(st) => format!(
            "Exported {} ({} mesh(es), {} bones, {} blendshape(s))",
            path.display(),
            st.meshes,
            st.bones,
            st.blendshapes
        ),
        Err(e) => format!("Export failed: {e}"),
    };
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let bundle = args
        .bundle
        .clone()
        .or_else(|| std::env::var_os("OXY_BUNDLE").map(PathBuf::from))
        .unwrap_or_else(|| DEFAULT_BUNDLE.into());
    let asset = match File::open(&bundle)
        .map_err(oxy_io::IoError::from)
        .and_then(|f| oxy_io::bundle::read(BufReader::new(f)))
    {
        Ok(a) => a,
        Err(e) => {
            eprintln!("could not load character bundle {}: {e}", bundle.display());
            eprintln!("run `tools/fetch-assets.sh` first (downloads the CC0 base mesh and builds the bundle)");
            return ExitCode::FAILURE;
        }
    };

    let user_dir =
        std::env::var_os("OXY_USER_DIR").map_or_else(|| PathBuf::from("user"), PathBuf::from);
    let mut editor = Editor::new(asset, &user_dir);
    editor.load_custom_morphs();
    editor.init_session(&user_dir);
    if let Some(p) = &args.open {
        if !editor.open_project(p) {
            eprintln!("{}", editor.status);
            return ExitCode::FAILURE;
        }
    }
    for (k, v) in &args.sets {
        if editor.asset.catalog.param(k).is_none() {
            eprintln!("unknown param {k}");
            return ExitCode::FAILURE;
        }
        editor.set(k, *v);
    }
    if let Some(sel) = args.select.clone() {
        editor.selected = sel;
    }
    editor.pinned_hover = args.hover.clone();
    if let Some(name) = &args.pose {
        match oxy_rig::pose::PRESETS
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
        {
            Some(i) => editor.set_pose(i),
            None => {
                let names: Vec<_> = oxy_rig::pose::PRESETS.iter().map(|p| p.name).collect();
                eprintln!("unknown pose {name}; one of {names:?}");
                return ExitCode::FAILURE;
            }
        }
    }
    editor.show_skeleton = args.skeleton;
    if args.send_oxybrush {
        // Scripted runs never start other programs.
        editor.bridge.launch = false;
        editor.send_to_oxybrush();
        println!(
            "sent {}",
            editor
                .bridge
                .watched
                .as_ref()
                .map_or_else(String::new, |w| w.0.display().to_string())
        );
    }
    if let Some(p) = &args.import_oxb {
        editor.open_oxb(p);
        if args.apply_import {
            editor.apply_pending();
        }
    }

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.085, 0.09, 0.1)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "OxyCharacter".into(),
                resolution: WindowResolution::new(args.size.0, args.size.1),
                ..default()
            }),
            // Closing asks about unsaved changes first (see document::close_requested).
            close_when_requested: false,
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_resource(editor)
        .insert_resource(CharacterHeight(1.7))
        .insert_resource(ViewportRect::default())
        .insert_resource(UiState {
            right_mode: if args.pose_tab {
                ui::RightMode::Pose
            } else {
                ui::RightMode::Morph
            },
            ..Default::default()
        })
        .insert_resource(Automation {
            screenshot: args.screenshot,
            save_as: args.save_as.clone(),
            close: args.close,
            frames: args.frames,
            frame: 0,
            head: args.head,
        })
        .add_systems(Startup, (scene::setup, scene::configure_gizmos))
        .add_systems(EguiPrimaryContextPass, ui::ui_system)
        .add_systems(
            Update,
            (
                scene::orbit_controls.run_if(|s: Res<UiState>| !s.pointer_busy),
                scene::update_character,
                scene::apply_camera,
                export,
                document::session_system,
                document::close_requested,
                bridge::watch,
                bridge::file_drop,
                scene::sync_accessories,
                scene::draw_skeleton,
                automation,
            )
                .chain(),
        )
        .run();
    ExitCode::SUCCESS
}
