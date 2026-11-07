//! OxyBrush project files (`.oxb`, format version 1), read and write.
//!
//! An independent implementation of the format OxyBrush documents in its
//! `oxy-doc` crate, so OxyCharacter does not depend on the sculpting app:
//!
//! ```text
//! file    := magic:"OXYBRUSH" version:u32 flags:u32 count:u32 section*count
//! section := tag:[u8;4] raw_len:u64 packed_len:u64 crc32(raw):u32 lz4_block(raw)
//! META    := UTF-8 `key=value` lines (`app`, `active`)
//! PIEC    := name_len:u32 name visible:u8 v:u32 t:u32 positions:f32*3v
//!            triangles:u32*3t mask:f32*v groups:u32*t hidden:u8*t
//! MRES    := (levels of the preceding PIEC) levels:u32 active:u32 t0:u32
//!            base_triangles:u32*3t0, then per level v:u32 positions:f32*3v
//! VCOL    := (vertex color of the preceding PIEC) v:u32 rgb:f32*3v (sRGB)
//! VMAP    := (vertex origin of the preceding PIEC, OxyBrush ≥ 2.5.8) v:u32
//!            source:u32*v — vertex i came from vertex source[i] of the file
//!            the piece was first loaded from. Only written while topology is
//!            unchanged since load and the piece has no MRES.
//! ```
//!
//! All integers little-endian. Unknown sections are skipped.
//!
//! Note: OxyBrush re-sorts vertices and triangles for memory locality when it
//! loads a mesh, so a file coming back from OxyBrush has the same connectivity
//! but a different vertex order. See [`oxy_mesh::correspond`].

use std::io::Write;
use std::path::Path;

use glam::Vec3;

use crate::{IoError, Result};

pub const MAGIC: &[u8; 8] = b"OXYBRUSH";
pub const VERSION: u32 = 1;
const MAX_SECTION_BYTES: u64 = 4 << 30;

/// Subdivision levels of a piece. Level 0 is the base mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct Levels {
    pub base_triangles: Vec<[u32; 3]>,
    pub positions: Vec<Vec<Vec3>>,
    pub active: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Piece {
    pub name: String,
    pub visible: bool,
    pub positions: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    pub mask: Vec<f32>,
    pub groups: Vec<u32>,
    pub hidden: Vec<bool>,
    /// sRGB vertex color, empty when unpainted.
    pub color: Vec<Vec3>,
    pub levels: Option<Levels>,
    /// VMAP: original index of each vertex in the file first loaded, or empty.
    pub source: Vec<u32>,
}

impl Piece {
    pub fn new(name: impl Into<String>, positions: Vec<Vec3>, triangles: Vec<[u32; 3]>) -> Self {
        Self {
            name: name.into(),
            visible: true,
            positions,
            triangles,
            ..Default::default()
        }
    }

    /// The lowest-resolution version of this piece: subdivision level 0 when
    /// the piece has levels, otherwise the mesh itself.
    pub fn base_mesh(&self) -> (&[Vec3], &[[u32; 3]]) {
        match &self.levels {
            Some(l) => (&l.positions[0], &l.base_triangles),
            None => (&self.positions, &self.triangles),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    pub pieces: Vec<Piece>,
    pub active: usize,
    /// The `app` META value of the writer, e.g. `oxybrush 2.5.6-beta`.
    pub app: String,
}

// ---------------------------------------------------------------- writing

fn put_u32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}

fn put_vec3s(b: &mut Vec<u8>, v: &[Vec3]) {
    for p in v {
        for c in p.to_array() {
            b.extend_from_slice(&c.to_le_bytes());
        }
    }
}

fn put_tris(b: &mut Vec<u8>, t: &[[u32; 3]]) {
    for tri in t {
        for &i in tri {
            put_u32(b, i);
        }
    }
}

pub fn encode(scene: &Scene) -> Vec<u8> {
    let mut sections: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    let meta = format!(
        "app=oxycharacter {}\nactive={}\n",
        env!("CARGO_PKG_VERSION"),
        scene.active
    );
    sections.push((*b"META", meta.into_bytes()));
    for p in &scene.pieces {
        let (v, t) = (p.positions.len(), p.triangles.len());
        let mut b = Vec::with_capacity(16 + p.name.len() + v * 16 + t * 17);
        put_u32(&mut b, p.name.len() as u32);
        b.extend_from_slice(p.name.as_bytes());
        b.push(u8::from(p.visible));
        put_u32(&mut b, v as u32);
        put_u32(&mut b, t as u32);
        put_vec3s(&mut b, &p.positions);
        put_tris(&mut b, &p.triangles);
        for i in 0..v {
            b.extend_from_slice(&p.mask.get(i).copied().unwrap_or(0.0).to_le_bytes());
        }
        for i in 0..t {
            put_u32(&mut b, p.groups.get(i).copied().unwrap_or(0));
        }
        for i in 0..t {
            b.push(u8::from(p.hidden.get(i).copied().unwrap_or(false)));
        }
        sections.push((*b"PIEC", b));
        if let Some(l) = &p.levels {
            let mut b = Vec::new();
            put_u32(&mut b, l.positions.len() as u32);
            put_u32(&mut b, l.active as u32);
            put_u32(&mut b, l.base_triangles.len() as u32);
            put_tris(&mut b, &l.base_triangles);
            for pos in &l.positions {
                put_u32(&mut b, pos.len() as u32);
                put_vec3s(&mut b, pos);
            }
            sections.push((*b"MRES", b));
        }
        if p.source.len() == v && v > 0 && p.levels.is_none() {
            let mut b = Vec::with_capacity(4 + v * 4);
            put_u32(&mut b, v as u32);
            for &i in &p.source {
                put_u32(&mut b, i);
            }
            sections.push((*b"VMAP", b));
        }
        if p.color.len() == v && v > 0 {
            let mut b = Vec::with_capacity(4 + v * 12);
            put_u32(&mut b, v as u32);
            put_vec3s(&mut b, &p.color);
            sections.push((*b"VCOL", b));
        }
    }

    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(sections.len() as u32).to_le_bytes());
    for (tag, raw) in sections {
        let packed = lz4_flex::block::compress(&raw);
        out.extend_from_slice(&tag);
        out.extend_from_slice(&(raw.len() as u64).to_le_bytes());
        out.extend_from_slice(&(packed.len() as u64).to_le_bytes());
        out.extend_from_slice(&crc32fast::hash(&raw).to_le_bytes());
        out.extend_from_slice(&packed);
    }
    out
}

/// Write atomically (temp file in the same folder, then rename), so a file
/// OxyBrush is watching or has open is never seen half-written.
pub fn save(scene: &Scene, path: &Path) -> Result<()> {
    write_atomic(path, &encode(scene))
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(())
}

// ---------------------------------------------------------------- reading

fn corrupt(msg: impl Into<String>) -> IoError {
    IoError::Invalid(format!("damaged .oxb file: {}", msg.into()))
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.data.len())
            .ok_or_else(|| corrupt("unexpected end of data"))?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    /// Refuse counts the remaining bytes cannot hold, before allocating.
    fn check(&self, count: usize, size: usize) -> Result<()> {
        match count.checked_mul(size) {
            Some(n) if n <= self.data.len() - self.pos => Ok(()),
            _ => Err(corrupt("unexpected end of data")),
        }
    }
    fn vec3s(&mut self, n: usize) -> Result<Vec<Vec3>> {
        self.check(n, 12)?;
        (0..n)
            .map(|_| {
                let v = Vec3::new(self.f32()?, self.f32()?, self.f32()?);
                if v.is_finite() {
                    Ok(v)
                } else {
                    Err(corrupt("non-finite coordinate"))
                }
            })
            .collect()
    }
    fn tris(&mut self, n: usize, vcount: usize) -> Result<Vec<[u32; 3]>> {
        self.check(n, 12)?;
        (0..n)
            .map(|_| {
                let t = [self.u32()?, self.u32()?, self.u32()?];
                if t.iter().any(|&i| i as usize >= vcount) {
                    return Err(corrupt("triangle index out of range"));
                }
                Ok(t)
            })
            .collect()
    }
}

pub fn decode(data: &[u8]) -> Result<Scene> {
    let mut r = Reader { data, pos: 0 };
    if data.len() < 8 || r.take(8)? != MAGIC {
        return Err(IoError::Invalid("not an OxyBrush project file".into()));
    }
    let version = r.u32()?;
    if version > VERSION {
        return Err(IoError::Invalid(format!(
            "OxyBrush project format version {version} is newer than this OxyCharacter supports ({VERSION})"
        )));
    }
    let _flags = r.u32()?;
    let count = r.u32()?;
    let mut scene = Scene::default();
    for _ in 0..count {
        let tag: [u8; 4] = r.take(4)?.try_into().unwrap();
        let raw_len = r.u64()?;
        let packed_len = r.u64()?;
        let crc = r.u32()?;
        if raw_len > MAX_SECTION_BYTES {
            return Err(corrupt("section too large"));
        }
        let packed =
            r.take(usize::try_from(packed_len).map_err(|_| corrupt("section too large"))?)?;
        let raw = lz4_flex::block::decompress(packed, raw_len as usize)
            .map_err(|e| corrupt(format!("decompression failed: {e}")))?;
        if crc32fast::hash(&raw) != crc {
            return Err(corrupt(format!(
                "checksum mismatch in {} section",
                String::from_utf8_lossy(&tag)
            )));
        }
        let mut s = Reader { data: &raw, pos: 0 };
        match &tag {
            b"META" => {
                for line in String::from_utf8_lossy(&raw).lines() {
                    if let Some(v) = line.strip_prefix("active=") {
                        scene.active = v.trim().parse().unwrap_or(0);
                    } else if let Some(v) = line.strip_prefix("app=") {
                        scene.app = v.trim().to_owned();
                    }
                }
            }
            b"PIEC" => {
                let name_len = s.u32()? as usize;
                let name = String::from_utf8_lossy(s.take(name_len)?).into_owned();
                let visible = s.u8()? != 0;
                let v = s.u32()? as usize;
                let t = s.u32()? as usize;
                s.check(v, 16)?;
                s.check(t, 17)?;
                let positions = s.vec3s(v)?;
                let triangles = s.tris(t, v)?;
                let mask = (0..v).map(|_| s.f32()).collect::<Result<_>>()?;
                let groups = (0..t).map(|_| s.u32()).collect::<Result<_>>()?;
                let hidden = (0..t)
                    .map(|_| s.u8().map(|b| b != 0))
                    .collect::<Result<_>>()?;
                scene.pieces.push(Piece {
                    name,
                    visible,
                    positions,
                    triangles,
                    mask,
                    groups,
                    hidden,
                    color: Vec::new(),
                    levels: None,
                    source: Vec::new(),
                });
            }
            b"MRES" => {
                let n = s.u32()? as usize;
                let active = s.u32()? as usize;
                if n == 0 || n > 16 || active >= n {
                    return Err(corrupt("bad subdivision level header"));
                }
                let t = s.u32()? as usize;
                // Indices are validated against level 0 once it is read.
                let base_triangles = s.tris(t, usize::MAX)?;
                let mut positions = Vec::with_capacity(n);
                for _ in 0..n {
                    let v = s.u32()? as usize;
                    positions.push(s.vec3s(v)?);
                }
                if base_triangles
                    .iter()
                    .flatten()
                    .any(|&i| i as usize >= positions[0].len())
                {
                    return Err(corrupt("subdivision base triangle out of range"));
                }
                let piece = scene
                    .pieces
                    .last_mut()
                    .ok_or_else(|| corrupt("MRES before any piece"))?;
                // Like OxyBrush: levels that don't describe the saved mesh are dropped, not fatal.
                if positions[active].len() == piece.positions.len() {
                    piece.levels = Some(Levels {
                        base_triangles,
                        positions,
                        active,
                    });
                }
            }
            b"VCOL" => {
                let piece = scene
                    .pieces
                    .last_mut()
                    .ok_or_else(|| corrupt("VCOL before any piece"))?;
                let n = s.u32()? as usize;
                if n == piece.positions.len() {
                    piece.color = s.vec3s(n)?;
                }
            }
            b"VMAP" => {
                let piece = scene
                    .pieces
                    .last_mut()
                    .ok_or_else(|| corrupt("VMAP before any piece"))?;
                let n = s.u32()? as usize;
                s.check(n, 4)?;
                let source: Vec<u32> = (0..n).map(|_| s.u32()).collect::<Result<_>>()?;
                // Ignored (not fatal) when it doesn't describe the piece.
                if n == piece.positions.len() {
                    piece.source = source;
                }
            }
            _ => {} // Unknown sections are skipped by design.
        }
    }
    if scene.pieces.is_empty() {
        return Err(corrupt("no pieces"));
    }
    Ok(scene)
}

pub fn load(path: &Path) -> Result<Scene> {
    decode(&std::fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Scene {
        let mut a = Piece::new(
            "Body",
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
            vec![[0, 1, 2], [0, 2, 3]],
        );
        a.mask = vec![0.0, 0.5, 1.0, 0.25];
        a.groups = vec![1, 2];
        a.hidden = vec![false, true];
        a.color = vec![Vec3::ONE, Vec3::X, Vec3::Y, Vec3::Z];
        let mut b = Piece::new("Hat ✓", vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
        b.visible = false;
        a.source = vec![3, 2, 1, 0];
        (b.mask, b.groups, b.hidden) = (vec![0.0; 3], vec![0], vec![false]);
        b.levels = Some(Levels {
            base_triangles: vec![[0, 1, 2]],
            positions: vec![b.positions.clone()],
            active: 0,
        });
        Scene {
            pieces: vec![a, b],
            active: 1,
            app: String::new(),
        }
    }

    #[test]
    fn roundtrip() {
        let s = sample();
        let mut back = decode(&encode(&s)).unwrap();
        assert!(back.app.starts_with("oxycharacter"));
        back.app.clear();
        assert_eq!(back, s);
        assert_eq!(back.pieces[1].base_mesh().1, &[[0, 1, 2]]);
    }

    #[test]
    fn rejects_damage_and_skips_unknown_sections() {
        let mut bytes = encode(&sample());
        let n = bytes.len();
        assert!(decode(&bytes[..n / 2]).is_err());
        assert!(decode(b"definitely not").is_err());
        let mut future = bytes.clone();
        future[8..12].copy_from_slice(&9u32.to_le_bytes());
        assert!(decode(&future).is_err());

        let count = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) + 1;
        bytes[16..20].copy_from_slice(&count.to_le_bytes());
        let raw = b"later";
        bytes.extend_from_slice(b"XTRA");
        bytes.extend_from_slice(&(raw.len() as u64).to_le_bytes());
        let packed = lz4_flex::block::compress(raw);
        bytes.extend_from_slice(&(packed.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&crc32fast::hash(raw).to_le_bytes());
        bytes.extend_from_slice(&packed);
        assert_eq!(decode(&bytes).unwrap().pieces.len(), 2);

        bytes[n - 3] ^= 0xff;
        assert!(decode(&bytes).is_err());
    }
}
