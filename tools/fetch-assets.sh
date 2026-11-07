#!/usr/bin/env bash
# Fetch the CC0 MakeHuman base mesh + morph targets and build OxyCharacter's
# binary bundle (assets/generated/makehuman.oxyb).
#
# Only the *data* directories are checked out (sparse checkout); MakeHuman's
# AGPL program code is never downloaded or used.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CACHE="${OXY_CACHE:-$ROOT/assets/cache/makehuman}"
OUT="$ROOT/assets/generated/makehuman.oxyb"
REPO="https://github.com/makehumancommunity/makehuman"
# Pin the data to a known revision for reproducible bundles.
REV="${MAKEHUMAN_REV:-master}"

if [ ! -d "$CACHE/.git" ]; then
  mkdir -p "$(dirname "$CACHE")"
  GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 --filter=blob:none --sparse --branch "$REV" "$REPO" "$CACHE"
fi
git -C "$CACHE" sparse-checkout set --no-cone \
  '/LICENSE.md' '/LICENSE.ASSETS.md' '/makehuman/data/3dobjs/' '/makehuman/data/targets/'
echo "MakeHuman data revision: $(git -C "$CACHE" rev-parse HEAD)"

mkdir -p "$(dirname "$OUT")"
cargo run --release -q -p oxy-cli -- import-makehuman "$CACHE/makehuman/data" "$OUT"
cargo run --release -q -p oxy-cli -- info "$OUT"
