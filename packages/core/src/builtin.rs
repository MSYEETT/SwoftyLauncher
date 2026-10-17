use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::install::PROXY;
use crate::manifest::AddonManifest;
use crate::{Result, io_err};

pub use swofty_addon::CORE_ID;
pub const CORE_MANIFEST: &str = include_str!("../../../addons/swoftymod/swofty.json");
pub const CORE_DLL: &str = "swofty/addons/core/swofty_core.dll";
pub const LOADER_ENV: &str = "SWOFTY_LOADER_DIR";

pub fn core_manifest() -> AddonManifest {
    AddonManifest::parse(CORE_MANIFEST).expect("the built-in manifest is valid")
}

pub fn loader_dir() -> Option<PathBuf> {
    let bin = std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.to_path_buf()));
    [
        std::env::var_os(LOADER_ENV).map(PathBuf::from),
        bin.as_ref().and_then(|b| Some(b.parent()?.join("loader"))),
        bin.map(|b| b.join("loader")),
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../native/dist")),
    ]
    .into_iter()
    .flatten()
    .find(|d| is_loader(d))
}

pub fn is_loader(dir: &Path) -> bool {
    dir.join(PROXY).is_file() && dir.join(CORE_DLL).is_file()
}

pub fn loader_files(dir: &Path) -> Result<BTreeMap<String, PathBuf>> {
    fn walk(dir: &Path, prefix: &str, out: &mut BTreeMap<String, PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir).map_err(io_err(dir))? {
            let path = entry.map_err(io_err(dir))?.path();
            let name = path.file_name().expect("has a name").to_string_lossy();
            let rel = format!("{prefix}{name}");
            if path.is_dir() {
                walk(&path, &format!("{rel}/"), out)?;
            } else {
                out.insert(rel, path);
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(dir, "", &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_manifest_is_valid() {
        let m = core_manifest();
        assert_eq!(m.id, CORE_ID);
        assert!(m.is_builtin());
    }

    #[test]
    fn lists_every_file_of_the_loader() {
        let dir = std::env::temp_dir().join(format!("swofty-loader-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let core = dir.join("swofty/addons/core");
        std::fs::create_dir_all(&core).unwrap();
        for f in [
            "swofty_core.dll",
            "swofty_gamelog.dll",
            "bindings.json",
            "symbols.tsv",
            "log_channels.tsv",
        ] {
            std::fs::write(core.join(f), f).unwrap();
        }
        assert!(!is_loader(&dir));
        std::fs::write(dir.join(PROXY), "proxy").unwrap();
        assert!(is_loader(&dir));
        let files = loader_files(&dir).unwrap();
        assert_eq!(
            files.keys().collect::<Vec<_>>(),
            [
                "swofty/addons/core/bindings.json",
                "swofty/addons/core/log_channels.tsv",
                "swofty/addons/core/swofty_core.dll",
                "swofty/addons/core/swofty_gamelog.dll",
                "swofty/addons/core/symbols.tsv",
                "version.dll",
            ]
        );
        assert_eq!(files[CORE_DLL], core.join("swofty_core.dll"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
