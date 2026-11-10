# ADR 0012: Facial expressions as transported units, exported as sparse blendshapes

**Status:** accepted (2026-11-10)

## Context

M6 adds facial expressions. They must work on any face the sliders make,
stay editable after the face is reshaped, export so engines can animate
them, and fit the clean-room rules ([CLEAN_ROOM.md](../CLEAN_ROOM.md)).
The CC0 MakeHuman data ships 34 *expression units* (one facial movement
each: jaw open, brow raise, lip pucker, …), three times over, one per
ancestry, all authored on the neutral base mesh.

## Decision

- **Units are the primitive.** The importer averages the three ancestry
  files of each unit into one sparse target `expr/<unit>` in the mesh. They
  are not catalog sliders; [`oxy_io::expression`] owns their ids, labels and
  groups. Ids are the CC0 file names and are stable: projects, the plugin API
  and exported morph names use them. Bundles without the targets simply have
  no expressions; the bundle format did not change (`VERSION` stays 3).
- **Presets and visemes are our own mixes** of unit weights (Smile, Sad,
  Surprise, …; A, E, I, O, U, M B P, F V, L, S Z, CH SH, TH). They are data in
  `expression.rs`, not separate targets.
- **Transport onto the shaped face.** A unit's deltas are authored on the
  base shape. For each vertex they touch we fit the rotation and uniform
  scale that best map its 3-ring neighbourhood (plus its normal) from the
  base shape to the current one (orthogonal Procrustes; rotation from the
  polar decomposition by Higham's iteration) and turn and scale the delta by
  it (`oxy_mesh::transport::Deformation`). An earlier per-vertex tangent-frame
  version changed deltas by a median 22 % on the plain default face; the
  neighbourhood fit is smoother (13 %, mostly genuine 6° turns of the
  surface). The editor caches the result per shape.
- **Expressions move body vertices only.** Joints come from the neutral
  shape, so the skeleton does not shift when the face moves, which is how an
  engine plays the exported blendshapes. Gaze is separate: it turns the eye
  bones (`eye.L`, `eye.R`) in the viewport pose.
- **Document state.** `Expression { units, gaze }` is in `DocState`, undo
  (labels "Expression" / "Gaze") and the `.oxyc` project as an optional
  `expression` field (no version bump; neutral faces don't write it). Unit
  ids a base doesn't have are kept and reported by `reconcile`.
- **Export.** By default the body is neutral and carries named morph
  targets: `expr.<unit>` (default weight = the current value), `face.<preset>`
  and `viseme.<id>` (weight 0), all transported to the exported shape. With
  `expressions: false` (`--no-expressions`) the current expression is baked
  in instead. Morph targets are written as glTF **sparse accessors** when that
  is smaller (all face targets are), so 55 targets add about 1 MB, not ~10 MB.
  Gaze is not exported (the file has no animation).
- **Plugin API.** `expression.list/get/set` and `body.mesh {expression}` are
  additive; `API_VERSION` stays 1.

## Consequences

- No teeth, tongue or mouth interior yet: an open mouth shows an empty
  cavity. Brows and lashes are still missing (M6 follow-up).
- Visemes and presets are mixes, so their quality is bounded by the 34 units
  (no cheek puff, no tongue). Adding units means new targets with stable ids.
- Transport is a local similarity, not a full deformation transfer: strongly
  sheared regions (very different lip shapes) can still distort a unit.
- No expression animation, keyframes or lip sync; engines can drive the
  exported targets.
