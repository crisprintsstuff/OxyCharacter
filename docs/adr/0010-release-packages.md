# ADR 0010: Release packages ship the character bundle

**Status:** accepted

## Context

The editor and the CLI need `makehuman.oxyb` (the CC0 base mesh, morphs and
skeleton) before they can do anything. In a checkout that file is produced by
`tools/fetch-assets.sh` and read from `assets/generated/` relative to the
current directory. A person who downloads a release has neither the script's
checkout nor a Cargo workspace, and the programs are not allowed to open a
network connection of their own.

The same file has to be found after install: next to an `.exe`, inside a
macOS `.app`, and inside an AppImage, not only when the process was started
from the repository root.

## Decision

- **Ship the bundle. Do not fetch it on first run.** Release CI runs
  `tools/fetch-assets.sh` once, on Linux, and puts the resulting `.oxyb` in
  every package. The data revision is the commit in
  `tools/release/makehuman.rev` (a full SHA, so a release does not float with
  upstream `master`). MakeHuman's assets are CC0, so redistributing the
  derived bundle is permitted. The AGPL program is not downloaded and its
  licence text is not included in the package. `MakeHuman-LICENSE.ASSETS.md`
  (the CC0 deed) is.
- **The bundle format is little-endian and platform-independent** (`.oxyb`
  version 3). One build of the file is copied into the Windows, macOS and
  Linux packages.
- **Search order** when `--bundle` and `OXY_BUNDLE` are unset
  (`oxy_io::locate`):
  1. `makehuman.oxyb` beside the executable (portable zip, Linux tar.gz,
     Windows install directory).
  2. `assets/makehuman.oxyb` beside the executable.
  3. `../Resources/makehuman.oxyb` (macOS `Contents/MacOS` → `Contents/Resources`).
  4. `../share/oxycharacter/makehuman.oxyb` (AppImage `usr/bin`).
  5. `assets/generated/makehuman.oxyb` from the current directory (a source
     checkout, including `cargo run`).
  A relative path that already exists from the current directory still wins,
  so existing commands keep working. The CLI applies the same resolution to
  the bundle argument it is given.
- **User files** (projects, autosave, custom morphs, files sent to OxyBrush):
  - `OXY_USER_DIR` if set.
  - `./user` when the current directory is this source checkout.
  - Inside a `.app`, `~/Library/Application Support/OxyCharacter`. The bundle
    is not written into.
  - If a `portable` marker file sits next to a writable executable,
    `<exe directory>/user`. The Linux tar.gz and the Windows zip include the
    marker. The installer and the `.app` do not.
  - Otherwise the platform data directory: `%APPDATA%\OxyCharacter`,
    `~/Library/Application Support/OxyCharacter`, or
    `$XDG_DATA_HOME/oxycharacter` (`~/.local/share/oxycharacter`).
- **What a tag publishes.** `.github/workflows/release.yml` runs on `v*` and
  on `workflow_dispatch`. Only a tag push creates the GitHub Release.
  `workflow_dispatch` uploads the same artifacts to the Actions run.
  Windows: an Inno Setup per-user installer (Start Menu group and
  uninstaller) and a portable zip. macOS: one universal disk image (arm64 and
  x86_64). Linux: a tar.gz and an AppImage. `SHA256SUMS.txt` covers the
  archives. The release body is `docs/release-notes/<tag>.md`. A tag whose
  name contains `-alpha`, `-beta` or `-rc` is a pre-release.
- **Signing is optional.** Developer ID signing and notarization run only
  when the Apple secrets named in the workflow are present. Otherwise the app
  is ad-hoc signed, so the arm64 slice will load, and notarization is
  skipped. Gatekeeper still warns; the workaround is in the release notes.

## Consequences

- Release artifacts are large (the bundle is tens of megabytes, repeated in
  each archive) and reproducible from the pinned data revision plus the
  git tag.
- Changing the install layout means changing `oxy_io::locate` and the
  packaging scripts together. The unit tests cover the search order and the
  user-directory rules.
- An AppImage's CLI is `OxyCharacter.AppImage oxy …`. The shared libraries
  it bundles are whatever linuxdeploy copies at build time; a desktop GPU
  driver is still the host's.
