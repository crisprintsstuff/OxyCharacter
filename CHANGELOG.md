# Changelog

All notable changes to OxyCharacter. Dates are when the change landed on the
development branch. Publishing a version is a git tag; see
[CONTRIBUTING.md](CONTRIBUTING.md#cutting-a-release).

## [Unreleased]

### Added
- **Script plugins.** A `.rhai` file is a plugin. It can read and write sliders,
  pose, built-in clothes, skin and eyes, and register named actions with
  `action`. The editor lists those actions in the Scripts menu (from
  `plugins/` under the user folder, or `OXY_PLUGINS`). `oxy script` runs a
  file headlessly. Scripts have no file or network access. The host functions
  are listed in [docs/scripting.md](docs/scripting.md). See
  [ADR 0011](docs/adr/0011-script-plugins.md).

## [0.0.1-alpha] — 2026-11-09

### Added
- **Release packages.** Pushing a tag `v*` builds the editor and the CLI for
  Linux x86_64, macOS (universal) and Windows x86_64 and publishes a GitHub
  Release. The CC0 character bundle is built in that workflow and shipped in
  the package; the app does not download it. Windows gets an installer and a
  portable zip, macOS a disk image, Linux a tar archive and an AppImage.
  Checksums are `SHA256SUMS.txt`. Tags containing `-alpha`, `-beta` or `-rc`
  are pre-releases. See [the notes](docs/release-notes/v0.0.1-alpha.md) and
  [ADR 0010](docs/adr/0010-release-packages.md).
- **Clothing and hair (M5):** `oxy-fit` binds garments to the body and refits
  them when it changes (surface fit, loose radial-basis fit, push-out,
  tightness); smoothed skin-weight transfer; hide-under masks, including a
  higher layer hiding a lower one. Wardrobe shelf (rail icon and chip) with
  original T-shirt, trousers, skirt and hair cards (procedural strand
  texture). Wardrobe packages (`manifest.json` + OBJ). OxyBrush pieces that
  sit on the body are fitted. Projects store items, fit settings and hidden
  triangles; glTF export includes them skinned, with hair alpha and
  `KHR_materials_anisotropy`. `oxy wardrobe`, `oxy export --wear` /
  `--wardrobe`. See [ADR 0009](docs/adr/0009-clothing-and-hair.md).
- **Editor chrome:** the window follows the Oxy family layout (dark panels, blue
  accent, rounded cards) from `crates/oxy-app/src/theme.rs`, so it can stay in
  step with OxyBrush. Morph, Material, Pose and Export are a centered mode
  switch; presets, skin tones, wardrobe and accessories sit on a bottom shelf;
  morph categories and sliders stack on the right; mesh stats and the OxyBrush
  link sit in the status bar. The typed-path file dialog uses the same theme.
  The UI face is Roboto (Apache-2.0); icons are original geometry. See
  [ADR 0008](docs/adr/0008-oxy-family-theme.md).
- **Native file dialogs.** Open, Save as, glTF export and Import .oxb use the
  system file chooser (the `rfd` crate, polled asynchronously so the editor
  keeps drawing) with filters for `.oxyc`, `.oxb` and `.glb` and a sensible
  starting folder. Recent projects stay in the File menu. Linux without an
  XDG file chooser, and headless runs, keep the typed-path dialog (recent
  files included). `OXY_FILE_DIALOG=native|fallback` overrides the probe;
  `--dialog open|save|export|import` always opens the typed dialog for
  screenshots. See [ADR 0007](docs/adr/0007-native-file-dialogs.md).
- **Engine checks for glTF.** `tools/check-gltf-engines` imports an export in
  Blender and, when it is installed, Godot: skeleton, skin weights, bind-pose
  error, a posed arm, shape keys, textures and a skinned accessory.
  `oxy export --accessory file.obj` adds an OBJ mesh to that export.
- **Materials (M4):** UV render mesh (bundle v3); `oxy-material` crate with a
  procedural layer stack baked to albedo/normal/roughness textures; skin
  controls (tone, variation, detail, roughness, freckles, blush, eye shadow,
  lips) with face regions from bone weights; generated eyes with procedural
  irises; Material tab; textured glTF export with tangents and embedded PNGs
  (`--texture-size`, `--no-textures`, `--no-eyes`); material settings in
  projects and undo.
- Smaller dev builds: dependencies without debug info.
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

### Changed
- The editor's Export button asks for a `.glb` path (default
  `exports/<project>.glb`) instead of writing `exports/character-<time>.glb`
  with no prompt.

### Known limitations
See [docs/ROADMAP.md › Known gaps](docs/ROADMAP.md#known-gaps-cross-cutting).
