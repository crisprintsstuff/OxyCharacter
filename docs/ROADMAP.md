# OxyCharacter roadmap

**Last updated:** 2026-11-09 (editor chrome) · **Version:** 0.1.0 (unreleased)

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
| M5 | Clothing & hair: fitting, weight transfer, hide-under | ⏳ next |
| M6 | Face & expressions | ⏳ |
| M7 | Production export: LODs, atlases, engine presets, FBX | ⏳ |
| M8+ | Stretch: photo-to-head, cloth preview, scripting, USD | ⏳ |

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

## Next: M5 — Clothing & hair ⏳

Goal: garments and hair that fit any body and follow poses.

1. **Fitting** (`oxy-fit`, new crate, no GPU): bind each garment vertex to
   the base body (closest triangle, barycentrics, normal offset, tangent
   frame); refit on every body change; Laplacian smoothing and push-out
   against the body; per-item tightness. RBF deformation for loose items.
2. **Weights:** transfer from the body with smoothing (beyond today's
   nearest-vertex), so sleeves and skirts bend well.
3. **Hide-under masks:** hide body triangles fully covered by a garment.
4. **Content:** an asset package format (`manifest` + glTF), a content
   browser tab, a few original/CC0 garments, and OxyBrush accessories as
   fittable items (bind at import).
5. **Hair:** card meshes with an anisotropic material (alpha-tested strands
   texture generated procedurally).
6. **Projects:** items, fit settings and hidden-triangle masks in
   `DocState`/`Project`, export as skinned meshes.

Open questions ❔: package format details; whether to support layered
garments (shirt under jacket) in M5 or later.

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
Photo-to-head fitting, cloth simulation preview, scripting/plugins, USD.

## Known gaps (cross-cutting)

- **Engine imports not tried:** Blender, Godot, Unity, Unreal (M2 exports
  were validated with the Khronos validator and three.js only).
- **Never run on a real GPU, Windows or macOS:** everything so far ran under
  Linux software rendering (lavapipe).
- **Native file dialogs:** paths are typed into a dialog with recent files.
- **No teeth, tongue, eyelashes or brows** yet (M6); eyes are simple spheres.
- **Accessories don't refit** to body changes (M5); weight transfer is
  nearest-vertex.
- **Skinning is linear blend**: elbows/shoulders pinch in extreme poses
  (corrective shapes or dual quaternions later).
- **Poses are presets** only; no interactive bone posing or animation clips.
- **Breast/cup and proportion targets** that depend on body type are not
  imported yet.
- **Packaging/releases:** none yet (no installers, no release workflow).

## How to update this file

When a milestone item lands, tick it here, update "Last updated", add a line
to [CHANGELOG.md](../CHANGELOG.md) under *Unreleased*, and mention what was
verified (and how) in the PR.
