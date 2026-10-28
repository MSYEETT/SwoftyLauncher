//! The launcher's own log, `state/launcher-app.log` in the data folder: a
//! line when it starts, what Restart as administrator did, and a panic that
//! closed it. Send Logs collects it with the other state logs.

use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) const FILE: &str = "state/launcher-app.log";
/// A log past this size starts over.
const CAP: u64 = 256 << 10;

fn path() -> PathBuf {
    swofty_core::paths::data_dir().join(FILE)
}

fn line(text: &str) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    format!(
        "{} [{}] {text}\n",
        swofty_core::send_logs::utc_stamp(secs),
        std::process::id()
    )
}

fn append(path: &Path, text: &str) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let full = std::fs::metadata(path).is_ok_and(|m| m.len() > CAP);
    std::fs::OpenOptions::new()
        .create(true)
        .append(!full)
        .write(true)
        .truncate(full)
        .open(path)
        .and_then(|mut f| f.write_all(line(text).as_bytes()))
        .ok();
}

pub(crate) fn note(text: &str) {
    append(&path(), text);
}

/// The start line, then every panic, before the default hook prints it.
pub(crate) fn start() {
    let path = path();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let started = format!(
        "started {}{}",
        env!("CARGO_PKG_VERSION"),
        if args.is_empty() {
            String::new()
        } else {
            format!(" with {}", args.join(" "))
        }
    );
    append(&path, &started);
    let at = path.clone();
    std::thread::spawn(move || {
        let elevated = if swofty_core::compat::elevated() {
            "yes"
        } else {
            "no"
        };
        append(&at, &format!("administrator: {elevated}"));
    });
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let backtrace = std::backtrace::Backtrace::force_capture();
        append(
            &path,
            &format!(
                "panic on thread {}: {info}\n{backtrace}",
                thread.name().unwrap_or("unnamed")
            ),
        );
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_lines_and_starts_over_when_full() {
        let dir = std::env::temp_dir().join(format!("swofty-app-log-{}", std::process::id()));
        let log = dir.join(FILE);
        append(&log, "started 1.0.0");
        append(&log, "administrator: no");
        let text = std::fs::read_to_string(&log).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].ends_with(&format!("[{}] started 1.0.0", std::process::id())));
        assert!(lines[1].ends_with("administrator: no"));
        std::fs::write(&log, vec![b'x'; CAP as usize + 1]).unwrap();
        append(&log, "started 1.0.1");
        let text = std::fs::read_to_string(&log).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.ends_with("started 1.0.1\n"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
