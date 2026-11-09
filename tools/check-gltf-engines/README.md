# Blender and Godot checks

Imports an OxyCharacter `.glb` with each engine's glTF importer and checks
the result. Original scripts; the engines are test tools only (see
[docs/CLEAN_ROOM.md](../../docs/CLEAN_ROOM.md)).

```sh
tools/fetch-assets.sh          # once
cargo build --release -p oxy-cli
tools/check-gltf-engines/check.sh /tmp/oxy-gltf-engines
```

`blender` (or `$BLENDER`) is required. Godot 4 (`godot` or `$GODOT`) is used
when it is on `PATH` and skipped with a message otherwise.

The script writes five files and checks each:

| File | What it covers |
|---|---|
| `static.glb` | Unskinned body, embedded skin textures |
| `rest.glb` | Skeleton, skin weights, bind pose, arm and eye deformation, albedo/normal/roughness and iris |
| `tpose.glb` | T-pose bind: left wrist level with the shoulder, arm toward +X |
| `blend.glb` | One custom morph as a shape key (`Nose bump`) |
| `accessory.glb` | A triangle OBJ skinned by nearest-vertex weights |

Each check prints `OXY_GLTF_REPORT {json}` and exits non-zero on failure.
This does not replace `tools/validate-gltf` (Khronos validator and three.js).

Unity and Unreal are not covered.
