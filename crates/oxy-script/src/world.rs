//! The character state a script is allowed to change.
//!
//! Scripts never see the mesh. They read and write this record, and the
//! editor or CLI applies it afterwards. That keeps plugins free of the
//! renderer and makes every host function testable on its own.

use std::collections::BTreeMap;

use oxy_fit::{builtin_ids, builtin_name};
use oxy_io::material::MaterialParams;
use oxy_morph::{Catalog, ParamDef, Params};
use oxy_rig::pose::PRESETS;

/// One slider, copied from the catalog so a script cannot change the ranges.
#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub id: String,
    pub label: String,
    pub group: Vec<String>,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub mirror: Option<String>,
}

impl ParamInfo {
    fn from_def(p: &ParamDef) -> Self {
        Self {
            id: p.id.clone(),
            label: p.label.clone(),
            group: p.group.clone(),
            min: p.min,
            max: p.max,
            default: p.default,
            mirror: p.mirror.clone(),
        }
    }
}

/// A built-in garment or hair piece the script put on the character.
#[derive(Debug, Clone, PartialEq)]
pub struct WornItem {
    pub id: String,
    /// 0..1. `None` means "leave the built-in tightness".
    pub tightness: Option<f32>,
    /// Linear RGB. `None` means "leave the built-in colour".
    pub color: Option<[f32; 3]>,
}

/// Named function a plugin offered with `action`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub id: String,
    pub label: String,
}

/// Mutable view a script runs against.
#[derive(Debug, Clone)]
pub struct World {
    params: Vec<ParamInfo>,
    values: BTreeMap<String, f32>,
    pose: String,
    worn: Vec<WornItem>,
    skin: [f32; 3],
    material: MaterialParams,
    symmetry: bool,
    vertex_count: usize,
    bone_count: usize,
    log: Vec<String>,
    actions: Vec<Action>,
}

impl World {
    /// Neutral character: every slider at its default, rest pose, nothing worn.
    pub fn from_catalog(catalog: &Catalog, vertex_count: usize, bone_count: usize) -> Self {
        let params: Vec<ParamInfo> = catalog.params.iter().map(ParamInfo::from_def).collect();
        let values = catalog.defaults();
        Self {
            params,
            values,
            pose: PRESETS[0].name.to_string(),
            worn: Vec::new(),
            skin: [0.75, 0.54, 0.42],
            material: MaterialParams::default(),
            symmetry: true,
            vertex_count,
            bone_count,
            log: Vec::new(),
            actions: Vec::new(),
        }
    }

    /// Replace slider values. Unknown ids are ignored. Missing ids stay put.
    pub fn load_values(&mut self, params: &Params) {
        for (id, v) in params {
            if let Some(def) = self.params.iter().find(|p| p.id == *id) {
                self.values.insert(id.clone(), v.clamp(def.min, def.max));
            }
        }
    }

    pub fn load_pose(&mut self, name: &str) {
        if let Some(p) = PRESETS.iter().find(|p| p.name.eq_ignore_ascii_case(name)) {
            self.pose = p.name.to_string();
        }
    }

    pub fn load_worn(&mut self, items: Vec<WornItem>) {
        self.worn = items
            .into_iter()
            .filter(|w| builtin_name(&w.id).is_some())
            .collect();
    }

    pub fn set_skin_tone(&mut self, rgb: [f32; 3]) {
        self.skin = clamp_rgb(rgb);
    }

    pub fn set_material(&mut self, material: MaterialParams) {
        self.material = material;
    }

    pub fn set_symmetry_flag(&mut self, on: bool) {
        self.symmetry = on;
    }

    pub fn values(&self) -> &BTreeMap<String, f32> {
        &self.values
    }

    pub fn pose(&self) -> &str {
        &self.pose
    }

    pub fn worn(&self) -> &[WornItem] {
        &self.worn
    }

    pub fn skin(&self) -> [f32; 3] {
        self.skin
    }

    pub fn material(&self) -> &MaterialParams {
        &self.material
    }

    pub fn symmetry(&self) -> bool {
        self.symmetry
    }

    pub fn log_lines(&self) -> &[String] {
        &self.log
    }

    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    pub fn bone_count(&self) -> usize {
        self.bone_count
    }

    pub fn slider_count(&self) -> usize {
        self.params.len()
    }

    fn def(&self, id: &str) -> Result<&ParamInfo, String> {
        self.params
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("unknown parameter \"{id}\""))
    }

    pub fn get(&self, id: &str) -> Result<f32, String> {
        let def = self.def(id)?;
        Ok(self.values.get(id).copied().unwrap_or(def.default))
    }

    pub fn set(&mut self, id: &str, value: f32) -> Result<f32, String> {
        let (min, max, mirror) = {
            let def = self.def(id)?;
            (def.min, def.max, def.mirror.clone())
        };
        let v = value.clamp(min, max);
        self.values.insert(id.to_string(), v);
        if self.symmetry {
            if let Some(other) = mirror {
                if self.def(&other).is_ok() {
                    self.values.insert(other, v);
                }
            }
        }
        Ok(v)
    }

    pub fn reset(&mut self, id: &str) -> Result<(), String> {
        let default = self.def(id)?.default;
        self.set(id, default)?;
        Ok(())
    }

    pub fn reset_all(&mut self) {
        self.values = self
            .params
            .iter()
            .map(|p| (p.id.clone(), p.default))
            .collect();
    }

    pub fn changed(&self) -> Vec<String> {
        self.params
            .iter()
            .filter(|p| (self.get(&p.id).unwrap_or(p.default) - p.default).abs() > 1e-5)
            .map(|p| p.id.clone())
            .collect()
    }

    pub fn param_ids(&self) -> Vec<String> {
        self.params.iter().map(|p| p.id.clone()).collect()
    }

    pub fn param(&self, id: &str) -> Result<&ParamInfo, String> {
        self.def(id)
    }

    pub fn set_pose(&mut self, name: &str) -> Result<(), String> {
        let Some(p) = PRESETS.iter().find(|p| p.name.eq_ignore_ascii_case(name)) else {
            let names: Vec<_> = PRESETS.iter().map(|p| p.name).collect();
            return Err(format!("unknown pose \"{name}\"; one of {names:?}"));
        };
        self.pose = p.name.to_string();
        Ok(())
    }

    pub fn pose_names(&self) -> Vec<&'static str> {
        PRESETS.iter().map(|p| p.name).collect()
    }

    pub fn garment_ids(&self) -> Vec<&'static str> {
        builtin_ids().to_vec()
    }

    pub fn wear(&mut self, id: &str, tightness: Option<f32>) -> Result<(), String> {
        if builtin_name(id).is_none() {
            return Err(format!(
                "unknown garment \"{id}\"; one of {:?}",
                builtin_ids()
            ));
        }
        let tight = tightness.map(|t| t.clamp(0.0, 1.0));
        if let Some(existing) = self.worn.iter_mut().find(|w| w.id == id) {
            if let Some(t) = tight {
                existing.tightness = Some(t);
            }
            return Ok(());
        }
        self.worn.push(WornItem {
            id: id.to_string(),
            tightness: tight,
            color: None,
        });
        Ok(())
    }

    pub fn take_off(&mut self, id: &str) -> Result<(), String> {
        if builtin_name(id).is_none() {
            return Err(format!("unknown garment \"{id}\""));
        }
        self.worn.retain(|w| w.id != id);
        Ok(())
    }

    pub fn set_tightness(&mut self, id: &str, value: f32) -> Result<(), String> {
        let item = self
            .worn
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or_else(|| format!("\"{id}\" is not worn"))?;
        item.tightness = Some(value.clamp(0.0, 1.0));
        Ok(())
    }

    pub fn set_garment_color(&mut self, id: &str, rgb: [f32; 3]) -> Result<(), String> {
        let item = self
            .worn
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or_else(|| format!("\"{id}\" is not worn"))?;
        item.color = Some(clamp_rgb(rgb));
        Ok(())
    }

    pub fn set_symmetry(&mut self, on: bool) {
        self.symmetry = on;
    }

    pub fn push_log(&mut self, message: impl Into<String>) {
        self.log.push(message.into());
    }

    pub fn add_action(&mut self, id: &str, label: &str) -> Result<(), String> {
        if id.is_empty() || id.chars().any(|c| !(c.is_ascii_alphanumeric() || c == '_')) {
            return Err(format!(
                "action id \"{id}\" must be letters, digits and underscores"
            ));
        }
        if id.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return Err(format!("action id \"{id}\" must not start with a digit"));
        }
        if let Some(existing) = self.actions.iter_mut().find(|a| a.id == id) {
            existing.label = label.to_string();
        } else {
            self.actions.push(Action {
                id: id.to_string(),
                label: label.to_string(),
            });
        }
        Ok(())
    }

    pub(crate) fn amount(&self, name: &str) -> Result<f32, String> {
        Ok(match name {
            "variation" => self.material.skin.variation,
            "detail" => self.material.skin.detail,
            "roughness" => self.material.skin.roughness,
            "freckles" => self.material.skin.freckles,
            "blush" => self.material.skin.blush,
            "eye_shadow" => self.material.skin.eye_shadow,
            "lips" => self.material.skin.lips,
            "iris_angle" => self.material.eyes.iris_angle,
            "pupil" => self.material.eyes.pupil,
            "eye_size" => self.material.eyes.size,
            other => return Err(format!("unknown amount \"{other}\"")),
        })
    }

    pub(crate) fn set_amount(&mut self, name: &str, value: f32) -> Result<f32, String> {
        let v = match name {
            "variation" | "detail" | "roughness" | "freckles" | "blush" | "eye_shadow" | "lips"
            | "pupil" => value.clamp(0.0, 1.0),
            "iris_angle" => value.clamp(0.05, 1.5),
            "eye_size" => value.clamp(0.05, 0.5),
            other => return Err(format!("unknown amount \"{other}\"")),
        };
        match name {
            "variation" => self.material.skin.variation = v,
            "detail" => self.material.skin.detail = v,
            "roughness" => self.material.skin.roughness = v,
            "freckles" => self.material.skin.freckles = v,
            "blush" => self.material.skin.blush = v,
            "eye_shadow" => self.material.skin.eye_shadow = v,
            "lips" => self.material.skin.lips = v,
            "iris_angle" => self.material.eyes.iris_angle = v,
            "pupil" => self.material.eyes.pupil = v,
            "eye_size" => self.material.eyes.size = v,
            _ => unreachable!(),
        }
        Ok(v)
    }

    pub(crate) fn color(&self, name: &str) -> Result<[f32; 3], String> {
        Ok(match name {
            "skin" => self.skin,
            "freckle" => self.material.skin.freckle_color,
            "blush" => self.material.skin.blush_color,
            "eye_shadow" => self.material.skin.eye_shadow_color,
            "lip" => self.material.skin.lip_color,
            "iris" => self.material.eyes.iris,
            other => return Err(format!("unknown colour \"{other}\"")),
        })
    }

    pub(crate) fn set_color(&mut self, name: &str, rgb: [f32; 3]) -> Result<(), String> {
        let rgb = clamp_rgb(rgb);
        match name {
            "skin" => self.skin = rgb,
            "freckle" => self.material.skin.freckle_color = rgb,
            "blush" => self.material.skin.blush_color = rgb,
            "eye_shadow" => self.material.skin.eye_shadow_color = rgb,
            "lip" => self.material.skin.lip_color = rgb,
            "iris" => self.material.eyes.iris = rgb,
            other => return Err(format!("unknown colour \"{other}\"")),
        }
        Ok(())
    }

    pub(crate) fn skin_seed(&self) -> u32 {
        self.material.skin.seed
    }

    pub(crate) fn set_skin_seed(&mut self, seed: i64) -> Result<(), String> {
        if seed < 0 {
            return Err("skin seed must be zero or positive".into());
        }
        self.material.skin.seed = u32::try_from(seed).unwrap_or(u32::MAX);
        Ok(())
    }
}

fn clamp_rgb(rgb: [f32; 3]) -> [f32; 3] {
    [
        rgb[0].clamp(0.0, 1.0),
        rgb[1].clamp(0.0, 1.0),
        rgb[2].clamp(0.0, 1.0),
    ]
}
