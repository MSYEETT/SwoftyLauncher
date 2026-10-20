# Career Mode Editor

Edits a running F1 25 career from the launcher's **Career Editor** page, in My Team and Driver Career: cash, resource points, renown, sponsor loyalty, cost cap refunds, driver ratings, seat swaps, contract money, R&D, facilities, the calendar, points and session weather. Needs Swofty Launcher for F1 25 1.0.10 or later.

User guide: [Career Mode Editor](https://f1.swofty.net/using/career-editor).

## Layout

| Path | What |
|---|---|
| `native/` | The in-game plugin: the bridge (mailbox, snapshot, journal) and one module per area. Build with `native/build.sh`. |
| `launcher/` | The launcher plugin that draws the page. Build with `launcher/build.sh`; it writes `bin/`. |
| `protocol/` | Each module's ops, snapshot section and game bindings. [`PROTOCOL.md`](PROTOCOL.md) indexes them. |

Package with `tools/pack_addon.py --zip addons/career-editor <out>`.
