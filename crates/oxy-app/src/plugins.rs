//! Running plugins in the editor.
//!
//! A plugin runs as a child process (see `oxy-plugin`). Its requests are
//! answered here, on the main thread, against the live editor, a batch per
//! frame. While a plugin runs, undo snapshots are held back; the whole run is
//! committed as one undo step labelled with the plugin's name.

use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Instant;

use bevy::prelude::*;
use oxy_io::export::ExportOptions;
use oxy_io::material::MaterialParams;
use oxy_io::oxybrush::{CustomMorph, CUSTOM_PREFIX};
use oxy_io::CharacterAsset;
use oxy_morph::Params;
use oxy_plugin::runner::{discover, sdk_dir, spawn, PluginInfo, Running};
use oxy_plugin::Host;
use serde_json::{json, Value};

use crate::editor::Editor;
use crate::ui::UiState;

/// Maximum requests answered per frame (keeps the UI responsive).
const CALLS_PER_FRAME: usize = 64;
const LOG_LINES: usize = 1000;

#[derive(Default)]
pub struct PluginState {
    pub list: Vec<PluginInfo>,
    pub running: Option<(PluginInfo, Running, Instant)>,
    pub log: Vec<String>,
    pub log_open: bool,
    pub script_input: String,
    /// Set by the menu: start this plugin next frame.
    pub request: Option<PathBuf>,
    pub stop: bool,
    pub scanned: bool,
}

impl PluginState {
    pub fn push_log(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
        if self.log.len() > LOG_LINES {
            let drop = self.log.len() - LOG_LINES;
            self.log.drain(..drop);
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }
}

/// Folders searched for plugins: the user's `plugins/` and the bundled
/// examples (next to the executable, or a checkout's `examples/plugins`).
pub fn plugin_dirs(user_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![user_dir.join("plugins")];
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        for rel in [
            "plugins",
            "../share/oxycharacter/plugins",
            "../Resources/plugins",
        ] {
            let d = exe_dir.join(rel);
            if d.is_dir() {
                dirs.push(d);
            }
        }
        for anc in exe_dir.ancestors() {
            let d = anc.join("examples/plugins");
            if d.is_dir() {
                dirs.push(d);
                break;
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let d = cwd.join("examples/plugins");
        if d.is_dir() && !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    dirs
}

pub fn rescan(state: &mut PluginState, user_dir: &Path) {
    let dirs = plugin_dirs(user_dir);
    let _ = std::fs::create_dir_all(&dirs[0]);
    state.list = discover(&dirs);
    state.scanned = true;
}

/// The live editor as a plugin host.
struct EditorHost<'a> {
    ed: &'a mut Editor,
    unsaved: bool,
    notes: Vec<String>,
}

impl Host for EditorHost<'_> {
    fn kind(&self) -> &'static str {
        "editor"
    }
    fn asset(&self) -> &CharacterAsset {
        &self.ed.asset
    }
    fn params(&self) -> &Params {
        &self.ed.params
    }
    fn set_param(&mut self, id: &str, value: f32) {
        // Plugins set exactly what they ask for (no symmetric mirroring).
        self.ed.params.insert(id.to_owned(), value);
        self.ed.dirty = true;
    }
    fn material(&self) -> (MaterialParams, [f32; 3]) {
        (self.ed.material.clone(), self.ed.skin)
    }
    fn set_material(&mut self, material: MaterialParams, skin_tone: [f32; 3]) {
        if material.eyes.size != self.ed.material.eyes.size {
            self.ed.dirty = true; // eye geometry
        }
        self.ed.material = material;
        self.ed.skin = skin_tone;
    }
    fn pose(&self) -> usize {
        self.ed.pose
    }
    fn set_pose(&mut self, index: usize) {
        self.ed.set_pose(index);
    }
    fn expression(&self) -> oxy_io::expression::Expression {
        self.ed.expression.clone()
    }
    fn set_expression(&mut self, expression: oxy_io::expression::Expression) {
        self.ed.set_expression(expression);
    }
    fn add_custom_morph(&mut self, morph: CustomMorph) -> String {
        let id = format!("{CUSTOM_PREFIX}{}", morph.id);
        self.ed.register_custom_morph(&morph);
        self.ed.params.insert(id.clone(), 0.0);
        id
    }
    fn wardrobe(&self) -> Vec<(String, String, bool)> {
        oxy_fit::builtin_ids()
            .iter()
            .map(|id| {
                let worn = self
                    .ed
                    .worn
                    .iter()
                    .any(|w| w.builtin.as_deref() == Some(*id));
                (
                    id.to_string(),
                    oxy_fit::builtin_name(id).unwrap_or(id).to_string(),
                    worn,
                )
            })
            .collect()
    }
    fn wear(&mut self, spec: &str) -> Result<(), String> {
        for item in spec.split(',').filter(|s| !s.is_empty()) {
            let id = item.split(':').next().unwrap_or("").trim();
            if oxy_fit::builtin_name(id).is_none() {
                return Err(format!("unknown wardrobe item `{id}`"));
            }
        }
        self.ed.wear_specs(spec);
        Ok(())
    }
    fn clear_wardrobe(&mut self) {
        self.ed.worn.clear();
        self.ed.dirty = true;
        self.ed.clothing_dirty = true;
    }
    fn export_options(&self) -> ExportOptions {
        ExportOptions {
            skin_color: self.ed.skin,
            expression: self.ed.expression.clone(),
            ..self.ed.export.clone()
        }
    }
    fn export_glb(&mut self, path: &Path, options: &ExportOptions) -> Result<Value, String> {
        let (accessories, worn, hidden) = self.ed.clothing_export();
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let st = oxy_io::export::character_glb(
            &self.ed.asset,
            &self.ed.params,
            &accessories,
            &self.ed.material,
            options,
            &worn,
            &hidden,
            BufWriter::new(f),
        )
        .map_err(|e| e.to_string())?;
        Ok(
            json!({ "path": path, "meshes": st.meshes, "bones": st.bones, "blendshapes": st.blendshapes, "textures": st.textures }),
        )
    }
    fn save_project(&mut self, path: &Path) -> Result<PathBuf, String> {
        if self.ed.save_project(path) {
            Ok(self
                .ed
                .session()
                .path
                .clone()
                .unwrap_or_else(|| path.to_owned()))
        } else {
            Err(self.ed.status.clone())
        }
    }
    fn document(&self) -> Value {
        json!({ "path": self.ed.session.as_ref().and_then(|s| s.path.clone()), "unsaved": self.unsaved })
    }
    fn notify(&mut self, text: &str) {
        self.ed.status = text.to_owned();
        self.notes.push(text.to_owned());
    }
}

/// Start, pump and finish plugins.
pub fn plugin_system(mut ui: ResMut<UiState>, mut editor: ResMut<Editor>) {
    let user_dir = editor.session.as_ref().map(|s| s.user_dir.clone());
    let st = &mut ui.plugins;
    if !st.scanned {
        if let Some(d) = &user_dir {
            rescan(st, d);
        }
    }
    if let Some(path) = st.request.take() {
        if st.running.is_none() {
            let info = oxy_plugin::runner::read_info(&path).unwrap_or(PluginInfo {
                path: path.clone(),
                name: path.display().to_string(),
                description: String::new(),
            });
            match spawn(&path, sdk_dir().as_deref(), &[]) {
                Ok(child) => {
                    st.push_log(format!("▶ {} ({})", info.name, path.display()));
                    editor.status = format!("Running plugin {}…", info.name);
                    editor.batch_label = Some(format!("Plugin: {}", info.name));
                    st.running = Some((info, Running::start(child), Instant::now()));
                }
                Err(e) => {
                    let msg = format!(
                        "Could not start {}: {e} (set OXY_PYTHON to your Python)",
                        oxy_plugin::runner::python_command().display()
                    );
                    st.push_log(msg.clone());
                    editor.status = msg;
                }
            }
        }
    }
    let Some((info, run, started)) = st.running.as_mut() else {
        return;
    };
    if std::mem::take(&mut st.stop) {
        run.stop();
    }
    let unsaved = editor.has_unsaved_changes();
    let mut host = EditorHost {
        ed: &mut editor,
        unsaved,
        notes: Vec::new(),
    };
    let (lines, finished) = run.pump(&mut host, CALLS_PER_FRAME);
    let (name, calls, elapsed) = (info.name.clone(), run.calls, started.elapsed());
    for l in lines {
        st.push_log(l);
    }
    if let Some(status) = finished {
        let ok = status.success();
        st.push_log(format!(
            "■ {name}: {} ({calls} calls, {elapsed:.1?})",
            if ok {
                "finished".to_string()
            } else {
                format!("failed, {status}")
            }
        ));
        st.running = None;
        // Commit the whole run as one undo step.
        let label = editor
            .batch_label
            .take()
            .unwrap_or_else(|| format!("Plugin: {name}"));
        editor.commit_history_labelled(&label);
        if !ok {
            editor.status = format!("Plugin {name} failed — see Plugins › Show log");
            st.log_open = true;
        } else if !editor.status.starts_with("Running plugin") {
            // keep the plugin's own notify text
        } else {
            editor.status = format!("Plugin {name} finished");
        }
    }
}

/// The Plugins menu.
pub fn menu(ui: &mut egui::Ui, st: &mut PluginState, user_dir: Option<&Path>) {
    egui::containers::menu::MenuButton::new("Plugins").ui(ui, |ui| {
        let running = st.is_running();
        if st.list.is_empty() {
            ui.label(egui::RichText::new("No plugins found").weak());
        }
        for p in st.list.clone() {
            let b = ui.add_enabled(!running, egui::Button::new(&p.name));
            let b = if p.description.is_empty() {
                b
            } else {
                b.on_hover_text(&p.description)
            };
            if b.clicked() {
                st.request = Some(p.path.clone());
            }
        }
        ui.separator();
        if running && ui.button("Stop plugin").clicked() {
            st.stop = true;
        }
        if ui.button("Run script…").clicked() {
            st.log_open = true;
        }
        if ui.button("Show log").clicked() {
            st.log_open = true;
        }
        if let Some(d) = user_dir {
            if ui.button("Rescan plugins").clicked() {
                rescan(st, d);
            }
            ui.label(
                egui::RichText::new(format!("Your plugins: {}", d.join("plugins").display()))
                    .small()
                    .weak(),
            );
        }
    });
}

/// Log window with a path box for running any script.
pub fn window(ctx: &egui::Context, st: &mut PluginState) {
    if !st.log_open {
        return;
    }
    let mut open = true;
    egui::Window::new("Plugins")
        .open(&mut open)
        .default_size([520.0, 320.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Script");
                ui.add(
                    egui::TextEdit::singleline(&mut st.script_input)
                        .hint_text("/path/to/plugin.py")
                        .desired_width(320.0),
                );
                let can = !st.is_running() && !st.script_input.trim().is_empty();
                if ui.add_enabled(can, egui::Button::new("Run")).clicked() {
                    st.request = Some(PathBuf::from(st.script_input.trim()));
                }
                if st.is_running() && ui.button("Stop").clicked() {
                    st.stop = true;
                }
            });
            if let Some((info, run, t)) = &st.running {
                ui.label(
                    egui::RichText::new(format!(
                        "Running {} · {} calls · {:.0?}",
                        info.name,
                        run.calls,
                        t.elapsed()
                    ))
                    .weak(),
                );
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .auto_shrink(false)
                .show(ui, |ui| {
                    for l in &st.log {
                        ui.label(egui::RichText::new(l).monospace().size(11.5));
                    }
                });
        });
    st.log_open = open;
}
