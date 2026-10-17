mod build;
mod doctor;
mod offline;
mod run;
mod time;
mod update;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Arg, ArgAction, ArgMatches, CommandFactory, FromArgMatches, Parser, Subcommand};
use swofty_core::launcher::CommandSpec;
use swofty_core::manifest::{Binds, MANIFEST_FILE};
use swofty_core::registry::{Host, Registry};
use swofty_core::store::Store;

#[derive(Parser)]
#[command(
    name = "swofty",
    version,
    about = "Swofty Launcher for F1 25: addons, mods and maps"
)]
struct Cli {
    #[arg(
        long,
        global = true,
        env = "SWOFTY_DATA_DIR",
        help = "Data directory (default: platform data dir)"
    )]
    data_dir: Option<PathBuf>,
    #[arg(long, global = true, help = "Profile (default: the active profile)")]
    profile: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Launch F1 25 modded and offline, streaming logs; use as the Steam launch option")]
    Run(run::RunArgs),
    #[command(about = "Resolve the profile and build its overlay")]
    Build {
        #[arg(long, help = "Game folder (default: discovered from Steam)")]
        game: Option<PathBuf>,
    },
    #[command(
        about = "Build the profile and install it into the game folder (database, maps); swofty restore undoes it"
    )]
    Apply {
        #[arg(long, help = "Game folder (default: discovered from Steam)")]
        game: Option<PathBuf>,
    },
    #[command(about = "Report drift from the baseline and check the game files parse and round-trip")]
    Verify {
        #[arg(long)]
        game: Option<PathBuf>,
    },
    #[command(
        about = "Put the game folder and prefix back to the baseline: roll back journals, remove additions"
    )]
    Restore {
        #[arg(long)]
        game: Option<PathBuf>,
    },
    #[command(
        subcommand,
        alias = "mods",
        about = "List, install, enable and disable addons"
    )]
    Addons(AddonsCommand),
    #[command(
        subcommand,
        about = "Offline play: consent and the launch extension that runs sessions without EA AntiCheat"
    )]
    Offline(offline::OfflineCommand),
    #[command(subcommand, about = "Show and change addon settings in the profile")]
    Settings(SettingsCommand),
    #[command(about = "Summarise a previous session's logs and crashes")]
    Logs {
        dir: Option<PathBuf>,
        #[arg(long, env = "SWOFTY_EXE_COPY")]
        exe: Option<PathBuf>,
    },
    #[command(about = "Report what was discovered: Steam, the game, Proton, the launch option and the store")]
    Doctor,
    #[command(about = "Check for a new release and install it into this copy")]
    Update {
        #[arg(long, help = "Only report whether a new release is available")]
        check: bool,
    },
}

#[derive(Subcommand)]
enum AddonsCommand {
    List,
    Install { path: PathBuf },
    Enable { id: String },
    Disable { id: String },
}

#[derive(Subcommand)]
enum SettingsCommand {
    #[command(about = "List every addon's settings with the profile's values")]
    List { id: Option<String> },
    #[command(about = "Set an addon setting; the value is JSON (true, 5, \"replica\") or plain text")]
    Set {
        id: String,
        option: String,
        value: String,
    },
    #[command(about = "Put an addon's settings back to their defaults")]
    Reset { id: String },
    #[command(about = "Write the session settings file (swofty/settings.json) for the profile")]
    Export {
        #[arg(long, help = "File to write (default: stdout)")]
        out: Option<PathBuf>,
    },
    #[command(about = "Merge the changes made in game into the profile")]
    Merge,
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let store = data_dir_arg(&args)
        .map(Store::new)
        .unwrap_or_else(Store::open_default)
        .with_builtins(swofty_builtins::all());
    match run_cli(&store, args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("swofty: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn migrate(store: &Store) {
    match store.migrate() {
        Ok(notes) => notes.iter().for_each(|n| eprintln!("swofty: {n}")),
        Err(e) => eprintln!("swofty: data migration failed: {e}"),
    }
}

fn data_dir_arg(args: &[OsString]) -> Option<PathBuf> {
    args.iter().enumerate().find_map(|(i, a)| {
        if a == "--data-dir" {
            return args.get(i + 1).map(PathBuf::from);
        }
        a.to_str()?.strip_prefix("--data-dir=").map(PathBuf::from)
    })
}

struct AddonCommands {
    registry: Registry,
    specs: Vec<(String, CommandSpec)>,
}

impl AddonCommands {
    fn load(store: &Store) -> Self {
        let registry = store.registry().unwrap_or_else(|e| {
            eprintln!("swofty: {e}");
            Registry::default()
        });
        let reserved: Vec<String> = Cli::command()
            .get_subcommands()
            .map(|c| c.get_name().to_owned())
            .chain(["help".to_owned()])
            .collect();
        let reserved: Vec<&str> = reserved.iter().map(String::as_str).collect();
        let (specs, refused) = registry.commands(&reserved);
        refused.iter().for_each(|r| eprintln!("swofty: {r}"));
        Self { registry, specs }
    }

    fn provides(&self, name: &str) -> bool {
        self.specs.iter().any(|(_, s)| s.name == name)
    }

    fn command(&self) -> clap::Command {
        let mut cmd = Cli::command();
        if self.provides("mods") {
            cmd = cmd.mut_subcommand("addons", |c| c.alias(None));
        }
        self.specs
            .iter()
            .fold(cmd, |cmd, (_, spec)| cmd.subcommand(clap_command(spec)))
    }
}

fn clap_command(spec: &CommandSpec) -> clap::Command {
    let mut cmd = clap::Command::new(spec.name.clone()).about(spec.about.clone());
    for a in &spec.args {
        let arg = Arg::new(a.name.clone()).help(a.help.clone()).required(a.required);
        cmd = cmd.arg(if a.many {
            arg.num_args(1..).action(ArgAction::Append)
        } else {
            arg
        });
    }
    if !spec.subcommands.is_empty() && spec.args.is_empty() {
        cmd = cmd.subcommand_required(true);
    }
    spec.subcommands
        .iter()
        .fold(cmd, |cmd, s| cmd.subcommand(clap_command(s)))
}

fn command_input(m: &ArgMatches, spec: &CommandSpec, path: &mut Vec<String>, args: &mut Vec<String>) {
    for a in &spec.args {
        if let Some(values) = m.get_many::<String>(&a.name) {
            args.extend(values.cloned());
        }
    }
    if let Some((name, sub)) = m.subcommand()
        && let Some(s) = spec.subcommands.iter().find(|s| s.name == name)
    {
        path.push(name.to_owned());
        command_input(sub, s, path, args);
    }
}

fn command_at(args: &[OsString]) -> Option<usize> {
    let mut skip = false;
    args.iter().enumerate().skip(1).find_map(|(i, a)| {
        if std::mem::take(&mut skip) {
            return None;
        }
        let a = a.to_str()?;
        skip = a == "--data-dir" || a == "--profile";
        (!a.starts_with('-')).then_some(i)
    })
}

fn forward_maps(args: &mut [OsString]) {
    if let Some(at) = command_at(args).filter(|&at| args[at] == "maps") {
        args[at] = "mods".into();
    }
}

fn forward_mods(store: &Store, args: &[OsString]) -> Option<(Vec<OsString>, String)> {
    let at = command_at(args)?;
    if args[at] != "mods" {
        return None;
    }
    let verb = args.get(at + 1)?.to_str()?;
    let target = args.get(at + 2)?;
    let addon = match verb {
        "install" => Path::new(target).join(MANIFEST_FILE).is_file(),
        "enable" | "disable" => store.addons().ok()?.contains_key(target.to_str()?),
        _ => false,
    };
    if !addon {
        return None;
    }
    let mut forwarded = args.to_vec();
    forwarded[at] = "addons".into();
    Some((
        forwarded,
        format!("swofty mods {verb} is now swofty addons {verb}"),
    ))
}

fn run_cli(store: &Store, mut args: Vec<OsString>) -> Result<ExitCode> {
    let addons = AddonCommands::load(store);
    if addons.provides("mods") {
        forward_maps(&mut args);
    }
    if addons.provides("mods")
        && let Some((forwarded, note)) = forward_mods(store, &args)
    {
        eprintln!("swofty: {note}");
        args = forwarded;
    }
    let matches = addons.command().get_matches_from(args);
    migrate(store);
    if let Some((name, sub)) = matches.subcommand()
        && let Some((id, spec)) = addons.specs.iter().find(|(_, s)| s.name == name)
    {
        let profile = matches
            .get_one::<String>("profile")
            .cloned()
            .unwrap_or_else(|| store.active_profile());
        let (mut path, mut input) = (vec![name.to_owned()], Vec::new());
        command_input(sub, spec, &mut path, &mut input);
        let entry = addons.registry.get(id).context("addon is gone")?;
        let p = store.profile(&profile)?;
        let game = build::game(store, None).ok();
        let host = Host::new(store, id, &p, game.as_ref())?;
        let out = entry.addon.command(&host.cx(), &path, &input)?;
        if !out.is_empty() {
            println!("{}", out.trim_end());
        }
        return Ok(ExitCode::SUCCESS);
    }
    dispatch(store, Cli::from_arg_matches(&matches)?)
}

fn dispatch(store: &Store, cli: Cli) -> Result<ExitCode> {
    let store = store.clone();
    let profile = cli.profile.unwrap_or_else(|| store.active_profile());
    match cli.command {
        Command::Run(args) => run::run(&store, &profile, args),
        Command::Build { game } => build::build(&store, &profile, game),
        Command::Apply { game } => build::apply(&store, &profile, game),
        Command::Verify { game } => build::verify(&store, game),
        Command::Restore { game } => build::restore(&store, game),
        Command::Addons(cmd) => addons(&store, &profile, cmd).map(|_| ExitCode::SUCCESS),
        Command::Offline(cmd) => offline::command(&store, cmd).map(|_| ExitCode::SUCCESS),
        Command::Settings(cmd) => settings(&store, &profile, cmd).map(|_| ExitCode::SUCCESS),
        Command::Logs { dir, exe } => run::logs(&store, dir, exe),
        Command::Doctor => doctor::doctor(&store),
        Command::Update { check } => update::update(&store, check),
    }
}

fn addons(store: &Store, profile: &str, cmd: AddonsCommand) -> Result<()> {
    match cmd {
        AddonsCommand::List => {
            let p = store.profile(profile)?;
            let enabled: Vec<&str> = p.enabled().collect();
            for (id, m) in store.addons()? {
                let state = if m.is_builtin() {
                    "built in"
                } else if enabled.contains(&id.as_str()) {
                    "enabled"
                } else {
                    "disabled"
                };
                println!("{id:32} {:10} {state:9} {}", m.version.to_string(), m.name);
            }
        }
        AddonsCommand::Install { path } => {
            let m = store
                .install_addon(&path)
                .with_context(|| format!("installing {}", path.display()))?;
            println!("installed {} {}", m.id, m.version);
        }
        AddonsCommand::Enable { id } | AddonsCommand::Disable { id }
            if !store.addons()?.contains_key(&id) =>
        {
            bail!("{id} is not installed")
        }
        AddonsCommand::Enable { id } => toggle(store, profile, &id, true)?,
        AddonsCommand::Disable { id } => toggle(store, profile, &id, false)?,
    }
    Ok(())
}

fn toggle(store: &Store, profile: &str, id: &str, on: bool) -> Result<()> {
    let mut p = store.profile(profile)?;
    store.registry()?.set_enabled(&mut p, id, on)?;
    store.save_profile(&p)?;
    println!(
        "{} {id} in profile {profile}",
        if on { "enabled" } else { "disabled" }
    );
    Ok(())
}

fn settings(store: &Store, profile: &str, cmd: SettingsCommand) -> Result<()> {
    let addons = store.addons()?;
    let installed = |id: &str| addons.get(id).with_context(|| format!("{id} is not installed"));
    match cmd {
        SettingsCommand::List { id } => {
            let p = store.profile(profile)?;
            let pending = p.rebuild_pending(&addons);
            for m in addons
                .values()
                .filter(|m| id.as_deref().is_none_or(|i| i == m.id))
            {
                if m.options.is_empty() {
                    continue;
                }
                println!("{} ({})", m.name, m.id);
                for o in &m.options {
                    let v = p.option(m, o);
                    let mark = if pending.contains(&format!("{}.{}", m.id, o.id)) {
                        ", rebuild pending"
                    } else {
                        ""
                    };
                    let binds = match o.binds {
                        Binds::Native => "live",
                        Binds::Db => "rebuild",
                    };
                    println!("  {:24} {:16} {binds}{mark}", o.id, o.text(&v));
                }
            }
        }
        SettingsCommand::Set { id, option, value } => {
            let m = installed(&id)?;
            let value = serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value));
            let mut p = store.profile(profile)?;
            let changed = p.set_option(m, &option, value)?;
            store.save_profile(&p)?;
            let o = m
                .options
                .iter()
                .find(|o| o.id == option)
                .expect("set_option checked it");
            println!(
                "{id}.{option} = {}{}",
                o.text(&p.option(m, o)),
                if !changed {
                    " (unchanged)"
                } else if o.binds == Binds::Db {
                    "; rebuild to apply"
                } else {
                    ""
                }
            );
        }
        SettingsCommand::Reset { id } => {
            let m = installed(&id)?;
            let mut p = store.profile(profile)?;
            p.reset_options(m)?;
            store.save_profile(&p)?;
            println!("{id}: settings back to their defaults in profile {profile}");
        }
        SettingsCommand::Export { out } => {
            let s = store.session_settings(profile)?;
            let text = serde_json::to_string_pretty(&s)? + "\n";
            match out {
                Some(path) => {
                    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                        std::fs::create_dir_all(dir)?;
                    }
                    std::fs::write(&path, text)?;
                    println!("wrote {}", path.display());
                }
                None => print!("{text}"),
            }
        }
        SettingsCommand::Merge => match store.merge_requests(profile, true)? {
            None => println!("no in-game changes for profile {profile}"),
            Some(m) => report_merge(&m),
        },
    }
    Ok(())
}

pub(crate) fn report_merge(m: &swofty_core::settings::Merge) {
    for c in &m.changed {
        println!("in game: {c}");
    }
    for s in &m.skipped {
        println!("in game, skipped: {s}");
    }
    if m.changed.is_empty() && m.skipped.is_empty() {
        println!("in game: no changes");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use swofty_core::launcher::{Addon, ArgSpec, Builtin};
    use swofty_core::manifest::AddonManifest;

    use super::*;

    struct Mods(AddonManifest);

    impl Addon for Mods {
        fn manifest(&self) -> &AddonManifest {
            &self.0
        }

        fn commands(&self) -> Vec<CommandSpec> {
            let spec = |name: &str, args: Vec<ArgSpec>| CommandSpec {
                name: name.into(),
                about: String::new(),
                args,
                subcommands: Vec::new(),
            };
            let paths = ArgSpec {
                name: "path".into(),
                help: String::new(),
                required: true,
                many: true,
            };
            vec![
                CommandSpec {
                    subcommands: vec![spec("list", Vec::new()), spec("import", vec![paths])],
                    ..spec("mods", Vec::new())
                },
                spec("build", Vec::new()),
            ]
        }
    }

    fn store(root: &Path) -> Store {
        let manifest = AddonManifest::parse(
            r#"{"schema_version":1,"id":"t.mods","name":"Mods","version":"1.0.0","author":"t","compat":{"game":"f1_25"}}"#,
        )
        .unwrap();
        Store::new(root).with_builtins(vec![Builtin {
            addon: Arc::new(Mods(manifest)),
            locked: false,
            default_on: true,
        }])
    }

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn addon_commands_take_mods_and_forward_the_old_forms() {
        let root = std::env::temp_dir().join(format!("swofty-cli-commands-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let addon = root.join("my-addon");
        std::fs::create_dir_all(&addon).unwrap();
        std::fs::write(addon.join(MANIFEST_FILE), "{}").unwrap();
        let store = store(&root.join("data"));
        let addons = AddonCommands::load(&store);
        assert_eq!(addons.specs.len(), 1);
        let cmd = addons.command();
        assert_eq!(
            cmd.find_subcommand("addons").unwrap().get_all_aliases().count(),
            0
        );
        let m = cmd
            .try_get_matches_from(["swofty", "--profile", "p", "mods", "import", "a.serps", "b.zip"])
            .unwrap();
        let (name, sub) = m.subcommand().unwrap();
        let (mut path, mut input) = (vec![name.to_owned()], Vec::new());
        command_input(sub, &addons.specs[0].1, &mut path, &mut input);
        assert_eq!(path, ["mods", "import"]);
        assert_eq!(input, ["a.serps", "b.zip"]);
        let dir = addon.to_str().unwrap();
        let (args, note) =
            forward_mods(&store, &os(&["swofty", "--profile", "p", "mods", "install", dir])).unwrap();
        assert_eq!(args, os(&["swofty", "--profile", "p", "addons", "install", dir]));
        assert_eq!(note, "swofty mods install is now swofty addons install");
        assert!(forward_mods(&store, &os(&["swofty", "mods", "enable", "swofty.core"])).is_some());
        assert!(forward_mods(&store, &os(&["swofty", "mods", "enable", "rm.livery"])).is_none());
        assert!(forward_mods(&store, &os(&["swofty", "mods", "install", "pack.serps"])).is_none());
        assert_eq!(
            data_dir_arg(&os(&["swofty", "build", "--data-dir", "/d"])),
            Some("/d".into())
        );
        assert_eq!(
            data_dir_arg(&os(&["swofty", "--data-dir=/e", "build"])),
            Some("/e".into())
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn mods_is_a_hidden_alias_of_addons() {
        for name in ["addons", "mods"] {
            let cli = Cli::try_parse_from(["swofty", name, "enable", "a.b"]).unwrap();
            assert!(matches!(cli.command, Command::Addons(AddonsCommand::Enable { id }) if id == "a.b"));
        }
        let cmd = Cli::command();
        let addons = cmd.find_subcommand("addons").unwrap();
        assert_eq!(addons.get_all_aliases().collect::<Vec<_>>(), ["mods"]);
        assert_eq!(addons.get_visible_aliases().count(), 0);
    }
}
