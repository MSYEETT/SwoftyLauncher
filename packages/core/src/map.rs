use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ops::DbOp;
use crate::{Error, Result, io_err};

pub const MANIFEST_FILE: &str = "manifest.json";
const ART_DIR: &str = ".art";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackPackage {
    pub track_code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_name: Option<String>,
    pub erp: String,
    #[serde(default)]
    pub activation: Activation,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ers: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copy_directories: Vec<CopyDirectory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<TrackUi>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Activation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy_from_track: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy_slot: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<Override>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub track_overrides: Vec<Override>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Override {
    pub path: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub datasets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopyDirectory {
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackUi {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimap_file: Option<String>,
}

impl TrackPackage {
    pub fn parse(json: &[u8]) -> Result<Self> {
        let p: Self = serde_json::from_slice(json).map_err(|e| Error::Map(e.to_string()))?;
        p.validate()?;
        Ok(p)
    }

    pub fn validate(&self) -> Result<()> {
        let code_ok = !self.track_code.is_empty()
            && self
                .track_code
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        if !code_ok {
            return Err(Error::Map(format!(
                "track_code {:?} is not a plain identifier",
                self.track_code
            )));
        }
        let paths = std::iter::once(&self.erp).chain(self.copy_directories.iter().map(|c| &c.source));
        let ui = self
            .ui
            .iter()
            .flat_map(|u| u.images_file.iter().chain(u.minimap_file.iter()));
        if let Some(p) = paths
            .chain(ui)
            .find(|p| !crate::manifest::relative_inside(p.trim_end_matches('/')))
        {
            return Err(Error::Map(format!(
                "{p} must be a relative path inside the package"
            )));
        }
        Ok(())
    }

    pub fn display_name(&self) -> &str {
        self.track_name.as_deref().unwrap_or(&self.track_code)
    }

    pub fn db_ops(&self) -> Vec<DbOp> {
        let track = |o: &Override| DbOp::Set {
            select: format!("tracks.Track[instanceName={}]", self.track_code),
            field: o.path.clone(),
            value: Some(o.value.clone()),
            value_from: None,
            datasets: o.datasets.clone(),
            create: false,
            tag: None,
        };
        self.activation.track_overrides.iter().map(track).collect()
    }
}

pub fn read_zip_manifest(zip_path: &Path) -> Result<TrackPackage> {
    let file = std::fs::File::open(zip_path).map_err(io_err(zip_path))?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut json = Vec::new();
    zip.by_name(MANIFEST_FILE)
        .map_err(|_| {
            Error::Map(format!(
                "{} has no {MANIFEST_FILE} at its root",
                zip_path.display()
            ))
        })?
        .read_to_end(&mut json)
        .map_err(io_err(zip_path))?;
    let pkg = TrackPackage::parse(&json)?;
    if zip.by_name(&pkg.erp).is_err() {
        return Err(Error::Map(format!(
            "{} is missing {}",
            zip_path.display(),
            pkg.erp
        )));
    }
    Ok(pkg)
}

pub fn unwrap_nested(zip_path: &Path, scratch: &Path) -> Result<Option<PathBuf>> {
    let file = std::fs::File::open(zip_path).map_err(io_err(zip_path))?;
    let mut zip = zip::ZipArchive::new(file)?;
    if zip.index_for_name(MANIFEST_FILE).is_some() {
        return Ok(None);
    }
    let inner: Vec<(usize, String)> = (0..zip.len())
        .filter_map(|i| Some((i, zip.name_for_index(i)?.ok()?.into_owned())))
        .filter(|(_, n)| n.to_ascii_lowercase().ends_with(".zip"))
        .collect();
    let [(index, name)] = inner.as_slice() else {
        return Ok(None);
    };
    let dest = scratch.join(Path::new(name).file_name().unwrap_or("package.zip".as_ref()));
    std::fs::create_dir_all(scratch).map_err(io_err(scratch))?;
    let mut out = std::fs::File::create(&dest).map_err(io_err(&dest))?;
    std::io::copy(&mut zip.by_index(*index)?, &mut out).map_err(io_err(&dest))?;
    let nested = std::fs::File::open(&dest).map_err(io_err(&dest))?;
    let found = zip::ZipArchive::new(nested).is_ok_and(|z| z.index_for_name(MANIFEST_FILE).is_some());
    Ok(found.then_some(dest))
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapArt {
    pub flag: Option<PathBuf>,
    pub preview: Option<PathBuf>,
}

pub fn art(dir: &Path, pkg: &TrackPackage) -> MapArt {
    let out = dir.join(ART_DIR);
    if !out.is_dir() && std::fs::create_dir_all(&out).is_ok() {
        extract_art(dir, pkg, &out).ok();
    }
    let file = |name: &str| Some(out.join(name)).filter(|p| p.is_file());
    MapArt {
        flag: file("flag.png"),
        preview: file("preview.png"),
    }
}

fn extract_art(dir: &Path, pkg: &TrackPackage, out: &Path) -> Result<()> {
    let Some(images) = pkg.ui.as_ref().and_then(|u| u.images_file.as_ref()) else {
        return Ok(());
    };
    let images: serde_json::Value = crate::read_json(&dir.join(images))?;
    let wanted: Vec<(String, &str)> = [("flag_rectangle", "flag.png"), ("preview_small", "preview.png")]
        .into_iter()
        .filter_map(|(key, file)| Some((format!("{}.image", images["resources"][key].as_str()?), file)))
        .collect();
    let found = swofty_formats::erp::Erp::read_matching(&dir.join(&pkg.erp), |name| {
        wanted.iter().any(|(w, _)| w == name)
    })?;
    for res in &found {
        if let Some((_, file)) = wanted.iter().find(|(w, _)| *w == res.name) {
            write_png(&out.join(file), &swofty_formats::texture::decode(res)?)?;
        }
    }
    Ok(())
}

fn write_png(path: &Path, image: &swofty_formats::texture::Image) -> Result<()> {
    let file = std::fs::File::create(path).map_err(io_err(path))?;
    let mut png = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()
        .and_then(|mut w| w.write_image_data(&image.rgba))
        .map_err(|e| Error::Map(format!("{}: {e}", path.display())))
}

pub fn extract_zip(zip_path: &Path, dest: &Path) -> Result<TrackPackage> {
    let pkg = read_zip_manifest(zip_path)?;
    unzip(zip_path, dest, Error::Map)?;
    Ok(pkg)
}

pub(crate) fn unzip(zip_path: &Path, dest: &Path, err: fn(String) -> Error) -> Result<()> {
    let file = std::fs::File::open(zip_path).map_err(io_err(zip_path))?;
    let mut zip = zip::ZipArchive::new(file)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(err(format!(
                "{}: unsafe entry {}",
                zip_path.display(),
                String::from_utf8_lossy(entry.name_raw())
            )));
        };
        let out: PathBuf = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(io_err(&out))?;
            continue;
        }
        if let Some(dir) = out.parent() {
            std::fs::create_dir_all(dir).map_err(io_err(dir))?;
        }
        let mut f = std::fs::File::create(&out).map_err(io_err(&out))?;
        std::io::copy(&mut entry, &mut f).map_err(io_err(&out))?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub const OKAYAMA_LIKE: &str = r#"{
      "track_code": "testring", "track_name": "Test Ring", "version": "0.1.0", "author_name": "tester",
      "erp": "testring.erp",
      "activation": {
        "copy_from_track": "melbourne", "copy_slot": "melbourne",
        "overrides": [{ "datasets": ["f1_2025"], "path": "TyreData_0.compound_harder", "value": "C3" }],
        "track_overrides": [{ "path": "allow_night_start", "value": "False" }]
      },
      "ers": { "HotLapDeployFactor": 0.7 },
      "copy_directories": [{ "source": "mipmaps/", "destination": "asset_groups/environment_package/tracks/testring/mipmaps/" }],
      "ui": { "schema_version": 1, "images_file": "ui/images.json", "minimap_file": "ui/minimap.json" }
    }"#;

    pub fn write_zip(path: &Path, manifest: &str, with_erp: bool) {
        let f = std::fs::File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file(MANIFEST_FILE, opts).unwrap();
        std::io::Write::write_all(&mut z, manifest.as_bytes()).unwrap();
        if with_erp {
            z.start_file("testring.erp", opts).unwrap();
            std::io::Write::write_all(&mut z, b"ERPK placeholder").unwrap();
        }
        z.start_file("ui/images.json", opts).unwrap();
        std::io::Write::write_all(&mut z, b"{}").unwrap();
        z.finish().unwrap();
    }

    #[test]
    fn parses_ego_manifest() {
        let p = TrackPackage::parse(OKAYAMA_LIKE.as_bytes()).unwrap();
        assert_eq!(p.display_name(), "Test Ring");
        assert_eq!(p.activation.overrides[0].datasets, ["f1_2025"]);
        assert_eq!(p.db_ops().len(), 1);
    }

    #[test]
    fn rejects_escaping_paths() {
        let bad = OKAYAMA_LIKE.replace("testring.erp", "../x.erp");
        assert!(TrackPackage::parse(bad.as_bytes()).is_err());
    }

    #[test]
    fn extracts_flag_art_from_the_erp() {
        use swofty_formats::erp::{Erp, Fragment, HEADER_LEN, MAGIC, Resource};
        let dir = std::env::temp_dir().join(format!("swofty-art-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("ui")).unwrap();
        let pkg = TrackPackage::parse(OKAYAMA_LIKE.as_bytes()).unwrap();
        std::fs::write(
            dir.join("ui/images.json"),
            r#"{ "resources": { "flag_rectangle": "eaid://ui/flag.tif" } }"#,
        )
        .unwrap();
        let fragment = |raw: Vec<u8>| Fragment {
            name: *b"temp",
            offset: 0,
            size: raw.len() as u64,
            flags: 0,
            compression: 0,
            packed: raw.len() as u64,
            raw,
        };
        let mut head = vec![0; 40];
        for (at, v) in [(8, 0x46u32), (12, 4), (16, 4)] {
            head[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        let block = vec![
            0x40, 0x20, 0x10, 0x02, 0xf9, 0xff, 0xff, 0xff, 0x01, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut header = [0; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        let mut erp = Erp {
            header,
            version: 3,
            resources: vec![Resource {
                name: "eaid://ui/flag.tif.image".into(),
                kind: [0; 16],
                unk: vec![0; 4],
                fragments: vec![fragment(head), fragment(block)],
                hash: [0; 16],
            }],
        };
        std::fs::write(dir.join(&pkg.erp), erp.to_bytes()).unwrap();
        let art = art(&dir, &pkg);
        let flag = std::fs::read(art.flag.unwrap()).unwrap();
        assert!(flag.starts_with(b"\x89PNG"));
        assert_eq!(art.preview, None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn zip_needs_its_erp() {
        let dir = std::env::temp_dir().join(format!("swofty-map-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("good.zip");
        let bad = dir.join("bad.zip");
        write_zip(&good, OKAYAMA_LIKE, true);
        write_zip(&bad, OKAYAMA_LIKE, false);
        assert_eq!(read_zip_manifest(&good).unwrap().track_code, "testring");
        assert!(read_zip_manifest(&bad).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
