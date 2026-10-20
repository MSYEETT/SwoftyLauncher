use std::time::Duration;

use semver::Version;

use crate::launcher::Progress;
use crate::store::Store;
use crate::update::{self, Failure, Install, Release};
use crate::{Error, Result, managed};

pub const CHECK_INTERVAL: Duration = Duration::from_secs(60);
pub const RELEASE_RECHECK: Duration = Duration::from_secs(5 * 60);
pub const UPDATED_ENV: &str = "SWOFTY_UPDATED_TO";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Part {
    Addon,
    Launcher,
}

impl Part {
    pub fn name(self) -> &'static str {
        match self {
            Self::Addon => managed::NAME,
            Self::Launcher => "Swofty Launcher",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Addon => "Simplified",
            Self::Launcher => "Launcher",
        }
    }

    pub fn releases_url(self) -> &'static str {
        match self {
            Self::Addon => managed::RELEASES_URL,
            Self::Launcher => update::RELEASES_URL,
        }
    }

    pub fn api_url(self) -> &'static str {
        match self {
            Self::Addon => managed::LATEST_URL,
            Self::Launcher => update::LATEST_URL,
        }
    }

    pub fn ready(self, release: &Release) -> bool {
        match self {
            Self::Addon => managed::ready(release),
            Self::Launcher => release
                .platform_asset()
                .is_ok_and(|a| update::verifiable(release, a)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Probe {
    Tag(String),
    NoRelease,
    Refused(Failure),
}

fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn tag_from_location(location: &str) -> Option<String> {
    let (_, rest) = location.split_once("/releases/tag/")?;
    let tag = unescape(rest.split(['?', '#', '/']).next()?);
    (!tag.is_empty()).then_some(tag)
}

pub fn read_probe(url: &str, status: u16, location: Option<&str>) -> Probe {
    if !(300..400).contains(&status) {
        return Probe::Refused(Failure::Failed(format!(
            "{url}: expected a redirect to the latest release, got HTTP {status}"
        )));
    }
    location
        .and_then(tag_from_location)
        .map_or(Probe::NoRelease, Probe::Tag)
}

pub fn probe(part: Part, timeout: Duration) -> Probe {
    let url = format!("{}/latest", part.releases_url());
    match update::request(&url, true, Some(timeout)) {
        Ok(r) => read_probe(
            &url,
            r.status().as_u16(),
            r.headers().get("location").and_then(|v| v.to_str().ok()),
        ),
        Err(f) => Probe::Refused(f),
    }
}

#[derive(Debug, Clone, Default)]
pub struct Track {
    pub tag: Option<String>,
    pub release: Option<Release>,
    pub probe_at: u64,
    pub fetch_at: u64,
    pub error: Option<String>,
}

impl Track {
    pub fn probe_due(&self, now: u64) -> bool {
        now >= self.probe_at
    }

    pub fn version(&self) -> Option<Version> {
        update::parse_version(self.tag.as_deref()?)
    }

    pub fn newer_than(&self, installed: Option<&Version>) -> bool {
        self.version().is_some_and(|v| managed::newer(installed, &v))
    }

    pub fn on_probe(&mut self, probe: Probe, now: u64) {
        let interval = CHECK_INTERVAL.as_secs();
        self.probe_at = now + interval;
        self.error = None;
        match probe {
            Probe::Tag(tag) => self.tag = Some(tag),
            Probe::NoRelease => {
                self.tag = None;
                self.release = None;
            }
            Probe::Refused(Failure::Wait(secs, text)) => {
                self.probe_at = now + secs.max(interval);
                self.error = Some(text);
            }
            Probe::Refused(Failure::Failed(text)) => self.error = Some(text),
        }
    }

    pub fn fetch_due(&self, installed: Option<&Version>, now: u64) -> bool {
        self.newer_than(installed)
            && self.release.as_ref().map(|r| &r.tag_name) != self.tag.as_ref()
            && now >= self.fetch_at
    }

    pub fn on_fetch(&mut self, part: Part, fetched: std::result::Result<Release, Failure>, now: u64) {
        self.fetch_at = now + RELEASE_RECHECK.as_secs();
        match fetched {
            Ok(r) if part.ready(&r) && r.version().is_some_and(|v| self.version().is_none_or(|t| v >= t)) => {
                self.tag = Some(r.tag_name.clone());
                self.release = Some(r);
            }
            Ok(r) => self.error = Some(format!("release {} is not complete yet", r.tag_name)),
            Err(Failure::Wait(secs, text)) => {
                self.fetch_at = now + secs.max(CHECK_INTERVAL.as_secs());
                self.error = Some(text);
            }
            Err(Failure::Failed(text)) => self.error = Some(text),
        }
    }

    pub fn offer(&self, installed: Option<&Version>) -> Option<&Release> {
        self.release.as_ref().filter(|r| {
            Some(&r.tag_name) == self.tag.as_ref()
                && r.version().is_some_and(|v| managed::newer(installed, &v))
        })
    }
}

pub fn check(part: Part, track: &mut Track, installed: Option<&Version>, fetch: bool, now: u64) {
    track.on_probe(probe(part, update::CHECK_TIMEOUT), now);
    if fetch && track.fetch_due(installed, now) {
        track.on_fetch(
            part,
            update::fetch_release(part.api_url(), update::CHECK_TIMEOUT),
            now,
        );
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Idle,
    Installing,
    Failed(String),
    Restarting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    Play,
    Update,
    Installing,
    Retry,
    Restarting,
}

pub fn gate(phase: &Phase, pending: bool, game_busy: bool) -> Gate {
    match phase {
        Phase::Installing => Gate::Installing,
        Phase::Restarting => Gate::Restarting,
        _ if game_busy || !pending => Gate::Play,
        Phase::Failed(_) => Gate::Retry,
        Phase::Idle => Gate::Update,
    }
}

pub fn plan(addon: Option<&Release>, launcher: Option<&Release>) -> Vec<(Part, Release)> {
    [(Part::Addon, addon), (Part::Launcher, launcher)]
        .into_iter()
        .filter_map(|(part, release)| release.map(|r| (part, r.clone())))
        .collect()
}

pub fn summary(parts: &[(Part, Version)]) -> String {
    match parts {
        [(part, version)] => format!("{} {version}", part.short()),
        _ => parts
            .iter()
            .map(|(part, _)| part.short())
            .collect::<Vec<_>>()
            .join(" + "),
    }
}

pub fn describe(parts: &[(Part, Version)]) -> String {
    parts
        .iter()
        .map(|(part, version)| format!("{} {version}", part.name()))
        .collect::<Vec<_>>()
        .join(" and ")
}

pub struct Step<'a> {
    inner: &'a dyn Progress,
    index: usize,
    count: usize,
}

impl Progress for Step<'_> {
    fn report(&self, done: f32, text: &str) {
        let overall = (self.index as f32 + done.clamp(0., 1.)) / self.count.max(1) as f32;
        if self.count > 1 {
            self.inner
                .report(overall, &format!("{text} ({} of {})", self.index + 1, self.count));
        } else {
            self.inner.report(overall, text);
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Ran {
    pub done: Vec<(Part, Version)>,
    pub error: Option<String>,
}

impl Ran {
    pub fn restart(&self) -> bool {
        self.error.is_none() && !self.done.is_empty()
    }

    pub fn launcher(&self) -> Option<&Version> {
        self.done
            .iter()
            .find(|(part, _)| *part == Part::Launcher)
            .map(|(_, v)| v)
    }
}

pub fn run(
    steps: &[(Part, Release)],
    progress: &dyn Progress,
    mut install: impl FnMut(Part, &Release, &dyn Progress) -> Result<Version>,
) -> Ran {
    let mut ran = Ran::default();
    for (index, (part, release)) in steps.iter().enumerate() {
        let step = Step {
            inner: progress,
            index,
            count: steps.len(),
        };
        step.report(0., &format!("Updating {}", part.name()));
        match install(*part, release, &step) {
            Ok(version) => ran.done.push((*part, version)),
            Err(e) => {
                ran.error = Some(format!("{} {}: {e}", part.name(), release.tag_name));
                break;
            }
        }
    }
    ran
}

pub fn install(
    store: &Store,
    copy: Option<&Install>,
    part: Part,
    release: &Release,
    progress: &dyn Progress,
) -> Result<Version> {
    if crate::process::game_running() {
        return Err(Error::Other("close F1 25 before updating".into()));
    }
    match part {
        Part::Addon => managed::install_release(store, release, progress),
        Part::Launcher => copy
            .ok_or_else(|| Error::Other("this copy cannot update itself".into()))?
            .update(release, progress),
    }
}

pub fn did_not_take(expected: Option<&str>, current: &Version) -> Option<String> {
    let wanted = update::parse_version(expected?)?;
    (wanted > *current).then(|| {
        format!("Swofty Launcher {wanted} was installed, but the restarted launcher is still {current}")
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::update::{Asset, SUMS_FILE};

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn asset(name: &str) -> Asset {
        Asset {
            name: name.into(),
            size: 10,
            browser_download_url: format!("https://example.invalid/{name}"),
            digest: None,
        }
    }

    fn launcher_release(tag: &str) -> Release {
        let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
        let platform = update::asset_name(tag, os, arch).unwrap_or_else(|| "none".into());
        Release {
            tag_name: tag.into(),
            html_url: String::new(),
            assets: vec![asset(&platform), asset(SUMS_FILE)],
        }
    }

    fn addon_release(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            html_url: String::new(),
            assets: vec![asset(managed::ASSET), asset(SUMS_FILE)],
        }
    }

    struct Log(Mutex<Vec<(f32, String)>>);

    impl Progress for Log {
        fn report(&self, done: f32, text: &str) {
            self.0.lock().unwrap().push((done, text.to_owned()));
        }
    }

    #[test]
    fn reads_the_tag_from_the_latest_redirect() {
        let url = "https://github.com/o/r/releases/latest";
        assert_eq!(
            read_probe(url, 302, Some("https://github.com/o/r/releases/tag/v1.0.8")),
            Probe::Tag("v1.0.8".into())
        );
        assert_eq!(
            read_probe(url, 302, Some("/o/r/releases/tag/v1.0.9-rc.1%2Bbuild?x=1")),
            Probe::Tag("v1.0.9-rc.1+build".into())
        );
        assert_eq!(
            read_probe(url, 302, Some("https://github.com/o/r/releases")),
            Probe::NoRelease
        );
        assert_eq!(read_probe(url, 302, None), Probe::NoRelease);
        assert!(matches!(
            read_probe(url, 200, None),
            Probe::Refused(Failure::Failed(_))
        ));
    }

    #[test]
    fn checks_every_minute_and_backs_off_when_limited() {
        let mut t = Track::default();
        assert!(t.probe_due(0));
        t.on_probe(Probe::Tag("v1.0.8".into()), 1000);
        assert!(!t.probe_due(1059));
        assert!(t.probe_due(1060));
        t.on_probe(Probe::Refused(Failure::Failed("offline".into())), 1060);
        assert_eq!(
            t.tag.as_deref(),
            Some("v1.0.8"),
            "a failed check keeps what it knew"
        );
        assert_eq!(t.error.as_deref(), Some("offline"));
        assert!(t.probe_due(1120));
        t.on_probe(Probe::Refused(Failure::Wait(900, "limited".into())), 1120);
        assert!(!t.probe_due(1120 + 899));
        assert!(t.probe_due(1120 + 900));
        t.on_probe(Probe::Refused(Failure::Wait(5, "limited".into())), 3000);
        assert!(!t.probe_due(3030), "never sooner than the interval");
        t.on_probe(Probe::Tag("v1.0.9".into()), 4000);
        assert!(t.error.is_none());
        t.on_probe(Probe::NoRelease, 5000);
        assert!(t.tag.is_none() && t.release.is_none());
    }

    #[test]
    fn asks_the_api_once_per_new_release() {
        let current = v("1.0.8");
        let mut t = Track::default();
        t.on_probe(Probe::Tag("v1.0.8".into()), 1000);
        assert!(!t.fetch_due(Some(&current), 1000), "up to date costs no API call");
        t.on_probe(Probe::Tag("v1.0.9".into()), 1060);
        assert!(t.fetch_due(Some(&current), 1060));
        t.on_fetch(Part::Launcher, Ok(launcher_release("v1.0.9")), 1060);
        assert_eq!(t.offer(Some(&current)).unwrap().tag_name, "v1.0.9");
        assert!(t.offer(Some(&v("1.0.9"))).is_none());
        t.on_probe(Probe::Tag("v1.0.9".into()), 1120);
        assert!(
            !t.fetch_due(Some(&current), 1120),
            "the known release is not fetched again"
        );
        t.on_probe(Probe::Tag("v1.1.0".into()), 2000);
        assert!(
            t.offer(Some(&current)).is_none(),
            "a stale release is not offered"
        );
        assert!(t.fetch_due(Some(&current), 2000));
    }

    #[test]
    fn waits_for_an_incomplete_or_limited_release() {
        let current = v("1.0.8");
        let mut t = Track::default();
        t.on_probe(Probe::Tag("v1.0.9".into()), 1000);
        let bare = Release {
            assets: vec![],
            ..launcher_release("v1.0.9")
        };
        t.on_fetch(Part::Launcher, Ok(bare), 1000);
        assert!(t.offer(Some(&current)).is_none());
        assert!(!t.fetch_due(Some(&current), 1000 + RELEASE_RECHECK.as_secs() - 1));
        assert!(t.fetch_due(Some(&current), 1000 + RELEASE_RECHECK.as_secs()));
        t.on_fetch(Part::Launcher, Ok(launcher_release("v1.0.8")), 1300);
        assert!(
            t.offer(Some(&current)).is_none(),
            "an older API answer is ignored"
        );
        t.on_fetch(Part::Launcher, Err(Failure::Wait(3000, "limited".into())), 1600);
        assert!(!t.fetch_due(Some(&current), 4599));
        assert!(t.fetch_due(Some(&current), 4600));
        t.on_fetch(Part::Launcher, Ok(launcher_release("v1.0.9")), 4600);
        assert!(t.offer(Some(&current)).is_some());
    }

    #[test]
    fn a_missing_addon_is_always_offered() {
        let mut t = Track::default();
        t.on_probe(Probe::Tag("v0.5.2".into()), 10);
        assert!(t.fetch_due(None, 10));
        t.on_fetch(Part::Addon, Ok(addon_release("v0.5.2")), 10);
        assert!(t.offer(None).is_some());
        assert!(t.offer(Some(&v("0.5.2"))).is_none());
        let mut l = Track::default();
        l.on_probe(Probe::Tag("v0.5.2".into()), 10);
        l.on_fetch(Part::Launcher, Ok(addon_release("v0.5.2")), 10);
        assert!(l.release.is_none(), "an addon release is not a launcher release");
    }

    #[test]
    fn update_takes_the_play_button_only_when_safe() {
        assert_eq!(gate(&Phase::Idle, false, false), Gate::Play);
        assert_eq!(gate(&Phase::Idle, true, false), Gate::Update);
        assert_eq!(
            gate(&Phase::Idle, true, true),
            Gate::Play,
            "the game keeps its button"
        );
        assert_eq!(gate(&Phase::Installing, true, false), Gate::Installing);
        assert_eq!(gate(&Phase::Restarting, true, true), Gate::Restarting);
        let failed = Phase::Failed("no network".into());
        assert_eq!(gate(&failed, true, false), Gate::Retry);
        assert_eq!(gate(&failed, true, true), Gate::Play);
        assert_eq!(gate(&failed, false, false), Gate::Play);
    }

    #[test]
    fn installs_the_addon_before_the_launcher() {
        let (a, l) = (addon_release("v0.5.3"), launcher_release("v1.0.9"));
        let order: Vec<Part> = plan(Some(&a), Some(&l)).into_iter().map(|(p, _)| p).collect();
        assert_eq!(order, [Part::Addon, Part::Launcher]);
        assert_eq!(plan(None, Some(&l))[0].0, Part::Launcher);
        assert!(plan(None, None).is_empty());
        let log = Log(Mutex::default());
        let mut seen = Vec::new();
        let ran = run(&plan(Some(&a), Some(&l)), &log, |part, release, p| {
            seen.push(part);
            p.report(0.5, "Downloading");
            Ok(release.version().unwrap())
        });
        assert_eq!(seen, [Part::Addon, Part::Launcher]);
        assert!(ran.restart());
        assert_eq!(ran.launcher(), Some(&v("1.0.9")));
        let log = log.0.into_inner().unwrap();
        assert!(log.contains(&(0.25, "Downloading (1 of 2)".into())), "{log:?}");
        assert!(log.contains(&(0.75, "Downloading (2 of 2)".into())), "{log:?}");
        assert_eq!(summary(&ran.done), "Simplified + Launcher");
        assert_eq!(summary(&ran.done[1..]), "Launcher 1.0.9");
        assert_eq!(
            describe(&ran.done),
            "Simplified for Swofty 0.5.3 and Swofty Launcher 1.0.9"
        );
    }

    #[test]
    fn a_failed_step_stops_without_a_restart() {
        let (a, l) = (addon_release("v0.5.3"), launcher_release("v1.0.9"));
        let log = Log(Mutex::default());
        let mut seen = Vec::new();
        let ran = run(&plan(Some(&a), Some(&l)), &log, |part, release, _| {
            seen.push(part);
            match part {
                Part::Addon => Err(Error::Other("SHA-256 mismatch".into())),
                Part::Launcher => Ok(release.version().unwrap()),
            }
        });
        assert_eq!(seen, [Part::Addon], "the launcher is not touched after a failure");
        assert!(!ran.restart() && ran.done.is_empty());
        assert!(
            ran.error
                .unwrap()
                .contains("Simplified for Swofty v0.5.3: SHA-256 mismatch")
        );
        let ran = run(&plan(Some(&a), Some(&l)), &log, |part, release, _| match part {
            Part::Addon => Ok(release.version().unwrap()),
            Part::Launcher => Err(Error::Other("disk full".into())),
        });
        assert_eq!(ran.done, [(Part::Addon, v("0.5.3"))]);
        assert!(!ran.restart() && ran.launcher().is_none());
        let only_addon = run(&plan(Some(&a), None), &log, |_, r, _| Ok(r.version().unwrap()));
        assert!(only_addon.restart(), "an addon-only update restarts too");
    }

    #[test]
    fn notices_an_update_that_did_not_take() {
        assert!(did_not_take(None, &v("1.0.8")).is_none());
        assert!(did_not_take(Some("v1.0.8"), &v("1.0.8")).is_none());
        assert!(did_not_take(Some("garbage"), &v("1.0.8")).is_none());
        let text = did_not_take(Some("v1.0.9"), &v("1.0.8")).unwrap();
        assert!(text.contains("1.0.9") && text.contains("1.0.8"), "{text}");
    }
}
