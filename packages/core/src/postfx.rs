//! Post-process passes: HLSL compute shaders compiled with dxc against `swofty/postfx.hlsli`.

use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

/// The pass contract, on the include path of every pass as `swofty/postfx.hlsli`.
pub const HEADER: &str = include_str!("../../sdk/include/swofty/postfx.hlsli");
/// The shader model passes compile to.
pub const PROFILE: &str = "cs_6_0";

/// The dxc the build compiles with: `SWOFTY_DXC`, then `dxc` next to the launcher, then on `PATH`.
pub fn dxc() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SWOFTY_DXC").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let exe = if cfg!(windows) { "dxc.exe" } else { "dxc" };
    if let Some(beside) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("dxc").join(exe)))
        .filter(|p| p.is_file())
    {
        return Some(beside);
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(exe))
        .find(|p| p.is_file())
}

fn key(source: &Path) -> Result<String, String> {
    let mut h = Sha256::new();
    h.update(PROFILE);
    h.update(HEADER);
    let dir = source.parent().unwrap_or(Path::new("."));
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("hlsl") || x.eq_ignore_ascii_case("hlsli"))
        })
        .collect();
    files.sort();
    for f in files {
        h.update(f.file_name().unwrap_or_default().to_string_lossy().as_bytes());
        h.update(std::fs::read(&f).map_err(|e| format!("{}: {e}", f.display()))?);
    }
    h.update(
        source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .as_bytes(),
    );
    Ok(crate::baseline::hex(&h.finalize()))
}

/// Compiles `source` to DXIL, reusing `cache` when the source, the files beside it and the
/// contract are unchanged.
pub fn compile(source: &Path, cache: &Path) -> Result<Vec<u8>, String> {
    let key = key(source)?;
    let cached = cache.join(format!("{key}.dxil"));
    if let Ok(bytes) = std::fs::read(&cached)
        && bytes.starts_with(b"DXBC")
    {
        return Ok(bytes);
    }
    let dxc = dxc().ok_or("dxc was not found (set SWOFTY_DXC, or put dxc on PATH)")?;
    let include = cache.join("include");
    let header = include.join("swofty").join("postfx.hlsli");
    std::fs::create_dir_all(header.parent().expect("has parent"))
        .map_err(|e| format!("{}: {e}", include.display()))?;
    std::fs::write(&header, HEADER).map_err(|e| format!("{}: {e}", header.display()))?;
    let out = cache.join(format!("{key}.tmp"));
    let run = Command::new(&dxc)
        .args(["-nologo", "-T", PROFILE, "-E", "main", "-HV", "2021", "-O3", "-I"])
        .arg(&include)
        .arg("-Fo")
        .arg(&out)
        .arg(source)
        .output()
        .map_err(|e| format!("{}: {e}", dxc.display()))?;
    if !run.status.success() {
        let _ = std::fs::remove_file(&out);
        let text = String::from_utf8_lossy(&run.stderr);
        let first = text.lines().find(|l| l.contains("error")).unwrap_or(text.trim());
        return Err(format!("dxc: {}", first.trim()));
    }
    let bytes = std::fs::read(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    if !bytes.starts_with(b"DXBC") {
        return Err("dxc wrote no DXIL container".into());
    }
    std::fs::rename(&out, &cached).map_err(|e| format!("{}: {e}", cached.display()))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_template_pass_compiles_when_dxc_is_at_hand() {
        if dxc().is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-postfx-{}", std::process::id()));
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/postfx-pass/fx/vignette.hlsl");
        let dxil = compile(&source, &root).unwrap();
        assert!(dxil.starts_with(b"DXBC"));
        assert_ne!(&dxil[4..20], &[0u8; 16], "the validator signs the container");
        assert_eq!(compile(&source, &root).unwrap(), dxil);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_broken_pass_reports_the_dxc_error() {
        if dxc().is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-postfx-bad-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("bad.hlsl");
        std::fs::write(
            &source,
            "#include \"swofty/postfx.hlsli\"\nfloat4 effect(float2 uv, uint2 p) { return nope; }\n",
        )
        .unwrap();
        let err = compile(&source, &root.join("cache")).unwrap_err();
        assert!(err.starts_with("dxc: ") && err.contains("nope"), "{err}");
        std::fs::remove_dir_all(&root).ok();
    }
}
