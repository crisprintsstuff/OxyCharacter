//! User-facing slider model: maps slider values to morph-target weights.
//!
//! * [`Slider::Unipolar`] — `0..=1` drives one target.
//! * [`Slider::Bipolar`]  — `-1..=1` drives a "negative" or "positive" target.
//! * [`MacroGrid`]        — N macro axes (gender, age, weight, …) whose authored
//!   corner shapes are blended with multilinear interpolation.

use std::collections::BTreeMap;

use oxy_mesh::MorphableMesh;
use serde::{Deserialize, Serialize};

/// Accumulated target weights, keyed by target name.
pub type TargetWeights = BTreeMap<String, f32>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Slider {
    Unipolar {
        id: String,
        target: String,
    },
    Bipolar {
        id: String,
        negative: String,
        positive: String,
    },
}

impl Slider {
    pub fn id(&self) -> &str {
        match self {
            Slider::Unipolar { id, .. } | Slider::Bipolar { id, .. } => id,
        }
    }

    pub fn range(&self) -> (f32, f32) {
        match self {
            Slider::Unipolar { .. } => (0.0, 1.0),
            Slider::Bipolar { .. } => (-1.0, 1.0),
        }
    }

    /// Add this slider's contribution for `value` (clamped to its range).
    pub fn apply(&self, value: f32, out: &mut TargetWeights) {
        let (lo, hi) = self.range();
        let v = value.clamp(lo, hi);
        match self {
            Slider::Unipolar { target, .. } => add(out, target, v),
            Slider::Bipolar {
                negative, positive, ..
            } => {
                if v < 0.0 {
                    add(out, negative, -v);
                } else {
                    add(out, positive, v);
                }
            }
        }
    }
}

fn add(out: &mut TargetWeights, name: &str, w: f32) {
    if w != 0.0 {
        *out.entry(name.to_owned()).or_default() += w;
    }
}

/// One macro axis, e.g. `age` with stops `[0.0, 0.5, 1.0]` (young, adult, old).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroAxis {
    pub id: String,
    /// Strictly increasing slider values at which shapes were authored.
    pub stops: Vec<f32>,
}

impl MacroAxis {
    /// Returns `(lower stop index, fraction toward the next stop)`.
    fn locate(&self, value: f32) -> (usize, f32) {
        let s = &self.stops;
        if s.len() < 2 {
            return (0, 0.0);
        }
        let v = value.clamp(s[0], s[s.len() - 1]);
        let i = s.windows(2).position(|w| v <= w[1]).unwrap_or(s.len() - 2);
        (i, (v - s[i]) / (s[i + 1] - s[i]))
    }
}

/// Multilinear blend over an N-dimensional grid of authored corner shapes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroGrid {
    pub axes: Vec<MacroAxis>,
    /// Grid coordinate (one stop index per axis) → target name.
    /// Missing corners are treated as "the base mesh" (no delta).
    pub corners: Vec<(Vec<usize>, String)>,
}

impl MacroGrid {
    /// Add contributions for the macro values (one per axis, in axis order).
    pub fn apply(&self, values: &[f32], out: &mut TargetWeights) {
        self.apply_scaled(values, 1.0, out);
    }

    /// Like [`MacroGrid::apply`], with every contribution multiplied by `scale`.
    pub fn apply_scaled(&self, values: &[f32], scale: f32, out: &mut TargetWeights) {
        if scale == 0.0 {
            return;
        }
        assert_eq!(values.len(), self.axes.len(), "one value per macro axis");
        let cells: Vec<(usize, f32)> = self
            .axes
            .iter()
            .zip(values)
            .map(|(a, &v)| a.locate(v))
            .collect();
        let lookup: BTreeMap<&[usize], &str> = self
            .corners
            .iter()
            .map(|(k, v)| (k.as_slice(), v.as_str()))
            .collect();

        // Visit the 2^N corners of the active cell.
        let mut coord = vec![0usize; cells.len()];
        for mask in 0u32..(1 << cells.len()) {
            let mut w = 1.0;
            for (axis, &(i, t)) in cells.iter().enumerate() {
                let upper = mask & (1 << axis) != 0;
                let at_end = i + 1 >= self.axes[axis].stops.len();
                if upper && at_end {
                    w = 0.0;
                    break;
                }
                coord[axis] = i + upper as usize;
                w *= if upper { t } else { 1.0 - t };
            }
            if w > 0.0 {
                if let Some(name) = lookup.get(coord.as_slice()) {
                    add(out, name, w * scale);
                }
            }
        }
    }
}

/// Parameter values keyed by parameter id.
pub type Params = BTreeMap<String, f32>;

/// A user-editable parameter shown as one slider in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    pub id: String,
    pub label: String,
    /// Category path, e.g. `["Head", "Nose"]`.
    pub group: Vec<String>,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    /// Left/right counterpart for symmetric editing.
    pub mirror: Option<String>,
    /// Labels for the low and high ends of the range, e.g. `["narrow", "wide"]`.
    #[serde(default)]
    pub ends: Option<[String; 2]>,
}

/// How strongly a [`MacroLayer`] contributes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Gain {
    One,
    Constant(f32),
    /// `max(param, 0)` — e.g. the "taller" half of a height slider.
    Positive(String),
    /// `max(-param, 0)` — e.g. the "shorter" half of a height slider.
    Negative(String),
}

/// A macro grid whose axes are read from parameters (axis id = param id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroLayer {
    pub grid: MacroGrid,
    pub gain: Gain,
}

/// The complete parameter → morph-weight model for one base mesh.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub params: Vec<ParamDef>,
    /// Simple sliders; each slider id is also a param id.
    pub sliders: Vec<Slider>,
    pub layers: Vec<MacroLayer>,
}

impl Catalog {
    pub fn defaults(&self) -> Params {
        self.params
            .iter()
            .map(|p| (p.id.clone(), p.default))
            .collect()
    }

    pub fn param(&self, id: &str) -> Option<&ParamDef> {
        self.params.iter().find(|p| p.id == id)
    }

    fn value(&self, params: &Params, id: &str) -> f32 {
        params
            .get(id)
            .copied()
            .or_else(|| self.param(id).map(|p| p.default))
            .unwrap_or(0.0)
    }

    /// Evaluate all sliders and macro layers into target weights.
    pub fn evaluate(&self, params: &Params) -> TargetWeights {
        let mut out = TargetWeights::new();
        for s in &self.sliders {
            s.apply(self.value(params, s.id()), &mut out);
        }
        for layer in &self.layers {
            let scale = match &layer.gain {
                Gain::One => 1.0,
                Gain::Constant(c) => *c,
                Gain::Positive(id) => self.value(params, id).max(0.0),
                Gain::Negative(id) => (-self.value(params, id)).max(0.0),
            };
            let values: Vec<f32> = layer
                .grid
                .axes
                .iter()
                .map(|a| self.value(params, &a.id))
                .collect();
            layer.grid.apply_scaled(&values, scale, &mut out);
        }
        out
    }

    /// Names of every target a parameter can influence (for UI highlighting).
    pub fn targets_for(&self, id: &str) -> Vec<&str> {
        let mut t: Vec<&str> = Vec::new();
        for s in &self.sliders {
            match s {
                Slider::Unipolar { id: sid, target } if sid == id => t.push(target),
                Slider::Bipolar {
                    id: sid,
                    negative,
                    positive,
                } if sid == id => t.extend([negative.as_str(), positive.as_str()]),
                _ => {}
            }
        }
        for layer in &self.layers {
            let gain_param = match &layer.gain {
                Gain::Positive(g) | Gain::Negative(g) => Some(g.as_str()),
                _ => None,
            };
            if gain_param == Some(id) || layer.grid.axes.iter().any(|a| a.id == id) {
                t.extend(layer.grid.corners.iter().map(|(_, n)| n.as_str()));
            }
        }
        t.sort_unstable();
        t.dedup();
        t
    }
}

/// Resolve named weights into a dense weight vector for [`MorphableMesh::evaluate`].
/// Unknown target names are returned so callers can report broken content.
pub fn resolve(mesh: &MorphableMesh, weights: &TargetWeights) -> (Vec<f32>, Vec<String>) {
    let mut dense = vec![0.0; mesh.targets.len()];
    let mut missing = Vec::new();
    for (name, &w) in weights {
        match mesh.target_index(name) {
            Some(i) => dense[i] += w,
            None => missing.push(name.clone()),
        }
    }
    (dense, missing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_applies_gains_and_defaults() {
        let axis = |id: &str| MacroAxis {
            id: id.into(),
            stops: vec![0.0, 1.0],
        };
        let cat = Catalog {
            params: vec![
                ParamDef {
                    id: "gender".into(),
                    label: "Gender".into(),
                    group: vec![],
                    min: 0.0,
                    max: 1.0,
                    default: 0.5,
                    mirror: None,
                    ends: None,
                },
                ParamDef {
                    id: "height".into(),
                    label: "Height".into(),
                    group: vec![],
                    min: -1.0,
                    max: 1.0,
                    default: 0.0,
                    mirror: None,
                    ends: None,
                },
            ],
            sliders: vec![],
            layers: vec![
                MacroLayer {
                    grid: MacroGrid {
                        axes: vec![axis("gender")],
                        corners: vec![(vec![0], "f".into()), (vec![1], "m".into())],
                    },
                    gain: Gain::Constant(0.5),
                },
                MacroLayer {
                    grid: MacroGrid {
                        axes: vec![axis("gender")],
                        corners: vec![(vec![1], "m_tall".into())],
                    },
                    gain: Gain::Positive("height".into()),
                },
            ],
        };
        let mut p = cat.defaults();
        let w = cat.evaluate(&p);
        assert_eq!((w["f"], w["m"]), (0.25, 0.25));
        assert!(!w.contains_key("m_tall"));
        p.insert("height".into(), 0.5);
        p.insert("gender".into(), 1.0);
        let w = cat.evaluate(&p);
        assert_eq!(w["m_tall"], 0.5);
        assert_eq!(cat.targets_for("height"), vec!["m_tall"]);
        assert_eq!(cat.targets_for("gender"), vec!["f", "m", "m_tall"]);
    }

    #[test]
    fn bipolar_picks_one_side_and_clamps() {
        let s = Slider::Bipolar {
            id: "nose_w".into(),
            negative: "nose_narrow".into(),
            positive: "nose_wide".into(),
        };
        let mut w = TargetWeights::new();
        s.apply(-3.0, &mut w);
        assert_eq!(w.get("nose_narrow"), Some(&1.0));
        assert!(!w.contains_key("nose_wide"));
    }

    #[test]
    fn macro_grid_is_multilinear_and_weights_sum_to_one() {
        let grid = MacroGrid {
            axes: vec![
                MacroAxis {
                    id: "gender".into(),
                    stops: vec![0.0, 1.0],
                },
                MacroAxis {
                    id: "age".into(),
                    stops: vec![0.0, 0.5, 1.0],
                },
            ],
            corners: (0..2)
                .flat_map(|g| (0..3).map(move |a| (vec![g, a], format!("g{g}_a{a}"))))
                .collect(),
        };
        let mut w = TargetWeights::new();
        grid.apply(&[0.25, 0.75], &mut w);
        let close = |k: &str, v: f32| assert!((w[k] - v).abs() < 1e-6, "{k}: {}", w[k]);
        close("g0_a1", 0.75 * 0.5);
        close("g0_a2", 0.75 * 0.5);
        close("g1_a1", 0.25 * 0.5);
        close("g1_a2", 0.25 * 0.5);
        assert!((w.values().sum::<f32>() - 1.0).abs() < 1e-6);

        // Exactly on the last stop must not index past the grid.
        let mut w = TargetWeights::new();
        grid.apply(&[1.0, 1.0], &mut w);
        assert_eq!(w.get("g1_a2"), Some(&1.0));
    }
}
