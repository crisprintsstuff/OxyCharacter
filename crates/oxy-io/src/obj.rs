//! Minimal Wavefront OBJ reader: `v`, `vt`, `f` and `g`/`o` records; everything
//! else is ignored.

use glam::Vec3;

use crate::{IoError, Result};

#[derive(Debug, Clone, Default)]
pub struct ObjData {
    pub positions: Vec<Vec3>,
    /// Texture coordinates as stored in the file (v up).
    pub uvs: Vec<[f32; 2]>,
    /// Polygon faces (0-based position indices) and the group index of each.
    pub faces: Vec<(Vec<u32>, usize)>,
    /// Per face, the 0-based `vt` index of each corner (empty if the face has none).
    pub face_uvs: Vec<Vec<u32>>,
    pub groups: Vec<String>,
}

impl ObjData {
    pub fn group_index(&self, name: &str) -> Option<usize> {
        self.groups.iter().position(|g| g == name)
    }

    /// Fan-triangulate every face, keeping the file's winding.
    pub fn triangles(&self) -> Vec<[u32; 3]> {
        let mut out = Vec::new();
        for (face, _) in &self.faces {
            for i in 1..face.len().saturating_sub(1) {
                out.push([face[0], face[i], face[i + 1]]);
            }
        }
        out
    }
}

fn index(t: &str, len: usize) -> Option<u32> {
    let i: i64 = t.parse().ok()?;
    // OBJ indices are 1-based; negatives are relative to the end.
    let i = if i < 0 { len as i64 + i } else { i - 1 };
    u32::try_from(i).ok()
}

pub fn parse(src: &str, file: &str) -> Result<ObjData> {
    let mut out = ObjData::default();
    let mut current = 0usize;
    let err = |line: usize, msg: &str| IoError::Parse {
        file: file.into(),
        line,
        msg: msg.into(),
    };
    for (n, line) in src.lines().enumerate() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let mut c = [0f32; 3];
                for v in &mut c {
                    *v = it
                        .next()
                        .and_then(|t| t.parse().ok())
                        .ok_or_else(|| err(n + 1, "bad vertex"))?;
                }
                out.positions.push(Vec3::from_array(c));
            }
            Some("vt") => {
                let mut c = [0f32; 2];
                for v in &mut c {
                    *v = it
                        .next()
                        .and_then(|t| t.parse().ok())
                        .ok_or_else(|| err(n + 1, "bad texture coordinate"))?;
                }
                out.uvs.push(c);
            }
            Some("g") | Some("o") => {
                let name = it.collect::<Vec<_>>().join(" ");
                current = match out.group_index(&name) {
                    Some(i) => i,
                    None => {
                        out.groups.push(name);
                        out.groups.len() - 1
                    }
                };
            }
            Some("f") => {
                if out.groups.is_empty() {
                    out.groups.push("default".into());
                }
                let (mut verts, mut uvs) = (Vec::new(), Vec::new());
                for t in it {
                    let mut parts = t.split('/');
                    let v = parts
                        .next()
                        .and_then(|p| index(p, out.positions.len()))
                        .ok_or_else(|| err(n + 1, "bad face index"))?;
                    if v as usize >= out.positions.len() {
                        return Err(err(n + 1, "face index out of range"));
                    }
                    verts.push(v);
                    if let Some(p) = parts.next().filter(|p| !p.is_empty()) {
                        let vt = index(p, out.uvs.len())
                            .filter(|&i| (i as usize) < out.uvs.len())
                            .ok_or_else(|| err(n + 1, "bad texture index"))?;
                        uvs.push(vt);
                    }
                }
                if verts.len() < 3 {
                    return Err(err(n + 1, "face with fewer than 3 vertices"));
                }
                if !uvs.is_empty() && uvs.len() != verts.len() {
                    return Err(err(
                        n + 1,
                        "face mixes corners with and without texture coordinates",
                    ));
                }
                out.faces.push((verts, current));
                out.face_uvs.push(uvs);
            }
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_groups_uvs_and_slashes() {
        let src = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\ng body\nf 1/1 2/2 3/3 4/4\ng other\nf -1 -2 -3\n";
        let o = super::parse(src, "t.obj").unwrap();
        assert_eq!(o.positions.len(), 4);
        assert_eq!(o.uvs.len(), 4);
        assert_eq!(o.groups, vec!["body", "other"]);
        assert_eq!(o.faces[0], (vec![0, 1, 2, 3], 0));
        assert_eq!(o.face_uvs[0], vec![0, 1, 2, 3]);
        assert_eq!(o.faces[1], (vec![3, 2, 1], 1));
        assert_eq!(o.triangles(), vec![[0, 1, 2], [0, 2, 3], [3, 2, 1]]);
        assert!(o.face_uvs[1].is_empty());
        assert!(super::parse("v 0 0 0\nf 1 1 9\n", "x").is_err());
        assert!(super::parse("v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nf 1/1 2 3\n", "x").is_err());
    }
}
