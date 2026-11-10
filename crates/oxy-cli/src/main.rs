//! `oxy` — headless tools for OxyCharacter.
//!
//! ```text
//! oxy import-makehuman <makehuman/data dir> <out.oxyb>
//! oxy info <bundle.oxyb>
//! oxy export <bundle.oxyb> <out.glb> [--tpose] [--no-skeleton] [--blendshapes] [--no-textures] [--no-eyes] [--expression smile,mouth-open:0.4] [--no-expressions] [--texture-size N] [--morphs DIR] [--wear tee,pants,hair] [--wardrobe DIR] [--accessory FILE.obj] [param=value ...]
//! oxy wardrobe <bundle.oxyb>
//! oxy expressions <bundle.oxyb>
//! oxy plugin <bundle.oxyb> <script.py> [--open project.oxyc] [--morphs DIR] [-- script args ...]
//! oxy send-oxybrush <bundle.oxyb> <out.oxb> [param=value ...]
//! oxy import-oxybrush <bundle.oxyb> <in.oxb> [--save-morphs DIR]
//! ```

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::process::ExitCode;
use std::time::Instant;

use std::path::{Path, PathBuf};

use oxy_fit::{author_builtins, bind, fit, occlusion, transfer_weights, CoverMesh, FitMethod};
use oxy_io::export::WornMesh;
use oxy_io::{bundle, export, locate, makehuman, obj, oxb, oxybrush, CharacterAsset};

fn load(path: &str) -> Result<CharacterAsset, String> {
    // A bare `makehuman.oxyb` is also accepted from another working directory
    // when the file sits beside this executable (or in the macOS / AppImage
    // install layout). An existing relative path still wins.
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("oxy"));
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let resolved = locate::resolve_bundle_argument(Path::new(path), &exe, &cwd);
    bundle::read(BufReader::new(File::open(&resolved).map_err(|e| {
        format!(
            "{}: {e} (from {path}; a bare file name is also tried beside the executable)",
            resolved.display()
        )
    })?))
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
        ["plugin", path, script, rest @ ..] => {
            run_plugin(path, script, rest)?;
        }
        ["expressions", path] => {
            use oxy_io::expression::{available, GROUPS, PRESETS, UNITS, VISEMES};
            let a = load(path)?;
            println!("{} of {} expression units in this bundle", available(&a).count(), UNITS.len());
            for g in GROUPS {
                let ids: Vec<&str> = available(&a).filter(|u| u.group == g).map(|u| u.id).collect();
                println!("  {g}: {}", ids.join(", "));
            }
            println!("presets: {}", PRESETS.iter().map(|m| m.id).collect::<Vec<_>>().join(", "));
            println!("visemes: {}", VISEMES.iter().map(|m| m.id).collect::<Vec<_>>().join(", "));
        }
        ["wardrobe", path] => {
            let a = load(path)?;
            let ev = a.evaluate_full(&a.catalog.defaults());
            let names = a.rig.as_ref().map(|r| r.bones.iter().map(|b| b.name.clone()).collect::<Vec<_>>());
            let items = author_builtins(&ev.body, &a.mesh.base.indices, a.rig.as_ref().map(|r| r.skin.as_slice()), names.as_deref());
            if items.is_empty() {
                println!("no built-in garments for this body");
            }
            for g in items {
                println!(
                    "{}\t{}\t{:?}\t{:?}\t{} verts",
                    g.id,
                    g.name,
                    g.kind,
                    g.settings.method,
                    g.mesh.positions.len()
                );
            }
        }
        ["export", path, out, rest @ ..] => {
            let mut a = load(path)?;
            let mut opts = export::ExportOptions::default();
            let mut params = Vec::new();
            let mut wear = None;
            let mut wardrobe = None;
            let mut accessories = Vec::new();
            let mut it = rest.iter();
            while let Some(arg) = it.next() {
                match *arg {
                    "--tpose" => opts.bind_pose = export::BindPose::TPose,
                    "--no-skeleton" => opts.skeleton = false,
                    "--blendshapes" => opts.blendshapes = true,
                    "--no-textures" => opts.textures = false,
                    "--no-eyes" => opts.eyes = false,
                    "--no-expressions" => opts.expressions = false,
                    "--expression" => {
                        opts.expression = oxy_io::expression::Expression::parse(it.next().ok_or("--expression needs a name, e.g. smile or mouth-open:0.5")?)?;
                    }
                    "--texture-size" => {
                        opts.texture_size = it.next().and_then(|v| v.parse().ok()).ok_or("--texture-size needs a number")?;
                    }
                    "--wear" => wear = Some(it.next().ok_or("--wear needs a list, e.g. tee,pants,hair")?),
                    "--wardrobe" => wardrobe = Some(it.next().ok_or("--wardrobe needs a folder")?),
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
                    "--accessory" => {
                        let file = it.next().ok_or("--accessory needs an .obj file")?;
                        accessories.push(load_accessory(file)?);
                        println!("accessory {}", accessories.last().map(|a| a.name.as_str()).unwrap_or(file));
                    }
                    kv => params.push(kv),
                }
            }
            let values = parse_params(&a, &params)?;
            let (worn, hidden) = assemble_clothing(&a, &values, wear.copied(), wardrobe.copied())?;
            let f = File::create(out).map_err(|e| format!("{out}: {e}"))?;
            let st = export::character_glb(&a, &values, &accessories, &Default::default(), &opts, &worn, &hidden, BufWriter::new(f)).map_err(|e| e.to_string())?;
            println!(
                "wrote {out}: {} mesh(es), {} bones, {} blendshape(s), {} texture(s), {} hidden body triangle(s)",
                st.meshes, st.bones, st.blendshapes, st.textures, hidden.len()
            );
        }
        _ => {
            return Err("usage: oxy import-makehuman <data dir> <out.oxyb> | info <bundle> | wardrobe <bundle> | expressions <bundle> | export <bundle> <out.glb> [--tpose] [--no-skeleton] [--blendshapes] [--no-textures] [--no-eyes] [--expression smile,mouth-open:0.4] [--no-expressions] [--texture-size N] [--morphs DIR] [--wear tee,pants:0.8,hair] [--wardrobe DIR] [--accessory FILE.obj] [param=value ...] | plugin <bundle> <script.py> [--open project.oxyc] [--morphs DIR] [-- args ...] | send-oxybrush <bundle> <out.oxb> [param=value ...] | import-oxybrush <bundle> <in.oxb> [--save-morphs DIR]".into())
        }
    }
    Ok(())
}

/// Built-ins are authored on the default body and fitted to `params`.
/// `--wear` is `id` or `id:tightness`, comma-separated. `--wardrobe` is a
/// package folder.
/// Run a plugin headless against the bundle (optionally starting from a project).
fn run_plugin(bundle: &str, script: &str, rest: &[&str]) -> Result<(), String> {
    use oxy_plugin::api::SimpleHost;
    let mut asset = load(bundle)?;
    let (mut open, mut morphs, mut script_args) = (None, None, Vec::new());
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match *a {
            "--open" => open = Some(*it.next().ok_or("--open needs a project file")?),
            "--morphs" => morphs = Some(*it.next().ok_or("--morphs needs a folder")?),
            "--" => {
                script_args.extend(it.by_ref().map(|s| s.to_string()));
            }
            other => {
                return Err(format!(
                    "unknown option {other} (pass script arguments after --)"
                ))
            }
        }
    }
    if let Some(dir) = morphs {
        let (list, errors) =
            oxybrush::load_custom_morphs(Path::new(dir), asset.mesh.base.vertex_count());
        for e in errors {
            eprintln!("warning: {e}");
        }
        for m in &list {
            asset.add_custom_morph(m);
        }
    }
    let ev = asset.evaluate_full(&asset.catalog.defaults());
    let names = asset
        .rig
        .as_ref()
        .map(|r| r.bones.iter().map(|b| b.name.clone()).collect::<Vec<_>>());
    let wardrobe_items: Vec<(String, String)> = author_builtins(
        &ev.body,
        &asset.mesh.base.indices,
        asset.rig.as_ref().map(|r| r.skin.as_slice()),
        names.as_deref(),
    )
    .into_iter()
    .map(|g| (g.id.to_string(), g.name.to_string()))
    .collect();

    let mut host = SimpleHost::new(asset);
    host.wardrobe_items = wardrobe_items;
    if let Some(p) = open {
        let mut project =
            oxy_project::Project::load(Path::new(p)).map_err(|e| format!("{p}: {e}"))?;
        for w in project.reconcile(&host.asset) {
            eprintln!("note: {w}");
        }
        for m in &project.custom_morphs {
            host.asset.add_custom_morph(m);
        }
        host.params = host.asset.catalog.defaults();
        host.params.extend(project.params.clone());
        host.material = project.material.clone();
        host.skin_tone = project.skin;
        host.pose = oxy_rig::pose::PRESETS
            .iter()
            .position(|x| x.name == project.pose)
            .unwrap_or(0);
        host.export = project.export.clone();
        host.expression = project.expression.clone();
        let worn: Vec<String> = project
            .wardrobe
            .iter()
            .filter_map(|w| w.builtin.clone())
            .collect();
        if !worn.is_empty() {
            host.wear = Some(worn.join(","));
        }
        if project.wardrobe.iter().any(|w| w.builtin.is_none()) {
            eprintln!("note: imported garments in the project are not available to headless plugins (built-in ones are)");
        }
    }
    host.exporter = Box::new(|v, path, opts| {
        let (worn, hidden) = assemble_clothing(v.asset, v.params, v.wear, None)?;
        let f = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let st = export::character_glb(
            v.asset,
            v.params,
            &[],
            v.material,
            opts,
            &worn,
            &hidden,
            BufWriter::new(f),
        )
        .map_err(|e| e.to_string())?;
        Ok(
            serde_json::json!({ "path": path, "meshes": st.meshes, "bones": st.bones, "blendshapes": st.blendshapes, "textures": st.textures }),
        )
    });
    host.saver = Box::new(|v, path| {
        let path = if path.extension().is_none() {
            path.with_extension(oxy_project::EXTENSION)
        } else {
            path.to_owned()
        };
        let mut p = oxy_project::Project::new(oxy_project::BaseRef::of(v.asset));
        p.set_params(v.asset, v.params);
        p.skin = v.skin_tone;
        p.pose = oxy_rig::pose::PRESETS[v.pose].name.into();
        p.material = v.material.clone();
        p.expression = v.expression.clone();
        p.custom_morphs = v
            .asset
            .catalog
            .params
            .iter()
            .filter(|d| {
                d.id.starts_with(oxybrush::CUSTOM_PREFIX)
                    && v.params.get(&d.id).is_some_and(|x| x.abs() > 1e-6)
            })
            .filter_map(|d| {
                let t = &v.asset.mesh.targets[v.asset.mesh.target_index(&d.id)?];
                Some(oxybrush::CustomMorph {
                    id: d.id.trim_start_matches(oxybrush::CUSTOM_PREFIX).into(),
                    label: d.label.clone(),
                    indices: t.indices.clone(),
                    deltas: t.deltas.clone(),
                })
            })
            .collect();
        if v.wear.is_some_and(|w| !w.is_empty()) {
            eprintln!("note: worn items are not saved by headless plugins yet");
        }
        p.save(&path).map_err(|e| e.to_string())?;
        Ok(path)
    });

    let sdk = oxy_plugin::runner::sdk_dir();
    if sdk.is_none() {
        eprintln!("warning: the oxycharacter Python SDK was not found; set OXY_PYTHON_SDK");
    }
    let child = oxy_plugin::runner::spawn(Path::new(script), sdk.as_deref(), &script_args)
        .map_err(|e| {
            format!(
                "could not start {} for {script}: {e}",
                oxy_plugin::runner::python_command().display()
            )
        })?;
    let t = Instant::now();
    let done = oxy_plugin::runner::run_blocking(child, &mut host, |line| eprintln!("{line}"))
        .map_err(|e| e.to_string())?;
    for n in &host.notes {
        println!("{n}");
    }
    println!(
        "plugin finished: {} ({} calls, {:.1?})",
        done.status,
        done.calls,
        t.elapsed()
    );
    if done.status.success() {
        Ok(())
    } else {
        Err(format!("plugin exited with {}", done.status))
    }
}

fn assemble_clothing(
    asset: &CharacterAsset,
    params: &oxy_morph::Params,
    wear: Option<&str>,
    wardrobe: Option<&str>,
) -> Result<(Vec<WornMesh>, Vec<u32>), String> {
    let base = asset.evaluate_full(&asset.catalog.defaults());
    let shaped = asset.evaluate_full(params);
    let names = asset
        .rig
        .as_ref()
        .map(|r| r.bones.iter().map(|b| b.name.clone()).collect::<Vec<_>>());
    let skin = asset.rig.as_ref().map(|r| r.skin.as_slice());
    let indices = &asset.mesh.base.indices;
    let authored = author_builtins(&base.body, indices, skin, names.as_deref());
    let mut selected: Vec<(String, Option<f32>)> = Vec::new();
    if let Some(spec) = wear {
        for part in spec.split(',').filter(|s| !s.is_empty()) {
            let (id, tight) = part
                .split_once(':')
                .map(|(i, t)| (i.trim(), t.parse::<f32>().ok()))
                .unwrap_or((part.trim(), None));
            if tight.is_none() && part.contains(':') {
                return Err(format!("bad tightness in {part}"));
            }
            selected.push((id.to_string(), tight));
        }
    }
    struct Piece {
        name: String,
        positions: Vec<oxy_io::glam::Vec3>,
        indices: Vec<u32>,
        uvs: Vec<[f32; 2]>,
        skin: Option<Vec<oxy_rig::Influence>>,
        color: [f32; 3],
        hair: bool,
        layer: u16,
        hide_body: bool,
        cover: f32,
    }
    let mut pieces = Vec::new();
    for (id, tight) in &selected {
        let Some(g) = authored.iter().find(|g| g.id == id) else {
            let known: Vec<_> = authored.iter().map(|g| g.id).collect();
            return Err(format!("unknown garment {id}; built-ins: {known:?}"));
        };
        let mut settings = g.settings.clone();
        if let Some(t) = tight {
            settings.tightness = t.clamp(0.0, 1.0);
        }
        let positions = if settings.method == FitMethod::Rigid {
            g.mesh.positions.clone()
        } else {
            fit(&shaped.body, indices, &[], &g.mesh, &g.binding, &settings)
        };
        pieces.push(Piece {
            name: g.name.to_string(),
            indices: g.mesh.flat_indices(),
            uvs: g.mesh.uvs.clone(),
            skin: (!g.skin.is_empty()).then(|| g.skin.clone()),
            color: g.color,
            hair: g.kind == oxy_fit::ItemKind::Hair,
            layer: settings.layer,
            hide_body: settings.hide_body,
            cover: settings.cover,
            positions,
        });
    }
    if let Some(dir) = wardrobe {
        let dir = std::path::Path::new(dir);
        let manifest = oxy_fit::Manifest::from_json(
            &std::fs::read(dir.join("manifest.json")).map_err(|e| e.to_string())?,
        )?;
        for item in manifest.items {
            let path = dir.join(&item.file);
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let mesh =
                obj::to_garment(&text, &path.display().to_string()).map_err(|e| e.to_string())?;
            let binding = bind(&base.body, indices, &[], &mesh.positions);
            let weights = skin.map(|s| transfer_weights(s, &binding, indices, &mesh.neighbors, 6));
            let positions = if item.settings.method == FitMethod::Rigid {
                mesh.positions.clone()
            } else {
                fit(&shaped.body, indices, &[], &mesh, &binding, &item.settings)
            };
            pieces.push(Piece {
                name: item.name,
                indices: mesh.flat_indices(),
                uvs: mesh.uvs,
                skin: weights,
                color: [0.22, 0.24, 0.28],
                hair: item.kind == oxy_fit::ItemKind::Hair,
                layer: item.settings.layer,
                hide_body: item.settings.hide_body,
                cover: item.settings.cover,
                positions,
            });
        }
    }
    let normals: Vec<Vec<oxy_io::glam::Vec3>> = pieces
        .iter()
        .map(|p| {
            oxy_mesh::Mesh::new(p.positions.clone(), p.indices.clone())
                .map(|m| m.compute_normals(&p.positions))
                .unwrap_or_else(|_| vec![oxy_io::glam::Vec3::Y; p.positions.len()])
        })
        .collect();
    let body_normals = asset.mesh.base.compute_normals(&shaped.body);
    let covers: Vec<CoverMesh<'_>> = pieces
        .iter()
        .zip(&normals)
        .map(|(p, n)| CoverMesh {
            positions: &p.positions,
            indices: &p.indices,
            normals: n,
            layer: p.layer,
            hide_body: p.hide_body,
            cover: p.cover,
        })
        .collect();
    let (hidden, per) = occlusion(&shaped.body, &body_normals, indices, &covers);
    let worn = pieces
        .into_iter()
        .zip(per)
        .map(|(p, self_hidden)| WornMesh {
            name: p.name,
            positions: p.positions,
            indices: oxy_fit::without_triangles(&p.indices, &self_hidden),
            uvs: p.uvs,
            skin: p.skin,
            base_color: [p.color[0], p.color[1], p.color[2], 1.0],
            hair: p.hair,
            double_sided: true,
        })
        .collect();
    Ok((worn, hidden))
}

/// OBJ mesh as an accessory. Faces are fan-triangulated; the file stem is the name.
fn load_accessory(path: &str) -> Result<oxybrush::Accessory, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let obj = oxy_io::obj::parse(&text, path).map_err(|e| e.to_string())?;
    let triangles = obj.triangles();
    if obj.positions.is_empty() || triangles.is_empty() {
        return Err(format!("{path}: no triangles"));
    }
    let name = Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "accessory".into());
    Ok(oxybrush::Accessory {
        name,
        positions: obj.positions,
        triangles,
        color: vec![],
    })
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
