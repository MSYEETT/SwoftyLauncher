---
title: Tools
description: The Tools page shows the tools that make or edit Swofty addons as large cards in a row you scroll sideways.
---

# Tools

**Tools**, in the sidebar, shows the tools that make or edit Swofty addons. Each tool has a large card with a picture, its name, who made it, what it does and a button.

The cards sit in one row. Scroll it sideways with the mouse wheel or a trackpad, drag the scrollbar under it, or use the arrows at the top right. The row stops with a card lined up at its edge. The dots beside the arrows show which card you're on, and clicking a dot moves to that card.

| Button | What it does |
|---|---|
| **Get** | Opens the tool's download page in your browser |
| **Open** | Opens the tool: a launcher page, or a program that ships with the launcher |
| **Turn on** | Turns the tool's addon on in the active profile, when it's installed but off |

A link under the description, such as **Team Simplified**, opens the maker's page in your browser.

## The tools

| Tool | Made by | What it does | Get |
|---|---|---|---|
| Simplified Suite | Team Simplified | Open-source modding tools for F1 25: Simplified Studio makes `.serps` mods, and the suite adds the SERPs Launcher, a track installer, swappers, editors and a database merger. The [Simplified Importer](/using/mods) turns mods made with them into Swofty addons. | [GitHub](https://github.com/TeamSimplified/simplified-suite) |
| EGO GameDatabases Editor | PavelGames | A standalone editor for the F1 games' databases: drivers, teams, line-ups, tracks, the calendar, points and weather, and a key editor for any value. | [Overtake](https://www.overtake.gg/downloads/ego-gamedatabases-editor.84903/) |

The EGO GameDatabases Editor picture is PavelGames' screenshot from its Overtake page.

## Adding a tool

The list is `apps/launcher/assets/tools.json` in the launcher's repository. Pictures are files in `apps/launcher/assets/tools/`, and the launcher embeds every file in that folder. A picture is a PNG, JPEG or WebP file, shown at 16:9 and cropped to fill the card. Replacing a picture's file with another image of the same name changes the card's picture.

Each entry has:

| Field | Value |
|---|---|
| `name` | The tool's name |
| `maker` | Who made it |
| `about` | Two to four sentences on what it does |
| `image` | Optional. The picture's file name in `apps/launcher/assets/tools/` |
| `credit` | Optional. A short credit shown on the picture, such as `Screenshot: PavelGames` |
| `icon` | Optional. The icon shown when there's no picture: one of `database`, `folder-input`, `app-window`, `user-pen`, `users`, `package`, `table`, `map`, `palette`, `code` |
| `open` | Optional. `page:database`, `page:importer` or `page:addons` opens that page; `addon:<id>` opens the addon's first page; `program:<name>` starts `bin/<name>` |
| `get` | Optional. The download URL, used when `open` can't open anything |
| `more` | Optional. A link to the maker, as `{ "label": …, "url": … }` |
