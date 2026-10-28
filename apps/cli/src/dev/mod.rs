mod check;
pub mod lock;
pub mod pe;

use std::ffi::OsString;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use serde_json::Value;
use swofty_core::manifest::{AddonManifest, MANIFEST_FILE};
use swofty_core::store::Store;

use lock::{LOCK_FILE, Lock};

pub const SDK_ENV: &str = "SWOFTY_SDK_DIR";
pub const RECORDER_ENV: &str = "SWOFTY_RECORDER_DIR";
pub const WINE_ENV: &str = "SWOFTY_WINE";
pub const DEVELOPER_ENV: &str = "SWOFTY_DEVELOPER";

#[derive(Subcommand, Debug, PartialEq)]
pub enum DevCommand {
    #[command(about = "Turn Developer mode on: the swofty dev tools and the launcher's Developer section")]
    On,
    #[command(about = "Turn Developer mode off")]
    Off,
    #[command(about = "Make a new addon from the template")]
    New {
        id: String,
        #[arg(long, help = "Folder to create (default: ./<id>)")]
        dir: Option<PathBuf>,
        #[arg(
            long,
            help = "A standard library 1.6 addon: a SWOFTY_REGISTER, the 1.6 headers and a fake-core unit test"
        )]
        registry: bool,
    },
    #[command(about = "Install an addon by reference: every build copies from the folder")]
    Link { folder: Option<PathBuf> },
    #[command(
        about = "Build the profile, start the game and stream its log lines (swofty run --apply --direct)"
    )]
    Run {
        #[arg(long, help = crate::GAME_HELP)]
        game: Option<PathBuf>,
    },
    #[command(about = "Show the addon's lines from loader.log; --follow keeps reading")]
    Logs {
        #[arg(help = "The addon's folder or id (default: the current folder)")]
        addon: Option<String>,
        #[arg(long, short, help = "Keep reading as the game writes")]
        follow: bool,
        #[arg(long, help = "A session folder to read instead of the game's loader.log")]
        session: Option<PathBuf>,
    },
    #[command(
        about = "Check the manifest, registry.json, the data files and references without starting the game"
    )]
    Check { folder: Option<PathBuf> },
    #[command(about = "Build and run the addon's unit tests (test/*.cpp) against the fake core")]
    Test { folder: Option<PathBuf> },
    #[command(about = "Write registry.lock.json from the addon's SWOFTY_REGISTER and pack the addon")]
    Pack {
        folder: Option<PathBuf>,
        #[arg(long, help = "Where the package goes (default: <folder>/dist)")]
        out: Option<PathBuf>,
        #[arg(long, help = "Write a .zip instead of a .swoftyaddon")]
        zip: bool,
    },
}

pub fn enabled(store: &Store) -> bool {
    store.config().developer || std::env::var(DEVELOPER_ENV).is_ok_and(|v| v == "1")
}

pub fn command(store: &Store, profile: &str, cmd: DevCommand) -> Result<ExitCode> {
    match cmd {
        DevCommand::On | DevCommand::Off => {
            let mut config = store.config();
            config.developer = cmd == DevCommand::On;
            store.save_config(&config)?;
            println!(
                "Developer mode is {}",
                if config.developer { "on" } else { "off" }
            );
            return Ok(ExitCode::SUCCESS);
        }
        _ if !enabled(store) => {
            bail!("Developer mode is off; turn it on in the launcher's Settings or with swofty dev on")
        }
        _ => {}
    }
    let here = || std::env::current_dir().context("no current folder");
    let folder = |f: Option<PathBuf>| -> Result<PathBuf> {
        let f = match f {
            Some(f) => f,
            None => here()?,
        };
        std::path::absolute(&f).with_context(|| format!("{}", f.display()))
    };
    match cmd {
        DevCommand::On | DevCommand::Off => unreachable!(),
        DevCommand::New { id, dir, registry } => {
            let dest = match dir {
                Some(d) => d,
                None => here()?.join(&id),
            };
            new_addon(&sdk_root()?, &id, &dest, registry)?;
            println!("made {} in {}", id, dest.display());
            Ok(ExitCode::SUCCESS)
        }
        DevCommand::Link { folder: f } => {
            let id = link(store, profile, &folder(f)?)?;
            println!("linked {id}: every build copies it from its folder");
            Ok(ExitCode::SUCCESS)
        }
        DevCommand::Run { game } => {
            let args = run_args(game.as_deref());
            let parsed = <RunWrap as clap::Parser>::try_parse_from(args)?;
            crate::run::run(store, profile, parsed.args)
        }
        DevCommand::Logs {
            addon,
            follow,
            session,
        } => logs(store, addon, follow, session),
        DevCommand::Check { folder: f } => {
            let report = check::check(store, &sdk_root().ok(), &folder(f)?)?;
            for line in &report.lines {
                println!("{line}");
            }
            println!(
                "{} error{}, {} warning{}",
                report.errors,
                if report.errors == 1 { "" } else { "s" },
                report.warnings,
                if report.warnings == 1 { "" } else { "s" }
            );
            Ok(if report.errors > 0 {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        DevCommand::Test { folder: f } => test(&sdk_root()?, &folder(f)?, store),
        DevCommand::Pack { folder: f, out, zip } => {
            let folder = folder(f)?;
            let out = out.unwrap_or_else(|| folder.join("dist"));
            let sdk = sdk_root()?;
            match write_lock(store, &sdk, &folder)? {
                Some(lock) => println!(
                    "wrote {LOCK_FILE}: {} record{}, {} provider{}, {} need{}",
                    lock.provides.values().map(Vec::len).sum::<usize>(),
                    plural(lock.provides.values().map(Vec::len).sum::<usize>()),
                    lock.providers.len(),
                    plural(lock.providers.len()),
                    lock.needs.len(),
                    plural(lock.needs.len())
                ),
                None => println!("no SWOFTY_REGISTER in the addon's DLLs: no {LOCK_FILE}"),
            }
            let pkg = pack(&sdk, &folder, &out, zip)?;
            println!("{}", pkg.trim_end());
            Ok(ExitCode::SUCCESS)
        }
    }
}

pub fn data_root(store: &Store) -> PathBuf {
    store
        .config_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

#[derive(clap::Parser)]
struct RunWrap {
    #[command(flatten)]
    args: crate::run::RunArgs,
}

pub fn run_args(game: Option<&Path>) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["run".into(), "--apply".into(), "--direct".into()];
    if let Some(g) = game {
        args.push("--game".into());
        args.push(g.into());
    }
    args
}

pub fn sdk_root() -> Result<PathBuf> {
    let bin = std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.to_path_buf()));
    [
        std::env::var_os(SDK_ENV).map(PathBuf::from),
        bin.as_ref().and_then(|b| Some(b.parent()?.join("sdk"))),
        bin.map(|b| b.join("sdk")),
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")),
    ]
    .into_iter()
    .flatten()
    .find(|d| is_sdk(d))
    .context("the SDK folder isn't there (tools/, templates/ and packages/sdk/); set SWOFTY_SDK_DIR to it")
}

fn is_sdk(dir: &Path) -> bool {
    dir.join("tools/pack_addon.py").is_file() && dir.join("packages/sdk/include/swofty_api.h").is_file()
}

pub fn manifest_json(folder: &Path) -> Result<Value> {
    let path = folder.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&path).with_context(|| format!("{}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("{}", path.display()))
}

pub fn native_dlls(manifest: &Value) -> Vec<String> {
    manifest["operations"]["native"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| n["dll"].as_str().map(str::to_owned))
        .collect()
}

pub fn register_dlls(folder: &Path, manifest: &Value) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for rel in native_dlls(manifest) {
        let path = folder.join(&rel);
        let image =
            std::fs::read(&path).with_context(|| format!("{rel} is missing; build the addon first"))?;
        if pe::has_register(&image) {
            out.push(path);
        }
    }
    Ok(out)
}

pub fn write_lock(store: &Store, sdk: &Path, folder: &Path) -> Result<Option<Lock>> {
    let manifest = manifest_json(folder)?;
    let dlls = register_dlls(folder, &manifest)?;
    let lock_path = folder.join(LOCK_FILE);
    if dlls.is_empty() {
        if lock_path.is_file() {
            std::fs::remove_file(&lock_path)?;
            println!("removed the old {LOCK_FILE}: no DLL has a SWOFTY_REGISTER now");
        }
        return Ok(None);
    }
    let recorder = recorder_dir(sdk)?;
    let scratch = data_root(store).join("cache/recorder");
    std::fs::create_dir_all(&scratch)?;
    let mut recordings = Vec::new();
    for dll in &dlls {
        recordings.push(record(&recorder, &scratch, dll, &folder.join(MANIFEST_FILE))?);
    }
    let lock = Lock::from_recordings(&recordings, &lock::generated_by());
    std::fs::write(&lock_path, lock.to_json()).with_context(|| format!("{}", lock_path.display()))?;
    Ok(Some(lock))
}

pub fn pack(sdk: &Path, folder: &Path, out: &Path, zip: bool) -> Result<String> {
    let mut cmd = Command::new(python()?);
    cmd.arg(sdk.join("tools/pack_addon.py"));
    if zip {
        cmd.arg("--zip");
    }
    let o = cmd
        .arg(folder)
        .arg(out)
        .output()
        .context("python3 is needed to pack an addon")?;
    if !o.status.success() {
        bail!(
            "pack_addon.py failed: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

pub fn python() -> Result<PathBuf> {
    ["python3", "python"]
        .iter()
        .find_map(|p| which(p))
        .context("python3 isn't installed; dev pack and dev check need it")
}

pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|d| {
        let exts: &[&str] = if cfg!(windows) {
            &[".exe", ".cmd", ""]
        } else {
            &[""]
        };
        exts.iter()
            .map(|e| d.join(format!("{name}{e}")))
            .find(|p| p.is_file())
    })
}

pub fn recorder_dir(sdk: &Path) -> Result<PathBuf> {
    let ready = |d: &Path| d.join("swofty_record.exe").is_file() && d.join("swofty_core.dll").is_file();
    if let Some(d) = std::env::var_os(RECORDER_ENV).map(PathBuf::from)
        && ready(&d)
    {
        return Ok(d);
    }
    let dist = sdk.join("packages/sdk/recorder/dist");
    if !ready(&dist) && cfg!(unix) && sdk.join("packages/sdk/recorder/build.sh").is_file() {
        let status = Command::new("bash")
            .arg(sdk.join("packages/sdk/recorder/build.sh"))
            .stdout(Stdio::null())
            .status();
        if !status.is_ok_and(|s| s.success()) {
            bail!("building the recorder failed; run packages/sdk/recorder/build.sh (it needs zig)");
        }
    }
    if ready(&dist) {
        return Ok(dist);
    }
    bail!(
        "the recorder (swofty_record.exe and swofty_core.dll) isn't in {}; set {RECORDER_ENV}",
        dist.display()
    )
}

pub fn wine_path(p: &Path) -> String {
    if cfg!(windows) {
        return p.display().to_string();
    }
    format!("Z:{}", p.display().to_string().replace('/', "\\"))
}

pub fn recorder_command(
    recorder: &Path,
    prefix: &Path,
    dll: &Path,
    manifest: &Path,
    out: &Path,
) -> Result<Command> {
    let exe = recorder.join("swofty_record.exe");
    let args = [
        wine_path(dll),
        wine_path(out),
        "--manifest".into(),
        wine_path(manifest),
    ];
    if cfg!(windows) {
        let mut c = Command::new(exe);
        c.args(args);
        return Ok(c);
    }
    let wine = std::env::var_os(WINE_ENV)
        .map(PathBuf::from)
        .or_else(|| which("wine"))
        .or_else(|| which("wine64"))
        .context("the recorder runs under Wine on Linux: install wine or set SWOFTY_WINE")?;
    let mut c = Command::new(wine);
    c.arg(exe).args(args).env("WINEDEBUG", "-all").env(
        "WINEDLLOVERRIDES",
        "mscoree,mshtml=;winemenubuilder.exe,winedbg.exe=d",
    );
    if std::env::var_os("WINEPREFIX").is_none() {
        c.env("WINEPREFIX", prefix);
    }
    Ok(c)
}

pub fn record(recorder: &Path, scratch: &Path, dll: &Path, manifest: &Path) -> Result<Value> {
    let name = dll.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let out = scratch.join(format!("{name}.recording.json"));
    std::fs::remove_file(&out).ok();
    let mut child = recorder_command(recorder, &scratch.join("prefix"), dll, manifest, &out)?
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("the recorder didn't start")?;
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if start.elapsed() > Duration::from_secs(180) {
            child.kill().ok();
            bail!("{name}: the recorder didn't finish in 180 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        e.read_to_string(&mut err).ok();
    }
    let text = std::fs::read_to_string(&out).with_context(|| {
        format!(
            "{name}: the recorder wrote no recording (exit {}): {}",
            status.code().unwrap_or(-1),
            err.trim()
        )
    })?;
    let rec: Value = serde_json::from_str(&text).with_context(|| format!("{}", out.display()))?;
    match rec["status"].as_str() {
        Some("ok") | Some("no_register") => Ok(rec),
        other => bail!(
            "{name}: {}: {}",
            describe_status(other.unwrap_or("unknown")),
            rec["error"]
        ),
    }
}

pub fn describe_status(status: &str) -> &'static str {
    match status {
        "crashed" => "SWOFTY_REGISTER crashed",
        "exception" => "SWOFTY_REGISTER threw a C++ exception",
        "timeout" => "SWOFTY_REGISTER didn't return",
        "unexpected" => "SWOFTY_REGISTER called outside the registry, hooks and services",
        "load_failed" => "the DLL didn't load",
        _ => "the recorder failed",
    }
}

fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src).with_context(|| format!("{}", src.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        let n = name.to_string_lossy();
        if n == "build" || n.ends_with(".dll") || n.starts_with('.') {
            continue;
        }
        let (from, to) = (entry.path(), dest.join(&name));
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

pub fn short_name(id: &str) -> String {
    let last = id.rsplit('.').next().unwrap_or(id);
    let s: String = last
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() { "addon".into() } else { s }
}

fn title(id: &str) -> String {
    id.rsplit('.')
        .next()
        .unwrap_or(id)
        .split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

const REGISTRY_SOURCE: &str = r#"#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/sounds.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto sound = sounds::engine_sounds.add("screamer", {
        .bundle = file("sounds/screamer.bdl"),
        .settings_from = sounds::engine_sounds["game/ferrari_2025"],
    });
    cars::vehicles.layer(cars::vehicles["game/ferrari_2025"], [=](cars::Vehicle &v) { v.engine_sound = sound; });
}

SWOFTY_ADDON()
"#;

const REGISTRY_TEST: &str = r#"#include <cstdio>
#include <string>
#include <vector>

#include <swofty/fake_core.hpp>

#include "../native/src/@NAME@.cpp"

static int failures = 0;

static void check(bool ok, const char *what)
{
    std::printf("%s  %s\n", ok ? "ok  " : "FAIL", what);
    failures += !ok;
}

int main()
{
    swofty::fake::Core core{"@ID@"};
    core.run_register();
    check(core.ids("engine_sound") == std::vector<std::string>{"@ID@/screamer"}, "adds the screamer sound");
    auto layers = core.layers("vehicle", "game/ferrari_2025");
    check(layers.size() == 1, "one layer on the 2025 Ferrari");
    if (layers.size() == 1) {
        auto v = core.apply(*layers[0], swofty::json::Value::object());
        check(v && (*v)["engine_sound"].as_string() == "@ID@/screamer", "the layer puts the screamer on it");
    }
    if (core.rules()) {
        auto car = core.effective("vehicle", "game/ferrari_2025");
        check(car && (*car)["engine_sound"].as_string() == "@ID@/screamer", "the effective Ferrari has it");
        check(core.origin("vehicle", "game/ferrari_2025", "/engine_sound") == "@ID@", "and the origin says so");
    }
    return failures ? 1 : 0;
}
"#;

pub fn new_addon(sdk: &Path, id: &str, dest: &Path, registry: bool) -> Result<()> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) {
        bail!("{id:?} isn't an addon id: use letters, digits, dots, dashes and underscores (author.name)");
    }
    if dest.exists() && std::fs::read_dir(dest)?.next().is_some() {
        bail!("{} already exists and isn't empty", dest.display());
    }
    copy_dir(&sdk.join("templates/addon"), dest)?;
    let path = dest.join(MANIFEST_FILE);
    let mut m: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    m["id"] = id.into();
    m["name"] = title(id).into();
    if registry {
        let name = short_name(id);
        copy_dir(&sdk.join("packages/sdk/include"), &dest.join("native/include"))?;
        let src = dest.join("native/src");
        std::fs::remove_file(src.join("sample_plugin.cpp")).ok();
        std::fs::write(src.join(format!("{name}.cpp")), REGISTRY_SOURCE)?;
        let build = dest.join("native/build.sh");
        let script = std::fs::read_to_string(&build)?
            .replace("sample_plugin", &name)
            .replace(
                "-O2 -Wall -Wextra",
                "-O2 -Wall -Wextra -Wno-nullability-completeness",
            );
        std::fs::write(&build, script)?;
        std::fs::create_dir_all(dest.join("test"))?;
        std::fs::write(
            dest.join("test/register_test.cpp"),
            REGISTRY_TEST.replace("@NAME@", &name).replace("@ID@", id),
        )?;
        for d in ["db", "ui"] {
            std::fs::remove_dir_all(dest.join(d)).ok();
        }
        m["description"] =
            "An engine sound under the addon's own key, put on the 2025 Ferrari by a layer.".into();
        m["kind"] = serde_json::json!(["native"]);
        m["compat"]["launcher"] = ">=1.6.0".into();
        m["options"] = serde_json::json!([]);
        m["operations"] = serde_json::json!({ "native": [ { "dll": format!("native/{name}.dll") } ] });
    }
    std::fs::write(&path, serde_json::to_string_pretty(&m)? + "\n")?;
    Ok(())
}

pub fn link(store: &Store, profile: &str, folder: &Path) -> Result<String> {
    let m = AddonManifest::load(folder).map_err(anyhow::Error::msg)?;
    let target = store.addons_dir().join(&m.id);
    if let Ok(meta) = std::fs::symlink_metadata(&target) {
        let linked = meta.file_type().is_symlink() || std::fs::read_link(&target).is_ok();
        if !linked {
            bail!(
                "{} is installed as a copy; remove it first with swofty addons remove {}",
                m.id,
                m.id
            );
        }
        remove_link(&target)?;
    }
    std::fs::create_dir_all(store.addons_dir())?;
    make_link(folder, &target)?;
    let mut p = store.profile(profile)?;
    store.registry()?.set_enabled(&mut p, &m.id, true)?;
    store.save_profile(&p)?;
    Ok(m.id)
}

#[cfg(unix)]
fn make_link(folder: &Path, target: &Path) -> Result<()> {
    std::os::unix::fs::symlink(folder, target).with_context(|| format!("linking {}", target.display()))
}

#[cfg(windows)]
fn make_link(folder: &Path, target: &Path) -> Result<()> {
    let ok = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(target)
        .arg(folder)
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        bail!("couldn't make a junction at {}", target.display());
    }
    Ok(())
}

#[cfg(unix)]
fn remove_link(target: &Path) -> Result<()> {
    std::fs::remove_file(target).with_context(|| format!("{}", target.display()))
}

#[cfg(windows)]
fn remove_link(target: &Path) -> Result<()> {
    std::fs::remove_dir(target).with_context(|| format!("{}", target.display()))
}

pub fn needles(id: &str, manifest: &Value) -> Vec<String> {
    let mut out = vec![id.to_owned()];
    for dll in native_dlls(manifest) {
        if let Some(name) = Path::new(&dll).file_name() {
            out.push(name.to_string_lossy().into_owned());
        }
    }
    out
}

pub fn matches(line: &str, needles: &[String]) -> bool {
    needles.iter().any(|n| {
        line.match_indices(n.as_str()).any(|(at, _)| {
            let before = line[..at].chars().next_back();
            let after = line[at + n.len()..].chars().next();
            let edge = |c: Option<char>| {
                c.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_'))
            };
            edge(before) && (edge(after) || after == Some('/'))
        })
    })
}

fn logs(store: &Store, addon: Option<String>, follow: bool, session: Option<PathBuf>) -> Result<ExitCode> {
    let (id, manifest) = match addon {
        Some(a) if !Path::new(&a).join(MANIFEST_FILE).is_file() => {
            let dir = store.addons_dir().join(&a);
            let m = manifest_json(&dir).unwrap_or(Value::Null);
            (a, m)
        }
        a => {
            let dir = a.map(PathBuf::from).unwrap_or(std::env::current_dir()?);
            let m = manifest_json(&dir)?;
            let id = m["id"].as_str().context("swofty.json has no id")?.to_owned();
            (id, m)
        }
    };
    let needles = needles(&id, &manifest);
    let file = match session {
        Some(dir) => dir.join("loader.log"),
        None => {
            let live = crate::build::game(store, None)
                .ok()
                .map(|g| g.dir.join("swofty/loader.log"))
                .filter(|p| p.is_file());
            match live {
                Some(p) => p,
                None => store
                    .latest_session()
                    .context("no loader.log yet: start the game with swofty dev run")?
                    .join("loader.log"),
            }
        }
    };
    let mut at = 0u64;
    let mut pending = String::new();
    loop {
        if let Ok(mut f) = std::fs::File::open(&file) {
            let len = f.metadata().map(|m| m.len()).unwrap_or(0);
            if len < at {
                at = 0;
            }
            f.seek(SeekFrom::Start(at))?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            at += buf.len() as u64;
            pending.push_str(&String::from_utf8_lossy(&buf));
            while let Some(nl) = pending.find('\n') {
                let line: String = pending.drain(..=nl).collect();
                let line = line.trim_end();
                if matches(line, &needles) {
                    println!("{line}");
                }
            }
        } else if !follow {
            bail!("{} isn't there", file.display());
        }
        if !follow {
            return Ok(ExitCode::SUCCESS);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

pub fn compiler() -> Result<(PathBuf, Vec<String>)> {
    if let Some(cxx) = std::env::var_os("CXX") {
        return Ok((PathBuf::from(cxx), Vec::new()));
    }
    if let Some(zig) = which("zig") {
        let mut args = vec!["c++".to_owned()];
        if cfg!(windows) {
            args.extend(["-target".into(), "x86_64-windows-gnu".into()]);
        }
        return Ok((zig, args));
    }
    ["clang++", "g++", "c++"]
        .iter()
        .find_map(|c| which(c))
        .map(|c| (c, Vec::new()))
        .context("no C++ compiler: install zig (or set CXX)")
}

pub fn test(sdk: &Path, folder: &Path, store: &Store) -> Result<ExitCode> {
    let dir = folder.join("test");
    let mut tests: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "cpp"))
                .collect()
        })
        .unwrap_or_default();
    tests.sort();
    if tests.is_empty() {
        println!("no tests: put unit tests in {}/*.cpp", dir.display());
        return Ok(ExitCode::SUCCESS);
    }
    let (cxx, pre) = compiler()?;
    let out = folder.join("build/test");
    std::fs::create_dir_all(&out)?;
    if cfg!(windows)
        && let Ok(rec) = recorder_dir(sdk)
    {
        std::fs::copy(rec.join("swofty_core.dll"), out.join("swofty_core.dll")).ok();
    }
    let mut failed = 0;
    for t in &tests {
        let name = t.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let exe = out.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.clone()
        });
        let mut cmd = Command::new(&cxx);
        cmd.args(&pre)
            .args(test_flags())
            .arg(format!("-I{}", folder.join("native/include").display()))
            .arg(format!("-I{}", sdk.join("packages/sdk/include").display()))
            .arg(format!("-I{}", folder.join("native/src").display()))
            .arg("-o")
            .arg(&exe)
            .arg(t);
        if which("zig").is_some() && std::env::var_os("ZIG_GLOBAL_CACHE_DIR").is_none() {
            let cache = data_root(store).join("cache/zig");
            cmd.env("ZIG_GLOBAL_CACHE_DIR", &cache)
                .env("ZIG_LOCAL_CACHE_DIR", &cache);
        }
        let built = cmd.output().context("the C++ compiler didn't start")?;
        if !built.status.success() {
            println!("FAIL  {name}: it doesn't build");
            print!("{}", String::from_utf8_lossy(&built.stderr));
            failed += 1;
            continue;
        }
        let ran = Command::new(&exe).current_dir(folder).output()?;
        print!("{}", String::from_utf8_lossy(&ran.stdout));
        if ran.status.success() {
            println!("ok    {name}");
        } else {
            print!("{}", String::from_utf8_lossy(&ran.stderr));
            println!("FAIL  {name}");
            failed += 1;
        }
    }
    println!("{} test{}, {failed} failed", tests.len(), plural(tests.len()));
    Ok(if failed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

pub fn test_flags() -> Vec<&'static str> {
    vec![
        "-std=c++23",
        "-O1",
        "-fdeclspec",
        "-Wno-ignored-attributes",
        "-Wno-nullability-completeness",
        "-Wno-unknown-warning-option",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        dev: DevCommand,
    }

    fn parse(args: &[&str]) -> DevCommand {
        Cli::try_parse_from(std::iter::once("dev").chain(args.iter().copied()))
            .unwrap()
            .dev
    }

    #[test]
    fn parses_every_form() {
        assert_eq!(
            parse(&["new", "tails.v10", "--registry"]),
            DevCommand::New {
                id: "tails.v10".into(),
                dir: None,
                registry: true
            }
        );
        assert_eq!(
            parse(&["link", "addons/x"]),
            DevCommand::Link {
                folder: Some("addons/x".into())
            }
        );
        assert_eq!(parse(&["run"]), DevCommand::Run { game: None });
        assert_eq!(
            parse(&["logs", "mod.esp", "-f"]),
            DevCommand::Logs {
                addon: Some("mod.esp".into()),
                follow: true,
                session: None
            }
        );
        assert_eq!(parse(&["check"]), DevCommand::Check { folder: None });
        assert_eq!(
            parse(&["test", "."]),
            DevCommand::Test {
                folder: Some(".".into())
            }
        );
        assert_eq!(
            parse(&["pack", "--zip", "--out", "o"]),
            DevCommand::Pack {
                folder: None,
                out: Some("o".into()),
                zip: true
            }
        );
        assert_eq!(parse(&["on"]), DevCommand::On);
    }

    #[test]
    fn run_delegates_to_run_apply_direct() {
        let args = run_args(Some(Path::new("/games/F1 25")));
        assert_eq!(args, ["run", "--apply", "--direct", "--game", "/games/F1 25"]);
        assert!(<RunWrap as Parser>::try_parse_from(&args).is_ok());
        assert!(<RunWrap as Parser>::try_parse_from(run_args(None)).is_ok());
    }

    #[test]
    fn log_lines_match_the_addon_and_its_dlls_only() {
        let m = serde_json::json!({ "operations": { "native": [ { "dll": "native/esp.dll" } ] } });
        let n = needles("mod.esp", &m);
        assert!(matches(
            "[registry] register: mod.esp (esp.dll) 1 engine_sound",
            &n
        ));
        assert!(matches("[registry] engine_sound mod.esp/v10: added", &n));
        assert!(matches("[loader] loaded esp.dll", &n));
        assert!(!matches("[registry] register: mod.esp2 (esp2.dll)", &n));
        assert!(!matches("[loader] loaded myesp.dll", &n));
        assert!(!matches("[registry] register: mod.espx (x.dll)", &n));
    }

    #[test]
    fn names_and_wine_paths() {
        assert_eq!(short_name("tails.whatif-history"), "whatif_history");
        assert_eq!(title("tails.whatif-history"), "Whatif History");
        if cfg!(unix) {
            assert_eq!(wine_path(Path::new("/a/b c/x.dll")), "Z:\\a\\b c\\x.dll");
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("swofty-dev-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        d
    }

    #[test]
    fn new_makes_a_registry_addon_from_the_template() {
        let sdk = sdk_root().unwrap();
        let dest = scratch("new");
        new_addon(&sdk, "tails.v10-sound", &dest, true).unwrap();
        let m = manifest_json(&dest).unwrap();
        assert_eq!(m["id"], "tails.v10-sound");
        assert_eq!(m["name"], "V10 Sound");
        assert_eq!(native_dlls(&m), ["native/v10_sound.dll"]);
        assert_eq!(m["compat"]["launcher"], ">=1.6.0");
        assert!(dest.join("native/src/v10_sound.cpp").is_file());
        assert!(dest.join("native/include/swofty/fake_core.hpp").is_file());
        assert!(dest.join("test/register_test.cpp").is_file());
        assert!(!dest.join("native/src/sample_plugin.cpp").exists());
        assert!(
            std::fs::read_to_string(dest.join("native/build.sh"))
                .unwrap()
                .contains("v10_sound.dll")
        );
        assert!(AddonManifest::load(&dest).is_ok());
        assert!(new_addon(&sdk, "tails.v10-sound", &dest, true).is_err());
        assert!(new_addon(&sdk, "bad id", &scratch("bad"), false).is_err());
        let plain = scratch("plain");
        new_addon(&sdk, "a.b", &plain, false).unwrap();
        assert!(plain.join("native/src/sample_plugin.cpp").is_file());
        assert_eq!(manifest_json(&plain).unwrap()["id"], "a.b");
        if which("zig").is_some() {
            let store = Store::new(scratch("new-store"));
            assert_eq!(test(&sdk, &dest, &store).unwrap(), ExitCode::SUCCESS);
        } else {
            eprintln!("skipped dev test of the new addon: zig isn't installed");
        }
        for d in [dest, plain] {
            std::fs::remove_dir_all(d).ok();
        }
    }

    #[test]
    fn link_points_the_installed_addon_at_the_folder() {
        let root = scratch("link");
        let store = Store::new(root.join("data"));
        let folder = root.join("work/mod.linked");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join(MANIFEST_FILE),
            r#"{"schema_version":1,"id":"mod.linked","name":"Linked","version":"0.1.0","author":"t","kind":["native"],"compat":{"game":"f1_25"}}"#,
        )
        .unwrap();
        let profile = store.active_profile();
        assert_eq!(link(&store, &profile, &folder).unwrap(), "mod.linked");
        assert!(store.addons().unwrap().contains_key("mod.linked"));
        assert!(
            store
                .profile(&profile)
                .unwrap()
                .enabled()
                .any(|id| id == "mod.linked")
        );
        std::fs::write(folder.join("README.md"), "changed").unwrap();
        assert!(store.addons_dir().join("mod.linked/README.md").is_file());
        assert_eq!(link(&store, &profile, &folder).unwrap(), "mod.linked");
        std::fs::remove_dir_all(&root).ok();
    }

    fn plain_addon(dir: &Path) {
        std::fs::create_dir_all(dir.join("db")).unwrap();
        std::fs::write(
            dir.join(MANIFEST_FILE),
            r#"{"schema_version":1,"id":"mod.plain","name":"Plain","version":"0.1.0","operations":{"database":["db/a.json"]}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("db/a.json"), r#"{"ops":[]}"#).unwrap();
        std::fs::write(dir.join("README.md"), "readme").unwrap();
        std::fs::write(dir.join("notes.txt"), "not packed").unwrap();
    }

    #[test]
    fn packs_an_addon_without_registry_code_like_pack_addon_py() {
        let Ok(py) = python() else {
            eprintln!("skipped: python3 isn't installed");
            return;
        };
        let sdk = sdk_root().unwrap();
        let root = scratch("pack");
        let folder = root.join("mod.plain");
        plain_addon(&folder);
        std::fs::write(folder.join(LOCK_FILE), "{}").unwrap();
        let store = Store::new(root.join("data"));
        assert!(write_lock(&store, &sdk, &folder).unwrap().is_none());
        assert!(!folder.join(LOCK_FILE).exists());
        pack(&sdk, &folder, &root.join("ours"), false).unwrap();
        let direct = Command::new(py)
            .arg(sdk.join("tools/pack_addon.py"))
            .arg(&folder)
            .arg(root.join("theirs"))
            .status()
            .unwrap();
        assert!(direct.success());
        let ours = std::fs::read(root.join("ours/mod.plain.swoftyaddon")).unwrap();
        let theirs = std::fs::read(root.join("theirs/mod.plain.swoftyaddon")).unwrap();
        assert_eq!(ours, theirs);
        assert!(!String::from_utf8_lossy(&ours).contains("notes.txt"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn pack_records_a_registry_dll_when_wine_is_there() {
        let sdk = sdk_root().unwrap();
        let built = sdk.join("native/build/recorder-test");
        let has_wine = cfg!(windows) || which("wine").is_some() || std::env::var_os(WINE_ENV).is_some();
        if !built.join("t_register.dll").is_file() || !has_wine || python().is_err() {
            eprintln!(
                "skipped: needs Wine, python3 and the DLLs packages/sdk/recorder/test/run_recorder_test.sh builds"
            );
            return;
        }
        let root = scratch("pack-reg");
        let folder = root.join("t.recorder");
        std::fs::create_dir_all(folder.join("native")).unwrap();
        std::fs::copy(built.join("t_register.dll"), folder.join("native/t_register.dll")).unwrap();
        std::fs::write(
            folder.join(MANIFEST_FILE),
            r#"{"schema_version":1,"id":"t.recorder","name":"T","version":"1.0.0","operations":{"native":[{"dll":"native/t_register.dll"}]}}"#,
        )
        .unwrap();
        let store = Store::new(root.join("data"));
        let lock = write_lock(&store, &sdk, &folder).unwrap().unwrap();
        assert_eq!(
            lock.provides["engine_sound"],
            [lock::Provided::Id("t.recorder/v10".into())]
        );
        let lock_text = std::fs::read_to_string(folder.join(LOCK_FILE)).unwrap();
        assert!(lock_text.contains("livery:mod.liveries/red"));
        pack(&sdk, &folder, &root.join("out"), false).unwrap();
        let pkg = std::fs::read(root.join("out/t.recorder.swoftyaddon")).unwrap();
        assert!(String::from_utf8_lossy(&pkg).contains(LOCK_FILE));
        std::fs::remove_dir_all(&root).ok();
    }
}
