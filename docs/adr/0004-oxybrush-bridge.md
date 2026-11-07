# ADR 0004: OxyBrush bridge via native `.oxb` files

**Status:** accepted

## Context

Users should be able to sculpt in OxyBrush (our sister sculpting app) and bring
the result straight into OxyCharacter, the way character creators commonly
round-trip with a sculpting package: a reshaped body becomes a morph, new
meshes become accessories.

## Decision

- **File format, not a library dependency.** OxyCharacter reads and writes
  OxyBrush's documented `.oxb` v1 format itself (`oxy_io::oxb`). Depending on
  OxyBrush's crates would pull in its engine and clash on crate names
  (`oxy-mesh`, `oxy-io` exist in both workspaces). The format is small,
  versioned and has skippable sections, so an independent reader is low-risk.
  Interoperability is verified against the real `oxybrush-cli` (see below).
- **Topology correspondence.** OxyBrush re-sorts vertices and triangles for
  locality when it loads a mesh (Morton order, see its `Mesh::from_data`) and
  saves in that order. We therefore match connectivity (`oxy_mesh::correspond`)
  instead of assuming identical indices: flood across consistently oriented
  shared edges from position-guided seeds; reject on any contradiction.
- **Reference body via a link file.** OxyBrush drops unknown META keys when it
  saves, so the parameters the body was sent with live in
  `<file>.oxb.oxylink.json`. Morph = sculpted − reference (in base vertex
  order), so later slider changes in OxyCharacter don't leak into it.
- **Subdivision levels.** If a piece has an `MRES` section, level 0 is the
  morph source. OxyBrush propagates higher-level sculpting into level 0 on
  save, so broad changes made at any level arrive; finer detail does not (yet).
- **Never overwrite user work.** Each send writes a new timestamped file; all
  writes are atomic. Imports go through a confirmation dialog.

## Verification (2026-11-07, oxybrush 2.5.6-beta)

- `oxybrush-cli convert` of a sent file → re-imported as "unchanged" (exact
  correspondence despite reordering).
- Scripted OxyBrush MCP sessions (Draw brush strokes in world space, X
  symmetry, OBJ import as a new piece, save) → body classified as a morph,
  the new piece as an accessory; in the editor, the watcher picks up the save
  and opens the import dialog.
- OxyBrush's own `Multires` API (add level, sculpt at level 1, save) → morph
  taken from level 0.
- OxyBrush's MCP `subdivide` is destructive (no levels): such a body is
  correctly reported as changed topology.

## Consequences

- If OxyBrush ever changes the meaning of existing `.oxb` sections it must
  bump the version (its own invariant 7); our reader refuses newer versions.
- An optional OxyBrush-side improvement — a "Send to OxyCharacter" action, or
  preserving original vertex order — would make the loop one click shorter but
  is not required.
