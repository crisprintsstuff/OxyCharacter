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
SPARSE=(
  '/LICENSE.md' '/LICENSE.ASSETS.md'
  '/makehuman/data/3dobjs/' '/makehuman/data/targets/' '/makehuman/data/rigs/'
)

# A 40-character revision is a commit. `git clone --branch` only accepts a
# branch or tag, so a pinned release revision is fetched by SHA instead.
# Releases pin that SHA in tools/release/makehuman.rev.
if [[ "$REV" =~ ^[0-9a-fA-F]{40}$ ]] && [ -d "$CACHE/.git" ]; then
  current="$(git -C "$CACHE" rev-parse HEAD)"
  if [ "$current" != "$REV" ]; then
    echo "MakeHuman cache is $current, wanted $REV; refetching"
    rm -rf "$CACHE"
  fi
fi

if [ ! -d "$CACHE/.git" ]; then
  mkdir -p "$(dirname "$CACHE")"
  if [[ "$REV" =~ ^[0-9a-fA-F]{40}$ ]]; then
    git init "$CACHE"
    git -C "$CACHE" remote add origin "$REPO"
    git -C "$CACHE" sparse-checkout set --no-cone "${SPARSE[@]}"
    GIT_LFS_SKIP_SMUDGE=1 git -C "$CACHE" fetch --depth 1 origin "$REV"
    git -C "$CACHE" checkout --detach FETCH_HEAD
  else
    GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 --filter=blob:none --sparse --branch "$REV" "$REPO" "$CACHE"
  fi
fi
git -C "$CACHE" sparse-checkout set --no-cone "${SPARSE[@]}"
REV_NOW="$(git -C "$CACHE" rev-parse HEAD)"
echo "MakeHuman data revision: $REV_NOW"

mkdir -p "$(dirname "$OUT")"
printf '%s\n' "$REV_NOW" > "$(dirname "$OUT")/BUNDLE-REVISION.txt"
# CC0 deed for the data. LICENSE.md also describes the AGPL program, which
# these packages do not ship; the asset deed is the text that covers the bundle.
if [ -f "$CACHE/LICENSE.ASSETS.md" ]; then
  cp "$CACHE/LICENSE.ASSETS.md" "$(dirname "$OUT")/MakeHuman-LICENSE.ASSETS.md"
fi
cargo run --release -q -p oxy-cli -- import-makehuman "$CACHE/makehuman/data" "$OUT"
cargo run --release -q -p oxy-cli -- info "$OUT"
