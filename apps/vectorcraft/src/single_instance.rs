//! One window for every file (DanCard fork, IN Vĩnh Phát): a file opened from the file manager
//! (double-click, Open With, a drop on the app's icon) while VectorCraft runs opens as a tab of the
//! window that is there, as in Illustrator, not in a second window (a second process).
//!
//! - The first launch holds the lock on `instance.lock` (in the settings folder, next to `ui.json`)
//!   while it runs and listens on 127.0.0.1, on a port the system picks; the port and a random
//!   token go in `instance.json` beside it (readable by its user alone on Linux).
//! - A later launch finds the lock held: it sends the token and the absolute paths of its files and
//!   ends. The running app opens them as tabs and brings its window to the front (no files: only
//!   that). While the window is busy (a long command), the files wait and open when it's done.
//! - The handover has a deadline, a few seconds from the launch: the running app takes the files
//!   only before it, and once (a launch that retries isn't opened twice). An app that is closing
//!   turns them down, and the launch takes the lock when it has closed. When the app that runs
//!   doesn't take them in time (it hangs), the launch opens its own window, as before, and the late
//!   request is turned down.
//! - Not on macOS (the Finder hands files to the app that runs, `open_documents`), nor for runs
//!   with a control channel (`--control`), without settings (`VECTORCRAFT_NO_PREFS`) or with
//!   `--new-window`.
//!
//! Nothing here panics: a failure only means the launch opens a window of its own.

use std::collections::VecDeque;
use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// Files later launches sent that haven't been opened yet, and whether one asked for the window.
static PENDING: Mutex<(Vec<String>, bool)> = Mutex::new((Vec::new(), false));
/// The requests already taken (their nonces), newest last: a launch that sends again isn't
/// opened twice.
static TAKEN: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
/// The UI to wake when files arrive (unset until the window exists; its first frame opens them).
static UI: OnceLock<egui::Context> = OnceLock::new();
/// The lock file, kept open (and locked) while this process runs.
static LOCK: OnceLock<std::fs::File> = OnceLock::new();
/// The app is closing: files are no longer taken (a later launch opens its own window).
static CLOSING: AtomicBool = AtomicBool::new(false);

/// Largest request read (bytes).
const MAX_REQUEST: usize = 1 << 20;
/// Most files one launch hands over.
const MAX_FILES: usize = 1000;
/// How long after its start a later launch may hand its files over (ms).
const HANDOVER_MS: u64 = 5000;
/// How long past that a launch waits for the answer to a request sent in time (ms).
const GRACE_MS: u64 = 1000;
/// How long the running app reads one request.
const READ_LIMIT: Duration = Duration::from_secs(2);
/// How many taken requests are remembered.
const TAKEN_KEPT: usize = 64;
const LOCK_FILE: &str = "instance.lock";
const INFO_FILE: &str = "instance.json";

/// What the launch does after [`start`].
#[derive(Debug, PartialEq, Eq)]
pub enum Start {
    /// Its files went to the app that runs: the launch ends.
    HandedOver,
    /// It opens the window (the first launch, or one that couldn't hand its files over).
    Window,
}

/// The running app's answer to a request.
#[derive(Debug, PartialEq, Eq)]
enum Reply {
    /// The files were taken (now, or by an earlier try of the same request).
    Taken,
    /// Turned down: the request came past its deadline.
    Late,
    /// The app is closing: its lock goes with it, for this launch to take.
    Closing,
}

/// Take the lock in `dir` (the settings folder) and listen, or hand `files` (as given on the
/// command line) to the app that holds it.
pub fn start(dir: &Path, files: &[String]) -> Start {
    if let Err(e) = std::fs::create_dir_all(dir) {
        log::warn!("single instance: no settings folder ({e}): this window opens on its own");
        return Start::Window;
    }
    let lock = match open_lock(dir) {
        Ok(f) => f,
        Err(e) => {
            log::warn!("single instance: can't open {LOCK_FILE} ({e}): this window opens on its own");
            return Start::Window;
        }
    };
    let files = absolute(files);
    let nonce = token();
    let deadline = now_ms().saturating_add(HANDOVER_MS);
    loop {
        match lock.try_lock() {
            Ok(()) => {
                // The first launch (or the app that held the lock has ended): this one listens.
                match serve(dir) {
                    // Kept for the life of the process: the lock goes when it ends, however it ends.
                    Ok(()) => {
                        let _ = LOCK.set(lock);
                    }
                    // Nobody can reach this window: let the next launch take the lock and listen,
                    // rather than wait for an answer that never comes (the lock goes with `lock`).
                    Err(e) => log::warn!("single instance: files opened later open in windows of their own ({e})"),
                }
                return Start::Window;
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                if let Some((port, token)) = read_info(dir) {
                    match send(port, &token, &files, &nonce, deadline) {
                        Ok(Reply::Taken) => return Start::HandedOver,
                        Ok(Reply::Late) => {
                            log::warn!("single instance: the VectorCraft that runs answered too late: this window opens on its own");
                            return Start::Window;
                        }
                        // Closing: its lock is let go of when it ends, and this launch takes it
                        // (later files then open here as tabs). Not listening yet (it is starting),
                        // or a port left by an earlier run: try again until the deadline.
                        Ok(Reply::Closing) | Err(_) => {}
                    }
                }
            }
            Err(std::fs::TryLockError::Error(e)) => {
                log::warn!("single instance: can't lock {LOCK_FILE} ({e}): this window opens on its own");
                return Start::Window;
            }
        }
        if now_ms() >= deadline {
            log::warn!("single instance: the VectorCraft that runs didn't take the files: this window opens on its own");
            return Start::Window;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The lock file in `dir`, this user's alone (another user holding it would keep every launch
/// waiting).
fn open_lock(dir: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.create(true).truncate(false).write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let lock = options.open(dir.join(LOCK_FILE))?;
    // A lock file left by an earlier version keeps its permissions on open. Not fatal: it only
    // matters when another user tries to hold it.
    #[cfg(unix)]
    let _ = lock.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600));
    Ok(lock)
}

/// Run `f` holding the lock in `dir` when no window of the app runs (`None` when one does): an
/// update replaces the app's files only then (`dancard_update`).
pub fn alone<R>(dir: &Path, f: impl FnOnce() -> R) -> Option<R> {
    std::fs::create_dir_all(dir).ok()?;
    let lock = open_lock(dir).ok()?;
    lock.try_lock().ok()?;
    let out = f();
    drop(lock);
    Some(out)
}

/// Wake `ctx` when files arrive while the app runs.
pub fn set_ui(ctx: &egui::Context) {
    // Set once; a second call would hand over the same context.
    let _ = UI.set(ctx.clone());
}

/// The files that arrived since the last call, and whether a launch asked for the window.
pub fn take() -> (Vec<String>, bool) {
    let mut p = PENDING.lock().unwrap_or_else(PoisonError::into_inner);
    (std::mem::take(&mut p.0), std::mem::take(&mut p.1))
}

/// The app is closing: later launches open their own window instead of handing files to it.
pub fn closing() {
    CLOSING.store(true, Ordering::SeqCst);
}

/// Milliseconds since the Unix epoch: launches on one machine share the clock, so a deadline one
/// sets holds in the other.
fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The paths of `files` from the current folder: the app they go to runs in another one.
fn absolute(files: &[String]) -> Vec<String> {
    files.iter().map(|f| std::path::absolute(f).map_or_else(|_| f.clone(), |p: PathBuf| p.to_string_lossy().into_owned())).collect()
}

/// A token no other launch can guess: the standard library's hash keys are drawn from the system's
/// random source for each process.
fn token() -> String {
    use std::hash::{BuildHasher as _, Hasher as _};
    let mut out = String::new();
    for salt in [0x5eed_u64, 0xdc4d] {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(salt);
        h.write_u64(now_ms());
        h.write_u32(std::process::id());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

/// Listen for later launches; `instance.json` says where.
fn serve(dir: &Path) -> std::io::Result<()> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let port = listener.local_addr()?.port();
    let token = token();
    let bytes = serde_json::to_vec(&json!({"port": port, "token": token})).map_err(std::io::Error::other)?;
    write_info(&dir.join(INFO_FILE), &bytes)?;
    std::thread::Builder::new().name("single-instance".into()).spawn(move || {
        for conn in listener.incoming() {
            // One launch's failure (it ended, sent rubbish, was slow) never stops the others.
            if let Ok(conn) = conn
                && let Err(e) = answer(conn, &token)
            {
                log::warn!("single instance: a launch's request wasn't taken ({e})");
            }
        }
    })?;
    Ok(())
}

/// `instance.json` written whole (a launch reading it never sees half of it), readable by this
/// user alone where files have owners' permissions (the token in it hands files to this app).
fn write_info(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut f = options.open(&tmp)?;
    // A file left by an earlier run keeps its permissions on open: set them again.
    #[cfg(unix)]
    f.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    f.write_all(bytes)?;
    drop(f);
    std::fs::rename(&tmp, path)
}

/// The port and token of the app that runs, from `instance.json`.
fn read_info(dir: &Path) -> Option<(u16, String)> {
    let bytes = std::fs::read(dir.join(INFO_FILE)).ok()?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    let port = u16::try_from(v.get("port")?.as_u64()?).ok().filter(|p| *p != 0)?;
    Some((port, v.get("token")?.as_str()?.to_string()))
}

/// One line from `conn` (without its end), read within [`READ_LIMIT`] in all and at most
/// [`MAX_REQUEST`] bytes: a launch that sends slowly or endlessly is cut off.
fn read_request(mut conn: &TcpStream) -> std::io::Result<String> {
    let until = Instant::now() + READ_LIMIT;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "the request took too long"));
        }
        conn.set_read_timeout(Some(left))?;
        let n = conn.read(&mut chunk)?;
        let Some(got) = chunk.get(..n) else { return Err(std::io::Error::other("read past the buffer")) };
        if let Some(end) = got.iter().position(|b| *b == b'\n') {
            buf.extend_from_slice(got.get(..end).unwrap_or_default());
            break;
        }
        if n == 0 {
            break;
        }
        buf.extend_from_slice(got);
        if buf.len() > MAX_REQUEST {
            return Err(std::io::Error::other("the request is too long"));
        }
    }
    String::from_utf8(buf).map_err(std::io::Error::other)
}

/// One later launch: `{"token", "files", "nonce", "deadline"}` on a line, answered `ok` when its
/// token is this run's and the files were taken, `late` past its deadline, `closing` while the
/// app closes.
fn answer(conn: TcpStream, token: &str) -> std::io::Result<()> {
    conn.set_write_timeout(Some(Duration::from_secs(2)))?;
    let line = read_request(&conn)?;
    let request: Value = serde_json::from_str(&line).map_err(std::io::Error::other)?;
    if request.get("token").and_then(Value::as_str) != Some(token) {
        return Err(std::io::Error::other("a request without this run's token"));
    }
    let nonce = request.get("nonce").and_then(Value::as_str).unwrap_or_default();
    let reply = if already_taken(nonce) {
        // A try of a request taken before (its answer was lost): taken, whatever the time.
        "ok"
    } else if CLOSING.load(Ordering::SeqCst) {
        "closing"
    } else if request.get("deadline").and_then(Value::as_u64).is_none_or(|d| now_ms() > d) {
        // That launch has opened, or is about to open, its own window.
        "late"
    } else {
        remember(nonce);
        let mut files: Vec<String> = request
            .get("files")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).filter(|f| !f.is_empty()).map(str::to_string).collect())
            .unwrap_or_default();
        if files.len() > MAX_FILES {
            log::warn!("single instance: {} files were opened at once: the first {MAX_FILES} open", files.len());
            files.truncate(MAX_FILES);
        }
        let mut p = PENDING.lock().unwrap_or_else(PoisonError::into_inner);
        p.0.extend(files);
        p.1 = true;
        drop(p);
        if let Some(ctx) = UI.get() {
            ctx.request_repaint();
        }
        "ok"
    };
    let mut conn = conn;
    conn.write_all(format!("{reply}\n").as_bytes())
}

/// The request `nonce` was taken before (an empty one never was).
fn already_taken(nonce: &str) -> bool {
    !nonce.is_empty() && TAKEN.lock().unwrap_or_else(PoisonError::into_inner).iter().any(|n| n == nonce)
}

/// Remember the request `nonce` as taken (the last [`TAKEN_KEPT`]).
fn remember(nonce: &str) {
    if nonce.is_empty() {
        return;
    }
    let mut taken = TAKEN.lock().unwrap_or_else(PoisonError::into_inner);
    if taken.len() >= TAKEN_KEPT {
        taken.pop_front();
    }
    taken.push_back(nonce.to_string());
}

/// Hand `files` to the app listening on `port` (request `nonce`, taken only before `deadline`, in
/// ms since the epoch); its answer is awaited until a little after the deadline.
fn send(port: u16, token: &str, files: &[String], nonce: &str, deadline: u64) -> std::io::Result<Reply> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(500))?;
    let wait = deadline.saturating_add(GRACE_MS).saturating_sub(now_ms()).max(100);
    s.set_read_timeout(Some(Duration::from_millis(wait)))?;
    s.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut line =
        serde_json::to_string(&json!({"token": token, "files": files, "nonce": nonce, "deadline": deadline})).map_err(std::io::Error::other)?;
    line.push('\n');
    s.write_all(line.as_bytes())?;
    let mut reply = [0u8; 16];
    let mut got = 0;
    // The answer is one short line; read until its end or the app hangs up.
    while got < reply.len() {
        let Some(rest) = reply.get_mut(got..) else { break };
        let n = s.read(rest)?;
        if n == 0 {
            break;
        }
        got += n;
        if reply.get(..got).is_some_and(|r| r.contains(&b'\n')) {
            break;
        }
    }
    match String::from_utf8_lossy(reply.get(..got).unwrap_or_default()).trim() {
        "ok" => Ok(Reply::Taken),
        "late" => Ok(Reply::Late),
        "closing" => Ok(Reply::Closing),
        _ => Err(std::io::Error::other("the app that runs didn't take the files")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A first launch listens; a later one hands its files over (made absolute) and the first
    /// takes them with the request for its window. The same request sent again isn't taken twice;
    /// one past its deadline, or without the token, isn't taken at all.
    #[test]
    fn a_later_launch_hands_its_files_to_the_first() {
        let dir = std::env::temp_dir().join(format!("vc-single-instance-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(start(&dir, &[]), Start::Window, "the first launch opens the window");
        let (port, token) = read_info(&dir).expect("instance.json");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(dir.join(INFO_FILE)).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "the token is this user's alone: {mode:o}");
        }
        // The lock is held: the next launch hands over (the same process stands in for it).
        assert_eq!(start(&dir, &["card.ai".to_string(), "/tmp/x/sheet.pdf".to_string()]), Start::HandedOver);
        let (files, focus) = take();
        assert!(focus, "the window is asked to come to the front");
        assert_eq!(files.len(), 2);
        assert!(Path::new(&files[0]).is_absolute() && files[0].ends_with("card.ai"), "{files:?}");
        assert!(files[1].ends_with("sheet.pdf"), "{files:?}");
        // A launch with no files only asks for the window.
        let later = now_ms() + HANDOVER_MS;
        assert_eq!(send(port, &token, &[], "n1", later).unwrap(), Reply::Taken);
        assert_eq!(take(), (vec![], true));
        // A request sent twice (the launch retried) is taken once.
        let one = vec!["a.ai".to_string()];
        assert_eq!(send(port, &token, &one, "n2", later).unwrap(), Reply::Taken);
        assert_eq!(send(port, &token, &one, "n2", later).unwrap(), Reply::Taken);
        assert_eq!(take(), (one.clone(), true));
        // Past its deadline: turned down, that launch opens its own window…
        assert_eq!(send(port, &token, &one, "n3", now_ms() - 1).unwrap(), Reply::Late);
        assert_eq!(take(), (vec![], false));
        // … but a request taken in time whose answer was lost is still "taken" when tried again late.
        assert_eq!(send(port, &token, &one, "n2", now_ms() - 1).unwrap(), Reply::Taken);
        assert_eq!(take(), (vec![], false));
        // Another token: nothing taken.
        assert!(send(port, "not-the-token", &one, "n4", later).is_err());
        assert_eq!(take(), (vec![], false));
        assert!(token.len() >= 32 && token != super::token());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A request sent a byte at a time, or never ended, is cut off: the next launch still gets in.
    #[test]
    fn a_slow_request_is_cut_off() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let slow = std::thread::spawn(move || {
            let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
            let _ = s.write_all(b"{\"token\"");
            std::thread::sleep(Duration::from_secs(3));
            let _ = s.write_all(b": \"x\"}\n");
        });
        let (conn, _) = listener.accept().unwrap();
        let started = Instant::now();
        assert!(read_request(&conn).is_err());
        assert!(started.elapsed() < Duration::from_millis(2800), "cut off at the limit: {:?}", started.elapsed());
        let _ = slow.join();
    }
}
