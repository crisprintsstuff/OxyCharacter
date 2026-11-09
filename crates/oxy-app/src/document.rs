//! The document: what gets saved, undone and autosaved.
//!
//! A [`DocState`] snapshot holds slider values, skin, pose, sculpted morphs
//! and accessories. Large parts sit behind an `Arc` and are only rebuilt when
//! a cheap *content key* changes, so taking a snapshot every frame is cheap.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::prelude::*;
use oxy_io::oxybrush::{CustomMorph, CUSTOM_PREFIX};
use oxy_morph::Params;
use oxy_project::{AccessoryEntry, BaseRef, History, Project, WardrobeEntry};

use crate::bridge::AccessoryItem;
use crate::editor::Editor;

#[derive(Debug, Default, PartialEq)]
pub struct Content {
    pub custom: Vec<CustomMorph>,
    pub accessories: Vec<AccessoryEntry>,
    pub wardrobe: Vec<WardrobeEntry>,
}

#[derive(Debug, Clone)]
pub struct DocState {
    pub params: Params,
    pub skin: [f32; 3],
    pub pose: usize,
    pub export: oxy_io::export::ExportOptions,
    pub material: oxy_io::material::MaterialParams,
    key: Vec<String>,
    pub content: Arc<Content>,
}

impl PartialEq for DocState {
    fn eq(&self, o: &Self) -> bool {
        self.params == o.params
            && self.skin == o.skin
            && self.pose == o.pose
            && self.export == o.export
            && self.material == o.material
            && self.key == o.key
    }
}

/// What to do once unsaved changes have been dealt with.
#[derive(Debug, Clone, PartialEq)]
pub enum After {
    New,
    Open(PathBuf),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathDialog {
    Open,
    SaveAs,
    Export,
    ImportOxb,
}

pub struct Session {
    pub history: History<DocState>,
    /// State at the last save/open (to know if there are unsaved changes).
    pub saved: DocState,
    pub path: Option<PathBuf>,
    pub recent: Vec<PathBuf>,
    pub user_dir: PathBuf,
    pub dialog: Option<PathDialog>,
    /// Menu, shortcut, or "save before closing" asked for a path. The UI
    /// turns this into a native dialog or the typed fallback.
    pub dialog_request: Option<PathDialog>,
    /// Scripted `--dialog` stays on the typed dialog so a screenshot run
    /// never opens a portal window.
    pub force_typed: bool,
    pub path_input: String,
    /// Unsaved-changes prompt.
    pub confirm: Option<After>,
    /// Action to run after a Save As started from the prompt succeeds.
    pub after_save: Option<After>,
    /// Autosave found at start-up, offered for recovery.
    pub recover: Option<PathBuf>,
    /// Problems found while opening, shown once.
    pub notes: Vec<String>,
    pub autosave_every: f32,
    since_autosave: f32,
    autosaved: Option<DocState>,
    cache: Option<(Vec<String>, Arc<Content>)>,
    pub quit: bool,
}

impl Session {
    pub fn autosave_path(&self) -> PathBuf {
        self.user_dir.join("autosave.oxyc")
    }
    fn recent_path(&self) -> PathBuf {
        self.user_dir.join("recent.json")
    }
    pub fn title(&self, dirty: bool) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or("Untitled".into(), |n| n.to_string_lossy().into_owned());
        format!("OxyCharacter — {name}{}", if dirty { " *" } else { "" })
    }
}

fn file_name(p: &Path) -> String {
    p.file_name().map_or_else(
        || p.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

fn content_key(ed: &Editor) -> Vec<String> {
    let mut k: Vec<String> = ed
        .asset
        .catalog
        .params
        .iter()
        .filter(|p| p.id.starts_with(CUSTOM_PREFIX))
        .map(|p| {
            let n = ed
                .asset
                .mesh
                .target_index(&p.id)
                .map_or(0, |i| ed.asset.mesh.targets[i].indices.len());
            format!("m:{}:{}:{n}", p.id, p.label)
        })
        .collect();
    for a in &ed.accessories {
        let first = a
            .acc
            .positions
            .first()
            .map_or([0; 3], |p| p.to_array().map(f32::to_bits));
        k.push(format!(
            "a:{}:{}:{}:{}:{first:?}:{:?}:{}",
            a.acc.name,
            a.visible,
            a.acc.positions.len(),
            a.acc.triangles.len(),
            a.settings.method,
            a.settings.tightness
        ));
    }
    for w in &ed.worn {
        k.push(format!(
            "w:{}:{}:{:?}:{}:{}:{:?}:{}",
            w.id,
            w.settings.visible,
            w.settings.method,
            w.settings.tightness,
            w.settings.hide_body,
            w.color.map(f32::to_bits),
            w.mesh.positions.len()
        ));
    }
    k
}

impl Editor {
    pub fn session(&self) -> &Session {
        self.session.as_ref().expect("session initialised")
    }
    fn session_mut(&mut self) -> &mut Session {
        self.session.as_mut().expect("session initialised")
    }

    /// Create the session (history, recent files, autosave check).
    pub fn init_session(&mut self, user_dir: &Path) {
        self.session = None;
        let mut s = Session {
            history: History::new(self.snapshot_uncached(), 200),
            saved: self.snapshot_uncached(),
            path: None,
            recent: Vec::new(),
            user_dir: user_dir.to_owned(),
            dialog: None,
            dialog_request: None,
            force_typed: false,
            path_input: String::new(),
            confirm: None,
            after_save: None,
            recover: None,
            notes: Vec::new(),
            autosave_every: 60.0,
            since_autosave: 0.0,
            autosaved: None,
            cache: None,
            quit: false,
        };
        s.recent = std::fs::read(s.recent_path())
            .ok()
            .and_then(|b| serde_json::from_slice::<Vec<PathBuf>>(&b).ok())
            .unwrap_or_default();
        if s.autosave_path().exists() {
            s.recover = Some(s.autosave_path());
        }
        self.session = Some(s);
    }

    fn custom_morphs_now(&self) -> Vec<CustomMorph> {
        self.asset
            .catalog
            .params
            .iter()
            .filter_map(|p| {
                let stem = p.id.strip_prefix(CUSTOM_PREFIX)?;
                let t = &self.asset.mesh.targets[self.asset.mesh.target_index(&p.id)?];
                Some(CustomMorph {
                    id: stem.into(),
                    label: p.label.clone(),
                    indices: t.indices.clone(),
                    deltas: t.deltas.clone(),
                })
            })
            .collect()
    }

    fn snapshot_uncached(&self) -> DocState {
        DocState {
            params: self.params.clone(),
            skin: self.skin,
            pose: self.pose,
            export: self.export.clone(),
            material: self.material.clone(),
            key: content_key(self),
            content: Arc::new(Content {
                custom: self.custom_morphs_now(),
                accessories: self
                    .accessories
                    .iter()
                    .map(|a| AccessoryEntry {
                        mesh: a.acc.clone(),
                        visible: a.visible,
                        settings: a.settings.clone(),
                        binding: a.binding.clone(),
                    })
                    .collect(),
                wardrobe: self.worn.iter().map(|w| w.to_entry()).collect(),
            }),
        }
    }

    /// Current document state (cheap unless sculpts/accessories changed).
    pub fn snapshot(&mut self) -> DocState {
        let key = content_key(self);
        let cached = self
            .session
            .as_ref()
            .and_then(|s| s.cache.as_ref())
            .filter(|(k, _)| *k == key)
            .map(|(_, c)| c.clone());
        let content = match cached {
            Some(c) => c,
            None => {
                let c = self.snapshot_uncached().content;
                if let Some(s) = self.session.as_mut() {
                    s.cache = Some((key.clone(), c.clone()));
                }
                c
            }
        };
        DocState {
            params: self.params.clone(),
            skin: self.skin,
            pose: self.pose,
            export: self.export.clone(),
            material: self.material.clone(),
            key,
            content,
        }
    }

    pub fn has_unsaved_changes(&mut self) -> bool {
        let now = self.snapshot();
        now != self.session().saved
    }

    /// Put the editor into `state` (undo/redo, open, recover).
    pub fn restore(&mut self, state: &DocState) {
        self.params = state.params.clone();
        self.skin = state.skin;
        self.pose = state.pose.min(oxy_rig::pose::PRESETS.len() - 1);
        self.export = state.export.clone();
        self.material = state.material.clone();
        if content_key(self) != state.key {
            // Sculpted morphs: replace the registered set.
            let ids: Vec<String> = self
                .asset
                .catalog
                .params
                .iter()
                .filter(|p| p.id.starts_with(CUSTOM_PREFIX))
                .map(|p| p.id.clone())
                .collect();
            for id in ids {
                self.asset.remove_param(&id);
            }
            for m in &state.content.custom {
                self.asset.add_custom_morph(m);
            }
            self.rebuild_categories();
            if !self
                .categories
                .iter()
                .any(|c| c.top == self.selected.0 && c.subs.contains(&self.selected.1))
            {
                self.selected = (
                    self.categories[0].top.clone(),
                    self.categories[0].subs[0].clone(),
                );
            }
            self.accessories = state
                .content
                .accessories
                .iter()
                .map(|e| AccessoryItem {
                    acc: e.mesh.clone(),
                    visible: e.visible,
                    influences: None,
                    settings: e.settings.clone(),
                    binding: e.binding.clone(),
                    fitted: None,
                    hidden_body: Vec::new(),
                })
                .collect();
            self.accessories_dirty = true;
            let wardrobe = state.content.wardrobe.clone();
            let worn = wardrobe
                .iter()
                .filter_map(|e| self.worn_from_entry(e))
                .collect();
            self.worn = worn;
            self.clothing_dirty = true;
            self.body_hidden.clear();
            if let Some(s) = self.session.as_mut() {
                s.cache = Some((state.key.clone(), state.content.clone()));
            }
        }
        // Keep parameters only for sliders that exist.
        let defaults = self.asset.catalog.defaults();
        self.params.retain(|k, _| defaults.contains_key(k));
        for (k, v) in defaults {
            self.params.entry(k).or_insert(v);
        }
        self.dirty = true;
        self.highlight_dirty = true;
    }

    /// Record finished edits as one undo step (call when no drag is in progress).
    pub fn commit_history(&mut self) {
        let snap = self.snapshot();
        let prev = self.session().history.current().clone();
        if snap == prev {
            return;
        }
        let label = self.describe_change(&prev, &snap);
        self.session_mut().history.commit(snap, label);
    }

    fn describe_change(&self, a: &DocState, b: &DocState) -> String {
        if a.key != b.key {
            let (ma, mb) = (a.content.custom.len(), b.content.custom.len());
            let (aa, ab) = (a.content.accessories.len(), b.content.accessories.len());
            let (wa, wb) = (a.content.wardrobe.len(), b.content.wardrobe.len());
            return match (mb.cmp(&ma), ab.cmp(&aa), wb.cmp(&wa)) {
                (std::cmp::Ordering::Greater, _, _) => "Add sculpted morph".into(),
                (std::cmp::Ordering::Less, _, _) => "Delete sculpted morph".into(),
                (_, _, std::cmp::Ordering::Greater) => b
                    .content
                    .wardrobe
                    .last()
                    .map(|w| format!("Wear {}", w.name))
                    .unwrap_or_else(|| "Wear clothing".into()),
                (_, _, std::cmp::Ordering::Less) => "Remove clothing".into(),
                (_, std::cmp::Ordering::Greater, _) => "Add accessory".into(),
                (_, std::cmp::Ordering::Less, _) => "Remove accessory".into(),
                _ if a.content.wardrobe != b.content.wardrobe => "Clothing fit".into(),
                _ => "Change accessories".into(),
            };
        }
        let changed: Vec<&String> = b
            .params
            .iter()
            .filter(|(k, v)| a.params.get(*k) != Some(v))
            .map(|(k, _)| k)
            .collect();
        match changed.as_slice() {
            [] if a.skin != b.skin => "Skin tone".into(),
            [] if a.pose != b.pose => format!("Pose: {}", oxy_rig::pose::PRESETS[b.pose].name),
            [] if a.material != b.material => "Material".into(),
            [] => "Export options".into(),
            [one] => self
                .asset
                .catalog
                .param(one)
                .map_or_else(|| (*one).clone(), |p| p.label.clone()),
            many => format!("{} sliders", many.len()),
        }
    }

    pub fn undo(&mut self) {
        let Some((state, label)) = self.session_mut().history.undo() else {
            return;
        };
        self.restore(&state);
        self.status = format!("Undo: {label}");
    }

    pub fn redo(&mut self) {
        let Some((state, label)) = self.session_mut().history.redo() else {
            return;
        };
        self.restore(&state);
        self.status = format!("Redo: {label}");
    }

    pub fn build_project(&mut self) -> Project {
        let snap = self.snapshot();
        let mut p = Project::new(BaseRef::of(&self.asset));
        p.set_params(&self.asset, &snap.params);
        p.skin = snap.skin;
        p.pose = oxy_rig::pose::PRESETS[snap.pose].name.into();
        p.export = snap.export.clone();
        p.material = snap.material.clone();
        // Embed sculpted morphs that are in use.
        p.custom_morphs = snap
            .content
            .custom
            .iter()
            .filter(|m| {
                snap.params
                    .get(&format!("{CUSTOM_PREFIX}{}", m.id))
                    .is_some_and(|v| v.abs() > 1e-6)
            })
            .cloned()
            .collect();
        p.accessories = snap.content.accessories.clone();
        // Masks are derived. Take them from the fitted garments, which the
        // snapshot's cached content may not have refreshed.
        p.wardrobe = self.worn.iter().map(|w| w.to_entry()).collect();
        p
    }

    /// Save to `path` (atomically); updates title, recent files and the clean state.
    pub fn save_project(&mut self, path: &Path) -> bool {
        let path = if path.extension().is_none() {
            path.with_extension(oxy_project::EXTENSION)
        } else {
            path.to_owned()
        };
        match self.build_project().save(&path) {
            Ok(()) => {
                let snap = self.snapshot();
                let s = self.session_mut();
                s.saved = snap;
                s.path = Some(path.clone());
                let _ = std::fs::remove_file(s.autosave_path());
                s.autosaved = None;
                self.add_recent(&path);
                self.status = format!("Saved {}", file_name(&path));
                true
            }
            Err(e) => {
                self.status = format!("Could not save {}: {e}", path.display());
                false
            }
        }
    }

    /// Open a project file, replacing the current document.
    pub fn open_project(&mut self, path: &Path) -> bool {
        let mut project = match Project::load(path) {
            Ok(p) => p,
            Err(e) => {
                self.status = format!("Could not open {}: {e}", path.display());
                return false;
            }
        };
        let mut notes = project.reconcile(&self.asset);
        notes.extend(self.apply_project(&project));
        let snap = self.snapshot();
        let s = self.session_mut();
        s.history.reset(snap.clone());
        s.saved = snap;
        s.path = Some(path.to_owned());
        s.notes = notes;
        self.add_recent(path);
        self.status = format!("Opened {}", file_name(path));
        true
    }

    fn apply_project(&mut self, project: &Project) -> Vec<String> {
        let mut params = self.asset.catalog.defaults();
        let custom_ids: Vec<String> = project
            .custom_morphs
            .iter()
            .map(|m| format!("{CUSTOM_PREFIX}{}", m.id))
            .collect();
        // Sculpted morphs from the library stay available; the project's own replace same-named ones.
        let mut custom = self.custom_morphs_now();
        custom.retain(|m| !custom_ids.contains(&format!("{CUSTOM_PREFIX}{}", m.id)));
        custom.extend(project.custom_morphs.iter().cloned());
        for m in &custom {
            params.insert(format!("{CUSTOM_PREFIX}{}", m.id), 0.0);
        }
        params.extend(project.params.iter().map(|(k, v)| (k.clone(), *v)));
        let state = DocState {
            params,
            skin: project.skin,
            pose: oxy_rig::pose::PRESETS
                .iter()
                .position(|p| p.name == project.pose)
                .unwrap_or(0),
            export: project.export.clone(),
            material: project.material.clone(),
            key: vec!["<forced>".into()],
            content: Arc::new(Content {
                custom,
                accessories: project.accessories.clone(),
                wardrobe: project.wardrobe.clone(),
            }),
        };
        self.restore(&state);
        let mut missed = Vec::new();
        if self.worn.len() != project.wardrobe.len() {
            missed.push(format!(
                "{} clothing item(s) could not be rebuilt for this body and were skipped.",
                project.wardrobe.len() - self.worn.len()
            ));
        }
        // `restore` keyed the cache with the placeholder; recompute it.
        if let Some(s) = self.session.as_mut() {
            s.cache = None;
        }
        missed
    }

    /// Start a new, default character.
    pub fn new_document(&mut self) {
        let fresh = DocState {
            params: self.asset.catalog.defaults(),
            skin: crate::editor::SKIN_TONES[2].1,
            pose: 0,
            export: oxy_io::export::ExportOptions::default(),
            material: Default::default(),
            key: vec!["<forced>".into()],
            content: Arc::new(Content {
                custom: self.custom_morphs_now(),
                accessories: Vec::new(),
                wardrobe: Vec::new(),
            }),
        };
        self.restore(&fresh);
        if let Some(s) = self.session.as_mut() {
            s.cache = None;
        }
        let snap = self.snapshot();
        let s = self.session_mut();
        s.history.reset(snap.clone());
        s.saved = snap;
        s.path = None;
        self.status = "New character".into();
    }

    fn add_recent(&mut self, path: &Path) {
        let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
        let s = self.session_mut();
        s.recent.retain(|p| *p != abs);
        s.recent.insert(0, abs);
        s.recent.truncate(8);
        if let Ok(json) = serde_json::to_vec_pretty(&s.recent) {
            let _ = oxy_io::oxb::write_atomic(&s.recent_path(), &json);
        }
    }

    /// Run `after`, asking about unsaved changes first.
    pub fn request(&mut self, after: After) {
        if self.has_unsaved_changes() {
            self.session_mut().confirm = Some(after);
        } else {
            self.perform(after);
        }
    }

    pub fn perform(&mut self, after: After) {
        match after {
            After::New => self.new_document(),
            After::Open(p) => {
                self.open_project(&p);
            }
            After::Quit => {
                let s = self.session_mut();
                let _ = std::fs::remove_file(s.autosave_path());
                s.quit = true;
            }
        }
    }

    /// Answer the unsaved-changes prompt.
    pub fn resolve_confirm(&mut self, save: Option<bool>) {
        let Some(after) = self.session_mut().confirm.take() else {
            return;
        };
        match save {
            None => {} // cancel
            Some(false) => self.perform(after),
            Some(true) => match self.session().path.clone() {
                Some(p) => {
                    if self.save_project(&p) {
                        self.perform(after);
                    }
                }
                None => {
                    let s = self.session_mut();
                    s.after_save = Some(after);
                    s.dialog_request = Some(PathDialog::SaveAs);
                }
            },
        }
    }

    /// Save to the current path, or ask for one.
    pub fn save_or_ask(&mut self) {
        match self.session().path.clone() {
            Some(p) => {
                self.save_project(&p);
            }
            None => self.session_mut().dialog_request = Some(PathDialog::SaveAs),
        }
    }

    /// Recover (or discard) the autosave offered at start-up.
    pub fn resolve_recover(&mut self, recover: bool) {
        let Some(path) = self.session_mut().recover.take() else {
            return;
        };
        if recover {
            if let Ok(mut p) = Project::load(&path) {
                let mut notes = p.reconcile(&self.asset);
                notes.extend(self.apply_project(&p));
                let snap = self.snapshot();
                let s = self.session_mut();
                s.history.reset(snap);
                s.notes = notes;
                // Still unsaved: the saved state stays the empty document.
                self.status = "Recovered unsaved work from the last session".into();
                return;
            }
            self.status = "The autosave could not be read".into();
        }
        let _ = std::fs::remove_file(path);
    }
}

/// Record finished edits, autosave, keep the window title current.
pub fn session_system(
    time: Res<Time>,
    state: Res<crate::ui::UiState>,
    mut editor: ResMut<Editor>,
    mut window: Single<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
) {
    if editor.session().quit {
        exit.write(AppExit::Success);
        return;
    }
    if !state.pointer_busy {
        editor.commit_history();
    }
    let dirty = editor.has_unsaved_changes();
    let title = editor.session().title(dirty);
    if window.title != title {
        window.title = title;
    }
    let dt = time.delta_secs();
    let s = editor.session.as_mut().expect("session");
    s.since_autosave += dt;
    if dirty && s.since_autosave >= s.autosave_every && s.recover.is_none() {
        s.since_autosave = 0.0;
        let snap = editor.snapshot();
        if editor.session().autosaved.as_ref() != Some(&snap) {
            let path = editor.session().autosave_path();
            if editor.build_project().save(&path).is_ok() {
                editor.session_mut().autosaved = Some(snap);
            }
        }
    }
}

/// Closing the window asks about unsaved changes.
pub fn close_requested(
    mut requests: MessageReader<bevy::window::WindowCloseRequested>,
    mut editor: ResMut<Editor>,
) {
    if requests.read().count() > 0 {
        editor.request(After::Quit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxy_io::glam::Vec3;
    use oxy_io::oxybrush::Accessory;
    use oxy_io::{AssetMeta, CharacterAsset};
    use oxy_mesh::{Mesh, MorphTarget, MorphableMesh};
    use oxy_morph::{Catalog, ParamDef, Slider};

    fn editor(dir: &Path) -> Editor {
        let base = Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
            vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
        )
        .unwrap();
        let catalog = Catalog {
            params: vec![ParamDef {
                id: "nose".into(),
                label: "Nose".into(),
                group: vec!["Head".into(), "Nose".into()],
                min: 0.0,
                max: 1.0,
                default: 0.0,
                mirror: None,
                ends: None,
            }],
            sliders: vec![Slider::Unipolar {
                id: "nose".into(),
                target: "nose".into(),
            }],
            layers: vec![],
        };
        let asset = CharacterAsset {
            meta: AssetMeta {
                name: "tetra".into(),
                ..Default::default()
            },
            mesh: MorphableMesh {
                base,
                targets: vec![MorphTarget::new("nose", vec![3], vec![Vec3::Z])],
                aux: vec![],
            },
            catalog,
            rig: None,
            render: None,
        };
        let mut ed = Editor::new(asset, dir);
        ed.bridge.launch = false;
        ed.init_session(dir);
        ed
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("oxy-doc-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn undo_redo_one_step_per_commit() {
        let dir = tmp("undo");
        let mut ed = editor(&dir);
        // A drag sets many values but commits once.
        for v in [0.1, 0.2, 0.5] {
            ed.set("nose", v);
        }
        ed.commit_history();
        ed.set("nose", 0.8);
        ed.commit_history();
        assert_eq!(ed.session().history.undo_label(), Some("Nose"));
        ed.material.skin.freckles = 0.5;
        ed.commit_history();
        assert_eq!(ed.session().history.undo_label(), Some("Material"));
        ed.undo();
        assert_eq!(ed.material.skin.freckles, 0.0);
        ed.undo();
        assert_eq!(ed.value("nose"), 0.5);
        ed.undo();
        assert_eq!(ed.value("nose"), 0.0);
        ed.redo();
        assert_eq!(ed.value("nose"), 0.5);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn save_new_open_round_trip() {
        let dir = tmp("save");
        let mut ed = editor(&dir);
        ed.set("nose", 0.4);
        ed.register_custom_morph(&CustomMorph {
            id: "brow".into(),
            label: "Brow".into(),
            indices: vec![1],
            deltas: vec![Vec3::Y * 0.1],
        });
        ed.set("custom/brow", 1.0);
        ed.accessories.push(AccessoryItem::new(
            Accessory {
                name: "Hat".into(),
                positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
                triangles: vec![[0, 1, 2]],
                color: vec![],
            },
            false,
        ));
        ed.skin = [0.3, 0.2, 0.1];
        ed.set_pose(1);
        ed.export.blendshapes = true;
        ed.material.skin.blush = 0.6;
        ed.material.eyes.iris = [0.1, 0.4, 0.2];
        assert!(ed.has_unsaved_changes());

        let path = dir.join("hero.oxyc");
        assert!(ed.save_project(&path));
        assert!(!ed.has_unsaved_changes());
        assert_eq!(ed.session().title(false), "OxyCharacter — hero.oxyc");

        ed.new_document();
        assert_eq!(ed.value("nose"), 0.0);
        assert!(ed.accessories.is_empty());
        // Delete the sculpt from the session to prove the project carries it.
        ed.delete_custom_morph("custom/brow");
        assert!(ed.asset.catalog.param("custom/brow").is_none());

        assert!(ed.open_project(&path));
        assert_eq!(ed.value("nose"), 0.4);
        assert_eq!(ed.value("custom/brow"), 1.0);
        assert!(ed.asset.catalog.param("custom/brow").is_some());
        assert_eq!(ed.accessories.len(), 1);
        assert!(!ed.accessories[0].visible);
        assert_eq!(ed.skin, [0.3, 0.2, 0.1]);
        assert_eq!(ed.pose, 1);
        assert!(ed.export.blendshapes);
        assert_eq!(ed.material.skin.blush, 0.6);
        assert_eq!(ed.material.eyes.iris, [0.1, 0.4, 0.2]);
        assert!(!ed.has_unsaved_changes(), "freshly opened");
        assert!(!ed.session().history.can_undo());
        assert_eq!(ed.session().recent.len(), 1);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn unsaved_changes_prompt_and_recovery() {
        let dir = tmp("prompt");
        let mut ed = editor(&dir);
        ed.request(After::New);
        assert!(ed.session().confirm.is_none(), "no prompt without changes");
        ed.set("nose", 0.7);
        ed.request(After::New);
        assert_eq!(ed.session().confirm, Some(After::New));
        ed.resolve_confirm(None);
        assert_eq!(ed.value("nose"), 0.7, "cancel keeps the work");
        ed.request(After::Quit);
        ed.resolve_confirm(Some(false));
        assert!(ed.session().quit);

        // An autosave left behind is offered at the next start.
        ed.build_project()
            .save(&ed.session().autosave_path())
            .unwrap();
        let mut next = editor(&dir);
        assert!(next.session().recover.is_some());
        next.resolve_recover(true);
        assert_eq!(next.value("nose"), 0.7);
        assert!(
            next.has_unsaved_changes(),
            "recovered work still needs saving"
        );
        let mut third = editor(&dir);
        third.resolve_recover(false);
        assert!(!third.session().autosave_path().exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn clothing_is_undoable_and_round_trips() {
        let dir = tmp("wear");
        let mut ed = editor(&dir);
        let positions = vec![Vec3::new(0.1, 0.2, 0.05), Vec3::X, Vec3::Y];
        let mesh = oxy_fit::GarmentMesh::from_parts(positions.clone(), vec![[0, 1, 2]], vec![]);
        let binding = oxy_fit::bind(
            &ed.asset.mesh.base.positions,
            &ed.asset.mesh.base.indices,
            &[],
            &mesh.positions,
        );
        ed.worn.push(crate::clothing::WornGarment {
            id: "wrap".into(),
            name: "Wrap".into(),
            kind: oxy_fit::ItemKind::Garment,
            builtin: None,
            color: [0.2, 0.3, 0.4],
            fitted: positions,
            settings: oxy_fit::FitSettings::surface(1),
            skin: vec![],
            hidden_body: vec![0],
            hidden_self: vec![],
            mesh,
            binding,
        });
        ed.commit_history();
        assert_eq!(ed.session().history.undo_label(), Some("Wear Wrap"));
        let path = dir.join("hero.oxyc");
        assert!(ed.save_project(&path));
        let saved = Project::load(&path).unwrap();
        assert_eq!(saved.wardrobe[0].hidden_tris, vec![0]);
        assert!(saved.wardrobe[0].mesh.is_some());
        assert!(saved.wardrobe[0].builtin.is_none());
        ed.undo();
        assert!(ed.worn.is_empty(), "undo removes the garment");
        ed.redo();
        assert_eq!(ed.worn.len(), 1);
        ed.new_document();
        assert!(ed.worn.is_empty());
        assert!(ed.open_project(&path));
        assert_eq!(ed.worn.len(), 1);
        assert_eq!(ed.worn[0].name, "Wrap");
        assert!((ed.worn[0].settings.tightness - 0.5).abs() < 1e-4);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn save_without_a_path_asks_for_one() {
        let dir = tmp("ask");
        let mut ed = editor(&dir);
        ed.set("nose", 0.2);
        ed.save_or_ask();
        assert_eq!(ed.session().dialog_request, Some(PathDialog::SaveAs));
        assert!(ed.session().dialog.is_none());
        ed.session_mut().dialog_request = None;
        ed.request(After::New);
        ed.resolve_confirm(Some(true));
        assert_eq!(ed.session().dialog_request, Some(PathDialog::SaveAs));
        assert_eq!(ed.session().after_save, Some(After::New));
        assert_eq!(
            ed.value("nose"),
            0.2,
            "the character is unchanged until saved"
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
