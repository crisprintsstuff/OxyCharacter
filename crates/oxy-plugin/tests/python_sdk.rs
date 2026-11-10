//! Runs real Python plugins against the SDK in `python/` (skipped without Python).

use std::path::PathBuf;

use oxy_io::glam::Vec3;
use oxy_io::{AssetMeta, CharacterAsset};
use oxy_mesh::{Mesh, MorphTarget, MorphableMesh};
use oxy_plugin::api::SimpleHost;
use oxy_plugin::runner::{python_available, run_blocking, spawn};

fn host() -> SimpleHost<'static> {
    let base = Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
        vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
    )
    .unwrap();
    let catalog = oxy_morph::Catalog {
        params: vec![oxy_morph::ParamDef {
            id: "nose".into(),
            label: "Nose".into(),
            group: vec!["Head".into(), "Nose".into()],
            min: 0.0,
            max: 1.0,
            default: 0.0,
            mirror: None,
            ends: None,
        }],
        sliders: vec![oxy_morph::Slider::Unipolar {
            id: "nose".into(),
            target: "nose".into(),
        }],
        layers: vec![],
    };
    SimpleHost::new(CharacterAsset {
        meta: AssetMeta {
            name: "tetra".into(),
            ..Default::default()
        },
        mesh: MorphableMesh {
            base,
            targets: vec![MorphTarget::new("nose", vec![3], vec![Vec3::Z])],
            aux: vec![],
        },
        catalog,
        rig: None,
        render: None,
    })
}

fn sdk() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../python")
}

fn script(name: &str, body: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxy-plugin-py-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p
}

#[test]
fn python_plugin_round_trip() {
    if !python_available() {
        eprintln!("python not available; skipping");
        return;
    }
    let path = script(
        "roundtrip.py",
        r#"
import oxycharacter, sys
app = oxycharacter.attach()
print("hello from print")          # must not corrupt the protocol
assert app.info["api_version"] == 1 and app.info["host"] == "cli"
app.set_params(nose=0.75)
assert app.get("nose") == {"nose": 0.75}
try:
    app.set_params(bogus=1)
    raise SystemExit("expected an error")
except oxycharacter.PluginError as e:
    assert e.code == -32602, e.code
m = app.measure()
assert abs(m["depth"] - 1.75) < 1e-5, m
app.set_material(eyes={"iris": [0.1, 0.2, 0.3]})
sid = app.add_morph("Lift", indices=[1], deltas=[[0, 0.5, 0]], value=0.5)
app.notify(f"made {sid} with args {sys.argv[1:]}")
"#,
    );
    let mut h = host();
    let child = spawn(&path, Some(&sdk()), &["one".into(), "two".into()]).unwrap();
    let mut log = Vec::new();
    let done = run_blocking(child, &mut h, |l| log.push(l.to_owned())).unwrap();
    assert!(done.status.success(), "plugin failed: {log:?}");
    assert!(log.iter().any(|l| l == "hello from print"), "{log:?}");
    assert_eq!(h.params["nose"], 0.75);
    assert_eq!(h.material.eyes.iris, [0.1, 0.2, 0.3]);
    assert_eq!(h.params["custom/lift"], 0.5);
    assert_eq!(h.notes, vec!["made custom/lift with args ['one', 'two']"]);
}

#[test]
fn failing_plugin_reports_status_and_traceback() {
    if !python_available() {
        return;
    }
    let path = script(
        "boom.py",
        "import oxycharacter\noxycharacter.attach()\nraise RuntimeError('boom')\n",
    );
    let mut h = host();
    let mut log = Vec::new();
    let done = run_blocking(spawn(&path, Some(&sdk()), &[]).unwrap(), &mut h, |l| {
        log.push(l.to_owned())
    })
    .unwrap();
    assert!(!done.status.success());
    assert!(
        log.iter().any(|l| l.contains("RuntimeError: boom")),
        "{log:?}"
    );
}
