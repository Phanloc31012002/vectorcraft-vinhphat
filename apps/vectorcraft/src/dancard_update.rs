//! Automatic updates of the DanCard fork (IN Vĩnh Phát) on Windows, from the fork's GitHub
//! Releases: every machine follows the newest build without anyone copying files around.
//!
//! - Only builds made by the fork's "DanCard Windows x64" workflow carry a build number
//!   (`DANCARD_BUILD`: the workflow's run number, set at compile time), and only those, installed
//!   per user in `%LOCALAPPDATA%\CongCuBinh\app` (`packaging/dancard/CaiDat-CongCuBinh.cmd`), update
//!   themselves: no administrator rights are needed to replace their files.
//! - In the background, half a minute after the start and then every hour, the app reads the
//!   newest release's `latest.json` (build, zip address, SHA-256). A newer build is downloaded with
//!   `curl.exe`, checked against its SHA-256 and unpacked with `tar.exe` (both part of Windows 10 and
//!   later); it must start on this machine and be that build (`--version`); its files are written
//!   through to the disk, and only then is it marked ready in `CongCuBinh\update\next`. The status
//!   bar and the DanCard panel say so. Nothing is asked: the work in progress goes on.
//! - The next start applies it, first thing and only when no other window of the app runs. The list
//!   of the files about to move is written to the disk first (`swap.json`); then each file the
//!   update replaces is renamed aside (`.old`: the running exe can be renamed, not overwritten) and
//!   the new one moved in, the exe last, and the new build starts with the same files to open. A
//!   swap that fails is undone at once, one cut short (a power cut) at the next start, from the list.
//! - The new build is on trial until its window has drawn a frame: the files it replaced wait beside
//!   it. One that doesn't draw its window in [`TRIAL_STARTS`] starts (it crashes on start) is rolled
//!   back: the old files come back, its own go aside (`.bad`, deleted later), and it isn't downloaded
//!   again (the next one is). A roll-back cut short goes on at the next start. A build that can't
//!   start can't download its fix: without this, every machine would need the installer again. A
//!   new build that can't even be started (an antivirus took it) is undone at once.
//! - The windows of the app take turns in `update` (`update.lock`). `update\update.log` keeps what
//!   the updater did: the app's own log isn't open yet when an update applies.
//!
//! Nothing here panics: a failure only means no update this time (logged).

use std::ffi::OsString;
use std::io::{Read as _, Write as _};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

/// The newest release's description of its build.
const LATEST: &str = "https://github.com/Phanloc31012002/vectorcraft-vinhphat/releases/latest/download/latest.json";
/// Builds are downloaded from the fork's releases only.
const DOWNLOADS: &str = "https://github.com/Phanloc31012002/vectorcraft-vinhphat/releases/download/";
/// The per-user install folder in `%LOCALAPPDATA%`.
const ROOT: &str = "CongCuBinh";
/// The installed app, in the install folder.
const APP: &str = "app";
/// Downloads, the update ready to apply and the marks below, in the install folder.
const UPDATE: &str = "update";
/// The files of the update ready to apply, in `update`.
const NEXT: &str = "next";
/// Where a downloaded zip is unpacked (`unpack-<process>`), in `update`.
const UNPACK: &str = "unpack";
/// `{"build": n}`: the build in `next`, written once it is complete and checked.
const READY: &str = "ready.json";
/// The swap under way ([`Plan`]): written to the disk before the first file moves, removed once the
/// trial mark holds the same list. Found at a start, it is a swap cut short, and is undone.
const SWAP: &str = "swap.json";
/// `{plan…, "starts": k, "rollingBack": b}` ([`Trial`]): a new build, swapped in by its plan, was
/// started `k` times without showing its window yet; the files it replaced wait beside it until it
/// does. `rollingBack`: its roll-back began (and goes on at the next start, if it was cut short).
const TRIAL: &str = "trial.json";
/// `{"build": n, "told": b}`: build `n` didn't start and was rolled back (`told`: the status bar
/// said so); it isn't downloaded again.
const BAD: &str = "bad.json";
/// `{"build": n, "count": k, "at": seconds}`: what follows the download of build `n` (its checks,
/// unpacking, a first start) failed `k` times, the last at `at`.
const FAILURES: &str = "failures.json";
/// Failures after which a build waits [`BACK_OFF`] seconds before it is downloaded again.
const MAX_FAILURES: u64 = 3;
const BACK_OFF: u64 = 24 * 3600;
/// The lock the windows of the app take turns with in `update`.
const LOCK: &str = "update.lock";
/// The updater's own log, in `update`, kept under [`MAX_LOG`] bytes (the one before it: `.1`).
const LOG: &str = "update.log";
const MAX_LOG: u64 = 256 * 1024;
/// The installed build, in `app` (for people and the installer).
const BUILD_FILE: &str = "build.txt";
const EXE: &str = if cfg!(windows) { "vectorcraft.exe" } else { "vectorcraft" };
/// Extension of a file an update replaced (`name.old`), kept for a roll-back.
const OLD: &str = "old";
/// Extension of a file of a build that went (`name.bad`; `name.bad1`… when an earlier one is
/// still there).
const FAILED: &str = "bad";
/// Starts a new build gets to show its window before the build before it comes back.
const TRIAL_STARTS: u64 = 2;
/// Set by `gpu::restart` (`gpu::SKIP_ENV`) on the start it makes after the graphics failed: the
/// same run going on, not one more start of a build on trial.
const GPU_SKIP: &str = "VECTORCRAFT_GPU_SKIP";
/// The first check, after the start.
const FIRST_CHECK: Duration = Duration::from_secs(30);
/// The checks after that (`latest.json` is a few hundred bytes).
const EVERY: Duration = Duration::from_secs(3600);
/// How long a downloaded build may take to answer `--version`.
const VERSION_TIMEOUT: Duration = Duration::from_secs(60);
/// Largest `latest.json` read, and largest state file (bytes).
const MAX_JSON: u64 = 64 * 1024;
const MAX_STATE: u64 = 4 * 1024 * 1024;
/// Most files an update holds, and how deep its folders go.
const MAX_FILES: usize = 10_000;
const MAX_DEPTH: usize = 8;
/// Windows: start a tool without a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The message for the status bar, taken by the window.
static NOTICE: Mutex<Option<String>> = Mutex::new(None);

/// This build's number, when the fork's workflow made it (local builds have none and never update).
pub fn build() -> Option<u64> {
    option_env!("DANCARD_BUILD").and_then(|b| b.trim().parse().ok()).filter(|b| *b > 0)
}

/// The per-user install folder, `%LOCALAPPDATA%\CongCuBinh`.
fn install_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join(ROOT))
}

/// This run is the per-user install in `root` (an MSI install or a copy elsewhere doesn't update).
fn is_managed(root: &Path) -> bool {
    let (Ok(exe), Ok(app)) = (std::env::current_exe().and_then(|e| e.canonicalize()), root.join(APP).canonicalize()) else { return false };
    // Windows paths compare without case.
    exe.parent().is_some_and(|dir| dir.to_string_lossy().to_lowercase() == app.to_string_lossy().to_lowercase())
}

/// Updates apply to this run: a numbered build, on Windows, installed per user. The install
/// folder and this build's number.
fn updating() -> Option<(PathBuf, u64)> {
    let (current, root) = (build()?, install_dir()?);
    (cfg!(windows) && is_managed(&root)).then_some((root, current))
}

fn set_notice(text: String) {
    *NOTICE.lock().unwrap_or_else(PoisonError::into_inner) = Some(text);
}

/// Each frame of the window (`ctx`): the second one ends this build's trial (its window was drawn,
/// so the files it replaced can go) → the status bar's message about updates, once.
pub fn on_frame(ctx: &egui::Context) -> Option<String> {
    static FRAMES: AtomicU8 = AtomicU8::new(0);
    match FRAMES.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| (n < 2).then_some(n + 1)) {
        // The first frame: a second one comes even when nothing moves.
        Ok(0) => ctx.request_repaint(),
        Ok(_) => {
            if let Some((root, current)) = updating() {
                let ctx = ctx.clone();
                // Off the window's thread: it waits for another window's check to let `update` go.
                let spawned = std::thread::Builder::new().name("dancard-trial".into()).spawn(move || {
                    let update = root.join(UPDATE);
                    if with_lock_wait(&update, || end_trial(&update, current)) == Some(true) {
                        note(&update, &format!("build {current} starts: its trial is over"));
                        set_notice(format!("DanCard: đã lên bản {current}."));
                        ctx.request_repaint();
                    }
                });
                if let Err(e) = spawned {
                    // The next start counts one more start of the trial.
                    log::warn!("DanCard update: trial of build {current} not ended ({e})");
                }
            }
        }
        Err(_) => {}
    }
    NOTICE.lock().unwrap_or_else(PoisonError::into_inner).take()
}

/// A newer build in the newest release.
#[derive(Debug, PartialEq)]
struct Release {
    build: u64,
    url: String,
    sha256: String,
}

/// `latest.json`: the build, its zip (a download of the fork's releases) and the zip's SHA-256.
fn parse_release(text: &str) -> Result<Release, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("latest.json: {e}"))?;
    let build = number(&v, "build").ok_or("latest.json has no build")?;
    let url = v.get("url").and_then(Value::as_str).ok_or("latest.json has no url")?;
    if !url.starts_with(DOWNLOADS) || url.contains("..") || url.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"') {
        return Err(format!("latest.json names a file outside the fork's releases: {url}"));
    }
    let sha256 = v
        .get("sha256")
        .and_then(Value::as_str)
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("latest.json has no SHA-256")?
        .to_ascii_lowercase();
    Ok(Release { build, url: url.to_string(), sha256 })
}

/// A positive number in `value`'s field `key`.
fn number(value: &Value, key: &str) -> Option<u64> {
    value.get(key)?.as_u64().filter(|n| *n > 0)
}

/// The JSON in the (small) file at `path`.
fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&read_capped(path, MAX_STATE).ok()?).ok()
}

/// The state file at `path`, read by `parse`: `Ok(None)` when there is none, an error when there is
/// one that can't be read (left as it is: what it says may be needed).
fn load<T>(path: &Path, parse: impl FnOnce(&Value) -> Option<T>) -> Result<Option<T>, String> {
    if std::fs::symlink_metadata(path).is_err() {
        return Ok(None);
    }
    let v: Value = serde_json::from_str(&read_capped(path, MAX_STATE)?).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&v).map(Some).ok_or_else(|| format!("{} isn't understood", path.display()))
}

/// Write `value` to `path` whole and through to the disk (by a temporary file of this process: a
/// half-written file is never read).
fn write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    let tmp = aside(path, &format!("{}.tmp", std::process::id()));
    let written = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(value.to_string().as_bytes())?;
        file.sync_all()
    })()
    .and_then(|()| retry(|| std::fs::rename(&tmp, path)));
    if written.is_err() {
        // Best effort: a leftover is cleared before the next download.
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

/// The build ready to apply in `update`, if one is.
fn ready_build(update: &Path) -> Option<u64> {
    number(&read_json(&update.join(READY))?, "build")
}

/// Mark `build` ready in `update`.
fn write_ready(update: &Path, build: u64) -> std::io::Result<()> {
    write_json(&update.join(READY), &json!({ "build": build }))
}

/// What a swap moves: build `build`'s files (relative to `app`, the exe last), each with whether
/// `app` had a file of that name (set aside as `.old` then), in place of build `previous`.
#[derive(Clone, Debug, PartialEq)]
struct Plan {
    build: u64,
    previous: Option<u64>,
    files: Vec<(PathBuf, bool)>,
}

impl Plan {
    fn to_json(&self) -> Value {
        let files: Vec<Value> = self.files.iter().map(|(path, existed)| json!({ "path": path.to_string_lossy(), "existed": existed })).collect();
        json!({ "build": self.build, "previous": self.previous, "files": files })
    }

    /// A plan read back: every path relative and inside the app's folder, at most [`MAX_FILES`].
    fn from_json(v: &Value) -> Option<Plan> {
        let list = v.get("files")?.as_array()?;
        if list.len() > MAX_FILES {
            return None;
        }
        let mut files = Vec::with_capacity(list.len());
        for f in list {
            let path = PathBuf::from(f.get("path")?.as_str()?);
            if path.as_os_str().is_empty() || !path.components().all(|c| matches!(c, Component::Normal(_))) {
                return None;
            }
            files.push((path, f.get("existed")?.as_bool()?));
        }
        Some(Plan { build: number(v, "build")?, previous: number(v, "previous"), files })
    }
}

/// A new build on trial.
#[derive(Clone, Debug, PartialEq)]
struct Trial {
    /// How it was swapped in.
    plan: Plan,
    /// Starts without showing its window.
    starts: u64,
    /// Its roll-back began.
    rolling_back: bool,
}

impl Trial {
    fn write(&self, update: &Path) -> std::io::Result<()> {
        let mut v = self.plan.to_json();
        if let Some(fields) = v.as_object_mut() {
            fields.insert("starts".into(), json!(self.starts));
            fields.insert("rollingBack".into(), json!(self.rolling_back));
        }
        write_json(&update.join(TRIAL), &v)
    }

    fn from_json(v: &Value) -> Option<Trial> {
        Some(Trial {
            plan: Plan::from_json(v)?,
            starts: v.get("starts").and_then(Value::as_u64).unwrap_or(0),
            rolling_back: v.get("rollingBack").and_then(Value::as_bool).unwrap_or(false),
        })
    }
}

/// The build on trial in `update` (none when its mark can't be read).
fn trial(update: &Path) -> Option<Trial> {
    load(&update.join(TRIAL), Trial::from_json).ok().flatten()
}

/// The build rolled back in `update`, and whether the status bar said so.
fn bad_build(update: &Path) -> Option<(u64, bool)> {
    let v = read_json(&update.join(BAD))?;
    Some((number(&v, "build")?, v.get("told").and_then(Value::as_bool).unwrap_or(false)))
}

/// Forget the update in `update` (the mark first: files without it are never applied).
fn discard(update: &Path) {
    // Best effort: what is left is cleared before the next download.
    let _ = std::fs::remove_file(update.join(READY));
    let _ = std::fs::remove_dir_all(update.join(NEXT));
}

/// `build` is worth downloading for `current`: newer, not there already, not rolled back.
fn wanted(update: &Path, current: u64, build: u64) -> bool {
    build > current && ready_build(update) != Some(build) && bad_build(update).is_none_or(|(bad, _)| bad != build)
}

/// Seconds since 1970.
fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// What follows the download of `build` failed [`MAX_FAILURES`] times, the last less than
/// [`BACK_OFF`] ago: it isn't downloaded again yet.
fn backing_off(update: &Path, build: u64) -> bool {
    let Some(v) = read_json(&update.join(FAILURES)) else { return false };
    let count = v.get("count").and_then(Value::as_u64).unwrap_or(0);
    let at = v.get("at").and_then(Value::as_u64).unwrap_or(0);
    number(&v, "build") == Some(build) && count >= MAX_FAILURES && now_secs().saturating_sub(at) < BACK_OFF
}

/// One more failure of what follows the download of `build` (after a back-off, it gets
/// [`MAX_FAILURES`] tries again).
fn count_failure(update: &Path, build: u64) {
    let before = read_json(&update.join(FAILURES))
        .filter(|v| number(v, "build") == Some(build))
        .and_then(|v| v.get("count").and_then(Value::as_u64))
        .unwrap_or(0);
    let count = if before >= MAX_FAILURES { 1 } else { before + 1 };
    // Best effort: without it, the build is only downloaded again sooner.
    let _ = write_json(&update.join(FAILURES), &json!({ "build": build, "count": count, "at": now_secs() }));
}

/// Start `command` without a console window.
fn no_window(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(command, CREATE_NO_WINDOW);
    command
}

/// One of Windows' own tools (`curl`, `tar`), from System32 rather than whatever `PATH` finds,
/// started without a console window.
fn tool(name: &str) -> Command {
    let exe = if cfg!(windows) {
        let system = std::env::var_os("SystemRoot").map(|r| PathBuf::from(r).join("System32").join(format!("{name}.exe")));
        system.filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from(format!("{name}.exe")))
    } else {
        PathBuf::from(name)
    };
    let mut command = Command::new(exe);
    no_window(&mut command);
    command
}

/// Download `url` into `dir` as `name` (through a `.part` file of this process, so the file is
/// never half there) → its path. curl runs in `dir` and gets bare file names: a user folder with
/// letters outside the system's code page (`C:\Users\Người dùng`) doesn't reach a tool that reads
/// its arguments in that code page.
fn download(url: &str, dir: &Path, name: &str) -> Result<PathBuf, String> {
    let part = format!("{name}.{}.part", std::process::id());
    let curl = |extra: &[&str]| {
        tool("curl")
            .current_dir(dir)
            .args(["-fsSL", "--retry", "2", "--connect-timeout", "20", "--max-time", "1800"])
            .args(extra)
            .args(["-o", part.as_str(), url])
            .status()
            .map_err(|e| format!("curl: {e}"))
    };
    // A revocation check that can't be made (an antivirus that inspects HTTPS, an offline
    // revocation server) doesn't fail the download; a curl too old for the option (exit 2: unknown
    // option) goes without it.
    let mut status = curl(&["--ssl-revoke-best-effort"])?;
    if status.code() == Some(2) {
        status = curl(&[])?;
    }
    let (part, out) = (dir.join(&part), dir.join(name));
    if !status.success() {
        // Best effort: the next download writes it again.
        let _ = std::fs::remove_file(&part);
        return Err(format!("curl {url}: {status}"));
    }
    retry(|| std::fs::rename(&part, &out)).map_err(|e| e.to_string())?;
    Ok(out)
}

/// The contents of `path`, at most `cap` bytes.
fn read_capped(path: &Path, cap: u64) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut text = String::new();
    file.take(cap + 1).read_to_string(&mut text).map_err(|e| e.to_string())?;
    if text.len() as u64 > cap {
        return Err(format!("{} is larger than {cap} bytes", path.display()));
    }
    Ok(text)
}

/// The SHA-256 of the file at `path`, in lowercase hex.
fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        let Some(chunk) = buf.get(..n).filter(|c| !c.is_empty()) else { break };
        hash.update(chunk);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The folder holding the app's exe in `unpacked`: itself, or a folder in it (the zip keeps the
/// files in a folder named after the version).
fn find_app_dir(unpacked: &Path) -> Option<PathBuf> {
    if unpacked.join(EXE).is_file() {
        return Some(unpacked.to_path_buf());
    }
    std::fs::read_dir(unpacked).ok()?.flatten().map(|e| e.path()).find(|p| p.is_dir() && p.join(EXE).is_file())
}

/// The exe at `exe` starts on this machine and is build `build`: `exe --version` ends in time,
/// saying so (an antivirus that took the file, a Windows without what the build needs, a damaged
/// unpacking: found here rather than at the start that would apply it).
fn runs_as(exe: &Path, build: u64) -> Result<(), String> {
    let mut command = Command::new(exe);
    no_window(&mut command).arg("--version").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = command.spawn().map_err(|e| format!("build {build} doesn't start ({e})"))?;
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < VERSION_TIMEOUT => std::thread::sleep(Duration::from_millis(100)),
            Ok(None) => {
                // Best effort: a build that hangs on `--version` isn't used either way.
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("build {build} doesn't answer --version"));
            }
            Err(e) => return Err(format!("build {build}: {e}")),
        }
    };
    let mut text = String::new();
    if let Some(out) = child.stdout.take() {
        // Best effort: what couldn't be read fails the check below.
        let _ = out.take(4096).read_to_string(&mut text);
    }
    let text = text.trim();
    if !status.success() || !text.ends_with(&format!("(DanCard build {build})")) {
        return Err(format!("build {build} --version: {status}, \"{text}\""));
    }
    Ok(())
}

/// Write the files under `dir` through to the disk.
fn sync_files(dir: &Path) -> std::io::Result<()> {
    let mut files = vec![];
    files_under(dir, Path::new(""), &mut files, 0)?;
    for rel in files {
        let path = dir.join(rel);
        retry(|| std::fs::OpenOptions::new().write(true).open(&path)?.sync_all())?;
    }
    Ok(())
}

/// What earlier downloads left in `update` (other builds' zips, unpacked folders, partial and
/// temporary files) goes, but the zip `keep`.
fn remove_leftovers(update: &Path, keep: &str) {
    let Ok(entries) = std::fs::read_dir(update) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let leftover = name.starts_with(UNPACK) || name.ends_with(".part") || name.ends_with(".tmp") || (name.ends_with(".zip") && name != keep);
        if leftover {
            // Best effort: one still in use (a curl or tar of a window that ended) goes next time.
            let _ = if entry.path().is_dir() { std::fs::remove_dir_all(entry.path()) } else { std::fs::remove_file(entry.path()) };
        }
    }
}

/// Download build `release` (or take the zip a failed try left, when it checks out), unpack it,
/// check it starts and is that build, write it to the disk and mark it ready in `update\next`. An
/// error, and whether it counts towards the back-off ([`count_failure`], [`MAX_FAILURES`]): it does once
/// the whole zip came (a damaged zip, one that doesn't unpack or start), not when the network fails.
fn fetch(update: &Path, release: &Release) -> Result<(), (bool, String)> {
    let zip_name = format!("{}.zip", release.build);
    let zip = update.join(&zip_name);
    remove_leftovers(update, &zip_name);
    if !(zip.is_file() && sha256_file(&zip).is_ok_and(|s| s == release.sha256)) {
        download(&release.url, update, &zip_name).map_err(|e| (false, e))?;
        let sum = sha256_file(&zip).map_err(|e| (false, e))?;
        if sum != release.sha256 {
            // Best effort: a damaged download is only left out.
            let _ = std::fs::remove_file(&zip);
            return Err((true, format!("build {}: SHA-256 {sum} instead of {}", release.build, release.sha256)));
        }
    }
    let unpack = format!("{UNPACK}-{}", std::process::id());
    let unpacked = update.join(&unpack);
    let checked = (|| {
        std::fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
        // In `update`, with bare names, as curl above.
        let status =
            tool("tar").current_dir(update).args(["-xf", zip_name.as_str(), "-C", unpack.as_str()]).status().map_err(|e| format!("tar: {e}"))?;
        if !status.success() {
            return Err(format!("tar: {status}"));
        }
        let found = find_app_dir(&unpacked).ok_or_else(|| format!("build {}: no {EXE} in its zip", release.build))?;
        // On the disk before it is marked ready: a power cut must not leave a build of empty files.
        sync_files(&found).map_err(|e| format!("build {}: {e}", release.build))?;
        runs_as(&found.join(EXE), release.build)?;
        discard(update);
        retry(|| std::fs::rename(&found, update.join(NEXT))).map_err(|e| e.to_string())?;
        write_ready(update, release.build).map_err(|e| e.to_string())
    })();
    // Best effort: what is left of the unpacking (also cleared before the next download).
    let _ = std::fs::remove_dir_all(&unpacked);
    if checked.is_ok() {
        // Best effort: done with it. Kept after a failure, for the next try.
        let _ = std::fs::remove_file(&zip);
    }
    checked.map_err(|e| (true, e))
}

/// Download the newest release into `root\update\next` when [`wanted`] → the build made ready, if
/// one was. Waits its turn with the other windows' checks (`update.lock`).
fn check_and_download(root: &Path, current: u64) -> Result<Option<u64>, String> {
    let update = root.join(UPDATE);
    std::fs::create_dir_all(&update).map_err(|e| e.to_string())?;
    with_lock_wait(&update, || {
        let latest = download(LATEST, &update, "latest.json")?;
        let release = parse_release(&read_capped(&latest, MAX_JSON)?)?;
        if !wanted(&update, current, release.build) || backing_off(&update, release.build) {
            return Ok(None);
        }
        match fetch(&update, &release) {
            Ok(()) => {
                // Best effort: an old count only delays a later try of this build.
                let _ = std::fs::remove_file(update.join(FAILURES));
                Ok(Some(release.build))
            }
            Err((counts, e)) => {
                if counts {
                    count_failure(&update, release.build);
                }
                Err(e)
            }
        }
    })
    .unwrap_or_else(|| Err(format!("{} can't be locked", update.join(LOCK).display())))
}

/// Check for a newer build in the background, half a minute after the start and then every hour,
/// and download it; `ctx` repaints for the status bar's message. Nothing for builds that don't
/// update.
pub fn start_checker(ctx: &egui::Context) {
    let Some((root, current)) = updating() else { return };
    let ctx = ctx.clone();
    let spawned = std::thread::Builder::new().name("dancard-update".into()).spawn(move || {
        std::thread::sleep(FIRST_CHECK);
        loop {
            match check_and_download(&root, current) {
                Ok(Some(build)) => {
                    note(&root.join(UPDATE), &format!("build {build} downloaded, used from the next start"));
                    set_notice(format!("DanCard: đã tải bản mới (bản {build}): đóng rồi mở lại app là dùng bản mới."));
                    vectorcraft_ui_egui::panels::dancard::set_update_ready(build);
                    ctx.request_repaint();
                }
                Ok(None) => {}
                // No network, GitHub unreachable…: try again later.
                Err(e) => note(&root.join(UPDATE), &e),
            }
            std::thread::sleep(EVERY);
        }
    });
    if let Err(e) = spawned {
        log::warn!("DanCard update: no checks ({e})");
    }
}

/// `path.ext`: where a file is set aside.
fn aside(path: &Path, ext: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".");
    s.push(ext);
    PathBuf::from(s)
}

/// `f`, tried again a few times over three seconds when it fails (an antivirus or the indexer may
/// hold a file a moment). Once in tests.
fn retry<T>(mut f: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    let mut wait = 50;
    loop {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) if cfg!(test) || wait > 1600 => return Err(e),
            Err(_) => {
                std::thread::sleep(Duration::from_millis(wait));
                wait *= 2;
            }
        }
    }
}

/// The lock file in `update`.
fn open_lock(update: &Path) -> std::io::Result<std::fs::File> {
    std::fs::create_dir_all(update)?;
    std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(update.join(LOCK))
}

/// Run `f` holding `update`'s lock, if no other window of the app holds it now.
fn with_lock<R>(update: &Path, f: impl FnOnce() -> R) -> Option<R> {
    let lock = open_lock(update).ok()?;
    lock.try_lock().ok()?;
    let out = f();
    drop(lock);
    Some(out)
}

/// Run `f` holding `update`'s lock, once the window of the app that holds it lets it go (`None`:
/// it can't be locked).
fn with_lock_wait<R>(update: &Path, f: impl FnOnce() -> R) -> Option<R> {
    let lock = open_lock(update).ok()?;
    lock.lock().ok()?;
    let out = f();
    drop(lock);
    Some(out)
}

/// `text` in the app's log and in the updater's own log in `update` (best effort: a log that can't
/// be written isn't a reason to stop).
fn note(update: &Path, text: &str) {
    log::info!("DanCard update: {text}");
    let path = update.join(LOG);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_LOG) {
        let _ = std::fs::rename(&path, aside(&path, "1"));
    }
    let line = format!("{} [{}] {text}\n", utc(now_secs()), std::process::id());
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = file.write_all(line.as_bytes());
    }
}

/// `secs` since 1970 as a UTC date and time (`2026-10-16 04:05:06Z`).
fn utc(secs: u64) -> String {
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil(i64::try_from(days).unwrap_or(0));
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

/// The date `days` after 1970-01-01 (the proleptic Gregorian calendar; Howard Hinnant's
/// `civil_from_days`).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// The files under `dir`, relative to it (at most [`MAX_FILES`], [`MAX_DEPTH`] folders deep).
fn files_under(dir: &Path, rel: &Path, out: &mut Vec<PathBuf>, depth: usize) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir.join(rel))? {
        let entry = entry?;
        let name = rel.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            if depth < MAX_DEPTH {
                files_under(dir, &name, out, depth + 1)?;
            }
        } else if out.len() < MAX_FILES {
            out.push(name);
        } else {
            return Err(std::io::Error::other("the update has too many files"));
        }
    }
    Ok(())
}

/// A file that was set aside: `.old`, `.bad`, `.bad1`….
fn is_aside(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e == OLD || e.strip_prefix(FAILED).is_some_and(|k| k.bytes().all(|b| b.is_ascii_digit())))
}

/// The plan of the swap of the files of `next` (build `build`) into `app` (build `previous`): the
/// exe last, so that the build in place starts (and undoes the rest) until the very last move.
fn plan_swap(next: &Path, app: &Path, build: u64, previous: u64) -> std::io::Result<Plan> {
    let mut files = vec![];
    files_under(next, Path::new(""), &mut files, 0)?;
    if files.iter().any(|f| f.to_str().is_none()) {
        return Err(std::io::Error::other("a file name in the update isn't Unicode"));
    }
    files.sort_by_key(|f| f.as_path() == Path::new(EXE));
    let files = files.into_iter().map(|f| (app.join(&f), f)).map(|(target, f)| (f, std::fs::symlink_metadata(target).is_ok())).collect();
    Ok(Plan { build, previous: Some(previous), files })
}

/// The `.old` files an earlier update left beside the files `plan` replaces go first: one would be
/// taken for the copy this swap sets aside.
fn clear_old(app: &Path, plan: &Plan) -> std::io::Result<()> {
    for (rel, existed) in &plan.files {
        let old = aside(&app.join(rel), OLD);
        if *existed && std::fs::symlink_metadata(&old).is_ok() {
            retry(|| std::fs::remove_file(&old))?;
        }
    }
    Ok(())
}

/// Move the files of `next` into `app` as `plan` lists them: each file it replaces is renamed aside
/// first (the running exe can be renamed, not overwritten). Stops at the first failure ([`undo`]
/// puts things back).
fn swap(next: &Path, app: &Path, plan: &Plan) -> std::io::Result<()> {
    for (rel, existed) in &plan.files {
        let (from, to) = (next.join(rel), app.join(rel));
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir)?;
        }
        if *existed {
            retry(|| std::fs::rename(&to, aside(&to, OLD)))?;
        }
        retry(|| std::fs::rename(&from, &to))?;
    }
    Ok(())
}

/// Set `file` (of a build that goes) aside as `.bad` (`.bad1`… when an earlier one can't go) → true
/// when it is out of the way. A running exe can be renamed.
fn set_failed(file: &Path) -> bool {
    for k in 0..5 {
        let bad = aside(file, &if k == 0 { FAILED.to_string() } else { format!("{FAILED}{k}") });
        // A leftover of an earlier update; if it can't go, the next name.
        let _ = std::fs::remove_file(&bad);
        if std::fs::symlink_metadata(&bad).is_err() && retry(|| std::fs::rename(file, &bad)).is_ok() {
            return true;
        }
    }
    false
}

/// Put `app` back as it was before `plan`'s swap, as far as it went: each file it replaced back
/// from `.old`, each new one set aside as `.bad` → true when every file is back. Doing it again
/// changes nothing.
fn undo(app: &Path, plan: &Plan) -> bool {
    let mut back = true;
    for (rel, existed) in plan.files.iter().rev() {
        let file = app.join(rel);
        let present = std::fs::symlink_metadata(&file).is_ok();
        if *existed {
            let old = aside(&file, OLD);
            // No `.old`: never moved, or back already. Else the new one (if it came) aside, then the
            // old one back.
            if std::fs::symlink_metadata(&old).is_ok() {
                back &= (!present || set_failed(&file)) && retry(|| std::fs::rename(&old, &file)).is_ok();
            }
        } else if present {
            back &= set_failed(&file);
        }
    }
    back
}

/// Delete the files earlier updates and roll-backs set aside in `app` (those still running stay
/// till a later start).
fn clean_aside(app: &Path) {
    let mut files = vec![];
    // Best effort: what can't be listed stays as it is.
    let _ = files_under(app, Path::new(""), &mut files, 0);
    for rel in files.into_iter().filter(|f| is_aside(f)) {
        // Still running (another window of that build): deleted at a later start.
        let _ = std::fs::remove_file(app.join(rel));
    }
}

/// The window of build `current` shows: its trial in `update` (if any) is over, and the files it
/// replaced go at the next start → true when a trial ended.
fn end_trial(update: &Path, current: u64) -> bool {
    if trial(update).is_none_or(|t| t.plan.build != current || t.rolling_back) {
        return false;
    }
    match std::fs::remove_file(update.join(TRIAL)) {
        Ok(()) => true,
        Err(e) => {
            // The next start counts one more start of the trial.
            note(update, &format!("trial of build {current} not ended ({e})"));
            false
        }
    }
}

/// What a start does after [`at_start`].
#[derive(Debug, PartialEq)]
enum Start {
    /// This run goes on.
    Go,
    /// The app's exe (another build now) starts in place of this run. `Some`: the plan of the new
    /// build just applied, undone if it can't be started.
    Restart(Option<Plan>),
}

/// A swap of `plan` was cut short (found at a start): it is undone → this run goes on, or (it is
/// the build that was coming in, its exe moved already) the build back in place starts instead.
fn undo_cut_swap(app: &Path, update: &Path, plan: &Plan, current: u64) -> Start {
    if !undo(app, plan) {
        note(update, &format!("the update to build {} was cut short and isn't undone yet: the rest at the next start", plan.build));
        return Start::Go;
    }
    // A trial mark written just before the cut is of this swap: over too.
    if trial(update).is_some_and(|t| t.plan.build == plan.build) {
        let _ = std::fs::remove_file(update.join(TRIAL));
    }
    if let Err(e) = std::fs::remove_file(update.join(SWAP)) {
        // Undone again (nothing left to do) at the next start.
        note(update, &format!("{SWAP} not removed ({e})"));
    }
    // `next` gave files to the swap: downloaded again.
    discard(update);
    note(update, &format!("the update to build {} was cut short: undone", plan.build));
    if current == plan.build { Start::Restart(None) } else { Start::Go }
}

/// The build on trial `t` didn't show its window in [`TRIAL_STARTS`] starts (or its roll-back was
/// cut short): the files it replaced come back (its own go aside as `.bad`), and it isn't
/// downloaded again → this run goes on, or (it is that build) the build back in place starts
/// instead.
fn roll_back(app: &Path, update: &Path, mut t: Trial, current: u64) -> Start {
    let build = t.plan.build;
    if !t.rolling_back {
        if !aside(&app.join(EXE), OLD).is_file() {
            note(update, &format!("build {build} doesn't start, and nothing is left to go back to"));
            let _ = std::fs::remove_file(update.join(TRIAL));
            return Start::Go;
        }
        // Marked before anything moves: a roll-back cut short goes on at the next start, and the
        // build isn't downloaded again whatever comes of it.
        t.rolling_back = true;
        if let Err(e) = t.write(update) {
            note(update, &format!("roll-back of build {build} not marked ({e})"));
        }
        if let Err(e) = write_json(&update.join(BAD), &json!({ "build": build, "told": false })) {
            note(update, &format!("build {build} not marked bad ({e})"));
        }
    }
    if !undo(app, &t.plan) {
        note(update, &format!("build {build} doesn't start, and the build before it isn't all back yet: the rest at the next start"));
        return Start::Go;
    }
    // Best effort: only for people and the installer to read.
    let _ = match t.plan.previous {
        Some(p) => std::fs::write(app.join(BUILD_FILE), p.to_string()),
        None => std::fs::remove_file(app.join(BUILD_FILE)),
    };
    if let Err(e) = std::fs::remove_file(update.join(TRIAL)) {
        // Rolled back again (nothing left to do) at the next start.
        note(update, &format!("{TRIAL} not removed ({e})"));
    }
    note(update, &format!("build {build} didn't show its window in {TRIAL_STARTS} starts: back to the build before it"));
    if current == build { Start::Restart(None) } else { Start::Go }
}

/// Apply the build downloaded into `update`, if it is newer than `current` and wasn't rolled back:
/// its plan on the disk, then the swap, then its trial → [`Start::Restart`] when it is in place.
fn apply_ready(app: &Path, update: &Path, current: u64) -> Start {
    let Some(ready) = ready_build(update) else { return Start::Go };
    let next = update.join(NEXT);
    let bad = bad_build(update).is_some_and(|(b, _)| b == ready);
    if ready <= current || bad || !next.join(EXE).is_file() {
        discard(update);
        return Start::Go;
    }
    // Kept after these failures: tried again at the next start.
    let plan = match plan_swap(&next, app, ready, current).and_then(|plan| clear_old(app, &plan).map(|()| plan)) {
        Ok(plan) => plan,
        Err(e) => {
            note(update, &format!("build {ready} not applied ({e}); build {current} goes on"));
            return Start::Go;
        }
    };
    if let Err(e) = write_json(&update.join(SWAP), &plan.to_json()) {
        note(update, &format!("build {ready} not applied ({e}); build {current} goes on"));
        return Start::Go;
    }
    let trial = Trial { plan, starts: 0, rolling_back: false };
    // On trial (its plan with it) once the files moved; the swap's own mark goes after that.
    let done = swap(&next, app, &trial.plan).and_then(|()| trial.write(update)).and_then(|()| std::fs::remove_file(update.join(SWAP)));
    if let Err(e) = done {
        let back = undo(app, &trial.plan);
        if back {
            let _ = std::fs::remove_file(update.join(TRIAL));
            let _ = std::fs::remove_file(update.join(SWAP));
        }
        // `next` may have given files to the swap: downloaded again.
        discard(update);
        let rest = if back { "" } else { ", and isn't all undone yet: the rest at the next start" };
        note(update, &format!("build {ready} not applied ({e}){rest}; build {current} goes on"));
        return Start::Go;
    }
    // Best effort: only for people and the installer to read.
    let _ = std::fs::write(app.join(BUILD_FILE), ready.to_string());
    discard(update);
    note(update, &format!("build {current} → {ready}"));
    Start::Restart(Some(trial.plan))
}

/// At a start while no other window of the app runs (`root`: the install folder, `current`: this
/// build; `count`: a start of its own, not a restart after the graphics failed): undo a swap cut
/// short, count or roll back a new build's trial, clear the files earlier updates set aside, apply
/// a downloaded build.
fn at_start(root: &Path, current: u64, count: bool) -> Start {
    let (app, update) = (root.join(APP), root.join(UPDATE));
    match load(&update.join(SWAP), Plan::from_json) {
        Ok(Some(plan)) => return undo_cut_swap(&app, &update, &plan, current),
        Ok(None) => {}
        Err(e) => {
            // Left as it is (the files it lists may be needed back): the installer sets things right.
            note(&update, &format!("{e}: no update applies"));
            return Start::Go;
        }
    }
    match load(&update.join(TRIAL), Trial::from_json) {
        Err(e) => {
            note(&update, &format!("{e}: no update applies"));
            return Start::Go;
        }
        Ok(Some(t)) if t.rolling_back || (t.plan.build == current && t.starts >= TRIAL_STARTS) => return roll_back(&app, &update, t, current),
        Ok(Some(t)) if t.plan.build == current => {
            if count && let Err(e) = (Trial { starts: t.starts + 1, ..t }).write(&update) {
                // Uncounted: a build that never starts is rolled back only later, or not at all.
                note(&update, &format!("start of build {current} on trial not counted ({e})"));
            }
            // The trial goes on: the files it replaced stay, and no other update meanwhile.
            return Start::Go;
        }
        Ok(Some(t)) => {
            // The trial of another build (installed again since): over.
            note(&update, &format!("trial of build {} over: build {current} runs", t.plan.build));
            let _ = std::fs::remove_file(update.join(TRIAL));
        }
        Ok(None) => {}
    }
    clean_aside(&app);
    if let Some((build, false)) = bad_build(&update)
        && build > current
    {
        set_notice(format!("DanCard: bản {build} không mở được nên app đã quay lại bản {current}; bản sửa sẽ tự tải về."));
        // Best effort: otherwise said again at the next start.
        let _ = write_json(&update.join(BAD), &json!({ "build": build, "told": true }));
    }
    apply_ready(&app, &update, current)
}

/// Updates may apply at a start with `args` (its arguments): not for `--version` or `--help`, a
/// window of its own (`--new-window`) or one an agent drives (`--control`,
/// `VECTORCRAFT_CONTROL_PORT`).
pub fn may_apply(args: &[OsString]) -> bool {
    let driven = std::env::var_os("VECTORCRAFT_CONTROL_PORT").is_some_and(|v| !v.is_empty());
    !driven
        && !args
            .iter()
            .any(|a| a == "--version" || a == "--help" || a == "-h" || a == "--new-window" || a.to_str().is_some_and(|a| a.starts_with("--control")))
}

/// At the start, before anything else: apply a downloaded newer build, roll back a new one that
/// doesn't start or undo a swap cut short, and start the app's exe with `args` (this launch's own
/// arguments) → true when it started (this launch ends). Only when no other window of the app runs
/// (`settings`: the settings folder, where its lock is); otherwise at a later start.
pub fn apply_pending(settings: &Path, args: &[OsString]) -> bool {
    let Some((root, current)) = updating() else { return false };
    let update = root.join(UPDATE);
    let count = std::env::var_os(GPU_SKIP).is_none();
    // `None`: another window runs from these files, or holds `update`.
    let Some(Some(Start::Restart(applied))) = crate::single_instance::alone(settings, || with_lock(&update, || at_start(&root, current, count)))
    else {
        return false;
    };
    let exe = root.join(APP).join(EXE);
    match Command::new(&exe).args(args).spawn() {
        Ok(_) => true,
        Err(e) => {
            note(&update, &format!("{} doesn't start ({e})", exe.display()));
            if let Some(plan) = applied {
                // The new build can't even be started (an antivirus took it…): undone, and not tried
                // again. This run, the build before it, goes on.
                let undone = with_lock_wait(&update, || {
                    let _ = write_json(&update.join(BAD), &json!({ "build": plan.build, "told": false }));
                    let back = undo(&root.join(APP), &plan);
                    if back {
                        let _ = std::fs::remove_file(update.join(TRIAL));
                        let _ = std::fs::write(root.join(APP).join(BUILD_FILE), current.to_string());
                    } else {
                        // Finished at the next start.
                        let _ = (Trial { plan: plan.clone(), starts: 0, rolling_back: true }).write(&update);
                    }
                    back
                });
                note(&update, &format!("build {} undone: {}", plan.build, if undone == Some(true) { "done" } else { "the rest at the next start" }));
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vc-dancard-update-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn exists(path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok()
    }

    /// An install folder: `app` with the given files, an empty `update`.
    fn install(name: &str, files: &[(&str, &str)]) -> (PathBuf, PathBuf, PathBuf) {
        let root = temp(name);
        let (app, update) = (root.join(APP), root.join(UPDATE));
        for (rel, text) in files {
            write(&app.join(rel), text);
        }
        std::fs::create_dir_all(&update).unwrap();
        (root, app, update)
    }

    /// `update\next` with the given files, marked ready as `build`.
    fn download_ready(update: &Path, build: u64, files: &[(&str, &str)]) {
        for (rel, text) in files {
            write(&update.join(NEXT).join(rel), text);
        }
        write_ready(update, build).unwrap();
    }

    /// `latest.json` gives the build, a zip of the fork's releases and its SHA-256; anything else
    /// is refused.
    #[test]
    fn latest_json_is_read_strictly() {
        let sha = "AB".repeat(32);
        let ok = format!("{{\"build\": 42, \"url\": \"{DOWNLOADS}dancard-b42/CongCuBinh-windows-x64.zip\", \"sha256\": \"{sha}\"}}");
        let r = parse_release(&ok).unwrap();
        assert_eq!((r.build, r.sha256.as_str()), (42, "ab".repeat(32).as_str()));
        assert!(r.url.ends_with("/dancard-b42/CongCuBinh-windows-x64.zip"));
        for bad in [
            "{\"build\": 0, \"url\": \"x\", \"sha256\": \"x\"}".to_string(),
            format!("{{\"build\": 42, \"url\": \"https://example.com/a.zip\", \"sha256\": \"{sha}\"}}"),
            format!("{{\"build\": 42, \"url\": \"{DOWNLOADS}../../a.zip\", \"sha256\": \"{sha}\"}}"),
            format!("{{\"build\": 42, \"url\": \"{DOWNLOADS}b/a.zip\", \"sha256\": \"1234\"}}"),
            "not json".to_string(),
        ] {
            assert!(parse_release(&bad).is_err(), "{bad}");
        }
    }

    /// The ready mark is written whole and read back; discarding removes it and the files.
    #[test]
    fn ready_mark_round_trip() {
        let update = temp("ready");
        assert_eq!(ready_build(&update), None);
        write_ready(&update, 7).unwrap();
        assert_eq!(ready_build(&update), Some(7));
        let names: Vec<_> = std::fs::read_dir(&update).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from(READY)], "no temporary file left");
        std::fs::create_dir_all(update.join(NEXT)).unwrap();
        discard(&update);
        assert_eq!(ready_build(&update), None);
        assert!(!update.join(NEXT).exists());
        let _ = std::fs::remove_dir_all(&update);
    }

    /// Only a newer build, not downloaded yet and not rolled back, is downloaded; one whose
    /// download keeps failing waits a day after its third failure.
    #[test]
    fn only_new_good_builds_are_downloaded() {
        let update = temp("wanted");
        assert!(wanted(&update, 5, 6));
        assert!(!wanted(&update, 5, 5) && !wanted(&update, 5, 4));
        write_ready(&update, 6).unwrap();
        assert!(!wanted(&update, 5, 6) && wanted(&update, 5, 7));
        write_json(&update.join(BAD), &json!({ "build": 7, "told": true })).unwrap();
        assert!(!wanted(&update, 5, 7) && wanted(&update, 5, 8));
        for k in 1..=MAX_FAILURES {
            assert!(!backing_off(&update, 8), "after {k} failures");
            count_failure(&update, 8);
        }
        assert!(backing_off(&update, 8) && !backing_off(&update, 9));
        // A day later: three tries again.
        write_json(&update.join(FAILURES), &json!({ "build": 8, "count": MAX_FAILURES, "at": now_secs() - BACK_OFF - 1 })).unwrap();
        assert!(!backing_off(&update, 8));
        count_failure(&update, 8);
        assert_eq!(read_json(&update.join(FAILURES)).unwrap()["count"], 1);
        let _ = std::fs::remove_dir_all(&update);
    }

    /// A plan goes to the disk and back; one naming a file outside the app's folder isn't read.
    #[test]
    fn plans_round_trip_and_stay_inside_the_app() {
        let plan = Plan { build: 6, previous: Some(5), files: vec![(PathBuf::from("docs").join("a.txt"), true), (PathBuf::from(EXE), false)] };
        assert_eq!(Plan::from_json(&plan.to_json()), Some(plan.clone()));
        let trial = Trial { plan, starts: 1, rolling_back: true };
        let update = temp("plan");
        trial.write(&update).unwrap();
        assert_eq!(super::trial(&update), Some(trial));
        for bad in ["../x", "/etc/passwd", "", "a/../../b"] {
            let v = json!({ "build": 6, "files": [{ "path": bad, "existed": true }] });
            assert_eq!(Plan::from_json(&v), None, "{bad}");
        }
        // A mark that can't be read is an error, not "none".
        write(&update.join(SWAP), "{");
        assert!(load(&update.join(SWAP), Plan::from_json).is_err());
        assert_eq!(load(&update.join("nothing.json"), Plan::from_json), Ok(None));
        let _ = std::fs::remove_dir_all(&update);
    }

    /// The exe moves last; each file says whether the app had one of its name.
    #[test]
    fn the_exe_moves_last() {
        let (root, app, update) = install("plan-order", &[(EXE, "build 5"), ("docs/a.txt", "a 5")]);
        download_ready(&update, 6, &[(EXE, "build 6"), ("docs/a.txt", "a 6"), ("docs/new.txt", "new"), ("z.txt", "z")]);
        let plan = plan_swap(&update.join(NEXT), &app, 6, 5).unwrap();
        assert_eq!(plan.files.last(), Some(&(PathBuf::from(EXE), true)));
        assert!(plan.files.contains(&(Path::new("docs").join("a.txt"), true)) && plan.files.contains(&(Path::new("docs").join("new.txt"), false)));
        assert_eq!(plan.files.len(), 4);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A downloaded newer build replaces the app at the start and goes on trial; the old files stay
    /// beside it until its window shows, then go at the next start.
    #[test]
    fn a_downloaded_build_is_applied_and_tried() {
        let (root, app, update) = install("apply", &[(EXE, "build 5"), ("docs/a.txt", "a 5"), (&format!("{EXE}.{OLD}"), "build 3")]);
        download_ready(&update, 6, &[(EXE, "build 6"), ("docs/a.txt", "a 6"), ("docs/new.txt", "new 6")]);
        // Build 5 starts: build 6 goes in place, on trial, and starts instead.
        let Start::Restart(Some(plan)) = at_start(&root, 5, true) else { panic!("build 6 applied") };
        assert_eq!(plan.build, 6);
        assert_eq!((read(&app.join(EXE)).as_str(), read(&app.join("docs/a.txt")).as_str()), ("build 6", "a 6"));
        assert_eq!(read(&app.join("docs/new.txt")), "new 6");
        // The leftover of an earlier update went first: the copy is build 5's.
        assert_eq!(read(&aside(&app.join(EXE), OLD)), "build 5");
        assert_eq!(read(&app.join(BUILD_FILE)), "6");
        assert_eq!((trial(&update).map(|t| (t.plan, t.starts)), ready_build(&update)), (Some((plan, 0)), None));
        assert!(!update.join(NEXT).exists() && !update.join(SWAP).exists());
        // Build 6 starts: one start counted (not a restart after the graphics failed), the old
        // files kept.
        assert_eq!(at_start(&root, 6, true), Start::Go);
        assert_eq!(at_start(&root, 6, false), Start::Go);
        assert_eq!(trial(&update).map(|t| t.starts), Some(1));
        assert!(aside(&app.join(EXE), OLD).exists());
        // Its window shows: the trial is over; the old files go at the next start.
        assert!(!end_trial(&update, 5));
        assert!(end_trial(&update, 6));
        assert_eq!(trial(&update), None);
        assert_eq!(at_start(&root, 6, true), Start::Go);
        assert!(!aside(&app.join(EXE), OLD).exists() && !aside(&app.join("docs/a.txt"), OLD).exists());
        assert_eq!(read(&app.join(EXE)), "build 6");
        assert!(read(&update.join(LOG)).contains("build 5 → 6"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A ready build that isn't newer, was rolled back, or whose files are missing, is dropped.
    #[test]
    fn a_stale_or_bad_download_is_dropped() {
        let (root, app, update) = install("stale", &[(EXE, "build 5")]);
        download_ready(&update, 4, &[(EXE, "build 4")]);
        assert_eq!(at_start(&root, 5, true), Start::Go);
        assert_eq!((ready_build(&update), update.join(NEXT).exists()), (None, false));
        write_ready(&update, 6).unwrap();
        assert_eq!(at_start(&root, 5, true), Start::Go);
        assert_eq!(ready_build(&update), None);
        // Downloaded again by a window of the build before it while it was on trial, then rolled back.
        download_ready(&update, 6, &[(EXE, "build 6")]);
        write_json(&update.join(BAD), &json!({ "build": 6, "told": true })).unwrap();
        assert_eq!(at_start(&root, 5, true), Start::Go);
        assert_eq!((ready_build(&update), read(&app.join(EXE)).as_str()), (None, "build 5"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A swap that fails half way is undone at once: every old file back, no new one left.
    #[test]
    fn a_failed_swap_is_undone_at_once() {
        let (root, app, update) = install("failed-swap", &[(EXE, "build 5"), ("a.txt", "old a"), ("sub", "a file")]);
        // `sub/c.txt` can't be written: a file named `sub` is in the way of its folder.
        download_ready(&update, 6, &[(EXE, "build 6"), ("a.txt", "new a"), ("sub/c.txt", "new c")]);
        assert_eq!(at_start(&root, 5, true), Start::Go);
        assert_eq!(
            (read(&app.join(EXE)).as_str(), read(&app.join("a.txt")).as_str(), read(&app.join("sub")).as_str()),
            ("build 5", "old a", "a file")
        );
        assert!(!exists(&aside(&app.join("a.txt"), OLD)) && !exists(&update.join(SWAP)) && trial(&update).is_none());
        // `next` gave a file to the swap: downloaded again.
        assert_eq!(ready_build(&update), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A swap cut short (a power cut, the process killed) is undone at the next start, whichever
    /// build then starts: when it is the new one, the old one starts instead.
    #[test]
    fn a_swap_cut_short_is_undone_at_the_next_start() {
        for moved in 0..=3 {
            let (root, app, update) = install(&format!("cut-{moved}"), &[(EXE, "build 5"), ("docs/a.txt", "a 5")]);
            download_ready(&update, 6, &[(EXE, "build 6"), ("docs/a.txt", "a 6"), ("docs/new.txt", "new 6")]);
            let plan = plan_swap(&update.join(NEXT), &app, 6, 5).unwrap();
            write_json(&update.join(SWAP), &plan.to_json()).unwrap();
            // The first `moved` files swapped, and the next one half (set aside, not moved in).
            let partial = Plan { files: plan.files.iter().take(moved).cloned().collect(), ..plan.clone() };
            swap(&update.join(NEXT), &app, &partial).unwrap();
            if let Some((rel, true)) = plan.files.get(moved) {
                std::fs::rename(app.join(rel), aside(&app.join(rel), OLD)).unwrap();
            }
            // The exe moves last: until then build 5 starts, after that build 6.
            let running = if moved == plan.files.len() { 6 } else { 5 };
            let expected = if running == 6 { Start::Restart(None) } else { Start::Go };
            assert_eq!(at_start(&root, running, true), expected, "{moved} moved");
            assert_eq!((read(&app.join(EXE)).as_str(), read(&app.join("docs/a.txt")).as_str()), ("build 5", "a 5"), "{moved} moved");
            assert!(!exists(&app.join("docs/new.txt")) && !exists(&update.join(SWAP)) && ready_build(&update).is_none(), "{moved} moved");
            // Nothing left to undo: the next start applies nothing and clears what went aside.
            assert_eq!(at_start(&root, 5, true), Start::Go);
            assert!(!exists(&aside(&app.join("docs/new.txt"), FAILED)) && !exists(&aside(&app.join(EXE), OLD)));
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    /// A new build that doesn't show its window in its first starts is rolled back: the old files
    /// come back, its own go aside (deleted later), and it is marked bad (said once). A roll-back
    /// cut short goes on at the next start.
    #[test]
    fn a_build_that_never_starts_is_rolled_back() {
        let (root, app, update) = install("never", &[(EXE, "build 5"), ("docs/a.txt", "a 5"), (BUILD_FILE, "5")]);
        download_ready(&update, 6, &[(EXE, "build 6"), ("docs/a.txt", "a 6"), ("docs/new.txt", "only 6")]);
        assert!(matches!(at_start(&root, 5, true), Start::Restart(Some(_))));
        // Two starts without a window.
        assert_eq!(at_start(&root, 6, true), Start::Go);
        assert_eq!(at_start(&root, 6, true), Start::Go);
        assert_eq!(trial(&update).map(|t| (t.starts, t.rolling_back)), Some((2, false)));
        // The third: back to build 5, which starts instead.
        assert_eq!(at_start(&root, 6, true), Start::Restart(None));
        assert_eq!((read(&app.join(EXE)).as_str(), read(&app.join("docs/a.txt")).as_str()), ("build 5", "a 5"));
        assert_eq!(read(&aside(&app.join(EXE), FAILED)), "build 6");
        assert_eq!(read(&aside(&app.join("docs/new.txt"), FAILED)), "only 6");
        assert!(!exists(&app.join("docs/new.txt")) && !aside(&app.join(EXE), OLD).exists());
        assert_eq!(read(&app.join(BUILD_FILE)), "5");
        assert_eq!((trial(&update), bad_build(&update)), (None, Some((6, false))));
        // Build 5 starts: told once, the failed files deleted, build 6 never downloaded again.
        *NOTICE.lock().unwrap() = None;
        assert_eq!(at_start(&root, 5, true), Start::Go);
        assert!(NOTICE.lock().unwrap().take().is_some_and(|n| n.contains("bản 6") && n.contains("bản 5")));
        assert_eq!(bad_build(&update), Some((6, true)));
        assert!(!aside(&app.join(EXE), FAILED).exists() && !aside(&app.join("docs/new.txt"), FAILED).exists());
        assert!(!wanted(&update, 5, 6) && wanted(&update, 5, 7));
        assert_eq!(at_start(&root, 5, true), Start::Go);
        assert!(NOTICE.lock().unwrap().take().is_none());
        let _ = std::fs::remove_dir_all(&root);

        // Cut short: the exe back already, a document not yet.
        let (root, app, update) = install("never-cut", &[(EXE, "build 5"), ("docs/a.txt", "a 6"), ("docs/a.txt.old", "a 5")]);
        let plan = Plan { build: 6, previous: Some(5), files: vec![(PathBuf::from("docs/a.txt"), true), (PathBuf::from(EXE), true)] };
        Trial { plan, starts: 2, rolling_back: true }.write(&update).unwrap();
        assert_eq!(at_start(&root, 5, true), Start::Go);
        assert_eq!((read(&app.join("docs/a.txt")).as_str(), trial(&update)), ("a 5", None));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// No roll-back without the old files (installed again since): the trial just ends.
    #[test]
    fn no_roll_back_without_the_old_files() {
        let (root, app, update) = install("noold", &[(EXE, "build 6")]);
        let plan = Plan { build: 6, previous: Some(5), files: vec![(PathBuf::from(EXE), true)] };
        Trial { plan: plan.clone(), starts: TRIAL_STARTS, rolling_back: false }.write(&update).unwrap();
        assert_eq!(at_start(&root, 6, true), Start::Go);
        assert_eq!((trial(&update), bad_build(&update)), (None, None));
        assert_eq!(read(&app.join(EXE)), "build 6");
        // A trial of another build is over at once.
        Trial { plan: Plan { build: 9, ..plan }, starts: 0, rolling_back: false }.write(&update).unwrap();
        assert_eq!(at_start(&root, 6, true), Start::Go);
        assert_eq!(trial(&update), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A mark that can't be read stops updates (the files it lists may be needed back) rather than
    /// clearing the files set aside.
    #[test]
    fn an_unreadable_mark_stops_updates() {
        let (root, app, update) = install("unreadable", &[(EXE, "build 6"), (&format!("{EXE}.{OLD}"), "build 5")]);
        write(&update.join(TRIAL), "not json");
        download_ready(&update, 7, &[(EXE, "build 7")]);
        assert_eq!(at_start(&root, 6, true), Start::Go);
        assert_eq!((read(&aside(&app.join(EXE), OLD)).as_str(), read(&app.join(EXE)).as_str()), ("build 5", "build 6"));
        assert_eq!(ready_build(&update), Some(7));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// One window at a time in `update`.
    #[test]
    fn windows_take_turns_in_the_update_folder() {
        let update = temp("lock");
        assert_eq!(with_lock(&update, || with_lock(&update, || ()).is_none()), Some(true));
        assert_eq!(with_lock_wait(&update, || 7), Some(7));
        assert_eq!(with_lock(&update, || 8), Some(8), "let go after use");
        let _ = std::fs::remove_dir_all(&update);
    }

    /// The updater's log lines carry the date in UTC.
    #[test]
    fn log_lines_are_dated() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_742), (2026, 10, 16));
        assert_eq!(civil(11_016), (2000, 2, 29));
        assert_eq!(civil(10_956), (1999, 12, 31));
        assert_eq!(civil(47_541), (2100, 3, 1));
        assert_eq!(utc(20_742 * 86_400 + 4 * 3600 + 5 * 60 + 6), "2026-10-16 04:05:06Z");
        let update = temp("log");
        note(&update, "hello");
        assert!(read(&update.join(LOG)).trim_end().ends_with("] hello"));
        let _ = std::fs::remove_dir_all(&update);
    }

    /// A downloaded build is used only when it starts and says it is that build.
    #[cfg(unix)]
    #[test]
    fn a_download_must_start_as_its_build() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = temp("runs");
        let exe = dir.join("vc");
        write(&exe, "#!/bin/sh\necho 'vectorcraft 0.11.0 (DanCard build 7)'\n");
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        // A test running a process at the same moment may hold the new file open an instant
        // ("text file busy").
        assert!((0..20).any(|_| runs_as(&exe, 7).is_ok() || {
            std::thread::sleep(Duration::from_millis(50));
            false
        }));
        assert!(runs_as(&exe, 8).is_err());
        assert!(runs_as(&dir.join("missing"), 7).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Only plain starts apply updates.
    #[test]
    fn some_starts_apply_no_update() {
        let args = |a: &[&str]| a.iter().map(OsString::from).collect::<Vec<_>>();
        if std::env::var_os("VECTORCRAFT_CONTROL_PORT").is_none() {
            assert!(may_apply(&args(&[])) && may_apply(&args(&["a.ai", "b.pdf"])));
        }
        for a in ["--version", "--help", "-h", "--new-window", "--control", "--control=7979"] {
            assert!(!may_apply(&args(&["x.ai", a])), "{a}");
        }
        #[cfg(feature = "wgpu")]
        assert_eq!(GPU_SKIP, crate::gpu::SKIP_ENV);
    }

    /// The zip's folder (named after the version) or the unpacked folder itself holds the app.
    #[test]
    fn the_app_is_found_in_the_zip_folder() {
        let dir = temp("find");
        assert_eq!(find_app_dir(&dir), None);
        let inner = dir.join("vectorcraft-0.11.0-windows-x64-portable");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(inner.join(EXE), "exe").unwrap();
        assert_eq!(find_app_dir(&dir), Some(inner.clone()));
        std::fs::write(dir.join(EXE), "exe").unwrap();
        assert_eq!(find_app_dir(&dir), Some(dir.clone()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SHA-256 of a file in lowercase hex; reading is capped; the leftovers of earlier downloads go.
    #[test]
    fn checksums_capped_reads_and_leftovers() {
        let dir = temp("sha");
        let f = dir.join("x");
        std::fs::write(&f, "abc").unwrap();
        assert_eq!(sha256_file(&f).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(read_capped(&f, 3).unwrap(), "abc");
        assert!(read_capped(&f, 2).is_err());
        for name in ["5.zip", "6.zip", "6.zip.12.part", "ready.json.12.tmp", "unpack-12/a", "ready.json", "next/vectorcraft"] {
            write(&dir.join(name), "x");
        }
        remove_leftovers(&dir, "6.zip");
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, ["6.zip", "next", "ready.json", "x"]);
        assert!(is_aside(Path::new("a.old")) && is_aside(Path::new("a.bad")) && is_aside(Path::new("a.bad3")));
        assert!(!is_aside(Path::new("a.badge")) && !is_aside(Path::new("a.exe")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Local builds (no build number) and runs outside the per-user install never update.
    #[test]
    fn only_numbered_installed_builds_update() {
        if build().is_none() {
            assert!(updating().is_none());
        }
        assert!(!is_managed(&std::env::temp_dir().join("no-such-install")));
    }
}
