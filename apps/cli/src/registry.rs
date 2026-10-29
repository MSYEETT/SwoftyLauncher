//! `swofty registry` and `swofty careers`: the 1.6 standard library's
//! dependency check, the core's last conflicts report and status lines, and
//! the career index.

use std::collections::BTreeMap;
use std::process::ExitCode;

use anyhow::Result;
use clap::Subcommand;
use swofty_core::stdlib::deps::{self, Level};
use swofty_core::stdlib::{careers, reports};
use swofty_core::store::Store;

#[derive(Subcommand)]
pub enum RegistryCommand {
    #[command(
        about = "Check the profile's addon dependencies (registry.json, registry.lock.json) as Play does; exits 3 when Play would stop"
    )]
    Check,
    #[command(about = "Show the core's conflicts report from your last session")]
    Conflicts,
    #[command(about = "Show what didn't apply for each addon in your last session")]
    Status,
    #[command(
        about = "Show each box's track list from your last session: its calendar, more tracks and what it leaves out"
    )]
    Tracks,
}

fn game_dir(store: &Store) -> Option<std::path::PathBuf> {
    crate::build::game(store, None).ok().map(|g| g.dir)
}

pub fn command(store: &Store, profile: &str, cmd: RegistryCommand) -> Result<ExitCode> {
    match cmd {
        RegistryCommand::Check => check(store, profile),
        RegistryCommand::Conflicts => {
            conflicts(store);
            Ok(ExitCode::SUCCESS)
        }
        RegistryCommand::Status => {
            let lines = status(store);
            if lines.is_empty() {
                println!("nothing to report from your last session");
            }
            let installed = store.addons()?;
            for (id, lines) in lines {
                let name = installed.get(&id).map_or(id.as_str(), |m| m.name.as_str());
                for l in lines {
                    println!("{name:30} {l}");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        RegistryCommand::Tracks => {
            for l in track_lines(store) {
                println!("{l}");
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// The box track lists of the core's last `catalog.json`, a few lines per box and mode.
fn track_lines(store: &Store) -> Vec<String> {
    let game = game_dir(store);
    let Some(boxes) = reports::read_box_tracks(&store.root, game.as_deref()) else {
        return vec![format!(
            "no box track lists yet: the core writes them in {} when you pick a box",
            reports::path(&store.root, reports::CATALOG).display()
        )];
    };
    box_lines(&boxes)
}

fn box_lines(boxes: &reports::BoxTracks) -> Vec<String> {
    let mut out = vec!["as of your last session:".to_owned()];
    for (id, modes) in boxes {
        for (mode, list) in modes {
            out.push(format!(
                "box {id} ({}): calendar {}, more tracks {}, left out {}",
                mode.replace('_', " "),
                list.calendar.len(),
                list.more.len(),
                list.left_out.len()
            ));
            if !list.calendar.is_empty() {
                out.push(format!("  calendar: {}", list.calendar.join(", ")));
            }
            if !list.more.is_empty() {
                out.push(format!("  more tracks: {}", list.more.join(", ")));
            }
            for l in &list.left_out {
                out.push(format!("  left out {}: {}", l.track, l.reason));
            }
        }
    }
    out
}

/// The profile's dependency check, as the build runs it before Play.
pub fn report(store: &Store, profile: &str) -> Result<deps::Report> {
    let p = store.profile(profile)?;
    let installed = store.addons()?;
    if p.is_vanilla() {
        return Ok(deps::Report::default());
    }
    let order = swofty_core::resolver::resolve(&p, &installed)?.order;
    let all = deps::read_all(store, &installed);
    Ok(deps::for_order(profile, &order, &installed, &all))
}

fn check(store: &Store, profile: &str) -> Result<ExitCode> {
    let r = report(store, profile)?;
    if r.is_empty() {
        println!("profile {profile}: every dependency is met");
    }
    for f in &r.findings {
        let tag = match f.level {
            Level::Stop => "stops Play",
            Level::Warn => "warning",
            Level::Hint => "hint",
        };
        println!("{tag}: {}", f.block());
    }
    for n in &r.notes {
        println!("note: {n}");
    }
    Ok(match r.stops() {
        true => ExitCode::from(deps::STOP_EXIT),
        false => ExitCode::SUCCESS,
    })
}

fn conflicts(store: &Store) {
    let game = game_dir(store);
    let Some(c) = reports::Conflicts::read(&store.root, game.as_deref()) else {
        println!(
            "no conflicts report yet: the core writes {} at each session load",
            reports::path(&store.root, reports::CONFLICTS).display()
        );
        return;
    };
    let installed = store.addons().unwrap_or_default();
    let name = |id: &str| {
        installed
            .get(id)
            .map_or_else(|| id.to_owned(), |m| m.name.clone())
    };
    if c.registry.is_empty() {
        println!("as of your last session: no registry conflicts");
    } else {
        println!("as of your last session:");
    }
    for e in &c.registry {
        println!("  {}", e.line(&name));
    }
}

/// The status lines of every addon from the last session's reports.
pub fn status(store: &Store) -> BTreeMap<String, Vec<String>> {
    let game = game_dir(store);
    reports::status_lines(
        reports::Conflicts::read(&store.root, game.as_deref()).as_ref(),
        reports::Room::read(&store.root, game.as_deref()).as_ref(),
    )
}

pub fn careers(store: &Store, profile: &str) -> Result<()> {
    let list = careers::sync(&store.root, game_dir(store).as_deref(), None)?;
    if list.is_empty() {
        println!("no careers in the index yet: the core lists each career it creates or loads");
        return Ok(());
    }
    let installed = store.addons()?;
    let p = store.profile(profile)?;
    let order = match p.is_vanilla() {
        true => Vec::new(),
        false => swofty_core::resolver::resolve(&p, &installed)
            .map(|r| r.order)
            .unwrap_or_default(),
    };
    for c in &list {
        let started = c.profile.as_deref().unwrap_or("unknown profile");
        let label = if c.label.is_empty() {
            c.career.as_str()
        } else {
            c.label.as_str()
        };
        println!("{label} [{}] (started in {started})", c.career);
        let missing = careers::missing(c, &order, &installed);
        if missing.is_empty() {
            println!("  loads in profile {profile}");
        }
        for m in missing {
            println!("  won't load in profile {profile}: {m}");
        }
    }
    Ok(())
}

/// The careers that need `id`, for the warning before it's turned off.
pub fn careers_needing(store: &Store, id: &str) -> Vec<careers::Career> {
    careers::sync(&store.root, game_dir(store).as_deref(), None)
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.addons.iter().any(|a| a.id == id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_lines_list_each_box_and_mode() {
        let text = r#"{ "tracks": { "game/f1_2026": {
            "grand_prix": { "calendar": ["game/melbourne"], "more": ["mod.okayama/okayama"],
                            "left_out": [ { "track": "mod.x/x", "reason": "no Time Trial support" } ] },
            "time_trial": { "calendar": [], "more": [], "left_out": [] } } } }"#;
        let boxes = reports::box_tracks(text).unwrap();
        assert_eq!(
            box_lines(&boxes),
            [
                "as of your last session:",
                "box game/f1_2026 (grand prix): calendar 1, more tracks 1, left out 1",
                "  calendar: game/melbourne",
                "  more tracks: mod.okayama/okayama",
                "  left out mod.x/x: no Time Trial support",
                "box game/f1_2026 (time trial): calendar 0, more tracks 0, left out 0",
            ]
        );
    }
}
