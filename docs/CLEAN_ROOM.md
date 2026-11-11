# Clean-room policy and source log

OxyCharacter reproduces the *category* of functionality found in commercial
character creators, not anyone's implementation, assets or trade dress. This
file states the rules and logs every external source consulted, so the
independent origin of the work can be shown.

## Rules

| Allowed | Not allowed |
|---|---|
| Public marketing pages, docs and videos, to learn *what features exist* | Decompiling, disassembling, debugging or patching commercial binaries |
| Open standards (glTF 2.0, OBJ, FBX via open readers, USD, ARKit-style blendshape *names*) | Reverse-engineering proprietary formats (vendor avatar/content containers) |
| Original, CC0 or CC-BY assets, listed in `assets/LICENSES.md` | Shipping, converting or tracing vendor meshes, morphs, textures, content-store items |
| Generic UI conventions (panels, sliders, trees) with our own styling | Copying a product's icons, artwork, colours, names or distinctive layouts |
| Reading the source of permissively licensed projects (MIT/Apache/BSD) | Reading or copying the source of copyleft (GPL/AGPL) projects we reimplement |
| Published papers (cite them in code comments) | Leaked material of any kind |

Process:
1. Describe features in our own words in `docs/specs/` before implementing.
2. Record design decisions in `docs/adr/`.
3. Add every new external source to the log below (date, what, why, how used).
4. Interoperate only through open formats or our own sister project's documented format.

## Source log

| Date | Source | Licence / nature | Used for | How |
|---|---|---|---|---|
| 2026-11-07 | Public product page of a commercial character creator (URL given by the maintainer) | Marketing material | Feature list only (sliders, skin, clothing, rig, export) | Not fetched by the agent; features came from general knowledge of the product category |
| 2026-11-07 | A screenshot of a commercial character creator's UI, supplied by the maintainer | Proprietary UI | General layout: content left, viewport centre, morph tree + sliders right, toolbar top | Only the generic three-panel arrangement was taken; colours, icons, names, widgets are our own ([ADR 0003](adr/0003-editor-ui-layout.md)). The screenshot is not in the repo |
| 2026-11-07 | MakeHuman `data/3dobjs`, `data/targets`, `data/rigs` (github.com/makehumancommunity/makehuman) | CC0-1.0 (data); program code is AGPL and was **not** downloaded or read | Base mesh, morph targets, skeleton, skin weights | Sparse checkout of data folders only; file formats understood by inspecting the data ([ADR 0002](adr/0002-makehuman-cc0-bootstrap.md)) |
| 2026-11-07 | OxyBrush source (github.com/crisprintsstuff/OxyBrush) | MIT OR Apache-2.0, sister project | `.oxb` format spec, MCP protocol for tests | Format reimplemented independently in `oxy_io::oxb`; no code copied ([ADR 0004](adr/0004-oxybrush-bridge.md)) |
| 2026-11-07 | Khronos glTF 2.0 specification; `gltf-validator` (npm) | Open standard; Apache-2.0 tool | Export format and validation | Spec followed; validator used as a test tool only |
| 2026-11-07 | three.js r170 (npm) | MIT | Independent check of exported skins | Test tool only, not shipped |
| 2026-11-07 | Bevy 0.20, bevy_egui 0.43, egui 0.36 sources and bundled examples | MIT/Apache-2.0 | API usage | Read to learn APIs |
| 2026-11-07 | MakeHuman `data/skins`, `data/eyes`, `data/litspheres` listings (same repo, CC0) | CC0 data | Checked for usable skin/eye assets | Only material settings, no skin textures; nothing imported. Skin and eyes are our own procedural designs ([ADR 0006](adr/0006-procedural-materials.md)) |
| 2026-11-09 | rfd 0.18 (github.com/PolyMeilex/rfd) | MIT | Native open/save dialogs | Dependency, XDG portal backend only (no GTK). Typed-path dialog remains when the portal is absent ([ADR 0007](adr/0007-native-file-dialogs.md)) |
| 2026-11-09 | Blender 4.0.2 glTF importer | GPL-2.0-or-later program, used only as an external process | Check exported skins, shape keys, materials | `tools/check-gltf-engines/blender_check.py` drives `blender --background`. No Blender source copied or linked |
| 2026-11-09 | Godot 4.7.2 glTF importer | MIT engine, test tool only | Same checks, headless | `tools/check-gltf-engines/godot/check.gd` drives the official binary. No Godot source copied |
| 2026-11-09 | OxyBrush editor mockup supplied with the task (sister application; the image is not committed) | Original design for our sister project | Family chrome: dark panels, blue accent, mode switch, tool rail, shelf, stacked inspector, status bar | Layout and density only. OxyCharacter maps its own editors onto that chrome ([ADR 0008](adr/0008-oxy-family-theme.md)). Icons are original geometry in `theme.rs`. Type is Roboto (Apache-2.0). No commercial UI was consulted for this restyle |
| 2026-11-09 | Roboto v2.138 (github.com/googlefonts/roboto) | Apache-2.0 | UI typeface | The Regular face is bundled at `assets/fonts/Roboto-Regular.ttf`; the upstream license text is `assets/fonts/LICENSE-ROBOTO`. Not used as a source of icons or layout |
| 2026-11-09 | Christer Ericson, *Real-Time Collision Detection* (2005), §5.1.5 | Published book | Closest point on a triangle | Algorithm reimplemented from the standard construction; the book was not fetched or copied |
| 2026-11-09 | Khronos `KHR_materials_anisotropy` (glTF 2.0 extension) | Open standard | Hair-card highlight direction in exports | Extension declared as used, not required; strand texture and cards are original ([ADR 0009](adr/0009-clothing-and-hair.md)) |
| 2026-11-09 | MakeHuman `LICENSE.md` and `LICENSE.ASSETS.md` at `a8bc2d54ff0ac92e78ff71431b1023eda42bf482` | CC0 for assets; the program licence is AGPL and the program was not downloaded | Whether the derived `.oxyb` may be redistributed in release packages | Sparse checkout of the two licence files only. The package ships the CC0 deed (`LICENSE.ASSETS.md`), not the AGPL program licence ([ADR 0010](adr/0010-release-packages.md)) |
| 2026-11-09 | GitHub Actions docs, "Manually running a workflow" and "Events that trigger workflows" (`workflow_dispatch`, tag pushes) | Public documentation | When the release workflow runs and how a manual dry-run is triggered | Read the published event description. No GitHub source copied |
| 2026-11-09 | GitHub changelog, "macOS 14 runner image retirement" (retired 2026-11-02) and the hosted-runner table | Public documentation | Which `runs-on` label still provides an Apple Silicon Mac | `macos-14` is rejected (`macos-14-arm64-us-default` is not supported). The workflow uses `macos-15` and cross-compiles the Intel slice |
| 2026-11-09 | Inno Setup 6 script syntax (`[Setup]`, `[Files]`, `[Icons]`, `{localappdata}`, `{group}`) and Apple `codesign` / `notarytool` flags | Public tool interfaces | Windows installer (Start Menu, uninstaller) and optional macOS signing | Scripts call those tools. Inno Setup, codesign and notarytool are not vendored. No sample project was copied |
| 2026-11-09 | linuxdeploy and appimagetool (GitHub continuous releases; MIT) | MIT tools, downloaded at build time | Linux AppImage of the editor and CLI | Not vendored. The AppRun script and the desktop file are original |
| 2026-11-11 | Rhai 1.27 language (rhai.rs, MIT) | MIT embedded scripting language | Plugin syntax | Used as a dependency. The host functions are original; no commercial plugin API was read ([ADR 0011](adr/0011-script-plugins.md)) |

Add new rows at the bottom.
