//! Minimal Wavefront OBJ reader: `v`, `f` and `g` records; everything else is ignored.

use glam::Vec3;

use crate::{IoError, Result};

#[derive(Debug, Clone, Default)]
pub struct ObjData {
    pub positions: Vec<Vec3>,
    /// Polygon faces (0-based position indices) and the group index of each.
    pub faces: Vec<(Vec<u32>, usize)>,
    pub groups: Vec<String>,
}

impl ObjData {
    pub fn group_index(&self, name: &str) -> Option<usize> {
        self.groups.iter().position(|g| g == name)
    }
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
                let verts = it
                    .map(|t| {
                        let i: i64 = t
                            .split('/')
                            .next()
                            .and_then(|s| s.parse().ok())
                            .ok_or_else(|| err(n + 1, "bad face index"))?;
                        // OBJ indices are 1-based; negatives are relative to the end.
                        let i = if i < 0 {
                            out.positions.len() as i64 + i
                        } else {
                            i - 1
                        };
                        u32::try_from(i).map_err(|_| err(n + 1, "face index out of range"))
                    })
                    .collect::<Result<Vec<_>>>()?;
                if verts.len() < 3 {
                    return Err(err(n + 1, "face with fewer than 3 vertices"));
                }
                out.faces.push((verts, current));
            }
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_groups_and_slashes() {
        let src =
            "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\ng body\nf 1/1 2/2 3/3 4/4\ng other\nf -1 -2 -3\n";
        let o = super::parse(src, "t.obj").unwrap();
        assert_eq!(o.positions.len(), 4);
        assert_eq!(o.groups, vec!["body", "other"]);
        assert_eq!(o.faces[0], (vec![0, 1, 2, 3], 0));
        assert_eq!(o.faces[1], (vec![3, 2, 1], 1));
    }
}
