# ADR 0003: Editor layout

**Status:** accepted, styling superseded in part by [ADR 0008](0008-oxy-family-theme.md)

The editor uses a conventional DCC arrangement — a viewport in the center,
content within reach, a property editor for the morph tree and sliders, and a
status bar. That arrangement is generic to 3D tools. Styling, iconography,
colors, naming and widgets are our own. ADR 0008 replaces the original dark
egui skin and the left-hand content column with the Oxy family chrome (mode
switch, tool rail, bottom shelf, stacked inspector). The feature split in this
ADR still holds: presets, skin and accessories are the content browser; morphs,
materials, pose and export are the property editor.
