#!/usr/bin/env bash
# Sign the macOS app, then optionally notarize a disk image.
#
#   sign-macos.sh app /path/to/OxyCharacter.app
#   sign-macos.sh notarize /path/to/OxyCharacter.dmg
#
# Developer ID signing runs only when both of these secrets are set:
#   APPLE_CERTIFICATE_P12_BASE64   base64-encoded .p12
#   APPLE_CERTIFICATE_PASSWORD     password for that .p12
# Optional:
#   APPLE_SIGNING_IDENTITY         e.g. "Developer ID Application: Name (TEAMID)"
#                                  (default: the identity imported from the .p12)
#
# Notarization runs only when these are set as well:
#   APPLE_NOTARIZE_APPLE_ID        Apple ID email
#   APPLE_NOTARIZE_PASSWORD        app-specific password
#   APPLE_TEAM_ID                  10-character team id
#
# With none of the certificate secrets set, the app is ad-hoc signed so macOS
# will load the arm64 slice, and notarization is skipped. Gatekeeper still
# warns on a downloaded ad-hoc build; see INSTALL.txt.
#
# Secret values are never printed. Keychain setup stays in this process:
# a command substitution would discard it before codesign runs.
set -euo pipefail

MODE="${1:?app or notarize}"
TARGET="${2:?path to the .app or .dmg}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ENTITLEMENTS="$ROOT/tools/release/entitlements.plist"

have_certificate() {
  [[ -n "${APPLE_CERTIFICATE_P12_BASE64:-}" && -n "${APPLE_CERTIFICATE_PASSWORD:-}" ]]
}

ad_hoc() {
  echo "No Apple Developer ID certificate secrets set. Ad-hoc signing ${TARGET}."
  echo "Gatekeeper will still warn on a downloaded app. Notarization skipped."
  codesign --force --deep --sign - "$TARGET"
}

case "$MODE" in
  app)
    if [[ ! -d "$TARGET" ]]; then
      echo "not an app bundle: $TARGET" >&2
      exit 1
    fi
    if ! have_certificate; then
      ad_hoc
      exit 0
    fi
    keychain="${RUNNER_TEMP:-/tmp}/oxy-signing.keychain-db"
    keychain_password="$(openssl rand -base64 24)"
    p12="${RUNNER_TEMP:-/tmp}/oxy-developer-id.p12"
    # shellcheck disable=SC2064
    trap "security delete-keychain '$keychain' >/dev/null 2>&1 || true; rm -f '$p12'" EXIT
    printf '%s' "$APPLE_CERTIFICATE_P12_BASE64" | base64 --decode > "$p12"
    security create-keychain -p "$keychain_password" "$keychain" >/dev/null
    security default-keychain -s "$keychain" >/dev/null
    security unlock-keychain -p "$keychain_password" "$keychain" >/dev/null
    security set-keychain-settings -lut 21600 "$keychain" >/dev/null
    security import "$p12" -k "$keychain" -P "$APPLE_CERTIFICATE_PASSWORD" \
      -T /usr/bin/codesign -T /usr/bin/security >/dev/null
    security set-key-partition-list -S apple-tool:,apple:,codesign: -s \
      -k "$keychain_password" "$keychain" >/dev/null
    if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
      identity="$APPLE_SIGNING_IDENTITY"
    else
      identity="$(
        security find-identity -p codesigning -v "$keychain" \
          | awk -F '"' 'NF > 1 { print $2; exit }'
      )"
    fi
    if [[ -z "$identity" ]]; then
      echo "The certificate was imported but no codesigning identity was found." >&2
      exit 1
    fi
    echo "Signing with a Developer ID identity."
    codesign --force --options runtime --timestamp --entitlements "$ENTITLEMENTS" --sign "$identity" \
      "$TARGET/Contents/MacOS/oxycharacter"
    codesign --force --options runtime --timestamp --entitlements "$ENTITLEMENTS" --sign "$identity" \
      "$TARGET/Contents/MacOS/oxy"
    codesign --force --options runtime --timestamp --entitlements "$ENTITLEMENTS" --sign "$identity" \
      "$TARGET"
    codesign --verify --verbose=2 "$TARGET"
    ;;
  notarize)
    if ! have_certificate; then
      echo "Notarization skipped (no Developer ID certificate secrets)."
      exit 0
    fi
    if [[ -z "${APPLE_NOTARIZE_APPLE_ID:-}" || -z "${APPLE_NOTARIZE_PASSWORD:-}" || -z "${APPLE_TEAM_ID:-}" ]]; then
      echo "Notarization skipped (APPLE_NOTARIZE_APPLE_ID, APPLE_NOTARIZE_PASSWORD and APPLE_TEAM_ID are not all set)."
      exit 0
    fi
    if [[ ! -f "$TARGET" ]]; then
      echo "not a disk image: $TARGET" >&2
      exit 1
    fi
    echo "Submitting the disk image for notarization."
    xcrun notarytool submit "$TARGET" \
      --apple-id "$APPLE_NOTARIZE_APPLE_ID" \
      --password "$APPLE_NOTARIZE_PASSWORD" \
      --team-id "$APPLE_TEAM_ID" \
      --wait
    xcrun stapler staple "$TARGET"
    echo "Notarization staple applied."
    ;;
  *)
    echo "usage: sign-macos.sh app|notarize PATH" >&2
    exit 1
    ;;
esac
