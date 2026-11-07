# OxyCharacter roadmap

**Last updated:** 2026-11-07 · **Version:** 0.1.0 (unreleased)

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
| M4 | Materials: UVs, layered skin, eyes, texture export | ⏳ next |
| M5 | Clothing & hair: fitting, weight transfer, hide-under | ⏳ |
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
- Bevy + egui editor: content panel, viewport, morph tree + sliders, region
  highlight on hover, symmetry, search, presets, randomize, skin swatches.

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

## Next: M4 — Materials ⏳

Goal: a textured, believable character in the editor and in exports.

1. **UVs and seams.** Import the base mesh's UVs. The render/export mesh needs
   split vertices at UV seams while morphs and skinning stay on the welded
   mesh: add a *render map* (render vertex → base vertex) applied after
   evaluation. Keep the base topology identity intact (invariant 2).
2. **Layer stack** (`oxy-material`, new crate, no GPU): layers of
   image / color / procedural noise with blend mode, mask, opacity, per
   channel (albedo, normal, roughness, AO, SSS strength). CPU reference
   compositor + tests; GPU compositor in `oxy-app` later.
3. **Skin & eyes.** Original or CC0 base skin textures (ledger!), tone control
   replacing today's flat color, eye geometry (MakeHuman has CC0 eye proxies
   as separate assets — check licence per asset) with iris color.
4. **Export.** Bake the stack to PNGs and reference them from glTF
   (`baseColorTexture`, `normalTexture`, `metallicRoughnessTexture`); keep
   exports validator-clean.
5. **Projects.** Material settings in `DocState`/`Project` (+ tests).

Open questions ❔: texture resolution defaults; whether to ship any textures in
the repo or fetch them like the mesh; SSS approximation in Bevy.

## Later

### M5 — Clothing & hair ⏳
Bind/fit/smooth/collide garments to any body (PLAN §3.4), RBF for loose
items, weight transfer better than nearest-vertex (smoothing, geodesic
falloff), hide-under masks, an asset package format (`manifest` + glTF),
hair cards with an anisotropic shader. Accessories from OxyBrush become
fittable items.

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
- **Body only:** no eyes, teeth, eyelashes or tongue yet (M4/M6).
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
