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

## Status

| Milestone | State |
|---|---|
| M0 foundations | ✅ |
| M1 see a human (import, bundle, editor, static glTF export) | ✅ |
| M2 rigged & exportable | next |

| Crate | What it does |
|---|---|
| `oxy-mesh` | Mesh, sparse morph targets, blending, normals |
| `oxy-morph` | Sliders, macro grids with gains, parameter catalog |
| `oxy-rig` | Skeleton with vertex-anchor joint refitting |
| `oxy-io` | OBJ / sparse-target import, MakeHuman CC0 importer, `.oxyb` bundle, `.glb` export |
| `oxy-cli` | `oxy` command-line tool |
| `oxy-app` | Bevy + egui editor (`oxycharacter`) |

```sh
cargo test --workspace
# include the real-data integration test:
OXY_MAKEHUMAN_DATA=assets/cache/makehuman/makehuman/data cargo test --workspace --release
```

Assets and their licenses: [assets/LICENSES.md](assets/LICENSES.md).
