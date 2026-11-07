//! `.oxyb` — OxyCharacter's compact binary container for a [`CharacterAsset`].
//!
//! Layout (all little-endian), version 2:
//! ```text
//! b"OXYB"  u32 version  u32 header_len  header (JSON: meta, catalog, rig, counts)
//! base positions  [f32; 3] × vertex_count
//! base indices    u32 × index_count
//! aux points      [f32; 3] × aux_count
//! per target:     u32 × len, [f32; 3] × len        (body deltas)
//!                 u32 × aux_len, [f32; 3] × aux_len (aux deltas)
//! ```
//! Version 1 (no aux points, no rig) is still read.
//! Parsing hundreds of text targets takes seconds; this loads in milliseconds.

use std::io::{Read, Write};

use glam::Vec3;
use oxy_mesh::{Mesh, MorphTarget, MorphableMesh};
use oxy_morph::Catalog;
use oxy_rig::Rig;
use serde::{Deserialize, Serialize};

use crate::{AssetMeta, CharacterAsset, IoError, Result};

const MAGIC: &[u8; 4] = b"OXYB";
pub const VERSION: u32 = 2;

#[derive(Serialize, Deserialize)]
struct Header {
    meta: AssetMeta,
    catalog: Catalog,
    vertex_count: usize,
    index_count: usize,
    /// (name, body delta count, aux delta count); v1 has no third field.
    targets: Vec<TargetHeader>,
    #[serde(default)]
    aux_count: usize,
    #[serde(default)]
    rig: Option<Rig>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum TargetHeader {
    V2(String, usize, usize),
    V1(String, usize),
}

fn put_u32s(w: &mut impl Write, v: &[u32]) -> Result<()> {
    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    Ok(w.write_all(&bytes)?)
}

fn put_vec3s(w: &mut impl Write, v: &[Vec3]) -> Result<()> {
    let bytes: Vec<u8> = v
        .iter()
        .flat_map(|p| p.to_array())
        .flat_map(f32::to_le_bytes)
        .collect();
    Ok(w.write_all(&bytes)?)
}

pub fn write(asset: &CharacterAsset, mut w: impl Write) -> Result<()> {
    let m = &asset.mesh;
    let header = Header {
        meta: asset.meta.clone(),
        catalog: asset.catalog.clone(),
        vertex_count: m.base.positions.len(),
        index_count: m.base.indices.len(),
        targets: m
            .targets
            .iter()
            .map(|t| TargetHeader::V2(t.name.clone(), t.indices.len(), t.aux_indices.len()))
            .collect(),
        aux_count: m.aux.len(),
        rig: asset.rig.clone(),
    };
    let json = serde_json::to_vec(&header)?;
    w.write_all(MAGIC)?;
    w.write_all(&VERSION.to_le_bytes())?;
    w.write_all(&(json.len() as u32).to_le_bytes())?;
    w.write_all(&json)?;
    put_vec3s(&mut w, &m.base.positions)?;
    put_u32s(&mut w, &m.base.indices)?;
    put_vec3s(&mut w, &m.aux)?;
    for t in &m.targets {
        put_u32s(&mut w, &t.indices)?;
        put_vec3s(&mut w, &t.deltas)?;
        put_u32s(&mut w, &t.aux_indices)?;
        put_vec3s(&mut w, &t.aux_deltas)?;
    }
    Ok(())
}

struct Cursor<'a>(&'a [u8]);

impl Cursor<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        if self.0.len() < n {
            return Err(IoError::Invalid("bundle truncated".into()));
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u32s(&mut self, n: usize, bound: usize, what: &str) -> Result<Vec<u32>> {
        let v: Vec<u32> = self
            .take(n * 4)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect();
        if v.iter().any(|&i| i as usize >= bound) {
            return Err(IoError::Invalid(format!("{what} index out of range")));
        }
        Ok(v)
    }
    fn vec3s(&mut self, n: usize) -> Result<Vec<Vec3>> {
        Ok(self
            .take(n * 12)?
            .as_chunks::<12>()
            .0
            .iter()
            .map(|c| {
                let f = |i: usize| f32::from_le_bytes(c[i..i + 4].try_into().unwrap());
                Vec3::new(f(0), f(4), f(8))
            })
            .collect())
    }
}

pub fn read(mut r: impl Read) -> Result<CharacterAsset> {
    let mut buf = Vec::new();
    r.read_to_end(&mut buf)?;
    let mut c = Cursor(&buf);
    if c.take(4)? != MAGIC {
        return Err(IoError::Invalid("not an .oxyb bundle".into()));
    }
    let version = c.u32()?;
    if version == 0 || version > VERSION {
        return Err(IoError::Invalid(format!(
            "unsupported bundle version {version} (rebuild it with tools/fetch-assets.sh)"
        )));
    }
    let len = c.u32()? as usize;
    let header: Header = serde_json::from_slice(c.take(len)?)?;
    let positions = c.vec3s(header.vertex_count)?;
    let indices = c.u32s(header.index_count, header.vertex_count, "triangle")?;
    let base = Mesh::new(positions, indices).map_err(|e| IoError::Invalid(e.to_string()))?;
    let n = base.positions.len();
    let aux = if version >= 2 {
        c.vec3s(header.aux_count)?
    } else {
        Vec::new()
    };
    let mut targets = Vec::with_capacity(header.targets.len());
    for th in header.targets {
        let (name, len, aux_len) = match th {
            TargetHeader::V2(n, l, a) => (n, l, a),
            TargetHeader::V1(n, l) => (n, l, 0),
        };
        let mut t = MorphTarget::new(name, c.u32s(len, n, "target vertex")?, c.vec3s(len)?);
        if version >= 2 {
            t.aux_indices = c.u32s(aux_len, aux.len(), "target aux")?;
            t.aux_deltas = c.vec3s(aux_len)?;
        }
        targets.push(t);
    }
    if let Some(rig) = &header.rig {
        rig.validate(n, aux.len())
            .map_err(|e| IoError::Invalid(format!("bundle rig: {e}")))?;
    }
    Ok(CharacterAsset {
        meta: header.meta,
        mesh: MorphableMesh { base, targets, aux },
        catalog: header.catalog,
        rig: header.rig,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxy_rig::{Bone, Influence, Joint};

    #[test]
    fn round_trip_with_aux_and_rig() {
        let base = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![0, 1, 2]).unwrap();
        let mut t = MorphTarget::new("t", vec![2], vec![Vec3::Z]);
        t.aux_indices = vec![0];
        t.aux_deltas = vec![Vec3::X];
        let rig = Rig {
            name: "r".into(),
            joints: vec![
                Joint {
                    name: "a".into(),
                    anchor: vec![(0, 1.0)],
                },
                Joint {
                    name: "b".into(),
                    anchor: vec![(3, 1.0)],
                },
            ],
            bones: vec![Bone {
                name: "bone".into(),
                parent: None,
                head: 0,
                tail: 1,
                roll_plane: None,
            }],
            skin: vec![Influence::from_pairs(vec![(0, 1.0)]); 3],
            aux_from_body: vec![vec![(0, 1.0)]],
        };
        let asset = CharacterAsset {
            meta: AssetMeta {
                name: "tri".into(),
                ..Default::default()
            },
            mesh: MorphableMesh {
                base,
                targets: vec![t.clone()],
                aux: vec![Vec3::Y * 2.0],
            },
            catalog: Catalog::default(),
            rig: Some(rig.clone()),
        };
        let mut bytes = Vec::new();
        write(&asset, &mut bytes).unwrap();
        let back = read(bytes.as_slice()).unwrap();
        assert_eq!(back.meta.name, "tri");
        assert_eq!(back.mesh.base.indices, vec![0, 1, 2]);
        assert_eq!(back.mesh.targets[0], t);
        assert_eq!(back.mesh.aux, vec![Vec3::Y * 2.0]);
        assert_eq!(back.rig, Some(rig));
        assert!(read(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn reads_version_one() {
        let header = serde_json::json!({
            "meta": AssetMeta::default(), "catalog": Catalog::default(),
            "vertex_count": 3, "index_count": 3, "targets": [["t", 1]]
        });
        let json = serde_json::to_vec(&header).unwrap();
        let mut b = Vec::new();
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&(json.len() as u32).to_le_bytes());
        b.extend_from_slice(&json);
        put_vec3s(&mut b, &[Vec3::ZERO, Vec3::X, Vec3::Y]).unwrap();
        put_u32s(&mut b, &[0, 1, 2]).unwrap();
        put_u32s(&mut b, &[1]).unwrap();
        put_vec3s(&mut b, &[Vec3::Z]).unwrap();
        let a = read(b.as_slice()).unwrap();
        assert_eq!(a.mesh.targets[0].deltas, vec![Vec3::Z]);
        assert!(a.rig.is_none());
    }
}
