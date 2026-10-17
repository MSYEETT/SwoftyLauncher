mod build;
mod doctor;
mod run;
mod time;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use swofty_core::store::Store;

#[derive(Parser)]
#[command(name = "swofty", version, about = "F1 Mod Manager: mods and maps for F1 25")]
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
    #[command(subcommand, about = "List, install, enable and disable mods")]
    Mods(ModsCommand),
    #[command(subcommand, about = "List, import and remove maps (EGO track packages)")]
    Maps(MapsCommand),
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
enum ModsCommand {
    List,
    Install { path: PathBuf },
    Enable { id: String },
    Disable { id: String },
}

#[derive(Subcommand)]
enum MapsCommand {
    List,
    Import { zip: PathBuf },
    Remove { code: String },
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
    let profile = cli.profile.unwrap_or_else(|| store.active_profile());
    match cli.command {
        Command::Run(args) => run::run(&store, &profile, args),
        Command::Build { game } => build::build(&store, &profile, game),
        Command::Verify { game } => build::verify(&store, game),
        Command::Restore { game } => build::restore(&store, game),
        Command::Mods(cmd) => mods(&store, &profile, cmd).map(|_| ExitCode::SUCCESS),
        Command::Maps(cmd) => maps(&store, cmd).map(|_| ExitCode::SUCCESS),
        Command::Logs { dir, exe } => run::logs(&store, dir, exe),
        Command::Doctor => doctor::doctor(&store),
    }
}

fn mods(store: &Store, profile: &str, cmd: ModsCommand) -> Result<()> {
    match cmd {
        ModsCommand::List => {
            let p = store.profile(profile)?;
            let enabled: Vec<&str> = p.enabled().collect();
            for (id, m) in store.mods()? {
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
        ModsCommand::Install { path } => {
            let m = store
                .install_mod(&path)
                .with_context(|| format!("installing {}", path.display()))?;
            println!("installed {} {}", m.id, m.version);
        }
        ModsCommand::Enable { id } | ModsCommand::Disable { id } if !store.mods()?.contains_key(&id) => {
            bail!("{id} is not installed")
        }
        ModsCommand::Enable { id } => toggle(store, profile, &id, true)?,
        ModsCommand::Disable { id } => toggle(store, profile, &id, false)?,
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

fn maps(store: &Store, cmd: MapsCommand) -> Result<()> {
    match cmd {
        MapsCommand::List => {
            for m in store.maps()? {
                let v = m.version.as_deref().unwrap_or("-");
                let by = m.author_name.as_deref().unwrap_or("-");
                println!("{:16} {:10} {:28} by {by}", m.track_code, v, m.display_name());
            }
        }
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
