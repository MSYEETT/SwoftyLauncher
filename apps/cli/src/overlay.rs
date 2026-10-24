//! The `swofty overlay` commands: whether the core keeps the in-game overlay off after the GPU
//! was removed, and turning it back on.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
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
    #[command(about = "Let an addon draw over the game again in the profile")]
    Enable { id: String },
    #[command(about = "Stop an addon drawing over the game in the profile; the addon stays on")]
    Disable { id: String },
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
            for (id, name) in overlay_guard::switched_off(store, profile) {
                println!("drawing turned off in {profile}: {name} ({id})");
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
        OverlayCommand::Enable { id } => switch(store, profile, &id, true)?,
        OverlayCommand::Disable { id } => switch(store, profile, &id, false)?,
    }
    Ok(())
}

fn switch(store: &Store, profile: &str, id: &str, on: bool) -> Result<()> {
    let addons = store.addons()?;
    let m = addons.get(id).with_context(|| format!("{id} is not installed"))?;
    if !overlay_guard::draws_over_the_game(m) {
        bail!("{} does not draw over the game", m.name);
    }
    store.update_profile(profile, |p| p.set_overlay(id, on))?;
    println!(
        "{} {} over the game in {profile}; it applies from the next launch",
        m.name,
        if on { "draws" } else { "no longer draws" }
    );
    Ok(())
}
