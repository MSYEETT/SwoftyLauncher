//! Crash causes the Send Logs triage keeps finding, recognised from a crashed
//! session's own files: the `[crash]`, `[overlay]` and `[offline] module:`
//! lines of its loader.log, the crash report the core wrote, and the files in
//! the game folder. The launcher shows the cause and its fix when one of the
//! rules here matches, and Send Logs and the support Discord when none does.
//! An addon whose changes don't apply to an F1 25 older than the newest
//! update is recognised from the build's error, for the launch's failure.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use crate::install::LOADER_DIR;
use crate::overlay_guard::SessionOverlay;
use crate::session::SessionRecord;

/// The most of a loader.log or crash report that is read.
const READ_CAP: u64 = 32 << 20;
/// The stack frames, nearest first, that count as the crash's own.
const NEAR_FRAMES: usize = 8;
/// The files a step names before it says how many more there are.
const NAMED_FILES: usize = 6;

/// The newest F1 25 update the launcher knows, as `F1_25.exe`'s file
/// version: Steam build 25251167, and the EA app's copy of the same update.
pub const LATEST_GAME_VERSION: &str = "1.0.141.7865";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnosis {
    /// A stable name for the rule that matched, for logs and Send Logs.
    pub id: &'static str,
    pub title: String,
    pub what: String,
    pub steps: Vec<String>,
}

impl Diagnosis {
    /// What happened, then its steps, as one paragraph.
    pub fn text(&self) -> String {
        std::iter::once(&self.what)
            .chain(&self.steps)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// A game version's four numbers, as in `1.0.141.7865`.
fn version_numbers(version: &str) -> Option<[u32; 4]> {
    let mut parts = version.trim().split('.');
    let mut numbers = [0; 4];
    for n in &mut numbers {
        let part = parts.next()?;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *n = part.parse().ok()?;
    }
    parts.next().is_none().then_some(numbers)
}

/// Whether game version `version` is older than `than`, number by number.
/// None when either isn't a version.
pub fn older_game(version: &str, than: &str) -> Option<bool> {
    Some(version_numbers(version)? < version_numbers(than)?)
}

/// The addon whose changes failed to apply, from a build error such as
/// `applying swofty.classic-seasons: eaid://…: no entry [Name=…]`, on its
/// own or after what was being done when it failed.
fn failed_addon(error: &str) -> Option<&str> {
    let rest = error
        .strip_prefix("applying ")
        .or_else(|| error.split_once(": applying ").map(|(_, rest)| rest))?;
    let id = rest.split([':', ' ', '\n']).next()?;
    let valid = !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    valid.then_some(id)
}

/// An addon whose changes failed to apply to an F1 25 older than
/// [`LATEST_GAME_VERSION`]: it needs that update. None when `error` isn't an
/// addon's changes failing, or when `version`, the game's own, is unknown or
/// not older.
pub fn outdated_game(
    error: &str,
    version: Option<&str>,
    names: &BTreeMap<String, String>,
) -> Option<Diagnosis> {
    let version = version?.trim();
    if older_game(version, LATEST_GAME_VERSION) != Some(true) {
        return None;
    }
    let id = failed_addon(error)?;
    let name = names.get(id).map_or(id, String::as_str);
    Some(Diagnosis {
        id: "outdated_game",
        title: format!("{name} needs the latest F1 25"),
        what: format!(
            "{name} needs F1 25 {LATEST_GAME_VERSION}, the latest update. Your F1 25 is {version}."
        ),
        steps: vec!["Update F1 25 in Steam or the EA app, then press Play again.".to_owned()],
    })
}

/// An F1 25 older than [`LATEST_GAME_VERSION`], before a modded launch: the
/// mod loader can't hook into it, so addons may not load. None when the
/// version is unknown or not older.
pub fn outdated_exe(version: Option<&str>) -> Option<Diagnosis> {
    let version = version?.trim();
    if older_game(version, LATEST_GAME_VERSION) != Some(true) {
        return None;
    }
    Some(Diagnosis {
        id: "outdated_exe",
        title: "F1 25 needs the latest update".to_owned(),
        what: format!(
            "Your F1 25 is {version}. Mods are made for {LATEST_GAME_VERSION}, the latest update, and may not \
             load on an older one."
        ),
        steps: vec![format!(
            "Update F1 25 in Steam or the EA app. If Steam says it is up to date, verify the game files \
             ({}), then press Play again.",
            crate::stock::STEAM_VERIFY_URL
        )],
    })
}

/// What a crashed session left behind, as the rules read it.
#[derive(Debug, Clone, Default)]
pub struct Evidence {
    /// The session's loader.log.
    pub loader: String,
    /// The crash report the core wrote for the session, when it can be found.
    pub report: Option<String>,
    /// The names of the files and folders in the game folder.
    pub game_files: Vec<String>,
    /// Addon names by id, for addons the rules name.
    pub names: BTreeMap<String, String>,
}

impl Evidence {
    pub fn read(session: &SessionRecord, game: Option<&Path>, names: BTreeMap<String, String>) -> Self {
        let loader = ["loader.log", "game-logs/loader.log"]
            .iter()
            .map(|rel| session.dir.join(rel))
            .find(|p| p.is_file())
            .and_then(|p| read_capped(&p))
            .unwrap_or_default();
        let report = report_name(&loader).and_then(|name| {
            let mut dirs = vec![session.dir.clone(), session.dir.join("game-logs")];
            if let Some(game) = game {
                dirs.insert(0, game.join(LOADER_DIR));
            }
            dirs.iter()
                .map(|d| d.join(&name))
                .find(|p| p.is_file())
                .and_then(|p| read_capped(&p))
        });
        let game_files = game
            .and_then(|g| std::fs::read_dir(g).ok())
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        Self {
            loader,
            report,
            game_files,
            names,
        }
    }
}

/// The cause of the crash `session` ended with, when a rule recognises it.
pub fn crash(
    session: &SessionRecord,
    game: Option<&Path>,
    names: BTreeMap<String, String>,
) -> Option<Diagnosis> {
    diagnose(&Evidence::read(session, game, names))
}

fn read_capped(path: &Path) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(READ_CAP)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The crash report's file name from the loader's `crash report swofty\crash-….txt:` line.
fn report_name(loader: &str) -> Option<String> {
    loader.lines().rev().find_map(|line| {
        let rest = &line[line.find("[crash] crash report ")? + "[crash] crash report ".len()..];
        let path = rest.split(':').next()?;
        let name = path.rsplit(['\\', '/']).next()?;
        (name.starts_with("crash-") && name.ends_with(".txt")).then(|| name.to_owned())
    })
}

/// The fatal exception as the loader log and the crash report describe it.
#[derive(Debug, Clone, Default, PartialEq)]
struct Fatal {
    /// The module the exception happened in, lower case.
    module: Option<String>,
    /// The modules on the crashing thread's stack, nearest first, lower case.
    stack: Vec<String>,
    /// Modules whose return addresses the crash report found on the stack.
    returns: Vec<String>,
}

impl Fatal {
    fn near(&self) -> impl Iterator<Item = &str> {
        self.module
            .iter()
            .chain(self.stack.iter().take(NEAR_FRAMES))
            .map(String::as_str)
    }

    fn all(&self) -> impl Iterator<Item = &str> {
        self.near().chain(self.returns.iter().map(String::as_str))
    }
}

/// The module in `text` before its first `+0x`, as in `F1_25.exe+0x39182a4`.
fn module_at(text: &str) -> Option<String> {
    let at = text.find("+0x")?;
    let name = text[..at].rsplit([' ', '\t']).next()?;
    (!name.is_empty() && !name.starts_with("0x")).then(|| name.to_ascii_lowercase())
}

fn fatal(e: &Evidence) -> Fatal {
    let mut f = Fatal::default();
    let crash: Vec<&str> = e
        .loader
        .lines()
        .filter_map(|l| Some(&l[l.find("[crash] ")? + "[crash] ".len()..]))
        .collect();
    let unhandled = crash
        .iter()
        .find(|l| l.starts_with("unhandled ") && l.contains("reached the top-level filter"));
    f.module = unhandled.and_then(|l| module_at(&l[l.find(" at ")? + 4..]));
    let address = unhandled.and_then(|l| {
        let at = &l[l.find(" at ")? + 4..];
        Some(at.split_whitespace().next()?.to_owned())
    });
    if let Some(address) = &address {
        let first = crash
            .iter()
            .position(|l| l.contains("first-chance") && l.contains(address.as_str()));
        if let Some(i) = first {
            f.stack = crash[i + 1..]
                .iter()
                .take_while(|l| l.starts_with("  "))
                .filter(|l| l.trim_start().starts_with('#'))
                .filter_map(|l| module_at(l))
                .collect();
        }
    }
    if let Some(report) = &e.report {
        let mut section = "";
        let mut stack = Vec::new();
        for line in report.lines() {
            if let Some(name) = line.strip_prefix("== ") {
                section = name;
                continue;
            }
            if section == "Exception" && f.module.is_none() {
                if let Some(at) = line.find(" at ") {
                    f.module = module_at(&line[at + 4..]);
                }
            } else if section.starts_with("Stack (") {
                stack.extend(line.split_whitespace().nth(3).and_then(module_at));
            } else if section.starts_with("Return addresses")
                && let Some(m) = line.split_whitespace().nth(2).and_then(module_at)
                && !f.returns.contains(&m)
            {
                f.returns.push(m);
            }
        }
        if f.stack.is_empty() {
            f.stack = stack;
        }
    }
    f
}

/// The modules the loader and the offline play hook saw in the game process.
fn loaded_modules(loader: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in loader.lines() {
        let Some(rest) = ["[offline] module: ", "[loader] module: "]
            .iter()
            .find_map(|tag| Some(&line[line.find(tag)? + tag.len()..]))
        else {
            continue;
        };
        let name = rest.split(" @").next().unwrap_or(rest).trim();
        if !name.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(name)) {
            out.push(name.to_owned());
        }
    }
    out
}

/// Whether the crash's lines (and the crash report) hold any of `codes`, the
/// HRESULTs the game hands its fatal abort, as in `rdi=00000000887a0005`.
fn crash_holds(e: &Evidence, codes: &[&str]) -> bool {
    let hit = |l: &str| {
        let l = l.to_ascii_lowercase();
        codes.iter().any(|c| l.contains(c))
    };
    e.loader
        .lines()
        .filter(|l| l.contains("[crash]") || l.contains("[overlay] the GPU was removed"))
        .any(hit)
        || e.report.as_deref().is_some_and(|r| r.lines().any(hit))
}

fn gpu_removed(e: &Evidence) -> bool {
    crash_holds(
        e,
        &[
            "887a0005",
            "887a0006",
            "887a0007",
            "dxgi_error_device_removed",
            "dxgi_error_device_hung",
            "dxgi_error_device_reset",
        ],
    )
}

/// Whether the game ran under Wine or Proton, where vkd3d reports its own
/// failures (a pipeline it can't build) with the same codes as a Windows
/// driver out of memory or lost.
fn wine(e: &Evidence, loaded: &[String]) -> bool {
    e.report
        .as_deref()
        .is_some_and(|r| r.lines().any(|l| l.starts_with("runtime") && l.contains("Wine")))
        || loaded.iter().any(|m| {
            let m = m.to_ascii_lowercase();
            m.starts_with("winex11") || m.starts_with("winewayland") || m.starts_with("winevulkan")
        })
}

fn out_of_memory(e: &Evidence) -> bool {
    crash_holds(e, &["8007000e", "e_outofmemory"])
}

/// The names of the addons that drew with Swofty's overlay in the session.
fn painters(e: &Evidence, overlay: &SessionOverlay) -> Vec<String> {
    overlay
        .painters
        .iter()
        .map(|id| e.names.get(id).cloned().unwrap_or_else(|| id.clone()))
        .collect()
}

/// Programs that hook into games as they run, by the start of their DLL's name.
struct Hook {
    prefix: &'static str,
    product: &'static str,
    step: &'static str,
    /// Only when the exception happened in it: the DLL also sits harmlessly
    /// on stacks of crashes it had no part in.
    own_only: bool,
}

const HOOKS: &[Hook] = &[
    Hook {
        prefix: "rtsshooks",
        product: "RivaTuner Statistics Server (RTSS), which comes with MSI Afterburner",
        step: "Close RivaTuner Statistics Server and MSI Afterburner, including from the system tray, before you \
               press Play. Or, in RTSS, add F1_25.exe and set its Application detection level to None.",
        own_only: false,
    },
    Hook {
        prefix: "windhawk",
        product: "Windhawk",
        step: "Quit Windhawk from its tray icon before you press Play, or add F1_25.exe to the processes \
               Windhawk doesn't inject into in its settings.",
        own_only: false,
    },
    Hook {
        prefix: "medal-hook",
        product: "Medal",
        step: "Close Medal, including from the system tray, before you press Play.",
        own_only: false,
    },
    Hook {
        prefix: "discordhook",
        product: "Discord's in-game overlay",
        step: "Turn off Discord's in-game overlay in Discord's settings (Game Overlay).",
        own_only: false,
    },
    Hook {
        prefix: "graphics-hook",
        product: "OBS Studio's game capture",
        step: "In OBS Studio, capture F1 25 with Window Capture or Display Capture instead of Game Capture, or \
               close OBS while you play.",
        own_only: false,
    },
    Hook {
        prefix: "nvpresent",
        product: "the NVIDIA app's overlay and frame tools",
        step: "Turn off the in-game overlay and Smooth Motion for F1 25 in the NVIDIA app.",
        own_only: true,
    },
    Hook {
        prefix: "nvspcap",
        product: "the NVIDIA app's overlay",
        step: "Turn off the in-game overlay in the NVIDIA app's settings.",
        own_only: true,
    },
    Hook {
        prefix: "gameoverlayrenderer",
        product: "the Steam overlay",
        step: "Turn off the Steam overlay for F1 25: right-click F1 25 in Steam, Properties, General, and untick \
               Enable the Steam Overlay while in-game.",
        own_only: true,
    },
    Hook {
        prefix: "specialk",
        product: "Special K",
        step: "Turn Special K off for F1 25, or remove it from the game folder.",
        own_only: false,
    },
    Hook {
        prefix: "pid.dll",
        product: "Windows' force feedback driver for your wheel",
        step: "Update your wheel's driver and software (for example Logitech G HUB, Fanatec, Thrustmaster or \
               Moza), plug the wheel straight into the PC, and make sure only one wheel program is running.",
        own_only: true,
    },
];

/// Graphics drivers' user-mode DLLs, by the start of their name.
const DRIVERS: &[(&str, &str)] = &[
    ("nvwgf2um", "NVIDIA"),
    ("nvd3dum", "NVIDIA"),
    ("nvoglv", "NVIDIA"),
    ("nvlddmkm", "NVIDIA"),
    ("amdxc64", "AMD"),
    ("amdxx64", "AMD"),
    ("atidxx64", "AMD"),
    ("igd12umd", "Intel"),
    ("igd10iumd", "Intel"),
    ("igdumdim", "Intel"),
    ("igc64", "Intel"),
];

/// The game's frame generation DLLs.
const FRAME_GEN: &[(&str, &str)] = &[
    ("nvngx_dlssg", "DLSS Frame Generation"),
    ("sl.dlss_g", "DLSS Frame Generation"),
    ("libxess_fg", "XeSS frame generation"),
];

/// Modules that mean the crash happened in graphics code.
fn graphics(module: &str) -> bool {
    module.starts_with("sl.")
        || module.starts_with("nvngx")
        || module.starts_with("dxgi")
        || module.starts_with("d3d1")
        || module.starts_with("dxcore")
        || module.starts_with("libxess")
        || module.starts_with("amd_fidelityfx")
        || DRIVERS.iter().any(|(p, _)| module.starts_with(p))
}

/// Names in `loaded` that only graphics mods use; Windows and the drivers
/// ship DLLs named like the others (dxgi.dll, nvapi64.dll).
fn distinctive(name: &str) -> bool {
    let f = name.to_ascii_lowercase();
    f.starts_with("reshade")
        || f.starts_with("optiscaler")
        || f.starts_with("specialk")
        || f.starts_with("dlssg_to_fsr3")
        || f.ends_with(".asi")
        || f.ends_with(".addon64")
        || f.ends_with(".addon")
}

/// Files in the game folder that F1 25 doesn't ship and that hook its
/// graphics, plus the distinctive ones among the game's loaded modules,
/// grouped by the tool they come from.
fn graphics_mods(game_files: &[String], loaded: &[String]) -> Vec<(&'static str, Vec<String>)> {
    let mut files = game_files.to_vec();
    for m in loaded.iter().filter(|m| distinctive(m)) {
        if !files.iter().any(|f| f.eq_ignore_ascii_case(m)) {
            files.push(m.clone());
        }
    }
    let files = &files;
    let mut groups: Vec<(&'static str, Vec<String>)> = Vec::new();
    let mut add = |tool: &'static str, name: &str| match groups.iter_mut().find(|(t, _)| *t == tool) {
        Some((_, names)) => names.push(name.to_owned()),
        None => groups.push((tool, vec![name.to_owned()])),
    };
    let reshade = files.iter().any(|f| {
        let f = f.to_ascii_lowercase();
        f == "reshade.ini" || f.starts_with("reshade64") || f.ends_with(".addon64") || f.ends_with(".addon")
    });
    let optiscaler = files
        .iter()
        .any(|f| f.to_ascii_lowercase().starts_with("optiscaler"));
    for name in files {
        let f = name.to_ascii_lowercase();
        let tool = if f.starts_with("optiscaler")
            || f == "nvngx.dll"
            || f == "fakenvapi.ini"
            || f == "nvapi64.dll"
            || f.starts_with("dlssg_to_fsr3")
            || f == "amdxc64.dll"
            || f == "libxess_dx11.dll"
        {
            "OptiScaler"
        } else if f == "reshade.ini"
            || f.starts_with("reshade64")
            || f.ends_with(".addon64")
            || f.ends_with(".addon")
            || f == "reshade-shaders"
        {
            "ReShade"
        } else if f == "dxgi.dll" || f == "d3d12.dll" || f == "d3d11.dll" {
            if optiscaler {
                "OptiScaler"
            } else if reshade {
                "ReShade"
            } else {
                "a graphics mod"
            }
        } else if f.starts_with("specialk") {
            "Special K"
        } else if f.ends_with(".asi") || f == "winmm.dll" || f == "dinput8.dll" {
            "an ASI loader"
        } else {
            continue;
        };
        add(tool, name);
    }
    groups
}

fn list(names: &[String]) -> String {
    let shown: Vec<&str> = names.iter().take(NAMED_FILES).map(String::as_str).collect();
    let more = names.len().saturating_sub(NAMED_FILES);
    match (shown.as_slice(), more) {
        ([one], 0) => (*one).to_owned(),
        (_, 0) => {
            let (last, rest) = shown.split_last().expect("at least two names");
            format!("{} and {last}", rest.join(", "))
        }
        _ => format!("{} and {more} more", shown.join(", ")),
    }
}

fn tools(groups: &[(&'static str, Vec<String>)]) -> String {
    let names: Vec<String> = groups.iter().map(|(t, _)| (*t).to_owned()).collect();
    list(&names)
}

fn move_out(groups: &[(&'static str, Vec<String>)]) -> String {
    let files: Vec<String> = groups.iter().flat_map(|(_, f)| f.iter().cloned()).collect();
    format!(
        "Close Swofty Launcher, then move {} out of the F1 25 folder into a folder of their own, so you can put \
         them back later.",
        list(&files)
    )
}

const VERIFY: &str = "Verify F1 25's files: in Steam, right-click F1 25, Properties, Installed Files, Verify integrity \
                      of game files. For the EA app copy, use Repair in the EA app.";

fn driver_steps() -> Vec<String> {
    vec![
        "Update your graphics driver, or reinstall it cleanly. If you installed a new driver in the last few days, \
         try the one before it."
            .to_owned(),
        "Set any graphics card overclock or undervolt (MSI Afterburner, the NVIDIA app's tuning, AMD Adrenalin) \
         back to stock."
            .to_owned(),
        "Turn off frame generation (DLSS, FSR or XeSS frame generation) in F1 25's graphics settings.".to_owned(),
        "On a graphics card with 8 GB of memory or less, lower Texture Quality, or play with fewer car, livery \
         and track addons."
            .to_owned(),
    ]
}

/// The cause of a crash, from the first rule that recognises it.
pub fn diagnose(e: &Evidence) -> Option<Diagnosis> {
    let f = fatal(e);
    let loaded = loaded_modules(&e.loader);
    let mods = graphics_mods(&e.game_files, &loaded);
    for hook in HOOKS {
        let own = f.module.as_deref().is_some_and(|m| m.starts_with(hook.prefix));
        let near = !hook.own_only && f.near().any(|m| m.starts_with(hook.prefix));
        if !own && !near {
            continue;
        }
        let module = f
            .near()
            .find(|m| m.starts_with(hook.prefix))
            .unwrap_or(hook.prefix)
            .to_owned();
        return Some(Diagnosis {
            id: "third_party_hook",
            title: format!("F1 25 crashed inside {}", hook.product),
            what: format!(
                "The crash happened in {module}, which belongs to {}. It hooks into games while they run, and here \
                 it took F1 25 down with it.",
                hook.product
            ),
            steps: vec![hook.step.to_owned(), "Press Play again.".to_owned()],
        });
    }
    let windows = !wine(e, &loaded);
    if windows && gpu_removed(e) {
        let overlay = SessionOverlay::scan(&e.loader).unwrap_or_default();
        if overlay.removed.is_some() && overlay.drew && !overlay.painters.is_empty() {
            let addons = painters(e, &overlay);
            let mut steps = vec![format!(
                "Turn off {} in your profile, then play again.",
                list(&addons)
            )];
            steps.extend(driver_steps().into_iter().take(2));
            return Some(Diagnosis {
                id: "gpu_removed_overlay",
                title: "Your graphics card stopped responding while Swofty's overlay was drawing".to_owned(),
                what: format!(
                    "Windows reset the graphics driver (DXGI_ERROR_DEVICE_REMOVED) shortly after Swofty's in-game \
                     overlay drew for {}, and F1 25 can't recover from that, so it closed. The overlay is \
                     experimental.",
                    list(&addons)
                ),
                steps,
            });
        }
        let mut steps = Vec::new();
        if !mods.is_empty() {
            steps.push(move_out(&mods));
        }
        steps.extend(driver_steps());
        let also = if mods.is_empty() {
            String::new()
        } else {
            format!(
                " Your game folder also has {} installed, which hooks F1 25's graphics.",
                tools(&mods)
            )
        };
        return Some(Diagnosis {
            id: "gpu_removed",
            title: "Your graphics driver stopped responding".to_owned(),
            what: format!(
                "Windows reset the graphics driver while F1 25 was running (DXGI_ERROR_DEVICE_REMOVED). F1 25 can't \
                 recover from that, so it closed. This comes from the graphics card or its driver, not from \
                 Swofty's code.{also}"
            ),
            steps,
        });
    }
    if windows && out_of_memory(e) {
        return Some(Diagnosis {
            id: "out_of_memory",
            title: "F1 25 ran out of graphics memory".to_owned(),
            what: "The game couldn't get the memory it asked for (E_OUTOFMEMORY), so it closed. Car, livery and \
                   track addons use a lot of graphics memory, and several at once can use more than the graphics \
                   card has."
                .to_owned(),
            steps: vec![
                "Lower Texture Quality, and the resolution if it's above your screen's, in F1 25's graphics \
                 settings."
                    .to_owned(),
                "Play with fewer car, livery and track addons on at once, for example only the Classic Seasons \
                 years you race."
                    .to_owned(),
                "Close other programs, especially web browsers, while you play.".to_owned(),
                "On a laptop with integrated graphics, give the graphics more memory if its settings allow it \
                 (often called UMA frame buffer size)."
                    .to_owned(),
            ],
        });
    }
    let started = crate::launch::LoaderSeen::scan(&e.loader).window;
    if !mods.is_empty() && (!started || f.all().any(graphics)) {
        let when = if started {
            "in its graphics code"
        } else {
            "while it was setting up its graphics, before its window opened"
        };
        return Some(Diagnosis {
            id: "graphics_mods",
            title: format!("{} in your game folder crashed F1 25", tools(&mods)),
            what: format!(
                "Your F1 25 folder has {} installed. It changes how F1 25 draws, and F1 25 crashed {when}.",
                tools(&mods)
            ),
            steps: vec![
                move_out(&mods),
                VERIFY.to_owned(),
                "Press Play. If F1 25 starts, you can put the files back one at a time to find the one that breaks \
                 it."
                    .to_owned(),
            ],
        });
    }
    if let Some((module, vendor)) = f.module.as_deref().filter(|_| windows).and_then(|m| {
        DRIVERS
            .iter()
            .find(|(p, _)| m.starts_with(p))
            .map(|(_, v)| (m, *v))
    }) {
        return Some(Diagnosis {
            id: "graphics_driver",
            title: format!("F1 25 crashed inside your {vendor} graphics driver"),
            what: format!("The crash happened in {module}, part of the {vendor} graphics driver."),
            steps: driver_steps(),
        });
    }
    if let Some((module, what)) = f.module.as_deref().and_then(|m| {
        FRAME_GEN
            .iter()
            .find(|(p, _)| m.starts_with(p))
            .map(|(_, w)| (m, *w))
    }) {
        let overlay = SessionOverlay::scan(&e.loader).unwrap_or_default();
        let mut steps = vec![format!(
            "Turn off {what} in F1 25's graphics settings, then play again."
        )];
        let mut also = String::new();
        if overlay.drew && !overlay.painters.is_empty() {
            let addons = list(&painters(e, &overlay));
            also = format!(
                " Swofty's in-game overlay was drawing for {addons} at the time, and it is experimental."
            );
            steps.insert(0, format!("Turn off {addons} in your profile."));
        }
        steps.push("Update your graphics driver.".to_owned());
        return Some(Diagnosis {
            id: "frame_generation",
            title: format!("F1 25 crashed in {what}"),
            what: format!("The crash happened in {module}, the game's {what} code.{also}"),
            steps,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const GPU: &str = "\
2026-10-24 02:52:26.633 [33008] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 33008, 463 s after load, at F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4
2026-10-24 02:52:26.633 [33008] [crash]   writing 0x1
2026-10-24 02:52:26.633 [33008] [crash]   rsi=00000001c3603440 rdi=00000000887a0005 rbp=00000000887a002b rsp=000000000015be50
2026-10-24 02:52:26.633 [33008] [crash]   stack:
2026-10-24 02:52:26.633 [33008] [crash]   #0  F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4
2026-10-24 02:52:26.633 [33008] [crash]   #1  F1_25.exe+0x39b8802 .xtls fn base+0x39b8510+0x2f2
2026-10-24 02:52:27.289 [20344] [crash] crash report swofty\\crash-20261024-025226.txt: the game aborted itself
2026-10-24 02:52:27.289 [33008] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4 reached the top-level filter; passing it to the game's filter
";

    fn crash_in(module: &str) -> String {
        format!(
            "2026-10-26 14:01:42.000 [1] [crash] #1 first-chance (the game may still handle it) BREAKPOINT (0x80000003) on thread 1, 21 s after load, at {module}+0x178fd
2026-10-26 14:01:42.000 [1] [crash]   stack:
2026-10-26 14:01:42.000 [1] [crash]   #0  {module}+0x178fd
2026-10-26 14:01:42.000 [1] [crash]   #1  F1_25.exe+0x1234 .xtls fn base+0x1200+0x34
2026-10-26 14:01:42.100 [1] [crash] unhandled BREAKPOINT (0x80000003) at {module}+0x178fd reached the top-level filter; passing it to the game's filter
"
        )
    }

    fn ev(loader: &str, files: &[&str]) -> Evidence {
        Evidence {
            loader: loader.to_owned(),
            game_files: files.iter().map(|f| (*f).to_owned()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn reads_the_fatal_exception_and_its_stack() {
        let f = fatal(&ev(GPU, &[]));
        assert_eq!(f.module.as_deref(), Some("f1_25.exe"));
        assert_eq!(f.stack, ["f1_25.exe", "f1_25.exe"]);
        assert_eq!(report_name(GPU).as_deref(), Some("crash-20261024-025226.txt"));
    }

    #[test]
    fn a_removed_gpu_is_the_driver() {
        let d = diagnose(&ev(GPU, &["F1_25.exe", "sl.common.dll"])).unwrap();
        assert_eq!(d.id, "gpu_removed");
        assert!(d.what.contains("not from Swofty's code"), "{}", d.what);
        assert!(d.steps[0].starts_with("Update your graphics driver"));
    }

    #[test]
    fn a_removed_gpu_with_graphics_mods_names_them_first() {
        let d = diagnose(&ev(
            GPU,
            &["F1_25.exe", "dxgi.dll", "ReShade.ini", "dlss5-feed.addon64"],
        ))
        .unwrap();
        assert_eq!(d.id, "gpu_removed");
        assert!(d.what.contains("ReShade"), "{}", d.what);
        assert!(
            d.steps[0].contains("dxgi.dll, ReShade.ini and dlss5-feed.addon64"),
            "{}",
            d.steps[0]
        );
    }

    #[test]
    fn a_removed_gpu_after_the_overlay_drew_names_the_addons() {
        let log = format!(
            "{GPU}2026-10-24 02:52:26.000 [1] [overlay] api: kizi.pitlane on_paint -> ok\n\
             2026-10-24 02:52:26.000 [1] [overlay] renderer up: d3d12 swapchain 1920x1080\n\
             2026-10-24 02:52:26.500 [1] [overlay] the GPU was removed (present 0x887a0005, reason 0x887a0006) 4.8 s after the overlay last drew; the overlay stops drawing\n"
        );
        let mut e = ev(&log, &[]);
        e.names.insert("kizi.pitlane".into(), "Manual Pit Lane".into());
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "gpu_removed_overlay");
        assert_eq!(
            d.steps[0],
            "Turn off Manual Pit Lane in your profile, then play again."
        );
    }

    #[test]
    fn a_crash_inside_a_hook_names_its_program() {
        let d = diagnose(&ev(&crash_in("windhawk.dll"), &[])).unwrap();
        assert_eq!(d.id, "third_party_hook");
        assert_eq!(d.title, "F1 25 crashed inside Windhawk");
        let d = diagnose(&ev(&crash_in("RTSSHooks64.dll"), &[])).unwrap();
        assert!(d.title.contains("RivaTuner"), "{}", d.title);
        let d = diagnose(&ev(&crash_in("pid.dll"), &[])).unwrap();
        assert!(d.title.contains("force feedback"), "{}", d.title);
    }

    #[test]
    fn the_steam_overlay_counts_only_when_the_crash_is_in_it() {
        let log = crash_in("F1_25.exe").replace(
            "#1  F1_25.exe+0x1234 .xtls fn base+0x1200+0x34",
            "#1  GameOverlayRenderer64.dll+0x1234",
        );
        assert_eq!(diagnose(&ev(&log, &[])), None);
        let d = diagnose(&ev(&crash_in("GameOverlayRenderer64.dll"), &[])).unwrap();
        assert!(d.title.contains("Steam overlay"));
    }

    #[test]
    fn graphics_mods_before_the_window_are_named() {
        let d = diagnose(&ev(
            &crash_in("sl.common.dll"),
            &[
                "F1_25.exe",
                "OptiScaler.ini",
                "nvngx.dll",
                "dxgi.dll",
                "OptiPatcher.asi",
            ],
        ))
        .unwrap();
        assert_eq!(d.id, "graphics_mods");
        assert!(d.title.starts_with("OptiScaler and an ASI loader"), "{}", d.title);
        assert!(d.what.contains("before its window opened"), "{}", d.what);
        assert!(d.steps[0].contains("OptiScaler.ini, nvngx.dll, dxgi.dll and OptiPatcher.asi"));
    }

    #[test]
    fn graphics_mods_play_no_part_in_a_game_code_crash_after_the_window() {
        let log = format!(
            "2026-10-26 14:00:00.000 [1] trigger: main window visible\n{}",
            crash_in("F1_25.exe")
        );
        assert_eq!(
            diagnose(&ev(&log, &["F1_25.exe", "dxgi.dll", "ReShade.ini"])),
            None
        );
    }

    #[test]
    fn a_crash_in_a_graphics_driver_names_the_vendor() {
        let d = diagnose(&ev(&crash_in("amdxc64.dll"), &["F1_25.exe"])).unwrap();
        assert_eq!(d.id, "graphics_driver");
        assert_eq!(d.title, "F1 25 crashed inside your AMD graphics driver");
        let d = diagnose(&ev(
            &crash_in("amdxc64.dll"),
            &["F1_25.exe", "amdxc64.dll", "OptiScaler.ini"],
        ))
        .unwrap();
        assert_eq!(d.id, "graphics_mods", "OptiScaler's own amdxc64.dll");
    }

    #[test]
    fn a_crash_in_frame_generation_says_turn_it_off() {
        let log = format!(
            "2026-10-26 14:00:00.000 [1] trigger: main window visible\n{}",
            crash_in("libxess_fg.dll")
        );
        let d = diagnose(&ev(&log, &["F1_25.exe"])).unwrap();
        assert_eq!(d.id, "frame_generation");
        assert_eq!(
            d.steps[0],
            "Turn off XeSS frame generation in F1 25's graphics settings, then play again."
        );
    }

    #[test]
    fn running_out_of_memory_says_so() {
        let d = diagnose(&ev(&GPU.replace("887a0005", "8007000e"), &[])).unwrap();
        assert_eq!(d.id, "out_of_memory");
    }

    #[test]
    fn under_wine_the_windows_driver_rules_stay_out() {
        let log = format!(
            "{}2026-10-21 07:05:50.000 [1] [offline] module: winex11.drv @ 0x6ffffde80000\n",
            GPU.replace("887a0005", "8007000e")
        );
        assert_eq!(diagnose(&ev(&log, &[])), None);
    }

    #[test]
    fn a_plain_game_crash_has_no_diagnosis() {
        assert_eq!(
            diagnose(&ev(&crash_in("F1_25.exe"), &["F1_25.exe", "sl.common.dll"])),
            None
        );
        assert_eq!(diagnose(&ev("", &[])), None);
    }

    /// `SWOFTY_KP_CASES=<folder of extracted Send Logs bundles> cargo test -p swofty-core --lib
    /// diagnoses_real_bundles -- --ignored --nocapture` prints each crashed session's diagnosis.
    #[test]
    #[ignore]
    fn diagnoses_real_bundles() {
        let Some(root) = std::env::var_os("SWOFTY_KP_CASES") else {
            return;
        };
        let dirs = |p: &Path| -> Vec<std::path::PathBuf> {
            let mut v: Vec<_> = std::fs::read_dir(p)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            v.sort();
            v
        };
        for case in dirs(Path::new(&root)) {
            let logs = case.join("logs");
            for session in dirs(&logs.join("sessions")) {
                let Some(loader) = read_capped(&session.join("loader.log")) else {
                    continue;
                };
                if !loader.contains("reached the top-level filter") {
                    continue;
                }
                let report = report_name(&loader).and_then(|n| read_capped(&logs.join("crash").join(n)));
                let e = Evidence {
                    loader,
                    report,
                    ..Default::default()
                };
                let found = diagnose(&e).map_or_else(|| "-".to_owned(), |d| format!("{}\t{}", d.id, d.title));
                println!(
                    "KP\t{}\t{}\t{found}",
                    case.file_name().unwrap().to_string_lossy(),
                    session.file_name().unwrap().to_string_lossy()
                );
            }
        }
    }

    const APPLY_FAILED: &str = "applying swofty.classic-seasons: eaid://ui_package/databases/overrides.uidb: \
                                overrides.Contents.teams.Contents.mp_team_select_logos.Contents: no entry \
                                [Name=vanamersfoort_26]";

    fn classic_seasons() -> BTreeMap<String, String> {
        BTreeMap::from([("swofty.classic-seasons".to_owned(), "Classic Seasons".to_owned())])
    }

    #[test]
    fn game_versions_compare_by_number() {
        assert_eq!(older_game("1.0.138.5601", LATEST_GAME_VERSION), Some(true));
        assert_eq!(older_game("1.0.136.6820", "1.0.141.7865"), Some(true));
        assert_eq!(older_game("1.0.141.7865", LATEST_GAME_VERSION), Some(false));
        assert_eq!(older_game("1.0.142.1", LATEST_GAME_VERSION), Some(false));
        assert_eq!(older_game("1.0.141.10000", "1.0.141.7865"), Some(false));
        assert_eq!(older_game("1.0.99.9999", "1.0.141.7865"), Some(true));
        assert_eq!(older_game(" 1.0.137.2811 ", LATEST_GAME_VERSION), Some(true));
        for unknown in [
            "",
            "1.0.138",
            "1.0.138.5601.2",
            "1.0.x.5601",
            "25251167",
            "1..138.5601",
            "1.0.+8.5601",
        ] {
            assert_eq!(older_game(unknown, LATEST_GAME_VERSION), None, "{unknown}");
        }
    }

    #[test]
    fn an_addon_that_does_not_apply_to_an_old_game_names_the_update() {
        let d = outdated_game(APPLY_FAILED, Some("1.0.136.6820"), &classic_seasons()).unwrap();
        assert_eq!(d.id, "outdated_game");
        assert_eq!(d.title, "Classic Seasons needs the latest F1 25");
        assert_eq!(
            d.text(),
            "Classic Seasons needs F1 25 1.0.141.7865, the latest update. Your F1 25 is 1.0.136.6820. Update F1 \
             25 in Steam or the EA app, then press Play again."
        );
        let wrapped = format!("the wrapper exited (exit code: 1) before the game started: {APPLY_FAILED}");
        let d = outdated_game(&wrapped, Some("1.0.138.5601"), &BTreeMap::new()).unwrap();
        assert!(
            d.what
                .starts_with("swofty.classic-seasons needs F1 25 1.0.141.7865"),
            "{}",
            d.what
        );
        let item = APPLY_FAILED.replace("classic-seasons:", "classic-seasons (2018):");
        let d = outdated_game(&item, Some("1.0.137.2811"), &classic_seasons()).unwrap();
        assert!(d.what.starts_with("Classic Seasons needs"), "{}", d.what);
    }

    #[test]
    fn other_games_and_other_errors_keep_their_own_message() {
        let names = classic_seasons();
        assert_eq!(
            outdated_game(APPLY_FAILED, Some(LATEST_GAME_VERSION), &names),
            None
        );
        assert_eq!(outdated_game(APPLY_FAILED, Some("1.0.142.1"), &names), None);
        assert_eq!(outdated_game(APPLY_FAILED, None, &names), None);
        assert_eq!(outdated_game(APPLY_FAILED, Some("unknown"), &names), None);
        for other in [
            "version.dll already exists in the game folder; run swofty restore",
            "building profile default: reading x: missing",
            "downloading the offline play extension: offline play 0.3.11: the download was blocked",
            "applying : no entry",
        ] {
            assert_eq!(
                outdated_game(other, Some("1.0.136.6820"), &names),
                None,
                "{other}"
            );
        }
    }

    #[test]
    fn an_older_game_is_told_to_update_before_play() {
        let d = outdated_exe(Some("1.0.138.5601")).unwrap();
        assert_eq!(d.id, "outdated_exe");
        assert!(d.what.starts_with("Your F1 25 is 1.0.138.5601."), "{}", d.what);
        assert!(
            d.text().contains("Update F1 25 in Steam or the EA app"),
            "{}",
            d.text()
        );
        assert!(d.text().contains("steam://validate/3059520"), "{}", d.text());
        assert_eq!(outdated_exe(Some(LATEST_GAME_VERSION)), None);
        assert_eq!(outdated_exe(Some("1.0.142.1")), None);
        assert_eq!(outdated_exe(Some("unknown")), None);
        assert_eq!(outdated_exe(None), None);
    }

    #[test]
    fn modules_the_offline_hook_lists_are_read() {
        let log = "2026-10-25 19:18:28.000 [1] [offline] module: RTSSHooks64.dll @ 0x7dd50000\n";
        assert_eq!(loaded_modules(log), ["RTSSHooks64.dll"]);
    }
}
