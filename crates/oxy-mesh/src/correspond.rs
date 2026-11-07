//! Vertex correspondence between two meshes with the same connectivity.
//!
//! Tools such as OxyBrush re-sort vertices and triangles when they load a
//! mesh, so a sculpt that comes back has the topology of our base mesh but a
//! different vertex order (and moved positions). Index-by-index comparison is
//! then meaningless. [`match_topology`] recovers the permutation.
//!
//! Method (our own): pick a seed triangle in A and candidate triangles in B
//! near it (by centroid). From each candidate (in each of its three
//! rotations) flood across shared edges: if A-triangle `(a0, a1, x)` maps to
//! B-triangle `(b0, b1, y)`, the A-triangle on the other side of directed edge
//! `a0→a1` (it contains `a1→a0`) must map to the B-triangle containing
//! `b1→b0`, which fixes its third vertex. Any contradiction rejects the
//! candidate. Orientation is preserved, so mirror-image matches of a
//! symmetric mesh cannot succeed. Among complete, consistent matches the one
//! with the smallest median vertex displacement wins. Disconnected parts are
//! seeded one after another.

use std::collections::HashMap;

use glam::Vec3;

/// For each directed edge, the triangle containing it and its third vertex.
fn edge_map(tris: &[[u32; 3]]) -> HashMap<(u32, u32), (u32, u32)> {
    let mut m = HashMap::with_capacity(tris.len() * 3);
    for (t, &[a, b, c]) in tris.iter().enumerate() {
        m.insert((a, b), (t as u32, c));
        m.insert((b, c), (t as u32, a));
        m.insert((c, a), (t as u32, b));
    }
    m
}

struct Side<'a> {
    pos: &'a [Vec3],
    tris: &'a [[u32; 3]],
    edges: HashMap<(u32, u32), (u32, u32)>,
}

/// Result of flooding one connected region.
struct Flood {
    /// (A vertex, B vertex) pairs assigned by this flood.
    pairs: Vec<(u32, u32)>,
    tris: Vec<(u32, u32)>,
    median_dist: f32,
}

/// What earlier regions already matched.
struct Claimed {
    /// A vertex → B vertex (`u32::MAX` when free).
    va: Vec<u32>,
    /// B vertex used.
    ub: Vec<bool>,
    /// A / B triangle used.
    ta: Vec<bool>,
    tb: Vec<bool>,
}

fn flood(a: &Side, b: &Side, seed_a: [u32; 3], seed_b: [u32; 3], c: &Claimed) -> Option<Flood> {
    let (va, ub, ta, tb) = (&c.va, &c.ub, &c.ta, &c.tb);
    let mut map: HashMap<u32, u32> = HashMap::new();
    let mut used: HashMap<u32, u32> = HashMap::new();
    let mut tri_pairs: Vec<(u32, u32)> = Vec::new();
    let mut seen_a: HashMap<u32, ()> = HashMap::new();
    let mut seen_b: HashMap<u32, ()> = HashMap::new();
    let tri_of = |s: &Side, v: [u32; 3]| s.edges.get(&(v[0], v[1])).map(|e| e.0);

    let mut queue = vec![(seed_a, seed_b)];
    while let Some((ra, rb)) = queue.pop() {
        let (Some(t_a), Some(t_b)) = (tri_of(a, ra), tri_of(b, rb)) else {
            return None;
        };
        if seen_a.contains_key(&t_a) {
            continue;
        }
        if ta[t_a as usize] || tb[t_b as usize] || seen_b.contains_key(&t_b) {
            return None;
        }
        for k in 0..3 {
            let (x, y) = (ra[k], rb[k]);
            // Consistency with this flood and with earlier regions.
            let prev_a = map
                .get(&x)
                .copied()
                .or((va[x as usize] != u32::MAX).then(|| va[x as usize]));
            if prev_a.is_some_and(|p| p != y) {
                return None;
            }
            let prev_b = used.get(&y).copied();
            if prev_b.is_some_and(|p| p != x)
                || (prev_b.is_none() && prev_a.is_none() && ub[y as usize])
            {
                return None;
            }
            map.insert(x, y);
            used.insert(y, x);
        }
        seen_a.insert(t_a, ());
        seen_b.insert(t_b, ());
        tri_pairs.push((t_a, t_b));
        for k in 0..3 {
            let (a0, a1) = (ra[k], ra[(k + 1) % 3]);
            let (b0, b1) = (rb[k], rb[(k + 1) % 3]);
            match (a.edges.get(&(a1, a0)), b.edges.get(&(b1, b0))) {
                (Some(&(_, xa)), Some(&(_, xb))) => queue.push(([a1, a0, xa], [b1, b0, xb])),
                (None, None) => {} // open border in both
                _ => return None,  // border in one mesh only
            }
        }
    }
    let mut d: Vec<f32> = map
        .iter()
        .map(|(&x, &y)| a.pos[x as usize].distance(b.pos[y as usize]))
        .collect();
    d.sort_by(f32::total_cmp);
    let median_dist = d.get(d.len() / 2).copied().unwrap_or(0.0);
    let pairs = map
        .into_iter()
        .filter(|&(x, _)| va[x as usize] == u32::MAX)
        .collect();
    Some(Flood {
        pairs,
        tris: tri_pairs,
        median_dist,
    })
}

/// Map every vertex of A to its counterpart in B (`result[a] = b`), or `None`
/// if the meshes don't share connectivity. Positions only guide seed choice
/// and tie-breaking, so B may be heavily sculpted.
pub fn match_topology(
    pos_a: &[Vec3],
    tris_a: &[[u32; 3]],
    pos_b: &[Vec3],
    tris_b: &[[u32; 3]],
) -> Option<Vec<u32>> {
    if pos_a.len() != pos_b.len() || tris_a.len() != tris_b.len() || tris_a.is_empty() {
        return None;
    }
    let a = Side {
        pos: pos_a,
        tris: tris_a,
        edges: edge_map(tris_a),
    };
    let b = Side {
        pos: pos_b,
        tris: tris_b,
        edges: edge_map(tris_b),
    };
    let centroid =
        |s: &Side, t: usize| s.tris[t].iter().map(|&i| s.pos[i as usize]).sum::<Vec3>() / 3.0;
    let cent_b: Vec<Vec3> = (0..tris_b.len()).map(|t| centroid(&b, t)).collect();

    let mut c = Claimed {
        va: vec![u32::MAX; pos_a.len()],
        ub: vec![false; pos_b.len()],
        ta: vec![false; tris_a.len()],
        tb: vec![false; tris_b.len()],
    };
    const SEEDS: usize = 6;
    const NEAREST: usize = 4;

    while let Some(first_free) = c.ta.iter().position(|&d| !d) {
        // Spread seed attempts over the remaining triangles.
        let free: Vec<usize> = (first_free..tris_a.len()).filter(|&t| !c.ta[t]).collect();
        let mut best: Option<Flood> = None;
        for s in 0..SEEDS.min(free.len()) {
            let t_a = free[s * free.len() / SEEDS.min(free.len())];
            let seed_c = centroid(&a, t_a);
            let mut near: Vec<(f32, usize)> = (0..tris_b.len())
                .filter(|&t| !c.tb[t])
                .map(|t| (cent_b[t].distance_squared(seed_c), t))
                .collect();
            let k = NEAREST.min(near.len());
            if k == 0 {
                return None;
            }
            near.select_nth_unstable_by(k - 1, |x, y| x.0.total_cmp(&y.0));
            for &(_, t_b) in &near[..k] {
                let [b0, b1, b2] = tris_b[t_b];
                for rb in [[b0, b1, b2], [b1, b2, b0], [b2, b0, b1]] {
                    if let Some(f) = flood(&a, &b, tris_a[t_a], rb, &c) {
                        if best.as_ref().is_none_or(|bst| {
                            f.tris.len() > bst.tris.len()
                                || (f.tris.len() == bst.tris.len()
                                    && f.median_dist < bst.median_dist)
                        }) {
                            best = Some(f);
                        }
                    }
                }
            }
            // A region matched from two different seeds is trustworthy.
            if s >= 1 && best.is_some() {
                break;
            }
        }
        let f = best?;
        for (x, y) in f.pairs {
            c.va[x as usize] = y;
            c.ub[y as usize] = true;
        }
        for (x, y) in f.tris {
            c.ta[x as usize] = true;
            c.tb[y as usize] = true;
        }
    }
    c.va.iter().all(|&v| v != u32::MAX).then_some(c.va)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Open `n×n` grid in the XY plane, two triangles per cell.
    fn grid(n: u32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
        let mut p = Vec::new();
        for y in 0..=n {
            for x in 0..=n {
                p.push(Vec3::new(x as f32, y as f32, 0.0));
            }
        }
        let mut t = Vec::new();
        let id = |x: u32, y: u32| y * (n + 1) + x;
        for y in 0..n {
            for x in 0..n {
                t.push([id(x, y), id(x + 1, y), id(x + 1, y + 1)]);
                t.push([id(x, y), id(x + 1, y + 1), id(x, y + 1)]);
            }
        }
        (p, t)
    }

    fn shuffle<T>(v: &mut [T], seed: &mut u64) {
        for i in (1..v.len()).rev() {
            *seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            v.swap(i, (*seed >> 33) as usize % (i + 1));
        }
    }

    /// Permute vertices and triangles, rotate corners, and "sculpt" positions.
    fn scramble(p: &[Vec3], t: &[[u32; 3]], seed: u64) -> (Vec<Vec3>, Vec<[u32; 3]>, Vec<u32>) {
        let mut s = seed;
        let mut perm: Vec<u32> = (0..p.len() as u32).collect();
        shuffle(&mut perm, &mut s); // perm[a] = b
        let mut pb = vec![Vec3::ZERO; p.len()];
        for (a, &b) in perm.iter().enumerate() {
            let q = p[a];
            pb[b as usize] = q + Vec3::new(0.0, 0.0, (q.x * 0.7).sin() * 0.8); // a sculpted bump
        }
        let mut tb: Vec<[u32; 3]> = t.iter().map(|tri| tri.map(|i| perm[i as usize])).collect();
        shuffle(&mut tb, &mut s);
        for (i, tri) in tb.iter_mut().enumerate() {
            tri.rotate_left(i % 3);
        }
        (pb, tb, perm)
    }

    #[test]
    fn recovers_permutation_of_sculpted_mesh() {
        let (p, t) = grid(12);
        let (pb, tb, perm) = scramble(&p, &t, 7);
        assert_eq!(match_topology(&p, &t, &pb, &tb), Some(perm));
    }

    #[test]
    fn handles_disconnected_parts() {
        let (mut p, mut t) = grid(4);
        let (p2, t2) = grid(3);
        let off = p.len() as u32;
        p.extend(p2.iter().map(|q| *q + Vec3::new(20.0, 0.0, 0.0)));
        t.extend(t2.iter().map(|tri| tri.map(|i| i + off)));
        let (pb, tb, perm) = scramble(&p, &t, 99);
        assert_eq!(match_topology(&p, &t, &pb, &tb), Some(perm));
    }

    #[test]
    fn rejects_different_topology() {
        let (p, t) = grid(6);
        let (pb, mut tb, _) = scramble(&p, &t, 3);
        // Flip one diagonal: same counts, different connectivity.
        let i = tb.iter().position(|_| true).unwrap();
        tb[i].swap(1, 2);
        assert_eq!(match_topology(&p, &t, &pb, &tb), None);
        let (p2, t2) = grid(7);
        assert_eq!(match_topology(&p, &t, &p2, &t2), None);
    }
}
