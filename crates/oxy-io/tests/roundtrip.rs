//! Cross-checks our writers against independent readers / real data.

use glam::Vec3;
use oxy_io::gltf::{write_glb, ExportMesh};

#[test]
fn glb_parses_with_gltf_crate() {
    let positions = [Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z];
    let normals = [Vec3::Y; 4];
    let indices = [0, 1, 2, 0, 2, 3];
    let mut bytes = Vec::new();
    let mesh = ExportMesh {
        name: "quad",
        positions: &positions,
        normals: &normals,
        indices: &indices,
        base_color: [0.8, 0.6, 0.5, 1.0],
        colors: None,
    };
    write_glb(&mesh, &mut bytes).unwrap();

    let (doc, buffers, _) = gltf::import_slice(&bytes).expect("valid glb");
    let prim = doc.meshes().next().unwrap().primitives().next().unwrap();
    let reader = prim.reader(|b| Some(&buffers[b.index()]));
    let read: Vec<[f32; 3]> = reader.read_positions().unwrap().collect();
    assert_eq!(read, positions.map(|p| p.to_array()));
    let idx: Vec<u32> = reader.read_indices().unwrap().into_u32().collect();
    assert_eq!(idx, indices);
}

/// Runs against the real CC0 data when `OXY_MAKEHUMAN_DATA` points at a
/// MakeHuman `data` directory (see `tools/fetch-assets.sh`); skipped otherwise.
#[test]
fn imports_makehuman_data_if_available() {
    let Some(dir) = std::env::var_os("OXY_MAKEHUMAN_DATA") else {
        eprintln!("OXY_MAKEHUMAN_DATA not set; skipping");
        return;
    };
    let asset = oxy_io::makehuman::import(dir.as_ref()).unwrap();
    let cat = &asset.catalog;
    assert!(asset.mesh.base.vertex_count() > 10_000);
    assert!(cat.params.len() > 200);

    // Neutral body is roughly human-sized; "tall" is taller than "short".
    let height = |h: f32| {
        let mut p = cat.defaults();
        p.insert("height".into(), h);
        let (w, missing) = oxy_morph::resolve(&asset.mesh, &cat.evaluate(&p));
        assert!(missing.is_empty(), "{missing:?}");
        let (lo, hi) = oxy_io::makehuman::bounds(&asset.mesh.evaluate(&w));
        hi.y - lo.y
    };
    let (short, neutral, tall) = (height(-1.0), height(0.0), height(1.0));
    assert!((1.4..2.0).contains(&neutral), "neutral height {neutral}");
    assert!(short < neutral && neutral < tall);

    // Every slider resolves to loaded targets, and mirrors exist.
    for p in &cat.params {
        for t in cat.targets_for(&p.id) {
            assert!(asset.mesh.target_index(t).is_some(), "{} -> {t}", p.id);
        }
        if let Some(m) = &p.mirror {
            assert!(cat.param(m).is_some(), "mirror {m} of {}", p.id);
        }
    }
}

#[test]
fn multi_mesh_glb_with_colors_parses() {
    let p = [Vec3::ZERO, Vec3::X, Vec3::Y];
    let n = [Vec3::Z; 3];
    let c = [[1.0, 0.0, 0.0, 1.0]; 3];
    let body = ExportMesh {
        name: "body",
        positions: &p,
        normals: &n,
        indices: &[0, 1, 2],
        base_color: [1.0; 4],
        colors: None,
    };
    let hat = ExportMesh {
        name: "hat",
        positions: &p,
        normals: &n,
        indices: &[0, 2, 1],
        base_color: [1.0; 4],
        colors: Some(&c),
    };
    let mut bytes = Vec::new();
    oxy_io::gltf::write_glb_scene(&[body, hat], &mut bytes).unwrap();
    let (doc, buffers, _) = gltf::import_slice(&bytes).expect("valid glb");
    assert_eq!(doc.nodes().count(), 2);
    let hat = doc.meshes().nth(1).unwrap().primitives().next().unwrap();
    let colors: Vec<[f32; 4]> = hat
        .reader(|b| Some(&buffers[b.index()]))
        .read_colors(0)
        .unwrap()
        .into_rgba_f32()
        .collect();
    assert_eq!(colors, c);
}

/// Skeleton checks on the real CC0 data (skipped without `OXY_MAKEHUMAN_DATA`).
#[test]
fn makehuman_rig_refits_to_morphs_if_available() {
    let Some(dir) = std::env::var_os("OXY_MAKEHUMAN_DATA") else {
        eprintln!("OXY_MAKEHUMAN_DATA not set; skipping");
        return;
    };
    let asset = oxy_io::makehuman::import(dir.as_ref()).unwrap();
    let rig = asset.rig.as_ref().expect("default skeleton imported");
    rig.validate(asset.mesh.base.vertex_count(), asset.mesh.aux.len())
        .unwrap();
    assert!(rig.bones.len() > 100, "{} bones", rig.bones.len());
    for inf in &rig.skin {
        let s: f32 = inf.weights.iter().sum();
        assert!((s - 1.0).abs() < 1e-4, "every body vertex is weighted");
    }

    let head = rig.bone_index("head").unwrap();
    let knee = rig.bone_index("lowerleg01.L").unwrap();
    let eval = |h: f32| {
        let mut p = asset.catalog.defaults();
        p.insert("height".into(), h);
        asset.evaluate_full(&p)
    };
    let (short, tall) = (eval(-1.0), eval(1.0));
    let head_y = |e: &oxy_io::Evaluated| e.joints[rig.bones[head].head].y;
    assert!(
        head_y(&tall) > head_y(&short) + 0.1,
        "head joint rises with height"
    );

    // Joints stay inside the body's bounding box, and the knee is at roughly knee height.
    let (lo, hi) = oxy_io::makehuman::bounds(&tall.body);
    for j in &tall.joints {
        assert!(
            j.cmpge(lo - 0.02).all() && j.cmple(hi + 0.02).all(),
            "joint {j:?} outside body"
        );
    }
    let knee_h = tall.joints[rig.bones[knee].head].y / (hi.y - lo.y);
    assert!((0.2..0.35).contains(&knee_h), "knee at {knee_h} of height");

    // Bind pose with identity local rotations reproduces the mesh exactly.
    let bind = rig.bind_matrices(&tall.joints);
    let posed = rig.pose_matrices(&bind, &vec![oxy_io::glam::Quat::IDENTITY; rig.bones.len()]);
    let normals = asset.mesh.base.compute_normals(&tall.body);
    let (skinned, _) = rig.skin_points(&tall.body, &normals, &bind, &posed);
    let err = skinned
        .iter()
        .zip(&tall.body)
        .map(|(a, b)| a.distance(*b))
        .fold(0.0f32, f32::max);
    assert!(err < 1e-4, "bind-pose skinning error {err}");
}
