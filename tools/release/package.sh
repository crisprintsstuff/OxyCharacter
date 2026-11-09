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
  # linuxdeploy's AppRun is often a symlink to the editor. Writing the script
  # through that link replaces the binary, and the script then execs itself
  # until the kernel returns "Argument list too long" (exit 126).
  rm -f "$appdir/usr/bin/oxycharacter" "$appdir/AppRun"
  cp -a "$WORK/oxycharacter" "$appdir/usr/bin/oxycharacter"
  cp -a "$WORK/oxy" "$appdir/usr/bin/oxy"
  chmod +x "$appdir/usr/bin/oxycharacter" "$appdir/usr/bin/oxy"
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
  # winit and wgpu load these with dlopen. linuxdeploy only follows
  # DT_NEEDED, so without this the AppImage has no X11 or Vulkan loader.
  mkdir -p "$appdir/usr/lib"
  bundle_soname() {
    local soname="$1" path real base dep dep_path dep_real dep_base
    path="$(ldconfig -p 2>/dev/null | awk -v n="$soname" '$1==n { print $NF; exit }')"
    if [[ -z "$path" || ! -e "$path" ]]; then
      echo "warning: host library $soname was not found; the AppImage may not open a window"
      return 0
    fi
    real="$(readlink -f "$path")"
    base="$(basename "$real")"
    cp -a "$real" "$appdir/usr/lib/$base"
    if [[ "$base" != "$soname" ]]; then
      ln -sfn "$base" "$appdir/usr/lib/$soname"
    fi
    while read -r dep dep_path; do
      case "$dep" in
        linux-vdso.so* | ld-linux* | libc.so* | libm.so* | libdl.so* | libpthread.so* | librt.so* | libresolv.so* | libutil.so*)
          continue
          ;;
      esac
      [[ -n "${dep_path:-}" && -e "$dep_path" ]] || continue
      dep_real="$(readlink -f "$dep_path")"
      dep_base="$(basename "$dep_real")"
      if [[ ! -e "$appdir/usr/lib/$dep_base" ]]; then
        cp -a "$dep_real" "$appdir/usr/lib/$dep_base"
      fi
      if [[ "$dep_base" != "$dep" && ! -e "$appdir/usr/lib/$dep" ]]; then
        ln -sfn "$dep_base" "$appdir/usr/lib/$dep"
      fi
    done < <(ldd "$real" | awk '/=> \// { print $1, $3 }')
  }
  local soname
  for soname in \
    libxkbcommon.so.0 libxkbcommon-x11.so.0 \
    libX11.so.6 libXcursor.so.1 libXrandr.so.2 libXi.so.6 libXext.so.6 \
    libXrender.so.1 libXfixes.so.3 libXinerama.so.1 \
    libvulkan.so.1; do
    bundle_soname "$soname"
  done
  chmod +x "$appdir/AppRun"
  local out
  out="$OUT_DIR/${name}.AppImage"
  "$tools/appimagetool.AppImage" "$appdir" "$out"
  chmod +x "$out"
  rm -rf "$tools" "$(dirname "$appdir")"
}

package_macos() {
  local app dmg_src dmg attempt
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
  # "Resource busy" is a flake when a volume of this name is still attached.
  for attempt in 1 2 3 4 5; do
    if [[ -d /Volumes/OxyCharacter ]]; then
      hdiutil detach /Volumes/OxyCharacter >/dev/null 2>&1 \
        || hdiutil detach -force /Volumes/OxyCharacter >/dev/null 2>&1 \
        || true
    fi
    if hdiutil create -volname "OxyCharacter" -srcfolder "$dmg_src" -ov -format UDZO "$dmg"; then
      break
    fi
    if [[ "$attempt" -eq 5 ]]; then
      echo "hdiutil create failed after ${attempt} attempts" >&2
      exit 1
    fi
    echo "hdiutil create failed (attempt ${attempt}); retrying"
    sleep $((attempt * 3))
  done
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
  # ISCC resolves a relative OutputDir against the script, which lives in
  # tools/release/, so the directory has to be absolute.
  stage_win="$(cygpath -w "$stage")"
  out_win="$(cygpath -w "$(cd "$OUT_DIR" && pwd)")"
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
