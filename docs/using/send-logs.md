---
title: Send your logs to the developers
description: Send the launcher and game logs to the Swofty Launcher developers in one click, with your Discord username and what went wrong.
---

# Send your logs to the developers

When something goes wrong, you can send your logs to the developers straight from the launcher.

1. Select **Send Logs to Developers** at the bottom of the sidebar, above **ACTIVE PROFILE**.
2. Enter your Discord username. The developers use it to reply to you.
3. Under **What went wrong?**, describe what happened and what you were doing when it went wrong, in 10 to 1000 characters.
4. Select **Send**. Until both fields are filled in, **Send** is greyed out and a hint under each field says what's missing.

When your data folder has no launch log and no game session, the dialog warns: **We don't see a game launch in your logs yet. Press Play and reproduce the problem first, then send logs.** You can still send, and `summary.txt` then says that no game session was found.

The dialog shows the progress, then **Sent — thanks!**. If the logs could not be sent, it shows the reason, such as no connection to Discord or the HTTP status Discord returned, and **Try again**. The launcher remembers your username for next time.

## Send the zip yourself

The launcher keeps a copy of the zip in `$DATA/logs/`, the newest 10. To send it to someone else, such as the author of a track or a mod:

1. Send your logs as above. The zip is kept whether or not they were sent.
2. Under the result, select **Click here to open them in File Explorer** (on Linux, **in your file manager**). The folder opens with the zip selected.
3. Drag the zip into Discord or a message.

On Linux, the launcher asks your file manager to select the zip over D-Bus (`org.freedesktop.FileManager1`). Without one that answers, it opens the `logs` folder.

## What gets sent

The logs go to the developers' Discord as one zip file, `swofty-logs-<username>-<time>.zip`. A large zip goes in several parts.

| In the zip | Contents |
|---|---|
| `summary.txt` | Your username and message, the launcher version, your operating system, CPU and memory, the game's version and folder, a line when no launch log or game session was found, a summary of the latest crash report, the summary of the latest performance report, and the active profile's addons by name: those on in load order, then the rest, each with its version, id, where it came from (built in, DB Editor, imported from which mod file, or installed) and the settings changed from their defaults |
| `crash/crash-<time>.txt`, `crash/crash-<time>.dmp` | The newest crash report the game wrote and its minidump, when there is one. The minidump is left out, noted in the summary, when it is too big for the zip |
| `game/loader.log`, `game/perf.log` | The loader log and the performance report of the game that's running |
| `state/*.log` | The launcher's own logs, such as `launcher-run.log` and `wrapper-run.log` |
| `sessions/<time>/` | The logs and small files of your three latest game sessions, such as `loader.log`, `perf.log`, `env.txt` and `exit_code` |
| `addons/<id>/helper.log` | Each addon helper's log |
| `addons.json` | The id, name, version, source, category, types and origin of every installed addon, and each profile's addons with their names and settings |
| `conflicts.json` | From the active profile's last build: each pair of addons that change the same thing, which one wins, and the records, resources, files and text they both change |
| `config.json` | The launcher's settings |
| `game/career-editor/snapshot.json` | The Career Mode Editor's snapshot, when the game folder has one |

The logs can include folder paths with your computer's user name, your hardware, and your installed mods and profiles. Passwords, tokens and keys in `config.json`, in addon settings and in each session's `env.txt` are replaced with `[redacted]`. Each file is cut to its last 5 MB, and the whole zip holds at most 40 MB before compression.

A bot in the developers' Discord reads each report with Anthropic's Claude API to suggest a cause, so the zip's contents are also sent to Anthropic.

## From the command line

```sh
swofty logs send --name <your Discord username> --message "What happened and what you were doing"
```

`--message` takes 10 to 1000 characters, and the command stops without it unless you add `--dry-run`. When your data folder has no launch log and no game session, the command warns first. The command keeps the zip in `$DATA/logs/` too and prints its path. To write the zip and list what it holds without sending it, add `--dry-run`. See [`swofty logs send`](/reference/cli#swofty-logs-send).

Builds from source don't include the developers' Discord address. There, the button reads **Save Logs**: it saves the zip to `$DATA/logs/` without sending it, then offers the same link to the zip. `swofty logs send` only works with `--dry-run`.
