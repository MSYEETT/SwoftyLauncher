---
title: Career Mode Editor
description: Edit a running F1 25 career from the launcher, in My Team and Driver Career.
---

# Career Mode Editor

The Career Mode Editor addon adds a **Career Editor** page to the launcher. It edits the career that is loaded in F1 25 while the game runs, in My Team and in Driver Career. It needs Swofty Launcher for F1 25 1.0.10 or later.

## Install

1. Download `swofty.career-editor.zip` from the [1.0.10 release](https://github.com/Swofty-Developments/SwoftyLauncher/releases/tag/v1.0.10).
2. Drag it onto the launcher window, or follow [Install an addon](/using/addons#install-an-addon).
3. Turn it on in your profile and start the game.

## Use it

Load a career and go to Team HQ. The **Career Editor** page shows the game, the career mode, the season and round, and whether edits can apply now. Its tabs:

| Tab | Edits |
|---|---|
| Overview | Status, recent edits and backups |
| Team | Cash, resource points, renown, sponsor loyalty, cost cap refunds and facilities (My Team); the driver's resource points (Driver Career) |
| Drivers | Ratings, seat swaps and contract money |
| R&D | Complete, fit, unlock, refund or reset upgrades |
| Season | Calendar rounds and points adjustments |
| Weather | Weather for each session of a weekend, and the weather seed |
| History | Every edit with its before and after values, undo, and save backups |

Edits queue while a session is loading, time is advancing or the game is in a menu transition, and apply when Team HQ is idle. Weather edits apply at any time in a loaded career. Screens that are open when an edit applies show it the next time they open.

The game saves the edited career at its next save. The editor backs up your saves before the first edit of each game session; **History** restores a backup while the game is closed.

Edits marked **Unsafe** write a value directly instead of through the game's own path. Contract money is one.
