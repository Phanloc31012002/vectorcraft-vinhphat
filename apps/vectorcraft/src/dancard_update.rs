//! Automatic updates of the DanCard fork (IN Vĩnh Phát) on Windows, from the fork's GitHub
//! Releases: every machine follows the newest build without anyone copying files around.
//!
//! - Only builds made by the fork's "DanCard Windows x64" workflow carry a build number
//!   (`DANCARD_BUILD`: the workflow's run number, set at compile time), and only those, installed
//!   per user in `%LOCALAPPDATA%\CongCuBinh\app` (`packaging/dancard/CaiDat-CongCuBinh.cmd`), update
//!   themselves: no administrator rights are needed to replace their files.
//! - In the background, half a minute after the start and then every hour, the app reads the
//!   newest release's `latest.json` (build, zip address, SHA-256). A newer build is downloaded with
//!   `curl.exe`, checked against its SHA-256, unpacked with `tar.exe` (both part of Windows 10 and
//!   later) into `CongCuBinh\update\next` and marked ready; the status bar and the DanCard panel say
//!   so. Nothing is asked: the work in progress goes on.
//! - The next start applies it first, when no other window of the app runs: the new files replace
//!   the old ones (the running exe is renamed aside, which Windows allows), and the new version
//!   starts with the same files to open. A failure puts the old files back and starts as before.
//! - The new build is on trial until its window has drawn a frame: the files it replaced wait
//!   beside it (`.old`). One that doesn't draw its window in [`TRIAL_STARTS`] starts (it crashes on
//!   start) is rolled back: the old files come back, and that build isn't downloaded again (the next
//!   one is). A build that can't start can't download its fix: without this, every machine would
//!   need the installer again.
//!
//! Nothing here panics: a failure only means no update this time (logged).

use std::ffi::OsString;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

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
/// Where a downloaded zip is unpacked, in `update`.
const UNPACK: &str = "unpack";
/// `{"build": n}`: the build in `next`, written once it is complete.
const READY: &str = "ready.json";
/// `{"build": n, "starts": k, "previous": p}`: build `n` replaced build `p` and was started `k`
/// times without showing its window yet; the files it replaced wait beside it until it does.
const TRIAL: &str = "trial.json";
/// `{"build": n, "told": b}`: build `n` didn't start and was rolled back (`told`: the status bar
/// said so); it isn't downloaded again.
const BAD: &str = "bad.json";
/// The installed build, in `app` (for people and the installer).
const BUILD_FILE: &str = "build.txt";
const EXE: &str = if cfg!(windows) { "vectorcraft.exe" } else { "vectorcraft" };
/// Extension of a file an update replaced (`name.old`), kept for a roll-back.
const OLD: &str = "old";
/// Extension of a file of a build that was rolled back (`name.bad`).
const FAILED: &str = "bad";
/// Starts a new build gets to show its window before the build before it comes back.
const TRIAL_STARTS: u64 = 2;
/// The first check, after the start.
const FIRST_CHECK: Duration = Duration::from_secs(30);
/// The checks after that (`latest.json` is a few hundred bytes).
const EVERY: Duration = Duration::from_secs(3600);
/// Largest JSON file read (bytes).
const MAX_JSON: u64 = 64 * 1024;
/// Most files an update holds, and how deep its folders go.
const MAX_FILES: usize = 10_000;
const MAX_DEPTH: usize = 8;

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
            if let Some((root, current)) = updating()
                && end_trial(&root.join(UPDATE), current)
            {
                log::info!("DanCard update: build {current} starts");
                set_notice(format!("DanCard: đã lên bản {current}."));
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
    serde_json::from_str(&read_capped(path, MAX_JSON).ok()?).ok()
}

/// Write `value` to `path` whole (through a temporary file: a half-written file is never read).
fn write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    let tmp = aside(path, "tmp");
    std::fs::write(&tmp, value.to_string())?;
    std::fs::rename(&tmp, path)
}

/// The build ready to apply in `update`, if one is.
fn ready_build(update: &Path) -> Option<u64> {
    number(&read_json(&update.join(READY))?, "build")
}

/// Mark `build` ready in `update`.
fn write_ready(update: &Path, build: u64) -> std::io::Result<()> {
    write_json(&update.join(READY), &json!({ "build": build }))
}

/// A new build on trial.
#[derive(Debug, PartialEq)]
struct Trial {
    build: u64,
    /// Starts without showing its window.
    starts: u64,
    /// The build it replaced.
    previous: Option<u64>,
}

impl Trial {
    fn write(&self, update: &Path) -> std::io::Result<()> {
        write_json(&update.join(TRIAL), &json!({ "build": self.build, "starts": self.starts, "previous": self.previous }))
    }
}

/// The build on trial in `update`.
fn trial(update: &Path) -> Option<Trial> {
    let v = read_json(&update.join(TRIAL))?;
    Some(Trial { build: number(&v, "build")?, starts: v.get("starts").and_then(Value::as_u64).unwrap_or(0), previous: number(&v, "previous") })
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

/// One of Windows' own tools (`curl`, `tar`), from System32 rather than whatever `PATH` finds,
/// started without a console window.
fn tool(name: &str) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let system = std::env::var_os("SystemRoot").map(|r| PathBuf::from(r).join("System32").join(format!("{name}.exe")));
        let mut c = Command::new(system.filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from(format!("{name}.exe"))));
        c.creation_flags(CREATE_NO_WINDOW);
        c
    }
    #[cfg(not(windows))]
    {
        Command::new(name)
    }
}

/// Download `url` into `dir` as `name` (through `name.part`, so the file is never half there) →
/// its path. curl runs in `dir` and gets bare file names: a user folder with letters outside the
/// system's code page (`C:\Users\Người dùng`) doesn't reach a tool that reads its arguments in
/// that code page.
fn download(url: &str, dir: &Path, name: &str) -> Result<PathBuf, String> {
    let part = format!("{name}.part");
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
    std::fs::rename(&part, &out).map_err(|e| e.to_string())?;
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

/// Download the newest release into `root\update\next` when [`wanted`] → the build made ready, if
/// one was.
fn check_and_download(root: &Path, current: u64) -> Result<Option<u64>, String> {
    let update = root.join(UPDATE);
    std::fs::create_dir_all(&update).map_err(|e| e.to_string())?;
    let latest = download(LATEST, &update, "latest.json")?;
    let release = parse_release(&read_capped(&latest, MAX_JSON)?)?;
    if !wanted(&update, current, release.build) {
        return Ok(None);
    }
    let zip_name = format!("{}.zip", release.build);
    let zip = download(&release.url, &update, &zip_name)?;
    let sum = sha256_file(&zip)?;
    if sum != release.sha256 {
        // Best effort: a damaged download is only left out.
        let _ = std::fs::remove_file(&zip);
        return Err(format!("build {}: SHA-256 {sum} instead of {}", release.build, release.sha256));
    }
    let unpacked = update.join(UNPACK);
    // Best effort: a leftover of an interrupted unpack, if any.
    let _ = std::fs::remove_dir_all(&unpacked);
    std::fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
    // In `update`, with bare names, as curl above.
    let status = tool("tar").current_dir(&update).args(["-xf", zip_name.as_str(), "-C", UNPACK]).status().map_err(|e| format!("tar: {e}"))?;
    if !status.success() {
        return Err(format!("tar: {status}"));
    }
    let found = find_app_dir(&unpacked).ok_or_else(|| format!("build {}: no {EXE} in its zip", release.build))?;
    discard(&update);
    std::fs::rename(&found, update.join(NEXT)).map_err(|e| e.to_string())?;
    write_ready(&update, release.build).map_err(|e| e.to_string())?;
    // Best effort: done with them.
    let _ = std::fs::remove_dir_all(&unpacked);
    let _ = std::fs::remove_file(&zip);
    Ok(Some(release.build))
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
                    log::info!("DanCard update: build {build} downloaded, used from the next start");
                    set_notice(format!("DanCard: đã tải bản mới (bản {build}): đóng rồi mở lại app là dùng bản mới."));
                    vectorcraft_ui_egui::panels::dancard::set_update_ready(build);
                    ctx.request_repaint();
                }
                Ok(None) => {}
                // No network, GitHub unreachable…: try again later.
                Err(e) => log::info!("DanCard update: {e}"),
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

/// The files under `dir` set aside with extension `ext`, relative to it.
fn set_aside(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut files = vec![];
    // Best effort: what can't be listed stays as it is.
    let _ = files_under(dir, Path::new(""), &mut files, 0);
    files.retain(|f| f.extension().is_some_and(|e| e == ext));
    files
}

/// Copy the files of `next` over those of `app`: each file it replaces is set aside first (the
/// running exe can be renamed, not overwritten). A failure puts every old file back.
fn swap(next: &Path, app: &Path) -> std::io::Result<()> {
    let mut files = vec![];
    files_under(next, Path::new(""), &mut files, 0)?;
    let (mut moved, mut written) = (vec![], vec![]);
    let mut copy_all = || -> std::io::Result<()> {
        for rel in &files {
            let target = app.join(rel);
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir)?;
            }
            if target.exists() {
                let old = aside(&target, OLD);
                // A file set aside by an earlier update; if it can't go, the rename below fails.
                let _ = std::fs::remove_file(&old);
                std::fs::rename(&target, &old)?;
                moved.push(target.clone());
            }
            // Before the copy: a copy that fails half way leaves part of a file to remove.
            written.push(target.clone());
            std::fs::copy(next.join(rel), &target)?;
        }
        Ok(())
    };
    let result = copy_all();
    if result.is_err() {
        // Back as it was, as far as it goes: the new files out, the old ones back.
        for t in written.iter().rev() {
            let _ = std::fs::remove_file(t);
        }
        for t in moved.iter().rev() {
            let _ = std::fs::rename(aside(t, OLD), t);
        }
    }
    result
}

/// Delete the files earlier updates and roll-backs set aside in `app` (those still running stay
/// till a later start).
fn clean_aside(app: &Path) {
    for ext in [OLD, FAILED] {
        for rel in set_aside(app, ext) {
            // Still running (another window of that build): deleted at a later start.
            let _ = std::fs::remove_file(app.join(rel));
        }
    }
}

/// The window of build `current` shows: its trial in `update` (if any) is over, and the files it
/// replaced go at the next start → true when a trial ended.
fn end_trial(update: &Path, current: u64) -> bool {
    if trial(update).is_none_or(|t| t.build != current) {
        return false;
    }
    match std::fs::remove_file(update.join(TRIAL)) {
        Ok(()) => true,
        Err(e) => {
            // The next start counts one more start of the trial.
            log::warn!("DanCard update: trial of build {current} not ended ({e})");
            false
        }
    }
}

/// Build `current` didn't show its window in [`TRIAL_STARTS`] starts: the files it replaced in
/// `app` come back (its own go aside as `.bad`), and it isn't downloaded again → true when the
/// build before it (`previous`, if known) is in place to start.
fn roll_back(app: &Path, update: &Path, current: u64, previous: Option<u64>) -> bool {
    // Over, whatever comes of it: a roll-back is tried once.
    let _ = std::fs::remove_file(update.join(TRIAL));
    let exe = app.join(EXE);
    if !aside(&exe, OLD).is_file() {
        log::warn!("DanCard update: build {current} doesn't start, and nothing is left to go back to");
        return false;
    }
    for rel in set_aside(app, OLD) {
        let (old, file) = (app.join(&rel), app.join(rel.with_extension("")));
        let failed = aside(&file, FAILED);
        // A leftover of an earlier roll-back; if it can't go, the rename below fails.
        let _ = std::fs::remove_file(&failed);
        // The failing build's file aside (the running exe can be renamed), the old one back. When
        // the old one can't come back, the new one stays: each file is one build's or the other's.
        if (!file.exists() || std::fs::rename(&file, &failed).is_ok()) && std::fs::rename(&old, &file).is_err() {
            let _ = std::fs::rename(&failed, &file);
        }
    }
    if aside(&exe, OLD).exists() {
        log::warn!("DanCard update: build {current} doesn't start, and the build before it couldn't come back");
        return false;
    }
    // Best effort: only for people and the installer to read.
    let _ = match previous {
        Some(p) => std::fs::write(app.join(BUILD_FILE), p.to_string()),
        None => std::fs::remove_file(app.join(BUILD_FILE)),
    };
    // Best effort: without the mark, the failing build is downloaded and tried again.
    if let Err(e) = write_json(&update.join(BAD), &json!({ "build": current, "told": false })) {
        log::warn!("DanCard update: build {current} not marked bad ({e})");
    }
    log::warn!("DanCard update: build {current} didn't show its window in {TRIAL_STARTS} starts: back to the build before it");
    true
}

/// Apply the build downloaded into `update`, if it is newer than `current` → true when it is in
/// place in `app` (on trial) to start.
fn apply_ready(app: &Path, update: &Path, current: u64) -> bool {
    let Some(ready) = ready_build(update) else { return false };
    let next = update.join(NEXT);
    if ready <= current || !next.join(EXE).is_file() {
        discard(update);
        return false;
    }
    if let Err(e) = swap(&next, app) {
        // Kept: tried again at the next start.
        log::warn!("DanCard update: build {ready} not applied ({e}); build {current} goes on");
        return false;
    }
    // Best effort: only for people and the installer to read.
    let _ = std::fs::write(app.join(BUILD_FILE), ready.to_string());
    if let Err(e) = (Trial { build: ready, starts: 0, previous: Some(current) }).write(update) {
        // The update is in place; only a roll-back, should it not start, isn't possible.
        log::warn!("DanCard update: build {ready} isn't on trial ({e})");
    }
    discard(update);
    log::info!("DanCard update: build {current} → {ready}");
    true
}

/// At a start while no other window of the app runs (`root`: the install folder, `current`: this
/// build): count or roll back a new build's trial, clear the files earlier updates set aside, apply
/// a downloaded build → true when the app's exe is to start in place of this run.
fn at_start(root: &Path, current: u64) -> bool {
    let (app, update) = (root.join(APP), root.join(UPDATE));
    match trial(&update) {
        Some(t) if t.build == current => {
            if t.starts >= TRIAL_STARTS {
                return roll_back(&app, &update, current, t.previous);
            }
            if let Err(e) = (Trial { starts: t.starts + 1, ..t }).write(&update) {
                // Uncounted: a build that never starts is rolled back only later, or not at all.
                log::warn!("DanCard update: start of build {current} on trial not counted ({e})");
            }
            // The trial goes on: the files it replaced stay, and no other update meanwhile.
            return false;
        }
        Some(_) => {
            // The trial of another build (rolled back, or installed again): over.
            let _ = std::fs::remove_file(update.join(TRIAL));
        }
        None => {}
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

/// At the start, before anything else: apply a downloaded newer build, or roll back a new one that
/// doesn't start, and start the app's exe with `args` (this launch's own arguments) → true when it
/// started (this launch ends). Only when no other window of the app runs (`settings`: the settings
/// folder, where its lock is); otherwise at a later start.
pub fn apply_pending(settings: &Path, args: &[OsString]) -> bool {
    let Some((root, current)) = updating() else { return false };
    // `None`: another window runs from these files.
    if crate::single_instance::alone(settings, || at_start(&root, current)) != Some(true) {
        return false;
    }
    match Command::new(root.join(APP).join(EXE)).args(args).spawn() {
        Ok(_) => true,
        Err(e) => {
            // This run goes on with the build it started with; the files in place start next time.
            log::warn!("DanCard update: the app didn't start ({e})");
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

    /// An install folder: `app` with the given files, an empty `update`.
    fn install(name: &str, files: &[(&str, &str)]) -> (PathBuf, PathBuf, PathBuf) {
        let root = temp(name);
        let (app, update) = (root.join(APP), root.join(UPDATE));
        for (rel, text) in files {
            let path = app.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        std::fs::create_dir_all(&update).unwrap();
        (root, app, update)
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
        assert!(!aside(&update.join(READY), "tmp").exists());
        std::fs::create_dir_all(update.join(NEXT)).unwrap();
        discard(&update);
        assert_eq!(ready_build(&update), None);
        assert!(!update.join(NEXT).exists());
        let _ = std::fs::remove_dir_all(&update);
    }

    /// Only a newer build, not downloaded yet and not rolled back, is downloaded.
    #[test]
    fn only_new_good_builds_are_downloaded() {
        let update = temp("wanted");
        assert!(wanted(&update, 5, 6));
        assert!(!wanted(&update, 5, 5) && !wanted(&update, 5, 4));
        write_ready(&update, 6).unwrap();
        assert!(!wanted(&update, 5, 6) && wanted(&update, 5, 7));
        write_json(&update.join(BAD), &json!({ "build": 7, "told": true })).unwrap();
        assert!(!wanted(&update, 5, 7) && wanted(&update, 5, 8));
        let _ = std::fs::remove_dir_all(&update);
    }

    /// New files replace the old ones (set aside, deleted at a later start); new folders are made.
    #[test]
    fn swap_replaces_files_and_sets_old_ones_aside() {
        let dir = temp("swap");
        let (next, app) = (dir.join("next"), dir.join("app"));
        std::fs::create_dir_all(next.join("docs")).unwrap();
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(next.join(EXE), "new exe").unwrap();
        std::fs::write(next.join("docs/README.md"), "new readme").unwrap();
        std::fs::write(app.join(EXE), "old exe").unwrap();
        std::fs::write(app.join("keep.txt"), "kept").unwrap();
        swap(&next, &app).unwrap();
        assert_eq!(read(&app.join(EXE)), "new exe");
        assert_eq!(read(&app.join("docs/README.md")), "new readme");
        assert_eq!(read(&aside(&app.join(EXE), OLD)), "old exe");
        assert_eq!(read(&app.join("keep.txt")), "kept");
        clean_aside(&app);
        assert!(!aside(&app.join(EXE), OLD).exists());
        assert!(app.join(EXE).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A failure half way puts every old file back and leaves no new one.
    #[test]
    fn a_failed_swap_puts_the_old_files_back() {
        let dir = temp("rollback");
        let (next, app) = (dir.join("next"), dir.join("app"));
        std::fs::create_dir_all(next.join("sub")).unwrap();
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(next.join("a.txt"), "new a").unwrap();
        std::fs::write(next.join("sub/c.txt"), "new c").unwrap();
        std::fs::write(app.join("a.txt"), "old a").unwrap();
        // `sub/c.txt` can't be written: a file named `sub` is in the way of its folder.
        std::fs::write(app.join("sub"), "a file").unwrap();
        assert!(swap(&next, &app).is_err());
        assert_eq!(read(&app.join("a.txt")), "old a");
        assert!(!aside(&app.join("a.txt"), OLD).exists());
        assert_eq!(read(&app.join("sub")), "a file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A downloaded newer build replaces the app at the start and goes on trial; the old files stay
    /// beside it until its window shows, then go at the next start.
    #[test]
    fn a_downloaded_build_is_applied_and_tried() {
        let (root, app, update) = install("apply", &[(EXE, "build 5"), ("docs/a.txt", "a 5")]);
        std::fs::create_dir_all(update.join(NEXT).join("docs")).unwrap();
        std::fs::write(update.join(NEXT).join(EXE), "build 6").unwrap();
        std::fs::write(update.join(NEXT).join("docs/a.txt"), "a 6").unwrap();
        write_ready(&update, 6).unwrap();
        // Build 5 starts: build 6 goes in place, on trial, and starts instead.
        assert!(at_start(&root, 5));
        assert_eq!((read(&app.join(EXE)).as_str(), read(&app.join("docs/a.txt")).as_str()), ("build 6", "a 6"));
        assert_eq!(read(&aside(&app.join(EXE), OLD)), "build 5");
        assert_eq!(read(&app.join(BUILD_FILE)), "6");
        assert_eq!((trial(&update), ready_build(&update)), (Some(Trial { build: 6, starts: 0, previous: Some(5) }), None));
        assert!(!update.join(NEXT).exists());
        // Build 6 starts: one start counted, the old files kept.
        assert!(!at_start(&root, 6));
        assert_eq!(trial(&update).map(|t| t.starts), Some(1));
        assert!(aside(&app.join(EXE), OLD).exists());
        // Its window shows: the trial is over; the old files go at the next start.
        assert!(!end_trial(&update, 5));
        assert!(end_trial(&update, 6));
        assert_eq!(trial(&update), None);
        assert!(!at_start(&root, 6));
        assert!(!aside(&app.join(EXE), OLD).exists() && !aside(&app.join("docs/a.txt"), OLD).exists());
        assert_eq!(read(&app.join(EXE)), "build 6");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A ready build that isn't newer, or whose files are missing, is dropped.
    #[test]
    fn a_stale_download_is_dropped() {
        let (root, app, update) = install("stale", &[(EXE, "build 5")]);
        std::fs::create_dir_all(update.join(NEXT)).unwrap();
        std::fs::write(update.join(NEXT).join(EXE), "build 4").unwrap();
        write_ready(&update, 4).unwrap();
        assert!(!at_start(&root, 5));
        assert_eq!((ready_build(&update), update.join(NEXT).exists()), (None, false));
        write_ready(&update, 6).unwrap();
        assert!(!at_start(&root, 5));
        assert_eq!(ready_build(&update), None);
        assert_eq!(read(&app.join(EXE)), "build 5");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A new build that doesn't show its window in its first starts is rolled back: the old files
    /// come back, its own go aside (deleted later), and it is marked bad (said once).
    #[test]
    fn a_build_that_never_starts_is_rolled_back() {
        let old = format!("{EXE}.{OLD}");
        let (root, app, update) = install(
            "never",
            &[
                (EXE, "build 6"),
                (old.as_str(), "build 5"),
                ("docs/a.txt", "a 6"),
                ("docs/a.txt.old", "a 5"),
                ("docs/new.txt", "only 6"),
                (BUILD_FILE, "6"),
            ],
        );
        Trial { build: 6, starts: 0, previous: Some(5) }.write(&update).unwrap();
        // Two starts without a window.
        assert!(!at_start(&root, 6));
        assert!(!at_start(&root, 6));
        assert_eq!(trial(&update), Some(Trial { build: 6, starts: 2, previous: Some(5) }));
        // The third: back to build 5, which starts instead.
        assert!(at_start(&root, 6));
        assert_eq!((read(&app.join(EXE)).as_str(), read(&app.join("docs/a.txt")).as_str()), ("build 5", "a 5"));
        assert_eq!(read(&aside(&app.join(EXE), FAILED)), "build 6");
        assert!(!aside(&app.join(EXE), OLD).exists());
        assert_eq!((read(&app.join(BUILD_FILE)).as_str(), read(&app.join("docs/new.txt")).as_str()), ("5", "only 6"));
        assert_eq!((trial(&update), bad_build(&update)), (None, Some((6, false))));
        // Build 5 starts: told once, the failed files deleted, build 6 never downloaded again.
        *NOTICE.lock().unwrap() = None;
        assert!(!at_start(&root, 5));
        assert!(NOTICE.lock().unwrap().take().is_some_and(|n| n.contains("bản 6") && n.contains("bản 5")));
        assert_eq!(bad_build(&update), Some((6, true)));
        assert!(!aside(&app.join(EXE), FAILED).exists() && !aside(&app.join("docs/a.txt"), FAILED).exists());
        assert!(!wanted(&update, 5, 6) && wanted(&update, 5, 7));
        assert!(!at_start(&root, 5));
        assert!(NOTICE.lock().unwrap().take().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// No roll-back without the old files (installed again since): the trial just ends.
    #[test]
    fn no_roll_back_without_the_old_files() {
        let (root, app, update) = install("noold", &[(EXE, "build 6")]);
        Trial { build: 6, starts: TRIAL_STARTS, previous: Some(5) }.write(&update).unwrap();
        assert!(!at_start(&root, 6));
        assert_eq!((trial(&update), bad_build(&update)), (None, None));
        assert_eq!(read(&app.join(EXE)), "build 6");
        // A trial of another build is over at once.
        Trial { build: 9, starts: 0, previous: None }.write(&update).unwrap();
        assert!(!at_start(&root, 6));
        assert_eq!(trial(&update), None);
        let _ = std::fs::remove_dir_all(&root);
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

    /// SHA-256 of a file in lowercase hex; reading is capped.
    #[test]
    fn checksums_and_capped_reads() {
        let dir = temp("sha");
        let f = dir.join("x");
        std::fs::write(&f, "abc").unwrap();
        assert_eq!(sha256_file(&f).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(read_capped(&f, 3).unwrap(), "abc");
        assert!(read_capped(&f, 2).is_err());
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
