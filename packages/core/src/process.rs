use std::process::Command;
use std::time::SystemTime;

use crate::GAME_EXE;

#[derive(Debug, Clone, PartialEq)]
pub struct GameProcess {
    pub pid: u32,
    pub started: Option<SystemTime>,
}

pub fn game_running() -> bool {
    !game_processes().is_empty()
}

pub fn game_processes() -> Vec<GameProcess> {
    named(GAME_EXE)
}

pub fn steamvr_running() -> bool {
    !named(if cfg!(windows) { "vrserver.exe" } else { "vrserver" }).is_empty()
}

#[cfg(target_os = "linux")]
fn named(name: &str) -> Vec<GameProcess> {
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
fn started(pid: u32) -> Option<SystemTime> {
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
fn named(name: &str) -> Vec<GameProcess> {
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
            (cols.next()? == name).then_some(())?;
            Some(GameProcess {
                pid: cols.next()?.parse().ok()?,
                started: None,
            })
        })
        .collect()
}

#[cfg(not(any(target_os = "linux", windows)))]
fn named(_: &str) -> Vec<GameProcess> {
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
