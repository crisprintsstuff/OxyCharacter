# AGENTS.md — instructions for AI coding agents

This file tells AI agents (Claude Code, Codex, Copilot, Cursor and others) how
to work on OxyCharacter. Humans should start with
[CONTRIBUTING.md](CONTRIBUTING.md); the two agree, and where they differ,
CONTRIBUTING.md and the documents it links win.

## Project in one paragraph

OxyCharacter is a clean-room, Rust-native **parametric 3D character creator**
(MIT OR Apache-2.0). You shape a base human with sliders, the skeleton refits
to the shape, you pose and dress it, sculpt it in the sister app
[OxyBrush](https://github.com/crisprintsstuff/OxyBrush), and export a rigged
glTF. Status and plan: [docs/ROADMAP.md](docs/ROADMAP.md). Design and
algorithms: [docs/PLAN.md](docs/PLAN.md). Decisions: [docs/adr/](docs/adr/).

## Repository layout

```
crates/
  oxy-mesh     mesh, sparse morph targets, aux points, topology correspondence   (no GPU)
  oxy-morph    sliders, macro grids, parameter catalog → target weights           (no GPU)
  oxy-rig      joints/bones, refit, bind orientation, posing, skinning, presets   (no GPU)
  oxy-io       OBJ/target readers, MakeHuman CC0 importer, .oxyb bundle,
               glTF export (skinned), .oxb read/write, OxyBrush bridge            (no GPU)
  oxy-material UV rasterizer, noise, layer stack, skin/eye bakes, eyeball mesh     (no GPU)
  oxy-plugin   plugin API v1 (stdio JSON-RPC), Host trait, runner, discovery     (no GPU)
  oxy-project  .oxyc project format, migrations, snapshot undo history           (no GPU)
  oxy-cli      headless tools, binary `oxy`                                       (no GPU)
  oxy-app      Bevy 0.20 + egui editor, binary `oxycharacter`
assets/        LICENSES.md (ledger); cache/ and generated/ are fetched/built, git-ignored
docs/          PLAN.md, ROADMAP.md, TESTING.md, CLEAN_ROOM.md, adr/, specs/, images/
python/        oxycharacter plugin SDK (pure Python)
examples/      plugins/ (example plugins, shipped in packages)
tools/         fetch-assets.sh (CC0 data → bundle), validate-gltf/ (Khronos validator, three.js)
```

## Commands

The toolchain is pinned in `rust-toolchain.toml` (1.99.1; Bevy 0.20 needs
≥ 1.97.1). Run these before every commit; CI runs the first three:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# with the real CC0 data (after tools/fetch-assets.sh):
OXY_MAKEHUMAN_DATA=assets/cache/makehuman/makehuman/data cargo test --workspace --release
```

Run a chained check with `&&` or `set -e` so a failing step stops the commit.

Useful extras (details in [docs/TESTING.md](docs/TESTING.md)):

```bash
tools/fetch-assets.sh                                   # CC0 base mesh + targets + rig → assets/generated/makehuman.oxyb
cargo run --release -p oxy-app                          # the editor
cargo run --release -p oxy-app -- --pose wave --skeleton --pose-tab --screenshot out.png --frames 40
cargo run --release -p oxy-cli -- info assets/generated/makehuman.oxyb
cargo run --release -p oxy-cli -- export assets/generated/makehuman.oxyb out.glb --tpose
(cd tools/validate-gltf && npm install && node validate.mjs ../../out.glb)
```

**Look at a screenshot whenever you change rendering or UI**; tests alone
don't show visual regressions. A release build of the editor takes ~8 minutes
from scratch (Bevy); incremental builds are seconds.

## Architecture invariants (do not break)

1. **Engine-agnostic core.** Only `oxy-app` may depend on Bevy/egui. Everything
   else builds and tests headless.
2. **The base topology is an identity.** Vertex order and triangles of the base
   mesh index morph targets, skin weights, joint anchors, sculpted morphs and
   project files (whose `base.topology` fingerprint guards it). Changing the
   importer's vertex order, filtering or triangulation invalidates all of them:
   that needs an ADR, a bundle version bump and a project migration.
3. **Morphs are sparse and linear.** `P = base + Σ wᵢ Δᵢ`. Joint locators that
   aren't body vertices are *aux points* (`MorphableMesh::aux`) moved by the
   same targets; a new kind of morph must also provide aux deltas (sculpts use
   `Rig::derive_aux_deltas`), or the skeleton stops following the body.
4. **Document state is explicit.** Anything the user edits that should be
   saved or undone must be in `DocState` (`crates/oxy-app/src/document.rs`),
   `content_key`, `restore`, the `Project` format and their tests. State
   outside it is neither saved nor undoable.
5. **Formats are versioned.**
   - `.oxyb` bundle (`oxy_io::bundle`): bump `VERSION` on layout changes, keep
     reading the previous version; it is regenerated, not user data.
   - `.oxyc` project (`oxy-project`): **user data.** Add fields with
     `#[serde(default)]`; anything incompatible bumps `VERSION` and adds a step
     to `migrate`. Never silently drop data: `reconcile` reports what it skips.
   - `.oxb` is **OxyBrush's** format. Follow its spec in OxyBrush's
     `crates/oxy-doc/src/format.rs`; skip unknown sections; never require
     sections OxyBrush may omit (VMAP, MRES, VCOL).
6. **Never lose user data.** All writes are atomic (`oxy_io::oxb::write_atomic`);
   never overwrite a file the user gave us; "Send to OxyBrush" always writes a
   new file.
7. **Render mesh vs base mesh.** Drawing and export use `RenderMesh`
   (vertices split at UV seams, `map` → base vertex). Compute on the base mesh,
   then `gather`; never index morphs/weights by render vertex.
8. **Units and axes.** Metres, Y up, character faces +Z, its left is +X, feet
   at y = 0 in the editor and exports (MakeHuman's decimetres are scaled 0.1).
9. **Plugin API compatibility.** `oxy-plugin` methods are answered by one
   dispatcher for every host. New methods and optional fields are additive;
   changing or removing anything bumps `API_VERSION`. Keep
   [docs/PLUGINS.md](docs/PLUGINS.md) and the Python SDK in step with it.
10. **glTF output stays valid.** Exports must pass the Khronos validator with
   0 errors and 0 warnings (e.g. inverse bind matrices need an exact
   `0 0 0 1` bottom row).

## Hard rules

- **Clean room.** Read [docs/CLEAN_ROOM.md](docs/CLEAN_ROOM.md). In short:
  never look at, decompile or reverse-engineer commercial character-creator
  binaries, file formats or content; never copy their assets, names, icons or
  trade dress; describe features in our own words (`docs/specs/`). Log every
  external source you consult in CLEAN_ROOM.md.
- **Assets.** Only original, CC0 or CC-BY data, each listed in
  [assets/LICENSES.md](assets/LICENSES.md). **Never commit fetched or
  generated assets** (`assets/cache/`, `assets/generated/`). MakeHuman is
  used for its CC0 *data only*: `tools/fetch-assets.sh` sparse-checks out data
  folders; never download, read or copy MakeHuman's AGPL program code.
- **Licenses.** Dependencies and ported code must be MIT/Apache-2.0/BSD/zlib
  compatible. Never copy GPL/AGPL code. Studying behaviour and published
  papers is fine (cite the paper in a comment).
- **No `unsafe`** (forbidden workspace-wide).
- **No network access from the editor or CLI.** Only `tools/fetch-assets.sh`
  downloads (on request). The editor may start OxyBrush (`$OXYBRUSH_BIN` or
  `oxybrush`) only when the user clicks *Send to OxyBrush*, and a plugin's
  Python only when the user runs that plugin (menu, `--run-plugin`,
  `oxy plugin`). Plugins talk over the child's stdio; never add sockets or
  ports for them.
- **Trademarks.** Don't use other products' names for features, in code, UI or
  docs. Naming a product only to identify a file format or for interoperability
  is fine.

## Working with OxyBrush

OxyBrush is a separate repository with its own AGENTS.md. Interop is **file
format only**: OxyCharacter has its own `.oxb` reader/writer and does not
depend on OxyBrush's crates (crate names clash: both have `oxy-mesh`,
`oxy-io`). If a change needs something from OxyBrush (a new section, a
behaviour), describe it as a spec and open it there. The end-to-end recipe
(OxyBrush's headless MCP server sculpting a sent file) is in
[docs/TESTING.md](docs/TESTING.md#oxybrush-round-trip).

## Workflow

- **Branch and pull request.** Never push to `main` directly. Work on a feature
  branch, open a pull request (template: `.github/pull_request_template.md`),
  and let a maintainer merge it. Don't merge or approve your own PRs unless the
  maintainer explicitly asks.
- **Releases.** A tag matching `v*` publishes a GitHub Release
  (`.github/workflows/release.yml`). Cutting one means bumping the workspace
  version, moving CHANGELOG *Unreleased* to that version, and writing
  `docs/release-notes/<tag>.md` (see CONTRIBUTING.md). Do not create or push a
  tag, and do not publish a release, unless the maintainer explicitly asks.
- **Sign off** commits (`git commit -s`) and explain *why* in the body.
- **Keep docs current in the same change:**
  [docs/ROADMAP.md](docs/ROADMAP.md) (status, "Last updated"),
  [CHANGELOG.md](CHANGELOG.md) (Unreleased), the README for user-visible
  behaviour, [docs/PLAN.md](docs/PLAN.md) for design changes, and an ADR in
  `docs/adr/` for decisions that constrain future work.
- **Tests with every change:** unit tests in the crate you changed; editor
  tests in `crates/oxy-app/src/document.rs` / `bridge.rs` for interaction;
  a real-data test in `crates/oxy-io/tests/roundtrip.rs` when behaviour
  depends on the actual base mesh (gated on `OXY_MAKEHUMAN_DATA`).
- **Be honest in summaries and PR descriptions:** say what was verified and
  how (unit tests, real-data tests, headless screenshot, validator, three.js),
  what wasn't, and known limitations. Never claim something works on a real
  GPU, on Windows/macOS or in Blender/Unity/Unreal/Godot unless it was run there.
- If CI fails because a new Rust release added lints, fix the code; bump
  `rust-toolchain.toml` deliberately in a separate commit.

## Environment notes for cloud agents

- Headless runs: `Xvfb :99 &`, `DISPLAY=:99`,
  `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json` (package
  `mesa-vulkan-drivers`). Software rendering gives ~10 FPS; that's expected.
- The editor's scripted options (`--set`, `--select`, `--hover`, `--head`,
  `--pose`, `--skeleton`, `--pose-tab`, `--material-tab`, `--iris`, `--import-oxb [--apply-import]`,
  `--send-oxybrush`, `--run-plugin`, `--open`, `--save-as`, `--close`, `--screenshot`,
  `--frames`, `--wardrobe-tab`, `--wear tee,pants,hair`) cover most UI states without clicking. Set `OXY_USER_DIR` to a
  scratch folder so runs don't touch `user/`.
- `tools/fetch-assets.sh` needs git access to github.com; set `OXY_CACHE` to
  reuse an existing sparse checkout.
