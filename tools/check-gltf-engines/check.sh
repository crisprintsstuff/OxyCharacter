#!/usr/bin/env bash
# Export a few .glb variants and import them with Blender (required) and
# Godot (when a binary is on PATH or in $GODOT).
#
#   tools/check-gltf-engines/check.sh [out-dir]
#
# Needs the CC0 bundle (tools/fetch-assets.sh) and a release `oxy` binary.
# Blender is invoked as `blender` or $BLENDER. Godot 4 as `godot` or $GODOT.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="${1:-${OXY_ENGINE_OUT:-/tmp/oxy-gltf-engines}}"
BUNDLE="${OXY_BUNDLE:-$ROOT/assets/generated/makehuman.oxyb}"
OXY="${OXY_BIN:-$ROOT/target/release/oxy}"
BLENDER_BIN="${BLENDER:-blender}"
GODOT_BIN="${GODOT:-godot}"

if [[ ! -f "$BUNDLE" ]]; then
  echo "missing bundle $BUNDLE — run tools/fetch-assets.sh first" >&2
  exit 1
fi
if [[ ! -x "$OXY" ]]; then
  echo "building oxy…"
  cargo build --release -p oxy-cli --manifest-path "$ROOT/Cargo.toml"
fi

mkdir -p "$OUT/morphs"
cat >"$OUT/morphs/nose-bump.target" <<'EOF'
# oxycharacter-morph label=Nose bump
0 0 0.02 0.01
EOF
# A small triangle near the head. Nearest-vertex weights skin it to the skull.
cat >"$OUT/hat.obj" <<'EOF'
o hat
v 0.00 1.72 0.08
v 0.06 1.68 0.04
v -0.06 1.68 0.04
f 1 2 3
EOF

echo "exporting to $OUT"
"$OXY" export "$BUNDLE" "$OUT/static.glb" --no-skeleton --no-eyes --texture-size 128
"$OXY" export "$BUNDLE" "$OUT/rest.glb" --texture-size 128
"$OXY" export "$BUNDLE" "$OUT/tpose.glb" --tpose --no-textures --no-eyes
"$OXY" export "$BUNDLE" "$OUT/blend.glb" --blendshapes --morphs "$OUT/morphs" --no-textures --no-eyes
"$OXY" export "$BUNDLE" "$OUT/accessory.glb" --accessory "$OUT/hat.obj" --no-textures --no-eyes

run_blender() {
  local mode="$1"
  echo "== blender $mode =="
  if ! "$BLENDER_BIN" --background --python "$ROOT/tools/check-gltf-engines/blender_check.py" -- \
      "$OUT/$mode.glb" "$mode" >"$OUT/$mode.blender.log" 2>&1; then
    echo "Blender check failed for $mode" >&2
    grep -E 'OXY_GLTF_REPORT|Error|Traceback' "$OUT/$mode.blender.log" | tail -n 40 >&2 || true
    tail -n 30 "$OUT/$mode.blender.log" >&2
    exit 1
  fi
  grep OXY_GLTF_REPORT "$OUT/$mode.blender.log"
}

if ! command -v "$BLENDER_BIN" >/dev/null 2>&1; then
  echo "blender not found (set BLENDER=...)" >&2
  exit 1
fi
for mode in static rest tpose blend accessory; do
  run_blender "$mode"
done

run_godot() {
  local mode="$1"
  echo "== godot $mode =="
  # --quit-after keeps a failed script from sitting on the process.
  if ! "$GODOT_BIN" --headless --path "$ROOT/tools/check-gltf-engines/godot" \
      -s res://check.gd -- "$OUT/$mode.glb" "$mode" >"$OUT/$mode.godot.log" 2>&1; then
    echo "Godot check failed for $mode" >&2
    grep -E 'OXY_GLTF_REPORT|SCRIPT ERROR|Parse Error' "$OUT/$mode.godot.log" | tail -n 40 >&2 || true
    tail -n 40 "$OUT/$mode.godot.log" >&2
    exit 1
  fi
  if ! grep -q OXY_GLTF_REPORT "$OUT/$mode.godot.log"; then
    echo "Godot produced no report for $mode" >&2
    tail -n 40 "$OUT/$mode.godot.log" >&2
    exit 1
  fi
  grep OXY_GLTF_REPORT "$OUT/$mode.godot.log"
}

if command -v "$GODOT_BIN" >/dev/null 2>&1; then
  for mode in static rest tpose blend accessory; do
    run_godot "$mode"
  done
else
  echo "godot not found (set GODOT=...); Blender checks passed, Godot skipped" >&2
fi

echo "engine checks passed"
