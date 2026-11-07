//! Sparse morph-target text format: one `index dx dy dz` record per line,
//! `#` starts a comment.

use glam::Vec3;

use crate::{IoError, Result};

pub fn parse(src: &str, file: &str) -> Result<Vec<(u32, Vec3)>> {
    let mut out = Vec::new();
    for (n, line) in src.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.split_whitespace();
        let bad = || IoError::Parse {
            file: file.into(),
            line: n + 1,
            msg: "expected `index dx dy dz`".into(),
        };
        let i: u32 = it.next().and_then(|t| t.parse().ok()).ok_or_else(bad)?;
        let mut d = [0f32; 3];
        for v in &mut d {
            *v = it.next().and_then(|t| t.parse().ok()).ok_or_else(bad)?;
        }
        out.push((i, Vec3::from_array(d)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_and_skips_comments() {
        let t = super::parse("# hi\n\n3 0 .5 -1\n7 1 2 3 # tail\n", "x").unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].0, 3);
        assert_eq!(t[0].1.y, 0.5);
        assert!(super::parse("1 2\n", "x").is_err());
    }
}
