use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use swofty_core::launcher::Progress;
use swofty_core::paths;
use swofty_core::process;
use swofty_core::recovery::{self, SessionLock};
use swofty_core::update;

const PARTS: [(&str, &str); 3] = [
    ("swofty-launcher", "Swofty Launcher"),
    ("swofty-ui", "Swofty UI"),
    ("swofty", "the swofty command line"),
];
pub const CLOSING: &str = "Closing Swofty Launcher";
const GRACE: Duration = Duration::from_secs(10);
const FORCED: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Running {
    pub pid: u32,
    pub part: &'static str,
}

impl Running {
    fn title(&self) -> &'static str {
        PARTS
            .iter()
            .find(|(n, _)| *n == self.part)
            .map_or(self.part, |(_, t)| *t)
    }
}

pub fn placement(folder: &str) -> String {
    let Ok(root) = crate::resolve(folder) else {
        return String::new();
    };
    if root.join("bin").join(update::exe("swofty-launcher")).is_file() {
        format!(
            "Swofty Launcher is already in {}. Installing updates it in place; your mods and settings stay.",
            root.display()
        )
    } else if root.is_dir() {
        format!("Installs into {}.", root.display())
    } else {
        format!("Creates {} and installs there.", root.display())
    }
}

pub fn game_block() -> Option<&'static str> {
    if process::game_running() {
        return Some("F1 25 is running. Close the game, then select Try again.");
    }
    let data = paths::data_dir();
    (recovery::lock_path(&data).is_file() && SessionLock::held_elsewhere(&data)).then_some(
        "Swofty Launcher is busy with the game folder. Wait for it to finish, then select Try again.",
    )
}

pub fn clear(root: &Path, close: bool, progress: &dyn Progress) -> Result<(), String> {
    if close {
        self::close(root)?;
        progress.report(0.0, crate::FINDING);
    }
    match game_block() {
        Some(why) => Err(why.into()),
        None => Ok(()),
    }
}

pub fn from(root: &Path) -> Vec<Running> {
    let bin = root.join("bin");
    let me = std::process::id();
    processes()
        .into_iter()
        .filter(|(pid, _)| *pid != me)
        .filter_map(|(pid, exe)| {
            Some(Running {
                pid,
                part: part_in(&exe, &bin)?,
            })
        })
        .collect()
}

fn part_in(exe: &str, bin: &Path) -> Option<&'static str> {
    let exe = exe.strip_suffix(" (deleted)").unwrap_or(exe);
    let cut = exe.rfind(['/', '\\'])?;
    let file = &exe[cut + 1..];
    let lower = file.to_ascii_lowercase();
    let windows = lower.strip_suffix(".exe");
    let part = PARTS
        .iter()
        .map(|(n, _)| *n)
        .find(|n| file == *n || windows == Some(*n))?;
    process::same_folder(Path::new(&exe[..cut.max(1)]), bin).then_some(part)
}

pub fn describe(open: &[Running]) -> String {
    let titles: Vec<&str> = PARTS
        .iter()
        .filter(|(n, _)| open.iter().any(|r| r.part == *n))
        .map(|(_, t)| *t)
        .collect();
    let (names, verb) = match titles.as_slice() {
        [one] => (capital(one), "is"),
        [rest @ .., last] => (format!("{} and {last}", capital(&rest.join(", "))), "are"),
        [] => return String::new(),
    };
    format!("{names} {verb} open from this folder and must close before the files are replaced.")
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

pub fn close(root: &Path) -> Result<(), String> {
    let open = from(root);
    for r in &open {
        if !ask(r.pid) {
            force(r.pid);
        }
    }
    let left = wait(root, &open, GRACE);
    for r in &left {
        force(r.pid);
    }
    match wait(root, &left, FORCED).first() {
        None => Ok(()),
        Some(r) => Err(format!(
            "Could not close {} (process {}). Close it, then select Try again.",
            r.title(),
            r.pid
        )),
    }
}

fn wait(root: &Path, open: &[Running], timeout: Duration) -> Vec<Running> {
    let start = Instant::now();
    loop {
        let left: Vec<Running> = from(root).into_iter().filter(|r| open.contains(r)).collect();
        if left.is_empty() || start.elapsed() >= timeout {
            return left;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn ask(pid: u32) -> bool {
    if cfg!(windows) {
        quiet("taskkill", &["/PID", &pid.to_string()])
    } else {
        quiet("kill", &["-TERM", &pid.to_string()])
    }
}

fn force(pid: u32) {
    if cfg!(windows) {
        quiet("taskkill", &["/F", "/PID", &pid.to_string()]);
    } else {
        quiet("kill", &["-KILL", &pid.to_string()]);
    }
}

fn quiet(program: &str, args: &[&str]) -> bool {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.status().is_ok_and(|s| s.success())
}

#[cfg(target_os = "linux")]
fn processes() -> Vec<(u32, String)> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let pid = e.file_name().to_str()?.parse().ok()?;
            let exe = std::fs::read_link(e.path().join("exe")).ok()?;
            Some((pid, exe.to_string_lossy().into_owned()))
        })
        .collect()
}

#[cfg(windows)]
fn processes() -> Vec<(u32, String)> {
    win32::processes()
        .into_iter()
        .filter(|(_, file)| {
            PARTS
                .iter()
                .any(|(n, _)| file.eq_ignore_ascii_case(&format!("{n}.exe")))
        })
        .filter_map(|(pid, _)| Some((pid, win32::image(pid)?)))
        .collect()
}

#[cfg(not(any(target_os = "linux", windows)))]
fn processes() -> Vec<(u32, String)> {
    Vec::new()
}

#[cfg(windows)]
mod win32 {
    use std::ffi::c_void;

    const TH32CS_SNAPPROCESS: u32 = 0x2;
    const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;
    const MAX_PATH: usize = 260;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    #[repr(C)]
    struct ProcessEntry32W {
        size: u32,
        usage: u32,
        process_id: u32,
        default_heap_id: usize,
        module_id: u32,
        threads: u32,
        parent_process_id: u32,
        priority: i32,
        flags: u32,
        exe_file: [u16; MAX_PATH],
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
        fn Process32FirstW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn QueryFullProcessImageNameW(
            process: *mut c_void,
            flags: u32,
            name: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    pub fn processes() -> Vec<(u32, String)> {
        // SAFETY: the snapshot handle is checked before use and closed once;
        // the entry is zeroed with its size set, as Process32FirstW requires,
        // and exe_file is read only up to its NUL inside the array.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
                return Vec::new();
            }
            let mut entry: ProcessEntry32W = std::mem::zeroed();
            entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
            let mut out = Vec::new();
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                let len = entry.exe_file.iter().position(|&c| c == 0).unwrap_or(MAX_PATH);
                out.push((entry.process_id, String::from_utf16_lossy(&entry.exe_file[..len])));
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
            out
        }
    }

    pub fn image(pid: u32) -> Option<String> {
        let mut name = vec![0u16; 32_768];
        let mut len = name.len() as u32;
        // SAFETY: the handle is checked before use and closed once, and `len`
        // tells the call how many u16s `name` holds.
        let ok = unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return None;
            }
            let ok = QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut len);
            CloseHandle(process);
            ok
        };
        (ok != 0).then(|| String::from_utf16_lossy(&name[..len as usize]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_launchers_parts_in_this_installs_bin_only() {
        let bin = Path::new("/home/a/SwoftyLauncher/bin");
        let part = |exe: &str| part_in(exe, bin);
        assert_eq!(
            part("/home/a/SwoftyLauncher/bin/swofty-launcher"),
            Some("swofty-launcher")
        );
        assert_eq!(part("/home/a/SwoftyLauncher/bin/swofty-ui"), Some("swofty-ui"));
        assert_eq!(part("/home/a/SwoftyLauncher/bin/swofty"), Some("swofty"));
        assert_eq!(
            part("/home/a/SwoftyLauncher/bin/swofty-launcher (deleted)"),
            Some("swofty-launcher"),
            "a launcher whose file an update replaced"
        );
        assert_eq!(part("/home/a/Other/bin/swofty-launcher"), None);
        assert_eq!(part("/home/a/SwoftyLauncher/swofty-launcher"), None);
        assert_eq!(part("/home/a/SwoftyLauncher/bin/old/swofty-launcher"), None);
        assert_eq!(part("/home/a/SwoftyLauncher/bin/swofty-launcher-old"), None);
        assert_eq!(part("/home/a/SwoftyLauncher/bin/swofty-launcher.sh"), None);
        assert_eq!(part("/home/a/SwoftyLauncher/bin/F1_25.exe"), None);
        assert_eq!(part("swofty-launcher"), None);
    }

    #[test]
    fn windows_images_match_whatever_the_case_and_slashes() {
        let bin = Path::new(r"C:\Users\A\AppData\Local\Programs\SwoftyLauncher\bin");
        let part = |exe: &str| part_in(exe, bin);
        assert_eq!(
            part(r"C:\Users\A\AppData\Local\Programs\SwoftyLauncher\bin\swofty-launcher.exe"),
            Some("swofty-launcher")
        );
        assert_eq!(
            part(r"c:\users\a\appdata\local\programs\swoftylauncher\bin\Swofty-UI.EXE"),
            Some("swofty-ui")
        );
        assert_eq!(
            part(r"\\?\C:/Users/A/AppData/Local/Programs/SwoftyLauncher/bin/swofty.exe"),
            Some("swofty")
        );
        assert_eq!(part(r"C:\Users\A\SwoftyLauncher\bin\swofty-launcher.exe"), None);
        assert_eq!(
            part(r"C:\Users\A\AppData\Local\Programs\SwoftyLauncher\bin\swofty-launcher.exe.bak"),
            None
        );
        assert_eq!(
            part(r"C:\Users\A\AppData\Local\Programs\SwoftyLauncher\bin\F1_25.exe"),
            None,
            "never the game"
        );
    }

    #[test]
    fn names_what_is_open() {
        let r = |part| Running { pid: 1, part };
        assert_eq!(
            describe(&[r("swofty-launcher")]),
            "Swofty Launcher is open from this folder and must close before the files are replaced."
        );
        assert_eq!(
            describe(&[r("swofty-ui"), r("swofty-launcher"), r("swofty-ui")]),
            "Swofty Launcher and Swofty UI are open from this folder and must close before the files are replaced."
        );
        assert_eq!(
            describe(&[r("swofty")]),
            "The swofty command line is open from this folder and must close before the files are replaced."
        );
        assert_eq!(describe(&[]), "");
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn closes_a_launcher_running_from_the_install_and_leaves_others() {
        let dir = std::env::temp_dir().join(format!("swofty-installer-running-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let root = dir.join("SwoftyLauncher");
        let other = dir.join("Other");
        for r in [&root, &other] {
            std::fs::create_dir_all(r.join("bin")).unwrap();
        }
        let exe = std::env::current_exe().unwrap();
        let start = |r: &Path| {
            let copy = r.join("bin").join(update::exe("swofty-launcher"));
            std::fs::copy(&exe, &copy).unwrap();
            Command::new(copy)
                .args(["--ignored", "--exact", "running::tests::dummy_launcher"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap()
        };
        let mut ours = start(&root);
        let mut theirs = start(&other);
        let found = |r: &Path, pid: u32| {
            from(r)
                .iter()
                .any(|x| x.pid == pid && x.part == "swofty-launcher")
        };
        let seen = Instant::now();
        while !(found(&root, ours.id()) && found(&other, theirs.id())) {
            assert!(seen.elapsed() < Duration::from_secs(10), "{:?}", from(&root));
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!found(&root, theirs.id()));
        close(&root).unwrap();
        ours.wait().unwrap();
        assert!(from(&root).is_empty());
        assert!(
            theirs.try_wait().unwrap().is_none(),
            "another install's launcher stays"
        );
        theirs.kill().ok();
        theirs.wait().ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    #[ignore = "the dummy launcher the close test starts"]
    fn dummy_launcher() {
        std::thread::sleep(Duration::from_secs(60));
    }
}
