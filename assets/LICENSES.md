# Asset license ledger

Every asset used by OxyCharacter must be listed here (see the clean-room policy
in docs/PLAN.md §0). Fetched assets are **not** committed; they are downloaded
by `tools/fetch-assets.sh` into `assets/cache/` and converted into
`assets/generated/` (both git-ignored). Release packages build that bundle in
CI from the commit in `tools/release/makehuman.rev` and ship the `.oxyb` file.
The editor and the CLI do not download it. See
[ADR 0010](../docs/adr/0010-release-packages.md).

| Asset | Source | License | Notes | Date added |
|---|---|---|---|---|
| MakeHuman hm08 base mesh (`data/3dobjs/base.obj`, `body` group only) | https://github.com/makehumancommunity/makehuman | CC0-1.0 (released Sept 2020, see upstream `LICENSE.ASSETS.md`) | Fetched via sparse checkout of data dirs only; AGPL program code is never downloaded. Releases pin `tools/release/makehuman.rev` (currently `a8bc2d54ff0ac92e78ff71431b1023eda42bf482`) and redistribute the derived `.oxyb` | 2026-11-07 |
| MakeHuman morph targets (`data/targets/**`) | same | CC0-1.0 | Expression units (`expression/units/*`) are imported since M6 as the mean of the three ancestry files; genital and body-type-dependent breast targets are not imported | 2026-11-07 |
| MakeHuman default skeleton + weights (`data/rigs/default.mhskel`, `default_weights.mhw`) | same | CC0-1.0 (stated in the files) | Joints, bone hierarchy, roll planes and skin weights; bone names kept | 2026-11-07 |
| `docs/images/*.png` | Rendered by OxyCharacter from the CC0 assets above, plus original procedural clothing and hair (OxyBrush bridge shots: sculpt and cap from scripted OxyBrush sessions, still the 2026-11-07 captures) | CC0-1.0 | Editor screenshots. Re-rendered 2026-11-09 under lavapipe for the family chrome and wardrobe | 2026-11-07 |
| Procedural wardrobe (T-shirt, trousers, skirt, hair cards, strand texture) | Generated at runtime by `oxy-fit` and `oxy-material` | MIT OR Apache-2.0, same as the code | Not stored as asset files; built from the current body | 2026-11-09 |
| Roboto Regular (`assets/fonts/Roboto-Regular.ttf`) | https://github.com/googlefonts/roboto release v2.138, face `Roboto-Regular.ttf` | Apache-2.0 | Proportional UI typeface, embedded in the editor. Upstream license text: `assets/fonts/LICENSE-ROBOTO` | 2026-11-09 |
| Editor icons (`crates/oxy-app/src/theme.rs`, `Icon`) | Original | MIT OR Apache-2.0 (this project) | Geometric marks drawn in code, including the wardrobe mark. No third-party icon font or SVG set | 2026-11-09 |
| App icon (`tools/release/oxycharacter.png`) | Original, drawn by `tools/release/make_icon.py` | MIT OR Apache-2.0 (this project) | Dark rounded square, blue disc, simple figure. Used by the AppImage. Not a third-party mark | 2026-11-09 |
| egui built-in fallback fonts | Shipped inside epaint 0.36 (not copied into this repo) | Hack: MIT. Ubuntu Light: Ubuntu Font License 1.0. Noto Emoji: SIL OFL 1.1 | Monospace digits and glyphs Roboto does not cover. Not redistributed by us | 2026-11-09 |
