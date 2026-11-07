# OxyCharacter — Plan & Implementation Path

OxyCharacter is a clean-room, Rust-native **parametric 3D character creator**.
It belongs to the same category of tool as commercial character creators: you
start from a base human, shape it with sliders, set skin and materials, fit
clothing and hair, and export a rigged, game-ready character.

---

## 0. Clean-room ground rules (read first)

"Clean-room" means we reproduce the *category of functionality*, not anyone's
implementation, assets, or trade dress.

| Allowed | Not allowed |
|---|---|
| Studying public marketing pages, docs and videos to learn *what features exist* | Decompiling, disassembling or debugging the commercial binaries |
| Using open standards (glTF 2.0, FBX via open readers, USD, OBJ, ARKit blendshape *names*) | Reverse-engineering proprietary file formats (e.g. the vendor's avatar/content containers) |
| Original or permissively licensed assets (CC0 / CC-BY / our own) | Shipping, converting or "inspired-by tracing" of vendor base meshes, morphs, textures or content-store items |
| Our own names, UI and icons | Using the vendor's product names, logos, UI layout or screenshots as design references to copy |

Process rules:
1. Keep a `docs/specs/` folder of **behavioral specs written in our own words**
   ("the user can drag a slider that changes nose width; clothing follows").
   Implementers work from these specs, not from the commercial product.
2. Every asset in `assets/` gets an entry in `assets/LICENSES.md` (source, license, author).
3. Record design decisions in `docs/adr/` so the independent origin of each
   algorithm can be shown.
4. Interoperability with other tools happens **only through open formats**
   (glTF/FBX/USD export, ARKit-style expression names), never by reading
   vendor-specific files.

---

## 1. Feature scope

What the finished tool does, grouped by subsystem:

| Subsystem | Capabilities |
|---|---|
| **Base body** | One base human mesh with a consistent topology (quad-dominant, ~15–25k verts), UVs, eyes/teeth/tongue/eyelash sub-meshes |
| **Morphs** | Hundreds of sparse morph targets; user-facing sliders (bipolar ±, unipolar), macro sliders (gender, age, weight, muscle, height) that blend lower-level targets, left/right symmetry, slider dependencies |
| **Rig** | Humanoid skeleton that **auto-refits** to the morphed body; skin weights; optional facial bones; twist bones |
| **Expressions** | Facial blendshape set (52 ARKit-style names + extras), corrective shapes that are re-derived after body morphing |
| **Materials** | PBR skin with layered texture editing (base, tone, makeup, tattoos, blemishes, SSS, micro-normal), eyes, hair shading |
| **Content fitting** | Clothing, shoes and hair *conform* to any morphed body; skin-weight transfer; per-item hide-under-clothes masks |
| **Posing / preview** | Pose library, simple animation playback (glTF clips), turntable, lighting presets |
| **Export** | glTF 2.0 (primary), FBX (secondary), USD (later); LOD generation; texture baking/atlasing; engine presets (Unity/Unreal/Godot/Bevy) |
| **Project files** | Our own versioned, diff-friendly format (RON/JSON + binary blobs) storing *parameters*, not baked meshes |
| **Advanced (stretch)** | Photo-to-head fitting, procedural hair cards, cloth physics preview, plugin/scripting API |

---

## 2. Architecture

```
            ┌────────────────────────────── oxy-app (Bevy + egui) ──────────────────────────────┐
            │  viewport · slider panels · material editor · content browser · export dialog     │
            └──────────────▲───────────────────────────────▲───────────────────────▲────────────┘
                           │ evaluated meshes / skeleton   │ textures              │ commands
┌──────────────┐  ┌────────┴──────┐  ┌──────────────┐  ┌───┴──────────┐  ┌─────────┴────────┐
│ oxy-morph    │→ │ oxy-mesh      │→ │ oxy-rig      │  │ oxy-material │  │ oxy-project      │
│ sliders →    │  │ mesh, sparse  │  │ skeleton,    │  │ layer stack, │  │ save/load, undo, │
│ target wts   │  │ morph blend   │  │ joint refit, │  │ GPU compose  │  │ versioning       │
└──────────────┘  └───────┬───────┘  │ skinning     │  └──────────────┘  └──────────────────┘
                          │          └──────┬───────┘
                  ┌───────┴──────┐   ┌──────┴───────┐   ┌──────────────┐
                  │ oxy-fit      │   │ oxy-expr     │   │ oxy-io       │
                  │ clothing/hair│   │ blendshapes, │   │ glTF/FBX/OBJ │
                  │ conforming   │   │ correctives  │   │ import/export│
                  └──────────────┘   └──────────────┘   └──────────────┘
```

**Key principle:** all character logic lives in **engine-agnostic crates**
(depending only on `glam`, `serde`, etc.). The GUI is a thin shell. This keeps
the core testable headlessly, usable from a CLI (`oxy-cli` batch export), and
portable to another renderer later.

### Technology choices

| Concern | Choice | Why |
|---|---|---|
| Math | `glam` | Fast, SIMD, standard in the Rust gamedev ecosystem |
| App/render | **Bevy** (+ `bevy_egui`) | PBR, skinning, morph targets, glTF loading and ECS out of the box; we get a viewport in days instead of months. Raw `wgpu` remains an option behind the same core crates. |
| UI | `egui` | Immediate mode suits hundreds of sliders; easy to make dockable panels (`egui_dock`) |
| glTF | `gltf` (read) + own writer / `gltf-json` | Mature, spec-complete |
| FBX | `ufbx` bindings (read), own minimal ASCII/binary writer later | ufbx is MIT and robust |
| Spatial queries | `bvh` / own BVH, `kiddo` (k-d tree) | Closest-point queries for fitting |
| Linear algebra | `nalgebra` / `faer` | RBF solves, least-squares joint fitting |
| Serialization | `serde` + `ron` (human-readable) + `bincode`/raw blobs | Diff-friendly projects |
| Image | `image`, GPU compositing via wgpu compute | Texture layer stacks |
| Parallelism | `rayon` | Morph evaluation, fitting over many vertices |

---

## 3. Core algorithms (our own designs)

### 3.1 Sparse morph blending (`oxy-mesh`)
- Each morph target stores only the vertices it moves: `(index: u32, delta: Vec3)`, plus optional normal deltas.
- Evaluation: `P = base + Σ wᵢ · Δᵢ` — accumulate into one scratch buffer; skip targets with `|w| < ε`.
- Upload either the final positions (CPU path, simple) or the active targets as GPU morph targets (fast interactive path for the slider being dragged).
- Normals recomputed from the topology after blending (area-weighted), or blended from normal deltas for speed.

### 3.2 Slider → target mapping (`oxy-morph`)
- **Bipolar slider** `[-1, 1]` drives a "neg" target and a "pos" target.
- **Macro sliders** (gender, age, weight, muscle, proportions) form an N-D grid; the active corner targets get **multilinear interpolation weights**, so e.g. *old + heavy + female* is a blend of authored corner shapes.
- **Dependent targets**: a target can be scaled by macro weights (e.g. a nose shape authored separately for male/female bases).
- Symmetry: L/R sliders can be linked; mirrored targets are generated via a precomputed vertex symmetry map.

### 3.3 Skeleton refit (`oxy-rig`)
- Each joint has an **anchor**: a set of vertex indices + weights on the base mesh (e.g. a ring of verts around the knee). After morphing, joint position = weighted centroid of its anchor verts. Robust, cheap, and independent of any vendor approach.
- Joint orientations recomputed from child positions with a fixed roll reference (keeps the rig "game-standard").
- Bind pose recomputed → inverse bind matrices regenerated for export.
- Skin weights stay valid because topology never changes.

### 3.4 Clothing / hair conforming (`oxy-fit`)
1. **Bind** (once per asset, at authoring time): for every garment vertex, find the closest point on the *base* body (BVH), store `(triangle, barycentrics, signed offset along normal, local tangent frame)`.
2. **Fit** (every body change): rebuild each garment vertex from the *morphed* body triangle frame + stored offset.
3. **Smooth & collide**: a few Laplacian smoothing iterations plus push-out against the body to fix interpenetration; per-asset "tightness" parameter.
4. **Alternative for loose items** (skirts, coats): RBF deformation driven by a sparse set of body control points, avoiding garment "shrink-wrap".
5. **Skin weights** transferred from the body via the same bind data, then smoothed.
6. **Hide-under masks**: body polygons fully covered by a garment get hidden to prevent poke-through.

### 3.5 Expressions & correctives (`oxy-expr`)
- Expression shapes authored on the base face as sparse deltas.
- After heavy face morphing, expression deltas are **transported** by re-expressing them in local per-vertex tangent frames (deformation transfer-lite), so a smile still looks right on a wide face.
- Export as glTF morph targets with stable ARKit-compatible names.

### 3.6 Layered skin materials (`oxy-material`)
- Material = stack of layers (`image | color | procedural noise`) × blend mode × mask × opacity, per channel (albedo, normal, roughness, SSS, AO).
- Composited by a wgpu compute shader into final textures at edit time; baked to plain PBR textures on export.

---

## 4. Assets strategy (the real long pole)

Software is half the problem; a **good base mesh and morph library** is the other half.

1. **Bootstrap** with CC0 data: the MakeHuman base mesh and targets are CC0 (verify the specific release before importing; record in `assets/LICENSES.md`). This gets a working human on day one.
2. **Replace progressively** with an original base mesh designed for games (cleaner edge loops around the face/joints, fewer verts, separated eyes/teeth), with a re-projection tool (`oxy-fit` reused!) that ports existing targets onto the new topology.
3. Morph-authoring pipeline: artists sculpt in Blender → export OBJ/glTF with identical topology → `oxy-cli bake-target` computes sparse deltas.
4. Clothing starter kit: a handful of CC0/self-made garments to exercise fitting.

---

## 5. Implementation path (milestones)

Each milestone ends with something demoable and tested.

### M0 — Foundations (week 1) ✅ *started in this repo*
- Cargo workspace, CI (fmt, clippy, test), license, contribution & clean-room policy.
- `oxy-mesh`: mesh type, sparse morph targets, blending, normal recompute.
- `oxy-morph`: unipolar/bipolar/macro slider definitions → target weights.
- `oxy-rig`: skeleton with vertex-anchor joint refit.

### M1 — See a human (weeks 2–3) ✅ *implemented*
- `oxy-io`: OBJ + sparse-target import of the CC0 base mesh & targets, `.oxyb` binary bundle, static glTF (`.glb`) export.
- `oxy-cli` (`oxy`): `import-makehuman`, `info`, `export`.
- `oxy-app`: Bevy window, orbit camera, three-point lighting, egui editor with live sliders driving `oxy-morph` → `oxy-mesh`.
- Asset bootstrap with CC0 base mesh via `tools/fetch-assets.sh`.
- Known M1 limits: body mesh only (no eyes/teeth/eyelashes yet), no UVs/textures, static export.

### M2 — Rigged & exportable (weeks 4–6)
- Joint anchors authored for the base mesh; refit on every slider change.
- Skin weights, skinning in the viewport, a T/A-pose toggle and a few test poses.
- glTF export with skeleton, inverse bind matrices, skin weights, and morph targets.
- Validate exports in Blender, Godot, Unity, and Unreal (glTF importer).

### M3 — Project files & UX (weeks 7–8)
- `oxy-project`: save/load parameters (RON), asset references by stable IDs, schema versioning + migrations.
- Undo/redo command stack; slider search; categories (face → nose → bridge…); symmetry toggle; presets/randomize.

### M4 — Materials (weeks 9–11)
- `oxy-material`: layer stack + GPU compositing; skin tone, makeup, tattoo decals with UV projection; eye color/iris; export baking.

### M5 — Clothing & hair (weeks 12–16)
- `oxy-fit`: bind/fit/smooth/collide; skin-weight transfer; hide-under masks.
- Content browser with thumbnails; asset package format (our own: a folder/zip with `manifest.ron` + glTF).
- Hair as card meshes with an anisotropic shader.

### M6 — Face & expressions (weeks 17–20)
- `oxy-expr`: expression set, deformation transport, expression preview sliders, viseme set.
- Optional facial bones; eye/jaw rig.

### M7 — Production export (weeks 21–24)
- LODs (`meshopt` simplification), texture atlasing, engine presets, FBX export, batch `oxy-cli export`.
- Performance pass: GPU morph path for dragged sliders, rayon everywhere else; target < 16 ms per slider update.

### M8+ — Stretch
- Photo-to-head fitting (landmark detection → solve morph weights by least squares).
- Cloth simulation preview (XPBD).
- Scripting/plugin API (Rhai or WASM).
- USD export.

---

## 5a. Editor UI layout

A conventional three-panel 3D-editor layout (shared by most DCC tools), with
OxyCharacter's own styling (dark theme, blue accent, text-and-symbol buttons):

```
┌ Menu: File · Edit · View ───────────────────────────────────────────────────┐
├ Toolbar: Reset · Random · Body/Head framing · Symmetry · Export   Morph|Material|Pose ┤
├──────────────┬──────────────────────────────────────┬───────────────────────┤
│ Content      │ Viewport                             │ Morph                 │
│ [Presets|Skin]│  stats overlay (verts, tris, morphs) │  search               │
│  filter chips│                                      │  category tree        │
│  preset cards│        3D character, orbit camera    │   (modified counts)   │
│  (procedural │   hovered slider's region is tinted  │  sliders: label,      │
│  silhouettes)│                                      │   value, reset, ends  │
│              │                                      │  [modified only][sym] │
├──────────────┴──────────────────────────────────────┴───────────────────────┤
│ Status bar: last action · base-mesh license                                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

Behaviours: hovering or dragging a slider tints the affected region; symmetric
editing links `l-`/`r-` sliders; double-click a slider to reset it; "Show
modified only" filters the list; search covers labels, ids and categories.

## 6. Repository layout (target)

```
OxyCharacter/
├── Cargo.toml              # workspace
├── crates/
│   ├── oxy-mesh/           # mesh + sparse morph blending          (M0)
│   ├── oxy-morph/          # slider model                          (M0)
│   ├── oxy-rig/            # skeleton, joint refit, skinning       (M0/M2)
│   ├── oxy-io/             # OBJ/glTF/FBX                          (M1)
│   ├── oxy-project/        # save format, undo                     (M3)
│   ├── oxy-material/       # layered materials                     (M4)
│   ├── oxy-fit/            # clothing/hair conforming              (M5)
│   ├── oxy-expr/           # expressions                           (M6)
│   ├── oxy-cli/            # headless batch tools                  (M1+)
│   └── oxy-app/            # Bevy + egui editor                    (M1)
├── assets/                 # licensed content + LICENSES.md
└── docs/
    ├── PLAN.md
    ├── specs/              # behavioral specs (clean-room)
    └── adr/                # architecture decision records
```

---

## 7. Testing strategy
- **Unit tests** for every algorithm on tiny synthetic meshes (cubes, strips) — no assets needed.
- **Golden tests**: evaluate a known parameter set on the real base mesh and compare vertex hashes / max deviation.
- **Round-trip tests**: export glTF → re-import → compare.
- **Validation**: run the official `gltf-validator` in CI on exported files.
- **Benchmarks** (`criterion`): morph evaluation, refit, and fitting at full resolution.

## 8. Top risks
| Risk | Mitigation |
|---|---|
| Base mesh / morph quality determines perceived quality | Bootstrap with CC0, budget real art time, build the target-baking pipeline early |
| Clothing fitting artifacts | Start with tight-fitting items; add RBF + collision iteratively; per-asset tuning parameters |
| Bevy API churn | Keep Bevy confined to `oxy-app`; pin the version |
| Scope creep | Ship M2 (rigged glTF export) as the first usable release before anything else |
| IP contamination | Clean-room rules in §0, asset license ledger, ADRs |
