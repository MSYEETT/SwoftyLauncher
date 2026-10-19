use std::time::Duration;

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::store::Store;
use crate::update::{self, Asset, Release};
use crate::{Error, Result, io_err};

pub const ID: &str = "teamsimplified.mods";
pub const ASSET: &str = "teamsimplified.mods.swoftyaddon";
pub const LATEST_URL: &str =
    "https://api.github.com/repos/TeamSimplified/simplified-swofty-addon/releases/latest";
pub const RELEASES_URL: &str = "https://github.com/TeamSimplified/simplified-swofty-addon/releases";
pub const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600);
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(10);
const SUMS_TIMEOUT: Duration = Duration::from_secs(60);
const CACHE_DIR: &str = "cache/managed";

pub fn is_managed(id: &str) -> bool {
    id == ID
}

pub fn now_secs() -> u64 {
    update::now_secs()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ManagedConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<u64>,
}

impl ManagedConfig {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn due(&self, now: u64) -> bool {
        self.checked_at
            .is_none_or(|t| t > now || now - t >= CHECK_INTERVAL.as_secs())
    }

    pub fn record(&mut self, now: u64) {
        self.checked_at = Some(now);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    UpToDate(Version),
    Installed(Version),
}

pub fn newer(installed: Option<&Version>, release: &Version) -> bool {
    installed.is_none_or(|current| release > current)
}

pub fn fetch_latest(timeout: Duration) -> Result<Release> {
    let text = update::get_text(LATEST_URL, timeout)?;
    serde_json::from_str(&text).map_err(|e| Error::Other(format!("{LATEST_URL}: {e}")))
}

pub fn package_asset(release: &Release) -> Result<&Asset> {
    release
        .assets
        .iter()
        .find(|a| a.name == ASSET)
        .ok_or_else(|| Error::Other(format!("release {} has no {ASSET}", release.tag_name)))
}

pub fn check_and_install(store: &Store, timeout: Duration) -> Result<Outcome> {
    let release = fetch_latest(timeout)?;
    let version = release
        .version()
        .ok_or_else(|| Error::Other(format!("{} is not a version", release.tag_name)))?;
    let installed = store.addons()?.get(ID).map(|m| m.version.clone());
    if !newer(installed.as_ref(), &version) {
        return Ok(Outcome::UpToDate(version));
    }
    let asset = package_asset(&release)?;
    let sums = match release.sums() {
        Some(s) => Some(update::parse_sums(&update::get_text(
            &s.browser_download_url,
            SUMS_TIMEOUT,
        )?)),
        None => None,
    };
    let dir = store.root.join(CACHE_DIR);
    std::fs::create_dir_all(&dir).map_err(io_err(&dir))?;
    let file = dir.join(&asset.name);
    let verified = download_and_verify(asset, &file, sums.as_ref())
        .and_then(|()| store.install_addon(&file).map(|m| m.version));
    std::fs::remove_file(&file).ok();
    Ok(Outcome::Installed(verified?))
}

fn download_and_verify(
    asset: &Asset,
    file: &std::path::Path,
    sums: Option<&std::collections::BTreeMap<String, String>>,
) -> Result<()> {
    let (len, sha256) = update::download(asset, file, &|_| {})?;
    update::check_download(asset, len, &sha256, sums)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::update::SUMS_FILE;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn asset(name: &str, size: u64) -> Asset {
        Asset {
            name: name.into(),
            size,
            browser_download_url: format!("https://example.invalid/{name}"),
            digest: None,
        }
    }

    fn release(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            html_url: String::new(),
            assets: vec![asset(ASSET, 10), asset(SUMS_FILE, 98)],
        }
    }

    #[test]
    fn knows_the_managed_addon_id() {
        assert!(is_managed(ID));
        assert!(is_managed("teamsimplified.mods"));
        assert!(!is_managed("swofty.core"));
        assert!(!is_managed("teamsimplified.mods.extra"));
    }

    #[test]
    fn installs_only_a_newer_release() {
        assert!(newer(None, &v("0.5.0")));
        assert!(newer(Some(&v("0.4.0")), &v("0.5.0")));
        assert!(!newer(Some(&v("0.5.0")), &v("0.5.0")));
        assert!(!newer(Some(&v("0.6.0")), &v("0.5.0")));
    }

    #[test]
    fn selects_the_package_asset_and_verifies_its_sum() {
        let r = release("v0.5.0");
        assert_eq!(r.version(), Some(v("0.5.0")));
        let picked = package_asset(&r).unwrap();
        assert_eq!(picked.name, ASSET);
        let good = "a".repeat(64);
        let bad = "b".repeat(64);
        let sums = BTreeMap::from([(ASSET.to_owned(), good.clone())]);
        assert!(update::check_download(picked, 10, &good, Some(&sums)).is_ok());
        assert!(update::check_download(picked, 10, &bad, Some(&sums)).is_err());
        assert!(update::check_download(picked, 9, &good, Some(&sums)).is_err());
        let empty = Release {
            assets: vec![asset(SUMS_FILE, 98)],
            ..release("v0.5.0")
        };
        assert!(package_asset(&empty).is_err());
    }

    #[test]
    fn checks_at_most_once_per_interval() {
        let interval = CHECK_INTERVAL.as_secs();
        let mut config = ManagedConfig::default();
        assert!(config.is_default());
        assert!(config.due(1_000_000));
        config.record(1_000_000);
        assert!(!config.is_default());
        assert!(!config.due(1_000_000));
        assert!(!config.due(1_000_000 + interval - 1));
        assert!(config.due(1_000_000 + interval));
        assert!(config.due(1), "a clock that went back checks again");
        let json = serde_json::to_value(ManagedConfig::default()).unwrap();
        assert_eq!(json, serde_json::json!({}));
    }
}
