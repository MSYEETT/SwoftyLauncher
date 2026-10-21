use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use swofty_addon::launcher::GameStatus;

use crate::GAME_EXE;
use crate::session::SessionRecord;

#[derive(Debug, Clone, PartialEq)]
pub struct GameProcess {
    pub pid: u32,
    pub started: Option<SystemTime>,
}

pub fn game_running() -> bool {
    !game_processes().is_empty()
}

/// The running game for a launcher plugin's game_status: its process, and the
/// latest swofty session under `data` while that session is still live.
pub fn game_status(data: Option<&Path>) -> GameStatus {
    status_of(&game_processes(), data)
}

fn status_of(processes: &[GameProcess], data: Option<&Path>) -> GameStatus {
    let Some(first) = processes.first() else {
        return GameStatus::default();
    };
    let session = data
        .and_then(|d| std::fs::read_to_string(d.join("sessions/latest.txt")).ok())
        .map(|dir| SessionRecord::read(Path::new(dir.trim())))
        .filter(|s| s.started.is_some() && s.ended.is_none())
        .and_then(|s| Some(s.dir.file_name()?.to_string_lossy().into_owned()));
    GameStatus {
        running: true,
        pid: Some(first.pid),
        since: first
            .started
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64()),
        session,
    }
}

pub fn game_processes() -> Vec<GameProcess> {
    named(GAME_EXE)
}

/// The copies in `running` that were not there before a launch (`before`) and
/// that the session did not start (`ours`): the EA App build hands a launch
/// back to the EA app, which starts the game again outside the session.
pub fn started_elsewhere(running: &[GameProcess], before: &[u32], ours: impl Fn(u32) -> bool) -> Vec<u32> {
    running
        .iter()
        .map(|g| g.pid)
        .filter(|&pid| !before.contains(&pid) && !ours(pid))
        .collect()
}

pub fn steamvr_running() -> bool {
    !named(if cfg!(windows) { "vrserver.exe" } else { "vrserver" }).is_empty()
}

#[cfg(target_os = "linux")]
pub(crate) fn named(name: &str) -> Vec<GameProcess> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let pid = e.file_name().to_str()?.parse().ok()?;
            let comm = std::fs::read_to_string(e.path().join("comm")).ok()?;
            (comm.trim_end() == name).then(|| GameProcess {
                pid,
                started: started(pid),
            })
        })
        .collect()
}

#[cfg(target_os = "linux")]
pub fn descends_from(pid: u32, ancestor: u32) -> bool {
    std::iter::successors(parent(pid), |&p| parent(p))
        .take_while(|&p| p > 1)
        .any(|p| p == ancestor)
}

#[cfg(not(target_os = "linux"))]
pub fn descends_from(_: u32, _: u32) -> bool {
    false
}

#[cfg(target_os = "linux")]
fn parent(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(')')?.1.split_whitespace().nth(1)?.parse().ok()
}

#[cfg(target_os = "linux")]
pub(crate) fn started(pid: u32) -> Option<SystemTime> {
    const TICKS_PER_SEC: f64 = 100.0;
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let ticks: f64 = stat
        .rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()?;
    let uptime: f64 = std::fs::read_to_string("/proc/uptime")
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    let age = (uptime - ticks / TICKS_PER_SEC).max(0.0);
    SystemTime::now().checked_sub(std::time::Duration::from_secs_f64(age))
}

#[cfg(windows)]
pub(crate) fn named(name: &str) -> Vec<GameProcess> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let filter = format!("IMAGENAME eq {name}");
    let Ok(out) = Command::new("tasklist")
        .args(["/FI", &filter, "/FO", "CSV", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut cols = line.split("\",\"").map(|c| c.trim_matches('"'));
            cols.next()?.eq_ignore_ascii_case(name).then_some(())?;
            Some(GameProcess {
                pid: cols.next()?.parse().ok()?,
                started: None,
            })
        })
        .collect()
}

#[cfg(not(any(target_os = "linux", windows)))]
pub(crate) fn named(_: &str) -> Vec<GameProcess> {
    Vec::new()
}

pub fn stop_process(pid: u32) -> std::io::Result<()> {
    let pid = pid.to_string();
    if cfg!(windows) {
        Command::new("taskkill").args(["/PID", &pid]).status()?;
    } else {
        Command::new("kill").args(["-TERM", &pid]).status()?;
    }
    Ok(())
}

pub fn stop_game() -> std::io::Result<()> {
    if cfg!(windows) {
        Command::new("taskkill").args(["/IM", GAME_EXE]).status()?;
    } else {
        Command::new("pkill").args(["-TERM", "-x", GAME_EXE]).status()?;
    }
    Ok(())
}

#[cfg(test)]
mod status_tests {
    use super::*;

    #[test]
    fn reports_the_live_session_of_a_running_game() {
        let root = std::env::temp_dir().join(format!("swofty-status-{}", std::process::id()));
        let dir = root.join("sessions/2026-10-20T14-22-33");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(root.join("sessions/latest.txt"), format!("{}\n", dir.display())).unwrap();
        std::fs::write(dir.join("game_start"), "100.5\n").unwrap();
        let game = [GameProcess {
            pid: 48211,
            started: Some(UNIX_EPOCH + std::time::Duration::from_secs(100)),
        }];
        let status = status_of(&game, Some(&root));
        assert_eq!(
            status,
            GameStatus {
                running: true,
                pid: Some(48211),
                since: Some(100.0),
                session: Some("2026-10-20T14-22-33".into()),
            }
        );
        std::fs::write(dir.join("game_end"), "200\n").unwrap();
        assert_eq!(status_of(&game, Some(&root)).session, None);
        assert_eq!(status_of(&[], Some(&root)), GameStatus::default());
        assert_eq!(status_of(&game, None).session, None);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn picks_only_copies_the_session_neither_saw_nor_started() {
        let running = [11076, 15472, 9960, 4242].map(|pid| GameProcess { pid, started: None });
        let ours = |pid| pid == 9960;
        assert_eq!(started_elsewhere(&running, &[4242], ours), vec![11076, 15472]);
        assert_eq!(started_elsewhere(&running[2..], &[4242], ours), Vec::<u32>::new());
        assert_eq!(started_elsewhere(&[], &[], ours), Vec::<u32>::new());
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn traces_ancestry() {
        let mut child = std::process::Command::new("sleep").arg("5").spawn().unwrap();
        assert!(super::descends_from(child.id(), std::process::id()));
        assert!(!super::descends_from(std::process::id(), child.id()));
        child.kill().ok();
        child.wait().ok();
    }

    #[test]
    fn reads_own_start_time() {
        let age = super::started(std::process::id())
            .unwrap()
            .elapsed()
            .unwrap_or_default();
        assert!(age.as_secs() < 3600, "{age:?}");
    }
}
