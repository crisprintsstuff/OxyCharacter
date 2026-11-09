//! Bounding-volume hierarchy over a triangle mesh and closest-point queries.
//!
//! The closest point on a triangle is the standard region-based construction
//! (vertex, edge or face Voronoi region). See Christer Ericson, *Real-Time
//! Collision Detection* (2005), §5.1.5 — reimplemented here, not copied.

use glam::Vec3;

#[derive(Clone, Copy)]
struct Aabb {
    min: Vec3,
    max: Vec3,
}

impl Aabb {
    fn point(p: Vec3) -> Self {
        Self { min: p, max: p }
    }
    fn tri(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self {
            min: a.min(b).min(c),
            max: a.max(b).max(c),
        }
    }
    fn merge(self, o: Self) -> Self {
        Self {
            min: self.min.min(o.min),
            max: self.max.max(o.max),
        }
    }
    fn dist_sq(self, p: Vec3) -> f32 {
        let mut d = 0.0;
        for i in 0..3 {
            let v = p[i];
            if v < self.min[i] {
                let t = self.min[i] - v;
                d += t * t;
            } else if v > self.max[i] {
                let t = v - self.max[i];
                d += t * t;
            }
        }
        d
    }
}

struct Node {
    bounds: Aabb,
    /// Child node indices when `count == 0`.
    left: u32,
    right: u32,
    /// Start of this leaf in `order`.
    start: u32,
    /// Triangle count. Zero marks an internal node.
    count: u32,
}

/// Closest point of a query on the mesh.
#[derive(Debug, Clone, Copy)]
pub struct Closest {
    pub point: Vec3,
    /// Triangle index (not a vertex index).
    pub triangle: u32,
    /// Barycentric coordinates of `point` in that triangle (sum to 1).
    pub bary: [f32; 3],
    pub dist_sq: f32,
}

/// A BVH over a triangle list. Positions and indices are copied so the
/// structure stays valid after the caller mutates its own buffers.
pub struct Bvh {
    nodes: Vec<Node>,
    /// Triangle indices in leaf order.
    order: Vec<u32>,
    positions: Vec<Vec3>,
    indices: Vec<u32>,
    root: usize,
}

const LEAF: usize = 8;

impl Bvh {
    pub fn build(positions: &[Vec3], indices: &[u32]) -> Self {
        let n = indices.len() / 3;
        let mut entries: Vec<(u32, Vec3, Aabb)> = (0..n)
            .map(|t| {
                let [a, b, c] = corners(positions, indices, t);
                let bounds = Aabb::tri(a, b, c);
                let centroid = (a + b + c) / 3.0;
                (t as u32, centroid, bounds)
            })
            .collect();
        let mut nodes = Vec::new();
        let mut order = Vec::with_capacity(n);
        let root = if n == 0 {
            0
        } else {
            build_range(&mut entries, 0, n, &mut nodes, &mut order)
        };
        Self {
            nodes,
            order,
            positions: positions.to_vec(),
            indices: indices.to_vec(),
            root,
        }
    }

    pub fn closest(&self, p: Vec3) -> Option<Closest> {
        if self.nodes.is_empty() {
            return None;
        }
        let mut best = Closest {
            point: p,
            triangle: 0,
            bary: [1.0, 0.0, 0.0],
            dist_sq: f32::MAX,
        };
        self.query(self.root, p, &mut best);
        (best.dist_sq < f32::MAX).then_some(best)
    }

    fn query(&self, node: usize, p: Vec3, best: &mut Closest) {
        let n = &self.nodes[node];
        if n.bounds.dist_sq(p) > best.dist_sq {
            return;
        }
        if n.count > 0 {
            for k in 0..n.count {
                let ti = self.order[(n.start + k) as usize] as usize;
                let [a, b, c] = corners(&self.positions, &self.indices, ti);
                let (q, bary) = closest_on_triangle(p, a, b, c);
                let d = q.distance_squared(p);
                if d < best.dist_sq {
                    *best = Closest {
                        point: q,
                        triangle: ti as u32,
                        bary,
                        dist_sq: d,
                    };
                }
            }
        } else {
            let (l, r) = (n.left as usize, n.right as usize);
            let dl = self.nodes[l].bounds.dist_sq(p);
            let dr = self.nodes[r].bounds.dist_sq(p);
            if dl < dr {
                self.query(l, p, best);
                self.query(r, p, best);
            } else {
                self.query(r, p, best);
                self.query(l, p, best);
            }
        }
    }
}

fn build_range(
    entries: &mut [(u32, Vec3, Aabb)],
    start: usize,
    end: usize,
    nodes: &mut Vec<Node>,
    order: &mut Vec<u32>,
) -> usize {
    let mut bounds = Aabb::point(entries[start].1);
    for e in &entries[start..end] {
        bounds = bounds.merge(e.2);
    }
    if end - start <= LEAF {
        let node = nodes.len();
        let leaf_start = order.len() as u32;
        for e in &entries[start..end] {
            order.push(e.0);
        }
        nodes.push(Node {
            bounds,
            left: 0,
            right: 0,
            start: leaf_start,
            count: (end - start) as u32,
        });
        return node;
    }
    let extent = bounds.max - bounds.min;
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    entries[start..end].sort_by(|a, b| a.1[axis].total_cmp(&b.1[axis]));
    let mid = (start + end) / 2;
    let left = build_range(entries, start, mid, nodes, order);
    let right = build_range(entries, mid, end, nodes, order);
    let node = nodes.len();
    nodes.push(Node {
        bounds,
        left: left as u32,
        right: right as u32,
        start: 0,
        count: 0,
    });
    node
}

fn corners(positions: &[Vec3], indices: &[u32], tri: usize) -> [Vec3; 3] {
    let b = tri * 3;
    [0, 1, 2].map(|k| positions[indices[b + k] as usize])
}

/// Closest point on triangle `abc` and its barycentric coordinates.
pub fn closest_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> (Vec3, [f32; 3]) {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, [1.0, 0.0, 0.0]);
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (b, [0.0, 1.0, 0.0]);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (a + ab * v, [1.0 - v, v, 0.0]);
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (c, [0.0, 0.0, 1.0]);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (a + ac * w, [1.0 - w, 0.0, w]);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (b + (c - b) * w, [0.0, 1.0 - w, w]);
    }
    let denom = va + vb + vc;
    if denom.abs() < 1e-20 {
        return (a, [1.0, 0.0, 0.0]);
    }
    let v = vb / denom;
    let w = vc / denom;
    let u = 1.0 - v - w;
    (a + ab * v + ac * w, [u, v, w])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closest_point_hits_vertex_edge_and_face() {
        let (a, b, c) = (Vec3::ZERO, Vec3::X, Vec3::Y);
        let (p, bary) = closest_on_triangle(Vec3::new(-1.0, -1.0, 0.0), a, b, c);
        assert!(p.distance(a) < 1e-5 && bary[0] > 0.99);
        let (p, bary) = closest_on_triangle(Vec3::new(0.5, -1.0, 0.0), a, b, c);
        assert!(p.distance(Vec3::new(0.5, 0.0, 0.0)) < 1e-5);
        assert!(bary[2].abs() < 1e-5 && (bary[0] - 0.5).abs() < 1e-4);
        let (p, bary) = closest_on_triangle(Vec3::new(0.2, 0.2, 3.0), a, b, c);
        assert!(p.distance(Vec3::new(0.2, 0.2, 0.0)) < 1e-5);
        assert!((bary[0] + bary[1] + bary[2] - 1.0).abs() < 1e-5);
        assert!(bary.iter().all(|w| *w >= -1e-5));
    }

    #[test]
    fn bvh_matches_a_brute_force_scan() {
        let positions = vec![
            Vec3::ZERO,
            Vec3::X,
            Vec3::Y,
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.2, 0.2, 1.0),
            Vec3::new(2.0, 0.0, 0.5),
        ];
        let indices = vec![0, 1, 2, 1, 3, 2, 2, 3, 4, 1, 5, 3];
        let bvh = Bvh::build(&positions, &indices);
        let queries = [
            Vec3::new(0.2, 0.2, 0.4),
            Vec3::new(-1.0, 0.5, 0.2),
            Vec3::new(3.0, 3.0, 3.0),
            Vec3::new(0.5, 0.1, -0.5),
            Vec3::new(1.5, 0.2, 0.4),
        ];
        for q in queries {
            let hit = bvh.closest(q).unwrap();
            let mut best = f32::MAX;
            for t in 0..indices.len() / 3 {
                let [a, b, c] = corners(&positions, &indices, t);
                let (p, _) = closest_on_triangle(q, a, b, c);
                best = best.min(p.distance_squared(q));
            }
            assert!(
                (hit.dist_sq - best).abs() < 1e-5,
                "query {q:?}: bvh {} vs brute {best}",
                hit.dist_sq
            );
        }
    }
}
