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
| 2026-11-09 | Christer Ericson, *Real-Time Collision Detection* (2005), §5.1.5 | Published book | Closest point on a triangle | Algorithm reimplemented from the standard construction; the book was not fetched or copied |
| 2026-11-09 | Khronos `KHR_materials_anisotropy` (glTF 2.0 extension) | Open standard | Hair-card highlight direction in exports | Extension declared as used, not required; strand texture and cards are original ([ADR 0007](adr/0007-clothing-and-hair.md)) |

Add new rows at the bottom.
