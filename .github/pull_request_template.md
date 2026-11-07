## What and why

<!-- One or two paragraphs: the change and the reason for it. Link issues/ADRs. -->

## How it was verified

<!-- Tick what you actually ran; say "not run" otherwise. See docs/TESTING.md. -->
- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
- [ ] Real-data tests (`OXY_MAKEHUMAN_DATA=… cargo test --workspace --release`)
- [ ] Headless screenshot(s) looked at (attach for UI changes)
- [ ] Exported .glb checked with `tools/validate-gltf` (validator / three.js)
- [ ] OxyBrush round trip
- Platforms / GPUs / engines actually tried:

## Docs and data

- [ ] docs/ROADMAP.md ("Last updated") and CHANGELOG.md (*Unreleased*) updated
- [ ] README / PLAN / ADR updated if behaviour or design changed
- [ ] Any new asset listed in assets/LICENSES.md; any external source logged in docs/CLEAN_ROOM.md
- [ ] Format changes (`.oxyb`, `.oxyc`) are versioned and migrated

## Known limitations
