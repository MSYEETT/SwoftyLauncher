# Log triage bot

`swofty-triage` diagnoses the reports that **Send Logs to Developers** posts to the developers' Discord channel. For each report it opens a thread on the report message and posts, as plain messages:

- the likely cause, with evidence from the logs and the source code;
- whether `main` already fixes it, and earlier reports with the same cause;
- a reply for the player;
- a prompt for a coding agent, when the fix needs a code change.

## How it works

1. The bot reads every message in the channel and keeps the ones the launcher's webhook posted (`TRIAGE_WEBHOOK_ID`) in the format from `packages/core/src/send_logs/webhook.rs`. A report sent in several parts waits until every part has arrived, or for 30 minutes.
2. It downloads the zip, unpacks it with path and size checks, and builds a briefing: the header, `summary.txt`, the file list, `addons.json`, `config.json`, each session's exit code and `[crash]` lines, the last lines of each log, and a crash signature.
3. It fetches bare mirrors of the source repositories and adds the release tag the player ran (`v<launcher version>`) and the commits on `main` since that tag.
4. Claude (`TRIAGE_MODEL`, adaptive thinking at `TRIAGE_EFFORT`) investigates with read-only tools: list, read and search the log bundle; list, read and search a repository at any ref; `git log` and `git show`; earlier diagnoses. It ends by calling `submit_diagnosis`. The tools cannot reach anything outside the bundle and the git object stores, and the process environment is not passed to git.
5. The bot posts the diagnosis in the thread and reacts on the report: 🔍 working, ✅ done, ⚠️ failed, ⏭️ skipped by a limit.

React 🔁 on a report to diagnose it again. State is in `$TRIAGE_DATA_DIR/triage.db`; each case's logs, zips and `diagnosis.json` are in `$TRIAGE_DATA_DIR/cases/<message id>/`. Logs and zips are deleted after 30 days.

## Configuration

Environment variables, read from `/etc/swofty-triage.env` by the service:

| Variable | Default | Meaning |
|---|---|---|
| `ANTHROPIC_API_KEY` | | Claude API key |
| `DISCORD_TOKEN` | | Bot token. The bot needs the Message Content intent, and View Channel, Read Message History, Add Reactions, Create Public Threads and Send Messages in Threads in the channel |
| `TRIAGE_CHANNEL_ID` | | Channel the webhook posts to |
| `TRIAGE_WEBHOOK_ID` | | Webhook id from `SWOFTY_LOG_WEBHOOK` (`/api/webhooks/<id>/<token>`) |
| `TRIAGE_REPOS` | `launcher=<SwoftyLauncher URL>` | Comma-separated `name=url`, `launcher` first. For a private repository over SSH, put its deploy key at `$TRIAGE_DATA_DIR/repos/<name>.key` |
| `TRIAGE_DATA_DIR` | `/var/lib/swofty-triage` | State, mirrors and cases |
| `TRIAGE_MODEL` | `claude-opus-5-5` | Model |
| `TRIAGE_EFFORT` | `high` | `low`, `medium`, `high`, `xhigh` or `max` |
| `TRIAGE_MAX_TURNS` | `40` | Model turns per report |
| `TRIAGE_MAX_COST_USD` | `4` | Spend per report at which the model is told to submit; the run stops at 1.5 times this |
| `TRIAGE_DAILY_LIMIT` | `40` | Reports diagnosed per 24 hours |
| `TRIAGE_PER_NAME_HOURLY` | `3` | Reports diagnosed per Discord name per hour |
| `TRIAGE_BACKFILL_FROM` | | On a first start, also diagnose reports from this message id onwards |

## Deploy

On a Debian or Ubuntu host or container, as root:

```sh
curl -fsSL https://raw.githubusercontent.com/Swofty-Developments/SwoftyLauncher/main/triage/deploy/install.sh | sh
```

The first run installs the service and writes `/etc/swofty-triage.env` from `deploy/swofty-triage.env.example`. Fill it in, then run `systemctl enable --now swofty-triage`. Run the script again to update; it takes a git ref as its argument (default `main`). Logs: `journalctl -u swofty-triage -f`.

## Development

```sh
cd triage
uv sync
uv run pytest
uv run ruff check . && uv run ruff format --check .
```

Diagnose a report locally without Discord. The reply prints to stdout, and the case goes under `TRIAGE_DATA_DIR`:

```sh
export ANTHROPIC_API_KEY=... TRIAGE_DATA_DIR=/tmp/triage
uv run swofty-triage diagnose swofty-logs-<name>-<time>.zip --message-json message.json
```

`--message-json` takes a Discord message object as JSON; `--content` takes the message text instead.
