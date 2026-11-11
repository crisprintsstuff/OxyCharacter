//! Run `.rhai` plugins against the open character.
//!
//! The host API is `oxy_script`. This module only copies the editor into that
//! view, runs the script, and writes the view back as one undo step.

use std::path::{Path, PathBuf};

use oxy_fit::builtin_ids;
use oxy_script::{World, WornItem};

use crate::editor::Editor;

/// One script file from the plugins folder.
pub struct PluginFile {
    pub path: PathBuf,
    pub source: String,
    pub actions: Vec<oxy_script::Action>,
}

impl Editor {
    /// Read `dir` for `*.rhai`. Failures are reported in the status line and
    /// skipped; a bad plugin does not stop the others.
    pub fn load_plugins(&mut self, dir: &Path) {
        self.plugin_dir = dir.to_path_buf();
        self.plugins.clear();
        let files = match oxy_script::plugin_files(dir) {
            Ok(files) => files,
            Err(e) => {
                self.status = e.to_string();
                return;
            }
        };
        let mut errors = Vec::new();
        for path in files {
            let source = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) => {
                    errors.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            let mut scratch = self.script_world();
            let name = path.display().to_string();
            match oxy_script::run(&mut scratch, &source, &name) {
                Ok(()) => {
                    let actions = scratch.actions().to_vec();
                    self.plugins.push(PluginFile {
                        path,
                        source,
                        actions,
                    });
                }
                Err(e) => errors.push(e.to_string()),
            }
        }
        if let Some(last) = errors.last() {
            self.status = last.clone();
        }
    }

    /// Character view the script is allowed to edit.
    pub fn script_world(&self) -> World {
        let mut world = World::from_catalog(
            &self.asset.catalog,
            self.asset.mesh.base.vertex_count(),
            self.asset.rig.as_ref().map(|r| r.bones.len()).unwrap_or(0),
        );
        world.load_values(&self.params);
        world.load_pose(oxy_rig::pose::PRESETS[self.pose].name);
        world.set_skin_tone(self.skin);
        world.set_material(self.material.clone());
        world.set_symmetry_flag(self.symmetry);
        world.load_worn(
            self.worn
                .iter()
                .filter_map(|g| {
                    g.builtin.clone().map(|id| WornItem {
                        id,
                        tightness: Some(g.settings.tightness),
                        color: Some(g.color),
                    })
                })
                .collect(),
        );
        world
    }

    /// Write a script result back. Slider mirroring already happened in the
    /// script, so this assignment does not mirror a second time.
    pub fn apply_script_world(&mut self, world: &World) {
        let symmetry = self.symmetry;
        self.symmetry = false;
        for (id, value) in world.values() {
            if self.asset.catalog.param(id).is_some() {
                self.set(id, *value);
            }
        }
        self.symmetry = symmetry;
        if let Some(index) = oxy_rig::pose::PRESETS
            .iter()
            .position(|p| p.name == world.pose())
        {
            self.set_pose(index);
        }
        self.skin = world.skin();
        self.material = world.material().clone();
        self.eyes_dirty = true;
        let builtins = builtin_ids();
        self.worn.retain(|w| {
            w.builtin
                .as_ref()
                .is_none_or(|id| !builtins.contains(&id.as_str()))
        });
        for item in world.worn() {
            if self.wear_builtin(&item.id) {
                if let Some(worn) = self
                    .worn
                    .iter_mut()
                    .rev()
                    .find(|w| w.builtin.as_deref() == Some(item.id.as_str()))
                {
                    if let Some(t) = item.tightness {
                        worn.settings.tightness = t;
                    }
                    if let Some(color) = item.color {
                        worn.color = color;
                    }
                }
            }
        }
        self.clothing_dirty = true;
        self.dirty = true;
        self.commit_history();
    }

    /// Run a script. `call` invokes that function after the file loads.
    pub fn run_script(
        &mut self,
        source: &str,
        filename: &str,
        call: Option<&str>,
    ) -> Result<(), String> {
        let mut world = self.script_world();
        if let Some(name) = call {
            oxy_script::call(&mut world, source, filename, name).map_err(|e| e.to_string())?;
        } else {
            oxy_script::run(&mut world, source, filename).map_err(|e| e.to_string())?;
        }
        self.apply_script_world(&world);
        self.status = match call {
            Some(name) => format!("Ran {name} from {filename}"),
            None => format!("Ran {filename}"),
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use oxy_io::glam::Vec3;
    use oxy_io::{AssetMeta, CharacterAsset};
    use oxy_mesh::{Mesh, MorphTarget, MorphableMesh};
    use oxy_morph::{Catalog, ParamDef, Slider};

    use super::Editor;

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

    #[test]
    fn a_script_is_one_undo_step() {
        let dir = std::env::temp_dir().join(format!("oxy-script-ed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut ed = editor(&dir);
        ed.run_script(r#"set("nose", 0.4); set_pose("T-pose");"#, "t.rhai", None)
            .unwrap();
        assert!((ed.value("nose") - 0.4).abs() < 1e-5);
        assert_eq!(oxy_rig::pose::PRESETS[ed.pose].name, "T-pose");
        assert!(ed.session().history.undo_label().is_some());
        ed.undo();
        assert!(ed.value("nose").abs() < 1e-5);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
