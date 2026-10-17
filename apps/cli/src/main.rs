mod build;
mod doctor;
mod offline;
mod run;
mod time;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use swofty_core::manifest::Binds;
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
    #[command(subcommand, about = "List, import and remove maps (EGO track packages)")]
    Maps(MapsCommand),
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

#[derive(Subcommand)]
enum MapsCommand {
    List,
    Import {
        zip: PathBuf,
    },
    Remove {
        code: String,
    },
    #[command(about = "Enable an imported map in the profile; swofty apply installs it")]
    Enable {
        code: String,
    },
    #[command(about = "Disable a map in the profile")]
    Disable {
        code: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("swofty: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(cli: Cli) -> Result<ExitCode> {
    let store = cli.data_dir.map(Store::new).unwrap_or_else(Store::open_default);
    match store.migrate() {
        Ok(notes) => notes.iter().for_each(|n| eprintln!("swofty: {n}")),
        Err(e) => eprintln!("swofty: data migration failed: {e}"),
    }
    let profile = cli.profile.unwrap_or_else(|| store.active_profile());
    match cli.command {
        Command::Run(args) => run::run(&store, &profile, args),
        Command::Build { game } => build::build(&store, &profile, game),
        Command::Apply { game } => build::apply(&store, &profile, game),
        Command::Verify { game } => build::verify(&store, game),
        Command::Restore { game } => build::restore(&store, game),
        Command::Addons(cmd) => addons(&store, &profile, cmd).map(|_| ExitCode::SUCCESS),
        Command::Maps(cmd) => maps(&store, &profile, cmd).map(|_| ExitCode::SUCCESS),
        Command::Offline(cmd) => offline::command(&store, cmd).map(|_| ExitCode::SUCCESS),
        Command::Settings(cmd) => settings(&store, &profile, cmd).map(|_| ExitCode::SUCCESS),
        Command::Logs { dir, exe } => run::logs(&store, dir, exe),
        Command::Doctor => doctor::doctor(&store),
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
    p.set_enabled(id, on)?;
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

fn set_map(store: &Store, profile: &str, code: &str, on: bool) -> Result<()> {
    let mut p = store.profile(profile)?;
    p.set_map(code, on)?;
    store.save_profile(&p)?;
    println!(
        "{} map {code} in profile {profile}",
        if on { "enabled" } else { "disabled" }
    );
    Ok(())
}

fn maps(store: &Store, profile: &str, cmd: MapsCommand) -> Result<()> {
    match cmd {
        MapsCommand::List => {
            let p = store.profile(profile)?;
            for m in store.maps()? {
                let v = m.version.as_deref().unwrap_or("-");
                let by = m.author_name.as_deref().unwrap_or("-");
                let state = if p.maps.contains(&m.track_code) {
                    "enabled"
                } else {
                    "disabled"
                };
                println!(
                    "{:16} {:10} {:28} {state:9} by {by}",
                    m.track_code,
                    v,
                    m.display_name()
                );
            }
        }
        MapsCommand::Enable { code } | MapsCommand::Disable { code }
            if !store.maps()?.iter().any(|m| m.track_code == code) =>
        {
            bail!("{code} is not imported; run swofty maps import first")
        }
        MapsCommand::Enable { code } => set_map(store, profile, &code, true)?,
        MapsCommand::Disable { code } => set_map(store, profile, &code, false)?,
        MapsCommand::Import { zip } => {
            let m = store
                .import_map(&zip)
                .with_context(|| format!("importing {}", zip.display()))?;
            println!(
                "imported {} ({}) into {}",
                m.display_name(),
                m.track_code,
                store.maps_dir().display()
            );
        }
        MapsCommand::Remove { code } => {
            store.remove_map(&code)?;
            println!("removed {code}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

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
