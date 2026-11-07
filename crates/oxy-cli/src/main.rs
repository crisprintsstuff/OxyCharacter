//! `oxy` — headless tools for OxyCharacter.
//!
//! ```text
//! oxy import-makehuman <makehuman/data dir> <out.oxyb>
//! oxy info <bundle.oxyb>
//! oxy export <bundle.oxyb> <out.glb> [param=value ...]
//! ```

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::process::ExitCode;
use std::time::Instant;

use oxy_io::{bundle, gltf, makehuman, CharacterAsset};

fn load(path: &str) -> Result<CharacterAsset, String> {
    bundle::read(BufReader::new(
        File::open(path).map_err(|e| format!("{path}: {e}"))?,
    ))
    .map_err(|e| e.to_string())
}

fn run(args: &[String]) -> Result<(), String> {
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["import-makehuman", data, out] => {
            let t = Instant::now();
            let asset = makehuman::import(data.as_ref()).map_err(|e| e.to_string())?;
            let f = File::create(out).map_err(|e| format!("{out}: {e}"))?;
            bundle::write(&asset, BufWriter::new(f)).map_err(|e| e.to_string())?;
            println!(
                "wrote {out}: {} vertices, {} targets, {} params ({:.1?})",
                asset.mesh.base.vertex_count(),
                asset.mesh.targets.len(),
                asset.catalog.params.len(),
                t.elapsed()
            );
        }
        ["info", path] => {
            let a = load(path)?;
            println!("{} — {} ({})", a.meta.name, a.meta.license, a.meta.source);
            println!("vertices {}, triangles {}", a.mesh.base.vertex_count(), a.mesh.base.indices.len() / 3);
            println!("targets {}, params {}, macro layers {}", a.mesh.targets.len(), a.catalog.params.len(), a.catalog.layers.len());
            let mut groups: Vec<String> = a.catalog.params.iter().map(|p| p.group.join(" / ")).collect();
            groups.sort();
            groups.dedup();
            for g in groups {
                let n = a.catalog.params.iter().filter(|p| p.group.join(" / ") == g).count();
                println!("  {g}: {n}");
            }
        }
        ["export", path, out, params @ ..] => {
            let a = load(path)?;
            let mut values = a.catalog.defaults();
            for kv in params {
                let (k, v) = kv.split_once('=').ok_or(format!("expected param=value, got {kv}"))?;
                if a.catalog.param(k).is_none() {
                    return Err(format!("unknown param {k}"));
                }
                values.insert(k.to_owned(), v.parse().map_err(|_| format!("bad value {v}"))?);
            }
            let (dense, missing) = oxy_morph::resolve(&a.mesh, &a.catalog.evaluate(&values));
            if !missing.is_empty() {
                eprintln!("warning: {} unknown targets", missing.len());
            }
            let positions = a.mesh.evaluate(&dense);
            let normals = a.mesh.base.compute_normals(&positions);
            let f = File::create(out).map_err(|e| format!("{out}: {e}"))?;
            let mesh = gltf::ExportMesh {
                name: "character",
                positions: &positions,
                normals: &normals,
                indices: &a.mesh.base.indices,
                base_color: [0.8, 0.62, 0.52, 1.0],
            };
            gltf::write_glb(&mesh, BufWriter::new(f)).map_err(|e| e.to_string())?;
            println!("wrote {out}");
        }
        _ => return Err("usage: oxy import-makehuman <data dir> <out.oxyb> | info <bundle> | export <bundle> <out.glb> [param=value ...]".into()),
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
