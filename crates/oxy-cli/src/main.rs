//! `oxy` — headless tools for OxyCharacter.
//!
//! ```text
//! oxy import-makehuman <makehuman/data dir> <out.oxyb>
//! oxy info <bundle.oxyb>
//! oxy export <bundle.oxyb> <out.glb> [--tpose] [--no-skeleton] [--blendshapes] [--no-textures] [--no-eyes] [--texture-size N] [--morphs DIR] [param=value ...]
//! oxy send-oxybrush <bundle.oxyb> <out.oxb> [param=value ...]
//! oxy import-oxybrush <bundle.oxyb> <in.oxb> [--save-morphs DIR]
//! ```

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::process::ExitCode;
use std::time::Instant;

use std::path::Path;

use oxy_io::{bundle, export, makehuman, oxb, oxybrush, CharacterAsset};

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
            match &a.rig {
                Some(r) => println!("rig {:?}: {} bones, {} joints, {} aux points", r.name, r.bones.len(), r.joints.len(), a.mesh.aux.len()),
                None => println!("no rig"),
            }
            let mut groups: Vec<String> = a.catalog.params.iter().map(|p| p.group.join(" / ")).collect();
            groups.sort();
            groups.dedup();
            for g in groups {
                let n = a.catalog.params.iter().filter(|p| p.group.join(" / ") == g).count();
                println!("  {g}: {n}");
            }
        }
        ["send-oxybrush", path, out, params @ ..] => {
            let a = load(path)?;
            let values = parse_params(&a, params)?;
            let (body, ground_offset) = a.evaluate_grounded(&values);
            let link = oxybrush::Link { params: values, ground_offset, vertex_count: body.len() };
            oxybrush::send(out.as_ref(), &body, &a.triangles(), &[], &link).map_err(|e| e.to_string())?;
            println!("wrote {out} and {}", oxybrush::link_path(out.as_ref()).display());
        }
        ["import-oxybrush", path, input, rest @ ..] => {
            let a = load(path)?;
            let save_dir = match rest {
                [] => None,
                ["--save-morphs", dir] => Some(*dir),
                _ => return Err("usage: oxy import-oxybrush <bundle> <in.oxb> [--save-morphs DIR]".into()),
            };
            let scene = oxb::load(input.as_ref()).map_err(|e| e.to_string())?;
            let link = oxybrush::read_link(input.as_ref());
            let params = link.as_ref().map_or_else(|| a.catalog.defaults(), |l| l.params.clone());
            println!(
                "{input}: {} piece(s), written by {:?}; reference body from {}",
                scene.pieces.len(),
                scene.app,
                if link.is_some() { "link file" } else { "default parameters (no link file)" }
            );
            let (reference, _) = a.evaluate_grounded(&params);
            let t = Instant::now();
            let items = oxybrush::classify(&scene, &a.triangles(), &reference);
            for item in &items {
                match item {
                    oxybrush::Imported::Morph { piece, indices, max_displacement, used_base_level, .. } => {
                        println!(
                            "  {piece}: MORPH, {} vertices moved, max {:.1} mm{}",
                            indices.len(),
                            max_displacement * 1000.0,
                            if *used_base_level { " (from subdivision level 0)" } else { "" }
                        );
                    }
                    oxybrush::Imported::Unchanged { piece } => println!("  {piece}: body, unchanged"),
                    oxybrush::Imported::Accessory(acc) => {
                        println!("  {}: ACCESSORY, {} vertices, {} triangles", acc.name, acc.positions.len(), acc.triangles.len())
                    }
                }
            }
            println!("classified in {:.1?}", t.elapsed());
            if let Some(dir) = save_dir {
                for item in items {
                    if let oxybrush::Imported::Morph { piece, indices, deltas, .. } = item {
                        let m = oxybrush::CustomMorph { id: oxybrush::slug(&piece), label: piece, indices, deltas };
                        let p = oxybrush::save_custom_morph(Path::new(dir), &m).map_err(|e| e.to_string())?;
                        println!("saved {}", p.display());
                    }
                }
            }
        }
        ["export", path, out, rest @ ..] => {
            let mut a = load(path)?;
            let mut opts = export::ExportOptions::default();
            let mut params = Vec::new();
            let mut it = rest.iter();
            while let Some(arg) = it.next() {
                match *arg {
                    "--tpose" => opts.bind_pose = export::BindPose::TPose,
                    "--no-skeleton" => opts.skeleton = false,
                    "--blendshapes" => opts.blendshapes = true,
                    "--no-textures" => opts.textures = false,
                    "--no-eyes" => opts.eyes = false,
                    "--texture-size" => {
                        opts.texture_size = it.next().and_then(|v| v.parse().ok()).ok_or("--texture-size needs a number")?;
                    }
                    "--morphs" => {
                        let dir = it.next().ok_or("--morphs needs a folder")?;
                        let (morphs, errors) = oxybrush::load_custom_morphs(Path::new(dir), a.mesh.base.vertex_count());
                        for e in errors {
                            eprintln!("warning: {e}");
                        }
                        for m in &morphs {
                            let id = a.add_custom_morph(m);
                            println!("custom morph {id} ({})", m.label);
                        }
                    }
                    kv => params.push(kv),
                }
            }
            let values = parse_params(&a, &params)?;
            let f = File::create(out).map_err(|e| format!("{out}: {e}"))?;
            let st = export::character_glb(&a, &values, &[], &Default::default(), &opts, BufWriter::new(f)).map_err(|e| e.to_string())?;
            println!("wrote {out}: {} mesh(es), {} bones, {} blendshape(s), {} texture(s)", st.meshes, st.bones, st.blendshapes, st.textures);
        }
        _ => {
            return Err("usage: oxy import-makehuman <data dir> <out.oxyb> | info <bundle> | export <bundle> <out.glb> [--tpose] [--no-skeleton] [--blendshapes] [--no-textures] [--no-eyes] [--texture-size N] [--morphs DIR] [param=value ...] | send-oxybrush <bundle> <out.oxb> [param=value ...] | import-oxybrush <bundle> <in.oxb> [--save-morphs DIR]".into())
        }
    }
    Ok(())
}

fn parse_params(a: &CharacterAsset, params: &[&str]) -> Result<oxy_morph::Params, String> {
    let mut values = a.catalog.defaults();
    for kv in params {
        let (k, v) = kv
            .split_once('=')
            .ok_or(format!("expected param=value, got {kv}"))?;
        if a.catalog.param(k).is_none() {
            return Err(format!("unknown param {k}"));
        }
        values.insert(
            k.to_owned(),
            v.parse().map_err(|_| format!("bad value {v}"))?,
        );
    }
    Ok(values)
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
