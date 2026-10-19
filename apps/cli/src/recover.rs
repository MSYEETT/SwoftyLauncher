use std::time::Duration;

use anyhow::{Result, bail};
use swofty_core::install::{Conflict, Game};
use swofty_core::recovery::{self, Blocked, HEADLINE, Notice, Recovered, Scope, SessionLock};
use swofty_core::store::Store;

pub const LOCK_WAIT: Duration = Duration::from_secs(120);

/// Take the session lock, waiting while a previous session finishes rolling
/// back its game files.
pub fn lock(store: &Store, out: &dyn Fn(&str)) -> Result<SessionLock> {
    if let Some(lock) = SessionLock::try_acquire(&store.root)? {
        return Ok(lock);
    }
    out("waiting for the previous session to finish rolling back its game files");
    match SessionLock::wait(&store.root, LOCK_WAIT)? {
        Some(lock) => Ok(lock),
        None => bail!(
            "another swofty session is still running or rolling back ({} is held); close F1 25, wait for it to finish, then try again",
            recovery::lock_path(&store.root).display()
        ),
    }
}

/// Roll back what unfinished sessions left before anything else happens,
/// and say so.
pub fn stale(
    store: &Store,
    lock: &SessionLock,
    g: &Game,
    scope: Scope,
    out: &dyn Fn(&str),
) -> Result<Recovered> {
    let r = recovery::recover(lock, &store.root, g, scope)?;
    if r.recovered() {
        announce(store, g, &r.lines(), out);
    } else {
        for l in r.lines() {
            out(&l);
        }
    }
    Ok(r)
}

/// The recovery headline, what was put back, and the doctor's report.
pub fn announce(store: &Store, g: &Game, lines: &[String], out: &dyn Fn(&str)) {
    out(HEADLINE);
    for l in lines {
        out(&format!("  {l}"));
    }
    let (report, _) = crate::doctor::report(store, Some(g));
    for l in report {
        out(&format!("doctor: {l}"));
    }
}

/// The error for files that block a profile and are not provably ours:
/// nothing was touched, and the message says what a restore would do.
pub fn blocked(store: &Store, g: &Game, found: &[Conflict]) -> anyhow::Error {
    let err = swofty_core::Error::Leftovers(found.to_vec());
    if found.iter().all(|c| c.owned) {
        return anyhow::anyhow!("{err}; swofty restore rolls the game folder back to the baseline");
    }
    match Blocked::record(&store.root, &g.dir, found) {
        Ok(b) => anyhow::anyhow!(
            "{err}; it was left untouched in case it is a mod copied in by hand. {}",
            b.explain(&store.root)
        ),
        Err(e) => anyhow::anyhow!("{err}; it was left untouched ({e})"),
    }
}

pub fn record_cleared(store: &Store, lines: &[String]) {
    Notice::record(&store.root, lines).ok();
}
