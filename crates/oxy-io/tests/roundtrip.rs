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
