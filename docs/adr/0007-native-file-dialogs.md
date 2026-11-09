# ADR 0007: Native file dialogs, with a typed-path fallback

**Status:** accepted

## Context

Open, Save as, glTF export and OxyBrush import asked for a path in a small
egui field. That works in screenshots and over SSH, and it is awkward on a
desktop where people expect the system file chooser, filters and the last
folder they used.

## Decision

- **Ask the OS first.** The editor uses the `rfd` crate's async file dialog
  (XDG desktop portal on Linux, the system panels on Windows and macOS). The
  future is polled from the UI thread, so Bevy and egui keep drawing while
  the chooser is open.
- **Filters and starting folders.** Projects are `.oxyc`, OxyBrush imports
  are `.oxb`, exports are `.glb`. Open starts in the most recent project's
  folder, Save as in the current file's folder, export in `exports/`, import
  in `user/oxybrush/` when that folder exists.
- **Recent files stay.** File › Open recent, and the recent list inside the
  typed dialog, are unchanged. A native chooser cannot show our list inside
  the OS window.
- **Typed path when there is no native dialog.** Linux without a session bus
  or an `org.freedesktop.portal.FileChooser` (CI, Xvfb, a container) uses the
  existing path field. A native dialog that returns before it could have been
  shown is treated the same way, so a missing portal is not reported as
  Cancel. `OXY_FILE_DIALOG=native` or `fallback` overrides the probe.
  `--dialog open|save|export|import` always opens the typed dialog, which is
  what screenshot and smoke runs use.
- **No GTK backend.** `rfd`'s GTK feature needs the C library and is not
  enabled. We do not fall through to zenity: if the portal is absent, the
  in-app field is the fallback, not a second external dialog.

## Consequences

- Headless `cargo test` and `--open` / `--save-as` do not open a dialog.
- A desktop without `busctl` is classified as "no portal" and gets the typed
  field; `OXY_FILE_DIALOG=native` forces the OS dialog anyway.
- Export no longer writes `exports/character-<time>.glb` silently. The button
  asks for a path, still defaulting to `exports/<project>.glb`.
