//! Worn garments and hair: library, refit, project entries, export meshes.

use oxy_fit::{
    author_builtins, bind, builtin_name, fit, occlusion, transfer_weights, AuthoredGarment,
    CoverMesh, FitMethod, FitSettings, GarmentBind, GarmentMesh, ItemKind,
};
use oxy_io::export::WornMesh;
use oxy_io::glam::Vec3;
use oxy_io::oxybrush::Accessory;
use oxy_project::{EmbeddedMesh, WardrobeEntry};
use oxy_rig::Influence;

use crate::bridge::AccessoryItem;
use crate::editor::Editor;

/// One garment or hair piece currently on the character.
pub struct WornGarment {
    pub id: String,
    pub name: String,
    pub kind: ItemKind,
    pub builtin: Option<String>,
    pub color: [f32; 3],
    pub mesh: GarmentMesh,
    pub settings: FitSettings,
    pub binding: GarmentBind,
    pub skin: Vec<Influence>,
    /// Fitted positions in the unposed, floor-standing body space.
    pub fitted: Vec<Vec3>,
    /// Body triangles this item hides.
    pub hidden_body: Vec<u32>,
    /// This item's own triangles hidden by a higher layer.
    pub hidden_self: Vec<u32>,
}

impl WornGarment {
    fn from_authored(src: &AuthoredGarment) -> Self {
        Self {
            id: src.id.to_string(),
            name: src.name.to_string(),
            kind: src.kind,
            builtin: Some(src.id.to_string()),
            color: src.color,
            fitted: src.mesh.positions.clone(),
            mesh: src.mesh.clone(),
            settings: src.settings.clone(),
            binding: src.binding.clone(),
            skin: src.skin.clone(),
            hidden_body: Vec::new(),
            hidden_self: Vec::new(),
        }
    }

    pub fn to_entry(&self) -> WardrobeEntry {
        WardrobeEntry {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.kind,
            builtin: self.builtin.clone(),
            mesh: self.builtin.is_none().then(|| EmbeddedMesh {
                positions: self.mesh.positions.clone(),
                triangles: self.mesh.triangles.clone(),
                uvs: self.mesh.uvs.clone(),
            }),
            settings: self.settings.clone(),
            color: self.color,
            binding: self.builtin.is_none().then(|| self.binding.clone()),
            hidden_tris: self.hidden_body.clone(),
        }
    }
}

struct Prepared {
    positions: Vec<Vec3>,
    indices: Vec<u32>,
    normals: Vec<Vec3>,
    layer: u16,
    hide_body: bool,
    cover: f32,
    /// `None` is an accessory; `Some` is an index into `worn`.
    worn: Option<usize>,
    accessory: Option<usize>,
}

impl Editor {
    pub fn ensure_library(&mut self) {
        if self.library.is_some() {
            return;
        }
        let ev = self.asset.evaluate_full(&self.asset.catalog.defaults());
        let names = self
            .asset
            .rig
            .as_ref()
            .map(|r| r.bones.iter().map(|b| b.name.clone()).collect::<Vec<_>>());
        let skin = self.asset.rig.as_ref().map(|r| r.skin.as_slice());
        self.library = Some(author_builtins(
            &ev.body,
            &self.asset.mesh.base.indices,
            skin,
            names.as_deref(),
        ));
    }

    pub fn library_items(&mut self) -> Vec<(String, String, ItemKind, bool)> {
        self.ensure_library();
        let worn: Vec<String> = self.worn.iter().filter_map(|w| w.builtin.clone()).collect();
        self.library.as_ref().map_or_else(Vec::new, |lib| {
            lib.iter()
                .map(|g| {
                    (
                        g.id.to_string(),
                        g.name.to_string(),
                        g.kind,
                        worn.iter().any(|id| id == g.id),
                    )
                })
                .collect()
        })
    }

    /// Wear a built-in, or take it off if it is already worn. Returns false if
    /// this body has no such item.
    pub fn toggle_builtin(&mut self, id: &str) -> bool {
        if let Some(i) = self
            .worn
            .iter()
            .position(|w| w.builtin.as_deref() == Some(id))
        {
            let name = self.worn[i].name.clone();
            self.worn.remove(i);
            self.status = format!("Removed {name}");
            self.dirty = true;
            self.clothing_dirty = true;
            return true;
        }
        self.wear_builtin(id)
    }

    pub fn wear_builtin(&mut self, id: &str) -> bool {
        if self.worn.iter().any(|w| w.builtin.as_deref() == Some(id)) {
            return true;
        }
        self.ensure_library();
        let Some(src) = self
            .library
            .as_ref()
            .and_then(|l| l.iter().find(|g| g.id == id).cloned())
        else {
            return false;
        };
        let name = src.name;
        self.worn.push(WornGarment::from_authored(&src));
        self.status = format!("Wearing {name}");
        self.dirty = true;
        self.clothing_dirty = true;
        true
    }

    /// `spec` is `id` or `id:tightness` (0–1), comma-separated, as on the CLI.
    pub fn wear_specs(&mut self, spec: &str) {
        for part in spec.split(',').filter(|s| !s.is_empty()) {
            let (id, tight) = part
                .split_once(':')
                .map(|(i, t)| (i, t.parse::<f32>().ok()))
                .unwrap_or((part, None));
            if !self.wear_builtin(id.trim()) && builtin_name(id.trim()).is_none() {
                self.status = format!("Unknown garment \"{}\"", id.trim());
            }
            if let Some(t) = tight {
                if let Some(w) = self
                    .worn
                    .iter_mut()
                    .rev()
                    .find(|w| w.builtin.as_deref() == Some(id.trim()))
                {
                    w.settings.tightness = t.clamp(0.0, 1.0);
                }
            }
        }
    }

    pub fn load_wardrobe_dir(&mut self, dir: &std::path::Path) -> Result<usize, String> {
        let manifest = std::fs::read(dir.join("manifest.json")).map_err(|e| e.to_string())?;
        let manifest = oxy_fit::Manifest::from_json(&manifest)?;
        let body = self
            .asset
            .evaluate_full(&self.asset.catalog.defaults())
            .body;
        let indices = self.asset.mesh.base.indices.clone();
        let mut n = 0;
        for item in manifest.items {
            let path = dir.join(&item.file);
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let mesh = oxy_io::obj::to_garment(&text, &path.display().to_string())
                .map_err(|e| e.to_string())?;
            let binding = bind(&body, &indices, &[], &mesh.positions);
            let skin = self.asset.rig.as_ref().map_or_else(Vec::new, |r| {
                transfer_weights(&r.skin, &binding, &indices, &mesh.neighbors, 6)
            });
            let id = item.id.clone();
            self.worn.retain(|w| w.id != id);
            self.worn.push(WornGarment {
                id,
                name: item.name,
                kind: item.kind,
                builtin: None,
                color: if item.kind == ItemKind::Hair {
                    [0.12, 0.07, 0.045]
                } else {
                    [0.22, 0.24, 0.28]
                },
                fitted: mesh.positions.clone(),
                mesh,
                settings: item.settings,
                binding,
                skin,
                hidden_body: Vec::new(),
                hidden_self: Vec::new(),
            });
            n += 1;
        }
        self.dirty = true;
        self.clothing_dirty = true;
        self.status = format!("Loaded {n} garment(s) from {}", dir.display());
        Ok(n)
    }

    pub fn worn_from_entry(&mut self, e: &WardrobeEntry) -> Option<WornGarment> {
        if let Some(id) = &e.builtin {
            self.ensure_library();
            let src = self
                .library
                .as_ref()?
                .iter()
                .find(|g| g.id == id)
                .cloned()?;
            let mut w = WornGarment::from_authored(&src);
            w.id = e.id.clone();
            w.name = e.name.clone();
            w.settings = e.settings.clone();
            w.color = e.color;
            w.hidden_body = e.hidden_tris.clone();
            return Some(w);
        }
        let mesh = e.mesh.as_ref()?;
        let gmesh = GarmentMesh::from_parts(
            mesh.positions.clone(),
            mesh.triangles.clone(),
            mesh.uvs.clone(),
        );
        let body = self
            .asset
            .evaluate_full(&self.asset.catalog.defaults())
            .body;
        let indices = self.asset.mesh.base.indices.clone();
        let binding = e
            .binding
            .clone()
            .unwrap_or_else(|| bind(&body, &indices, &[], &gmesh.positions));
        let skin = self.asset.rig.as_ref().map_or_else(Vec::new, |r| {
            transfer_weights(&r.skin, &binding, &indices, &gmesh.neighbors, 6)
        });
        Some(WornGarment {
            id: e.id.clone(),
            name: e.name.clone(),
            kind: e.kind,
            builtin: None,
            color: e.color,
            fitted: gmesh.positions.clone(),
            mesh: gmesh,
            settings: e.settings.clone(),
            binding,
            skin,
            hidden_body: e.hidden_tris.clone(),
            hidden_self: Vec::new(),
        })
    }

    /// Refit every visible garment and fitted accessory to `body` (unposed)
    /// and recompute hide-under masks, including between layers.
    pub fn refit_clothing(&mut self, body: &[Vec3]) {
        let indices = self.asset.mesh.base.indices.clone();
        let normals = self.asset.mesh.base.compute_normals(body);
        let rig_skin = self.asset.rig.as_ref().map(|r| r.skin.clone());
        for w in &mut self.worn {
            if w.settings.method == FitMethod::Rigid {
                w.fitted = w.mesh.positions.clone();
            } else {
                w.fitted = fit(body, &indices, &normals, &w.mesh, &w.binding, &w.settings);
            }
            if w.skin.is_empty() {
                if let Some(skin) = &rig_skin {
                    w.skin = transfer_weights(skin, &w.binding, &indices, &w.mesh.neighbors, 6);
                }
            }
        }
        for a in &mut self.accessories {
            refit_accessory(a, body, &indices, &normals, rig_skin.as_deref());
        }
        let mut prepared = Vec::new();
        for (i, w) in self.worn.iter().enumerate() {
            if !w.settings.visible {
                continue;
            }
            let idx = visible_indices(&w.mesh.flat_indices(), &[]);
            prepared.push(prep(
                &w.fitted,
                &idx,
                w.settings.layer,
                w.settings.hide_body && w.settings.visible,
                w.settings.cover,
                Some(i),
                None,
            ));
        }
        for (i, a) in self.accessories.iter().enumerate() {
            if !a.visible || a.settings.method == FitMethod::Rigid {
                continue;
            }
            let pos = a.fitted.as_deref().unwrap_or(&a.acc.positions);
            let idx: Vec<u32> = a.acc.triangles.iter().flatten().copied().collect();
            prepared.push(prep(
                pos,
                &idx,
                a.settings.layer,
                a.settings.hide_body,
                a.settings.cover,
                None,
                Some(i),
            ));
        }
        let covers: Vec<CoverMesh<'_>> = prepared
            .iter()
            .map(|p| CoverMesh {
                positions: &p.positions,
                indices: &p.indices,
                normals: &p.normals,
                layer: p.layer,
                hide_body: p.hide_body,
                cover: p.cover,
            })
            .collect();
        let (body_hidden, per_item) = occlusion(body, &normals, &indices, &covers);
        self.body_hidden = body_hidden;
        for w in &mut self.worn {
            w.hidden_body.clear();
            w.hidden_self.clear();
        }
        for a in &mut self.accessories {
            a.hidden_body.clear();
        }
        for (item, hidden) in prepared.iter().zip(per_item) {
            if let Some(i) = item.worn {
                self.worn[i].hidden_self = hidden;
            }
        }
        // Per-item body masks (the union above is what drawing and export use).
        let own: Vec<(Option<usize>, Option<usize>, Vec<u32>)> = prepared
            .iter()
            .filter(|p| p.hide_body)
            .map(|p| {
                let one = [CoverMesh {
                    positions: &p.positions,
                    indices: &p.indices,
                    normals: &p.normals,
                    layer: p.layer,
                    hide_body: true,
                    cover: p.cover,
                }];
                let (tris, _) = occlusion(body, &normals, &indices, &one);
                (p.worn, p.accessory, tris)
            })
            .collect();
        for (worn_i, acc_i, tris) in own {
            if let Some(i) = worn_i {
                self.worn[i].hidden_body = tris;
            } else if let Some(i) = acc_i {
                self.accessories[i].hidden_body = tris;
            }
        }
        self.clothing_dirty = true;
        if self.accessories.iter().any(|a| a.visible) {
            self.accessories_dirty = true;
        }
    }

    /// Rigid accessories stay on the accessory path. Fitted ones, garments
    /// and hair are skinned meshes. Hidden body triangles are the union.
    pub fn clothing_export(&self) -> (Vec<Accessory>, Vec<WornMesh>, Vec<u32>) {
        let mut rigid = Vec::new();
        let mut worn = Vec::new();
        for a in self.accessories.iter().filter(|a| a.visible) {
            if a.settings.method == FitMethod::Rigid || a.fitted.is_none() {
                rigid.push(a.acc.clone());
                continue;
            }
            let positions = a.fitted.clone().unwrap_or_else(|| a.acc.positions.clone());
            let indices: Vec<u32> = a.acc.triangles.iter().flatten().copied().collect();
            let color = a.acc.color.first().map_or([0.42, 0.44, 0.48, 1.0], |c| {
                [
                    oxy_material::srgb_to_linear(c.x),
                    oxy_material::srgb_to_linear(c.y),
                    oxy_material::srgb_to_linear(c.z),
                    1.0,
                ]
            });
            worn.push(WornMesh {
                name: a.acc.name.clone(),
                positions,
                indices,
                uvs: Vec::new(),
                skin: a.influences.clone(),
                base_color: color,
                hair: false,
                double_sided: true,
            });
        }
        for w in self.worn.iter().filter(|w| w.settings.visible) {
            let indices = visible_indices(&w.mesh.flat_indices(), &w.hidden_self);
            worn.push(WornMesh {
                name: w.name.clone(),
                positions: if w.fitted.len() == w.mesh.positions.len() {
                    w.fitted.clone()
                } else {
                    w.mesh.positions.clone()
                },
                indices,
                uvs: w.mesh.uvs.clone(),
                skin: (!w.skin.is_empty()).then(|| w.skin.clone()),
                base_color: [w.color[0], w.color[1], w.color[2], 1.0],
                hair: w.kind == ItemKind::Hair,
                double_sided: true,
            });
        }
        (rigid, worn, self.body_hidden.clone())
    }
}

fn refit_accessory(
    a: &mut AccessoryItem,
    body: &[Vec3],
    indices: &[u32],
    normals: &[Vec3],
    rig_skin: Option<&[Influence]>,
) {
    if !a.visible || a.settings.method == FitMethod::Rigid {
        a.fitted = None;
        return;
    }
    if a.binding
        .as_ref()
        .is_none_or(|b| b.verts.len() != a.acc.positions.len())
    {
        a.binding = Some(bind(body, indices, normals, &a.acc.positions));
        a.influences = None;
    }
    let mesh = GarmentMesh::from_parts(a.acc.positions.clone(), a.acc.triangles.clone(), vec![]);
    if let Some(binding) = &a.binding {
        a.fitted = Some(fit(body, indices, normals, &mesh, binding, &a.settings));
        if a.influences.is_none() {
            if let Some(skin) = rig_skin {
                a.influences = Some(transfer_weights(skin, binding, indices, &mesh.neighbors, 6));
            }
        }
    }
}

fn prep(
    positions: &[Vec3],
    indices: &[u32],
    layer: u16,
    hide_body: bool,
    cover: f32,
    worn: Option<usize>,
    accessory: Option<usize>,
) -> Prepared {
    let normals = oxy_mesh::Mesh::new(positions.to_vec(), indices.to_vec())
        .map(|m| m.compute_normals(positions))
        .unwrap_or_else(|_| vec![Vec3::Y; positions.len()]);
    Prepared {
        positions: positions.to_vec(),
        indices: indices.to_vec(),
        normals,
        layer,
        hide_body,
        cover,
        worn,
        accessory,
    }
}

fn visible_indices(indices: &[u32], hidden: &[u32]) -> Vec<u32> {
    oxy_fit::without_triangles(indices, hidden)
}

/// Bind a new accessory against the body it was modelled on.
pub fn accessory_fit(
    acc: &Accessory,
    body: &[Vec3],
    indices: &[u32],
    skin: Option<&[Influence]>,
) -> (FitSettings, Option<GarmentBind>, Option<Vec<Influence>>) {
    let method = oxy_fit::suggest_method(body, &acc.positions);
    let mut settings = FitSettings::rigid();
    if method == FitMethod::Rigid {
        return (settings, None, None);
    }
    settings.method = method;
    settings.hide_body = true;
    settings.layer = 4;
    let binding = bind(body, indices, &[], &acc.positions);
    let influences = skin.map(|s| {
        let mesh = GarmentMesh::from_parts(acc.positions.clone(), acc.triangles.clone(), vec![]);
        transfer_weights(s, &binding, indices, &mesh.neighbors, 6)
    });
    (settings, Some(binding), influences)
}
