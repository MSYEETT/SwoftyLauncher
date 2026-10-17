# Custom Grid Size

Choose how many cars race in F1 2026 Grand Prix sessions.

## Setting

| Setting | Values | Default |
|---|---|---|
| `cars` | 22 to 30, in steps of 2 | 26 |

Change it in the launcher or with `swofty settings set swofty.custom-grid-size cars <n>`. The next launch or `swofty build` applies it.

## What each size changes

22 leaves the game as shipped. From 24 cars up:

- The F1 2026 world loads the My Team world variant on every track. That world has 24 grid slots, 12 garages and 12 pit boxes.
- Each step of 2 adds one team. Its team and car rows are complete copies of a real 2026 team, with their own label, code, colours and stat id.
- The `f1_2026` grid list holds `cars` entrants. `max_entrants` is `cars` for `f1_26` and `f1_generic_2026`, and `cars + 2` for `f1_custom_team_2026`, which the Grand Prix uses.
- The team, vehicle and grid row counts include the new rows.

| Team | Copied from | Car | Drivers | Livery preset | Colours |
|---|---|---|---|---|---|
| T12 | Williams | `williams_2026` | Jack Tremblay #66, Rashid Nair #65 | `jack_tremblay_2026` | teal, navy |
| T13 | Haas | `haas_2026` | Martin Giles #70, Alex Murray #40 | `martin_giles_2026` | orange, black |
| T14 | Aston Martin | `force_india_2026` | Lucas Roth #95, Igor Correia #79 | `lucas_roth_2026` | purple, dark blue |
| T15 | Audi | `sauber_2026` | Sophie Levasseur #53, Jonas Schiffer #76 | `sophie_levasseur_2026` | magenta, grey |

From 26 cars up, each track's My Team world gets grid slots 25 onwards, garages and pit boxes 13 onwards, and the garage dressing, pit-wall crew and pit boards for each new garage. New garages go in the next bays after garage 12. When a bay there fails a check, the rest go in the bays before garage 1.

## Room per track

A track holds a size when every new pit box sits on the pit lane, inside the speed limit zone with 8 m to spare and clear of the other boxes, and every new grid slot sits on the track clear of the other slots. From 27 cars up, each new pit box must also be 8 m from the safety car's parking spot, and each new grid slot must turn less than 10 degrees from slot 24 and sit at least 10 m past the last corner.

| Track | Most cars | Garages 13, 14, 15 | Limit |
|---|---|---|---|
| Abu Dhabi | 26 | after 12 | grid slot 27 would be 1 m past the last corner |
| Austria | 30 | after 12, after 12, before 1 | |
| Austria (reverse) | 28 | after 12, after 12 | grid slot 29 would be 6 m past the last corner |
| Bahrain | 30 | after 12 (the three closed spare bays) | |
| Baku | 30 | after 12, before 1, before 1 | |
| Brazil | 30 | after 12, before 1, before 1 | |
| Catalunya | 30 | after 12, before 1, before 1 | |
| Hungaroring | 28 | after 12, after 12 | grid slot 30 would be on a bend |
| Imola | 26 | after 12 | grid slot 27 would be inside the last corner |
| Jeddah | 30 | after 12, before 1, before 1 | |
| Las Vegas | 30 | after 12, after 12, before 1 | |
| Losail | 30 | after 12 | |
| Madrid | 30 | after 12, after 12, before 1 | |
| Melbourne | 30 | after 12, after 12, before 1 | |
| Mexico | 30 | after 12, after 12, before 1 | |
| Miami | 28 | after 12, before 1 | grid slot 29 would be on a bend |
| Monaco | 28 | after 12, after 12 | grid slot 29 would be 6 m past the last corner |
| Montreal | 30 | after 12 | |
| Monza | 30 | after 12 | |
| Shanghai | 30 | after 12, after 12, before 1 | |
| Silverstone | 28 | after 12, after 12 | grid slot 29 would be on a bend |
| Silverstone (reverse) | 30 | after 12, before 1, before 1 | |
| Singapore | 26 | after 12 | grid slot 28 would be 6 m past the last corner |
| Spa-Francorchamps | 30 | after 12, before 1, before 1 | |
| Suzuka | 30 | after 12 | |
| Texas | 30 | after 12 | |
| Zandvoort | 30 | after 12 | |
| Zandvoort (reverse) | 28 | after 12, before 1 | grid slot 30 would be on a bend |

A track with less room than the setting keeps its most cars' worth of slots, garages and boxes, and the build prints a warning naming it. A race there leaves the extra cars without a grid slot or garage.

An enabled map gets grid slots, garages, in-garage spots and pit boxes up to `cars` in its world when it is installed, each one row or bay on from the two before it. Maps get no garage dressing, pit-wall crew or pit boards, and no room checks. See [How maps are installed](../../docs/reference/maps.md).

Pit box 13 is 4.1 m from the safety car's spot at Zandvoort and 7.1 m at Catalunya. Grid slot 26 is at the end of the last corner zone at Imola and 8.5 m past it at Abu Dhabi.

## Playing

- Drive for a real team. A custom team adds 2 cars to the grid.
- Choose realistic performance: the equal-performance grid stays at 22 cars.
- Choose the random grid order: the weighted grid has no weights for T12 to T15.
- Turn UDP telemetry off: its format holds 22 cars.
- Garages 13 and up use the My Team garage dressing. The new teams use their source team's kit, badge and timing-tower art.

## Files

- `db/grid.json`: the database operations for each size.
- `tracks/<track>.json`: the grid, garage and pit-box edits for each size and track, with a warning where the track has less room.

The track files are generated from each track's grid, pit lane, AI line and track markup.
