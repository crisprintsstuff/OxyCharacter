# Testing OxyCharacter

What to run, and how to check the things unit tests can't see. Agents: also
read [AGENTS.md](../AGENTS.md).

## 1. Always

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Unit tests use tiny synthetic meshes and need no assets.

## 2. Real data

```bash
tools/fetch-assets.sh        # sparse checkout of CC0 MakeHuman data → assets/generated/makehuman.oxyb
OXY_MAKEHUMAN_DATA=assets/cache/makehuman/makehuman/data cargo test --workspace --release
```

The gated tests in `crates/oxy-io/tests/roundtrip.rs` check the importer
(sensible heights, every slider resolves, mirrors exist), the rig (validates,
joints inside the body, refits with height, identity pose = rest mesh) and the
rigged export (re-read with the `gltf` crate: world bind × inverse bind =
identity, weights sum to 1, T-pose arms level). Without the variable they
print "skipping" and pass.

## 3. Headless editor screenshots

```bash
Xvfb :99 -screen 0 1920x1080x24 &        # once
export DISPLAY=:99 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json
export OXY_USER_DIR=$(mktemp -d)         # keep runs out of ./user
cargo build --release -p oxy-app
B=./target/release/oxycharacter
$B --screenshot full.png --frames 40
$B --select Head/Nose --hover nose/nose-scale-horiz-decr-incr --head --screenshot nose.png
$B --pose-tab --pose wave --skeleton --screenshot pose.png
$B --wardrobe-tab --wear tee,pants,hair --screenshot wardrobe.png
$B --pose wave --wear tee,pants,hair --screenshot wardrobe-pose.png
$B --set gender=0.1 --save-as $OXY_USER_DIR/p.oxyc --screenshot a.png
$B --open $OXY_USER_DIR/p.oxyc --set age=0.8 --close --screenshot prompt.png   # unsaved-changes prompt
```

`--frames N` takes the screenshot after N frames, then exits. **Look at the
image** after any UI or rendering change. Software rendering runs at ~10 FPS.

## 4. Exported glTF

```bash
cargo run --release -p oxy-cli -- export assets/generated/makehuman.oxyb /tmp/rest.glb
cargo run --release -p oxy-cli -- export assets/generated/makehuman.oxyb /tmp/tpose.glb --tpose
cargo run --release -p oxy-cli -- wardrobe assets/generated/makehuman.oxyb
cargo run --release -p oxy-cli -- export assets/generated/makehuman.oxyb /tmp/dressed.glb --wear tee,pants,hair --texture-size 256
cd tools/validate-gltf && npm install
node validate.mjs /tmp/rest.glb /tmp/tpose.glb              # must be 0 errors, 0 warnings
cp /tmp/rest.glb . && node threejs-shot.cjs "rest.glb,rest.glb:bend" /tmp/three.png
```

The three.js render is an independent engine check: `:bend` rotates bones in
three.js, so a correct skin deforms smoothly. three.js strips `.` from node
names (`upperarm01.L` → `upperarm01L`).

Still manual and not yet done: importing into Blender, Godot, Unity, Unreal.

## 5. OxyBrush round trip

Build OxyBrush's CLI (`cargo build --release -p oxy-cli` in the OxyBrush repo,
binary `oxybrush-cli`), then:

```bash
oxy send-oxybrush assets/generated/makehuman.oxyb /tmp/c.oxb gender=1
oxybrush-cli info /tmp/c.oxb
oxybrush-cli convert /tmp/c.oxb /tmp/c2.oxb && cp /tmp/c.oxb.oxylink.json /tmp/c2.oxb.oxylink.json
oxy import-oxybrush assets/generated/makehuman.oxyb /tmp/c2.oxb       # → "body, unchanged"
```

For a real sculpt, drive OxyBrush's headless MCP server
(`oxybrush-cli mcp FILE.oxb`, LSP-style `Content-Length` framing over stdio):
`initialize`, then `tools/call` with `select_brush {"name":"draw"}`,
`set_brush {"size":6,"strength":0.35}`, `stroke {"space":"world","points":[[x,y,z],…]}`
(nose tip ≈ `[0, 1.58, 0.168]` m on the default body), optionally
`import {"path":"cap.obj"}`, then `save {"path":…}`. Importing the result
should report one MORPH and one ACCESSORY. Note that MCP `subdivide` is
destructive (not subdivision levels), so such a body comes back as an accessory
— that is correct behaviour.

Live loop in the editor: run with `--send-oxybrush --frames 260 --screenshot …`
and save over the sent file from another process; the import dialog should
appear in the screenshot.

## 6. Reporting

In PRs and summaries, list which of the above you ran. Don't claim results for
steps you didn't run (real GPU, other OSes, other engines).
