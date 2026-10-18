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

use crate::baseline::{sha256_file, sha256_opt};
use crate::{Error, Result, io_err, read_json, write_json};

/// The terms shown before consent and by `swofty offline accept`.
pub const DISCLOSURE: &str = "\
Offline play - read this before accepting

Modded sessions run F1 25 without EA AntiCheat (Javelin) so the loader and
mods can work. For the session, the manager swaps EAAntiCheat.GameServiceLauncher.exe
for a stub that starts the game, and lets the game continue past its
anti-cheat check.

- This is outside EA's terms of service. EA can detect modified launch
  behaviour and may take action against your account, including suspension
  or termination. The risk is yours.
- Sessions run offline. Multiplayer is unavailable, and this component
  cannot enable it.
- Everything is scoped to the session: the game folder is restored to
  vanilla when the game exits, by `swofty restore`, and at the next start.
- The extension is downloaded from Swofty's site and verified against
  pinned SHA-256 digests before it is used. Source: Swofty-Developments/F1-OfflinePlay.";

/// The extension release this manager pins.
pub const EXTENSION_VERSION: &str = "0.2.3";
/// Where the artifacts are hosted (binaries; source lives in the repository).
pub const DEFAULT_BASE_URL: &str = "https://swofty-developments.github.io/F1-OfflinePlay";
pub const SOURCE_URL: &str = "https://github.com/Swofty-Developments/F1-OfflinePlay";

/// Pinned digests for release v0.2.3. Every artifact is checked against its
/// pin at install time and again at use time.
pub const PINS: &[(&str, &str)] = &[
    (
        "f1op_hook.dll",
        "f7133707796045aac9f4233b5ae5c0d7a78a7ecc97d53a4f329836e648422cc3",
    ),
    (
        "f1op_stub.exe",
        "d9954bd7ccec53545fd21fc9fe63bd99abc55708552501049b495f9e5534cf9d",
    ),
    (
        "f1op-prelaunch.exe",
        "3c03067460f76ef2cdfc65ac87e3416f684c01a8f7956d10956b3d410d5f0f04",
    ),
    (
        "f1op-probe.exe",
        "fe422e02e75f2e23bc0670a33c1a114288bef8f703fac23d51ed14fd4ae43731",
    ),
    (
        "f1op-prelaunch-linux-x86_64",
        "3bf6a16879312b7aa2363b863152426b554a461b6e344de956cc84d123ec9edc",
    ),
    (
        "f1op-probe-linux-x86_64",
        "c08ba64e0d8f6c2040fc990a296395192bd592bfcae49a9c6a9674e438cf1e28",
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

fn verify_file(path: &Path, name: &str) -> Result<()> {
    let want = pin(name).ok_or_else(|| Error::Other(format!("no pinned digest for {name}")))?;
    let have = sha256_file(path)?;
    if have != want {
        return Err(Error::Other(format!(
            "{name}: digest mismatch (have {have}, want {want}); refusing to use it"
        )));
    }
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
    #[cfg(windows)]
    let (prelaunch, probe) = ("f1op-prelaunch.exe", "f1op-probe.exe");
    #[cfg(not(windows))]
    let (prelaunch, probe) = ("f1op-prelaunch-linux-x86_64", "f1op-probe-linux-x86_64");
    let out = Artifacts {
        hook: dir.join("f1op_hook.dll"),
        stub: dir.join("f1op_stub.exe"),
        prelaunch: dir.join(prelaunch),
        probe: dir.join(probe),
        dir,
    };
    // Downloads and copies do not carry the execute bit, and the runners need
    // it on Unix. Re-applied on every use so an install made before this fix
    // heals itself without being downloaded again.
    #[cfg(unix)]
    for path in [&out.prelaunch, &out.probe] {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)
            .map_err(|e| io_err(path.to_path_buf())(e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).map_err(|e| io_err(path.to_path_buf())(e))?;
    }
    Ok(out)
}

/// Verify a downloaded release folder against the pins and install it.
pub fn install(data: &Path, src: &Path) -> Result<Artifacts> {
    for name in REQUIRED {
        let path = src.join(name);
        if !path.is_file() {
            return Err(Error::Other(format!("{}: not found in {}", name, src.display())));
        }
        verify_file(&path, name)?;
    }
    let dir = extension_dir(data);
    std::fs::create_dir_all(&dir).map_err(io_err(&dir))?;
    for name in REQUIRED {
        let to = dir.join(name);
        std::fs::copy(src.join(name), &to).map_err(io_err(&to))?;
    }
    artifacts(data)
}

/// Download the pinned release from the artifact host and install it. The
/// digests decide what is trusted; the host is only a transport.
pub fn fetch(data: &Path) -> Result<Artifacts> {
    let tmp = data.join("extensions/.download");
    std::fs::create_dir_all(&tmp).map_err(io_err(&tmp))?;
    for name in REQUIRED {
        let url = format!("{}/v{}/{}", base_url(), EXTENSION_VERSION, name);
        let to = tmp.join(name);
        download(&url, &to)?;
        verify_file(&to, name).map_err(|e| {
            let _ = std::fs::remove_file(&to);
            Error::Other(format!("{url}: {e}"))
        })?;
    }
    let out = install(data, &tmp)?;
    std::fs::remove_dir_all(&tmp).ok();
    Ok(out)
}

fn download(url: &str, to: &Path) -> Result<()> {
    let resp = ureq::get(url)
        .config()
        .timeout_connect(Some(std::time::Duration::from_secs(30)))
        .build()
        .call()
        .map_err(|e| Error::Other(format!("{url}: {e}")))?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::Other(format!("{url}: HTTP {status}")));
    }
    let mut reader = resp.into_body().into_reader();
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut buf).map_err(|e| Error::Other(format!("{url}: {e}")))?;
    std::fs::write(to, &buf).map_err(io_err(to))
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

/// Ask the extension whether this game build is supported (fail closed).
pub fn probe(game: &Path, a: &Artifacts) -> Result<Probe> {
    let out = Command::new(&a.probe)
        .arg(game.join(crate::GAME_EXE))
        .output()
        .map_err(|e| Error::Other(format!("{}: {e}", a.probe.display())))?;
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
            "probe failed ({code:?}): {}",
            String::from_utf8_lossy(&out.stderr).trim()
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
    notes.extend(heal_launcher(game)?);
    // A hook left in the loader's slot is ours: the journal is what normally
    // owns it, and staging cannot drop the new one while it is there.
    let hook = game.join("swofty/mods/core/f1op_hook.dll");
    if hook.exists() {
        std::fs::remove_file(&hook).map_err(io_err(&hook))?;
        notes.push("removed a stale hook from a previous session".into());
    }
    Ok(notes)
}

fn heal_launcher(game: &Path) -> Result<Vec<String>> {
    let mut notes = Vec::new();
    let launcher = game.join("EAAntiCheat.GameServiceLauncher.exe");
    let parked = game.join("EAAntiCheat.GameServiceLauncher.exe.bak");
    if !parked.exists() {
        return Ok(notes);
    }
    let backup = sha256_file(&parked)?;
    if sha256_opt(&launcher)?.as_deref() == Some(backup.as_str()) {
        std::fs::remove_file(&parked).map_err(io_err(&parked))?;
        notes.push("removed a stale backup identical to the launcher".into());
        return Ok(notes);
    }
    if launcher.exists() {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let keep = game
            .join("swofty/offline-recovery")
            .join(format!("EAAntiCheat.GameServiceLauncher.exe.{secs}"));
        if let Some(parent) = keep.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_err(parent.to_path_buf())(e))?;
        }
        std::fs::rename(&launcher, &keep).map_err(io_err(&launcher))?;
        notes.push(format!("kept the staged file at {}", keep.display()));
    }
    std::fs::rename(&parked, &launcher).map_err(io_err(&launcher))?;
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
    let out = Command::new(&a.prelaunch)
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
        .map_err(|e| Error::Other(format!("{}: {e}", a.prelaunch.display())))?;
    if !out.status.success() {
        return Err(Error::Other(format!(
            "offline play staging failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
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
    let mut done = Vec::new();
    for e in lines.iter().rev() {
        match e.op.as_str() {
            "meta" => {}
            "copy" => {
                let to = Path::new(&e.to);
                if !to.exists() {
                    continue;
                }
                if !force {
                    let have = sha256_file(to)?;
                    if have != e.sha256 {
                        return Err(Error::Other(format!(
                            "{}: changed since staging (digest mismatch); refusing to remove it",
                            e.to
                        )));
                    }
                }
                std::fs::remove_file(to).map_err(io_err(to))?;
                done.push(format!("removed {}", e.to));
            }
            "rename" => {
                let (from, to) = (Path::new(&e.from), Path::new(&e.to));
                if !to.exists() {
                    return Err(Error::Other(format!("staged backup is missing: {}", e.to)));
                }
                if from.exists() && !force {
                    return Err(Error::Other(format!(
                        "{}: already exists; refusing to overwrite it",
                        e.from
                    )));
                }
                std::fs::rename(to, from).map_err(io_err(from))?;
                done.push(format!("restored {}", e.from));
            }
            other => {
                return Err(Error::Other(format!(
                    "unknown staging journal op: {other} (update swofty)"
                )));
            }
        }
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
}
