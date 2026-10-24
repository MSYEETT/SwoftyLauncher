//! The `swofty overlay` commands: whether the core keeps the in-game overlay off after the GPU
//! was removed, and turning it back on.

use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use swofty_core::overlay_guard::{self, OverlayOff};
use swofty_core::store::Store;

#[derive(Subcommand)]
pub enum OverlayCommand {
    #[command(about = "Show whether the in-game overlay is off and which addons draw over the game")]
    Status {
        #[arg(long, help = crate::GAME_HELP)]
        game: Option<PathBuf>,
    },
    #[command(about = "Turn the in-game overlay back on: delete the core's GPU-removed record")]
    Reset {
        #[arg(long, help = crate::GAME_HELP)]
        game: Option<PathBuf>,
    },
}

pub fn command(store: &Store, profile: &str, cmd: OverlayCommand) -> Result<()> {
    match cmd {
        OverlayCommand::Status { game } => {
            let dir = crate::build::game_dir(store, game)?;
            match OverlayOff::read(&dir) {
                Some(off) => {
                    println!("overlay: off; {}", off.describe());
                    println!("record: {}", off.path.display());
                    println!("turn it back on with `swofty overlay reset`");
                }
                None => println!("overlay: on (no GPU-removed record in {})", dir.display()),
            }
            let painters = overlay_guard::painters(store, profile);
            if painters.is_empty() {
                println!("draws over the game in {profile}: no addon");
            }
            for (id, name) in painters {
                println!("draws over the game in {profile}: {name} ({id})");
            }
        }
        OverlayCommand::Reset { game } => {
            let dir = crate::build::game_dir(store, game)?;
            if overlay_guard::reset(&dir)? {
                println!(
                    "overlay: on from the next launch; removed {}",
                    overlay_guard::path(&dir).display()
                );
            } else {
                println!("overlay: already on (no GPU-removed record in {})", dir.display());
            }
        }
    }
    Ok(())
}
