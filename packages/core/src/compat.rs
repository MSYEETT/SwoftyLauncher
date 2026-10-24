//! The Windows "Run this program as an administrator" setting on the game.
//!
//! Windows will not start an exe that is set to run as administrator from an
//! unelevated process (CreateProcess error 740), and the offline play session
//! launcher does not elevate the game, so a modded session cannot start while
//! the setting is on and the launcher runs unelevated. The setting is a
//! RUNASADMIN layer on the exe's path under `AppCompatFlags\Layers`, per user
//! (HKCU) or for all users (HKLM). This only reads it; the registry is never
//! changed.

use std::path::Path;

use crate::GAME_EXE;

pub const LAYERS_KEY: &str = r"Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers";

/// The exes a modded session starts: the game, and the session launcher in
/// the anti-cheat launcher's slot.
pub const SESSION_EXES: &[&str] = &[GAME_EXE, "EAAntiCheat.GameServiceLauncher.exe"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminLayer {
    pub all_users: bool,
    /// The file name of the exe the session starts.
    pub name: String,
    /// The exe path as the setting stores it.
    pub exe: String,
    /// The layers set on it, e.g. `~ RUNASADMIN`.
    pub layers: String,
}

impl AdminLayer {
    pub fn scope(&self) -> &'static str {
        if self.all_users {
            "for all users"
        } else {
            "for this user"
        }
    }
}

pub fn has_run_as_admin(layers: &str) -> bool {
    layers
        .split_whitespace()
        .any(|l| l.eq_ignore_ascii_case("RUNASADMIN"))
}

/// The string values `reg query <key>` lists, as (name, data).
pub fn parse_reg_query(out: &str) -> Vec<(String, String)> {
    out.lines()
        .filter_map(|line| {
            let rest = line.trim_end_matches('\r').strip_prefix("    ")?;
            ["    REG_SZ", "    REG_EXPAND_SZ"].iter().find_map(|ty| {
                let (name, data) = rest.split_once(ty)?;
                let data = data.strip_prefix("    ").or(data.is_empty().then_some(""))?;
                Some((name.to_owned(), data.to_owned()))
            })
        })
        .collect()
}

/// A REG_DWORD value `reg query <key>` lists, by name.
pub fn reg_dword(out: &str, name: &str) -> Option<u32> {
    out.lines().find_map(|line| {
        let rest = line.trim_end_matches('\r').strip_prefix("    ")?;
        let (value, data) = rest.split_once("    REG_DWORD    ")?;
        value.eq_ignore_ascii_case(name).then_some(())?;
        u32::from_str_radix(data.trim().strip_prefix("0x")?, 16).ok()
    })
}

/// The setting keeps the path as Explorer saw it, which need not be the
/// spelling the launcher found (case, separators, a linked library folder).
fn same_file(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\");
    norm(a).eq_ignore_ascii_case(&norm(b))
        || matches!(
            (std::fs::canonicalize(a), std::fs::canonicalize(b)),
            (Ok(x), Ok(y)) if x == y
        )
}

fn matching(listing: &str, all_users: bool, exe: &Path) -> Vec<AdminLayer> {
    let path = exe.to_string_lossy();
    let name = path.rsplit(['\\', '/']).next().unwrap_or(&path);
    parse_reg_query(listing)
        .into_iter()
        .filter(|(stored, data)| has_run_as_admin(data) && same_file(Path::new(stored), exe))
        .map(|(stored, layers)| AdminLayer {
            all_users,
            name: name.to_owned(),
            exe: stored,
            layers,
        })
        .collect()
}

/// Every run-as-administrator setting on `exe`, per user first.
pub fn admin_layers(exe: &Path) -> Vec<AdminLayer> {
    [("HKCU", false), ("HKLM", true)]
        .into_iter()
        .filter_map(|(hive, all_users)| {
            let key = format!(r"{hive}\{LAYERS_KEY}");
            let listing = windows_tool("reg", &["query", &key, "/reg:64"])?;
            Some(matching(&listing, all_users, exe))
        })
        .flatten()
        .collect()
}

/// True when this process runs with a high (or system) integrity level.
pub fn elevated() -> bool {
    windows_tool("whoami", &["/groups", "/fo", "csv", "/nh"])
        .is_some_and(|groups| groups.contains("S-1-16-12288") || groups.contains("S-1-16-16384"))
}

#[cfg(windows)]
pub(crate) fn windows_tool(program: &str, args: &[&str]) -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(not(windows))]
pub(crate) fn windows_tool(_: &str, _: &[&str]) -> Option<String> {
    None
}

/// A Windows tool's output, or why there is none: it could not start,
/// failed (its first error line), or did not finish `within` and was ended.
#[cfg(windows)]
pub(crate) fn windows_tool_within(
    program: &str,
    args: &[&str],
    within: std::time::Duration,
) -> std::result::Result<String, String> {
    use std::io::Read;
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut child = std::process::Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{program} did not start ({e})"))?;
    fn drain(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut data = Vec::new();
            if let Some(mut p) = pipe {
                p.read_to_end(&mut data).ok();
            }
            data
        })
    }
    let (out, err) = (drain(child.stdout.take()), drain(child.stderr.take()));
    let until = std::time::Instant::now() + within;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < until => {
                std::thread::sleep(std::time::Duration::from_millis(20))
            }
            Ok(None) => {
                child.kill().ok();
                child.wait().ok();
                return Err(format!("{program} gave no answer within {} s", within.as_secs()));
            }
            Err(e) => return Err(format!("{program}: {e}")),
        }
    };
    let out = String::from_utf8_lossy(&out.join().unwrap_or_default()).into_owned();
    if status.success() {
        return Ok(out);
    }
    let err = String::from_utf8_lossy(&err.join().unwrap_or_default()).into_owned();
    let first = err
        .lines()
        .chain(out.lines())
        .map(str::trim)
        .find(|l| !l.is_empty());
    Err(match first {
        Some(line) => line.to_owned(),
        None => format!("{program} failed ({status})"),
    })
}

/// The settings on the exes a modded session in `game` starts.
pub fn session_admin_layers(game: &Path) -> Vec<AdminLayer> {
    SESSION_EXES
        .iter()
        .flat_map(|exe| admin_layers(&game.join(exe)))
        .collect()
}

/// Why a modded session in `game` cannot start, when one of its exes is set
/// to run as administrator and this process is not elevated.
pub fn elevation_blocker(game: &Path) -> Option<String> {
    let found = session_admin_layers(game);
    if found.is_empty() || elevated() {
        return None;
    }
    Some(explain(&found, game))
}

pub fn explain(found: &[AdminLayer], game: &Path) -> String {
    let mut names: Vec<&str> = found.iter().map(|l| l.name.as_str()).collect();
    names.dedup();
    let (is, it) = if names.len() > 1 {
        ("are", "them")
    } else {
        ("is", "it")
    };
    let names = names.join(" and ");
    let all_users = found.iter().any(|l| l.all_users);
    format!(
        "{names} {is} set to run as administrator ({}), and Windows will not start {it} for a modded \
         session while Swofty Launcher runs unelevated (error 740). Right-click {names} in {}, \
         open Properties > Compatibility and untick \"Run this program as an administrator\"{}, \
         then press Play again, or run Swofty Launcher as administrator",
        if all_users {
            "for all users"
        } else {
            "for this user"
        },
        game.display(),
        if all_users {
            " under \"Change settings for all users\""
        } else {
            ""
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = concat!(
        "\r\n",
        "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows NT\\CurrentVersion\\AppCompatFlags\\Layers\r\n",
        "    C:\\Program Files (x86)\\Steam\\steamapps\\common\\F1 25\\F1_25.exe    REG_SZ    ~ RUNASADMIN\r\n",
        "    C:\\Games\\Other\\other.exe    REG_SZ    ~ HIGHDPIAWARE\r\n",
        "    C:\\Games\\Tool\\tool.exe    REG_EXPAND_SZ    RUNASADMIN WIN7RTM\r\n",
        "    C:\\Games\\Empty\\empty.exe    REG_SZ    \r\n",
        "    C:\\Games\\Flag\\flag.exe    REG_DWORD    0x1\r\n",
        "\r\n",
    );

    #[test]
    fn parses_reg_query_listings() {
        let values = parse_reg_query(LISTING);
        assert_eq!(
            values,
            [
                (
                    r"C:\Program Files (x86)\Steam\steamapps\common\F1 25\F1_25.exe",
                    "~ RUNASADMIN"
                ),
                (r"C:\Games\Other\other.exe", "~ HIGHDPIAWARE"),
                (r"C:\Games\Tool\tool.exe", "RUNASADMIN WIN7RTM"),
                (r"C:\Games\Empty\empty.exe", ""),
            ]
            .map(|(n, d)| (n.to_owned(), d.to_owned()))
        );
        assert!(
            parse_reg_query("ERROR: The system was unable to find the specified registry key or value.\r\n")
                .is_empty()
        );
    }

    #[test]
    fn reads_dword_values() {
        let listing = "\r\nHKEY_CURRENT_USER\\Software\\Valve\\Steam\\ActiveProcess\r\n    \
                       pid    REG_DWORD    0x4c8\r\n    ActiveUser    REG_DWORD    0x0\r\n    \
                       Universe    REG_SZ    Public\r\n";
        assert_eq!(reg_dword(listing, "pid"), Some(0x4c8));
        assert_eq!(reg_dword(listing, "activeuser"), Some(0));
        assert_eq!(reg_dword(listing, "Universe"), None);
        assert_eq!(reg_dword(listing, "SteamPID"), None);
    }

    #[test]
    fn reads_the_runasadmin_layer() {
        assert!(has_run_as_admin("~ RUNASADMIN"));
        assert!(has_run_as_admin("RUNASADMIN HIGHDPIAWARE"));
        assert!(has_run_as_admin("~ HIGHDPIAWARE runasadmin"));
        assert!(!has_run_as_admin("~ HIGHDPIAWARE"));
        assert!(!has_run_as_admin("~ RUNASINVOKER"));
        assert!(!has_run_as_admin("~ NORUNASADMIN"));
        assert!(!has_run_as_admin(""));
    }

    #[test]
    fn matches_the_game_however_the_path_is_spelled() {
        let game = Path::new(r"c:\program files (x86)\steam\steamapps\common\F1 25");
        let found = matching(LISTING, false, &game.join(GAME_EXE));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].name, GAME_EXE);
        assert_eq!(found[0].layers, "~ RUNASADMIN");
        assert!(!found[0].all_users);
        assert!(matching(LISTING, false, Path::new(r"C:\Games\Other\other.exe")).is_empty());
        let slashes = Path::new("C:/Program Files (x86)/Steam/steamapps/common/F1 25/F1_25.exe");
        assert_eq!(matching(LISTING, true, slashes).len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn matches_the_game_through_a_linked_folder() {
        let d = std::env::temp_dir().join(format!("swofty-compat-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(d.join("F1 25")).unwrap();
        std::fs::write(d.join("F1 25").join(GAME_EXE), "MZ").unwrap();
        std::os::unix::fs::symlink(d.join("F1 25"), d.join("library")).unwrap();
        let linked = d.join("library").join(GAME_EXE);
        let listing = format!("    {}    REG_SZ    ~ RUNASADMIN\n", linked.display());
        let found = matching(&listing, false, &d.join("F1 25").join(GAME_EXE));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].exe, linked.display().to_string());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn says_what_to_untick() {
        let game = Path::new(r"C:\Games\F1 25");
        let user = AdminLayer {
            all_users: false,
            name: GAME_EXE.into(),
            exe: r"C:\Games\F1 25\F1_25.exe".into(),
            layers: "~ RUNASADMIN".into(),
        };
        let text = explain(std::slice::from_ref(&user), game);
        assert!(
            text.starts_with("F1_25.exe is set to run as administrator (for this user)"),
            "{text}"
        );
        assert!(
            text.contains(r"Right-click F1_25.exe in C:\Games\F1 25"),
            "{text}"
        );
        assert!(
            text.contains("untick \"Run this program as an administrator\", then press Play"),
            "{text}"
        );
        assert!(
            text.ends_with("or run Swofty Launcher as administrator"),
            "{text}"
        );
        let all = AdminLayer {
            all_users: true,
            ..user.clone()
        };
        let text = explain(&[user, all.clone()], game);
        assert!(
            text.starts_with("F1_25.exe is set to run as administrator (for all users)"),
            "{text}"
        );
        assert!(text.contains("under \"Change settings for all users\""), "{text}");
        let both = AdminLayer {
            name: "EAAntiCheat.GameServiceLauncher.exe".into(),
            exe: r"C:\Games\F1 25\EAAntiCheat.GameServiceLauncher.exe".into(),
            ..all.clone()
        };
        let text = explain(&[all, both], game);
        assert!(
            text.starts_with("F1_25.exe and EAAntiCheat.GameServiceLauncher.exe are set to run"),
            "{text}"
        );
        assert!(text.contains("will not start them"), "{text}");
    }

    #[cfg(not(windows))]
    #[test]
    fn never_blocks_off_windows() {
        assert!(elevation_blocker(Path::new("/games/F1 25")).is_none());
    }
}
