//! Builds a [`CharacterAsset`] from the CC0 MakeHuman data set
//! (`data/3dobjs/base.obj` + `data/targets/**.target`).
//!
//! Only the *data* is used (released CC0 in 2020); the slider taxonomy, pairing
//! heuristics and macro model below are OxyCharacter's own.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use glam::Vec3;
use oxy_mesh::{Mesh, MorphTarget, MorphableMesh};
use oxy_morph::{Catalog, Gain, MacroAxis, MacroGrid, MacroLayer, ParamDef, Slider};

use crate::{obj, target, AssetMeta, CharacterAsset, IoError, Result};

/// MakeHuman works in decimetres; we use metres.
pub const UNIT_SCALE: f32 = 0.1;

/// (negative, positive) suffix pairs that become one bipolar slider.
const PAIRS: &[(&str, &str)] = &[
    ("decr", "incr"),
    ("down", "up"),
    ("in", "out"),
    ("backward", "forward"),
    ("concave", "convex"),
    ("compress", "uncompress"),
    ("less", "more"),
    ("min", "max"),
    ("l", "r"),
];

/// Target folder → UI category path. Folders not listed are not imported.
fn category(folder: &str, name: &str) -> Option<Vec<&'static str>> {
    Some(match folder {
        "head" => vec!["Head", "Head Shape"],
        "forehead" => vec!["Head", "Forehead"],
        "eyebrows" => vec!["Head", "Eyebrows"],
        "eyes" => vec!["Head", "Eyes"],
        "nose" => vec!["Head", "Nose"],
        "mouth" => vec!["Head", "Mouth"],
        "ears" => vec!["Head", "Ears"],
        "cheek" => vec!["Head", "Cheeks"],
        "chin" => vec!["Head", "Chin & Jaw"],
        "neck" => vec!["Body", "Neck"],
        "torso" => vec!["Body", "Torso"],
        "breast" => vec!["Body", "Chest"],
        "stomach" => vec!["Body", "Stomach"],
        "hip" => vec!["Body", "Hips"],
        "buttocks" => vec!["Body", "Buttocks"],
        "pelvis" => vec!["Body", "Pelvis"],
        "bodyshapes" => vec!["Body", "Body Shape"],
        "measure" => vec!["Body", "Measurements"],
        "asym" => vec!["Asymmetry"],
        "armslegs" if name.contains("arm") || name.contains("hand") => {
            let part = if name.contains("hand") {
                "Hands"
            } else {
                "Arms"
            };
            vec!["Limbs", part]
        }
        "armslegs" => {
            let part = if name.contains("foot") {
                "Feet"
            } else {
                "Legs"
            };
            vec!["Limbs", part]
        }
        _ => return None,
    })
}

fn word(w: &str) -> String {
    let mapped = match w {
        "trans" => "move",
        "horiz" => "width",
        "vert" => "height",
        "dist" => "distance",
        "l" => "left",
        "r" => "right",
        "upperarm" => "upper arm",
        "lowerarm" => "forearm",
        "upperleg" => "thigh",
        "lowerleg" => "calf",
        "lowerlegs" => "lower legs",
        "decr" => "less",
        "incr" => "more",
        other => other,
    };
    mapped.to_owned()
}

fn humanize(tokens: &[&str]) -> String {
    let s = tokens.iter().map(|t| word(t)).collect::<Vec<_>>().join(" ");
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// Drop leading tokens that just repeat the category (e.g. "nose-" in nose/).
fn label_tokens<'a>(folder: &str, tokens: &'a [&'a str]) -> &'a [&'a str] {
    let stem = folder.trim_end_matches('s');
    let mut t = tokens;
    while t.len() > 1 && (t[0] == folder || t[0] == stem || folder.starts_with(t[0])) {
        t = &t[1..];
    }
    t
}

fn mirror_of(id: &str) -> Option<String> {
    let (folder, rest) = id.split_once('/')?;
    let swapped = if let Some(r) = rest.strip_prefix("l-") {
        format!("r-{r}")
    } else {
        format!("l-{}", rest.strip_prefix("r-")?)
    };
    Some(format!("{folder}/{swapped}"))
}

struct Loader {
    targets_dir: PathBuf,
    remap: Vec<u32>,
    targets: Vec<MorphTarget>,
}

impl Loader {
    /// Load `rel` (relative to `targets/`, without extension) as target `name`.
    /// Returns false if the file is missing or moves no body vertex.
    fn load(&mut self, rel: &str, name: &str) -> Result<bool> {
        let path = self.targets_dir.join(format!("{rel}.target"));
        let Ok(src) = fs::read_to_string(&path) else {
            return Ok(false);
        };
        let (mut indices, mut deltas) = (Vec::new(), Vec::new());
        for (i, d) in target::parse(&src, &path.to_string_lossy())? {
            match self.remap.get(i as usize) {
                Some(&n) if n != u32::MAX => {
                    indices.push(n);
                    deltas.push(d * UNIT_SCALE);
                }
                _ => {}
            }
        }
        if indices.is_empty() {
            return Ok(false);
        }
        self.targets.push(MorphTarget {
            name: name.into(),
            indices,
            deltas,
        });
        Ok(true)
    }
}

/// `root` is a MakeHuman `data` directory.
pub fn import(root: &Path) -> Result<CharacterAsset> {
    let obj_path = root.join("3dobjs/base.obj");
    let src = fs::read_to_string(&obj_path)
        .map_err(|e| IoError::Invalid(format!("{}: {e}", obj_path.display())))?;
    let obj = obj::parse(&src, "base.obj")?;
    let body = obj
        .group_index("body")
        .ok_or_else(|| IoError::Invalid("base.obj has no `body` group".into()))?;

    // Keep only vertices used by body faces, in original order.
    let mut remap = vec![u32::MAX; obj.positions.len()];
    let body_faces: Vec<&Vec<u32>> = obj
        .faces
        .iter()
        .filter(|(_, g)| *g == body)
        .map(|(f, _)| f)
        .collect();
    for f in &body_faces {
        for &v in *f {
            remap[v as usize] = 0;
        }
    }
    let mut positions = Vec::new();
    for (old, slot) in remap.iter_mut().enumerate() {
        if *slot == 0 {
            *slot = positions.len() as u32;
            positions.push(obj.positions[old] * UNIT_SCALE);
        }
    }
    let mut indices = Vec::new();
    for f in body_faces {
        for k in 1..f.len() - 1 {
            indices.extend([f[0], f[k], f[k + 1]].map(|v| remap[v as usize]));
        }
    }
    let base = Mesh::new(positions, indices).map_err(|e| IoError::Invalid(e.to_string()))?;

    let mut loader = Loader {
        targets_dir: root.join("targets"),
        remap,
        targets: Vec::new(),
    };
    let mut catalog = Catalog::default();
    add_macros(&mut loader, &mut catalog)?;
    add_sliders(&mut loader, &mut catalog)?;

    Ok(CharacterAsset {
        meta: AssetMeta {
            name: "MakeHuman hm08 base (body)".into(),
            source: "https://github.com/makehumancommunity/makehuman (data/3dobjs, data/targets)"
                .into(),
            license: "CC0-1.0".into(),
        },
        mesh: MorphableMesh {
            base,
            targets: loader.targets,
        },
        catalog,
    })
}

fn macro_param(id: &str, label: &str, min: f32, default: f32, ends: [&str; 2]) -> ParamDef {
    ParamDef {
        id: id.into(),
        label: label.into(),
        group: vec!["Body".into(), "Full Body".into()],
        min,
        max: 1.0,
        default,
        mirror: None,
        ends: Some(ends.map(String::from)),
    }
}

fn add_macros(loader: &mut Loader, catalog: &mut Catalog) -> Result<()> {
    const GENDER: [&str; 2] = ["female", "male"];
    const AGE: [&str; 4] = ["baby", "child", "young", "old"];
    const LEVEL: [&str; 3] = ["min", "average", "max"];
    let axis = |id: &str, stops: &[f32]| MacroAxis {
        id: id.into(),
        stops: stops.to_vec(),
    };
    let gender = axis("gender", &[0.0, 1.0]);
    // Stops chosen so 0.5 is a young adult and the range reads roughly 1–90 years.
    let age = axis("age", &[0.0, 0.1875, 0.5, 1.0]);
    let muscle = axis("muscle", &[0.0, 0.5, 1.0]);
    let weight = axis("weight", &[0.0, 0.5, 1.0]);

    catalog.params.extend([
        macro_param("gender", "Gender", 0.0, 0.5, ["feminine", "masculine"]),
        macro_param("age", "Age", 0.0, 0.5, ["infant", "elderly"]),
        macro_param("muscle", "Muscle", 0.0, 0.5, ["slight", "muscular"]),
        macro_param("weight", "Weight", 0.0, 0.5, ["thin", "heavy"]),
        macro_param("height", "Height", -1.0, 0.0, ["short", "tall"]),
    ]);

    // Ancestry shape layers: an even blend gives a neutral reference body.
    let races = ["african", "asian", "caucasian"];
    for race in races {
        let mut corners = Vec::new();
        for (g, gn) in GENDER.iter().enumerate() {
            for (a, an) in AGE.iter().enumerate() {
                let name = format!("macro/{race}-{gn}-{an}");
                if loader.load(&format!("macrodetails/{race}-{gn}-{an}"), &name)? {
                    corners.push((vec![g, a], name));
                }
            }
        }
        catalog.layers.push(MacroLayer {
            grid: MacroGrid {
                axes: vec![gender.clone(), age.clone()],
                corners,
            },
            gain: Gain::Constant(1.0 / races.len() as f32),
        });
    }

    // Muscle/weight layer, plus height layers that depend on the same 4 axes.
    let axes4 = vec![gender, age, muscle, weight];
    let mut universal = Vec::new();
    let mut tall = Vec::new();
    let mut short = Vec::new();
    for (g, gn) in GENDER.iter().enumerate() {
        for (a, an) in AGE.iter().enumerate() {
            for (m, mn) in LEVEL.iter().enumerate() {
                for (w, wn) in LEVEL.iter().enumerate() {
                    let key = vec![g, a, m, w];
                    let stem = format!("{gn}-{an}-{mn}muscle-{wn}weight");
                    let name = format!("macro/universal-{stem}");
                    if loader.load(&format!("macrodetails/universal-{stem}"), &name)? {
                        universal.push((key.clone(), name));
                    }
                    for (suffix, list) in [("maxheight", &mut tall), ("minheight", &mut short)] {
                        let name = format!("macro/height-{stem}-{suffix}");
                        if loader.load(&format!("macrodetails/height/{stem}-{suffix}"), &name)? {
                            list.push((key.clone(), name));
                        }
                    }
                }
            }
        }
    }
    for (corners, gain) in [
        (universal, Gain::One),
        (tall, Gain::Positive("height".into())),
        (short, Gain::Negative("height".into())),
    ] {
        catalog.layers.push(MacroLayer {
            grid: MacroGrid {
                axes: axes4.clone(),
                corners,
            },
            gain,
        });
    }
    Ok(())
}

fn add_sliders(loader: &mut Loader, catalog: &mut Catalog) -> Result<()> {
    let mut folders: Vec<String> = fs::read_dir(&loader.targets_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    folders.sort();

    for folder in folders {
        let mut names: Vec<String> = fs::read_dir(loader.targets_dir.join(&folder))?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                e.file_name()
                    .to_string_lossy()
                    .strip_suffix(".target")
                    .map(String::from)
            })
            // Body-type dependent variants (e.g. breast cup grids) are not simple sliders.
            .filter(|n| !n.starts_with("female-") && !n.starts_with("male-"))
            .collect();
        names.sort();

        // Group by everything but the last token.
        let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for n in &names {
            let (prefix, suffix) = n.rsplit_once('-').unwrap_or(("", n));
            groups
                .entry(prefix.to_owned())
                .or_default()
                .push(suffix.to_owned());
        }

        for (prefix, mut suffixes) in groups {
            let Some(cat) = category(&folder, &prefix) else {
                continue;
            };
            let group: Vec<String> = cat.iter().map(|s| s.to_string()).collect();
            let full = |s: &str| {
                if prefix.is_empty() {
                    s.to_owned()
                } else {
                    format!("{prefix}-{s}")
                }
            };
            let tokens: Vec<&str> = prefix.split('-').filter(|t| !t.is_empty()).collect();
            let label = humanize(label_tokens(&folder, &tokens));

            for &(neg, pos) in PAIRS {
                let (Some(ni), Some(_)) = (
                    suffixes.iter().position(|s| s == neg),
                    suffixes.iter().position(|s| s == pos),
                ) else {
                    continue;
                };
                suffixes.remove(ni);
                suffixes.retain(|s| s != pos);
                let id = format!("{folder}/{prefix}-{neg}-{pos}");
                let (nn, pn) = (
                    format!("{folder}/{}", full(neg)),
                    format!("{folder}/{}", full(pos)),
                );
                let loaded_n = loader.load(&format!("{folder}/{}", full(neg)), &nn)?;
                let loaded_p = loader.load(&format!("{folder}/{}", full(pos)), &pn)?;
                if !(loaded_n || loaded_p) {
                    continue;
                }
                catalog.sliders.push(Slider::Bipolar {
                    id: id.clone(),
                    negative: nn,
                    positive: pn,
                });
                catalog.params.push(ParamDef {
                    mirror: mirror_of(&id),
                    id,
                    label: label.clone(),
                    group: group.clone(),
                    min: -1.0,
                    max: 1.0,
                    default: 0.0,
                    ends: Some([word(neg), word(pos)]),
                });
            }
            for s in suffixes {
                let id = format!("{folder}/{}", full(&s));
                if !loader.load(&id, &id)? {
                    continue;
                }
                let mut t = tokens.clone();
                t.push(&s);
                catalog.sliders.push(Slider::Unipolar {
                    id: id.clone(),
                    target: id.clone(),
                });
                catalog.params.push(ParamDef {
                    mirror: mirror_of(&id),
                    id,
                    label: humanize(label_tokens(&folder, &t)),
                    group: group.clone(),
                    min: 0.0,
                    max: 1.0,
                    default: 0.0,
                    ends: None,
                });
            }
        }
    }
    Ok(())
}

/// Sanity helper used by tests and the CLI: positions' bounding box.
pub fn bounds(p: &[Vec3]) -> (Vec3, Vec3) {
    p.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(lo, hi), &v| (lo.min(v), hi.max(v)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_mirrors() {
        assert_eq!(humanize(label_tokens("nose", &["nose", "trans"])), "Move");
        assert_eq!(
            humanize(label_tokens("armslegs", &["l", "upperarm", "fat"])),
            "Left upper arm fat"
        );
        assert_eq!(
            mirror_of("armslegs/l-foot-scale-decr-incr").as_deref(),
            Some("armslegs/r-foot-scale-decr-incr")
        );
        assert_eq!(mirror_of("nose/nose-trans-down-up"), None);
    }
}
