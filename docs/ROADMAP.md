# OxyCharacter roadmap

**Last updated:** 2026-11-11 (script plugins) · **Version:** 0.0.1-alpha

Status of each milestone, what's left, and what comes next. Design and
algorithms behind them are in [PLAN.md](PLAN.md); decisions in [adr/](adr/).

Legend: ✅ done · 🚧 in progress · ⏳ planned · ❔ needs a decision

## Summary

| # | Milestone | Status |
|---|---|---|
| M0 | Foundations: workspace, CI, mesh/morph/rig cores | ✅ |
| M1 | See a human: CC0 base, bundle, editor, live sliders, static glTF | ✅ |
| M1.5 | OxyBrush bridge: send, live re-import as morphs / accessories | ✅ |
| M2 | Rigged & exportable: skeleton refit, poses, skinned glTF | ✅ |
| M3 | Projects, undo/redo, autosave & recovery | ✅ |
| M4 | Materials: UVs, procedural skin, eyes, texture export | ✅ |
| M5 | Clothing & hair: fitting, weight transfer, hide-under | ✅ |
| M6 | Face & expressions | ⏳ |
| M7 | Production export: LODs, atlases, engine presets, FBX | ⏳ |
| M8+ | Stretch: photo-to-head, cloth preview, USD (script host started) | ⏳ |

## Done

### M0 — Foundations ✅
Cargo workspace, CI (fmt, clippy, test), clean-room policy, `oxy-mesh`
(sparse morph blending), `oxy-morph` (sliders), `oxy-rig` (anchor refit).

### M1 — See a human ✅
- CC0 MakeHuman base mesh (body group) and 840 targets → 328 sliders with our
  own taxonomy and auto-paired bipolar sliders; macro grid (gender, age,
  muscle, weight, height).
- `.oxyb` binary bundle; `oxy` CLI (`import-makehuman`, `info`, `export`).
- Bevy + egui editor: morph categories and sliders, region highlight on hover,
  symmetry, search, presets, randomize, skin swatches. The window chrome was
  restyled on 2026-11-09 to the shared Oxy family layout (mode switch, tool
  rail, shelf, stacked inspector) without dropping those editors.

### M1.5 — OxyBrush bridge ✅
- Independent `.oxb` reader/writer; `VMAP` (OxyBrush ≥ 2.5.8) for exact
  vertex correspondence, topology matching as fallback.
- Send to OxyBrush (+ link file), file watch, import dialog: reshaped body →
  custom morph slider (subdivision level 0 when present), new pieces →
  accessories. Drag-and-drop import.

### M2 — Rigged & exportable ✅
- CC0 MakeHuman default skeleton (163 bones) and weights; joints on aux
  points so the skeleton refits exactly to every morph; sculpts derive aux
  motion.
- Pose presets (rest/A, T, relaxed, wave, stride) with CPU skinning and a
  skeleton overlay; accessories follow poses.
- Skinned glTF: joint hierarchy, IBMs, 4 weights/vertex, T-pose bind option,
  sculpts as blendshapes, skinned accessories.
- Verified: Khronos validator 0 errors / 0 warnings; three.js deforms correctly.
  Blender 4.0.2 and Godot 4.7.2 (headless, `tools/check-gltf-engines`) import
  static, rigged rest, T-pose, blendshape, textured and accessory files and
  deform them. Unity and Unreal have not been run.

### M3 — Projects & UX ✅
- `.oxyc` JSON projects: changed sliders only, skin, pose, export options,
  embedded sculpts and accessories, base fingerprint, migration hook.
- Undo/redo (one step per finished edit), unsaved marker and prompts, recent
  files, autosave every 60 s with crash recovery, shortcuts.

### M4 — Materials ✅
- UVs from the base mesh: a separate *render mesh* split at UV seams, mapped
  back to base vertices; morphs, skinning and projects stay on the welded mesh
  (bundle v3).
- `oxy-material`: UV rasterizer (texel → surface point, dilated), 3D value
  noise, layer stack (colour/noise sources, region/noise masks, blend modes)
  baked to albedo (sRGB), tangent-space normal and occlusion-roughness-metal.
- Procedural skin (tone, variation, detail, roughness, freckles, blush, eye
  shadow, lips) with face regions from face-bone skin weights; generated
  eyeballs on the eye bones with a procedural iris.
- Editor Material tab; textured viewport (own tangents matching the bake);
  material settings undoable and saved in projects (optional field).
- glTF: UVs, tangents, embedded PNG textures, eyes rigged to the eye bones;
  Khronos validator 0 errors / 0 warnings; renders correctly in three.js.
- Viewport bake 1024² ≈ 0.2 s on 4 cores, done when a slider is released.

Not yet: image layers (tattoos/decals, imported textures), GPU compositing,
teeth/tongue/eyelashes/brows, subsurface scattering, eye wetness/cornea
geometry, texture painting.

### M5 — Clothing & hair ✅

- `oxy-fit`: bind (closest triangle, barycentrics, tangent-frame offset),
  refit, Laplacian smoothing, push-out, per-item tightness, Gaussian RBF for
  loose items. Skin weights are a smoothed barycentric transfer.
- Hide-under: body triangles fully covered by a garment are not drawn or
  exported. A higher layer hides covered triangles of a lower one (shirt over
  trousers). Garments fit to the body, not to each other.
- Wardrobe shelf (rail icon and chip) with four original items generated from the body (T-shirt,
  trousers, skirt, hair cards). Hair uses a procedural alpha-tested strand
  texture; export adds `KHR_materials_anisotropy`.
- Wardrobe packages: a folder with `manifest.json` and OBJ meshes
  (`oxycharacter-wardrobe` v1). OxyBrush pieces that sit on the body are
  bound at import and refit; pieces held away stay rigid.
- Items, fit settings and hidden-triangle lists are in projects and undo.
  Export adds them as skinned meshes and drops hidden body triangles.
- `oxy wardrobe` lists built-ins; `oxy export --wear` / `--wardrobe` includes
  them.

Not yet: mesh thumbnails in the browser, glTF import of packages, zip
packages, collision between garment layers, cloth simulation, anisotropic
highlights in the viewport (the export carries the extension; Bevy's standard
material does not).

## Next: M6 — Face & expressions ⏳

## Later

### M6 — Face & expressions ⏳
Expression set with stable names, deformation transport onto morphed faces,
visemes, jaw/eye rig, expression sliders and export as blendshapes. Bring in
the CC0 expression targets not imported today.

### M7 — Production export ⏳
LODs, texture atlasing, engine presets (Unity/Unreal/Godot/Bevy), reduced
game skeleton option, FBX, `oxy export` batch; performance pass (GPU morph
path while dragging, rayon elsewhere; < 16 ms per slider update).

### M8+ — Stretch ⏳
Photo-to-head fitting, cloth simulation preview, USD. A first scripting host
is in place ([docs/scripting.md](scripting.md)): people add `.rhai` plugins
that call the documented host functions. Photo fitting, cloth and USD are
still open.

## Known gaps (cross-cutting)

- **Unity and Unreal imports not tried.** Blender 4.0.2 and Godot 4.7.2 were
  checked headless (see above). Nothing has been opened in Unity or Unreal.
- **Never run on a real GPU, Windows or macOS:** everything so far ran under
  Linux software rendering (lavapipe). The native file dialog was not shown
  on a desktop portal for the same reason; the typed-path fallback was.
- **Native file dialogs need a desktop session.** Headless runs and Linux
  without `org.freedesktop.portal.FileChooser` keep the typed-path dialog
  (recent projects included). `OXY_FILE_DIALOG=native` forces the system dialog.
- **No teeth, tongue, eyelashes or brows** yet (M6); eyes are simple spheres.
- **Accessories that sit away from the body don't refit** (they only skin).
  Fitted ones use smoothed weight transfer; rigid ones still use
  nearest-vertex. There is no collision between overlapping garments.
- **Skinning is linear blend**: elbows/shoulders pinch in extreme poses
  (corrective shapes or dual quaternions later).
- **Poses are presets** only; no interactive bone posing or animation clips.
- **Breast/cup and proportion targets** that depend on body type are not
  imported yet.
- **The v0.0.1-alpha tag is not pushed by the packaging change.** The
  workflow is `.github/workflows/release.yml`. A `workflow_dispatch` run
  builds the packages and does not publish; pushing `v0.0.1-alpha` publishes
  the pre-release. Installers were exercised on the GitHub runners (see the
  pull request for that run). The editor GUI was smoked under Linux lavapipe
  from the packaged layout. It was not opened on a Windows or macOS desktop,
  and the macOS build is ad-hoc signed until Apple secrets are configured.
- **Clothing is still rough.** The starter garments are generated shells.
  Edges, sleeve joins and layering can look crude. Garments do not collide
  with each other. Hair's anisotropic highlight is in the glTF only.

## How to update this file

When a milestone item lands, tick it here, update "Last updated", add a line
to [CHANGELOG.md](../CHANGELOG.md) under *Unreleased*, and mention what was
verified (and how) in the PR.
