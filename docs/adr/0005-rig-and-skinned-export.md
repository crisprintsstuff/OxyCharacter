# ADR 0005: Morph-following rig and skinned glTF export

**Status:** accepted

## Decision

- **Skeleton source.** Use the CC0 MakeHuman default skeleton and weights
  (`data/rigs/`), read as data only. Bone names are kept so tools that know
  them can map them; a reduced game skeleton can be derived later.
- **Joints are anchors.** Each joint is the average of listed points. Most of
  those points are helper vertices inside the body, not part of the visible
  mesh; we keep them as *aux points* of the morphable mesh, and import their
  per-target deltas, so the skeleton refits exactly to every morph without
  any solve. Sculpted morphs (from OxyBrush) carry no aux motion, so we derive
  it from the eight nearest body vertices (inverse-distance weights).
- **Bind orientation.** +Y along the bone (head → tail); roll from the bone's
  roll plane (three joints) when it is well conditioned, else a world axis.
  Our own convention; it matches common engine expectations closely enough
  that retargeters can map it.
- **Skinning.** Linear blend skinning, top four influences per vertex,
  renormalised (the source data has up to nine). CPU skinning in the editor
  (13k vertices; well under a millisecond per pose change).
- **Poses.** Presets are bone *aiming rules* (bone → world direction), solved
  parents-first. This keeps presets independent of bone roll and body shape.
- **Export.** glTF joint hierarchy with local TRS; inverse bind matrices are
  computed as affine inverses so the bottom row is exactly (0,0,0,1), which
  the validator requires. Optional T-pose bind: pose the mesh (and morph
  deltas, skinned the same way) into the T-pose and re-bind there. Sculpted
  morphs are baked by default, or exported as named blendshapes at their
  current weight (the base then excludes them). Accessories get the nearest
  body vertex's weights.

## Verification

- Unit tests: refit, aiming, skinning, weight transfer, pose solving, bundle
  round trip with rig.
- Real-data tests: rig validates; joints inside the body and at plausible
  heights; head joint rises with the height slider; identity pose reproduces
  the mesh; exported file re-read with the `gltf` crate has world bind × IBM =
  identity for every joint, normalised weights, horizontal arms in T-pose.
- Khronos glTF validator 2.x: 0 errors, 0 warnings (rest, T-pose,
  blendshapes). three.js r170 loads the files and deforms correctly when
  bones are rotated.

## Not yet verified

Blender, Godot, Unity and Unreal imports; engine humanoid retargeting.
