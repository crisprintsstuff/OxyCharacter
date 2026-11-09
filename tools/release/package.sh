#!/usr/bin/env bash
# Pack a release of the editor and the CLI.
#
#   tools/release/package.sh <linux|macos|windows> <version> <bin-dir> <bundle-dir> <out-dir>
#
# version     tag-shaped, e.g. v0.0.1-alpha (the leading v is kept in file names)
# bin-dir     oxycharacter[.exe] and oxy[.exe]
# bundle-dir  makehuman.oxyb, BUNDLE-REVISION.txt, MakeHuman-LICENSE.ASSETS.md
# out-dir     receives the archives (created if needed)
#
# Linux: a .tar.gz with a portable marker, and an AppImage.
# macOS: a universal .app inside a .dmg (sign-macos.sh ad-hoc signs unless
#        Developer ID secrets are set).
# Windows: a portable .zip and an Inno Setup installer (no portable marker).
set -euo pipefail

PLATFORM="${1:?platform: linux, macos or windows}"
VERSION="${2:?version, e.g. v0.0.1-alpha}"
BIN_DIR="${3:?directory containing the binaries}"
BUNDLE_DIR="${4:?directory containing makehuman.oxyb}"
OUT_DIR="${5:?output directory}"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SHORT="${VERSION#v}"
mkdir -p "$OUT_DIR"

case "$PLATFORM" in
  windows) EXE=".exe" ;;
  linux | macos) EXE="" ;;
  *)
    echo "unknown platform: $PLATFORM" >&2
    exit 1
    ;;
esac

SRC_EDITOR="$BIN_DIR/oxycharacter$EXE"
SRC_CLI="$BIN_DIR/oxy$EXE"
for f in "$SRC_EDITOR" "$SRC_CLI" "$BUNDLE_DIR/makehuman.oxyb" "$BUNDLE_DIR/BUNDLE-REVISION.txt"; do
  if [[ ! -f "$f" ]]; then
    echo "missing $f" >&2
    exit 1
  fi
done

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cp "$SRC_EDITOR" "$WORK/oxycharacter$EXE"
cp "$SRC_CLI" "$WORK/oxy$EXE"
if [[ "$PLATFORM" != "windows" ]] && command -v strip >/dev/null 2>&1; then
  strip "$WORK/oxycharacter$EXE" "$WORK/oxy$EXE" || echo "strip failed; shipping unstripped binaries"
fi

copy_shared() {
  local dest="$1"
  mkdir -p "$dest"
  cp "$BUNDLE_DIR/makehuman.oxyb" "$dest/makehuman.oxyb"
  cp "$BUNDLE_DIR/BUNDLE-REVISION.txt" "$dest/BUNDLE-REVISION.txt"
  if [[ -f "$BUNDLE_DIR/MakeHuman-LICENSE.ASSETS.md" ]]; then
    cp "$BUNDLE_DIR/MakeHuman-LICENSE.ASSETS.md" "$dest/MakeHuman-LICENSE.ASSETS.md"
  fi
  cp "$ROOT/LICENSE-MIT" "$ROOT/LICENSE-APACHE" "$ROOT/assets/LICENSES.md" "$dest/"
  cp "$ROOT/tools/release/INSTALL.txt" "$dest/INSTALL.txt"
}

copy_flat() {
  local dest="$1"
  mkdir -p "$dest"
  cp "$WORK/oxycharacter$EXE" "$dest/oxycharacter$EXE"
  cp "$WORK/oxy$EXE" "$dest/oxy$EXE"
  if [[ "$PLATFORM" != "windows" ]]; then
    chmod +x "$dest/oxycharacter$EXE" "$dest/oxy$EXE"
  fi
  copy_shared "$dest"
}

package_linux() {
  local name stage
  name="OxyCharacter-${VERSION}-linux-x86_64"
  stage="$(mktemp -d)"
  copy_flat "$stage/$name"
  : > "$stage/$name/portable"
  tar -C "$stage" -czf "$OUT_DIR/${name}.tar.gz" "$name"
  rm -rf "$stage"
  package_appimage "$name"
}

package_appimage() {
  local name="$1"
  local tools appdir
  tools="$(mktemp -d)"
  appdir="$(mktemp -d)/AppDir"
  echo "Downloading linuxdeploy and appimagetool"
  curl -fsSL -o "$tools/linuxdeploy.AppImage" \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
  curl -fsSL -o "$tools/appimagetool.AppImage" \
    https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$tools/linuxdeploy.AppImage" "$tools/appimagetool.AppImage"
  (
    cd "$tools"
    sha256sum linuxdeploy.AppImage appimagetool.AppImage
  )
  export APPIMAGE_EXTRACT_AND_RUN=1
  export ARCH=x86_64
  export LDAI_NO_APPSTREAM=1
  export NO_APPSTREAM=1
  "$tools/linuxdeploy.AppImage" --appdir "$appdir" \
    --executable "$WORK/oxycharacter" \
    --desktop-file "$ROOT/tools/release/oxycharacter.desktop" \
    --icon-file "$ROOT/tools/release/oxycharacter.png"
  mkdir -p "$appdir/usr/bin" "$appdir/usr/share/oxycharacter"
  cp "$WORK/oxy" "$appdir/usr/bin/oxy"
  chmod +x "$appdir/usr/bin/oxy" "$appdir/usr/bin/oxycharacter"
  copy_shared "$appdir/usr/share/oxycharacter"
  cat > "$appdir/AppRun" << 'EOF'
#!/bin/sh
# Launch the editor, or the CLI when the first argument is "oxy":
#   OxyCharacter.AppImage oxy info makehuman.oxyb
set -e
if [ -n "${APPDIR:-}" ]; then
  HERE="$APPDIR"
else
  HERE="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
fi
export PATH="${HERE}/usr/bin:${PATH:-}"
lib_path="${HERE}/usr/lib"
if [ -d "${HERE}/usr/lib/x86_64-linux-gnu" ]; then
  lib_path="${lib_path}:${HERE}/usr/lib/x86_64-linux-gnu"
fi
if [ -n "${LD_LIBRARY_PATH:-}" ]; then
  export LD_LIBRARY_PATH="${lib_path}:${LD_LIBRARY_PATH}"
else
  export LD_LIBRARY_PATH="${lib_path}"
fi
if [ "${1:-}" = "oxy" ]; then
  shift
  exec "${HERE}/usr/bin/oxy" "$@"
fi
exec "${HERE}/usr/bin/oxycharacter" "$@"
EOF
  chmod +x "$appdir/AppRun"
  local out
  out="$OUT_DIR/${name}.AppImage"
  "$tools/appimagetool.AppImage" "$appdir" "$out"
  chmod +x "$out"
  rm -rf "$tools" "$(dirname "$appdir")"
}

package_macos() {
  local app dmg_src dmg
  app="$WORK/OxyCharacter.app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  cp "$WORK/oxycharacter" "$app/Contents/MacOS/oxycharacter"
  cp "$WORK/oxy" "$app/Contents/MacOS/oxy"
  chmod +x "$app/Contents/MacOS/oxycharacter" "$app/Contents/MacOS/oxy"
  copy_shared "$app/Contents/Resources"
  sed "s/VERSION_PLACEHOLDER/${SHORT}/g" "$ROOT/tools/release/Info.plist" > "$app/Contents/Info.plist"
  "$ROOT/tools/release/sign-macos.sh" app "$app"
  dmg_src="$(mktemp -d)"
  cp -R "$app" "$dmg_src/OxyCharacter.app"
  ln -s /Applications "$dmg_src/Applications"
  dmg="$OUT_DIR/OxyCharacter-${VERSION}-macos-universal.dmg"
  rm -f "$dmg"
  hdiutil create -volname "OxyCharacter" -srcfolder "$dmg_src" -ov -format UDZO "$dmg"
  rm -rf "$dmg_src"
  "$ROOT/tools/release/sign-macos.sh" notarize "$dmg"
}

package_windows() {
  local name stage stage_win out_win iss_win
  name="OxyCharacter-${VERSION}-windows-x86_64"
  stage="$(mktemp -d)"
  copy_flat "$stage/$name"
  : > "$stage/$name/portable"
  tar -a -c -f "$OUT_DIR/${name}.zip" -C "$stage" "$name"
  rm -rf "$stage"

  stage="$(mktemp -d)"
  copy_flat "$stage"
  stage_win="$(cygpath -w "$stage")"
  out_win="$(cygpath -w "$OUT_DIR")"
  iss_win="$(cygpath -w "$ROOT/tools/release/oxycharacter.iss")"
  OXY_ISS_VERSION="$SHORT" \
    OXY_ISS_SOURCE="$stage_win" \
    OXY_ISS_OUT="$out_win" \
    OXY_ISS_SCRIPT="$iss_win" \
    powershell.exe -NoProfile -File "$(cygpath -w "$ROOT/tools/release/innosetup.ps1")"
  rm -rf "$stage"
  local setup="$OUT_DIR/OxyCharacter-v${SHORT}-windows-x86_64-setup.exe"
  if [[ ! -f "$setup" ]]; then
    echo "installer was not written to $setup" >&2
    exit 1
  fi
}

case "$PLATFORM" in
  linux) package_linux ;;
  macos) package_macos ;;
  windows) package_windows ;;
esac

echo "Packages in $OUT_DIR:"
ls -lh "$OUT_DIR"
