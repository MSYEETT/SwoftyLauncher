---
title: Send your logs to the developers
description: Send the launcher and game logs to the Swofty Launcher developers in one click, with your Discord username and what went wrong.
---

# Send your logs to the developers

When something goes wrong, you can send your logs to the developers straight from the launcher.

1. Select **Send Logs to Developers** at the bottom of the sidebar, above **ACTIVE PROFILE**.
2. Enter your Discord username. The developers use it to reply to you.
3. Optionally, describe what went wrong under **What went wrong?** (up to 1000 characters).
4. Select **Send**.

The dialog shows the progress, then **Sent — thanks!**. If the logs could not be sent, it shows the reason, such as no connection to Discord or the HTTP status Discord returned, and **Try again**. The launcher remembers your username for next time.

## What gets sent

The logs go to the developers' Discord as one zip file, `swofty-logs-<username>-<time>.zip`. A large zip goes in several parts.

| In the zip | Contents |
|---|---|
| `summary.txt` | Your username and message, the launcher version, your operating system, CPU and memory, and the game's version and folder |
| `state/*.log` | The launcher's own logs, such as `launcher-run.log` and `wrapper-run.log` |
| `sessions/<time>/` | The logs and small files of your three latest game sessions, such as `loader.log`, `env.txt` and `exit_code` |
| `addons/<id>/helper.log` | Each addon helper's log |
| `addons.json` | The id and version of every installed addon, and the addons in each profile |
| `config.json` | The launcher's settings |
| `game/career-editor/snapshot.json` | The Career Mode Editor's snapshot, when the game folder has one |

The logs can include folder paths with your computer's user name, your hardware, and your installed mods and profiles. Passwords, tokens and keys in `config.json` and in each session's `env.txt` are replaced with `[redacted]`. Each file is cut to its last 5 MB, and the whole zip holds at most 40 MB before compression.

## From the command line

```sh
swofty logs send --name <your Discord username> --message "What went wrong"
```

To see what would be sent without sending it, add `--dry-run`. See [`swofty logs send`](/reference/cli#swofty-logs-send).

Builds from source don't include the developers' Discord address. There, the button reads **Sending logs isn't available in this build**, and `swofty logs send` only works with `--dry-run`.
