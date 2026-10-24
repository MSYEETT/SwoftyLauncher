//! Offline play: the launch extension that lets modded sessions start without
//! EA AntiCheat (Javelin).
//!
//! Modded sessions need in-process code, which Javelin blocks, and a game
//! started without the service aborts on its own launch check. The extension
//! (a separately distributed project) swaps the anti-cheat launcher for the
//! session and lets the game continue past that check. Sessions stay offline:
//! multiplayer is unavailable and cannot be enabled, because online play
//! requires the Javelin handshake with EA services.
//!
//! This module owns the policy around it:
//! - [consent]: the terms are shown and recorded once; without acceptance the
//!   manager refuses to launch the game modded.
//! - [install] / [fetch]: the extension is verified against the SHA-256 digests
//!   pinned here before anything is staged, so a tampered or lookalike download
//!   is refused however it arrived.
//! - [stage] / [restore]: session-scoped changes are journaled and returned to
//!   vanilla when the game exits, on `swofty restore`, and at the next start.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::app_control::Block;
use crate::baseline::{DepotHashes, sha256_file, sha256_opt};
use crate::{Error, Result, io_err, read_json, with_retry, write_json};

const EAC_EXE: &str = "EAAntiCheat.GameServiceLauncher.exe";
const EAC_BAK: &str = "EAAntiCheat.GameServiceLauncher.exe.bak";

/// The terms shown before consent and by `swofty offline accept`.
pub const DISCLOSURE: &str = "\
Offline play - read this before accepting

Modded sessions run F1 25 without EA AntiCheat so the loader and addons
can work. Everything is undone when the game exits.

- Playing without EA AntiCheat is outside EA's terms of service, even
  offline. We know of no action taken against players for offline modding,
  but EA can still act on accounts under its terms.
- Sessions run offline. Multiplayer is unavailable, and this component
  cannot enable it.
- Everything is scoped to the session: the game folder is restored to
  vanilla when the game exits, by `swofty restore`, and at the next start.
- The extension is downloaded from Swofty's site and verified against
  pinned SHA-256 digests before it is used. Source: Swofty-Developments/F1-OfflinePlay.

We may collect anonymised usage data from your use of the launcher.";

/// The extension release this manager pins.
pub const EXTENSION_VERSION: &str = "0.3.11";
/// Where the artifacts are hosted (binaries; source lives in the repository).
pub const DEFAULT_BASE_URL: &str = "https://swofty-developments.github.io/F1-OfflinePlay";
pub const SOURCE_URL: &str = "https://github.com/Swofty-Developments/F1-OfflinePlay";

/// Pinned digests for release v0.3.11. Every artifact is checked against its
/// pin at install time and again at use time.
pub const PINS: &[(&str, &str)] = &[
    (
        "f1op_hook.dll",
        "df644c8eb4d12e6eae666daed8e40060cc7aa036e40ca2b2215a9a3a55b02ed5",
    ),
    (
        "f1op_stub.exe",
        "b8f74f8345bef0c6308cba3c78bdd049c1a7d33ae6ba39ac651e7313d5d319cd",
    ),
    (
        "f1op-prelaunch.exe",
        "8a0df057d05ee7e5a0113713028ff4b1ddd4d134854c0bfa506894d19eeb5f6a",
    ),
    (
        "f1op-probe.exe",
        "9b3a72e693df750a411fcf05ff7263a017bce3ca1f266f12e3fc678606ab8dc2",
    ),
    (
        "f1op-prelaunch-linux-x86_64",
        "6a2bcbc0190f1c14a69590ef117974fa2c4562fe7cda25ab8e733417a12e2271",
    ),
    (
        "f1op-probe-linux-x86_64",
        "8c5a3353f38ddcb511f3502d7b6ee1032ede344c3a8db74165e46dce2b3cb9b8",
    ),
];

/// The artifacts this platform runs or stages.
#[cfg(windows)]
pub const REQUIRED: &[&str] = &[
    "f1op_hook.dll",
    "f1op_stub.exe",
    "f1op-prelaunch.exe",
    "f1op-probe.exe",
];
#[cfg(not(windows))]
pub const REQUIRED: &[&str] = &[
    "f1op_hook.dll",
    "f1op_stub.exe",
    "f1op-prelaunch-linux-x86_64",
    "f1op-probe-linux-x86_64",
];

fn pin(name: &str) -> Option<&'static str> {
    PINS.iter().find(|(n, _)| *n == name).map(|(_, d)| *d)
}

/// Where the verified extension is installed.
pub fn extension_dir(data: &Path) -> PathBuf {
    data.join("extensions/offline-play")
}

/// The staging journal for the current session; present only while staged.
pub fn journal_path(data: &Path) -> PathBuf {
    data.join("state/offline/staging.jsonl")
}

fn consent_path(data: &Path) -> PathBuf {
    data.join("state/offline_consent.json")
}

pub fn base_url() -> String {
    std::env::var("SWOFTY_OFFLINE_PLAY_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_owned())
}

// ---------------------------------------------------------------------------
// Consent
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Consent {
    pub schema: u32,
    pub text_sha256: String,
    pub accepted: bool,
    #[serde(default)]
    pub accepted_utc: String,
    #[serde(default)]
    pub extension_version: String,
}

pub fn consent(data: &Path) -> Option<Consent> {
    read_json(&consent_path(data)).ok()
}

/// True when the terms were accepted and the text has not changed since.
pub fn is_accepted(data: &Path) -> bool {
    consent(data)
        .is_some_and(|c| c.accepted && c.text_sha256 == crate::baseline::sha256_bytes(DISCLOSURE.as_bytes()))
}

pub fn record_accept(data: &Path) -> Result<Consent> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let c = Consent {
        schema: 1,
        text_sha256: crate::baseline::sha256_bytes(DISCLOSURE.as_bytes()),
        accepted: true,
        accepted_utc: secs.to_string(),
        extension_version: EXTENSION_VERSION.into(),
    };
    write_json(&consent_path(data), &c)?;
    Ok(c)
}

pub fn withdraw(data: &Path) -> Result<()> {
    let mut c = consent(data).unwrap_or(Consent {
        schema: 1,
        text_sha256: String::new(),
        accepted: false,
        accepted_utc: String::new(),
        extension_version: String::new(),
    });
    c.accepted = false;
    write_json(&consent_path(data), &c)
}

// ---------------------------------------------------------------------------
// Extension artifacts: install, fetch, verify
// ---------------------------------------------------------------------------

/// Verified artifact paths inside the extension directory.
#[derive(Debug, Clone)]
pub struct Artifacts {
    pub dir: PathBuf,
    pub hook: PathBuf,
    pub stub: PathBuf,
    pub prelaunch: PathBuf,
    pub probe: PathBuf,
}

/// The two artifacts the manager runs: (prelaunch, probe).
#[cfg(windows)]
const RUNNERS: [&str; 2] = ["f1op-prelaunch.exe", "f1op-probe.exe"];
#[cfg(not(windows))]
const RUNNERS: [&str; 2] = ["f1op-prelaunch-linux-x86_64", "f1op-probe-linux-x86_64"];

fn verify_digest(path: &Path, name: &str, want: &str) -> Result<()> {
    let have = sha256_file(path)?;
    if have != want {
        return Err(Error::Other(format!(
            "{name}: digest mismatch (have {have}, want {want}); refusing to use it"
        )));
    }
    Ok(())
}

fn verify_file(path: &Path, name: &str) -> Result<()> {
    let want = pin(name).ok_or_else(|| Error::Other(format!("no pinned digest for {name}")))?;
    verify_digest(path, name, want)
}

/// The files this platform needs, each with its pinned digest.
fn pinned_files() -> Result<Vec<(&'static str, &'static str)>> {
    REQUIRED
        .iter()
        .map(|name| {
            pin(name)
                .map(|digest| (*name, digest))
                .ok_or_else(|| Error::Other(format!("no pinned digest for {name}")))
        })
        .collect()
}

/// Downloads and copies do not carry the execute bit, and the runners need it
/// on Unix.
fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).map_err(io_err(path))?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).map_err(io_err(path))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Re-verify every required artifact before use; tampering after install is
/// caught here.
pub fn artifacts(data: &Path) -> Result<Artifacts> {
    let dir = extension_dir(data);
    for name in REQUIRED {
        let path = dir.join(name);
        if !path.is_file() {
            return Err(Error::Other(format!(
                "offline play extension is incomplete: {} is missing",
                path.display()
            )));
        }
        verify_file(&path, name)?;
    }
    let [prelaunch, probe] = RUNNERS;
    let out = Artifacts {
        hook: dir.join("f1op_hook.dll"),
        stub: dir.join("f1op_stub.exe"),
        prelaunch: dir.join(prelaunch),
        probe: dir.join(probe),
        dir,
    };
    // Re-applied on every use so an older install without the bit heals
    // itself without being downloaded again.
    set_executable(&out.prelaunch)?;
    set_executable(&out.probe)?;
    Ok(out)
}

/// Where a release is assembled before it replaces the installed one. It sits
/// next to the extension directory so the final rename stays on one
/// filesystem.
fn staging_dir(data: &Path) -> PathBuf {
    data.join("extensions/.offline-play.partial")
}

fn fresh_dir(dir: &Path) -> Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(io_err(dir))?;
    }
    std::fs::create_dir_all(dir).map_err(io_err(dir))
}

/// Verify a fully assembled release in the staging directory, then swap it in
/// for the installed one. Until the swap the installed copy is untouched; a
/// crash between the two renames leaves no extension directory, which reads
/// as "not installed" and is fetched again.
fn commit(data: &Path, staging: &Path, files: &[(&str, &str)]) -> Result<()> {
    for (name, digest) in files {
        verify_digest(&staging.join(name), name, digest)?;
    }
    for name in RUNNERS {
        let path = staging.join(name);
        if path.is_file() {
            set_executable(&path)?;
        }
    }
    let dir = extension_dir(data);
    let old = data.join("extensions/.offline-play.old");
    if old.exists() {
        std::fs::remove_dir_all(&old).map_err(io_err(&old))?;
    }
    if dir.exists() {
        with_retry(&dir, || std::fs::rename(&dir, &old))?;
    }
    if let Err(e) = with_retry(&dir, || std::fs::rename(staging, &dir)) {
        if old.exists() {
            std::fs::rename(&old, &dir).ok();
        }
        return Err(e);
    }
    std::fs::remove_dir_all(&old).ok();
    Ok(())
}

/// Verify a downloaded release folder against the pins and install it.
pub fn install(data: &Path, src: &Path) -> Result<Artifacts> {
    install_files(data, src, &pinned_files()?)?;
    artifacts(data)
}

fn install_files(data: &Path, src: &Path, files: &[(&str, &str)]) -> Result<()> {
    let staging = staging_dir(data);
    fresh_dir(&staging)?;
    let done = files
        .iter()
        .try_for_each(|(name, _)| {
            let from = src.join(name);
            if !from.is_file() {
                return Err(Error::Other(format!("{name}: not found in {}", src.display())));
            }
            let to = staging.join(name);
            std::fs::copy(&from, &to).map(|_| ()).map_err(io_err(&to))
        })
        .and_then(|()| commit(data, &staging, files));
    std::fs::remove_dir_all(&staging).ok();
    done
}

/// Download the pinned release from the artifact host and install it. The
/// digests decide what is trusted; the host is only a transport.
pub fn fetch(data: &Path) -> Result<Artifacts> {
    fetch_release(data, &base_url(), EXTENSION_VERSION, &pinned_files()?)?;
    artifacts(data)
}

/// Download every file into the staging directory, verify each one, and only
/// then replace the installed extension. Any failure leaves the installed copy
/// as it was and names the file and the reason.
fn fetch_release(data: &Path, base: &str, version: &str, files: &[(&str, &str)]) -> Result<()> {
    // Earlier launchers downloaded into this folder and could leave it behind.
    std::fs::remove_dir_all(data.join("extensions/.download")).ok();
    let staging = staging_dir(data);
    fresh_dir(&staging)?;
    let done = files
        .iter()
        .try_for_each(|(name, digest)| {
            let url = format!("{}/v{version}/{name}", base.trim_end_matches('/'));
            let to = staging.join(name);
            download(&url, &to).map_err(|e| Error::Other(format!("offline play {version}: {e}")))?;
            verify_digest(&to, name, digest)
                .map_err(|e| Error::Other(format!("offline play {version}: {url}: {e}")))
        })
        .and_then(|()| commit(data, &staging, files));
    std::fs::remove_dir_all(&staging).ok();
    done
}

fn download(url: &str, to: &Path) -> std::result::Result<(), String> {
    let resp = ureq::get(url)
        .config()
        .timeout_connect(Some(std::time::Duration::from_secs(30)))
        .timeout_global(Some(std::time::Duration::from_secs(300)))
        .http_status_as_error(false)
        .build()
        .call()
        .map_err(|e| format!("could not download {url}: {e}"))?;
    let status = resp.status();
    if status == 404 {
        return Err(format!(
            "{url}: HTTP {status}: the release on the host has no such file"
        ));
    }
    if !status.is_success() {
        return Err(format!("{url}: HTTP {status}"));
    }
    let expected = resp.body().content_length();
    let mut body = Vec::new();
    let read = std::io::Read::read_to_end(&mut resp.into_body().into_reader(), &mut body);
    if let Err(e) = read {
        return Err(format!(
            "{url}: the download was cut off after {} bytes: {e}",
            body.len()
        ));
    }
    if let Some(n) = expected
        && body.len() as u64 != n
    {
        return Err(format!(
            "{url}: the download was cut off ({} of {n} bytes)",
            body.len()
        ));
    }
    std::fs::write(to, &body).map_err(|e| format!("{}: {e}", to.display()))
}

// ---------------------------------------------------------------------------
// Running the extension's tools
// ---------------------------------------------------------------------------

/// How a runner is started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exec {
    /// Executed as is.
    Direct,
    /// The binary names a program loader (ELF interpreter) this system does
    /// not have, so it runs through the system's copy of that loader.
    Loader { wanted: PathBuf, system: PathBuf },
}

/// Where Linux distributions keep the program loader.
#[cfg(target_os = "linux")]
const LOADER_DIRS: &[&str] = &[
    "/lib64",
    "/usr/lib64",
    "/lib/x86_64-linux-gnu",
    "/usr/lib/x86_64-linux-gnu",
    "/lib",
    "/usr/lib",
];

/// Decide how to start a runner, refusing one that is missing or cannot start
/// here. A Linux binary whose ELF interpreter does not exist fails to exec with
/// "No such file or directory" even though the file is present; it is run
/// through the system loader instead when there is one.
pub fn exec_mode(program: &Path) -> Result<Exec> {
    #[cfg(target_os = "linux")]
    let dirs: Vec<&Path> = LOADER_DIRS.iter().map(Path::new).collect();
    #[cfg(not(target_os = "linux"))]
    let dirs: Vec<&Path> = Vec::new();
    exec_mode_in(program, &dirs)
}

fn exec_mode_in(program: &Path, loader_dirs: &[&Path]) -> Result<Exec> {
    if !program.is_file() {
        return Err(Error::Other(format!(
            "{} is missing; reinstall the extension with `swofty offline fetch`",
            program.display()
        )));
    }
    if !cfg!(target_os = "linux") {
        return Ok(Exec::Direct);
    }
    let Some(wanted) = elf_interpreter(program)? else {
        return Ok(Exec::Direct);
    };
    if wanted.exists() {
        return Ok(Exec::Direct);
    }
    let system = wanted
        .file_name()
        .and_then(|name| loader_dirs.iter().map(|d| d.join(name)).find(|p| p.is_file()));
    match system {
        Some(system) => Ok(Exec::Loader { wanted, system }),
        None => Err(Error::Other(format!(
            "{} cannot run on this system: it was built to start through {}, which does not \
             exist here, and no system copy of that loader was found. This is a packaging \
             problem in the offline play {EXTENSION_VERSION} Linux build, not in your install.",
            program.display(),
            wanted.display()
        ))),
    }
}

/// The PT_INTERP path of a 64-bit little-endian ELF file, or None for a static
/// binary or anything that is not one.
fn elf_interpreter(path: &Path) -> Result<Option<PathBuf>> {
    const PT_INTERP: usize = 3;
    let data = std::fs::read(path).map_err(io_err(path))?;
    if data.get(..6) != Some(b"\x7fELF\x02\x01".as_slice()) {
        return Ok(None);
    }
    let le = |at: usize, len: usize| -> Option<usize> {
        let bytes = data.get(at..at.checked_add(len)?)?;
        let value = bytes.iter().rev().fold(0u64, |v, b| (v << 8) | u64::from(*b));
        usize::try_from(value).ok()
    };
    let (Some(phoff), Some(phentsize), Some(phnum)) = (le(0x20, 8), le(0x36, 2), le(0x38, 2)) else {
        return Ok(None);
    };
    for i in 0..phnum {
        let Some(ph) = i.checked_mul(phentsize).and_then(|o| o.checked_add(phoff)) else {
            return Ok(None);
        };
        if le(ph, 4) != Some(PT_INTERP) {
            continue;
        }
        let interp = (|| {
            let offset = le(ph.checked_add(0x08)?, 8)?;
            let size = le(ph.checked_add(0x20)?, 8)?;
            data.get(offset..offset.checked_add(size)?)
        })();
        return Ok(interp.map(|raw| PathBuf::from(String::from_utf8_lossy(raw).trim_end_matches('\0'))));
    }
    Ok(None)
}

fn command(program: &Path) -> Result<Command> {
    Ok(command_for(program, exec_mode(program)?))
}

fn command_for(program: &Path, exec: Exec) -> Command {
    match exec {
        Exec::Direct => Command::new(program),
        Exec::Loader { system, .. } => {
            let mut cmd = Command::new(system);
            cmd.arg(program);
            cmd
        }
    }
}

/// A tool that would not start; one Windows refused says what blocked it.
fn start_error(program: &Path, e: std::io::Error) -> Error {
    match Block::of(program, &e) {
        Some(b) => Error::Blocked(b),
        None => Error::Other(format!("could not start {}: {e}", program.display())),
    }
}

/// Explain an exit that came from the system loader rather than the tool: a C
/// library older than the build needs, or a missing shared library.
fn loader_hint(program: &Path, status: std::process::ExitStatus) -> &'static str {
    let via_loader = matches!(exec_mode(program), Ok(Exec::Loader { .. }));
    if via_loader && status.code() == Some(127) {
        " (the system loader could not start it: this system's libraries are older than the \
         offline play Linux build needs)"
    } else {
        ""
    }
}

// ---------------------------------------------------------------------------
// Staging
// ---------------------------------------------------------------------------

/// Result of probing the game build for support.
#[derive(Debug, Clone)]
pub struct Probe {
    pub supported: bool,
    pub report: String,
}

pub fn probe_path(data: &Path) -> PathBuf {
    data.join("state/offline/probe.json")
}

fn probe_key(exe: &Path) -> Option<String> {
    let meta = std::fs::metadata(exe).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some(format!(
        "{EXTENSION_VERSION}|{}|{}|{modified}",
        exe.display(),
        meta.len()
    ))
}

/// [`probe`], skipped when the same extension release already found this
/// exact game executable supported.
pub fn probe_cached(data: &Path, game: &Path, a: &Artifacts) -> Result<Probe> {
    let key = probe_key(&game.join(crate::GAME_EXE));
    let path = probe_path(data);
    if let Some(key) = &key
        && let Ok((seen, report)) = crate::read_json::<(String, String)>(&path)
        && seen == *key
    {
        return Ok(Probe {
            supported: true,
            report,
        });
    }
    let found = probe(game, a)?;
    if found.supported
        && let Some(key) = key
    {
        crate::write_json(&path, &(key, &found.report)).ok();
    }
    Ok(found)
}

/// Ask the extension whether this game build is supported (fail closed).
pub fn probe(game: &Path, a: &Artifacts) -> Result<Probe> {
    let out = command(&a.probe)?
        .arg(game.join(crate::GAME_EXE))
        .output()
        .map_err(|e| start_error(&a.probe, e))?;
    let report = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    match out.status.code() {
        Some(0) => Ok(Probe {
            supported: true,
            report,
        }),
        Some(1) => Ok(Probe {
            supported: false,
            report,
        }),
        code => Err(Error::Other(format!(
            "probe failed ({code:?}): {}{}",
            String::from_utf8_lossy(&out.stderr).trim(),
            loader_hint(&a.probe, out.status)
        ))),
    }
}

/// Repair a half-staged game folder left by an interrupted session, so a new
/// session can start without anyone digging files out by hand. The backup is
/// put back in place; whatever sat in the launcher's slot is kept under
/// swofty/offline-recovery/ rather than destroyed. Returns what it did.
pub fn heal_stale_backup(game: &Path, journal: &Path) -> Result<Vec<String>> {
    let mut notes = Vec::new();
    if journal.exists() {
        match restore(journal, false) {
            Ok(done) => notes.extend(done),
            Err(e) => notes.push(format!(
                "a leftover journal could not be replayed ({e}); recovered from the backup instead"
            )),
        }
        std::fs::remove_file(journal).map_err(io_err(journal))?;
    }
    notes.extend(heal_launcher(game, None)?);
    // A hook left in the loader's slot is ours: the journal is what normally
    // owns it, and staging cannot drop the new one while it is there.
    let hook = game.join("swofty/mods/core/f1op_hook.dll");
    if hook.exists() {
        with_retry(&hook, || std::fs::remove_file(&hook))?;
        notes.push("removed a stale hook from a previous session".into());
    }
    Ok(notes)
}

/// Clear a stale anti-cheat backup before a build's stock check, so a leftover
/// `.bak` of the wrong size can't block every launch. It does nothing while a
/// session is pending ([`restore_pending`] owns the launcher then). With a
/// depot list it keeps a Steam-verified live exe and sets the stale backup
/// aside; without one it keeps today's restore-from-backup behaviour. Returns
/// what it did.
pub fn heal_before_check(data: &Path, game: &Path, depot: Option<&dyn DepotHashes>) -> Result<Vec<String>> {
    if journal_path(data).exists() {
        return Ok(Vec::new());
    }
    heal_launcher(game, depot)
}

fn set_aside(game: &Path, file: &Path) -> Result<PathBuf> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let keep = game
        .join("swofty/offline-recovery")
        .join(format!("{name}.{secs}"));
    if let Some(parent) = keep.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_err(parent.to_path_buf())(e))?;
    }
    with_retry(file, || std::fs::rename(file, &keep))?;
    Ok(keep)
}

fn heal_launcher(game: &Path, depot: Option<&dyn DepotHashes>) -> Result<Vec<String>> {
    let mut notes = Vec::new();
    let launcher = game.join(EAC_EXE);
    let parked = game.join(EAC_BAK);
    if !parked.exists() {
        return Ok(notes);
    }
    let backup = sha256_file(&parked)?;
    if sha256_opt(&launcher)?.as_deref() == Some(backup.as_str()) {
        with_retry(&parked, || std::fs::remove_file(&parked))?;
        notes.push("removed a stale backup identical to the launcher".into());
        return Ok(notes);
    }
    let live_is_stock = match depot {
        Some(d) if launcher.exists() => d.stock(EAC_EXE, &launcher)?,
        _ => None,
    };
    let bak_is_stock = match depot {
        Some(d) => d.stock(EAC_EXE, &parked)?,
        None => None,
    };
    if live_is_stock == Some(true) && bak_is_stock != Some(true) {
        let keep = set_aside(game, &parked)?;
        notes.push(format!(
            "set a stale anti-cheat backup aside at {}",
            keep.display()
        ));
        return Ok(notes);
    }
    if depot.is_some() && bak_is_stock != Some(true) {
        return Ok(notes);
    }
    if launcher.exists() {
        let keep = set_aside(game, &launcher)?;
        notes.push(format!("kept the staged file at {}", keep.display()));
    }
    with_retry(&launcher, || std::fs::rename(&parked, &launcher))?;
    notes.push("restored EAAntiCheat.GameServiceLauncher.exe from its backup".into());
    Ok(notes)
}

/// Stage the session: park the anti-cheat launcher, drop the stub, and place
/// the hook where the loader picks it up. Everything goes in the journal.
/// Repairs leftovers from interrupted sessions first; returns what it did.
pub fn stage(game: &Path, a: &Artifacts, journal: &Path) -> Result<Vec<String>> {
    // The journal lives under state/offline/, which nothing else creates.
    if let Some(parent) = journal.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_err(parent.to_path_buf())(e))?;
    }
    let mut notes = heal_stale_backup(game, journal)?;
    let hook_dest = game.join("swofty/mods/core/f1op_hook.dll");
    let out = command(&a.prelaunch)?
        .arg("stage")
        .arg("--game-dir")
        .arg(game)
        .arg("--payload")
        .arg(&a.stub)
        .arg("--hook")
        .arg(&a.hook)
        .arg("--hook-dest")
        .arg(&hook_dest)
        .arg("--journal")
        .arg(journal)
        .output()
        .map_err(|e| start_error(&a.prelaunch, e))?;
    if !out.status.success() {
        return Err(Error::Other(format!(
            "offline play staging failed: {}{}",
            String::from_utf8_lossy(&out.stderr).trim(),
            loader_hint(&a.prelaunch, out.status)
        )));
    }
    notes.push("staged offline play for this session".into());
    Ok(notes)
}

// ---------------------------------------------------------------------------
// Restore: the manager owns this, so `swofty restore` works even when the
// extension is not installed. The journal format is documented in the
// extension's docs/journal.md.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct JournalLine {
    op: String,
    #[serde(default)]
    from: String,
    #[serde(default)]
    to: String,
    #[serde(default)]
    sha256: String,
}

/// Replay the journal backwards, verifying digests before touching anything.
/// Returns one line per action for logging.
pub fn restore(journal: &Path, force: bool) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(journal).map_err(io_err(journal))?;
    let mut lines = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        lines.push(
            serde_json::from_str::<JournalLine>(line)
                .map_err(|e| Error::Other(format!("{}: {e}", journal.display())))?,
        );
    }
    // Best effort: every entry is attempted so one locked or changed file
    // never strands the rest half-modded. The all-important launcher rename
    // still happens even if a stub or hook delete is blocked. Transient locks
    // are retried; anything left is reported together at the end so `swofty
    // restore` can be re-run once the locker (usually an antivirus) lets go.
    let mut done = Vec::new();
    let mut failures = Vec::new();
    for e in lines.iter().rev() {
        match e.op.as_str() {
            "meta" => {}
            "copy" => {
                let to = Path::new(&e.to);
                if !to.exists() {
                    continue;
                }
                if !force {
                    match sha256_file(to) {
                        Ok(have) if have != e.sha256 => {
                            failures.push(format!(
                                "{}: changed since staging (digest mismatch); left in place",
                                e.to
                            ));
                            continue;
                        }
                        Ok(_) => {}
                        Err(err) => {
                            failures.push(err.to_string());
                            continue;
                        }
                    }
                }
                match with_retry(to, || std::fs::remove_file(to)) {
                    Ok(()) => done.push(format!("removed {}", e.to)),
                    Err(err) => failures.push(err.to_string()),
                }
            }
            "rename" => {
                let (from, to) = (Path::new(&e.from), Path::new(&e.to));
                if !to.exists() {
                    failures.push(format!("staged backup is missing: {}", e.to));
                    continue;
                }
                if from.exists() && !force {
                    failures.push(format!("{}: already exists; left as is", e.from));
                    continue;
                }
                match with_retry(from, || std::fs::rename(to, from)) {
                    Ok(()) => done.push(format!("restored {}", e.from)),
                    Err(err) => failures.push(err.to_string()),
                }
            }
            other => {
                failures.push(format!("unknown staging journal op: {other} (update swofty)"));
            }
        }
    }
    if !failures.is_empty() {
        return Err(Error::Other(format!(
            "offline-play restore could not finish cleanly; re-run `swofty restore` once \
             anything locking the files is closed:\n  {}",
            failures.join("\n  ")
        )));
    }
    Ok(done)
}

/// Restore any pending staging (crash recovery, `swofty restore`). Returns
/// true when something was restored.
pub fn restore_pending(data: &Path) -> Result<bool> {
    let journal = journal_path(data);
    if !journal.exists() {
        return Ok(false);
    }
    restore(&journal, false)?;
    std::fs::remove_file(&journal).map_err(io_err(&journal))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("swofty-offline-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[cfg(unix)]
    #[test]
    fn a_supported_build_is_probed_once_until_the_exe_changes() {
        use std::os::unix::fs::PermissionsExt;
        let d = temp("probe-cache");
        let (data, game) = (d.join("data"), d.join("game"));
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(game.join(crate::GAME_EXE), "exe").unwrap();
        let runs = d.join("runs");
        let probe = d.join("probe.sh");
        std::fs::write(
            &probe,
            format!(
                "#!/bin/sh\necho run >> '{}'\necho supported build\n",
                runs.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let a = Artifacts {
            dir: d.clone(),
            hook: d.join("hook"),
            stub: d.join("stub"),
            prelaunch: d.join("prelaunch"),
            probe,
        };
        let count = || std::fs::read_to_string(&runs).unwrap_or_default().lines().count();
        for _ in 0..2 {
            let p = probe_cached(&data, &game, &a).unwrap();
            assert!(p.supported);
            assert_eq!(p.report, "supported build");
        }
        assert_eq!(count(), 1);
        std::fs::write(game.join(crate::GAME_EXE), "updated exe").unwrap();
        assert!(probe_cached(&data, &game, &a).unwrap().supported);
        assert_eq!(count(), 2);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn consent_lifecycle() {
        let d = temp("consent");
        assert!(!is_accepted(&d));
        record_accept(&d).unwrap();
        assert!(is_accepted(&d));
        withdraw(&d).unwrap();
        assert!(!is_accepted(&d));
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn consent_follows_the_disclosure_text() {
        let d = temp("text");
        record_accept(&d).unwrap();
        let mut c = consent(&d).unwrap();
        c.text_sha256 = "old".into();
        write_json(&consent_path(&d), &c).unwrap();
        assert!(!is_accepted(&d), "changed terms must be accepted again");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn install_rejects_wrong_digests() {
        let d = temp("pins");
        let src = d.join("src");
        std::fs::create_dir_all(&src).unwrap();
        for name in REQUIRED {
            std::fs::write(src.join(name), "not the real thing").unwrap();
        }
        let err = install(&d, &src).unwrap_err().to_string();
        assert!(err.contains("digest mismatch"), "{err}");
        assert!(!extension_dir(&d).join("f1op_hook.dll").exists());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn install_from_a_folder_leaves_no_staging_behind() {
        let d = temp("install-ok");
        let src = d.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let release = fake_release();
        for (name, body) in &release {
            std::fs::write(src.join(name), body).unwrap();
        }
        let pins = digests(&release);
        let pins: Vec<(&str, &str)> = pins.iter().map(|(n, h)| (*n, h.as_str())).collect();
        install_files(&d, &src, &pins).unwrap();
        for (name, body) in &release {
            assert_eq!(
                std::fs::read(extension_dir(&d).join(name)).unwrap(),
                body.as_bytes()
            );
        }
        assert!(!staging_dir(&d).exists());
        std::fs::remove_dir_all(&d).ok();
    }

    enum Reply {
        Body(Vec<u8>),
        Status(u16),
        CutOff { claimed: usize, sent: Vec<u8> },
    }

    fn serve(routes: Vec<(String, Reply)>) -> String {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                reader.read_line(&mut request).ok();
                let path = request.split_whitespace().nth(1).unwrap_or_default().to_owned();
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).unwrap_or(0) <= 2 {
                        break;
                    }
                }
                let (status, length, body) = match routes.iter().find(|(p, _)| *p == path) {
                    Some((_, Reply::Body(b))) => (200, b.len(), b.clone()),
                    Some((_, Reply::Status(code))) => (*code, 0, Vec::new()),
                    Some((_, Reply::CutOff { claimed, sent })) => (200, *claimed, sent.clone()),
                    None => (404, 0, Vec::new()),
                };
                let head =
                    format!("HTTP/1.1 {status} X\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n");
                stream.write_all(head.as_bytes()).ok();
                stream.write_all(&body).ok();
            }
        });
        base
    }

    fn fake_release() -> Vec<(&'static str, String)> {
        REQUIRED.iter().map(|n| (*n, format!("fake {n}"))).collect()
    }

    fn digests(release: &[(&'static str, String)]) -> Vec<(&'static str, String)> {
        release
            .iter()
            .map(|(n, b)| (*n, crate::baseline::sha256_bytes(b.as_bytes())))
            .collect()
    }

    fn fetch_fake(d: &Path, base: &str, release: &[(&'static str, String)]) -> Result<()> {
        let pins = digests(release);
        let pins: Vec<(&str, &str)> = pins.iter().map(|(n, h)| (*n, h.as_str())).collect();
        fetch_release(d, base, "9.9.9", &pins)
    }

    fn routes(
        release: &[(&'static str, String)],
        mut tweak: impl FnMut(&str) -> Option<Reply>,
    ) -> Vec<(String, Reply)> {
        release
            .iter()
            .map(|(n, b)| {
                let reply = tweak(n).unwrap_or_else(|| Reply::Body(b.clone().into_bytes()));
                (format!("/v9.9.9/{n}"), reply)
            })
            .collect()
    }

    fn previous_install(d: &Path) -> PathBuf {
        let marker = extension_dir(d).join("previous.txt");
        std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
        std::fs::write(&marker, "previous install").unwrap();
        marker
    }

    fn assert_untouched(d: &Path, marker: &Path) {
        assert_eq!(std::fs::read_to_string(marker).unwrap(), "previous install");
        assert!(!extension_dir(d).join(REQUIRED[0]).exists());
        assert!(!staging_dir(d).exists(), "the partial download is cleaned up");
    }

    #[test]
    fn fetch_installs_atomically_over_an_incomplete_copy() {
        let d = temp("fetch-ok");
        let release = fake_release();
        let ext = extension_dir(&d);
        std::fs::create_dir_all(&ext).unwrap();
        std::fs::write(ext.join(REQUIRED[0]), "half an old install").unwrap();
        std::fs::write(ext.join("stale.bin"), "junk").unwrap();
        std::fs::create_dir_all(d.join("extensions/.download")).unwrap();

        let base = serve(routes(&release, |_| None));
        fetch_fake(&d, &base, &release).unwrap();

        for (name, body) in &release {
            assert_eq!(std::fs::read_to_string(ext.join(name)).unwrap(), *body);
        }
        assert!(
            !ext.join("stale.bin").exists(),
            "the old copy is replaced, not merged"
        );
        assert!(!staging_dir(&d).exists());
        assert!(!d.join("extensions/.offline-play.old").exists());
        assert!(!d.join("extensions/.download").exists());
        #[cfg(unix)]
        for runner in RUNNERS {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(ext.join(runner)).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "{runner} is executable");
        }
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn fetch_names_a_file_missing_from_the_release() {
        let d = temp("fetch-404");
        let marker = previous_install(&d);
        let release = fake_release();
        let missing = RUNNERS[1];
        let base = serve(routes(&release, |n| (n == missing).then_some(Reply::Status(404))));
        let err = fetch_fake(&d, &base, &release).unwrap_err().to_string();
        assert!(err.contains(missing), "{err}");
        assert!(err.contains("HTTP 404"), "{err}");
        assert!(err.contains("has no such file"), "{err}");
        assert_untouched(&d, &marker);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn fetch_reports_a_server_error() {
        let d = temp("fetch-503");
        let marker = previous_install(&d);
        let release = fake_release();
        let base = serve(routes(&release, |n| {
            (n == REQUIRED[2]).then_some(Reply::Status(503))
        }));
        let err = fetch_fake(&d, &base, &release).unwrap_err().to_string();
        assert!(err.contains("HTTP 503"), "{err}");
        assert_untouched(&d, &marker);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn fetch_reports_a_cut_off_download() {
        let d = temp("fetch-partial");
        let marker = previous_install(&d);
        let release = fake_release();
        let last = REQUIRED[REQUIRED.len() - 1];
        let base = serve(routes(&release, |n| {
            (n == last).then(|| Reply::CutOff {
                claimed: 4096,
                sent: b"only the start".to_vec(),
            })
        }));
        let err = fetch_fake(&d, &base, &release).unwrap_err().to_string();
        assert!(err.contains(last), "{err}");
        assert!(err.contains("cut off"), "{err}");
        assert_untouched(&d, &marker);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn fetch_refuses_a_digest_mismatch() {
        let d = temp("fetch-digest");
        let marker = previous_install(&d);
        let release = fake_release();
        let base = serve(routes(&release, |n| {
            (n == REQUIRED[1]).then(|| Reply::Body(b"<html>captive portal</html>".to_vec()))
        }));
        let err = fetch_fake(&d, &base, &release).unwrap_err().to_string();
        assert!(err.contains("digest mismatch"), "{err}");
        assert!(err.contains(&format!("/v9.9.9/{}", REQUIRED[1])), "{err}");
        assert_untouched(&d, &marker);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn fetch_reports_an_unreachable_host() {
        let d = temp("fetch-refused");
        let marker = previous_install(&d);
        // Port 1 is below every system's ephemeral range, so no other test's server
        // can be listening there; a port freed by a bind could be taken in between.
        let err = fetch_fake(&d, "http://127.0.0.1:1", &fake_release())
            .unwrap_err()
            .to_string();
        assert!(err.contains("could not download"), "{err}");
        assert_untouched(&d, &marker);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_missing_runner_is_never_started() {
        let d = temp("no-runner");
        let a = Artifacts {
            hook: d.join("f1op_hook.dll"),
            stub: d.join("f1op_stub.exe"),
            prelaunch: d.join(RUNNERS[0]),
            probe: d.join(RUNNERS[1]),
            dir: d.clone(),
        };
        let err = probe(&d, &a).unwrap_err().to_string();
        assert!(err.contains("is missing"), "{err}");
        assert!(err.contains("swofty offline fetch"), "{err}");
        let err = stage(&d.join("game"), &a, &d.join("state/offline/staging.jsonl"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("is missing"), "{err}");
        std::fs::remove_dir_all(&d).ok();
    }

    /// A copy of this test binary whose ELF interpreter points at a loader
    /// that does not exist: what a Linux build linked on NixOS looks like on a
    /// Steam Deck. Returns the copy and the real loader.
    #[cfg(target_os = "linux")]
    fn binary_with_a_missing_loader(d: &Path) -> Option<(PathBuf, PathBuf)> {
        let exe = std::env::current_exe().unwrap();
        let real = elf_interpreter(&exe).unwrap()?;
        let name = real.file_name()?.to_str()?.to_owned();
        let dir_len = real.to_str()?.len() - name.len() - 1;
        let fake = format!("/{}/{name}", "z".repeat(dir_len.checked_sub(1)?));
        let mut data = std::fs::read(&exe).unwrap();
        let at = data
            .windows(real.as_os_str().len())
            .position(|w| w == real.as_os_str().as_encoded_bytes())?;
        data[at..at + fake.len()].copy_from_slice(fake.as_bytes());
        let copy = d.join("runner");
        std::fs::write(&copy, data).unwrap();
        set_executable(&copy).unwrap();
        Some((copy, real))
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_runner_whose_loader_is_missing_runs_through_the_system_loader() {
        let d = temp("loader");
        let Some((runner, real)) = binary_with_a_missing_loader(&d) else {
            eprintln!("skipping: the test binary is statically linked");
            return;
        };
        let direct = Command::new(&runner).arg("--list").output().unwrap_err();
        assert_eq!(
            direct.kind(),
            std::io::ErrorKind::NotFound,
            "the Steam Deck symptom"
        );

        let dirs = [real.parent().unwrap()];
        let exec = exec_mode_in(&runner, &dirs).unwrap();
        assert_eq!(
            exec,
            Exec::Loader {
                wanted: elf_interpreter(&runner).unwrap().unwrap(),
                system: real.clone(),
            }
        );
        let out = command_for(&runner, exec).arg("--list").output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        std::fs::remove_dir_all(&d).ok();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_runner_with_no_usable_loader_is_explained() {
        let d = temp("no-loader");
        let Some((runner, _)) = binary_with_a_missing_loader(&d) else {
            eprintln!("skipping: the test binary is statically linked");
            return;
        };
        let err = exec_mode_in(&runner, &[]).unwrap_err().to_string();
        let wanted = elf_interpreter(&runner).unwrap().unwrap();
        assert!(err.contains("cannot run on this system"), "{err}");
        assert!(err.contains(&wanted.display().to_string()), "{err}");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_runner_that_is_not_elf_runs_directly() {
        let d = temp("not-elf");
        let runner = d.join("runner");
        std::fs::write(&runner, "#!/bin/sh\n").unwrap();
        assert_eq!(exec_mode_in(&runner, &[]).unwrap(), Exec::Direct);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn restore_replays_the_journal_backwards() {
        let d = temp("restore");
        let game = d.join("game");
        std::fs::create_dir_all(game.join("swofty/mods/core")).unwrap();
        let launcher = game.join("EAAntiCheat.GameServiceLauncher.exe");
        let parked = game.join("EAAntiCheat.GameServiceLauncher.exe.bak");
        let hook = game.join("swofty/mods/core/f1op_hook.dll");
        std::fs::write(&launcher, "original").unwrap();
        std::fs::rename(&launcher, &parked).unwrap();
        std::fs::write(&launcher, "stub").unwrap();
        std::fs::write(&hook, "hook").unwrap();

        let stub_digest = sha256_file(&launcher).unwrap();
        let hook_digest = sha256_file(&hook).unwrap();
        let journal = d.join("staging.jsonl");
        // Written through serde_json so paths are escaped the way the real
        // writer escapes them (Windows paths contain backslashes).
        let lines = [
            serde_json::json!({"op": "meta", "game_dir": "x", "created_utc": "", "schema_version": "1"}),
            serde_json::json!({"op": "rename", "from": launcher.display().to_string(), "to": parked.display().to_string(), "source": "", "sha256": ""}),
            serde_json::json!({"op": "copy", "from": "", "to": launcher.display().to_string(), "source": "stub", "sha256": stub_digest}),
            serde_json::json!({"op": "copy", "from": "", "to": hook.display().to_string(), "source": "hook", "sha256": hook_digest}),
        ];
        std::fs::write(
            &journal,
            lines
                .iter()
                .map(serde_json::Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        )
        .unwrap();

        restore(&journal, false).unwrap();
        assert_eq!(std::fs::read_to_string(&launcher).unwrap(), "original");
        assert!(!parked.exists());
        assert!(!hook.exists());

        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn restore_refuses_a_changed_file() {
        let d = temp("tamper");
        let launcher = d.join("EAAntiCheat.GameServiceLauncher.exe");
        std::fs::write(&launcher, "tampered").unwrap();
        let journal = d.join("staging.jsonl");
        let line = serde_json::json!({
            "op": "copy",
            "from": "",
            "to": launcher.display().to_string(),
            "source": "stub",
            "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
        });
        std::fs::write(&journal, format!("{line}\n")).unwrap();
        assert!(restore(&journal, false).is_err());
        assert!(restore(&journal, true).is_ok());
        assert!(!launcher.exists());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn heal_restores_a_half_staged_folder() {
        // The exact state an interrupted session leaves: the real launcher is
        // in .bak and the stub sits in its slot, with no journal.
        let d = temp("heal");
        let game = d.join("game");
        std::fs::create_dir_all(&game).unwrap();
        let launcher = game.join("EAAntiCheat.GameServiceLauncher.exe");
        std::fs::write(&launcher, "stub").unwrap();
        std::fs::write(game.join("EAAntiCheat.GameServiceLauncher.exe.bak"), "original").unwrap();
        let hook = game.join("swofty/mods/core/f1op_hook.dll");
        std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
        std::fs::write(&hook, "stale hook").unwrap();

        let journal = d.join("state/offline/staging.jsonl");
        let notes = heal_stale_backup(&game, &journal).unwrap();
        assert!(!hook.exists(), "the stale hook is swept");
        assert!(notes.iter().any(|n| n.contains("stale hook")), "{notes:?}");
        assert_eq!(std::fs::read_to_string(&launcher).unwrap(), "original");
        assert!(!game.join("EAAntiCheat.GameServiceLauncher.exe.bak").exists());
        let kept = std::fs::read_dir(game.join("swofty/offline-recovery"))
            .unwrap()
            .flatten()
            .next()
            .expect("the stub is kept, never destroyed");
        assert_eq!(std::fs::read_to_string(kept.path()).unwrap(), "stub");
        assert!(notes.iter().any(|n| n.contains("restored")), "{notes:?}");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn heal_drops_an_identical_stale_backup() {
        let d = temp("heal-same");
        let game = d.join("game");
        std::fs::create_dir_all(&game).unwrap();
        let launcher = game.join("EAAntiCheat.GameServiceLauncher.exe");
        std::fs::write(&launcher, "same").unwrap();
        std::fs::write(game.join("EAAntiCheat.GameServiceLauncher.exe.bak"), "same").unwrap();

        let journal = d.join("state/offline/staging.jsonl");
        heal_stale_backup(&game, &journal).unwrap();
        assert_eq!(std::fs::read_to_string(&launcher).unwrap(), "same");
        assert!(!game.join("EAAntiCheat.GameServiceLauncher.exe.bak").exists());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn heal_replays_a_leftover_journal_first() {
        let d = temp("heal-journal");
        let game = d.join("game");
        std::fs::create_dir_all(&game).unwrap();
        let launcher = game.join("EAAntiCheat.GameServiceLauncher.exe");
        let parked = game.join("EAAntiCheat.GameServiceLauncher.exe.bak");
        std::fs::write(&launcher, "stub").unwrap();
        std::fs::write(&parked, "original").unwrap();
        let journal = d.join("state/offline/staging.jsonl");
        let line = serde_json::json!({
            "op": "rename",
            "from": launcher.display().to_string(),
            "to": parked.display().to_string(),
            "source": "",
            "sha256": "",
        });
        std::fs::create_dir_all(journal.parent().unwrap()).unwrap();
        std::fs::write(&journal, format!("{line}\n")).unwrap();

        let notes = heal_stale_backup(&game, &journal).unwrap();
        assert_eq!(std::fs::read_to_string(&launcher).unwrap(), "original");
        assert!(!journal.exists());
        assert!(notes.iter().any(|n| n.contains("restored")), "{notes:?}");
        std::fs::remove_dir_all(&d).ok();
    }

    struct FakeDepot {
        rel: String,
        bytes: Vec<u8>,
    }

    impl DepotHashes for FakeDepot {
        fn stock(&self, rel: &str, path: &Path) -> Result<Option<bool>> {
            if rel != self.rel {
                return Ok(None);
            }
            match std::fs::read(path) {
                Ok(data) => Ok(Some(data == self.bytes)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Some(false)),
                Err(e) => Err(io_err(path)(e)),
            }
        }
    }

    #[test]
    fn a_stale_backup_is_set_aside_when_the_live_exe_is_steams_copy() {
        let d = temp("heal-depot-stale");
        let game = d.join("game");
        std::fs::create_dir_all(&game).unwrap();
        let launcher = game.join(EAC_EXE);
        let parked = game.join(EAC_BAK);
        std::fs::write(&launcher, "stock exe").unwrap();
        std::fs::write(&parked, "a stale backup of the wrong size").unwrap();
        let depot = FakeDepot {
            rel: EAC_EXE.into(),
            bytes: b"stock exe".to_vec(),
        };

        let notes = heal_launcher(&game, Some(&depot)).unwrap();
        assert_eq!(
            std::fs::read_to_string(&launcher).unwrap(),
            "stock exe",
            "the Steam-verified live exe is kept"
        );
        assert!(!parked.exists(), "the stale backup is moved out of the game root");
        let kept = std::fs::read_dir(game.join("swofty/offline-recovery"))
            .unwrap()
            .flatten()
            .next()
            .expect("the stale backup is set aside, never destroyed");
        assert_eq!(
            std::fs::read_to_string(kept.path()).unwrap(),
            "a stale backup of the wrong size"
        );
        assert!(notes.iter().any(|n| n.contains("set a stale")), "{notes:?}");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn an_offline_session_backup_is_restored_over_a_stub_with_a_depot() {
        let d = temp("heal-depot-session");
        let game = d.join("game");
        std::fs::create_dir_all(&game).unwrap();
        let launcher = game.join(EAC_EXE);
        let parked = game.join(EAC_BAK);
        std::fs::write(&launcher, "offline play stub").unwrap();
        std::fs::write(&parked, "the real launcher").unwrap();
        let depot = FakeDepot {
            rel: EAC_EXE.into(),
            bytes: b"the real launcher".to_vec(),
        };

        let notes = heal_launcher(&game, Some(&depot)).unwrap();
        assert_eq!(std::fs::read_to_string(&launcher).unwrap(), "the real launcher");
        assert!(!parked.exists());
        let kept = std::fs::read_dir(game.join("swofty/offline-recovery"))
            .unwrap()
            .flatten()
            .next()
            .expect("the stub is kept, never destroyed");
        assert_eq!(std::fs::read_to_string(kept.path()).unwrap(), "offline play stub");
        assert!(notes.iter().any(|n| n.contains("restored")), "{notes:?}");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn heal_before_check_waits_while_a_session_is_pending() {
        let d = temp("heal-pending");
        let game = d.join("game");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(game.join(EAC_EXE), "stub").unwrap();
        std::fs::write(game.join(EAC_BAK), "real").unwrap();
        let journal = journal_path(&d);
        std::fs::create_dir_all(journal.parent().unwrap()).unwrap();
        std::fs::write(&journal, "{}\n").unwrap();
        let depot = FakeDepot {
            rel: EAC_EXE.into(),
            bytes: b"real".to_vec(),
        };

        let notes = heal_before_check(&d, &game, Some(&depot)).unwrap();
        assert!(
            notes.is_empty(),
            "the pending session's journal owns the launcher"
        );
        assert!(game.join(EAC_BAK).exists());
        assert_eq!(std::fs::read_to_string(game.join(EAC_EXE)).unwrap(), "stub");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn restore_parses_escaped_paths() {
        // Guard the JSON escaping of paths with backslashes on every platform,
        // not just Windows where the paths actually look like this.
        let d = temp("escape");
        let journal = d.join("staging.jsonl");
        let line = serde_json::json!({
            "op": "copy",
            "from": "",
            "to": "C:\\Games\\Steam\\steamapps\\common\\F1 25\\staged.dll",
            "source": "stub",
            "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
        });
        std::fs::write(&journal, format!("{line}\n")).unwrap();
        // The file does not exist, so restore reports nothing to undo — the
        // point is that the line parses and the backslashes survive.
        let done = restore(&journal, true).unwrap();
        assert!(done.is_empty());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn restore_follows_non_ascii_paths() {
        // The extension writes paths as raw UTF-8, as for a data folder under
        // a Windows user name such as Naël.
        let d = temp("unicode");
        let base = d.join("Naël Müller 测试");
        let game = base.join("game");
        std::fs::create_dir_all(&game).unwrap();
        let launcher = game.join("EAAntiCheat.GameServiceLauncher.exe");
        let parked = game.join("EAAntiCheat.GameServiceLauncher.exe.bak");
        std::fs::write(&parked, "original").unwrap();
        std::fs::write(&launcher, "stub").unwrap();
        let stub_digest = sha256_file(&launcher).unwrap();
        let journal = journal_path(&base);
        std::fs::create_dir_all(journal.parent().unwrap()).unwrap();
        let lines = [
            serde_json::json!({"op": "meta", "game_dir": game.display().to_string(), "created_utc": "", "schema_version": "1"}),
            serde_json::json!({"op": "rename", "from": launcher.display().to_string(), "to": parked.display().to_string(), "source": "", "sha256": ""}),
            serde_json::json!({"op": "copy", "from": "", "to": launcher.display().to_string(), "source": base.join("stub.exe").display().to_string(), "sha256": stub_digest}),
        ];
        let text = lines.iter().map(|l| l.to_string() + "\n").collect::<String>();
        assert!(text.contains("Naël Müller 测试"), "{text}");
        std::fs::write(&journal, text).unwrap();

        restore(&journal, false).unwrap();
        assert_eq!(std::fs::read_to_string(&launcher).unwrap(), "original");
        assert!(!parked.exists());
        std::fs::remove_dir_all(&d).ok();
    }
}
