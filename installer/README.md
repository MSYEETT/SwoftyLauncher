# Swofty Launcher Installer

A small window that installs the latest Swofty Launcher release and opens it.

| Platform | Download |
|---|---|
| Windows | [SwoftyLauncherInstaller.exe](https://github.com/Swofty-Developments/SwoftyLauncher/raw/main/installer/SwoftyLauncherInstaller.exe) |
| Linux and Steam Deck | [SwoftyLauncherInstaller-linux.zip](https://github.com/Swofty-Developments/SwoftyLauncher/raw/main/installer/SwoftyLauncherInstaller-linux.zip), then extract it |
| Both | [SwoftyLauncherInstaller.zip](https://github.com/Swofty-Developments/SwoftyLauncher/raw/main/installer/SwoftyLauncherInstaller.zip) |

Run `SwoftyLauncherInstaller.exe` on Windows, or `SwoftyLauncherInstaller` on Linux and Steam Deck, choose a folder and select **Install**.

The installer downloads the latest release from GitHub, checks it against the release's `SHA256SUMS`, extracts it into the folder, adds a Start menu or app menu entry and, if ticked, a desktop shortcut, then opens the launcher. Running it again finds the existing install, in the default folder, behind the Start menu, app menu or desktop shortcut, or from a running launcher, and updates it in place. Choosing another folder offers to move that install there: the installer moves the release files, checks them, deletes the old folder once it's empty and points the shortcuts at the new folder. The launcher's data folder stays where it is. If the launcher is open from the folder it installs to, the installer offers to close it first, and while F1 25 is running it doesn't install. After the first install the launcher updates itself.

Default folders:

| Platform | Folder |
|---|---|
| Windows | `%LOCALAPPDATA%\Programs\SwoftyLauncher` |
| Linux | `~/SwoftyLauncher` |

## Building

```bash
cargo build --release -p swofty-launcher-installer
```

CI builds both executables on every push (`Installer` job, artifacts `installer-Linux` and `installer-Windows`). The files in this folder are those artifacts, renamed: `SwoftyLauncherInstaller.exe` is the Windows build, `SwoftyLauncherInstaller-linux.zip` holds the Linux build as `SwoftyLauncherInstaller`, and `SwoftyLauncherInstaller.zip` holds both.
