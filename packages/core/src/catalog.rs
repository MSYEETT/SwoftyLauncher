//! The addon catalog the Addon Browser shows: the addons published to the
//! Swofty registry, fetched and saved so the last catalog shows offline, and
//! downloads checked against the size and SHA-256 the registry lists.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use semver::VersionReq;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::manifest::{AddonManifest, Category};
use crate::store::Store;
use crate::{Error, Result, io_err};

pub const URL_ENV: &str = "SWOFTY_REGISTRY_URL";
pub const DEFAULT_URL: &str = "https://api.f1.swofty.net";
pub const CACHE_DIR: &str = "cache/registry";
pub const CATALOG_FILE: &str = "catalog.json";
pub const TIMEOUT: Duration = Duration::from_secs(10);
pub const DOWNLOAD_LIMIT: u64 = 300 * 1024 * 1024;
const CATALOG_LIMIT: u64 = 16 * 1024 * 1024;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

static INSTALLING: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Publisher {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Addon {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub kind: Vec<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub publishers: Vec<Publisher>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub latest: Option<Version>,
    #[serde(default)]
    pub versions: Vec<Version>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Version {
    pub version: semver::Version,
    #[serde(default)]
    pub launcher: Option<VersionReq>,
    #[serde(default)]
    pub game: String,
    pub size: u64,
    pub sha256: String,
    pub url: String,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub changelog: String,
    #[serde(default)]
    pub native: bool,
    #[serde(default)]
    pub yanked: bool,
}

impl Version {
    pub fn accepts(&self, launcher: &semver::Version) -> bool {
        self.launcher.as_ref().is_none_or(|r| r.matches(launcher))
    }
}

impl Addon {
    /// Every listed version once, newest first, yanked ones left out.
    pub fn listed(&self) -> Vec<&Version> {
        let mut out: Vec<&Version> = Vec::new();
        for v in self.versions.iter().chain(self.latest.iter()) {
            if !v.yanked && !out.iter().any(|o| o.version == v.version) {
                out.push(v);
            }
        }
        out.sort_by(|a, b| b.version.cmp(&a.version));
        out
    }

    /// The newest version `launcher` can install.
    pub fn offered(&self, launcher: &semver::Version) -> Option<&Version> {
        self.listed().into_iter().find(|v| v.accepts(launcher))
    }

    pub fn newest(&self) -> Option<&Version> {
        self.listed().into_iter().next()
    }

    pub fn native(&self, launcher: &semver::Version) -> bool {
        self.offered(launcher)
            .or_else(|| self.newest())
            .is_some_and(|v| v.native)
    }

    pub fn category_label(&self) -> Option<String> {
        let raw = self.category.as_deref()?.trim();
        if raw.is_empty() {
            return None;
        }
        let known = serde_json::from_value::<Category>(serde_json::Value::String(raw.to_owned()));
        Some(match known {
            Ok(c) => c.label().to_owned(),
            Err(_) => title_case(raw),
        })
    }

    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || [&self.name, &self.id, &self.author, &self.summary]
                .into_iter()
                .chain(self.category.as_ref())
                .any(|s| s.to_lowercase().contains(&query))
    }
}

fn title_case(text: &str) -> String {
    text.split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    pub id: String,
    pub name: String,
    pub installed: semver::Version,
    pub to: Version,
}

/// The installed addons the catalog has a newer version of that `launcher` can install.
pub fn updates<'a>(
    installed: impl IntoIterator<Item = &'a AddonManifest>,
    catalog: &[Addon],
    launcher: &semver::Version,
) -> Vec<Update> {
    installed
        .into_iter()
        .filter_map(|m| {
            let addon = catalog.iter().find(|a| a.id == m.id)?;
            let to = addon.offered(launcher).filter(|v| v.version > m.version)?;
            Some(Update {
                id: m.id.clone(),
                name: addon.name.clone(),
                installed: m.version.clone(),
                to: to.clone(),
            })
        })
        .collect()
}

/// [`updates`] for the addons in `store`, leaving out the ones the launcher ships.
pub fn store_updates(store: &Store, catalog: &[Addon], launcher: &semver::Version) -> Result<Vec<Update>> {
    let addons = store.addons()?;
    let installed = addons
        .values()
        .filter(|m| !store.is_builtin(&m.id) && !crate::managed::is_managed(&m.id));
    Ok(updates(installed, catalog, launcher))
}

pub fn base_url() -> String {
    std::env::var(URL_ENV)
        .ok()
        .map(|u| u.trim().trim_end_matches('/').to_owned())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| DEFAULT_URL.to_owned())
}

pub fn catalog_url() -> String {
    format!("{}/v1/addons", base_url())
}

#[derive(Deserialize, Serialize)]
struct Catalog {
    addons: Vec<Addon>,
}

pub fn parse(text: &str) -> Option<Vec<Addon>> {
    serde_json::from_str::<Catalog>(text).ok().map(|c| c.addons)
}

pub fn cache_path(root: &Path) -> PathBuf {
    root.join(CACHE_DIR).join(CATALOG_FILE)
}

pub fn cached(root: &Path) -> Option<Vec<Addon>> {
    std::fs::read_to_string(cache_path(root))
        .ok()
        .and_then(|t| parse(&t))
}

#[derive(Debug, Clone, PartialEq)]
pub enum Trouble {
    Offline(String),
    Failed(String),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Offline(text) | Self::Failed(text) => f.write_str(text),
        }
    }
}

fn get(url: &str, timeout: Duration) -> std::result::Result<ureq::Body, Trouble> {
    let response = ureq::get(url)
        .header(
            "User-Agent",
            concat!("swofty-launcher/", env!("CARGO_PKG_VERSION")),
        )
        .config()
        .timeout_global(Some(timeout))
        .timeout_connect(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .build()
        .call()
        .map_err(|e| Trouble::Offline(format!("{url}: {e}")))?;
    let status = response.status().as_u16();
    if status >= 400 {
        return Err(Trouble::Failed(format!("{url}: HTTP {status}")));
    }
    Ok(response.into_body())
}

/// The catalog from the registry, saved for the next start.
pub fn fetch(root: &Path) -> std::result::Result<Vec<Addon>, Trouble> {
    let url = catalog_url();
    let text = get(&url, TIMEOUT)?
        .with_config()
        .limit(CATALOG_LIMIT)
        .read_to_string()
        .map_err(|e| Trouble::Failed(format!("{url}: {e}")))?;
    let addons = parse(&text).ok_or_else(|| Trouble::Failed(format!("{url}: not an addon catalog")))?;
    let path = cache_path(root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    if let Err(e) = std::fs::write(&path, text) {
        eprintln!("swofty-core: could not save the addon catalog: {e}");
    }
    Ok(addons)
}

pub fn verify(v: &Version, len: u64, sha256: &str) -> Result<()> {
    if len != v.size {
        return Err(Error::Other(format!(
            "downloaded {len} bytes, the registry lists {}",
            v.size
        )));
    }
    if !sha256.eq_ignore_ascii_case(v.sha256.trim()) {
        return Err(Error::Other(format!(
            "the download's SHA-256 is {sha256}, the registry lists {}",
            v.sha256
        )));
    }
    Ok(())
}

/// Copies `reader` to `to`, refusing more bytes than `v` lists, then checks
/// the size and SHA-256.
pub fn save(mut reader: impl Read, v: &Version, to: &Path, progress: &dyn Fn(u64)) -> Result<()> {
    if v.size > DOWNLOAD_LIMIT {
        return Err(Error::Other(format!(
            "{} bytes is over the {DOWNLOAD_LIMIT}-byte download limit",
            v.size
        )));
    }
    let mut file = std::fs::File::create(to).map_err(io_err(to))?;
    let mut hash = Sha256::new();
    let mut buf = vec![0; 64 * 1024];
    let mut len = 0u64;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| Error::Other(format!("{}: {e}", v.url)))?;
        if n == 0 {
            break;
        }
        len += n as u64;
        if len > v.size {
            return Err(Error::Other(format!(
                "larger than the {} bytes the registry lists",
                v.size
            )));
        }
        hash.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(io_err(to))?;
        progress(len);
    }
    file.sync_all().map_err(io_err(to))?;
    let digest: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    verify(v, len, &digest)
}

pub fn download(v: &Version, to: &Path, progress: &dyn Fn(u64)) -> Result<()> {
    let mut body = get(&v.url, DOWNLOAD_TIMEOUT).map_err(|t| Error::Other(t.to_string()))?;
    save(body.with_config().limit(DOWNLOAD_LIMIT).reader(), v, to, progress)
}

/// Downloads `v` of `addon`, checks it and installs it into `store`.
pub fn install(store: &Store, addon: &Addon, v: &Version, progress: &dyn Fn(u64)) -> Result<AddonManifest> {
    let dir = store.root.join(CACHE_DIR).join("downloads");
    std::fs::create_dir_all(&dir).map_err(io_err(&dir))?;
    let safe: String = addon
        .id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = dir.join(format!("{safe}-{}.zip", v.version));
    let result = download(v, &path, progress).and_then(|()| {
        let _one = INSTALLING.lock().unwrap_or_else(|e| e.into_inner());
        store.install_addon(&path)
    });
    std::fs::remove_file(&path).ok();
    let m = result?;
    if m.id != addon.id {
        return Err(Error::Other(format!(
            "the registry lists {}, the download installed {}",
            addon.id, m.id
        )));
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(v: &str, launcher: Option<&str>) -> Version {
        Version {
            version: semver::Version::parse(v).unwrap(),
            launcher: launcher.map(|l| VersionReq::parse(l).unwrap()),
            game: "f1_25".into(),
            size: 3,
            sha256: String::new(),
            url: format!("http://localhost/{v}"),
            published_at: String::new(),
            changelog: String::new(),
            native: false,
            yanked: false,
        }
    }

    fn addon(id: &str, versions: Vec<Version>) -> Addon {
        Addon {
            id: id.into(),
            name: id.into(),
            author: "a".into(),
            summary: String::new(),
            description: String::new(),
            category: None,
            kind: Vec::new(),
            homepage: None,
            icon: None,
            publishers: Vec::new(),
            downloads: 0,
            created_at: String::new(),
            updated_at: String::new(),
            latest: versions.first().cloned(),
            versions,
        }
    }

    fn manifest(id: &str, v: &str) -> AddonManifest {
        serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "id": id,
            "name": id,
            "version": v,
            "author": "a",
            "compat": { "game": "f1_25" }
        }))
        .unwrap()
    }

    fn launcher(v: &str) -> semver::Version {
        semver::Version::parse(v).unwrap()
    }

    #[test]
    fn offers_the_newest_version_this_launcher_can_install() {
        let mut yanked = version("0.4.0", None);
        yanked.yanked = true;
        let a = addon(
            "x",
            vec![
                yanked,
                version("0.3.0", Some(">=2.0.0")),
                version("0.2.0", Some(">=1.3.0")),
                version("0.1.0", None),
            ],
        );
        assert_eq!(
            a.offered(&launcher("1.3.7")).unwrap().version.to_string(),
            "0.2.0"
        );
        assert_eq!(
            a.offered(&launcher("2.1.0")).unwrap().version.to_string(),
            "0.3.0"
        );
        assert_eq!(
            a.offered(&launcher("1.0.0")).unwrap().version.to_string(),
            "0.1.0"
        );
        assert_eq!(a.newest().unwrap().version.to_string(), "0.3.0");
        let picky = addon("y", vec![version("1.0.0", Some(">=9.0.0"))]);
        assert!(picky.offered(&launcher("1.3.7")).is_none());
    }

    #[test]
    fn finds_updates_for_installed_addons_only() {
        let catalog = vec![
            addon("a", vec![version("0.2.0", None), version("0.1.0", None)]),
            addon("b", vec![version("1.0.0", None)]),
            addon(
                "c",
                vec![version("2.0.0", Some(">=9.0.0")), version("1.0.0", None)],
            ),
            addon("d", vec![version("1.0.0", None)]),
        ];
        let installed = [
            manifest("a", "0.1.0"),
            manifest("b", "1.0.0"),
            manifest("c", "1.0.0"),
            manifest("e", "1.0.0"),
        ];
        let found = updates(&installed, &catalog, &launcher("1.3.7"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "a");
        assert_eq!(found[0].installed.to_string(), "0.1.0");
        assert_eq!(found[0].to.version.to_string(), "0.2.0");
        let newer = [manifest("a", "0.3.0")];
        assert!(updates(&newer, &catalog, &launcher("1.3.7")).is_empty());
    }

    #[test]
    fn rejects_downloads_that_do_not_match() {
        let dir = std::env::temp_dir().join(format!("swofty-catalog-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let to = dir.join("addon.zip");
        let mut v = version("1.0.0", None);
        v.sha256 = "39a3ab5f19d0e1e2a7d6ad4b1c4d68c0f5c6ff2e1ae4b8c5bbce7c5c2a8b3e6b".into();
        let wrong = save(&b"abc"[..], &v, &to, &|_| {}).unwrap_err();
        assert!(wrong.to_string().contains("SHA-256"), "{wrong}");
        v.sha256 = "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD".into();
        save(&b"abc"[..], &v, &to, &|_| {}).unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), b"abc");
        let long = save(&b"abcd"[..], &v, &to, &|_| {}).unwrap_err();
        assert!(long.to_string().contains("larger"), "{long}");
        v.size = 4;
        let short = save(&b"abc"[..], &v, &to, &|_| {}).unwrap_err();
        assert!(short.to_string().contains("downloaded 3 bytes"), "{short}");
        v.size = DOWNLOAD_LIMIT + 1;
        assert!(save(&b""[..], &v, &to, &|_| {}).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reads_the_registry_catalog() {
        let text = r#"{"addons":[{"id":"swofty.career-editor","name":"Career Mode Editor","author":"Swofty",
            "summary":"s","description":"","category":"tracks","kind":["native"],"homepage":null,
            "icon":null,"publishers":[{"id":"1","name":"swofty","avatar":null}],"downloads":12,
            "created_at":"2026-10-22T10:00:00Z","updated_at":"2026-10-23T01:00:00Z",
            "latest":{"version":"0.2.3","launcher":">=1.2.0","game":"F1 25","size":10,"sha256":"ab",
            "url":"http://x/d","published_at":"","changelog":"","native":true,"yanked":false},
            "versions":[{"version":"0.2.3","launcher":null,"game":"F1 25","size":10,"sha256":"ab",
            "url":"http://x/d","published_at":"","changelog":"- fix","native":true,"yanked":false}],
            "extra":1}]}"#;
        let addons = parse(text).unwrap();
        assert_eq!(addons[0].downloads, 12);
        assert_eq!(addons[0].category_label().as_deref(), Some("Tracks"));
        assert!(addons[0].native(&launcher("1.3.7")));
        assert_eq!(addons[0].listed().len(), 1);
        assert!(addons[0].matches("career"));
        assert!(!addons[0].matches("helmet"));
        assert!(parse("<html>").is_none());
    }
}
