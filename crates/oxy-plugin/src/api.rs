//! Plugin API version 1.
//!
//! | Method | Params | Result |
//! |---|---|---|
//! | `info` | — | api version, host kind, base mesh, document |
//! | `params.list` | `group?` (prefix such as `"Head/Nose"`), `search?` | `[{id, label, group, min, max, default, value}]` |
//! | `params.get` | `ids?` | `{id: value}` |
//! | `params.set` | `values: {id: value}` | `{set}`; values are clamped; unknown ids are an error and nothing is set |
//! | `params.reset` | `ids?` (all when absent) | `{reset}` |
//! | `material.get` | — | `{skin_tone, skin, eyes}` |
//! | `material.set` | `skin_tone?`, `skin?`, `eyes?` (partial objects) | material after the change |
//! | `pose.list` / `pose.get` / `pose.set` | `name` for set | names / current name / `{pose}` |
//! | `body.mesh` | `triangles?` | `{positions, triangles?}` (rest pose, metres, feet at y = 0) |
//! | `body.joints` | — | `{bones: [{name, parent, head, tail}]}` |
//! | `body.measure` | — | `{height, width, depth, min, max}` |
//! | `morph.add` | `label`, and `indices` + `deltas`, or `positions` (whole body) | `{id}`; the new slider is set to `value` (default 1) |
//! | `wardrobe.list` | — | `[{id, name, worn}]` |
//! | `wardrobe.wear` | `items` (`"tee,pants:0.8,hair"`) | `{worn}` |
//! | `wardrobe.clear` | — | `{}` |
//! | `export.glb` | `path`, option overrides (`tpose`, `blendshapes`, `textures`, `texture_size`, `eyes`, `skeleton`) | export stats |
//! | `project.save` | `path` | `{path}` |
//! | `notify` | `text` | `{}`; shown to the user |
//!
//! Errors use JSON-RPC codes: `-32601` unknown method, `-32602` bad params,
//! `-32000` the host couldn't do it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxy_io::export::{BindPose, ExportOptions};
use oxy_io::expression::{self as expr, Expression};
use oxy_io::glam::Vec3;
use oxy_io::material::MaterialParams;
use oxy_io::oxybrush::{slug, CustomMorph, CUSTOM_PREFIX};
use oxy_io::CharacterAsset;
use oxy_morph::Params;
use serde_json::{json, Map, Value};

pub const API_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{message} ({code})")]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    pub fn method(m: &str) -> Self {
        Self {
            code: -32601,
            message: format!("unknown method `{m}`"),
        }
    }
    pub fn params(msg: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: msg.into(),
        }
    }
    pub fn host(msg: impl Into<String>) -> Self {
        Self {
            code: -32000,
            message: msg.into(),
        }
    }
}

/// What a host (the editor or the CLI) provides. Everything that can be
/// derived from the asset and parameters is done by [`dispatch`].
pub trait Host {
    /// `"editor"` or `"cli"`.
    fn kind(&self) -> &'static str;
    fn asset(&self) -> &CharacterAsset;
    fn params(&self) -> &Params;
    /// Set one (already validated and clamped) parameter.
    fn set_param(&mut self, id: &str, value: f32);
    /// Material and linear skin tone.
    fn material(&self) -> (MaterialParams, [f32; 3]);
    fn set_material(&mut self, material: MaterialParams, skin_tone: [f32; 3]);
    /// Index into `oxy_rig::pose::PRESETS`.
    fn pose(&self) -> usize;
    fn set_pose(&mut self, index: usize);
    /// Facial expression and gaze.
    fn expression(&self) -> Expression;
    fn set_expression(&mut self, expression: Expression);
    /// Register a sculpted morph; returns its parameter id.
    fn add_custom_morph(&mut self, morph: CustomMorph) -> String;
    /// `(id, name, worn)` for every wardrobe item the host knows.
    fn wardrobe(&self) -> Vec<(String, String, bool)>;
    fn wear(&mut self, spec: &str) -> Result<(), String>;
    fn clear_wardrobe(&mut self);
    fn export_options(&self) -> ExportOptions;
    fn export_glb(&mut self, path: &Path, options: &ExportOptions) -> Result<Value, String>;
    fn save_project(&mut self, path: &Path) -> Result<PathBuf, String>;
    /// Document path and unsaved state, as JSON.
    fn document(&self) -> Value;
    fn notify(&mut self, text: &str);
}

fn arr3(v: Vec3) -> Value {
    json!([v.x, v.y, v.z])
}

fn vec3(v: &Value) -> Option<Vec3> {
    let a = v.as_array()?;
    if a.len() != 3 {
        return None;
    }
    Some(Vec3::new(
        a[0].as_f64()? as f32,
        a[1].as_f64()? as f32,
        a[2].as_f64()? as f32,
    ))
}

fn expression_json(e: &Expression) -> Value {
    json!({ "units": e.units, "gaze": e.gaze })
}

/// Merge `patch` into `base` (objects recursively; other values replace).
fn merge(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(b), Value::Object(p)) => {
            for (k, v) in p {
                merge(b.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (b, p) => *b = p.clone(),
    }
}

fn str_param<'a>(p: &'a Value, key: &str) -> Result<&'a str, RpcError> {
    p.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::params(format!("`{key}` (string) is required")))
}

/// Answer one request.
pub fn dispatch(host: &mut dyn Host, method: &str, p: &Value) -> Result<Value, RpcError> {
    match method {
        "info" => {
            let a = host.asset();
            Ok(json!({
                "api_version": API_VERSION,
                "app": format!("oxycharacter {}", env!("CARGO_PKG_VERSION")),
                "host": host.kind(),
                "base": {
                    "name": a.meta.name,
                    "license": a.meta.license,
                    "vertices": a.mesh.base.vertex_count(),
                    "triangles": a.mesh.base.indices.len() / 3,
                    "bones": a.rig.as_ref().map_or(0, |r| r.bones.len()),
                },
                "document": host.document(),
            }))
        }
        "params.list" => {
            let group = p
                .get("group")
                .and_then(|g| g.as_str())
                .map(|g| g.to_lowercase());
            let search = p
                .get("search")
                .and_then(|g| g.as_str())
                .map(|g| g.to_lowercase());
            let values = host.params();
            let list: Vec<Value> = host
                .asset()
                .catalog
                .params
                .iter()
                .filter(|d| {
                    group
                        .as_ref()
                        .is_none_or(|g| d.group.join("/").to_lowercase().starts_with(g))
                })
                .filter(|d| {
                    search.as_ref().is_none_or(|s| {
                        d.id.to_lowercase().contains(s) || d.label.to_lowercase().contains(s)
                    })
                })
                .map(|d| {
                    json!({
                        "id": d.id, "label": d.label, "group": d.group,
                        "min": d.min, "max": d.max, "default": d.default,
                        "value": values.get(&d.id).copied().unwrap_or(d.default),
                    })
                })
                .collect();
            Ok(json!(list))
        }
        "params.get" => {
            let values = host.params();
            let ids: Vec<String> = match p.get("ids").and_then(|i| i.as_array()) {
                Some(ids) => ids
                    .iter()
                    .filter_map(|i| i.as_str().map(String::from))
                    .collect(),
                None => values.keys().cloned().collect(),
            };
            let mut out = Map::new();
            for id in ids {
                let v = values
                    .get(&id)
                    .copied()
                    .ok_or_else(|| RpcError::params(format!("unknown parameter `{id}`")))?;
                out.insert(id, json!(v));
            }
            Ok(Value::Object(out))
        }
        "params.set" => {
            let vals = p
                .get("values")
                .and_then(|v| v.as_object())
                .ok_or_else(|| RpcError::params("`values` (object) is required"))?;
            // Validate everything first so a bad id changes nothing.
            let mut checked = Vec::new();
            for (id, v) in vals {
                let def = host
                    .asset()
                    .catalog
                    .param(id)
                    .ok_or_else(|| RpcError::params(format!("unknown parameter `{id}`")))?;
                let v = v
                    .as_f64()
                    .ok_or_else(|| RpcError::params(format!("`{id}` needs a number")))?
                    as f32;
                checked.push((id.clone(), v.clamp(def.min, def.max)));
            }
            for (id, v) in &checked {
                host.set_param(id, *v);
            }
            Ok(json!({ "set": checked.len() }))
        }
        "params.reset" => {
            let ids: Vec<(String, f32)> = match p.get("ids").and_then(|i| i.as_array()) {
                Some(ids) => ids
                    .iter()
                    .filter_map(|i| i.as_str())
                    .map(|id| {
                        host.asset()
                            .catalog
                            .param(id)
                            .map(|d| (d.id.clone(), d.default))
                            .ok_or_else(|| RpcError::params(format!("unknown parameter `{id}`")))
                    })
                    .collect::<Result<_, _>>()?,
                None => host
                    .asset()
                    .catalog
                    .params
                    .iter()
                    .map(|d| (d.id.clone(), d.default))
                    .collect(),
            };
            for (id, v) in &ids {
                host.set_param(id, *v);
            }
            Ok(json!({ "reset": ids.len() }))
        }
        "material.get" => {
            let (m, tone) = host.material();
            Ok(json!({ "skin_tone": tone, "skin": m.skin, "eyes": m.eyes }))
        }
        "material.set" => {
            let (m, tone) = host.material();
            let mut cur = json!({ "skin": m.skin, "eyes": m.eyes });
            for key in ["skin", "eyes"] {
                if let Some(patch) = p.get(key) {
                    if !patch.is_object() {
                        return Err(RpcError::params(format!("`{key}` must be an object")));
                    }
                    merge(&mut cur[key], patch);
                }
            }
            let m2: MaterialParams = serde_json::from_value(cur)
                .map_err(|e| RpcError::params(format!("bad material: {e}")))?;
            let tone2 = match p.get("skin_tone") {
                Some(v) => {
                    let t = vec3(v).ok_or_else(|| {
                        RpcError::params("`skin_tone` must be [r, g, b] (linear)")
                    })?;
                    t.clamp(Vec3::ZERO, Vec3::ONE).to_array()
                }
                None => tone,
            };
            host.set_material(m2.clone(), tone2);
            Ok(json!({ "skin_tone": tone2, "skin": m2.skin, "eyes": m2.eyes }))
        }
        "pose.list" => Ok(json!(oxy_rig::pose::PRESETS
            .iter()
            .map(|p| p.name)
            .collect::<Vec<_>>())),
        "pose.get" => Ok(json!(oxy_rig::pose::PRESETS[host.pose()].name)),
        "pose.set" => {
            let name = str_param(p, "name")?;
            let i = oxy_rig::pose::PRESETS
                .iter()
                .position(|x| x.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| RpcError::params(format!("unknown pose `{name}`")))?;
            host.set_pose(i);
            Ok(json!({ "pose": oxy_rig::pose::PRESETS[i].name }))
        }
        "expression.list" => {
            let mix = |m: &expr::Mix| json!({ "id": m.id, "label": m.label, "units": m.units.iter().map(|(u, w)| (u.to_string(), *w)).collect::<BTreeMap<_, _>>() });
            Ok(json!({
                "units": expr::available(host.asset())
                    .map(|u| json!({ "id": u.id, "label": u.label, "group": u.group }))
                    .collect::<Vec<_>>(),
                "presets": expr::PRESETS.iter().map(mix).collect::<Vec<_>>(),
                "visemes": expr::VISEMES.iter().map(mix).collect::<Vec<_>>(),
                "gaze_limit": expr::GAZE_LIMIT,
            }))
        }
        "expression.get" => Ok(expression_json(&host.expression())),
        "expression.set" => {
            let mut e = if p.get("replace").and_then(|v| v.as_bool()).unwrap_or(false) {
                Expression {
                    gaze: host.expression().gaze,
                    ..Default::default()
                }
            } else {
                host.expression()
            };
            for (key, find) in [
                (
                    "preset",
                    expr::preset as fn(&str) -> Option<&'static expr::Mix>,
                ),
                ("viseme", expr::viseme),
            ] {
                if let Some(id) = p.get(key) {
                    let id = id
                        .as_str()
                        .ok_or_else(|| RpcError::params(format!("`{key}` must be a string")))?;
                    let m = find(id)
                        .ok_or_else(|| RpcError::params(format!("unknown {key} `{id}`")))?;
                    // A mix replaces the units it names and leaves the others.
                    for &(u, w) in m.units {
                        e.set(u, w);
                    }
                }
            }
            if let Some(units) = p.get("units") {
                let units = units
                    .as_object()
                    .ok_or_else(|| RpcError::params("`units` must be an object of weights"))?;
                for (id, w) in units {
                    if expr::unit(id).is_none() {
                        return Err(RpcError::params(format!("unknown expression unit `{id}`")));
                    }
                    let w = w
                        .as_f64()
                        .ok_or_else(|| RpcError::params(format!("`{id}` needs a number")))?;
                    e.set(id, w as f32);
                }
            }
            if let Some(g) = p.get("gaze") {
                let g = g
                    .as_array()
                    .filter(|a| a.len() == 2)
                    .and_then(|a| Some([a[0].as_f64()? as f32, a[1].as_f64()? as f32]))
                    .ok_or_else(|| RpcError::params("`gaze` must be [yaw, pitch] in degrees"))?;
                e.set_gaze(g[0], g[1]);
            }
            host.set_expression(e.clone());
            Ok(expression_json(&e))
        }
        "body.mesh" => {
            let mut ev = host.asset().evaluate_full(host.params());
            if p.get("expression")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                let e = host.expression();
                expr::FaceShapes::new(host.asset(), &ev.body).apply_expression(&e, &mut ev.body);
            }
            let mut out =
                json!({ "positions": ev.body.iter().map(|v| arr3(*v)).collect::<Vec<_>>() });
            if p.get("triangles")
                .and_then(|t| t.as_bool())
                .unwrap_or(false)
            {
                out["triangles"] =
                    json!(host.asset().mesh.base.indices.chunks(3).collect::<Vec<_>>());
            }
            Ok(out)
        }
        "body.joints" => {
            let a = host.asset();
            let Some(rig) = &a.rig else {
                return Ok(json!({ "bones": [] }));
            };
            let ev = a.evaluate_full(host.params());
            let bones: Vec<Value> = rig
                .bones
                .iter()
                .map(|b| {
                    json!({
                        "name": b.name,
                        "parent": b.parent.map(|i| rig.bones[i].name.clone()),
                        "head": arr3(ev.joints[b.head]),
                        "tail": arr3(ev.joints[b.tail]),
                    })
                })
                .collect();
            Ok(json!({ "bones": bones }))
        }
        "body.measure" => {
            let ev = host.asset().evaluate_full(host.params());
            let (lo, hi) = oxy_io::makehuman::bounds(&ev.body);
            let s = hi - lo;
            Ok(
                json!({ "height": s.y, "width": s.x, "depth": s.z, "min": arr3(lo), "max": arr3(hi) }),
            )
        }
        "morph.add" => {
            let label = str_param(p, "label")?.trim().to_owned();
            if label.is_empty() {
                return Err(RpcError::params("`label` must not be empty"));
            }
            let n = host.asset().mesh.base.vertex_count();
            let (indices, deltas): (Vec<u32>, Vec<Vec3>) = if let Some(pos) =
                p.get("positions").and_then(|v| v.as_array())
            {
                if pos.len() != n {
                    return Err(RpcError::params(format!(
                        "`positions` needs {n} entries, got {}",
                        pos.len()
                    )));
                }
                let cur = host.asset().evaluate_full(host.params()).body;
                let mut out = (Vec::new(), Vec::new());
                for (i, v) in pos.iter().enumerate() {
                    let d = vec3(v).ok_or_else(|| {
                        RpcError::params(format!("`positions[{i}]` must be [x, y, z]"))
                    })? - cur[i];
                    if d.length() > 1e-6 {
                        out.0.push(i as u32);
                        out.1.push(d);
                    }
                }
                out
            } else {
                let idx = p.get("indices").and_then(|v| v.as_array()).ok_or_else(|| {
                    RpcError::params("give `positions`, or `indices` and `deltas`")
                })?;
                let del = p
                    .get("deltas")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| RpcError::params("`deltas` is required with `indices`"))?;
                if idx.len() != del.len() {
                    return Err(RpcError::params(
                        "`indices` and `deltas` must have the same length",
                    ));
                }
                let mut out = (Vec::new(), Vec::new());
                for (i, d) in idx.iter().zip(del) {
                    let i = i.as_u64().filter(|&i| (i as usize) < n).ok_or_else(|| {
                        RpcError::params(format!("vertex index out of range (0..{n})"))
                    })?;
                    out.0.push(i as u32);
                    out.1.push(
                        vec3(d).ok_or_else(|| RpcError::params("each delta must be [x, y, z]"))?,
                    );
                }
                out
            };
            if indices.is_empty() {
                return Err(RpcError::params("the morph moves no vertex"));
            }
            let mut id = slug(&label);
            let base = id.clone();
            let mut k = 2;
            while host
                .asset()
                .catalog
                .param(&format!("{CUSTOM_PREFIX}{id}"))
                .is_some()
            {
                id = format!("{base}-{k}");
                k += 1;
            }
            let param = host.add_custom_morph(CustomMorph {
                id,
                label,
                indices,
                deltas,
            });
            let value = p.get("value").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
            host.set_param(&param, value.clamp(0.0, 1.0));
            Ok(json!({ "id": param }))
        }
        "wardrobe.list" => Ok(json!(host
            .wardrobe()
            .into_iter()
            .map(|(id, name, worn)| json!({ "id": id, "name": name, "worn": worn }))
            .collect::<Vec<_>>())),
        "wardrobe.wear" => {
            let items = str_param(p, "items")?.to_owned();
            host.wear(&items).map_err(RpcError::host)?;
            let worn: Vec<String> = host
                .wardrobe()
                .into_iter()
                .filter(|w| w.2)
                .map(|w| w.0)
                .collect();
            Ok(json!({ "worn": worn }))
        }
        "wardrobe.clear" => {
            host.clear_wardrobe();
            Ok(json!({}))
        }
        "export.glb" => {
            let path = PathBuf::from(str_param(p, "path")?);
            let mut o = host.export_options();
            let flag = |k: &str| p.get(k).and_then(|v| v.as_bool());
            if let Some(t) = flag("tpose") {
                o.bind_pose = if t { BindPose::TPose } else { BindPose::Rest };
            }
            if let Some(v) = flag("blendshapes") {
                o.blendshapes = v;
            }
            if let Some(v) = flag("textures") {
                o.textures = v;
            }
            if let Some(v) = flag("eyes") {
                o.eyes = v;
            }
            if let Some(v) = flag("skeleton") {
                o.skeleton = v;
            }
            if let Some(v) = flag("expressions") {
                o.expressions = v;
            }
            if let Some(v) = p.get("texture_size").and_then(|v| v.as_u64()) {
                o.texture_size = (v as u32).clamp(64, 8192);
            }
            host.export_glb(&path, &o).map_err(RpcError::host)
        }
        "project.save" => {
            let path = PathBuf::from(str_param(p, "path")?);
            let saved = host.save_project(&path).map_err(RpcError::host)?;
            Ok(json!({ "path": saved }))
        }
        "notify" => {
            let text = str_param(p, "text")?.to_owned();
            host.notify(&text);
            Ok(json!({}))
        }
        other => Err(RpcError::method(other)),
    }
}

/// A plain in-memory host over an asset: what the CLI uses, and handy for
/// tests. Wardrobe, export and project saving are supplied by closures so the
/// caller decides how those work.
pub struct SimpleHost<'a> {
    pub asset: CharacterAsset,
    pub params: Params,
    pub material: MaterialParams,
    pub skin_tone: [f32; 3],
    pub pose: usize,
    pub expression: Expression,
    pub export: ExportOptions,
    pub wear: Option<String>,
    pub wardrobe_items: Vec<(String, String)>,
    pub notes: Vec<String>,
    #[allow(clippy::type_complexity)]
    pub exporter:
        Box<dyn FnMut(&SimpleHostView, &Path, &ExportOptions) -> Result<Value, String> + 'a>,
    #[allow(clippy::type_complexity)]
    pub saver: Box<dyn FnMut(&SimpleHostView, &Path) -> Result<PathBuf, String> + 'a>,
}

/// Read-only view handed to [`SimpleHost`] callbacks.
pub struct SimpleHostView<'b> {
    pub asset: &'b CharacterAsset,
    pub params: &'b Params,
    pub material: &'b MaterialParams,
    pub skin_tone: [f32; 3],
    pub pose: usize,
    pub expression: &'b Expression,
    pub wear: Option<&'b str>,
}

impl<'a> SimpleHost<'a> {
    pub fn new(asset: CharacterAsset) -> Self {
        let params = asset.catalog.defaults();
        Self {
            asset,
            params,
            material: MaterialParams::default(),
            skin_tone: [0.62, 0.40, 0.29],
            pose: 0,
            expression: Expression::default(),
            export: ExportOptions::default(),
            wear: None,
            wardrobe_items: Vec::new(),
            notes: Vec::new(),
            exporter: Box::new(|_, _, _| Err("export is not available here".into())),
            saver: Box::new(|_, _| Err("saving projects is not available here".into())),
        }
    }

    fn view(&self) -> SimpleHostView<'_> {
        SimpleHostView {
            asset: &self.asset,
            params: &self.params,
            material: &self.material,
            skin_tone: self.skin_tone,
            pose: self.pose,
            expression: &self.expression,
            wear: self.wear.as_deref(),
        }
    }

    fn worn_ids(&self) -> Vec<String> {
        self.wear
            .as_deref()
            .unwrap_or("")
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.split(':').next().unwrap_or("").to_owned())
            .collect()
    }
}

impl Host for SimpleHost<'_> {
    fn kind(&self) -> &'static str {
        "cli"
    }
    fn asset(&self) -> &CharacterAsset {
        &self.asset
    }
    fn params(&self) -> &Params {
        &self.params
    }
    fn set_param(&mut self, id: &str, value: f32) {
        self.params.insert(id.to_owned(), value);
    }
    fn material(&self) -> (MaterialParams, [f32; 3]) {
        (self.material.clone(), self.skin_tone)
    }
    fn set_material(&mut self, material: MaterialParams, skin_tone: [f32; 3]) {
        self.material = material;
        self.skin_tone = skin_tone;
    }
    fn pose(&self) -> usize {
        self.pose
    }
    fn set_pose(&mut self, index: usize) {
        self.pose = index;
    }
    fn expression(&self) -> Expression {
        self.expression.clone()
    }
    fn set_expression(&mut self, expression: Expression) {
        self.expression = expression;
    }
    fn add_custom_morph(&mut self, morph: CustomMorph) -> String {
        let id = self.asset.add_custom_morph(&morph);
        self.params.insert(id.clone(), 0.0);
        id
    }
    fn wardrobe(&self) -> Vec<(String, String, bool)> {
        let worn = self.worn_ids();
        self.wardrobe_items
            .iter()
            .map(|(id, name)| (id.clone(), name.clone(), worn.contains(id)))
            .collect()
    }
    fn wear(&mut self, spec: &str) -> Result<(), String> {
        for item in spec.split(',').filter(|s| !s.is_empty()) {
            let id = item.split(':').next().unwrap_or("");
            if !self.wardrobe_items.iter().any(|(i, _)| i == id) {
                return Err(format!("unknown wardrobe item `{id}`"));
            }
        }
        let mut all: Vec<String> = self
            .wear
            .as_deref()
            .unwrap_or("")
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        all.extend(spec.split(',').filter(|s| !s.is_empty()).map(String::from));
        self.wear = Some(all.join(","));
        Ok(())
    }
    fn clear_wardrobe(&mut self) {
        self.wear = None;
    }
    fn export_options(&self) -> ExportOptions {
        ExportOptions {
            skin_color: self.skin_tone,
            expression: self.expression.clone(),
            ..self.export.clone()
        }
    }
    fn export_glb(&mut self, path: &Path, options: &ExportOptions) -> Result<Value, String> {
        let mut f = std::mem::replace(&mut self.exporter, Box::new(|_, _, _| Err("busy".into())));
        let r = f(&self.view(), path, options);
        self.exporter = f;
        r
    }
    fn save_project(&mut self, path: &Path) -> Result<PathBuf, String> {
        let mut f = std::mem::replace(&mut self.saver, Box::new(|_, _| Err("busy".into())));
        let r = f(&self.view(), path);
        self.saver = f;
        r
    }
    fn document(&self) -> Value {
        json!({ "path": null, "unsaved": false })
    }
    fn notify(&mut self, text: &str) {
        self.notes.push(text.to_owned());
    }
}

/// Parameter values as a plain map (for hosts that need a copy).
pub fn changed_params(asset: &CharacterAsset, params: &Params) -> BTreeMap<String, f32> {
    params
        .iter()
        .filter(|(id, v)| {
            asset
                .catalog
                .param(id)
                .is_none_or(|d| (**v - d.default).abs() > 1e-6)
        })
        .map(|(k, v)| (k.clone(), *v))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxy_io::AssetMeta;
    use oxy_mesh::{Mesh, MorphTarget, MorphableMesh};

    /// A tetrahedron with one slider; no rig.
    fn host() -> SimpleHost<'static> {
        let base = Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
            vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
        )
        .unwrap();
        let catalog = oxy_morph::Catalog {
            params: vec![oxy_morph::ParamDef {
                id: "nose".into(),
                label: "Nose".into(),
                group: vec!["Head".into(), "Nose".into()],
                min: -1.0,
                max: 1.0,
                default: 0.0,
                mirror: None,
                ends: None,
            }],
            sliders: vec![oxy_morph::Slider::Unipolar {
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
        let mut h = SimpleHost::new(asset);
        h.wardrobe_items = vec![("tee".into(), "T-shirt".into())];
        h
    }

    #[test]
    fn params_list_get_set_validate_and_clamp() {
        let mut h = host();
        let list = dispatch(&mut h, "params.list", &json!({ "group": "head" })).unwrap();
        assert_eq!(list[0]["id"], "nose");
        assert_eq!(
            dispatch(&mut h, "params.set", &json!({ "values": { "nose": 5.0 } })).unwrap()["set"],
            1
        );
        assert_eq!(h.params["nose"], 1.0, "clamped");
        let err = dispatch(
            &mut h,
            "params.set",
            &json!({ "values": { "nose": 0.2, "bogus": 1 } }),
        )
        .unwrap_err();
        assert_eq!(err.code, -32602);
        assert_eq!(h.params["nose"], 1.0, "nothing set on error");
        dispatch(&mut h, "params.reset", &json!({})).unwrap();
        assert_eq!(
            dispatch(&mut h, "params.get", &json!({ "ids": ["nose"] })).unwrap()["nose"],
            0.0
        );
        assert_eq!(
            dispatch(&mut h, "nope", &json!({})).unwrap_err().code,
            -32601
        );
    }

    #[test]
    fn body_measure_material_pose_and_morphs() {
        let mut h = host();
        let m = dispatch(&mut h, "body.measure", &json!({})).unwrap();
        assert!((m["height"].as_f64().unwrap() - 1.0).abs() < 1e-6);
        let mat = dispatch(
            &mut h,
            "material.set",
            &json!({ "skin": { "freckles": 0.5 }, "skin_tone": [0.3, 0.2, 0.1] }),
        )
        .unwrap();
        assert_eq!(mat["skin"]["freckles"], 0.5);
        assert_eq!(h.material.skin.freckles, 0.5);
        assert_eq!(
            h.material.skin.blush,
            MaterialParams::default().skin.blush,
            "unmentioned fields kept"
        );
        assert!(dispatch(&mut h, "material.set", &json!({ "skin": 3 })).is_err());
        dispatch(&mut h, "pose.set", &json!({ "name": "t-pose" })).unwrap();
        assert_eq!(dispatch(&mut h, "pose.get", &json!({})).unwrap(), "T-pose");

        // Expressions: a preset, a unit on top, gaze; replace starts over.
        let r = dispatch(
            &mut h,
            "expression.set",
            &json!({ "preset": "smile", "units": { "mouth-open": 0.4 }, "gaze": [90, -3] }),
        )
        .unwrap();
        assert_eq!(r["units"]["mouth-open"], json!(0.4f32));
        assert_eq!(r["gaze"], json!([35.0, -3.0]));
        assert!(r["units"]["mouth-corner-puller"].as_f64().unwrap() > 0.5);
        let r = dispatch(
            &mut h,
            "expression.set",
            &json!({ "replace": true, "viseme": "oh" }),
        )
        .unwrap();
        assert!(r["units"].get("mouth-corner-puller").is_none());
        assert_eq!(r["gaze"], json!([35.0, -3.0]), "replace keeps the gaze");
        assert_eq!(dispatch(&mut h, "expression.get", &json!({})).unwrap(), r);
        assert!(dispatch(&mut h, "expression.set", &json!({ "units": { "grin": 1 } })).is_err());
        assert!(dispatch(&mut h, "expression.set", &json!({ "preset": "nope" })).is_err());
        let list = dispatch(&mut h, "expression.list", &json!({})).unwrap();
        assert!(list["presets"].as_array().unwrap().len() >= 10);
        assert_eq!(
            list["units"].as_array().unwrap().len(),
            0,
            "tiny asset has no units"
        );

        // A morph given as whole-body positions: lift the apex by 0.5.
        let mesh = dispatch(&mut h, "body.mesh", &json!({ "triangles": true })).unwrap();
        assert_eq!(mesh["triangles"].as_array().unwrap().len(), 4);
        let mut pos = mesh["positions"].clone();
        let apex = pos
            .as_array()
            .unwrap()
            .iter()
            .position(|v| v[1].as_f64().unwrap() > 0.9)
            .unwrap();
        pos[apex][1] = json!(pos[apex][1].as_f64().unwrap() + 0.5);
        let r = dispatch(
            &mut h,
            "morph.add",
            &json!({ "label": "Tall tip", "positions": pos }),
        )
        .unwrap();
        let id = r["id"].as_str().unwrap().to_owned();
        assert_eq!(h.params[&id], 1.0);
        let m2 = dispatch(&mut h, "body.measure", &json!({})).unwrap();
        assert!((m2["height"].as_f64().unwrap() - 1.5).abs() < 1e-5, "{m2}");
        // Same label again gets a fresh id.
        let r2 = dispatch(
            &mut h,
            "morph.add",
            &json!({ "label": "Tall tip", "indices": [1], "deltas": [[0.1, 0, 0]], "value": 0.5 }),
        )
        .unwrap();
        assert_ne!(r2["id"], r["id"]);
        assert!(dispatch(
            &mut h,
            "morph.add",
            &json!({ "label": "x", "indices": [99], "deltas": [[0, 0, 0]] })
        )
        .is_err());
    }

    #[test]
    fn wardrobe_export_and_notify() {
        let mut h = host();
        assert!(dispatch(&mut h, "wardrobe.wear", &json!({ "items": "cape" })).is_err());
        assert_eq!(
            dispatch(&mut h, "wardrobe.wear", &json!({ "items": "tee:0.8" })).unwrap()["worn"],
            json!(["tee"])
        );
        dispatch(&mut h, "wardrobe.clear", &json!({})).unwrap();
        assert!(
            dispatch(&mut h, "export.glb", &json!({ "path": "x.glb" })).is_err(),
            "no exporter configured"
        );
        h.exporter = Box::new(|v, path, o| {
            Ok(
                json!({ "path": path, "tpose": o.bind_pose == BindPose::TPose, "params": v.params.len() }),
            )
        });
        let r = dispatch(
            &mut h,
            "export.glb",
            &json!({ "path": "x.glb", "tpose": true }),
        )
        .unwrap();
        assert_eq!(r["tpose"], true);
        dispatch(&mut h, "notify", &json!({ "text": "hello" })).unwrap();
        assert_eq!(h.notes, vec!["hello"]);
        assert_eq!(
            dispatch(&mut h, "info", &json!({})).unwrap()["api_version"],
            API_VERSION
        );
    }
}
