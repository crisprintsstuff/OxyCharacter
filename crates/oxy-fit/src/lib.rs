//! Clothing and hair fitting (no GPU).
//!
//! A garment is **bound** once to the body it was authored on: every vertex
//! stores the closest body triangle, the barycentric coordinates of that
//! point, and the offset in the triangle's tangent frame. **Fitting** rebuilds
//! the vertex from the morphed triangle, so the garment follows body changes.
//! Loose items can instead follow a radial-basis displacement of nearby body
//! vertices. Skin weights are transferred from the bind and smoothed across
//! the garment. Triangles the garment fully covers can be hidden so the body
//! does not poke through.
//!
//! See `docs/adr/0009-clothing-and-hair.md`.

mod bvh;
pub mod package;
pub mod wardrobe;

use std::collections::HashMap;

use glam::{IVec3, Vec3};
use oxy_rig::Influence;
use serde::{Deserialize, Serialize};

pub use bvh::{closest_on_triangle, Bvh, Closest};
pub use package::{Manifest, ManifestItem, FORMAT as PACKAGE_FORMAT, VERSION as PACKAGE_VERSION};
pub use wardrobe::{author_builtins, builtin_ids, builtin_name, AuthoredGarment};

/// How a garment follows the body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FitMethod {
    /// Rebuild each vertex from its bound triangle. Shirts, trousers, hair cards.
    #[default]
    Surface,
    /// Displace by a radial-basis field of nearby body vertices. Skirts, coats.
    Loose,
    /// Do not refit. The mesh only skins with the skeleton (props).
    Rigid,
}

/// Garment or hair. Hair uses a strand card material; garments are opaque cloth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    #[default]
    Garment,
    Hair,
}

fn tightness_default() -> f32 {
    0.5
}
fn smooth_default() -> u32 {
    2
}
fn margin_default() -> f32 {
    0.003
}
fn cover_default() -> f32 {
    0.02
}
fn visible_default() -> bool {
    true
}

/// Per-item fitting parameters. `tightness` 0.5 keeps the authored offset;
/// higher pulls the cloth toward the skin, lower lets it stand off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FitSettings {
    pub method: FitMethod,
    #[serde(default = "tightness_default")]
    pub tightness: f32,
    #[serde(default = "smooth_default")]
    pub smooth_steps: u32,
    /// Minimum distance from the body after the push-out, in metres.
    #[serde(default = "margin_default")]
    pub margin: f32,
    /// Hide body triangles this item fully covers.
    pub hide_body: bool,
    /// Higher layers are worn outside lower ones and can hide them.
    pub layer: u16,
    /// A body (or inner-garment) vertex within this distance, on the outside,
    /// counts as covered. Metres.
    #[serde(default = "cover_default")]
    pub cover: f32,
    #[serde(default = "visible_default")]
    pub visible: bool,
}

impl Default for FitSettings {
    fn default() -> Self {
        Self::rigid()
    }
}

impl FitSettings {
    pub fn rigid() -> Self {
        Self {
            method: FitMethod::Rigid,
            tightness: 0.5,
            smooth_steps: 2,
            margin: 0.003,
            hide_body: false,
            layer: 0,
            cover: 0.02,
            visible: true,
        }
    }

    /// Tight clothing: surface fit, hides the body underneath.
    pub fn surface(layer: u16) -> Self {
        Self {
            method: FitMethod::Surface,
            hide_body: true,
            layer,
            ..Self::rigid()
        }
    }

    /// Loose clothing: mostly the radial-basis field, still hides the body.
    pub fn loose(layer: u16) -> Self {
        Self {
            method: FitMethod::Loose,
            tightness: 0.15,
            hide_body: true,
            layer,
            ..Self::rigid()
        }
    }

    pub fn hair() -> Self {
        Self {
            method: FitMethod::Surface,
            smooth_steps: 0,
            margin: 0.004,
            hide_body: false,
            layer: 3,
            cover: 0.012,
            ..Self::rigid()
        }
    }
}

/// One garment vertex bound to a body triangle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VertexBind {
    /// `u32::MAX` when the vertex could not be bound.
    pub triangle: u32,
    pub bary: [f32; 3],
    /// Offset in the triangle frame (tangent, bitangent, normal), metres.
    pub offset: [f32; 3],
}

/// Binding of a whole garment, including the control points used by loose fitting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GarmentBind {
    pub verts: Vec<VertexBind>,
    /// Body vertex indices that drive loose (RBF) fitting.
    pub controls: Vec<u32>,
    /// Positions of `controls` on the body the garment was authored on.
    pub control_rest: Vec<Vec3>,
}

/// A triangle mesh with adjacency for smoothing.
#[derive(Debug, Clone, PartialEq)]
pub struct GarmentMesh {
    pub positions: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub neighbors: Vec<Vec<u32>>,
}

impl GarmentMesh {
    pub fn from_parts(positions: Vec<Vec3>, triangles: Vec<[u32; 3]>, uvs: Vec<[f32; 2]>) -> Self {
        let neighbors = adjacency(positions.len(), &triangles);
        let uvs = if uvs.len() == positions.len() {
            uvs
        } else {
            vec![[0.0, 0.0]; positions.len()]
        };
        Self {
            positions,
            triangles,
            uvs,
            neighbors,
        }
    }

    /// Fan-triangulate polygon faces. `corner_uvs` is empty or one UV per corner.
    pub fn from_polygons(
        positions: Vec<Vec3>,
        faces: &[Vec<u32>],
        corner_uvs: &[Vec<[f32; 2]>],
    ) -> Self {
        let mut weld: HashMap<(u32, [u32; 2]), u32> = HashMap::new();
        let mut out_pos = Vec::new();
        let mut out_uv = Vec::new();
        let mut tris = Vec::new();
        for (f, face) in faces.iter().enumerate() {
            if face.len() < 3 {
                continue;
            }
            let uvs = corner_uvs.get(f).filter(|u| u.len() == face.len());
            let mut ids = Vec::with_capacity(face.len());
            for (k, &v) in face.iter().enumerate() {
                let uv = uvs.map_or([0.0, 0.0], |u| u[k]);
                let key = (v, uv.map(f32::to_bits));
                let id = *weld.entry(key).or_insert_with(|| {
                    let i = out_pos.len() as u32;
                    out_pos.push(positions.get(v as usize).copied().unwrap_or(Vec3::ZERO));
                    out_uv.push(uv);
                    i
                });
                ids.push(id);
            }
            for k in 1..ids.len() - 1 {
                tris.push([ids[0], ids[k], ids[k + 1]]);
            }
        }
        Self::from_parts(out_pos, tris, out_uv)
    }

    pub fn flat_indices(&self) -> Vec<u32> {
        self.triangles.iter().flatten().copied().collect()
    }
}

fn adjacency(n: usize, triangles: &[[u32; 3]]) -> Vec<Vec<u32>> {
    let mut edges = vec![Vec::new(); n];
    for t in triangles {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (a as usize, b as usize);
            if a < n && b < n {
                edges[a].push(b as u32);
                edges[b].push(a as u32);
            }
        }
    }
    for v in &mut edges {
        v.sort_unstable();
        v.dedup();
    }
    edges
}

/// Bind `garment` vertices to the closest point on `body`.
///
/// `body_normals` may be empty; vertex normals are then computed from `body_indices`.
pub fn bind(
    body: &[Vec3],
    body_indices: &[u32],
    body_normals: &[Vec3],
    garment: &[Vec3],
) -> GarmentBind {
    let normals = if body_normals.len() == body.len() {
        body_normals.to_vec()
    } else {
        oxy_mesh::Mesh::new(body.to_vec(), body_indices.to_vec())
            .map(|m| m.compute_normals(body))
            .unwrap_or_else(|_| vec![Vec3::Y; body.len()])
    };
    let bvh = Bvh::build(body, body_indices);
    let verts = garment
        .iter()
        .map(|&p| match bvh.closest(p) {
            Some(hit) => {
                let (a, b, c, na, nb, nc) =
                    tri_frame_inputs(body, body_indices, &normals, hit.triangle);
                let (t, bt, n) = frame(a, b, c, hit.bary, na, nb, nc);
                let d = p - hit.point;
                VertexBind {
                    triangle: hit.triangle,
                    bary: hit.bary,
                    offset: [d.dot(t), d.dot(bt), d.dot(n)],
                }
            }
            None => VertexBind {
                triangle: u32::MAX,
                bary: [1.0, 0.0, 0.0],
                offset: [0.0; 3],
            },
        })
        .collect();
    let controls = pick_controls(body, garment, 48);
    let control_rest = controls
        .iter()
        .map(|&i| body.get(i as usize).copied().unwrap_or(Vec3::ZERO))
        .collect();
    GarmentBind {
        verts,
        controls,
        control_rest,
    }
}

/// Refit `garment` onto `body` (same topology as the body used to bind).
pub fn fit(
    body: &[Vec3],
    body_indices: &[u32],
    body_normals: &[Vec3],
    garment: &GarmentMesh,
    binding: &GarmentBind,
    settings: &FitSettings,
) -> Vec<Vec3> {
    if settings.method == FitMethod::Rigid || binding.verts.len() != garment.positions.len() {
        return garment.positions.clone();
    }
    let normals = if body_normals.len() == body.len() {
        body_normals.to_vec()
    } else {
        oxy_mesh::Mesh::new(body.to_vec(), body_indices.to_vec())
            .map(|m| m.compute_normals(body))
            .unwrap_or_else(|_| vec![Vec3::Y; body.len()])
    };
    let scale = offset_scale(settings.tightness);
    let mut surface = Vec::with_capacity(garment.positions.len());
    for (i, bind) in binding.verts.iter().enumerate() {
        surface.push(placed(
            body,
            body_indices,
            &normals,
            bind,
            scale,
            garment.positions[i],
        ));
    }
    let mut positions = if settings.method == FitMethod::Loose {
        let loose = rbf_positions(garment, binding, body);
        let t = settings.tightness.clamp(0.0, 1.0);
        loose
            .into_iter()
            .zip(surface)
            .map(|(l, s)| l.lerp(s, t))
            .collect()
    } else {
        surface
    };
    relax(&mut positions, &garment.neighbors, settings);
    push_out(
        &mut positions,
        body,
        body_indices,
        &normals,
        settings.margin,
    );
    positions
}

fn offset_scale(tightness: f32) -> f32 {
    let t = tightness.clamp(0.0, 1.0);
    if t >= 0.5 {
        1.0 - (t - 0.5) * 0.9
    } else {
        1.0 + (0.5 - t)
    }
}

fn placed(
    body: &[Vec3],
    indices: &[u32],
    normals: &[Vec3],
    bind: &VertexBind,
    scale: f32,
    fallback: Vec3,
) -> Vec3 {
    let Some((a, b, c, na, nb, nc)) = tri_inputs(body, indices, normals, bind.triangle) else {
        return fallback;
    };
    let (t, bt, n) = frame(a, b, c, bind.bary, na, nb, nc);
    let [ox, oy, oz] = bind.offset;
    let point = a * bind.bary[0] + b * bind.bary[1] + c * bind.bary[2];
    point + (t * ox + bt * oy + n * oz) * scale
}

fn tri_inputs(
    body: &[Vec3],
    indices: &[u32],
    normals: &[Vec3],
    triangle: u32,
) -> Option<(Vec3, Vec3, Vec3, Vec3, Vec3, Vec3)> {
    if triangle == u32::MAX {
        return None;
    }
    let base = triangle as usize * 3;
    if base + 2 >= indices.len() {
        return None;
    }
    let ids = [0, 1, 2].map(|k| indices[base + k] as usize);
    if ids.iter().any(|&i| i >= body.len()) {
        return None;
    }
    let n = |i: usize| {
        normals
            .get(i)
            .copied()
            .filter(|n| n.length_squared() > 0.0)
            .unwrap_or(Vec3::Y)
    };
    Some((
        body[ids[0]],
        body[ids[1]],
        body[ids[2]],
        n(ids[0]),
        n(ids[1]),
        n(ids[2]),
    ))
}

fn tri_frame_inputs(
    body: &[Vec3],
    indices: &[u32],
    normals: &[Vec3],
    triangle: u32,
) -> (Vec3, Vec3, Vec3, Vec3, Vec3, Vec3) {
    tri_inputs(body, indices, normals, triangle).unwrap_or((
        Vec3::ZERO,
        Vec3::X,
        Vec3::Y,
        Vec3::Y,
        Vec3::Y,
        Vec3::Y,
    ))
}

/// Tangent, bitangent, normal. The normal is the barycentric vertex normal;
/// the tangent follows the first well-defined edge, orthogonalised against it.
fn frame(
    a: Vec3,
    b: Vec3,
    c: Vec3,
    bary: [f32; 3],
    na: Vec3,
    nb: Vec3,
    nc: Vec3,
) -> (Vec3, Vec3, Vec3) {
    let mut n = (na * bary[0] + nb * bary[1] + nc * bary[2]).normalize_or_zero();
    if n.length_squared() < 1e-8 {
        n = (b - a).cross(c - a).normalize_or_zero();
    }
    if n.length_squared() < 1e-8 {
        n = Vec3::Y;
    }
    let mut t = b - a;
    t -= n * t.dot(n);
    if t.length_squared() < 1e-10 {
        t = c - a;
        t -= n * t.dot(n);
    }
    t = t.normalize_or_zero();
    if t.length_squared() < 1e-8 {
        t = n.cross(Vec3::Y).normalize_or_zero();
        if t.length_squared() < 1e-8 {
            t = n.cross(Vec3::X).normalize();
        }
    }
    let bt = n.cross(t);
    (t, bt, n)
}

fn relax(positions: &mut [Vec3], neighbors: &[Vec<u32>], settings: &FitSettings) {
    let lambda = (1.0 - settings.tightness.clamp(0.0, 1.0)) * 0.45;
    if lambda < 1e-4 || settings.smooth_steps == 0 {
        return;
    }
    for _ in 0..settings.smooth_steps {
        let prev = positions.to_vec();
        for (i, p) in positions.iter_mut().enumerate() {
            let nbrs = neighbors.get(i).map(Vec::as_slice).unwrap_or(&[]);
            if nbrs.is_empty() {
                continue;
            }
            let avg = nbrs
                .iter()
                .map(|&j| prev.get(j as usize).copied().unwrap_or(prev[i]))
                .sum::<Vec3>()
                / nbrs.len() as f32;
            *p = prev[i].lerp(avg, lambda);
        }
    }
}

fn push_out(positions: &mut [Vec3], body: &[Vec3], indices: &[u32], normals: &[Vec3], margin: f32) {
    if margin <= 0.0 || body.is_empty() || indices.len() < 3 {
        return;
    }
    let bvh = Bvh::build(body, indices);
    for p in positions.iter_mut() {
        let Some(hit) = bvh.closest(*p) else {
            continue;
        };
        let n = {
            let (a, b, c, na, nb, nc) = tri_frame_inputs(body, indices, normals, hit.triangle);
            let (_, _, n) = frame(a, b, c, hit.bary, na, nb, nc);
            let _ = (a, b, c);
            n
        };
        let outside = (*p - hit.point).dot(n);
        if outside < margin {
            *p = hit.point + n * margin;
        }
    }
}

fn rbf_positions(garment: &GarmentMesh, binding: &GarmentBind, body: &[Vec3]) -> Vec<Vec3> {
    let m = binding.controls.len().min(binding.control_rest.len());
    if m == 0 {
        return garment.positions.clone();
    }
    let mut disp = Vec::with_capacity(m);
    for k in 0..m {
        let i = binding.controls[k] as usize;
        let now = body.get(i).copied().unwrap_or(binding.control_rest[k]);
        disp.push(now - binding.control_rest[k]);
    }
    if disp.iter().all(|d| d.length_squared() < 1e-12) {
        return garment.positions.clone();
    }
    let rest = &binding.control_rest[..m];
    let Some(weights) = rbf_weights(rest, &disp) else {
        return garment.positions.clone();
    };
    let sigma = rbf_sigma(rest);
    garment
        .positions
        .iter()
        .map(|p| {
            let mut d = Vec3::ZERO;
            for (k, c) in rest.iter().enumerate() {
                let phi = (-p.distance_squared(*c) / (sigma * sigma)).exp();
                d += weights[k] * phi;
            }
            *p + d
        })
        .collect()
}

fn rbf_sigma(rest: &[Vec3]) -> f32 {
    if rest.len() < 2 {
        return 0.05;
    }
    let mut acc = 0.0;
    for (i, c) in rest.iter().enumerate() {
        let mut best = f32::MAX;
        for (j, o) in rest.iter().enumerate() {
            if i != j {
                best = best.min(c.distance_squared(*o));
            }
        }
        acc += best.sqrt();
    }
    (acc / rest.len() as f32 * 2.0).max(0.02)
}

/// Gaussian RBF weights so the field matches `disp` at the control points.
fn rbf_weights(rest: &[Vec3], disp: &[Vec3]) -> Option<Vec<Vec3>> {
    let n = rest.len();
    let sigma = rbf_sigma(rest);
    let mut a = vec![0.0f32; n * n];
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] = (-rest[i].distance_squared(rest[j]) / (sigma * sigma)).exp();
        }
        a[i * n + i] += 1.0e-3;
    }
    let mut w = vec![Vec3::ZERO; n];
    for axis in 0..3 {
        let b: Vec<f32> = disp.iter().map(|d| d[axis]).collect();
        let x = solve(&a, &b, n)?;
        for (k, v) in x.into_iter().enumerate() {
            w[k][axis] = v;
        }
    }
    Some(w)
}

fn solve(matrix: &[f32], rhs: &[f32], n: usize) -> Option<Vec<f32>> {
    let mut m = matrix.to_vec();
    let mut x = rhs.to_vec();
    for col in 0..n {
        let mut piv = col;
        let mut best = m[col * n + col].abs();
        for row in col + 1..n {
            let v = m[row * n + col].abs();
            if v > best {
                best = v;
                piv = row;
            }
        }
        if best < 1e-8 {
            return None;
        }
        if piv != col {
            for c in 0..n {
                m.swap(col * n + c, piv * n + c);
            }
            x.swap(col, piv);
        }
        let diag = m[col * n + col];
        for row in col + 1..n {
            let f = m[row * n + col] / diag;
            if f == 0.0 {
                continue;
            }
            for c in col..n {
                m[row * n + c] -= f * m[col * n + c];
            }
            x[row] -= f * x[col];
        }
    }
    for col in (0..n).rev() {
        let mut s = x[col];
        for c in col + 1..n {
            s -= m[col * n + c] * x[c];
        }
        x[col] = s / m[col * n + col];
    }
    Some(x)
}

fn pick_controls(body: &[Vec3], garment: &[Vec3], max_n: usize) -> Vec<u32> {
    if body.is_empty() || garment.is_empty() || max_n == 0 {
        return Vec::new();
    }
    let radius = 0.07f32;
    let cell = 0.05f32;
    let origin = body
        .iter()
        .chain(garment.iter())
        .copied()
        .fold(Vec3::splat(f32::MAX), |a, p| a.min(p));
    let key = |p: Vec3| ((p - origin) / cell).floor().as_ivec3();
    let mut grid: HashMap<IVec3, Vec<u32>> = HashMap::new();
    for (i, &p) in garment.iter().enumerate() {
        grid.entry(key(p)).or_default().push(i as u32);
    }
    let near: Vec<u32> = body
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            let c = key(**p);
            for dz in -2..=2 {
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        if let Some(list) = grid.get(&(c + IVec3::new(dx, dy, dz))) {
                            for &gi in list {
                                if garment[gi as usize].distance_squared(**p) <= radius * radius {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
            false
        })
        .map(|(i, _)| i as u32)
        .collect();
    if near.is_empty() {
        // Nothing close: a sparse sample of the whole body still drives a loose item.
        let step = (body.len() / max_n).max(1);
        return (0..body.len())
            .step_by(step)
            .take(max_n)
            .map(|i| i as u32)
            .collect();
    }
    let mut voxel = 0.04f32;
    loop {
        let mut seen: HashMap<IVec3, u32> = HashMap::new();
        for &i in &near {
            let k = ((body[i as usize] - origin) / voxel).floor().as_ivec3();
            seen.entry(k).or_insert(i);
        }
        if seen.len() <= max_n || voxel > 0.25 {
            let mut ids: Vec<u32> = seen.into_values().collect();
            ids.sort_unstable();
            return ids;
        }
        voxel *= 1.4;
    }
}

/// Median distance from garment vertices to the nearest body vertex.
/// Used to decide whether a new mesh is clothing (close) or a prop (far).
pub fn median_body_distance(body: &[Vec3], points: &[Vec3]) -> f32 {
    if body.is_empty() || points.is_empty() {
        return f32::MAX;
    }
    let cell = 0.05f32;
    let origin = body.iter().fold(Vec3::splat(f32::MAX), |a, p| a.min(*p));
    let key = |p: Vec3| ((p - origin) / cell).floor().as_ivec3();
    let mut grid: HashMap<IVec3, Vec<u32>> = HashMap::new();
    for (i, &p) in body.iter().enumerate() {
        grid.entry(key(p)).or_default().push(i as u32);
    }
    let step = (points.len() / 64).max(1);
    let mut dists: Vec<f32> = points
        .iter()
        .step_by(step)
        .map(|&p| {
            let c = key(p);
            let mut best = f32::MAX;
            for r in 0..=3i32 {
                for z in -r..=r {
                    for y in -r..=r {
                        for x in -r..=r {
                            if x.abs().max(y.abs()).max(z.abs()) != r {
                                continue;
                            }
                            if let Some(list) = grid.get(&(c + IVec3::new(x, y, z))) {
                                for &i in list {
                                    best = best.min(body[i as usize].distance_squared(p));
                                }
                            }
                        }
                    }
                }
                if best < f32::MAX && (r as f32 * cell).powi(2) >= best {
                    break;
                }
            }
            best.sqrt()
        })
        .collect();
    if dists.is_empty() {
        return f32::MAX;
    }
    dists.sort_by(f32::total_cmp);
    dists[dists.len() / 2]
}

/// Surface-fit when the mesh sits on the body; otherwise leave it rigid.
pub fn suggest_method(body: &[Vec3], points: &[Vec3]) -> FitMethod {
    if median_body_distance(body, points) < 0.04 {
        FitMethod::Surface
    } else {
        FitMethod::Rigid
    }
}

/// Barycentric blend of the bound triangle's influences, then neighbor smoothing.
/// The four strongest bones are kept and renormalised.
pub fn transfer_weights(
    body_skin: &[Influence],
    binding: &GarmentBind,
    body_indices: &[u32],
    neighbors: &[Vec<u32>],
    smooth_iterations: u32,
) -> Vec<Influence> {
    let n = binding.verts.len();
    if body_skin.is_empty() || n == 0 {
        return vec![Influence::default(); n];
    }
    let bones = body_skin
        .iter()
        .flat_map(|s| s.bones)
        .map(|b| b as usize + 1)
        .max()
        .unwrap_or(1);
    let mut dense = vec![0.0f32; n * bones];
    for (i, bind) in binding.verts.iter().enumerate() {
        let base = bind.triangle as usize * 3;
        if bind.triangle == u32::MAX || base + 2 >= body_indices.len() {
            continue;
        }
        for k in 0..3 {
            let v = body_indices[base + k] as usize;
            let w = bind.bary[k];
            if w <= 0.0 || v >= body_skin.len() {
                continue;
            }
            add_influence(&mut dense[i * bones..(i + 1) * bones], &body_skin[v], w);
        }
    }
    let lambda = 0.5f32;
    for _ in 0..smooth_iterations {
        let prev = dense.clone();
        for i in 0..n {
            let nbrs = neighbors.get(i).map(Vec::as_slice).unwrap_or(&[]);
            if nbrs.is_empty() {
                continue;
            }
            for b in 0..bones {
                let avg = nbrs
                    .iter()
                    .map(|&j| prev.get(j as usize * bones + b).copied().unwrap_or(0.0))
                    .sum::<f32>()
                    / nbrs.len() as f32;
                dense[i * bones + b] = (1.0 - lambda) * prev[i * bones + b] + lambda * avg;
            }
        }
    }
    (0..n)
        .map(|i| {
            let row = &dense[i * bones..(i + 1) * bones];
            let pairs: Vec<(u16, f32)> = row
                .iter()
                .enumerate()
                .filter(|(_, w)| **w > 1e-5)
                .map(|(b, w)| (b as u16, *w))
                .collect();
            let inf = Influence::from_pairs(pairs);
            if inf.weights.iter().sum::<f32>() > 0.0 {
                inf
            } else {
                Influence {
                    bones: [0; 4],
                    weights: [1.0, 0.0, 0.0, 0.0],
                }
            }
        })
        .collect()
}

fn add_influence(row: &mut [f32], inf: &Influence, weight: f32) {
    for k in 0..4 {
        let w = inf.weights[k];
        if w > 0.0 {
            let b = inf.bones[k] as usize;
            if b < row.len() {
                row[b] += w * weight;
            }
        }
    }
}

/// One mesh that can cover the body or another item.
pub struct CoverMesh<'a> {
    pub positions: &'a [Vec3],
    pub indices: &'a [u32],
    pub normals: &'a [Vec3],
    pub layer: u16,
    pub hide_body: bool,
    pub cover: f32,
}

/// Body triangles fully covered by any hiding item, and per-item triangles
/// covered by a higher layer.
pub fn occlusion(
    body: &[Vec3],
    body_normals: &[Vec3],
    body_indices: &[u32],
    items: &[CoverMesh<'_>],
) -> (Vec<u32>, Vec<Vec<u32>>) {
    let mut body_covered = vec![false; body.len()];
    for item in items.iter().filter(|i| i.hide_body) {
        let covered =
            covered_vertices(body, body_normals, item.positions, item.indices, item.cover);
        for (c, flag) in body_covered.iter_mut().zip(covered) {
            *c |= flag;
        }
    }
    let body_tris = hidden_triangles(&body_covered, body_indices);
    let mut item_tris = Vec::with_capacity(items.len());
    for item in items {
        let mut covered = vec![false; item.positions.len()];
        for other in items.iter().filter(|o| o.layer > item.layer) {
            let flags = covered_vertices(
                item.positions,
                item.normals,
                other.positions,
                other.indices,
                other.cover,
            );
            for (c, flag) in covered.iter_mut().zip(flags) {
                *c |= flag;
            }
        }
        item_tris.push(hidden_triangles(&covered, item.indices));
    }
    (body_tris, item_tris)
}

fn covered_vertices(
    points: &[Vec3],
    normals: &[Vec3],
    garment: &[Vec3],
    indices: &[u32],
    cover: f32,
) -> Vec<bool> {
    if points.is_empty() || indices.len() < 3 || cover <= 0.0 {
        return vec![false; points.len()];
    }
    let bvh = Bvh::build(garment, indices);
    let limit = cover * cover;
    points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let Some(hit) = bvh.closest(*p) else {
                return false;
            };
            if hit.dist_sq > limit {
                return false;
            }
            let n = normals.get(i).copied().unwrap_or(Vec3::Y);
            // The garment's closest point should lie outside the surface.
            (hit.point - *p).dot(n) > -0.003
        })
        .collect()
}

pub fn hidden_triangles(vertex_covered: &[bool], indices: &[u32]) -> Vec<u32> {
    indices
        .as_chunks::<3>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, tri)| {
            tri.iter()
                .all(|&i| vertex_covered.get(i as usize).copied().unwrap_or(false))
        })
        .map(|(t, _)| t as u32)
        .collect()
}

/// Drop triangles listed in `hidden` (triangle indices, not vertex indices).
pub fn without_triangles(indices: &[u32], hidden: &[u32]) -> Vec<u32> {
    if hidden.is_empty() {
        return indices.to_vec();
    }
    let n = indices.len() / 3;
    let mut drop = vec![false; n];
    for &t in hidden {
        if (t as usize) < n {
            drop[t as usize] = true;
        }
    }
    let mut out = Vec::with_capacity(indices.len());
    for (t, tri) in indices.as_chunks::<3>().0.iter().enumerate() {
        if !drop[t] {
            out.extend_from_slice(tri);
        }
    }
    if out.is_empty() {
        indices.to_vec()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle_body() -> (Vec<Vec3>, Vec<u32>, Vec<Vec3>) {
        let p = vec![Vec3::ZERO, Vec3::X, Vec3::Y];
        let idx = vec![0, 1, 2];
        let n = vec![Vec3::Z; 3];
        (p, idx, n)
    }

    fn settings_exact() -> FitSettings {
        FitSettings {
            method: FitMethod::Surface,
            tightness: 0.5,
            smooth_steps: 0,
            margin: 0.0,
            ..FitSettings::surface(1)
        }
    }

    #[test]
    fn bind_and_fit_reproduce_the_authored_garment() {
        let (body, idx, nrm) = triangle_body();
        let garment_pos = vec![
            Vec3::new(0.2, 0.2, 0.05),
            Vec3::new(0.6, 0.2, 0.04),
            Vec3::new(0.3, 0.5, 0.06),
        ];
        let mesh = GarmentMesh::from_parts(garment_pos.clone(), vec![[0, 1, 2]], vec![]);
        let binding = bind(&body, &idx, &nrm, &mesh.positions);
        let fitted = fit(&body, &idx, &nrm, &mesh, &binding, &settings_exact());
        for (a, b) in fitted.iter().zip(&garment_pos) {
            assert!(a.distance(*b) < 1e-4, "{a} vs {b}");
        }
    }

    #[test]
    fn fit_follows_a_moved_triangle() {
        let (mut body, idx, nrm) = triangle_body();
        let garment_pos = vec![Vec3::new(0.2, 0.2, 0.05)];
        let mesh = GarmentMesh::from_parts(garment_pos, vec![], vec![]);
        let binding = bind(&body, &idx, &nrm, &mesh.positions);
        for p in &mut body {
            *p += Vec3::new(0.0, 0.3, 0.1);
        }
        let fitted = fit(&body, &idx, &nrm, &mesh, &binding, &settings_exact());
        let expect = Vec3::new(0.2, 0.5, 0.15);
        assert!(
            fitted[0].distance(expect) < 1e-3,
            "got {} want {expect}",
            fitted[0]
        );
    }

    #[test]
    fn tightness_changes_the_gap() {
        let (body, idx, nrm) = triangle_body();
        let garment_pos = vec![Vec3::new(0.25, 0.25, 0.04)];
        let mesh = GarmentMesh::from_parts(garment_pos, vec![], vec![]);
        let binding = bind(&body, &idx, &nrm, &mesh.positions);
        let mut loose = settings_exact();
        loose.tightness = 0.0;
        loose.margin = 0.0;
        let mut tight = loose.clone();
        tight.tightness = 1.0;
        let a = fit(&body, &idx, &nrm, &mesh, &binding, &loose);
        let b = fit(&body, &idx, &nrm, &mesh, &binding, &tight);
        assert!(a[0].z > b[0].z + 0.005, "loose {} tight {}", a[0].z, b[0].z);
    }

    #[test]
    fn push_out_lifts_a_vertex_out_of_the_body() {
        let (body, idx, nrm) = triangle_body();
        // Authored on the underside (negative z); the body normal is +Z.
        let garment_pos = vec![Vec3::new(0.3, 0.3, -0.02)];
        let mesh = GarmentMesh::from_parts(garment_pos, vec![], vec![]);
        let binding = bind(&body, &idx, &nrm, &mesh.positions);
        let mut s = settings_exact();
        s.margin = 0.01;
        s.tightness = 0.5;
        let fitted = fit(&body, &idx, &nrm, &mesh, &binding, &s);
        assert!(fitted[0].z > 0.009, "z = {}", fitted[0].z);
    }

    #[test]
    fn smoothing_pulls_a_spike_toward_its_neighbors() {
        let (body, idx, nrm) = triangle_body();
        // A flat sheet with one spike. Binding keeps the spike; smoothing relaxes it.
        let garment_pos = vec![
            Vec3::new(0.1, 0.1, 0.02),
            Vec3::new(0.4, 0.1, 0.02),
            Vec3::new(0.25, 0.35, 0.02),
            Vec3::new(0.25, 0.18, 0.08),
        ];
        let mesh = GarmentMesh::from_parts(
            garment_pos.clone(),
            vec![[0, 1, 3], [1, 2, 3], [2, 0, 3]],
            vec![],
        );
        let binding = bind(&body, &idx, &nrm, &mesh.positions);
        let mut s = settings_exact();
        s.tightness = 0.0;
        s.smooth_steps = 8;
        s.margin = 0.0;
        let fitted = fit(&body, &idx, &nrm, &mesh, &binding, &s);
        assert!(
            fitted[3].z < garment_pos[3].z - 0.01,
            "spike stayed at {}",
            fitted[3].z
        );
    }

    #[test]
    fn loose_fit_is_identity_until_the_body_moves_then_follows() {
        let (body, idx, nrm) = triangle_body();
        // A "skirt" of four points hovering over the triangle, plus extras so
        // control picking has something to grab. Use the body verts themselves
        // duplicated slightly off the surface.
        let mut garment_pos = body.iter().map(|p| *p + Vec3::Z * 0.03).collect::<Vec<_>>();
        garment_pos.push(Vec3::new(0.4, 0.4, 0.05));
        let mesh = GarmentMesh::from_parts(garment_pos.clone(), vec![[0, 1, 2], [0, 1, 3]], vec![]);
        let binding = bind(&body, &idx, &nrm, &mesh.positions);
        assert!(
            binding.controls.len() >= 3,
            "controls {}",
            binding.controls.len()
        );
        let mut s = FitSettings::loose(1);
        s.smooth_steps = 0;
        s.margin = 0.0;
        s.tightness = 0.0;
        let fitted = fit(&body, &idx, &nrm, &mesh, &binding, &s);
        for (a, b) in fitted.iter().zip(&garment_pos) {
            assert!(a.distance(*b) < 1e-3, "identity {a} vs {b}");
        }
        let moved: Vec<Vec3> = body.iter().map(|p| *p + Vec3::Y).collect();
        let fitted = fit(&moved, &idx, &nrm, &mesh, &binding, &s);
        // Points that sit on the control vertices follow the translation.
        // A point outside the control hull is only approximate.
        for (a, b) in fitted.iter().zip(&garment_pos).take(3) {
            assert!(
                (a.y - (b.y + 1.0)).abs() < 0.08,
                "expected ~1 m lift, {a} from {b}"
            );
        }
    }

    #[test]
    fn weights_interpolate_and_then_spread() {
        let skin = vec![
            Influence {
                bones: [0, 1, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            Influence {
                bones: [1, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            Influence {
                bones: [0, 1, 0, 0],
                weights: [0.0, 1.0, 0.0, 0.0],
            },
        ];
        // Garment vertex 0 sits on body vertex 0; vertex 1 is its only neighbor
        // and was bound to body vertex 1.
        let binding = GarmentBind {
            verts: vec![
                VertexBind {
                    triangle: 0,
                    bary: [1.0, 0.0, 0.0],
                    offset: [0.0; 3],
                },
                VertexBind {
                    triangle: 0,
                    bary: [0.0, 1.0, 0.0],
                    offset: [0.0; 3],
                },
            ],
            controls: vec![],
            control_rest: vec![],
        };
        let neighbors = vec![vec![1], vec![0]];
        let raw = transfer_weights(&skin, &binding, &[0, 1, 2], &neighbors, 0);
        assert!((raw[0].weights[0] - 1.0).abs() < 1e-4 && raw[0].bones[0] == 0);
        assert!((raw[1].weights[0] - 1.0).abs() < 1e-4 && raw[1].bones[0] == 1);
        let smooth = transfer_weights(&skin, &binding, &[0, 1, 2], &neighbors, 1);
        let w0 = bone_weight(&smooth[0], 1);
        assert!(
            w0 > 0.2,
            "smoothing should share bone 1 onto vertex 0, got {smooth:?}"
        );
    }

    fn bone_weight(inf: &Influence, bone: u16) -> f32 {
        inf.bones
            .iter()
            .zip(inf.weights)
            .find(|(b, _)| **b == bone)
            .map(|(_, w)| w)
            .unwrap_or(0.0)
    }

    #[test]
    fn hide_under_covers_a_triangle_and_not_a_far_one() {
        let body = vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(3.0, 0.0, 0.0)];
        let body_n = vec![Vec3::Z; 4];
        let body_i = vec![0, 1, 2, 1, 3, 2];
        // A sheet just above the first triangle, not reaching x = 3.
        let cloth = vec![
            Vec3::new(-0.2, -0.2, 0.01),
            Vec3::new(1.2, -0.2, 0.01),
            Vec3::new(1.2, 1.2, 0.01),
            Vec3::new(-0.2, 1.2, 0.01),
        ];
        let cloth_i = vec![0, 1, 2, 0, 2, 3];
        let cloth_n = vec![Vec3::Z; 4];
        let items = [CoverMesh {
            positions: &cloth,
            indices: &cloth_i,
            normals: &cloth_n,
            layer: 1,
            hide_body: true,
            cover: 0.02,
        }];
        let (hidden, _) = occlusion(&body, &body_n, &body_i, &items);
        assert_eq!(hidden, vec![0], "only the covered triangle, got {hidden:?}");
    }

    #[test]
    fn outer_layer_hides_inner_triangles() {
        let inner = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let outer = vec![
            Vec3::new(-0.2, -0.2, 0.01),
            Vec3::new(1.2, -0.2, 0.01),
            Vec3::new(1.2, 1.2, 0.01),
            Vec3::new(-0.2, 1.2, 0.01),
        ];
        let n = vec![Vec3::Z; 3];
        let outer_n = vec![Vec3::Z; 4];
        let idx = vec![0, 1, 2];
        let outer_i = vec![0, 1, 2, 0, 2, 3];
        let items = [
            CoverMesh {
                positions: &inner,
                indices: &idx,
                normals: &n,
                layer: 0,
                hide_body: false,
                cover: 0.02,
            },
            CoverMesh {
                positions: &outer,
                indices: &outer_i,
                normals: &outer_n,
                layer: 1,
                hide_body: false,
                cover: 0.02,
            },
        ];
        let (_, per) = occlusion(&[], &[], &[], &items);
        assert_eq!(per[0], vec![0], "inner triangle hidden, got {per:?}");
        assert!(per[1].is_empty(), "outer is not hidden by the inner");
    }

    #[test]
    fn suggest_method_splits_clothing_from_props() {
        let body = vec![Vec3::ZERO, Vec3::X, Vec3::Y];
        let shirt = vec![Vec3::new(0.0, 0.0, 0.01), Vec3::new(0.02, 0.0, 0.012)];
        let sword = vec![Vec3::new(0.0, 0.0, 0.5), Vec3::new(0.0, 0.0, 1.2)];
        assert_eq!(suggest_method(&body, &shirt), FitMethod::Surface);
        assert_eq!(suggest_method(&body, &sword), FitMethod::Rigid);
    }
}
