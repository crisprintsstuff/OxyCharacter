//! Where an installed editor finds `makehuman.oxyb`, and where it writes
//! user files.
//!
//! Source checkouts keep the old layout: the bundle is
//! `assets/generated/makehuman.oxyb` relative to the current directory, and
//! user files go in `./user`. An installed copy cannot depend on that. The
//! editor does not download the bundle (the process has no network access);
//! release packages ship the file next to the executable or inside the
//! bundle. See ADR 0010.

use std::path::{Path, PathBuf};

/// File name of the shipped character bundle.
pub const BUNDLE_NAME: &str = "makehuman.oxyb";

/// Empty file next to a portable build. User files then live in `user/`
/// beside the executable, so deleting the folder removes the app and its data.
pub const PORTABLE_MARKER: &str = "portable";

/// Operating system used to choose the per-user data directory.
///
/// Passed in (rather than read from `cfg`) so every platform's path can be
/// tested on one machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Macos,
    Windows,
}

/// Inputs for [`default_user_dir`]. All paths are borrowed from the caller.
pub struct UserDirQuery<'a> {
    pub exe: &'a Path,
    pub cwd: &'a Path,
    pub platform: Platform,
    pub home: Option<&'a Path>,
    pub appdata: Option<&'a Path>,
    pub xdg_data_home: Option<&'a Path>,
    /// Whether the directory that contains `exe` accepts new files.
    /// Only consulted for a portable build (see [`PORTABLE_MARKER`]).
    pub exe_dir_writable: bool,
}

/// This machine's platform, for [`user_dir_from_env`].
pub fn host_platform() -> Platform {
    #[cfg(target_os = "windows")]
    {
        Platform::Windows
    }
    #[cfg(target_os = "macos")]
    {
        Platform::Macos
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Platform::Linux
    }
}

/// `true` when `exe` is `Something.app/Contents/MacOS/<binary>`.
pub fn inside_macos_app(exe: &Path) -> bool {
    let comps: Vec<std::ffi::OsString> = exe
        .components()
        .map(|c| c.as_os_str().to_os_string())
        .collect();
    comps
        .windows(3)
        .any(|w| w[0].to_string_lossy().ends_with(".app") && w[1] == "Contents" && w[2] == "MacOS")
}

/// A Cargo workspace that looks like this repository.
fn is_source_checkout(cwd: &Path) -> bool {
    cwd.join("crates/oxy-app/Cargo.toml").is_file() && cwd.join("tools/fetch-assets.sh").is_file()
}

fn has_portable_marker(exe: &Path) -> bool {
    exe.parent()
        .is_some_and(|dir| dir.join(PORTABLE_MARKER).is_file())
}

/// Directories, other than the executable's own directory, that may hold the
/// bundle under [`BUNDLE_NAME`] or a caller-supplied file name.
fn extra_bundle_dirs(exe_dir: &Path) -> [PathBuf; 3] {
    [
        exe_dir.join("assets"),
        // OxyCharacter.app/Contents/MacOS → Contents/Resources
        exe_dir.join("..").join("Resources"),
        // AppImage usr/bin → usr/share/oxycharacter
        exe_dir.join("..").join("share").join("oxycharacter"),
    ]
}

/// Places [`find_bundle`] searches when the caller does not pass a path.
///
/// Order: beside the executable, `assets/` beside it, the macOS app
/// `Resources` directory, the AppImage `share/oxycharacter` directory, then
/// the source-checkout path relative to `cwd`.
pub fn bundle_candidates(exe: &Path, cwd: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = exe.parent() {
        out.push(dir.join(BUNDLE_NAME));
        for extra in extra_bundle_dirs(dir) {
            out.push(extra.join(BUNDLE_NAME));
        }
    }
    out.push(cwd.join("assets").join("generated").join(BUNDLE_NAME));
    out
}

/// Resolve a bundle path the user typed.
///
/// Absolute paths are returned unchanged. A relative path that exists from
/// `cwd` wins, so `assets/generated/makehuman.oxyb` in a checkout behaves as
/// before. Otherwise a bare file name is tried beside the executable and in
/// the install layouts from [`bundle_candidates`].
pub fn resolve_bundle_argument(arg: &Path, exe: &Path, cwd: &Path) -> PathBuf {
    if arg.is_absolute() {
        return arg.to_path_buf();
    }
    let from_cwd = cwd.join(arg);
    if from_cwd.is_file() {
        return from_cwd;
    }
    if let Some(dir) = exe.parent() {
        let beside = dir.join(arg);
        if beside.is_file() {
            return beside;
        }
        // A nested relative path (`assets/generated/...`) is a checkout path,
        // not an install-layout path. Only a bare file name is tried in
        // Resources / share.
        if arg.components().count() == 1 {
            for extra in extra_bundle_dirs(dir) {
                let candidate = extra.join(arg);
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }
    from_cwd
}

/// The bundle to open.
///
/// `explicit` is `--bundle` or `OXY_BUNDLE`. When it is set, only that path
/// is considered (after [`resolve_bundle_argument`]). Otherwise the first
/// existing candidate is used.
///
/// The error value is the list of paths that were tried, for the message
/// the editor prints.
pub fn find_bundle(
    explicit: Option<&Path>,
    exe: &Path,
    cwd: &Path,
) -> Result<PathBuf, Vec<PathBuf>> {
    if let Some(path) = explicit {
        let resolved = resolve_bundle_argument(path, exe, cwd);
        if resolved.is_file() {
            Ok(resolved)
        } else {
            Err(vec![resolved])
        }
    } else {
        let candidates = bundle_candidates(exe, cwd);
        if let Some(hit) = candidates.iter().find(|p| p.is_file()) {
            Ok(hit.clone())
        } else {
            Err(candidates)
        }
    }
}

fn platform_user_dir(q: &UserDirQuery<'_>) -> PathBuf {
    match q.platform {
        Platform::Windows => {
            if let Some(appdata) = q.appdata {
                return appdata.join("OxyCharacter");
            }
            if let Some(home) = q.home {
                return home.join("AppData").join("Roaming").join("OxyCharacter");
            }
        }
        Platform::Macos => {
            if let Some(home) = q.home {
                return home
                    .join("Library")
                    .join("Application Support")
                    .join("OxyCharacter");
            }
        }
        Platform::Linux => {
            if let Some(xdg) = q.xdg_data_home {
                return xdg.join("oxycharacter");
            }
            if let Some(home) = q.home {
                return home.join(".local").join("share").join("oxycharacter");
            }
        }
    }
    PathBuf::from("user")
}

/// Directory for projects, autosave, custom morphs and files sent to OxyBrush.
///
/// `OXY_USER_DIR` is applied by the caller and skips this function.
///
/// 1. Source checkout (`cwd` contains this repo) → `<cwd>/user`.
/// 2. Inside a macOS `.app` → `~/Library/Application Support/OxyCharacter`.
///    Nothing is written into the bundle.
/// 3. Portable marker beside a writable executable → `<exe dir>/user`.
/// 4. Otherwise the per-user data directory for `platform`.
pub fn default_user_dir(q: &UserDirQuery<'_>) -> PathBuf {
    if is_source_checkout(q.cwd) {
        return q.cwd.join("user");
    }
    if inside_macos_app(q.exe) {
        return platform_user_dir(q);
    }
    if has_portable_marker(q.exe) && q.exe_dir_writable {
        if let Some(dir) = q.exe.parent() {
            return dir.join("user");
        }
    }
    platform_user_dir(q)
}

fn dir_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".oxy-write-probe-{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// [`default_user_dir`] using this process's environment.
///
/// The writable probe runs only when a portable marker is present, so a
/// source checkout does not touch the executable's directory.
pub fn user_dir_from_env(exe: &Path, cwd: &Path) -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
    let xdg = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
    let exe_dir_writable = has_portable_marker(exe) && exe.parent().is_some_and(dir_writable);
    default_user_dir(&UserDirQuery {
        exe,
        cwd,
        platform: host_platform(),
        home: home.as_deref(),
        appdata: appdata.as_deref(),
        xdg_data_home: xdg.as_deref(),
        exe_dir_writable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SCRATCH_SEQ: AtomicU64 = AtomicU64::new(0);

    fn scratch(name: &str) -> PathBuf {
        let n = SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("oxy-locate-{name}-{n}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"bundle").unwrap();
    }

    fn checkout(dir: &Path) {
        touch(&dir.join("crates/oxy-app/Cargo.toml"));
        touch(&dir.join("tools/fetch-assets.sh"));
    }

    fn query<'a>(
        exe: &'a Path,
        cwd: &'a Path,
        platform: Platform,
        home: Option<&'a Path>,
        appdata: Option<&'a Path>,
        xdg: Option<&'a Path>,
        writable: bool,
    ) -> UserDirQuery<'a> {
        UserDirQuery {
            exe,
            cwd,
            platform,
            home,
            appdata,
            xdg_data_home: xdg,
            exe_dir_writable: writable,
        }
    }

    #[test]
    fn dev_checkout_bundle_and_user_dir() {
        let root = scratch("dev");
        checkout(&root);
        let bundle = root.join("assets/generated").join(BUNDLE_NAME);
        touch(&bundle);
        let exe = root.join("target/release/oxycharacter");
        touch(&exe);
        let found = find_bundle(None, &exe, &root).unwrap();
        assert_eq!(
            found.canonicalize().unwrap(),
            bundle.canonicalize().unwrap()
        );
        let home = root.join("home");
        let dir = default_user_dir(&query(
            &exe,
            &root,
            Platform::Linux,
            Some(&home),
            None,
            None,
            true,
        ));
        assert_eq!(dir, root.join("user"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn flat_install_prefers_the_file_beside_the_executable() {
        let root = scratch("flat");
        let exe_dir = root.join("OxyCharacter");
        let exe = exe_dir.join("oxycharacter");
        let bundle = exe_dir.join(BUNDLE_NAME);
        touch(&exe);
        touch(&bundle);
        // A checkout-shaped path must not win when the install copy exists.
        let cwd = root.join("elsewhere");
        checkout(&cwd);
        touch(&cwd.join("assets/generated").join(BUNDLE_NAME));
        let found = find_bundle(None, &exe, &cwd).unwrap();
        assert_eq!(
            found.canonicalize().unwrap(),
            bundle.canonicalize().unwrap()
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn macos_app_resources_and_application_support() {
        let root = scratch("macapp");
        let exe = root.join("OxyCharacter.app/Contents/MacOS/oxycharacter");
        let bundle = root
            .join("OxyCharacter.app/Contents/Resources")
            .join(BUNDLE_NAME);
        touch(&exe);
        touch(&bundle);
        let cwd = root.join("cwd");
        fs::create_dir_all(&cwd).unwrap();
        let found = find_bundle(None, &exe, &cwd).unwrap();
        assert_eq!(
            found.canonicalize().unwrap(),
            bundle.canonicalize().unwrap()
        );
        assert!(inside_macos_app(&exe));
        let home = root.join("home");
        fs::write(exe.parent().unwrap().join(PORTABLE_MARKER), b"").unwrap();
        let dir = default_user_dir(&query(
            &exe,
            &cwd,
            Platform::Macos,
            Some(&home),
            None,
            None,
            true,
        ));
        assert_eq!(
            dir,
            home.join("Library")
                .join("Application Support")
                .join("OxyCharacter")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn appimage_share_directory() {
        let root = scratch("appimage");
        let exe = root.join("usr/bin/oxycharacter");
        let bundle = root.join("usr/share/oxycharacter").join(BUNDLE_NAME);
        touch(&exe);
        touch(&bundle);
        let cwd = root.join("cwd");
        fs::create_dir_all(&cwd).unwrap();
        let found = find_bundle(None, &exe, &cwd).unwrap();
        assert_eq!(
            found.canonicalize().unwrap(),
            bundle.canonicalize().unwrap()
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn explicit_path_is_not_replaced_when_missing() {
        let root = scratch("explicit");
        let exe_dir = root.join("bin");
        let exe = exe_dir.join("oxycharacter");
        touch(&exe);
        touch(&exe_dir.join(BUNDLE_NAME));
        let cwd = root.join("cwd");
        fs::create_dir_all(&cwd).unwrap();
        let missing = PathBuf::from("no-such.oxyb");
        let err = find_bundle(Some(&missing), &exe, &cwd).unwrap_err();
        assert_eq!(err, vec![cwd.join("no-such.oxyb")]);
        let found = find_bundle(Some(Path::new(BUNDLE_NAME)), &exe, &cwd).unwrap();
        assert_eq!(
            found.canonicalize().unwrap(),
            exe_dir.join(BUNDLE_NAME).canonicalize().unwrap()
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cwd_relative_argument_beats_the_install_copy() {
        let root = scratch("cwdarg");
        let exe_dir = root.join("bin");
        let exe = exe_dir.join("oxy");
        touch(&exe);
        touch(&exe_dir.join(BUNDLE_NAME));
        let cwd = root.join("work");
        let local = cwd.join(BUNDLE_NAME);
        touch(&local);
        let resolved = resolve_bundle_argument(Path::new(BUNDLE_NAME), &exe, &cwd);
        assert_eq!(
            resolved.canonicalize().unwrap(),
            local.canonicalize().unwrap()
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_bundle_lists_every_candidate() {
        let root = scratch("missing");
        let exe = root.join("bin/oxycharacter");
        touch(&exe);
        let cwd = root.join("cwd");
        fs::create_dir_all(&cwd).unwrap();
        let err = find_bundle(None, &exe, &cwd).unwrap_err();
        assert!(err.len() >= 4);
        assert!(err.iter().any(|p| p.ends_with(BUNDLE_NAME)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn portable_marker_uses_the_exe_directory_when_it_is_writable() {
        let root = scratch("portable");
        let exe_dir = root.join("OxyCharacter");
        let exe = exe_dir.join("oxycharacter.exe");
        touch(&exe);
        fs::write(exe_dir.join(PORTABLE_MARKER), b"").unwrap();
        let cwd = root.join("cwd");
        fs::create_dir_all(&cwd).unwrap();
        let home = root.join("home");
        let appdata = root.join("AppData/Roaming");
        let dir = default_user_dir(&query(
            &exe,
            &cwd,
            Platform::Windows,
            Some(&home),
            Some(&appdata),
            None,
            true,
        ));
        assert_eq!(dir, exe_dir.join("user"));
        let locked = default_user_dir(&query(
            &exe,
            &cwd,
            Platform::Windows,
            Some(&home),
            Some(&appdata),
            None,
            false,
        ));
        assert_eq!(locked, appdata.join("OxyCharacter"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn installed_layouts_use_the_platform_data_dir() {
        let root = scratch("platform");
        let exe = root.join("Program Files/OxyCharacter/oxycharacter.exe");
        touch(&exe);
        let cwd = root.join("System32");
        fs::create_dir_all(&cwd).unwrap();
        let home = root.join("home");
        let appdata = root.join("AppData/Roaming");
        let xdg = root.join("xdg");
        assert_eq!(
            default_user_dir(&query(
                &exe,
                &cwd,
                Platform::Windows,
                Some(&home),
                Some(&appdata),
                None,
                false
            )),
            appdata.join("OxyCharacter")
        );
        assert_eq!(
            default_user_dir(&query(
                &exe,
                &cwd,
                Platform::Windows,
                Some(&home),
                None,
                None,
                false
            )),
            home.join("AppData").join("Roaming").join("OxyCharacter")
        );
        assert_eq!(
            default_user_dir(&query(
                &exe,
                &cwd,
                Platform::Linux,
                Some(&home),
                None,
                Some(&xdg),
                false
            )),
            xdg.join("oxycharacter")
        );
        assert_eq!(
            default_user_dir(&query(
                &exe,
                &cwd,
                Platform::Linux,
                Some(&home),
                None,
                None,
                false
            )),
            home.join(".local").join("share").join("oxycharacter")
        );
        assert_eq!(
            default_user_dir(&query(
                &exe,
                &cwd,
                Platform::Macos,
                Some(&home),
                None,
                None,
                false
            )),
            home.join("Library")
                .join("Application Support")
                .join("OxyCharacter")
        );
        assert_eq!(
            default_user_dir(&query(&exe, &cwd, Platform::Linux, None, None, None, false)),
            PathBuf::from("user")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn source_checkout_wins_over_a_portable_marker() {
        let root = scratch("devportable");
        checkout(&root);
        let exe = root.join("target/release/oxycharacter");
        touch(&exe);
        fs::write(exe.parent().unwrap().join(PORTABLE_MARKER), b"").unwrap();
        let home = root.join("home");
        let dir = default_user_dir(&query(
            &exe,
            &root,
            Platform::Linux,
            Some(&home),
            None,
            None,
            true,
        ));
        assert_eq!(dir, root.join("user"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn env_helper_removes_its_write_probe() {
        let root = scratch("probe");
        let exe_dir = root.join("app");
        let exe = exe_dir.join("oxycharacter");
        touch(&exe);
        fs::write(exe_dir.join(PORTABLE_MARKER), b"").unwrap();
        let cwd = root.join("cwd");
        fs::create_dir_all(&cwd).unwrap();
        let dir = user_dir_from_env(&exe, &cwd);
        assert_eq!(dir, exe_dir.join("user"));
        let leftover = fs::read_dir(&exe_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".oxy-write-probe")
            });
        assert!(!leftover);
        let _ = fs::remove_dir_all(&root);
    }
}
