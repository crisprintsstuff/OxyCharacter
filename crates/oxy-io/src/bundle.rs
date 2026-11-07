//! `.oxyb` — OxyCharacter's compact binary container for a [`CharacterAsset`].
//!
//! Layout (all little-endian):
//! ```text
//! b"OXYB"  u32 version  u32 header_len  header (JSON)
//! base positions  [f32; 3] × vertex_count
//! base indices    u32 × index_count
//! per target:     u32 × len (vertex indices), then [f32; 3] × len (deltas)
//! ```
//! Parsing hundreds of text targets takes seconds; this loads in milliseconds.

use std::io::{Read, Write};

use glam::Vec3;
use oxy_mesh::{Mesh, MorphTarget, MorphableMesh};
use oxy_morph::Catalog;
use serde::{Deserialize, Serialize};

use crate::{AssetMeta, CharacterAsset, IoError, Result};

const MAGIC: &[u8; 4] = b"OXYB";
pub const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Header {
    meta: AssetMeta,
    catalog: Catalog,
    vertex_count: usize,
    index_count: usize,
    targets: Vec<(String, usize)>,
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
            .map(|t| (t.name.clone(), t.indices.len()))
            .collect(),
    };
    let json = serde_json::to_vec(&header)?;
    w.write_all(MAGIC)?;
    w.write_all(&VERSION.to_le_bytes())?;
    w.write_all(&(json.len() as u32).to_le_bytes())?;
    w.write_all(&json)?;
    put_vec3s(&mut w, &m.base.positions)?;
    put_u32s(&mut w, &m.base.indices)?;
    for t in &m.targets {
        put_u32s(&mut w, &t.indices)?;
        put_vec3s(&mut w, &t.deltas)?;
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
    fn u32s(&mut self, n: usize) -> Result<Vec<u32>> {
        Ok(self
            .take(n * 4)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect())
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
    if version != VERSION {
        return Err(IoError::Invalid(format!(
            "unsupported bundle version {version}"
        )));
    }
    let len = c.u32()? as usize;
    let header: Header = serde_json::from_slice(c.take(len)?)?;
    let positions = c.vec3s(header.vertex_count)?;
    let indices = c.u32s(header.index_count)?;
    let base = Mesh::new(positions, indices).map_err(|e| IoError::Invalid(e.to_string()))?;
    let mut targets = Vec::with_capacity(header.targets.len());
    for (name, n) in header.targets {
        let indices = c.u32s(n)?;
        if indices.iter().any(|&i| i as usize >= base.positions.len()) {
            return Err(IoError::Invalid(format!(
                "target {name} indexes past the mesh"
            )));
        }
        targets.push(MorphTarget {
            name,
            indices,
            deltas: c.vec3s(n)?,
        });
    }
    Ok(CharacterAsset {
        meta: header.meta,
        mesh: MorphableMesh { base, targets },
        catalog: header.catalog,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let base = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![0, 1, 2]).unwrap();
        let t = MorphTarget {
            name: "t".into(),
            indices: vec![2],
            deltas: vec![Vec3::Z],
        };
        let asset = CharacterAsset {
            meta: AssetMeta {
                name: "tri".into(),
                ..Default::default()
            },
            mesh: MorphableMesh {
                base,
                targets: vec![t],
            },
            catalog: Catalog::default(),
        };
        let mut bytes = Vec::new();
        write(&asset, &mut bytes).unwrap();
        let back = read(bytes.as_slice()).unwrap();
        assert_eq!(back.meta.name, "tri");
        assert_eq!(back.mesh.base.indices, vec![0, 1, 2]);
        assert_eq!(back.mesh.targets[0].deltas, vec![Vec3::Z]);
        assert!(read(&bytes[..bytes.len() - 1]).is_err());
    }
}
