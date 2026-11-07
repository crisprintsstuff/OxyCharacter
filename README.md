# OxyCharacter

A clean-room, Rust-native parametric 3D character creator: shape a base human
with sliders, auto-refit its skeleton, dress it, and export rigged glTF.

See **[docs/PLAN.md](docs/PLAN.md)** for scope, architecture, algorithms and milestones.

## Status — M0 (foundations)

| Crate | What it does |
|---|---|
| `oxy-mesh` | Mesh, sparse morph targets, blending, normals |
| `oxy-morph` | Unipolar / bipolar / macro-grid sliders → target weights |
| `oxy-rig` | Skeleton with vertex-anchor joint refitting |

```sh
cargo test --workspace
```
