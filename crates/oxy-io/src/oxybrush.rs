//! The OxyBrush bridge: send a character to OxyBrush, bring sculpts back.
//!
//! **Send** writes the current (morphed, floor-standing) body as an `.oxb`
//! piece, plus any accessories, and a small *link* file next to it
//! (`<file>.oxylink.json`) recording the parameters the body was made with.
//! OxyBrush drops metadata it doesn't know on save, so the link lives beside
//! the project rather than inside it.
//!
//! **Import** classifies every piece of an `.oxb`:
//! * same connectivity as the base body (found by [`match_topology`], since
//!   OxyBrush reorders vertices) → a **morph**: the per-vertex difference from
//!   the reference body, as a sparse target in base-mesh vertex order. If the
//!   piece has subdivision levels, level 0 is used (finer detail is not
//!   transferable to the base mesh).
//! * anything else → an **accessory** mesh (props, hair, clothing, …).
//!
//! The reference body is rebuilt from the link's parameters when available,
//! so later slider changes in OxyCharacter don't leak into the morph.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use glam::Vec3;
use oxy_mesh::correspond::match_topology;
use serde::{Deserialize, Serialize};

use crate::oxb::{self, Piece, Scene};
use crate::{IoError, Result};

/// Param/target id prefix of sculpted (custom) morphs.
pub const CUSTOM_PREFIX: &str = "custom/";

/// Name of the body piece in files we send.
pub const BODY_PIECE: &str = "OxyCharacter Body";
/// Vertex displacements below this (metres) are treated as unchanged.
pub const MIN_DELTA: f32 = 1e-5;

/// Sidecar metadata for a sent file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Link {
    pub params: BTreeMap<String, f32>,
    /// Vertical offset applied to stand the body on the floor (added to y).
    pub ground_offset: f32,
    pub vertex_count: usize,
}

pub fn link_path(oxb: &Path) -> PathBuf {
    let mut s = oxb.as_os_str().to_owned();
    s.push(".oxylink.json");
    PathBuf::from(s)
}

pub fn read_link(oxb: &Path) -> Option<Link> {
    serde_json::from_slice(&std::fs::read(link_path(oxb)).ok()?).ok()
}

/// A mesh carried alongside the body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Accessory {
    pub name: String,
    pub positions: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    /// sRGB vertex color; empty when unpainted.
    pub color: Vec<Vec3>,
}

/// Write `body` (+ accessories) to `path` with its link file.
pub fn send(
    path: &Path,
    body: &[Vec3],
    triangles: &[[u32; 3]],
    accessories: &[Accessory],
    link: &Link,
) -> Result<()> {
    let mut pieces = vec![Piece::new(BODY_PIECE, body.to_vec(), triangles.to_vec())];
    for a in accessories {
        let mut p = Piece::new(a.name.clone(), a.positions.clone(), a.triangles.clone());
        p.color = a.color.clone();
        pieces.push(p);
    }
    oxb::save(
        &Scene {
            pieces,
            active: 0,
            app: String::new(),
        },
        path,
    )?;
    oxb::write_atomic(&link_path(path), &serde_json::to_vec_pretty(link)?)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Imported {
    /// A reshaped body: sparse deltas in base-mesh vertex order.
    Morph {
        piece: String,
        indices: Vec<u32>,
        deltas: Vec<Vec3>,
        max_displacement: f32,
        used_base_level: bool,
    },
    /// Same topology as the body and not moved.
    Unchanged {
        piece: String,
    },
    Accessory(Accessory),
}

impl Imported {
    pub fn piece_name(&self) -> &str {
        match self {
            Imported::Morph { piece, .. } | Imported::Unchanged { piece } => piece,
            Imported::Accessory(a) => &a.name,
        }
    }
}

/// Classify every piece of `scene` against the body topology.
/// `reference` is the body (in base vertex order) the sculpt started from.
pub fn classify(scene: &Scene, base_triangles: &[[u32; 3]], reference: &[Vec3]) -> Vec<Imported> {
    scene
        .pieces
        .iter()
        .map(|p| {
            let (pos, tris) = p.base_mesh();
            let mapping = vmap_correspondence(p, base_triangles, reference.len())
                .or_else(|| match_topology(reference, base_triangles, pos, tris));
            match mapping {
                Some(map) => {
                    let (mut indices, mut deltas) = (Vec::new(), Vec::new());
                    let mut max_displacement = 0.0f32;
                    for (a, &b) in map.iter().enumerate() {
                        let d = pos[b as usize] - reference[a];
                        if d.length() > MIN_DELTA {
                            max_displacement = max_displacement.max(d.length());
                            indices.push(a as u32);
                            deltas.push(d);
                        }
                    }
                    if indices.is_empty() {
                        Imported::Unchanged {
                            piece: p.name.clone(),
                        }
                    } else {
                        Imported::Morph {
                            piece: p.name.clone(),
                            indices,
                            deltas,
                            max_displacement,
                            used_base_level: p.levels.is_some(),
                        }
                    }
                }
                None => Imported::Accessory(Accessory {
                    name: p.name.clone(),
                    positions: p.positions.clone(),
                    triangles: p.triangles.clone(),
                    color: p.color.clone(),
                }),
            }
        })
        .collect()
}

/// Exact correspondence from an OxyBrush `VMAP` section (`source[b] = a`),
/// accepted only if it is a bijection that maps the piece's triangles onto
/// the base topology. Returns `result[a] = b` like [`match_topology`].
pub fn vmap_correspondence(
    p: &Piece,
    base_triangles: &[[u32; 3]],
    vertex_count: usize,
) -> Option<Vec<u32>> {
    if p.levels.is_some()
        || p.source.len() != vertex_count
        || p.positions.len() != vertex_count
        || p.triangles.len() != base_triangles.len()
    {
        return None;
    }
    let mut map = vec![u32::MAX; vertex_count];
    for (b, &a) in p.source.iter().enumerate() {
        let slot = map.get_mut(a as usize)?;
        if *slot != u32::MAX {
            return None; // not a bijection
        }
        *slot = b as u32;
    }
    // Every piece triangle, mapped back, must be a base triangle with the same winding.
    let canon = |t: [u32; 3]| {
        let k = (0..3).min_by_key(|&i| t[i]).unwrap();
        [t[k], t[(k + 1) % 3], t[(k + 2) % 3]]
    };
    let base: std::collections::HashSet<[u32; 3]> =
        base_triangles.iter().map(|&t| canon(t)).collect();
    p.triangles
        .iter()
        .all(|t| base.contains(&canon(t.map(|i| p.source[i as usize]))))
        .then_some(map)
}

// ------------------------------------------------------- custom morph files

/// A user morph saved as a sparse target file (`<id>.target`) whose header
/// carries the label: `# oxycharacter-morph label=<label>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomMorph {
    pub id: String,
    pub label: String,
    pub indices: Vec<u32>,
    pub deltas: Vec<Vec3>,
}

/// Turn a label into a file-system-safe id.
pub fn slug(label: &str) -> String {
    let s: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let s = s
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if s.is_empty() {
        "morph".into()
    } else {
        s
    }
}

pub fn save_custom_morph(dir: &Path, m: &CustomMorph) -> Result<PathBuf> {
    let mut text = format!(
        "# oxycharacter-morph label={}\n# sparse deltas in metres: index dx dy dz\n",
        m.label.replace('\n', " ")
    );
    for (i, d) in m.indices.iter().zip(&m.deltas) {
        text.push_str(&format!("{i} {} {} {}\n", d.x, d.y, d.z));
    }
    let path = dir.join(format!("{}.target", m.id));
    oxb::write_atomic(&path, text.as_bytes())?;
    Ok(path)
}

/// Load every custom morph in `dir` (missing folder = none). Files that fail
/// to parse or index past `vertex_count` are reported, not fatal.
pub fn load_custom_morphs(dir: &Path, vertex_count: usize) -> (Vec<CustomMorph>, Vec<String>) {
    let (mut ok, mut errors) = (Vec::new(), Vec::new());
    let Ok(rd) = std::fs::read_dir(dir) else {
        return (ok, errors);
    };
    let mut paths: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "target"))
        .collect();
    paths.sort();
    for path in paths {
        let id = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let res = std::fs::read_to_string(&path)
            .map_err(IoError::from)
            .and_then(|src| {
                let label = src
                    .lines()
                    .find_map(|l| l.strip_prefix("# oxycharacter-morph label="))
                    .map(|s| s.trim().to_owned())
                    .unwrap_or_else(|| id.clone());
                let t = crate::target::parse(&src, &path.to_string_lossy())?;
                if t.iter().any(|(i, _)| *i as usize >= vertex_count) {
                    return Err(IoError::Invalid("vertex index past the base mesh".into()));
                }
                let (indices, deltas) = t.into_iter().unzip();
                Ok(CustomMorph {
                    id: id.clone(),
                    label,
                    indices,
                    deltas,
                })
            });
        match res {
            Ok(m) => ok.push(m),
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    (ok, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tetra() -> (Vec<Vec3>, Vec<[u32; 3]>) {
        (
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
            vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        )
    }

    #[test]
    fn classifies_reordered_sculpt_and_accessory() {
        let (p, t) = tetra();
        // "OxyBrush" reorders: reverse vertex order, rotate triangle corners.
        let perm = [3u32, 2, 1, 0];
        let mut pb = vec![Vec3::ZERO; 4];
        for (a, &b) in perm.iter().enumerate() {
            pb[b as usize] = p[a];
        }
        pb[perm[3] as usize].z += 0.5; // sculpt vertex 3 upward
        let tb: Vec<[u32; 3]> = t
            .iter()
            .rev()
            .map(|tri| {
                let mut x = tri.map(|i| perm[i as usize]);
                x.rotate_left(1);
                x
            })
            .collect();
        let scene = Scene {
            pieces: vec![
                Piece::new(BODY_PIECE, pb, tb),
                Piece::new("Hat", vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]),
                Piece::new("Copy", p.clone(), t.clone()),
            ],
            ..Default::default()
        };
        let out = classify(&scene, &t, &p);
        match &out[0] {
            Imported::Morph {
                indices, deltas, ..
            } => {
                assert_eq!(indices, &[3]);
                assert!((deltas[0] - Vec3::Z * 0.5).length() < 1e-6);
            }
            other => panic!("expected morph, got {other:?}"),
        }
        assert!(matches!(&out[1], Imported::Accessory(a) if a.name == "Hat"));
        assert!(matches!(&out[2], Imported::Unchanged { .. }));
    }

    #[test]
    fn uses_vmap_when_valid_and_rejects_bad_ones() {
        let (p, t) = tetra();
        // Reverse the vertex order, as OxyBrush might, and record it in VMAP.
        let perm = [3u32, 2, 1, 0]; // new index of old vertex
        let mut piece = Piece::new(
            BODY_PIECE,
            vec![Vec3::ZERO; 4],
            t.iter().map(|tri| tri.map(|i| perm[i as usize])).collect(),
        );
        for (a, &b) in perm.iter().enumerate() {
            piece.positions[b as usize] = p[a] + Vec3::Y * (a as f32);
        }
        piece.source = vec![3, 2, 1, 0];
        let map = vmap_correspondence(&piece, &t, 4).expect("valid vmap");
        assert_eq!(map, perm.to_vec());
        // A VMAP that doesn't fit the triangles (here: flips winding) is
        // ignored, and the matcher is used instead.
        piece.source = vec![2, 3, 1, 0];
        assert!(vmap_correspondence(&piece, &t, 4).is_none());
        piece.source = vec![0, 0, 1, 2];
        assert!(vmap_correspondence(&piece, &t, 4).is_none());
        // classify still produces the right morph either way.
        let scene = Scene {
            pieces: vec![piece],
            ..Default::default()
        };
        match &classify(&scene, &t, &p)[0] {
            Imported::Morph { indices, .. } => assert_eq!(indices, &[1, 2, 3]),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn send_writes_link_and_custom_morphs_roundtrip() {
        let dir = std::env::temp_dir().join(format!("oxy-bridge-{}", std::process::id()));
        let (p, t) = tetra();
        let link = Link {
            params: [("gender".to_string(), 1.0)].into(),
            ground_offset: 0.25,
            vertex_count: 4,
        };
        let path = dir.join("character.oxb");
        send(&path, &p, &t, &[], &link).unwrap();
        assert_eq!(read_link(&path), Some(link));
        let scene = oxb::load(&path).unwrap();
        assert_eq!(scene.pieces[0].name, BODY_PIECE);

        let m = CustomMorph {
            id: slug("Big Nose!"),
            label: "Big Nose!".into(),
            indices: vec![1, 3],
            deltas: vec![Vec3::X, Vec3::Y * 0.5],
        };
        assert_eq!(m.id, "big-nose");
        save_custom_morph(&dir, &m).unwrap();
        let (loaded, errs) = load_custom_morphs(&dir, 4);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(loaded, vec![m]);
        let (_, errs) = load_custom_morphs(&dir, 2);
        assert_eq!(errs.len(), 1);
        std::fs::remove_dir_all(dir).ok();
    }
}
