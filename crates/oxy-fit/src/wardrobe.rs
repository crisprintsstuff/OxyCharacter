//! Built-in garments and hair, generated from the body they have to fit.
//!
//! The meshes are original: a shirt, trousers and a skirt are offset shells
//! (or, for the skirt, a flared tube) of body regions, and the hair is a set
//! of ribbon cards over the scalp. Nothing here is traced from another
//! character system. Regions come from the rig's bone names when those names
//! are the ones this project already uses, and from body proportions otherwise.

use glam::Vec3;
use oxy_rig::Influence;

use crate::{bind, transfer_weights, FitSettings, GarmentMesh, ItemKind};

pub const TEE: &str = "tee";
pub const PANTS: &str = "pants";
pub const SKIRT: &str = "skirt";
pub const HAIR: &str = "hair";

/// A garment authored on a specific body, ready to fit and skin.
#[derive(Debug, Clone)]
pub struct AuthoredGarment {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: ItemKind,
    /// Linear RGB.
    pub color: [f32; 3],
    pub mesh: GarmentMesh,
    pub settings: FitSettings,
    pub binding: crate::GarmentBind,
    pub skin: Vec<Influence>,
}

/// Ids the editor and CLI offer, in wardrobe order.
pub fn builtin_ids() -> &'static [&'static str] {
    &[TEE, PANTS, SKIRT, HAIR]
}

pub fn builtin_name(id: &str) -> Option<&'static str> {
    Some(match id {
        TEE => "T-shirt",
        PANTS => "Trousers",
        SKIRT => "Skirt",
        HAIR => "Hair",
        _ => return None,
    })
}

/// Build every built-in item on `positions` (the grounded base body).
///
/// `bone_names` indexes the same bones as `skin`. Either may be absent; the
/// generator then classifies vertices by proportion.
pub fn author_builtins(
    positions: &[Vec3],
    indices: &[u32],
    skin: Option<&[Influence]>,
    bone_names: Option<&[String]>,
) -> Vec<AuthoredGarment> {
    let Some(scan) = BodyScan::analyze(positions, indices, skin, bone_names) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(g) = scan.shell(
        Draft::new(
            TEE,
            "T-shirt",
            ItemKind::Garment,
            [0.16, 0.28, 0.48],
            FitSettings::surface(2),
        ),
        0.008,
        1,
        |role, y, scan| {
            // Neck is included so the collar covers the base of the neck
            // instead of leaving the shoulders bare.
            matches!(
                role,
                Role::Torso | Role::UpperArm | Role::Neck | Role::Pelvis
            ) && y > scan.crotch_y - 0.01
                && y < scan.neck_y + 0.02
        },
    ) {
        out.push(g);
    }
    if let Some(g) = scan.shell(
        Draft::new(
            PANTS,
            "Trousers",
            ItemKind::Garment,
            [0.11, 0.12, 0.15],
            FitSettings::surface(0),
        ),
        0.006,
        2,
        |role, y, scan| {
            matches!(role, Role::Pelvis | Role::Thigh | Role::Calf)
                && y < scan.waist_y + 0.03
                && y > scan.ankle_y
        },
    ) {
        out.push(g);
    }
    if let Some(g) = scan.skirt() {
        out.push(g);
    }
    if let Some(g) = scan.hair() {
        out.push(g);
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    Head,
    Neck,
    Torso,
    UpperArm,
    Forearm,
    Hand,
    Pelvis,
    Thigh,
    Calf,
    Foot,
    Other,
}

struct BodyScan<'a> {
    positions: &'a [Vec3],
    indices: &'a [u32],
    normals: Vec<Vec3>,
    roles: Vec<Role>,
    skin: Option<&'a [Influence]>,
    height: f32,
    shoulder_half: f32,
    neck_y: f32,
    crotch_y: f32,
    waist_y: f32,
    hip_half: f32,
    ankle_y: f32,
    center_z: f32,
}

impl<'a> BodyScan<'a> {
    fn analyze(
        positions: &'a [Vec3],
        indices: &'a [u32],
        skin: Option<&'a [Influence]>,
        bone_names: Option<&[String]>,
    ) -> Option<Self> {
        if positions.len() < 4 || indices.len() < 3 {
            return None;
        }
        let height = positions.iter().map(|p| p.y).fold(0.0f32, f32::max);
        if height < 1e-3 {
            return None;
        }
        let normals = oxy_mesh::Mesh::new(positions.to_vec(), indices.to_vec())
            .map(|m| m.compute_normals(positions))
            .unwrap_or_else(|_| vec![Vec3::Y; positions.len()]);
        let measures = measures(positions, height);
        let roles = roles_from_bones(skin, bone_names)
            .filter(|r| r.len() == positions.len())
            .unwrap_or_else(|| {
                positions
                    .iter()
                    .map(|p| geometric_role(*p, &measures))
                    .collect()
            });
        Some(Self {
            positions,
            indices,
            normals,
            roles,
            skin,
            height,
            shoulder_half: measures.shoulder_half,
            neck_y: measures.neck_y,
            crotch_y: measures.crotch_y,
            waist_y: measures.waist_y,
            hip_half: measures.hip_half,
            ankle_y: measures.ankle_y,
            center_z: measures.center_z,
        })
    }

    fn shell(
        &self,
        draft: Draft,
        gap: f32,
        min_votes: usize,
        include: impl Fn(Role, f32, &Self) -> bool,
    ) -> Option<AuthoredGarment> {
        let mut map = vec![u32::MAX; self.positions.len()];
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut triangles = Vec::new();
        let y0 = self.ankle_y;
        let y1 = self.height.max(y0 + 0.01);
        for t in 0..self.indices.len() / 3 {
            let ids = [0, 1, 2].map(|k| self.indices[t * 3 + k] as usize);
            if ids.iter().any(|&i| i >= self.positions.len()) {
                continue;
            }
            let centroid =
                (self.positions[ids[0]] + self.positions[ids[1]] + self.positions[ids[2]]) / 3.0;
            let votes = ids
                .iter()
                .filter(|&&i| include(self.roles[i], centroid.y, self))
                .count();
            if votes < min_votes {
                continue;
            }
            let mut tri = [0u32; 3];
            for (k, &v) in ids.iter().enumerate() {
                if map[v] == u32::MAX {
                    let p = self.positions[v];
                    let n = self.normals[v];
                    map[v] = positions.len() as u32;
                    positions.push(p + n * gap);
                    let ang = p.z.atan2(p.x);
                    uvs.push([
                        ang / std::f32::consts::TAU + 0.5,
                        ((p.y - y0) / (y1 - y0)).clamp(0.0, 1.0),
                    ]);
                }
                tri[k] = map[v];
            }
            if tri[0] != tri[1] && tri[1] != tri[2] && tri[2] != tri[0] {
                triangles.push(tri);
            }
        }
        self.finish(draft, positions, triangles, uvs)
    }

    fn skirt(&self) -> Option<AuthoredGarment> {
        let hip_y = self.waist_y;
        let hem_y = (self.crotch_y * 0.62).max(self.ankle_y + 0.05);
        if hip_y - hem_y < 0.05 {
            return None;
        }
        let hip_r = self.hip_half.max(0.08) * 1.05;
        let hem_r = hip_r * 1.45;
        let segments = 18usize;
        let rows = 5usize;
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut triangles = Vec::new();
        for row in 0..=rows {
            let t = row as f32 / rows as f32;
            let y = hip_y + (hem_y - hip_y) * t;
            let r = hip_r + (hem_r - hip_r) * t;
            for s in 0..segments {
                let ang = s as f32 / segments as f32 * std::f32::consts::TAU;
                positions.push(Vec3::new(ang.sin() * r, y, self.center_z + ang.cos() * r));
                uvs.push([s as f32 / segments as f32, t]);
            }
        }
        let idx = |row: usize, s: usize| (row * segments + (s % segments)) as u32;
        for row in 0..rows {
            for s in 0..segments {
                let a = idx(row, s);
                let b = idx(row, s + 1);
                let c = idx(row + 1, s);
                let d = idx(row + 1, s + 1);
                triangles.push([a, c, b]);
                triangles.push([b, c, d]);
            }
        }
        self.finish(
            Draft::new(
                SKIRT,
                "Skirt",
                ItemKind::Garment,
                [0.38, 0.16, 0.24],
                FitSettings::loose(1),
            ),
            positions,
            triangles,
            uvs,
        )
    }

    fn hair(&self) -> Option<AuthoredGarment> {
        let scalp: Vec<(Vec3, Vec3)> = self
            .positions
            .iter()
            .zip(&self.normals)
            .enumerate()
            .filter(|(i, (p, n))| {
                p.y > self.neck_y - 0.01
                    && p.x.abs() < self.shoulder_half.max(0.12)
                    && (self.roles[*i] == Role::Head
                        || self.roles[*i] == Role::Other
                        || p.y > self.neck_y)
                    && n.z < 0.72
                    && p.y > self.neck_y + (self.height - self.neck_y) * 0.15
            })
            .map(|(_, (p, n))| (*p, *n))
            .collect();
        if scalp.len() < 4 {
            return None;
        }
        let center = scalp.iter().map(|s| s.0).sum::<Vec3>() / scalp.len() as f32;
        let top = scalp.iter().map(|s| s.0.y).fold(center.y, f32::max);
        let radius = (scalp
            .iter()
            .map(|s| {
                let d = s.0 - center;
                d.x * d.x + d.z * d.z
            })
            .sum::<f32>()
            / scalp.len() as f32)
            .sqrt()
            .max(0.04);
        let front_z = scalp.iter().map(|s| s.0.z).fold(f32::MIN, f32::max);
        let back_z = scalp.iter().map(|s| s.0.z).fold(f32::MAX, f32::min);
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut triangles = Vec::new();
        let nearest = |q: Vec3| -> Option<(Vec3, Vec3)> {
            scalp
                .iter()
                .filter(|s| s.0.distance_squared(q) < (radius * 1.6).powi(2))
                .min_by(|a, b| a.0.distance_squared(q).total_cmp(&b.0.distance_squared(q)))
                .map(|(p, n)| (*p, n.normalize_or_zero()))
        };
        // Each card is a fin standing off the scalp, so it has a silhouette
        // from the front instead of lying flat against the head.
        let mut add_ribbon = |samples: &[(Vec3, Vec3)], half_w: f32| {
            if samples.len() < 2 {
                return;
            }
            let base = positions.len() as u32;
            for (i, (p, n)) in samples.iter().enumerate() {
                let up = (*n + Vec3::Y * 0.65).normalize_or_zero();
                let up = if up.length_squared() < 1e-6 {
                    Vec3::Y
                } else {
                    up
                };
                positions.push(*p + up * half_w * 0.15);
                positions.push(*p + up * half_w * 2.4);
                let v = i as f32 / (samples.len() - 1) as f32;
                uvs.push([0.0, v]);
                uvs.push([1.0, v]);
            }
            for i in 0..samples.len() - 1 {
                let a = base + (i as u32) * 2;
                triangles.push([a, a + 2, a + 1]);
                triangles.push([a + 1, a + 2, a + 3]);
            }
        };
        for xi in -3i32..=3 {
            let xoff = xi as f32 * radius * 0.2;
            let mut samples = Vec::new();
            for s in 0..12 {
                let t = s as f32 / 11.0;
                let z = front_z + (back_z - radius * 0.35 - front_z) * t;
                let mut y = top - 0.005 - (t - 0.28).abs() * radius * 0.55;
                if t > 0.72 {
                    y = y + (center.y - radius * 0.35 - y) * ((t - 0.72) / 0.28);
                }
                if let Some((p, n)) = nearest(Vec3::new(center.x + xoff, y, z)) {
                    let n = if n.length_squared() < 1e-6 {
                        Vec3::Y
                    } else {
                        n
                    };
                    samples.push((p + n * (0.012 + xi.abs() as f32 * 0.001), n));
                }
            }
            add_ribbon(&samples, radius * 0.28);
        }
        for sign in [-1.0f32, 1.0] {
            for k in 0..3 {
                let mut samples = Vec::new();
                for s in 0..8 {
                    let t = s as f32 / 7.0;
                    let y = (top - radius * 0.15)
                        + (center.y - radius * 0.85 - (top - radius * 0.15)) * t;
                    let x = sign * radius * (0.78 + k as f32 * 0.16);
                    let z = center.z + radius * (0.2 - t * 0.45);
                    if let Some((p, n)) = nearest(Vec3::new(center.x + x, y, z)) {
                        let n = if n.length_squared() < 1e-6 {
                            Vec3::new(sign, 0.2, 0.0).normalize()
                        } else {
                            n
                        };
                        samples.push((p + n * 0.014, n));
                    }
                }
                add_ribbon(&samples, radius * 0.22);
            }
        }
        // Bangs: a short row across the forehead.
        let mut bangs = Vec::new();
        for s in 0..8 {
            let t = s as f32 / 7.0;
            let x = center.x + (t - 0.5) * radius * 1.3;
            let y = top - radius * (0.35 + t * 0.05);
            let z = front_z + 0.01;
            if let Some((p, n)) = nearest(Vec3::new(x, y, z)) {
                let n = if n.length_squared() < 1e-6 {
                    Vec3::Z
                } else {
                    n
                };
                bangs.push((p + n * 0.01, n));
            }
        }
        add_ribbon(&bangs, radius * 0.08);
        self.finish(
            Draft::new(
                HAIR,
                "Hair",
                ItemKind::Hair,
                [0.12, 0.07, 0.045],
                FitSettings::hair(),
            ),
            positions,
            triangles,
            uvs,
        )
    }

    fn finish(
        &self,
        draft: Draft,
        positions: Vec<Vec3>,
        triangles: Vec<[u32; 3]>,
        uvs: Vec<[f32; 2]>,
    ) -> Option<AuthoredGarment> {
        if positions.len() < 3 || triangles.is_empty() {
            return None;
        }
        let mesh = GarmentMesh::from_parts(positions, triangles, uvs);
        let binding = bind(self.positions, self.indices, &self.normals, &mesh.positions);
        let skin = self.skin.map_or_else(Vec::new, |skin| {
            transfer_weights(skin, &binding, self.indices, &mesh.neighbors, 6)
        });
        Some(AuthoredGarment {
            id: draft.id,
            name: draft.name,
            kind: draft.kind,
            color: draft.color,
            mesh,
            settings: draft.settings,
            binding,
            skin,
        })
    }
}

struct Draft {
    id: &'static str,
    name: &'static str,
    kind: ItemKind,
    color: [f32; 3],
    settings: FitSettings,
}

impl Draft {
    fn new(
        id: &'static str,
        name: &'static str,
        kind: ItemKind,
        color: [f32; 3],
        settings: FitSettings,
    ) -> Self {
        Self {
            id,
            name,
            kind,
            color,
            settings,
        }
    }
}

struct Measures {
    shoulder_y: f32,
    shoulder_half: f32,
    neck_y: f32,
    crotch_y: f32,
    waist_y: f32,
    hip_half: f32,
    ankle_y: f32,
    center_z: f32,
    height: f32,
}

fn measures(positions: &[Vec3], height: f32) -> Measures {
    let slices = 48usize;
    let mut extent = vec![0.0f32; slices];
    let mut count = vec![0u32; slices];
    for p in positions {
        let i = ((p.y / height) * (slices as f32 - 0.001)).clamp(0.0, slices as f32 - 1.0) as usize;
        extent[i] = extent[i].max(p.x.abs());
        count[i] += 1;
    }
    let mut shoulder_half = 0.0f32;
    let mut shoulder_y = 0.75 * height;
    for (i, ext) in extent.iter().enumerate() {
        let y = (i as f32 + 0.5) / slices as f32 * height;
        let band = y / height;
        if (0.62..0.88).contains(&band) && *ext > shoulder_half && count[i] > 0 {
            shoulder_half = *ext;
            shoulder_y = y;
        }
    }
    shoulder_half = shoulder_half.max(0.05);
    let mut neck_y = (shoulder_y + height) * 0.5;
    for (i, ext) in extent.iter().enumerate() {
        let y = (i as f32 + 0.5) / slices as f32 * height;
        if y > shoulder_y + 0.02 * height && count[i] > 0 && *ext < shoulder_half * 0.55 {
            neck_y = y;
            break;
        }
    }
    let mut crotch_y = 0.47 * height;
    let mut found_crotch = false;
    for i in (0..slices).rev() {
        let y = (i as f32 + 0.5) / slices as f32 * height;
        if !(0.28 * height..0.62 * height).contains(&y) {
            continue;
        }
        let band = height / slices as f32;
        let (mut left, mut right, mut mid) = (0u32, 0u32, 0u32);
        for p in positions {
            if (p.y - y).abs() > band {
                continue;
            }
            if p.x < -0.025 {
                left += 1;
            } else if p.x > 0.025 {
                right += 1;
            } else {
                mid += 1;
            }
        }
        if left > 2 && right > 2 && mid == 0 {
            crotch_y = y;
            found_crotch = true;
            break;
        }
    }
    if !found_crotch {
        crotch_y = 0.47 * height;
    }
    let hip_band_y = (crotch_y + 0.04 * height).min(shoulder_y);
    let hip_i =
        ((hip_band_y / height) * (slices as f32 - 0.001)).clamp(0.0, slices as f32 - 1.0) as usize;
    let hip_half = extent[hip_i].max(shoulder_half * 0.35);
    let waist_y = crotch_y + (shoulder_y - crotch_y) * 0.45;
    let center_z = {
        let mut acc = 0.0;
        let mut n = 0.0;
        for p in positions {
            if (p.y - waist_y).abs() < 0.05 * height && p.x.abs() < hip_half {
                acc += p.z;
                n += 1.0;
            }
        }
        if n > 0.0 {
            acc / n
        } else {
            0.0
        }
    };
    Measures {
        shoulder_y,
        shoulder_half,
        neck_y,
        crotch_y,
        waist_y,
        hip_half,
        ankle_y: 0.06 * height,
        center_z,
        height,
    }
}

fn geometric_role(p: Vec3, m: &Measures) -> Role {
    if p.y > m.neck_y && p.x.abs() < m.shoulder_half * 0.7 {
        return Role::Head;
    }
    if p.y >= m.crotch_y - 0.02 && p.y <= m.neck_y + 0.02 {
        let torso_half = if p.y > m.waist_y {
            m.hip_half * 0.95
        } else {
            m.hip_half
        };
        if p.x.abs() > torso_half.max(m.shoulder_half * 0.45) {
            return if p.y > m.shoulder_y - 0.16 * m.height {
                Role::UpperArm
            } else {
                Role::Forearm
            };
        }
        if p.y > m.neck_y - 0.02 && p.x.abs() < m.shoulder_half * 0.25 {
            return Role::Neck;
        }
        return if p.y > m.waist_y {
            Role::Torso
        } else {
            Role::Pelvis
        };
    }
    if p.y >= m.ankle_y && p.y < m.crotch_y {
        return if p.y > m.crotch_y * 0.55 {
            Role::Thigh
        } else {
            Role::Calf
        };
    }
    if p.y < m.ankle_y {
        Role::Foot
    } else {
        Role::Other
    }
}

fn role_from_bone(name: &str) -> Role {
    let n = name.to_ascii_lowercase();
    if n.contains("finger")
        || n.contains("thumb")
        || n.contains("hand")
        || n.contains("wrist")
        || n.contains("palm")
        || n.contains("metacarp")
    {
        Role::Hand
    } else if n.contains("lowerarm") || n.contains("forearm") {
        Role::Forearm
    } else if n.contains("upperarm") || n.contains("clavicle") || n.contains("shoulder") {
        Role::UpperArm
    } else if n.contains("breast")
        || n.contains("pectoral")
        || n.contains("chest")
        || n.contains("spine")
    {
        Role::Torso
    } else if n.contains("neck") {
        Role::Neck
    } else if n.contains("head")
        || n.contains("jaw")
        || n.contains("eye")
        || n.contains("orbicular")
        || n.contains("levator")
        || n.contains("temporal")
        || n.contains("masseter")
        || n.contains("oculi")
        || n.contains("oris")
        || n.contains("brow")
        || n.contains("lip")
        || n.contains("nose")
        || n.contains("ear")
        || n.contains("tongue")
    {
        Role::Head
    } else if n.contains("pelvis") || n.contains("hip") || n == "root" {
        Role::Pelvis
    } else if n.contains("upperleg") || n.contains("thigh") {
        Role::Thigh
    } else if n.contains("lowerleg") || n.contains("calf") || n.contains("shin") {
        Role::Calf
    } else if n.contains("foot") || n.contains("toe") || n.contains("ankle") {
        Role::Foot
    } else {
        Role::Other
    }
}

fn roles_from_bones(skin: Option<&[Influence]>, names: Option<&[String]>) -> Option<Vec<Role>> {
    let (skin, names) = (skin?, names?);
    if names
        .iter()
        .filter(|n| role_from_bone(n) != Role::Other)
        .count()
        < 8
    {
        return None;
    }
    Some(
        skin.iter()
            .map(|inf| {
                let mut best = (0.0f32, Role::Other);
                for k in 0..4 {
                    let w = inf.weights[k];
                    if w > best.0 {
                        let name = names
                            .get(inf.bones[k] as usize)
                            .map(String::as_str)
                            .unwrap_or("");
                        best = (w, role_from_bone(name));
                    }
                }
                best.1
            })
            .collect(),
    )
}

/// A coarse standing figure (metres, feet at 0, facing +Z) for tests.
#[cfg(test)]
pub fn block_figure() -> (Vec<Vec3>, Vec<u32>, Vec<Influence>, Vec<String>) {
    fn box_mesh(
        origin: Vec3,
        size: Vec3,
        bone: u16,
        positions: &mut Vec<Vec3>,
        indices: &mut Vec<u32>,
        skin: &mut Vec<Influence>,
    ) {
        let b = positions.len() as u32;
        let (x, y, z) = (size.x, size.y, size.z);
        let corners = [
            origin,
            origin + Vec3::X * x,
            origin + Vec3::new(x, y, 0.0),
            origin + Vec3::Y * y,
            origin + Vec3::Z * z,
            origin + Vec3::new(x, 0.0, z),
            origin + size,
            origin + Vec3::new(0.0, y, z),
        ];
        positions.extend(corners);
        let inf = Influence {
            bones: [bone, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        };
        skin.extend(std::iter::repeat_n(inf, 8));
        // Outward-facing triangles so vertex normals point out of the box.
        for [a, c, d] in [
            [0, 3, 2],
            [0, 2, 1],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [3, 7, 6],
            [3, 6, 2],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
        ] {
            indices.extend([b + a, b + c, b + d]);
        }
    }
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    let mut skin = Vec::new();
    // 0 head, 1 neck, 2 spine, 3 pelvis, 4 upperarm.L, 5 upperarm.R,
    // 6 lowerarm.L, 7 upperleg.L, 8 upperleg.R, 9 lowerleg.L, 10 lowerleg.R, 11 foot
    box_mesh(
        Vec3::new(-0.09, 1.42, -0.09),
        Vec3::splat(0.18),
        0,
        &mut positions,
        &mut indices,
        &mut skin,
    );
    box_mesh(
        Vec3::new(-0.045, 1.28, -0.045),
        Vec3::splat(0.09),
        1,
        &mut positions,
        &mut indices,
        &mut skin,
    );
    box_mesh(
        Vec3::new(-0.16, 0.95, -0.1),
        Vec3::new(0.32, 0.33, 0.2),
        2,
        &mut positions,
        &mut indices,
        &mut skin,
    );
    box_mesh(
        Vec3::new(-0.17, 0.72, -0.11),
        Vec3::new(0.34, 0.23, 0.22),
        3,
        &mut positions,
        &mut indices,
        &mut skin,
    );
    box_mesh(
        Vec3::new(0.16, 1.02, -0.05),
        Vec3::new(0.28, 0.12, 0.1),
        4,
        &mut positions,
        &mut indices,
        &mut skin,
    );
    box_mesh(
        Vec3::new(-0.44, 1.02, -0.05),
        Vec3::new(0.28, 0.12, 0.1),
        5,
        &mut positions,
        &mut indices,
        &mut skin,
    );
    box_mesh(
        Vec3::new(0.40, 0.72, -0.045),
        Vec3::new(0.1, 0.32, 0.09),
        6,
        &mut positions,
        &mut indices,
        &mut skin,
    );
    box_mesh(
        Vec3::new(0.02, 0.38, -0.07),
        Vec3::new(0.13, 0.34, 0.14),
        7,
        &mut positions,
        &mut indices,
        &mut skin,
    );
    box_mesh(
        Vec3::new(-0.15, 0.38, -0.07),
        Vec3::new(0.13, 0.34, 0.14),
        8,
        &mut positions,
        &mut indices,
        &mut skin,
    );
    box_mesh(
        Vec3::new(0.03, 0.06, -0.06),
        Vec3::new(0.11, 0.32, 0.12),
        9,
        &mut positions,
        &mut indices,
        &mut skin,
    );
    box_mesh(
        Vec3::new(-0.14, 0.06, -0.06),
        Vec3::new(0.11, 0.32, 0.12),
        10,
        &mut positions,
        &mut indices,
        &mut skin,
    );
    box_mesh(
        Vec3::new(-0.08, 0.0, -0.05),
        Vec3::new(0.16, 0.05, 0.22),
        11,
        &mut positions,
        &mut indices,
        &mut skin,
    );
    let names = [
        "head",
        "neck01",
        "spine03",
        "pelvis",
        "upperarm01.L",
        "upperarm01.R",
        "lowerarm01.L",
        "upperleg01.L",
        "upperleg01.R",
        "lowerleg01.L",
        "lowerleg01.R",
        "foot.L",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    (positions, indices, skin, names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fit, FitMethod};

    #[test]
    fn builtins_cover_the_right_regions_and_refit() {
        let (positions, indices, skin, names) = block_figure();
        let items = author_builtins(&positions, &indices, Some(&skin), Some(&names));
        let by_id = |id| {
            items
                .iter()
                .find(|g| g.id == id)
                .unwrap_or_else(|| panic!("missing {id}"))
        };
        let tee = by_id(TEE);
        let pants = by_id(PANTS);
        let skirt = by_id(SKIRT);
        let hair = by_id(HAIR);
        assert!(
            tee.mesh.triangles.len() > 8,
            "shirt {:?}",
            tee.mesh.triangles.len()
        );
        assert!(pants.mesh.triangles.len() > 8);
        assert!(!skirt.mesh.triangles.is_empty());
        assert!(hair.mesh.triangles.len() > 4, "hair cards");
        assert_eq!(tee.kind, ItemKind::Garment);
        assert_eq!(hair.kind, ItemKind::Hair);
        assert_eq!(skirt.settings.method, FitMethod::Loose);
        // The shirt's vertices sit just outside the torso/arms, not down at the feet.
        let shirt_min_y = tee
            .mesh
            .positions
            .iter()
            .map(|p| p.y)
            .fold(f32::MAX, f32::min);
        assert!(shirt_min_y > 0.65, "shirt hem at {shirt_min_y}");
        let pants_max_y = pants
            .mesh
            .positions
            .iter()
            .map(|p| p.y)
            .fold(0.0f32, f32::max);
        assert!(pants_max_y < 1.15, "trousers top at {pants_max_y}");
        // Identity fit, then a taller body (every vertex scaled in y) moves the shirt up.
        let settings = FitSettings {
            smooth_steps: 0,
            margin: 0.0,
            ..tee.settings.clone()
        };
        let same = fit(
            &positions,
            &indices,
            &[],
            &tee.mesh,
            &tee.binding,
            &settings,
        );
        let err = same
            .iter()
            .zip(&tee.mesh.positions)
            .map(|(a, b)| a.distance(*b))
            .fold(0.0f32, f32::max);
        assert!(err < 2e-3, "shirt identity error {err}");
        let tall: Vec<Vec3> = positions
            .iter()
            .map(|p| Vec3::new(p.x, p.y * 1.2, p.z))
            .collect();
        let fitted = fit(&tall, &indices, &[], &tee.mesh, &tee.binding, &settings);
        let lifted = fitted
            .iter()
            .zip(&tee.mesh.positions)
            .map(|(a, b)| a.y - b.y)
            .fold(0.0f32, f32::max);
        assert!(
            lifted > 0.05,
            "shirt should rise with the body, lift {lifted}"
        );
        assert_eq!(tee.skin.len(), tee.mesh.positions.len());
        assert!(tee
            .skin
            .iter()
            .all(|s| (s.weights.iter().sum::<f32>() - 1.0).abs() < 1e-3));
    }

    #[test]
    fn unknown_builtin_ids_are_rejected() {
        assert!(builtin_name("cape").is_none());
        assert!(builtin_ids().contains(&TEE));
    }
}
