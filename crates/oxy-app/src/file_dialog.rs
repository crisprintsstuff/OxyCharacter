//! Open / save / export / import paths.
//!
//! On a normal desktop the editor asks the OS (via [`rfd`], async, so the
//! Bevy/egui frame keeps running). Headless sessions and Linux without an
//! XDG file chooser keep the typed-path dialog, including the recent-project
//! list. `OXY_FILE_DIALOG=native` or `fallback` overrides the probe.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::task::Poll;
use std::time::{Duration, Instant};

use bevy_egui::egui::{self, Color32, RichText, Vec2};

use crate::document::{After, PathDialog};
use crate::editor::Editor;

/// Dim text, same tone as the rest of the editor.
const TEXT_DIM: Color32 = Color32::from_rgb(150, 154, 160);

/// Where a dialog opens, and the path the typed fallback shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogLocation {
    pub directory: PathBuf,
    pub file_name: String,
}

impl DialogLocation {
    pub fn full_path(&self) -> PathBuf {
        if self.file_name.is_empty() {
            self.directory.clone()
        } else {
            self.directory.join(&self.file_name)
        }
    }

    /// Text shown in the typed-path dialog. Import starts empty (a directory
    /// is not a file); the others suggest a full path.
    pub fn typed_text(&self, kind: PathDialog) -> String {
        if kind == PathDialog::ImportOxb {
            String::new()
        } else {
            self.full_path().display().to_string()
        }
    }
}

impl PathDialog {
    pub fn title(self) -> &'static str {
        match self {
            PathDialog::Open => "Open project",
            PathDialog::SaveAs => "Save project as",
            PathDialog::Export => "Export glTF",
            PathDialog::ImportOxb => "Import OxyBrush file",
        }
    }

    pub fn button(self) -> &'static str {
        match self {
            PathDialog::Open => "Open",
            PathDialog::SaveAs => "Save",
            PathDialog::Export => "Export",
            PathDialog::ImportOxb => "Open",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            PathDialog::Open | PathDialog::SaveAs => "oxyc",
            PathDialog::Export => "glb",
            PathDialog::ImportOxb => "oxb",
        }
    }

    pub fn filter_name(self) -> &'static str {
        match self {
            PathDialog::Open | PathDialog::SaveAs => "OxyCharacter project",
            PathDialog::Export => "glTF binary",
            PathDialog::ImportOxb => "OxyBrush project",
        }
    }

    pub fn is_save(self) -> bool {
        matches!(self, PathDialog::SaveAs | PathDialog::Export)
    }

    fn prompt(self) -> &'static str {
        match self {
            PathDialog::Open | PathDialog::SaveAs => "OxyCharacter project (.oxyc):",
            PathDialog::Export => "glTF binary (.glb):",
            PathDialog::ImportOxb => "OxyBrush project (.oxb):",
        }
    }

    fn pending_status(self) -> &'static str {
        match self {
            PathDialog::Open => "Choose a project to open…",
            PathDialog::SaveAs => "Choose where to save the project…",
            PathDialog::Export => "Choose where to export the .glb…",
            PathDialog::ImportOxb => "Choose an OxyBrush .oxb file…",
        }
    }
}

/// Starting folder and file name for each action.
///
/// `export_dir` is the editor's export folder (normally `exports/` in the
/// working directory). Open prefers the most recent project, then the file
/// that is already open. Save uses the current project path.
pub fn location_for(
    kind: PathDialog,
    current: Option<&Path>,
    recent: &[PathBuf],
    user_dir: &Path,
    export_dir: &Path,
) -> DialogLocation {
    match kind {
        PathDialog::Open => {
            if let Some(p) = recent.first().map(PathBuf::as_path).or(current) {
                split_file(p)
            } else {
                DialogLocation {
                    directory: user_dir.join("projects"),
                    file_name: "character.oxyc".into(),
                }
            }
        }
        PathDialog::SaveAs => {
            if let Some(p) = current {
                split_file(p)
            } else {
                DialogLocation {
                    directory: user_dir.join("projects"),
                    file_name: "character.oxyc".into(),
                }
            }
        }
        PathDialog::Export => {
            let stem = current
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "character".into());
            DialogLocation {
                directory: export_dir.to_path_buf(),
                file_name: format!("{stem}.glb"),
            }
        }
        PathDialog::ImportOxb => {
            let sent = user_dir.join("oxybrush");
            DialogLocation {
                directory: if sent.is_dir() {
                    sent
                } else {
                    user_dir.to_path_buf()
                },
                file_name: String::new(),
            }
        }
    }
}

fn split_file(path: &Path) -> DialogLocation {
    let directory = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    DialogLocation {
        directory,
        file_name,
    }
}

/// Append `ext` when the path doesn't already end with it. A directory
/// becomes `character.<ext>` inside that directory.
pub fn with_extension(path: PathBuf, ext: &str) -> PathBuf {
    if path.as_os_str().is_empty() {
        return path;
    }
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    {
        return path;
    }
    if path.is_dir() {
        return path.join(format!("character.{ext}"));
    }
    let mut os = path.into_os_string();
    os.push(".");
    os.push(ext);
    PathBuf::from(os)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogBackend {
    /// OS file dialog (XDG portal on Linux, native panels on Windows and macOS).
    Native,
    /// The in-app path field. Used when no native dialog can be shown.
    Typed,
}

/// `Some` when `OXY_FILE_DIALOG` forces a backend.
pub fn backend_from_override(value: Option<&str>) -> Option<DialogBackend> {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("fallback" | "typed" | "path" | "0") => Some(DialogBackend::Typed),
        Some("native" | "1") => Some(DialogBackend::Native),
        _ => None,
    }
}

pub fn dialog_backend() -> DialogBackend {
    if let Some(forced) = backend_from_override(std::env::var("OXY_FILE_DIALOG").ok().as_deref()) {
        return forced;
    }
    *BACKEND.get_or_init(probe_backend)
}

static BACKEND: OnceLock<DialogBackend> = OnceLock::new();

fn session_bus_configured() -> bool {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
        return true;
    }
    std::env::var_os("XDG_RUNTIME_DIR").is_some_and(|dir| Path::new(&dir).join("bus").exists())
}

/// Linux native dialogs go through xdg-desktop-portal. Without a session bus
/// or a `FileChooser` interface (CI, Xvfb, a minimal container) there is
/// nothing to show, and rfd would otherwise sit on a zenity fallback that
/// also isn't there. Windows and macOS always try the OS dialog.
pub fn probe_backend() -> DialogBackend {
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    {
        if !session_bus_configured() || !portal_file_chooser() {
            DialogBackend::Typed
        } else {
            DialogBackend::Native
        }
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )))]
    {
        DialogBackend::Native
    }
}

fn portal_file_chooser() -> bool {
    // `busctl` is the smallest way to ask the session bus without linking a
    // D-Bus crate. A missing binary or a portal without FileChooser means
    // "use the typed dialog", which is the safe outcome.
    std::process::Command::new("busctl")
        .args([
            "--user",
            "--timeout=2",
            "introspect",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.FileChooser",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

type DialogFuture = Pin<Box<dyn Future<Output = Option<PathBuf>> + Send>>;

struct NativeDialog {
    kind: PathDialog,
    started: Instant,
    fut: DialogFuture,
}

impl NativeDialog {
    fn poll(&mut self) -> Poll<Option<PathBuf>> {
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        self.fut.as_mut().poll(&mut cx)
    }
}

/// Native dialog in flight. The future is `Send` but not `Sync`; the mutex
/// makes the editor state shareable across Bevy's scheduler.
#[derive(Default)]
pub struct FileDialogState {
    native: Mutex<Option<NativeDialog>>,
    /// Set when a native dialog fails immediately, so the next attempt
    /// doesn't open another invisible one.
    pub native_broken: bool,
}

const NATIVE_FAIL_WINDOW: Duration = Duration::from_millis(700);
const EXPORT_DIR: &str = "exports";

fn location_for_editor(ed: &Editor, kind: PathDialog) -> DialogLocation {
    let s = ed.session();
    location_for(
        kind,
        s.path.as_deref(),
        &s.recent,
        &s.user_dir,
        Path::new(EXPORT_DIR),
    )
}

fn prepare_directory(dir: &Path) -> PathBuf {
    let _ = std::fs::create_dir_all(dir);
    std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
}

fn native_future(kind: PathDialog, loc: DialogLocation) -> DialogFuture {
    let title = kind.title().to_owned();
    let filter = kind.filter_name().to_owned();
    let ext = kind.extension().to_owned();
    let save = kind.is_save();
    let dir = loc.directory;
    let name = loc.file_name;
    Box::pin(async move {
        let dialog = rfd::AsyncFileDialog::new()
            .set_title(&title)
            .set_directory(&dir)
            .add_filter(&filter, &[ext.as_str()]);
        let picked = if save {
            dialog.set_file_name(&name).save_file().await
        } else {
            dialog.pick_file().await
        };
        picked.map(|handle| {
            let path = handle.path().to_path_buf();
            if save {
                with_extension(path, &ext)
            } else {
                path
            }
        })
    })
}

fn open_typed(ed: &mut Editor, kind: PathDialog) {
    let loc = location_for_editor(ed, kind);
    if kind.is_save() || kind == PathDialog::Open {
        let _ = std::fs::create_dir_all(&loc.directory);
    }
    let text = loc.typed_text(kind);
    let s = ed.session.as_mut().expect("session");
    s.path_input = text;
    s.dialog = Some(kind);
}

/// Start a native dialog, or the typed one when this session can't show one.
pub fn launch(ed: &mut Editor, kind: PathDialog, files: &mut FileDialogState) {
    let busy = files
        .native
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some();
    if busy || ed.session().dialog.is_some() {
        return;
    }
    let typed = ed.session().force_typed
        || files.native_broken
        || dialog_backend() != DialogBackend::Native;
    if typed {
        open_typed(ed, kind);
        return;
    }
    let mut loc = location_for_editor(ed, kind);
    loc.directory = prepare_directory(&loc.directory);
    let status = kind.pending_status().to_owned();
    *files.native.lock().unwrap_or_else(|e| e.into_inner()) = Some(NativeDialog {
        kind,
        started: Instant::now(),
        fut: native_future(kind, loc),
    });
    ed.status = status;
}

/// Turn a `dialog_request` (menu, shortcut, or "save before closing") into a dialog.
pub fn take_request(ed: &mut Editor, files: &mut FileDialogState) {
    let Some(kind) = ed.session.as_mut().and_then(|s| s.dialog_request.take()) else {
        return;
    };
    launch(ed, kind, files);
}

fn apply(ed: &mut Editor, kind: PathDialog, path: PathBuf, export_path: &mut Option<PathBuf>) {
    match kind {
        PathDialog::Open => ed.request(After::Open(path)),
        PathDialog::SaveAs => {
            if ed.save_project(&path) {
                if let Some(after) = ed.session.as_mut().and_then(|s| s.after_save.take()) {
                    ed.perform(after);
                }
            }
        }
        PathDialog::Export => *export_path = Some(path),
        PathDialog::ImportOxb => ed.open_oxb(&path),
    }
}

/// Poll the native dialog. A result that arrives before the dialog could
/// have been drawn means the backend isn't there; fall back to typing a path.
pub fn poll_native(
    ctx: &egui::Context,
    ed: &mut Editor,
    files: &mut FileDialogState,
    export_path: &mut Option<PathBuf>,
) {
    let (kind, started, path) = {
        let mut guard = files.native.lock().unwrap_or_else(|e| e.into_inner());
        let Some(native) = guard.as_mut() else {
            return;
        };
        ctx.request_repaint_after(Duration::from_millis(50));
        match native.poll() {
            Poll::Pending => return,
            Poll::Ready(path) => {
                let kind = native.kind;
                let started = native.started;
                *guard = None;
                (kind, started, path)
            }
        }
    };
    match path {
        Some(path) => apply(ed, kind, path, export_path),
        None if started.elapsed() < NATIVE_FAIL_WINDOW => {
            files.native_broken = true;
            open_typed(ed, kind);
            ed.status = "Native file dialog isn't available; type a path.".into();
        }
        None => {
            if kind == PathDialog::SaveAs {
                if let Some(s) = ed.session.as_mut() {
                    s.after_save = None;
                }
            }
            ed.status = "Cancelled".into();
        }
    }
}

pub fn draw_typed(ctx: &egui::Context, ed: &mut Editor, export_path: &mut Option<PathBuf>) {
    let Some(kind) = ed.session().dialog else {
        return;
    };
    let (mut open, mut go, mut pick) = (true, false, None);
    egui::Window::new(kind.title())
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(kind.prompt());
            ui.label(
                RichText::new("No native file dialog in this session — type a path.")
                    .size(11.0)
                    .color(TEXT_DIM),
            );
            let s = ed.session.as_mut().expect("session");
            let field = ui.add(
                egui::TextEdit::singleline(&mut s.path_input)
                    .desired_width(460.0)
                    .hint_text(match kind {
                        PathDialog::ImportOxb => "/path/to/sculpt.oxb",
                        PathDialog::Export => "exports/character.glb",
                        _ => "/path/to/character.oxyc",
                    }),
            );
            go = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if kind == PathDialog::Open && !s.recent.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new("Recent").size(11.0).color(TEXT_DIM));
                for p in &s.recent {
                    if ui.link(p.display().to_string()).clicked() {
                        pick = Some(p.clone());
                    }
                }
            }
            if kind == PathDialog::ImportOxb {
                ui.label(
                    RichText::new("Tip: you can also drop an .oxb file onto the window.")
                        .size(11.0)
                        .color(TEXT_DIM),
                );
            }
            ui.add_space(4.0);
            go |= ui
                .add_enabled(
                    !s.path_input.trim().is_empty(),
                    egui::Button::new(kind.button()),
                )
                .clicked();
        });
    let chosen = {
        let s = ed.session.as_mut().expect("session");
        if let Some(p) = pick {
            s.path_input = p.display().to_string();
            go = true;
        }
        if go {
            let mut path = PathBuf::from(s.path_input.trim());
            if kind.is_save() {
                path = with_extension(path, kind.extension());
            }
            s.dialog = None;
            Some(path)
        } else if !open {
            s.dialog = None;
            if kind == PathDialog::SaveAs {
                s.after_save = None;
            }
            None
        } else {
            None
        }
    };
    if let Some(path) = chosen {
        apply(ed, kind, path, export_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_names_map_to_a_backend() {
        assert_eq!(
            backend_from_override(Some("fallback")),
            Some(DialogBackend::Typed)
        );
        assert_eq!(
            backend_from_override(Some("TYPED")),
            Some(DialogBackend::Typed)
        );
        assert_eq!(backend_from_override(Some("0")), Some(DialogBackend::Typed));
        assert_eq!(
            backend_from_override(Some("native")),
            Some(DialogBackend::Native)
        );
        assert_eq!(
            backend_from_override(Some("1")),
            Some(DialogBackend::Native)
        );
        assert_eq!(backend_from_override(Some("maybe")), None);
        assert_eq!(backend_from_override(None), None);
    }

    #[test]
    fn missing_session_bus_has_no_native_dialog() {
        if session_bus_configured() {
            return;
        }
        assert_eq!(probe_backend(), DialogBackend::Typed);
    }

    #[test]
    fn extension_is_added_once() {
        assert_eq!(
            with_extension(PathBuf::from("hero"), "oxyc"),
            PathBuf::from("hero.oxyc")
        );
        assert_eq!(
            with_extension(PathBuf::from("hero.GLB"), "glb"),
            PathBuf::from("hero.GLB")
        );
        assert_eq!(
            with_extension(PathBuf::from("hero.glb.bak"), "glb"),
            PathBuf::from("hero.glb.bak.glb")
        );
        let dir = std::env::temp_dir().join(format!("oxy-dlg-ext-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(
            with_extension(dir.clone(), "glb"),
            dir.join("character.glb")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dialog_starts_in_a_sensible_folder() {
        let user = PathBuf::from("/tmp/oxy-user");
        let recent = vec![PathBuf::from("/tmp/work/hero.oxyc")];
        let export = Path::new("exports");

        let open = location_for(PathDialog::Open, None, &recent, &user, export);
        assert_eq!(open.directory, PathBuf::from("/tmp/work"));
        assert_eq!(open.file_name, "hero.oxyc");
        assert_eq!(open.typed_text(PathDialog::Open), "/tmp/work/hero.oxyc");

        let fresh = location_for(PathDialog::Open, None, &[], &user, export);
        assert_eq!(
            fresh.full_path(),
            user.join("projects").join("character.oxyc")
        );

        let save = location_for(
            PathDialog::SaveAs,
            Some(Path::new("/tmp/work/hero.oxyc")),
            &[],
            &user,
            export,
        );
        assert_eq!(save.full_path(), PathBuf::from("/tmp/work/hero.oxyc"));

        let untitled = location_for(PathDialog::SaveAs, None, &recent, &user, export);
        assert_eq!(
            untitled.full_path(),
            user.join("projects").join("character.oxyc")
        );

        let exp = location_for(
            PathDialog::Export,
            Some(Path::new("/tmp/work/hero.oxyc")),
            &[],
            &user,
            export,
        );
        assert_eq!(exp.directory, PathBuf::from("exports"));
        assert_eq!(exp.file_name, "hero.glb");

        let exp_new = location_for(PathDialog::Export, None, &[], &user, export);
        assert_eq!(exp_new.file_name, "character.glb");

        let imp = location_for(PathDialog::ImportOxb, None, &[], &user, export);
        assert_eq!(imp.directory, user);
        assert!(imp.file_name.is_empty());
        assert!(imp.typed_text(PathDialog::ImportOxb).is_empty());
    }
}
