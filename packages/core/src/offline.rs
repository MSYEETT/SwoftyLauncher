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

use crate::baseline::sha256_file;
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
pub const EXTENSION_VERSION: &str = "0.1.1";
/// Where the artifacts are hosted (binaries; source lives in the repository).
pub const DEFAULT_BASE_URL: &str = "https://swofty-developments.github.io/F1-OfflinePlay";
pub const SOURCE_URL: &str = "https://github.com/Swofty-Developments/F1-OfflinePlay";

/// Pinned digests for release v0.1.1. Every artifact is checked against its
/// pin at install time and again at use time.
pub const PINS: &[(&str, &str)] = &[
    (
        "f1op_hook.dll",
        "8cf3f755b13b523b22d1418307b3b045887cc96c43ba7fd6d4d642c1342d7737",
    ),
    (
        "f1op_stub.exe",
        "d90b2a95c03f85e334c0afccb03ac831ed445f2a4e8b905c8c46a93d5d2c348f",
    ),
    (
        "f1op-prelaunch.exe",
        "b8ebe8199eed32ea1f0b91a620576718276e9e8a2df4500fb49b616058094275",
    ),
    (
        "f1op-probe.exe",
        "5820b02dada4b5ae0822c63479d9fef40884b8891b1f38b64fbba1893dced743",
    ),
    (
        "f1op-prelaunch-linux-x86_64",
        "46c64e01a563c8448be28e1cc83cf7b13edf24fb37a7cc4b9cf80996bd35d797",
    ),
    (
        "f1op-probe-linux-x86_64",
        "5a14261e95022c4690d7b80a3806a1078ad4091eca12af2319e187f333e783b5",
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
    Ok(Artifacts {
        hook: dir.join("f1op_hook.dll"),
        stub: dir.join("f1op_stub.exe"),
        prelaunch: dir.join(prelaunch),
        probe: dir.join(probe),
        dir,
    })
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
        .call()
        .map_err(|e| Error::Other(format!("{url}: {e}")))?;
    let mut reader = resp.into_reader();
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

/// Stage the session: park the anti-cheat launcher, drop the stub, and place
/// the hook where the loader picks it up. Everything goes in the journal.
pub fn stage(game: &Path, a: &Artifacts, journal: &Path) -> Result<()> {
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
    Ok(())
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
        std::fs::write(
            &journal,
            format!(
                "{{\"op\":\"meta\",\"game_dir\":\"x\",\"created_utc\":\"\",\"schema_version\":\"1\"}}\n\
                 {{\"op\":\"rename\",\"from\":\"{from}\",\"to\":\"{to}\",\"source\":\"\",\"sha256\":\"\"}}\n\
                 {{\"op\":\"copy\",\"from\":\"\",\"to\":\"{from}\",\"source\":\"stub\",\"sha256\":\"{stub}\"}}\n\
                 {{\"op\":\"copy\",\"from\":\"\",\"to\":\"{hook}\",\"source\":\"hook\",\"sha256\":\"{hookd}\"}}\n",
                from = launcher.display(),
                to = parked.display(),
                hook = hook.display(),
                stub = stub_digest,
                hookd = hook_digest,
            ),
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
        std::fs::write(
            &journal,
            format!(
                "{{\"op\":\"copy\",\"from\":\"\",\"to\":\"{}\",\"source\":\"stub\",\"sha256\":\"{}\"}}\n",
                launcher.display(),
                "0000000000000000000000000000000000000000000000000000000000000000"
            ),
        )
        .unwrap();
        assert!(restore(&journal, false).is_err());
        assert!(restore(&journal, true).is_ok());
        assert!(!launcher.exists());
        std::fs::remove_dir_all(&d).ok();
    }
}
