use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use swofty_core::catalog;
use swofty_core::multiplayer::fingerprint::{self, Fingerprint};
use swofty_core::store::Store;

#[derive(Subcommand)]
pub enum MpCommand {
    #[command(
        about = "Print the profile's multiplayer fingerprint as JSON: the game build, its gameplay addons and their settings"
    )]
    Fingerprint {
        #[arg(long, help = crate::GAME_HELP)]
        game: Option<PathBuf>,
    },
    #[command(
        about = "Compare two fingerprints and list what the first must change to match the second, the host's; exits 1 when they differ"
    )]
    Compare {
        #[arg(help = "This player's fingerprint")]
        local: PathBuf,
        #[arg(help = "The host's fingerprint")]
        host: PathBuf,
        #[arg(long, help = "Print the differences as JSON")]
        json: bool,
    },
}

pub fn command(store: &Store, profile: &str, cmd: MpCommand) -> Result<ExitCode> {
    match cmd {
        MpCommand::Fingerprint { game } => {
            let g = crate::build::game(store, game)?;
            let p = store.profile(profile)?;
            let f = fingerprint::fingerprint(store, &p, &g.identity)?;
            println!("{}", serde_json::to_string_pretty(&f)?);
            Ok(ExitCode::SUCCESS)
        }
        MpCommand::Compare { local, host, json } => {
            let (local, host) = (read(&local)?, read(&host)?);
            let listing = catalog::cached(&store.root).unwrap_or_default();
            let found = fingerprint::compare(&local, &host, &listing);
            if json {
                println!("{}", serde_json::to_string_pretty(&found)?);
            } else if found.is_empty() {
                println!("the fingerprints match");
            } else {
                found.iter().for_each(|m| println!("{}", m.line()));
            }
            Ok(if found.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
    }
}

fn read(path: &Path) -> Result<Fingerprint> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let f: Fingerprint = serde_json::from_str(&text).with_context(|| format!("{}", path.display()))?;
    if f.version != fingerprint::VERSION {
        bail!(
            "{}: fingerprint version {} is not {}",
            path.display(),
            f.version,
            fingerprint::VERSION
        );
    }
    Ok(f)
}
