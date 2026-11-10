//! Starting plugin processes and discovering plugins.
//!
//! A plugin runs with the user's own rights, like any program they start; the
//! host only starts one when the user asks (menu, CLI). The child's stdout is
//! the protocol channel; its stderr is the plugin's log (the SDK redirects
//! `print` there).

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Mutex;

use crate::api::{dispatch, Host};
use crate::protocol::{parse_request, read_frame, response, write_frame, Request};

/// A plugin found on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginInfo {
    pub path: PathBuf,
    pub name: String,
    pub description: String,
}

/// Read `# Name:` / `# Description:` header lines (in comments or a
/// docstring) from the first 40 lines of a script.
pub fn read_info(path: &Path) -> Option<PluginInfo> {
    let src = std::fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut description = String::new();
    for line in src.lines().take(40) {
        let l = line.trim().trim_start_matches('#').trim();
        if let Some(v) = l.strip_prefix("Name:") {
            name = Some(v.trim().to_owned());
        } else if let Some(v) = l.strip_prefix("Description:") {
            description = v.trim().to_owned();
        }
    }
    let name = name.unwrap_or_else(|| {
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .replace('_', " ")
    });
    Some(PluginInfo {
        path: path.to_owned(),
        name,
        description,
    })
}

/// All `*.py` plugins in `dirs` (missing folders are skipped), sorted by name.
pub fn discover(dirs: &[PathBuf]) -> Vec<PluginInfo> {
    let mut out: Vec<PluginInfo> = dirs
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|e| e == "py")
                && !p
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('_'))
        })
        .filter_map(|p| read_info(&p))
        .collect();
    out.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.path.cmp(&b.path))
    });
    out
}

/// The Python interpreter: `$OXY_PYTHON`, else `python3` (`python` on Windows).
pub fn python_command() -> PathBuf {
    std::env::var_os("OXY_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "python".into()
            } else {
                "python3".into()
            }
        })
}

/// Folder containing the `oxycharacter` SDK package: `$OXY_PYTHON_SDK`, next
/// to the executable (`python/`, `../share/oxycharacter/python`, macOS
/// `../Resources/python`), or a checkout's `python/` found by walking up from
/// the executable and the working directory.
pub fn sdk_dir() -> Option<PathBuf> {
    let ok = |d: &Path| d.join("oxycharacter").join("__init__.py").is_file();
    if let Some(d) = std::env::var_os("OXY_PYTHON_SDK").map(PathBuf::from) {
        return ok(&d).then_some(d);
    }
    let mut starts = Vec::new();
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        for rel in [
            "python",
            "../share/oxycharacter/python",
            "../Resources/python",
        ] {
            let d = exe_dir.join(rel);
            if ok(&d) {
                return Some(d);
            }
        }
        starts.push(exe_dir);
    }
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    for s in starts {
        for anc in s.ancestors() {
            let d = anc.join("python");
            if ok(&d) {
                return Some(d);
            }
        }
    }
    None
}

/// Start `script` with the SDK on `PYTHONPATH`. `args` are passed to the script.
pub fn spawn(script: &Path, sdk: Option<&Path>, args: &[String]) -> std::io::Result<Child> {
    // The child runs in the script's folder, so pass an absolute path.
    let script = &std::path::absolute(script)?;
    let mut cmd = Command::new(python_command());
    cmd.arg("-u").arg(script).args(args);
    if let Some(sdk) = sdk {
        let mut paths = vec![sdk.to_path_buf()];
        if let Some(existing) = std::env::var_os("PYTHONPATH") {
            paths.extend(std::env::split_paths(&existing));
        }
        cmd.env(
            "PYTHONPATH",
            std::env::join_paths(paths).map_err(std::io::Error::other)?,
        );
    }
    if let Some(dir) = script.parent().filter(|d| !d.as_os_str().is_empty()) {
        cmd.current_dir(dir);
    }
    cmd.env("OXY_PLUGIN_API", crate::API_VERSION.to_string())
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

fn answer(host: &mut dyn Host, req: &Request, stdin: &mut ChildStdin) -> std::io::Result<()> {
    let result = dispatch(host, &req.method, &req.params);
    match &req.id {
        Some(id) => write_frame(stdin, &response(id, result)),
        None => Ok(()), // notification
    }
}

/// Outcome of a finished plugin run.
#[derive(Debug)]
pub struct Finished {
    pub status: ExitStatus,
    /// Requests answered.
    pub calls: usize,
}

/// Run a plugin to completion, answering its requests from `host`. Each line
/// the plugin writes to stderr is passed to `log`.
pub fn run_blocking(
    mut child: Child,
    host: &mut dyn Host,
    mut log: impl FnMut(&str),
) -> std::io::Result<Finished> {
    let stderr = child.stderr.take().expect("stderr piped");
    let (tx, rx) = channel();
    let t = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut stdin = child.stdin.take().expect("stdin piped");
    let mut out = BufReader::new(child.stdout.take().expect("stdout piped"));
    let mut calls = 0;
    while let Some(body) = read_frame(&mut out)? {
        while let Ok(line) = rx.try_recv() {
            log(&line);
        }
        match parse_request(&body) {
            Ok(req) => {
                answer(host, &req, &mut stdin)?;
                calls += 1;
            }
            Err(e) => log(&format!("[host] ignored a malformed message: {e}")),
        }
    }
    drop(stdin);
    let status = child.wait()?;
    let _ = t.join();
    for line in rx.try_iter() {
        log(&line);
    }
    Ok(Finished { status, calls })
}

/// A running plugin for hosts with a frame loop (the editor): reading happens
/// on background threads; [`Running::pump`] answers requests on the caller's
/// thread so the host can be borrowed mutably there.
pub struct Running {
    child: Child,
    stdin: Option<ChildStdin>,
    // Mutex only to make `Running` `Sync` (engine resources need it); the
    // receivers are used from one thread.
    requests: Mutex<Receiver<Result<Request, String>>>,
    logs: Mutex<Receiver<String>>,
    pub calls: usize,
}

impl Running {
    pub fn start(mut child: Child) -> Self {
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");
        let (rtx, requests) = channel();
        let (ltx, logs) = channel();
        std::thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            while let Ok(Some(body)) = read_frame(&mut r) {
                if rtx.send(parse_request(&body)).is_err() {
                    break;
                }
            }
        });
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if ltx.send(line).is_err() {
                    break;
                }
            }
        });
        let stdin = child.stdin.take();
        Self {
            child,
            stdin,
            requests: Mutex::new(requests),
            logs: Mutex::new(logs),
            calls: 0,
        }
    }

    /// Answer up to `max` pending requests; returns new log lines, and the
    /// exit status once the plugin has finished and everything was answered.
    pub fn pump(&mut self, host: &mut dyn Host, max: usize) -> (Vec<String>, Option<ExitStatus>) {
        let logs = self.logs.get_mut().unwrap_or_else(|e| e.into_inner());
        let mut log: Vec<String> = logs.try_iter().collect();
        let requests = self.requests.get_mut().unwrap_or_else(|e| e.into_inner());
        for _ in 0..max {
            match requests.try_recv() {
                Ok(Ok(req)) => {
                    if let Some(stdin) = self.stdin.as_mut() {
                        if answer(host, &req, stdin).is_err() {
                            self.stdin = None; // plugin went away mid-reply
                        }
                    }
                    self.calls += 1;
                }
                Ok(Err(e)) => log.push(format!("[host] ignored a malformed message: {e}")),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.stdin = None;
                    // stdout closed: the exit usually follows at once; if not,
                    // the next pump checks again (never block the caller).
                    let status = self.child.try_wait().ok().flatten();
                    log.extend(
                        self.logs
                            .get_mut()
                            .unwrap_or_else(|e| e.into_inner())
                            .try_iter(),
                    );
                    return (log, status);
                }
            }
        }
        (log, None)
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Does `python_command()` start? (For menus.)
pub fn python_available() -> bool {
    Command::new(python_command())
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_headers_and_skips_private_files() {
        let dir = std::env::temp_dir().join(format!("oxy-plugins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("b_tool.py"),
            "\"\"\"\n# Name: Beta tool\n# Description: Does beta things\n\"\"\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("alpha.py"), "# Description: no name line\n").unwrap();
        std::fs::write(dir.join("_helper.py"), "").unwrap();
        std::fs::write(dir.join("notes.txt"), "").unwrap();
        let found = discover(&[dir.clone(), dir.join("missing")]);
        let names: Vec<_> = found.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "Beta tool"]);
        assert_eq!(found[1].description, "Does beta things");
        std::fs::remove_dir_all(dir).ok();
    }
}
