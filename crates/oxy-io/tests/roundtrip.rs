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
        ..Default::default()
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

    // UVs: a render mesh with seams, consistent with the base, UVs inside the unit square.
    let r = asset.render.as_ref().expect("render mesh with UVs");
    assert!(r.matches(&asset.mesh.base));
    assert!(
        r.vertex_count() > asset.mesh.base.vertex_count(),
        "seams split some vertices"
    );
    assert!(
        r.uvs.iter().flatten().all(|c| (-1e-4..=1.0001).contains(c)),
        "UVs in [0,1]"
    );

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
        ..Default::default()
    };
    let hat = ExportMesh {
        name: "hat",
        positions: &p,
        normals: &n,
        indices: &[0, 2, 1],
        base_color: [1.0; 4],
        colors: Some(&c),
        ..Default::default()
    };
    let mut bytes = Vec::new();
    oxy_io::gltf::write_glb_scene(&[body, hat], None, &[], &mut bytes).unwrap();
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

/// Rigged export of the real character re-read with the `gltf` crate:
/// joint hierarchy × inverse bind = identity, weights normalised, T-pose arms
/// horizontal. Skipped without `OXY_MAKEHUMAN_DATA`.
#[test]
fn rigged_export_is_consistent_if_available() {
    use oxy_io::export::{character_glb, BindPose, ExportOptions};
    use oxy_io::glam::{Mat4, Quat, Vec3 as V};
    let Some(dir) = std::env::var_os("OXY_MAKEHUMAN_DATA") else {
        eprintln!("OXY_MAKEHUMAN_DATA not set; skipping");
        return;
    };
    let asset = oxy_io::makehuman::import(dir.as_ref()).unwrap();
    for pose in [BindPose::Rest, BindPose::TPose] {
        let mut bytes = Vec::new();
        let opts = ExportOptions {
            bind_pose: pose,
            ..Default::default()
        };
        character_glb(
            &asset,
            &asset.catalog.defaults(),
            &[],
            &Default::default(),
            &opts,
            &[],
            &[],
            &mut bytes,
        )
        .unwrap();
        let (doc, buffers, _) = gltf::import_slice(&bytes).expect("valid glb");
        let skin = doc.skins().next().expect("skin");
        let joints: Vec<_> = skin.joints().collect();
        assert_eq!(joints.len(), asset.rig.as_ref().unwrap().bones.len());

        // World transforms from the node hierarchy.
        let mut world = std::collections::HashMap::new();
        fn walk(n: gltf::Node, parent: Mat4, world: &mut std::collections::HashMap<usize, Mat4>) {
            let (t, r, s) = n.transform().decomposed();
            let m = parent
                * Mat4::from_scale_rotation_translation(
                    V::from(s),
                    Quat::from_array(r),
                    V::from(t),
                );
            world.insert(n.index(), m);
            for c in n.children() {
                walk(c, m, world);
            }
        }
        for n in doc.scenes().next().unwrap().nodes() {
            walk(n, Mat4::IDENTITY, &mut world);
        }
        let ibm: Vec<Mat4> = skin
            .reader(|b| Some(&buffers[b.index()]))
            .read_inverse_bind_matrices()
            .unwrap()
            .map(|m| Mat4::from_cols_array_2d(&m))
            .collect();
        for (j, ib) in joints.iter().zip(&ibm) {
            let err = (world[&j.index()] * *ib - Mat4::IDENTITY)
                .to_cols_array()
                .iter()
                .fold(0.0f32, |a, b| a.max(b.abs()));
            assert!(
                err < 1e-4,
                "{pose:?}: joint {:?} bind × IBM off by {err}",
                j.name()
            );
        }

        let prim = doc.meshes().next().unwrap().primitives().next().unwrap();
        // Textured body with UVs and tangents; eyes and four embedded images.
        assert!(
            prim.get(&gltf::Semantic::TexCoords(0)).is_some()
                && prim.get(&gltf::Semantic::Tangents).is_some()
        );
        assert!(prim
            .material()
            .pbr_metallic_roughness()
            .base_color_texture()
            .is_some());
        assert_eq!(doc.images().count(), 4);
        assert!(doc.meshes().any(|m| m.name() == Some("eye.L")));
        let r = prim.reader(|b| Some(&buffers[b.index()]));
        for w in r.read_weights(0).unwrap().into_f32() {
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-4);
        }
        if pose == BindPose::TPose {
            // Left wrist roughly level with the left shoulder.
            let name = |n: &str| {
                joints
                    .iter()
                    .find(|j| j.name() == Some(n))
                    .map(|j| world[&j.index()].w_axis.truncate())
                    .unwrap()
            };
            let (sh, wr) = (name("upperarm01.L"), name("wrist.L"));
            assert!(
                (wr.y - sh.y).abs() < 0.03 && wr.x > sh.x + 0.4,
                "T-pose arm: shoulder {sh:?} wrist {wr:?}"
            );
        }
    }
}

/// Skin bake and eyes on the real character (skipped without data).
/// Set `OXY_DUMP_DIR` to also write the textures as PNGs for inspection.
#[test]
fn skin_bake_and_eyes_if_available() {
    let Some(dir) = std::env::var_os("OXY_MAKEHUMAN_DATA") else {
        eprintln!("OXY_MAKEHUMAN_DATA not set; skipping");
        return;
    };
    use oxy_io::material::{eyes, iris, png_bytes, MaterialParams, SkinBaker};
    let asset = oxy_io::makehuman::import(dir.as_ref()).unwrap();
    let t0 = std::time::Instant::now();
    let baker = SkinBaker::new(&asset, 1024).expect("UVs");
    let t1 = t0.elapsed();
    let p = MaterialParams::default();
    let t = baker.bake([0.62, 0.40, 0.29], &p.skin);
    eprintln!(
        "rasterize {t1:.1?}, bake {:.1?}, coverage {:.2}",
        t0.elapsed() - t1,
        baker.map.coverage()
    );
    assert!(
        baker.map.coverage() > 0.3,
        "UV layout covers a good part of the texture"
    );
    // The lips region exists and differs from plain skin.
    assert!(baker.attrs.regions[oxy_material::skin::LIPS]
        .iter()
        .any(|&w| w > 0.6));
    assert_eq!(t.albedo.rgba.len(), 1024 * 1024 * 4);

    let ev = asset.evaluate_full(&asset.catalog.defaults());
    let e = eyes(&asset, &ev, &p.eyes);
    assert_eq!(e.len(), 2);
    let (lo, hi) = oxy_io::makehuman::bounds(&e[0].mesh.positions);
    let c = (lo + hi) * 0.5;
    assert!(
        c.y > 1.45 && c.y < 1.75 && c.x > 0.0,
        "left eye at eye height on the left: {c:?}"
    );
    assert!(
        (hi - lo).x > 0.015 && (hi - lo).x < 0.035,
        "eyeball size {:?}",
        hi - lo
    );

    if let Some(out) = std::env::var_os("OXY_DUMP_DIR") {
        let out = std::path::PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        for (name, img) in [
            ("albedo", &t.albedo),
            ("normal", &t.normal),
            ("orm", &t.orm),
        ] {
            std::fs::write(
                out.join(format!("skin_{name}.png")),
                png_bytes(img).unwrap(),
            )
            .unwrap();
        }
        std::fs::write(out.join("iris.png"), png_bytes(&iris(&p.eyes)).unwrap()).unwrap();
    }
}

/// Built-in clothes bind to the real body, follow a shape change, hide covered
/// skin, and export as skinned meshes. Skipped without `OXY_MAKEHUMAN_DATA`.
#[test]
fn clothing_fits_and_exports_if_available() {
    use oxy_fit::{author_builtins, fit, occlusion, CoverMesh, FitSettings};
    const TEE: &str = "tee";
    const PANTS: &str = "pants";
    use oxy_io::export::{character_glb, ExportOptions, WornMesh};
    let Some(dir) = std::env::var_os("OXY_MAKEHUMAN_DATA") else {
        eprintln!("OXY_MAKEHUMAN_DATA not set; skipping");
        return;
    };
    let asset = oxy_io::makehuman::import(dir.as_ref()).unwrap();
    let base = asset.evaluate_full(&asset.catalog.defaults());
    let names = asset
        .rig
        .as_ref()
        .map(|r| r.bones.iter().map(|b| b.name.clone()).collect::<Vec<_>>());
    let skin = asset.rig.as_ref().map(|r| r.skin.as_slice());
    let items = author_builtins(&base.body, &asset.mesh.base.indices, skin, names.as_deref());
    let tee = items.iter().find(|g| g.id == TEE).expect("t-shirt");
    let pants = items.iter().find(|g| g.id == PANTS).expect("trousers");
    assert!(items.iter().any(|g| g.id == "skirt"));
    assert!(items.iter().any(|g| g.id == "hair"), "hair cards");
    assert!(
        tee.mesh.positions.len() > 400,
        "shirt {:?}",
        tee.mesh.positions.len()
    );
    assert!(pants.mesh.positions.len() > 400);
    let settings = FitSettings {
        smooth_steps: 0,
        margin: 0.0,
        ..tee.settings.clone()
    };
    let same = fit(
        &base.body,
        &asset.mesh.base.indices,
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
    assert!(
        err < 2e-3,
        "shirt does not round-trip on the base body ({err})"
    );
    let tall: Vec<_> = base
        .body
        .iter()
        .map(|p| oxy_io::glam::Vec3::new(p.x, p.y * 1.15, p.z))
        .collect();
    let lifted = fit(
        &tall,
        &asset.mesh.base.indices,
        &[],
        &tee.mesh,
        &tee.binding,
        &settings,
    );
    let rise = lifted
        .iter()
        .zip(&tee.mesh.positions)
        .map(|(a, b)| a.y - b.y)
        .fold(0.0f32, f32::max);
    assert!(rise > 0.05, "shirt should rise with the body, {rise}");

    // The shirt covers a real patch of the torso.
    let shirt_idx = tee.mesh.flat_indices();
    let shirt_n = oxy_mesh::Mesh::new(tee.mesh.positions.clone(), shirt_idx.clone())
        .unwrap()
        .compute_normals(&tee.mesh.positions);
    let body_n = asset.mesh.base.compute_normals(&base.body);
    let (hidden, _) = occlusion(
        &base.body,
        &body_n,
        &asset.mesh.base.indices,
        &[CoverMesh {
            positions: &tee.mesh.positions,
            indices: &shirt_idx,
            normals: &shirt_n,
            layer: tee.settings.layer,
            hide_body: true,
            cover: tee.settings.cover,
        }],
    );
    assert!(
        hidden.len() > 200,
        "shirt should hide a patch of skin, got {}",
        hidden.len()
    );
    let mut worn = Vec::new();
    for id in [TEE, "hair"] {
        let g = items.iter().find(|g| g.id == id).unwrap();
        worn.push(WornMesh {
            name: g.name.to_string(),
            positions: g.mesh.positions.clone(),
            indices: g.mesh.flat_indices(),
            uvs: g.mesh.uvs.clone(),
            skin: Some(g.skin.clone()),
            base_color: [g.color[0], g.color[1], g.color[2], 1.0],
            hair: g.kind == oxy_fit::ItemKind::Hair,
            double_sided: true,
        });
    }
    let mut bytes = Vec::new();
    let opts = ExportOptions {
        texture_size: 64,
        ..Default::default()
    };
    character_glb(
        &asset,
        &asset.catalog.defaults(),
        &[],
        &Default::default(),
        &opts,
        &worn,
        &hidden,
        &mut bytes,
    )
    .unwrap();
    let (doc, buffers, _) = gltf::import_slice(&bytes).unwrap();
    assert!(doc.meshes().any(|m| m.name() == Some("T-shirt")));
    let hair = doc
        .meshes()
        .find(|m| m.name() == Some("Hair"))
        .expect("hair mesh");
    let mat = hair.primitives().next().unwrap().material();
    assert_eq!(mat.alpha_mode(), gltf::material::AlphaMode::Mask);
    assert!(mat.double_sided());
    let shirt = doc.meshes().find(|m| m.name() == Some("T-shirt")).unwrap();
    let prim = shirt.primitives().next().unwrap();
    let reader = prim.reader(|b| Some(&buffers[b.index()]));
    for w in reader.read_weights(0).unwrap().into_f32() {
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-3);
    }
    let body = doc.meshes().find(|m| m.name() == Some("body")).unwrap();
    let body_tris = body.primitives().next().unwrap().indices().unwrap().count() / 3;
    assert!(body_tris + 3 <= asset.mesh.base.indices.len() / 3);
}
