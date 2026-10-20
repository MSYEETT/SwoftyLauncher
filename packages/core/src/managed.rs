use std::time::Duration;

use semver::Version;

use crate::launcher::Progress;
use crate::store::Store;
use crate::update::{self, Asset, Failure, Release};
use crate::{Error, Result, io_err};

pub const ID: &str = "teamsimplified.mods";
pub const NAME: &str = "Simplified for Swofty";
pub const ASSET: &str = "teamsimplified.mods.swoftyaddon";
pub const LATEST_URL: &str =
    "https://api.github.com/repos/TeamSimplified/simplified-swofty-addon/releases/latest";
pub const RELEASES_URL: &str = "https://github.com/TeamSimplified/simplified-swofty-addon/releases";
const SUMS_TIMEOUT: Duration = Duration::from_secs(60);
const CACHE_DIR: &str = "cache/managed";

pub fn is_managed(id: &str) -> bool {
    id == ID
}

pub fn newer(installed: Option<&Version>, release: &Version) -> bool {
    installed.is_none_or(|current| release > current)
}

pub fn installed(store: &Store) -> Option<Version> {
    store.addons().ok()?.get(ID).map(|m| m.version.clone())
}

pub fn fetch_latest(timeout: Duration) -> std::result::Result<Release, Failure> {
    update::fetch_release(LATEST_URL, timeout)
}

pub fn package_asset(release: &Release) -> Result<&Asset> {
    release
        .assets
        .iter()
        .find(|a| a.name == ASSET)
        .ok_or_else(|| Error::Other(format!("release {} has no {ASSET}", release.tag_name)))
}

pub fn ready(release: &Release) -> bool {
    package_asset(release).is_ok_and(|a| update::verifiable(release, a))
}

pub fn install_release(store: &Store, release: &Release, progress: &dyn Progress) -> Result<Version> {
    let asset = package_asset(release)?;
    if !update::verifiable(release, asset) {
        return Err(Error::Other(format!(
            "release {} has no SHA-256 for {ASSET}",
            release.tag_name
        )));
    }
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
    let total = asset.size.max(1) as f32;
    let text = format!("Downloading {}", release.tag_name);
    let verified = update::download(asset, &file, &|n| progress.report(0.9 * n as f32 / total, &text))
        .and_then(|(len, sha256)| update::check_download(asset, len, &sha256, sums.as_ref()))
        .and_then(|()| {
            progress.report(0.95, &format!("Installing {}", release.tag_name));
            store.install_addon(&file).map(|m| m.version)
        });
    std::fs::remove_file(&file).ok();
    verified
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
    fn offers_only_a_release_it_can_verify() {
        assert!(ready(&release("v0.5.0")));
        let unsummed = Release {
            assets: vec![asset(ASSET, 10)],
            ..release("v0.5.0")
        };
        assert!(!ready(&unsummed));
        let digested = Release {
            assets: vec![Asset {
                digest: Some(format!("sha256:{}", "a".repeat(64))),
                ..asset(ASSET, 10)
            }],
            ..release("v0.5.0")
        };
        assert!(ready(&digested));
        let empty = Release {
            assets: vec![asset(SUMS_FILE, 98)],
            ..release("v0.5.0")
        };
        assert!(!ready(&empty));
    }
}
