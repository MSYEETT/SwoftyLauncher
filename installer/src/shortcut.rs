use std::path::{Path, PathBuf};

const NAME: &str = "Swofty Launcher";

#[cfg(windows)]
fn file_name() -> String {
    format!("{NAME}.lnk")
}

#[cfg(not(windows))]
fn file_name() -> String {
    "net.swofty.ModLauncher.desktop".into()
}

#[cfg(windows)]
fn menu_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(r"Microsoft\Windows\Start Menu\Programs"))
}

#[cfg(not(windows))]
fn menu_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("applications"))
}

#[cfg(windows)]
fn desktop_dir() -> Option<PathBuf> {
    dirs::desktop_dir()
}

#[cfg(not(windows))]
fn desktop_dir() -> Option<PathBuf> {
    let fallback = || dirs::home_dir().map(|h| h.join("Desktop")).filter(|d| d.is_dir());
    dirs::desktop_dir().or_else(fallback)
}

pub fn files() -> Vec<PathBuf> {
    menu_dir()
        .into_iter()
        .chain(desktop_dir())
        .map(|d| d.join(file_name()))
        .collect()
}

#[cfg(windows)]
pub fn create(root: &Path, exe: &Path, desktop: bool) {
    let link = |dir: &Path| -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let mut link = mslnk::ShellLink::new(exe).map_err(|e| e.to_string())?;
        link.set_working_dir(Some(root.display().to_string()));
        link.create_lnk(dir.join(file_name())).map_err(|e| e.to_string())
    };
    if let Some(dir) = menu_dir() {
        link(&dir).ok();
    }
    if desktop && let Some(dir) = desktop_dir() {
        link(&dir).ok();
    }
}

#[cfg(not(windows))]
pub fn create(root: &Path, exe: &Path, desktop: bool) {
    use std::os::unix::fs::PermissionsExt;

    let entry = entry(root, exe);
    let write = |dir: &Path| -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let file = dir.join(file_name());
        std::fs::write(&file, &entry)?;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))
    };
    if let Some(dir) = menu_dir() {
        write(&dir).ok();
    }
    if desktop && let Some(dir) = desktop_dir() {
        write(&dir).ok();
    }
}

#[cfg(not(windows))]
pub fn entry(root: &Path, exe: &Path) -> String {
    let icon = root.join("share/icons/hicolor/256x256/apps/net.swofty.ModLauncher.png");
    let icon = if icon.is_file() {
        icon.display().to_string()
    } else {
        "net.swofty.ModLauncher".into()
    };
    format!(
        "[Desktop Entry]\nType=Application\nName={NAME} for F1 25\nComment=Mods and tracks for F1 25\n\
         Exec={}\nPath={}\nIcon={icon}\nTerminal=false\nCategories=Game;\nStartupWMClass=net.swofty.ModLauncher\n",
        quote(&exe.display().to_string()),
        root.display(),
    )
}

#[cfg(not(windows))]
fn quote(arg: &str) -> String {
    let mut out = String::from("\"");
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '%' => out.push_str("%%"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
