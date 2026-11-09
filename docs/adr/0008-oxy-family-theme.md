# ADR 0008: Oxy family editor chrome

**Status:** accepted

## Context

OxyCharacter and OxyBrush are sister applications. Their first editor skins
were styled independently, so a file sent from one to the other looked like it
had changed products. The maintainer supplied an OxyBrush mockup (logged in
[CLEAN_ROOM.md](../CLEAN_ROOM.md)) as the shared target: near-black panels,
thin borders, a blue accent, a centered mode switch, a narrow tool rail, a
bottom shelf and a stacked inspector.

[ADR 0003](0003-editor-ui-layout.md) fixed the feature split (content browser,
viewport, property editor). It did not fix a common visual language, and the
left column plus right-hand tree no longer matches the family mockup.

## Decision

- The visual language lives in `crates/oxy-app/src/theme.rs` and depends only
  on egui: palette, spacing, corner radii, and the painters for the mode
  switch, tabs, icon buttons, search field, list rows and cards. No document
  types. OxyBrush can copy the module or it can move into a shared crate
  later; either way the tokens have one definition.
- `crates/oxy-app/src/shell.rs` places that chrome. Feature widgets stay in
  `ui.rs` (shelf pages, morph lists, material sections, pose and export
  controls, project and OxyBrush dialogs) so a change to clothing or file
  dialogs does not rewrite the bars.
- Existing editors map onto the mockup's slots rather than growing new ones:
  - mode switch: Morph, Material, Pose, Export
  - tool-options bar: the active mode's quick controls (mirror axis, material
    amounts, pose, bind pose)
  - rail: presets, skin, accessories, framing, skeleton, symmetry, reset,
    randomize
  - shelf: the same three content pages, as round thumbnail cards
  - right stack: category or pose list above, property sliders below
  - status bar: vertex and triangle counts, height, active morphs, FPS, the
    last action, and whether a file is linked to OxyBrush
- Mirror is the left/right axis only. The Y and Z chips are drawn so the
  control matches the family bar, and they are inert, with a tooltip that
  says so.
- Icons are geometry drawn in `theme.rs`. The proportional face is Roboto
  Regular (Apache-2.0), bundled under `assets/fonts/`. egui's built-in
  monospace stays for digits and as a glyph fallback.
- The 3D viewport uses the same dark key: a blue-gray clear color, an egui
  vignette, a floor disc and a grid. The corner gizmo is drawn in screen
  space from the orbit camera.

## Consequences

- A future OxyBrush theme change should edit these tokens (or the shared
  crate, once extracted), not ad-hoc colors in panel code.
- Adding a shelf page means a `LeftTab` variant plus an arm in `draw_shelf`,
  and a rail button. Adding a mode means a `RightMode` variant plus arms in
  the mode switch, the tool bar and the inspector stack.
- Screenshots in `docs/images/` show this chrome. The headless flags in
  [TESTING.md](../TESTING.md) are unchanged.
