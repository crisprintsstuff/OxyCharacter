//! OxyBrush bridge in the editor: send the character to OxyBrush, watch the
//! file for saves, and turn what comes back into custom morphs or accessories.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy::window::FileDragAndDrop;
use oxy_io::oxybrush::{self, Accessory, CustomMorph, Imported};

use crate::editor::Editor;

pub use oxy_io::oxybrush::CUSTOM_PREFIX;

/// One piece of an `.oxb` waiting for the user's decision.
pub struct PendingItem {
    pub item: Imported,
    pub include: bool,
    /// Morph label or accessory name.
    pub name: String,
}

pub struct PendingImport {
    pub path: PathBuf,
    pub items: Vec<PendingItem>,
    pub notes: Vec<String>,
}

pub struct AccessoryItem {
    pub acc: Accessory,
    pub visible: bool,
}

pub struct Bridge {
    /// Folder for files sent to OxyBrush.
    pub send_dir: PathBuf,
    /// Folder for saved custom morphs.
    pub morph_dir: PathBuf,
    /// File being watched for OxyBrush saves, and its last seen modification time.
    pub watched: Option<(PathBuf, Option<SystemTime>)>,
    pub pending: Option<PendingImport>,
    pub open_dialog: bool,
    pub path_input: String,
    pub launch: bool,
    since_check: f32,
}

impl Bridge {
    pub fn new(user_dir: &Path) -> Self {
        Self {
            send_dir: user_dir.join("oxybrush"),
            morph_dir: user_dir.join("morphs"),
            watched: None,
            pending: None,
            open_dialog: false,
            path_input: String::new(),
            launch: true,
            since_check: 0.0,
        }
    }
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn stamp() -> String {
    let s = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("{s}")
}

impl Editor {
    pub fn rebuild_categories(&mut self) {
        self.categories = crate::editor::categories_of(&self.asset.catalog);
    }

    /// Add (or replace) a custom morph slider backed by `m`.
    pub fn register_custom_morph(&mut self, m: &CustomMorph) {
        let id = self.asset.add_custom_morph(m);
        self.params.remove(&id);
        self.rebuild_categories();
        self.dirty = true;
    }

    fn remove_custom_param(&mut self, id: &str) {
        self.asset.remove_param(id);
        self.params.remove(id);
    }

    /// Delete a custom morph slider and its file.
    pub fn delete_custom_morph(&mut self, id: &str) {
        if let Some(stem) = id.strip_prefix(CUSTOM_PREFIX) {
            let _ = std::fs::remove_file(self.bridge.morph_dir.join(format!("{stem}.target")));
        }
        self.remove_custom_param(id);
        self.rebuild_categories();
        if self.selected.0 == "Custom"
            && self.modified_count("Custom", None) == 0
            && !self.categories.iter().any(|c| c.top == "Custom")
        {
            self.selected = (
                self.categories[0].top.clone(),
                self.categories[0].subs[0].clone(),
            );
        }
        self.dirty = true;
        self.status = format!("Deleted morph {id}");
    }

    pub fn load_custom_morphs(&mut self) {
        let n = self.asset.mesh.base.vertex_count();
        let (morphs, errors) = oxybrush::load_custom_morphs(&self.bridge.morph_dir, n);
        for m in &morphs {
            self.register_custom_morph(m);
        }
        for e in errors {
            eprintln!("skipping custom morph {e}");
        }
    }

    /// Write the current character to a new `.oxb`, start watching it and
    /// (optionally) open it in OxyBrush.
    pub fn send_to_oxybrush(&mut self) {
        let (body, ground_offset) = self.asset.evaluate_grounded(&self.params);
        let path = self
            .bridge
            .send_dir
            .join(format!("character-{}.oxb", stamp()));
        let link = oxybrush::Link {
            params: self.params.clone(),
            ground_offset,
            vertex_count: body.len(),
        };
        let accs: Vec<Accessory> = self.accessories.iter().map(|a| a.acc.clone()).collect();
        if let Err(e) = oxybrush::send(&path, &body, &self.asset.triangles(), &accs, &link) {
            self.status = format!("Could not write {}: {e}", path.display());
            return;
        }
        self.bridge.watched = Some((path.clone(), mtime(&path)));
        let launched = self.bridge.launch && launch_oxybrush(&path);
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        self.status = if launched {
            format!("Opened {name} in OxyBrush. Save there (Ctrl+S) to bring changes back.")
        } else {
            format!(
                "Saved {name} to {}. Open it in OxyBrush and save; changes come back here.",
                self.bridge.send_dir.display()
            )
        };
    }

    /// Read an `.oxb` and prepare the import dialog.
    pub fn open_oxb(&mut self, path: &Path) {
        let scene = match oxy_io::oxb::load(path) {
            Ok(s) => s,
            Err(e) => {
                self.status = format!("Could not open {}: {e}", path.display());
                return;
            }
        };
        let mut notes = Vec::new();
        let link = oxybrush::read_link(path)
            .filter(|l| l.vertex_count == self.asset.mesh.base.vertex_count());
        let params = match &link {
            Some(l) => l.params.clone(),
            None => {
                notes.push(
                    "No link file: morphs are measured against the character as it is now.".into(),
                );
                self.params.clone()
            }
        };
        let (reference, _) = self.asset.evaluate_grounded(&params);
        let items = oxybrush::classify(&scene, &self.asset.triangles(), &reference);
        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let mut pending = Vec::new();
        for item in items {
            let (include, name) = match &item {
                Imported::Morph {
                    piece,
                    used_base_level,
                    ..
                } => {
                    if *used_base_level {
                        notes.push(format!("“{piece}” has subdivision levels: its level 0 becomes the morph (finer detail stays in OxyBrush)."));
                    }
                    let label = if piece == oxybrush::BODY_PIECE {
                        format!("Sculpt {stem}")
                    } else {
                        piece.clone()
                    };
                    (true, label)
                }
                Imported::Unchanged { .. } => continue,
                Imported::Accessory(a) => {
                    if a.name == oxybrush::BODY_PIECE {
                        notes.push(format!(
                            "“{}” no longer matches the body's topology (subdivided or remeshed?), so it can only come in as a separate mesh. Use subdivision levels in OxyBrush to keep it a morph.",
                            a.name
                        ));
                    }
                    if self.accessories.iter().any(|x| x.acc.name == a.name) {
                        notes.push(format!(
                            "“{}” replaces the accessory of the same name.",
                            a.name
                        ));
                    }
                    (a.name != oxybrush::BODY_PIECE, a.name.clone())
                }
            };
            pending.push(PendingItem {
                item,
                include,
                name,
            });
        }
        if pending.is_empty() {
            self.status = format!("{}: nothing changed", path.display());
            return;
        }
        self.bridge.pending = Some(PendingImport {
            path: path.to_owned(),
            items: pending,
            notes,
        });
    }

    /// Apply the pending import: save + register morphs, add accessories.
    pub fn apply_pending(&mut self) {
        let Some(p) = self.bridge.pending.take() else {
            return;
        };
        let (mut morphs, mut accs) = (0, 0);
        for it in p.items.into_iter().filter(|i| i.include) {
            match it.item {
                Imported::Morph {
                    indices, deltas, ..
                } => {
                    let mut id = oxybrush::slug(&it.name);
                    let base = id.clone();
                    let mut n = 2;
                    while self
                        .asset
                        .catalog
                        .param(&format!("{CUSTOM_PREFIX}{id}"))
                        .is_some()
                    {
                        id = format!("{base}-{n}");
                        n += 1;
                    }
                    let m = CustomMorph {
                        id,
                        label: it.name,
                        indices,
                        deltas,
                    };
                    if let Err(e) = oxybrush::save_custom_morph(&self.bridge.morph_dir, &m) {
                        self.status = format!("Could not save morph: {e}");
                    }
                    self.register_custom_morph(&m);
                    self.set(&format!("{CUSTOM_PREFIX}{}", m.id), 1.0);
                    self.selected = ("Custom".into(), "Sculpted".into());
                    morphs += 1;
                }
                Imported::Accessory(mut a) => {
                    a.name = it.name;
                    self.accessories.retain(|x| x.acc.name != a.name);
                    self.accessories.push(AccessoryItem {
                        acc: a,
                        visible: true,
                    });
                    accs += 1;
                }
                Imported::Unchanged { .. } => {}
            }
        }
        self.accessories_dirty = true;
        let name = p.path.file_name().unwrap_or_default().to_string_lossy();
        self.status =
            format!("Imported {morphs} morph(s) and {accs} accessory mesh(es) from {name}");
    }
}

/// Start OxyBrush on `path`: `$OXYBRUSH_BIN`, else `oxybrush` on PATH.
fn launch_oxybrush(path: &Path) -> bool {
    let bin = std::env::var_os("OXYBRUSH_BIN").unwrap_or_else(|| "oxybrush".into());
    std::process::Command::new(bin).arg(path).spawn().is_ok()
}

/// Poll the watched file once a second; a newer save opens the import dialog.
pub fn watch(time: Res<Time>, mut editor: ResMut<Editor>) {
    let ed = &mut *editor;
    ed.bridge.since_check += time.delta_secs();
    if ed.bridge.since_check < 1.0 || ed.bridge.pending.is_some() {
        return;
    }
    ed.bridge.since_check = 0.0;
    let Some((path, seen)) = ed.bridge.watched.clone() else {
        return;
    };
    let now = mtime(&path);
    if now.is_some() && now != seen {
        ed.bridge.watched = Some((path.clone(), now));
        ed.open_oxb(&path);
    }
}

/// Dropping an `.oxb` on the window imports it.
pub fn file_drop(mut drops: MessageReader<FileDragAndDrop>, mut editor: ResMut<Editor>) {
    for d in drops.read() {
        if let FileDragAndDrop::DroppedFile { path_buf, .. } = d {
            if path_buf
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("oxb"))
            {
                editor.open_oxb(path_buf);
            } else {
                editor.status = format!(
                    "{}: only OxyBrush .oxb files can be dropped here",
                    path_buf.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxy_io::glam::Vec3;
    use oxy_io::{AssetMeta, CharacterAsset};
    use oxy_mesh::{Mesh, MorphableMesh};

    fn tetra_editor(dir: &Path) -> Editor {
        let base = Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
            vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
        )
        .unwrap();
        let asset = CharacterAsset {
            meta: AssetMeta::default(),
            mesh: MorphableMesh {
                base,
                targets: vec![],
                aux: vec![],
            },
            catalog: oxy_morph::Catalog::default(),
            rig: None,
        };
        let mut ed = Editor::new(asset, dir);
        ed.bridge.launch = false;
        ed
    }

    #[test]
    fn send_sculpt_import_apply_and_delete() {
        let dir = std::env::temp_dir().join(format!("oxy-app-bridge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut ed = tetra_editor(&dir);
        ed.send_to_oxybrush();
        let (sent, _) = ed.bridge.watched.clone().expect("watching the sent file");

        // "Sculpt" in another tool: move the apex up, reorder vertices, add a piece.
        let mut scene = oxy_io::oxb::load(&sent).unwrap();
        let body = &mut scene.pieces[0];
        let apex = body.positions.iter().position(|p| p.z > 0.5).unwrap();
        body.positions[apex].z += 0.25;
        body.positions.swap(0, 3);
        for t in &mut body.triangles {
            for i in t.iter_mut() {
                *i = match *i {
                    0 => 3,
                    3 => 0,
                    x => x,
                };
            }
        }
        scene.pieces.push(oxy_io::oxb::Piece::new(
            "Hat",
            vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            vec![[0, 1, 2]],
        ));
        oxy_io::oxb::save(&scene, &sent).unwrap();

        ed.open_oxb(&sent);
        let pending = ed.bridge.pending.as_ref().expect("import dialog");
        assert_eq!(pending.items.len(), 2);
        ed.apply_pending();

        let id = ed
            .asset
            .catalog
            .params
            .iter()
            .find(|p| p.id.starts_with(CUSTOM_PREFIX))
            .unwrap()
            .id
            .clone();
        assert_eq!(ed.value(&id), 1.0);
        assert_eq!(ed.accessories.len(), 1);
        let (pos, _) = ed.asset.evaluate_grounded(&ed.params);
        assert!(
            pos.iter().any(|p| (p.z - 1.25).abs() < 1e-5),
            "sculpt applied"
        );
        assert!(ed.categories.iter().any(|c| c.top == "Custom"));

        // Morph survives a restart, and deleting removes the file.
        let mut again = tetra_editor(&dir);
        again.load_custom_morphs();
        assert!(again.asset.catalog.param(&id).is_some());
        again.delete_custom_morph(&id);
        let mut third = tetra_editor(&dir);
        third.load_custom_morphs();
        assert!(third.asset.catalog.param(&id).is_none());
        std::fs::remove_dir_all(dir).ok();
    }
}
