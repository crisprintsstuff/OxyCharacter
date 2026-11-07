# OxyCharacter

A clean-room, Rust-native parametric 3D character creator: shape a base human
with sliders, auto-refit its skeleton, dress it, and export rigged glTF.

![Editor](docs/images/editor-full-body.png)

See **[docs/PLAN.md](docs/PLAN.md)** for scope, architecture, algorithms and milestones.

## Quick start

```sh
# 1. Fetch the CC0 base mesh + morph targets and build the binary bundle (~50 MB)
tools/fetch-assets.sh

# 2. Run the editor
cargo run --release -p oxy-app
```

Headless tools:

```sh
cargo run --release -p oxy-cli -- info   assets/generated/makehuman.oxyb
cargo run --release -p oxy-cli -- export assets/generated/makehuman.oxyb out.glb gender=1 muscle=0.8
```

Editor options (useful for scripted screenshots / smoke tests):

```sh
oxycharacter --set gender=1 --select Head/Nose --hover nose/nose-scale-horiz-decr-incr \
             --head --screenshot shot.png --frames 40
```

Linux without a GPU: `Xvfb :99 & DISPLAY=:99 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json oxycharacter …`
(needs `mesa-vulkan-drivers`).

## Editor

- **Content** (left): body presets with procedural silhouette cards, skin-tone swatches + custom color.
- **Viewport** (center): orbit (LMB), pan (RMB/MMB), zoom (wheel); stats overlay.
- **Morph** (right): search, category tree with modified counts, sliders with value
  readout, reset and range-end labels. Hovering or dragging a slider tints the
  region it affects. Symmetry links left/right sliders. Double-click resets a slider.
- **Toolbar**: reset, randomize, body/head framing, symmetry, glTF export (to `exports/`).

![Region highlight](docs/images/editor-nose-highlight.png)

## Sculpting in OxyBrush

OxyCharacter round-trips with [OxyBrush](https://github.com/crisprintsstuff/OxyBrush)
through its native `.oxb` project files: no export/import dance, no shared code.

1. **Send to OxyBrush** (toolbar or File menu) writes the current character to
   `user/oxybrush/character-<time>.oxb` and opens it in OxyBrush
   (`$OXYBRUSH_BIN`, else `oxybrush` on `PATH`; if neither starts, open the file yourself).
2. Sculpt the body, add new pieces (hats, hair, props…), and **save** (Ctrl+S).
3. OxyCharacter notices the save and shows **Import from OxyBrush**:
   - the reshaped **body becomes a morph slider** under *Custom › Sculpted*
     (saved to `user/morphs/` and reloaded on start; 🗑 deletes it);
   - **new pieces become accessories** (left panel → *Accessories*), exported with the body.

You can also import any `.oxb` with **Import .oxb** or by dropping it on the window.

How it works: OxyBrush re-sorts vertices when it loads a mesh, so OxyCharacter
recovers the vertex correspondence from mesh connectivity (≈30–60 ms for the
13k-vertex body) and measures the sculpt against the body it sent (parameters
are kept in a `.oxylink.json` file next to the `.oxb`). Sculpting on
**subdivision levels** (Ctrl+D in OxyBrush) is fine: level 0 becomes the
morph; detail finer than the base mesh stays in OxyBrush for now. Remeshing or
destructive subdivision changes the topology, so such a body can only come
back as a separate mesh. See [ADR 0004](docs/adr/0004-oxybrush-bridge.md).

![A sculpt coming back from OxyBrush](docs/images/oxybrush-live-loop.png)

![Imported morph slider and accessory](docs/images/oxybrush-imported.png)

Headless equivalents:

```sh
oxy send-oxybrush   assets/generated/makehuman.oxyb out.oxb gender=1
oxy import-oxybrush assets/generated/makehuman.oxyb out.oxb --save-morphs user/morphs
```

## Status

| Milestone | State |
|---|---|
| M0 foundations | ✅ |
| M1 see a human (import, bundle, editor, static glTF export) | ✅ |
| OxyBrush bridge (send, live re-import as morphs/accessories) | ✅ |
| M2 rigged & exportable | next |

| Crate | What it does |
|---|---|
| `oxy-mesh` | Mesh, sparse morph targets, blending, normals, topology correspondence |
| `oxy-morph` | Sliders, macro grids with gains, parameter catalog |
| `oxy-rig` | Skeleton with vertex-anchor joint refitting |
| `oxy-io` | OBJ / sparse-target import, MakeHuman CC0 importer, `.oxyb` bundle, `.glb` export, `.oxb` read/write, OxyBrush bridge |
| `oxy-cli` | `oxy` command-line tool |
| `oxy-app` | Bevy + egui editor (`oxycharacter`) |

```sh
cargo test --workspace
# include the real-data integration test:
OXY_MAKEHUMAN_DATA=assets/cache/makehuman/makehuman/data cargo test --workspace --release
```

Assets and their licenses: [assets/LICENSES.md](assets/LICENSES.md).
