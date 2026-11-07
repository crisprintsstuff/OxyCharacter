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
                    add(out, name, w);
                }
            }
        }
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
