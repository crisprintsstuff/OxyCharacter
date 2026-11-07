# Changelog

All notable changes to OxyCharacter. Dates are when the change landed on the
development branch. The project has not made a release yet.

## [Unreleased] — 0.1.0

### Added
- **Projects (M3):** `.oxyc` project files (changed sliders only, skin, pose,
  export options, embedded sculpted morphs and accessories, base-mesh
  fingerprint, migration hook); New/Open/Open recent/Save/Save as; undo/redo;
  unsaved-changes marker and prompts; autosave with crash recovery; shortcuts.
- **Rig and export (M2):** CC0 163-bone skeleton that refits to every morph;
  pose presets with skeleton overlay; skinned glTF with T-pose bind option,
  sculpts as blendshapes and skinned accessories; `oxy export --tpose
  --blendshapes --morphs --no-skeleton`.
- **OxyBrush bridge (M1.5):** `.oxb` read/write; Send to OxyBrush with live
  re-import (reshaped body → morph slider, new pieces → accessories); VMAP
  support (OxyBrush ≥ 2.5.8) with topology matching as fallback; drag-and-drop
  import; `oxy send-oxybrush` / `import-oxybrush`.
- **Editor (M1):** CC0 MakeHuman base body with 328 sliders, macro sliders,
  region highlight, symmetry, search, presets, randomize, skin swatches,
  static glTF export; `.oxyb` asset bundle; `oxy` CLI.
- **Foundations (M0):** workspace, CI, mesh/morph/rig cores, clean-room policy.
- Contributor docs: AGENTS.md, CONTRIBUTING.md, docs/ROADMAP.md,
  docs/TESTING.md, docs/CLEAN_ROOM.md; licence files; pinned toolchain.

### Known limitations
See [docs/ROADMAP.md › Known gaps](docs/ROADMAP.md#known-gaps-cross-cutting).
