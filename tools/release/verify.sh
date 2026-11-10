#!/usr/bin/env bash
# Check a just-built release directory. Used by .github/workflows/release.yml.
#
#   tools/release/verify.sh <linux|macos|windows> <out-dir>
set -euo pipefail

PLATFORM="${1:?platform}"
OUT="${2:?output directory}"

fail() {
  echo "verify: $*" >&2
  exit 1
}

# Globs from the workflow are relative to the repo. Later steps cd away,
# and a relative AppImage path then does not exist (the shell reports
# "No such file or directory", exit 127).
abs_path() {
  local f="$1"
  echo "$(cd "$(dirname "$f")" && pwd)/$(basename "$f")"
}

require_listing() {
  local listing="$1"
  shift
  local needle
  for needle in "$@"; do
    if ! grep -q "$needle" <<<"$listing"; then
      echo "$listing" >&2
      fail "archive is missing $needle"
    fi
  done
}

info_from() {
  local bin="$1"
  local work
  work="$(mktemp -d)"
  # A directory that is neither the install folder nor a source checkout.
  local out
  out="$(cd "$work" && "$bin" info makehuman.oxyb)"
  echo "$out"
  grep -q "vertices" <<<"$out"
  rm -rf "$work"
}

case "$PLATFORM" in
  linux)
    tarball="$(abs_path "$(echo "$OUT"/OxyCharacter-*-linux-x86_64.tar.gz)")"
    appimage="$(abs_path "$(echo "$OUT"/OxyCharacter-*-linux-x86_64.AppImage)")"
    [[ -f "$tarball" && -f "$appimage" ]] || fail "expected a tar.gz and an AppImage in $OUT"
    listing="$(tar -tzf "$tarball")"
    require_listing "$listing" '/oxycharacter$' '/oxy$' '/makehuman.oxyb$' '/portable$' \
      '/LICENSE-MIT$' '/LICENSE-APACHE$' '/LICENSES.md$' '/INSTALL.txt$' '/BUNDLE-REVISION.txt$' \
      '/python/oxycharacter/__init__.py$' '/plugins/athletic_build.py$'
    extracted="$(mktemp -d)"
    tar -xzf "$tarball" -C "$extracted"
    root="$(echo "$extracted"/OxyCharacter-*)"
    info_from "$root/oxy"
    chmod +x "$appimage"
    export APPIMAGE_EXTRACT_AND_RUN=1
    app_out="$(mktemp -d)"
    (
      cd "$app_out"
      "$appimage" oxy info makehuman.oxyb
    ) | tee /tmp/oxy-appimage-info.txt
    grep -q "vertices" /tmp/oxy-appimage-info.txt
    if command -v xvfb-run >/dev/null 2>&1 || command -v Xvfb >/dev/null 2>&1; then
      export VK_ICD_FILENAMES="${VK_ICD_FILENAMES:-/usr/share/vulkan/icd.d/lvp_icd.json}"
      smoke_editor() {
        local bin="$1" shot="$2"
        export OXY_USER_DIR
        OXY_USER_DIR="$(mktemp -d)"
        rm -f "$shot"
        if command -v xvfb-run >/dev/null 2>&1; then
          xvfb-run -a -s "-screen 0 1280x720x24" \
            "$bin" --frames 20 --screenshot "$shot"
        else
          local display=":98" xvfb_pid
          Xvfb "$display" -screen 0 1280x720x24 >/tmp/oxy-xvfb.log 2>&1 &
          xvfb_pid=$!
          DISPLAY="$display" "$bin" --frames 20 --screenshot "$shot"
          kill "$xvfb_pid" >/dev/null 2>&1 || true
        fi
        [[ -s "$shot" ]] || fail "editor smoke screenshot was not written ($shot)"
        echo "editor smoke screenshot: $shot"
      }
      smoke_editor "$root/oxycharacter" "${OXY_SMOKE_SHOT:-/tmp/oxy-release-smoke.png}"
      smoke_editor "$appimage" /tmp/oxy-appimage-smoke.png
    fi
    ;;
  macos)
    dmg="$(abs_path "$(echo "$OUT"/OxyCharacter-*-macos-universal.dmg)")"
    [[ -f "$dmg" ]] || fail "expected a dmg in $OUT"
    mnt="$(mktemp -d)"
    hdiutil attach -nobrowse -readonly -mountpoint "$mnt" "$dmg"
    trap 'hdiutil detach "$mnt" >/dev/null 2>&1 || hdiutil detach -force "$mnt" >/dev/null 2>&1 || true' EXIT
    app="$mnt/OxyCharacter.app"
    [[ -x "$app/Contents/MacOS/oxycharacter" ]] || fail "editor missing from the app bundle"
    [[ -x "$app/Contents/MacOS/oxy" ]] || fail "CLI missing from the app bundle"
    [[ -f "$app/Contents/Resources/makehuman.oxyb" ]] || fail "bundle missing from Resources"
    [[ -f "$app/Contents/Resources/LICENSE-MIT" ]] || fail "LICENCE-MIT missing"
    [[ -f "$app/Contents/Resources/LICENSE-APACHE" ]] || fail "LICENSE-APACHE missing"
    [[ -f "$app/Contents/Resources/LICENSES.md" ]] || fail "LICENSES.md missing"
    [[ -f "$app/Contents/Resources/INSTALL.txt" ]] || fail "INSTALL.txt missing"
    [[ -f "$app/Contents/Resources/python/oxycharacter/__init__.py" ]] || fail "plugin SDK missing"
    [[ ! -e "$app/Contents/MacOS/portable" ]] || fail "the .app must not be a portable layout"
    for bin in oxycharacter oxy; do
      info="$(lipo -info "$app/Contents/MacOS/$bin")"
      echo "$info"
      grep -q "x86_64" <<<"$info"
      grep -q "arm64" <<<"$info"
    done
    info_from "$app/Contents/MacOS/oxy"
    ;;
  windows)
    zip="$(abs_path "$(echo "$OUT"/OxyCharacter-*-windows-x86_64.zip)")"
    setup="$(abs_path "$(echo "$OUT"/OxyCharacter-*-windows-x86_64-setup.exe)")"
    [[ -f "$zip" && -f "$setup" ]] || fail "expected a zip and a setup exe in $OUT"
    listing="$(tar -tf "$zip")"
    require_listing "$listing" '/oxycharacter.exe$' '/oxy.exe$' '/makehuman.oxyb$' '/portable$' \
      '/LICENSE-MIT$' '/LICENSE-APACHE$' '/LICENSES.md$' '/INSTALL.txt$' \
      '/python/oxycharacter/__init__.py$' '/plugins/athletic_build.py$'
    size="$(wc -c < "$setup")"
    if [[ "$size" -lt 10000000 ]]; then
      fail "installer is only $size bytes"
    fi
    extracted="$(mktemp -d)"
    tar -xf "$zip" -C "$extracted"
    root="$(echo "$extracted"/OxyCharacter-*)"
    info_from "$root/oxy.exe"

    # Silent per-user install: Start Menu shortcut, then the uninstaller.
    dest="$(mktemp -d)"
    dest_win="$(cygpath -w "$dest")"
    log_win="$(cygpath -w /tmp/oxy-setup.log)"
    MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
      "$setup" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART "/DIR=$dest_win" "/LOG=$log_win"
    [[ -f "$dest/oxycharacter.exe" && -f "$dest/oxy.exe" && -f "$dest/makehuman.oxyb" ]] \
      || fail "installer did not lay down the editor, CLI and bundle"
    [[ ! -f "$dest/portable" ]] || fail "the installed copy must not carry the portable marker"
    info_from "$dest/oxy.exe"
    start_menu="$(cygpath "$APPDATA")/Microsoft/Windows/Start Menu/Programs/OxyCharacter"
    [[ -f "$start_menu/OxyCharacter.lnk" ]] || fail "Start Menu shortcut was not created ($start_menu)"
    [[ -f "$start_menu/Uninstall OxyCharacter.lnk" ]] || fail "uninstall shortcut was not created"
    uninstaller="$(echo "$dest"/unins*.exe)"
    [[ -f "$uninstaller" ]] || fail "uninstaller was not written"
    MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
      "$uninstaller" /VERYSILENT /NORESTART
    [[ ! -f "$dest/oxycharacter.exe" ]] || fail "uninstaller left the editor in place"
    ;;
  *)
    fail "unknown platform $PLATFORM"
    ;;
esac

echo "verify: $PLATFORM packages ok"
