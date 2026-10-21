# In-game test: rnd-tyres-thermal 0.9.0

What to look at on each screen, and the `swofty/loader.log` lines (in the game folder) that prove each part. Lines start with `[rnd-tyres]`; search for the quoted text.

## Setup

1. Build the DLL: `addons/rnd-tyres-thermal/native/build.sh`.
2. `swofty addons install addons/rnd-tyres-thermal`, enable `swofty.rnd-tyres-thermal` on a modded profile. The addon has no settings.
3. Press Play. Use a 2026 career save: a new one, or one from 0.8.0 (its three Tyres states land on A1-A3).

## At launch

| part | log line |
|---|---|
| the DLL loaded and the art arrays are installed | `tyres art: part images OWN (2/2 …), part masks OWN (2/2 with 15), category icons OWN (2/2 with 4 …)` |
| the department icon | `tyres icon LIVE: 6/6 grown R&D icon arrays` |
| the upgrade types | `upgrade types INSTALLED: 19 radUpgradeTyreThermal, 20 radUpgradeBrakeCooling, 21 radUpgradeTyreLife, 22 radUpgradePUCooling; 22 consumer hooks, 6 order-table leas` |
| the list icons | `dept tags INSTALLED: 9/9 icon sites` |
| the pie's Tyres slice, focus, tile route and data | `pie5 slice INSTALLED`, `pie5 focus INSTALLED`, `pie5 open INSTALLED`, `pie5 perf INSTALLED` |
| Tyres research capacity | `capacity INSTALLED: dept 4 mirrors Durability's team modifiers` |

Any line ending in `(SAFE)` means that piece kept the stock behaviour. Send the whole log.

## Career load

- `upgrade types: upgrade_type_metadata row "Tyre Thermal" -> type 19 (radUpgradeTyreThermal)`, and the same for 20, 21 and 22.
- `upgrade types: upgrade row "Brake Cooling" -> type 20 (radUpgradeBrakeCooling)` (one line per type).
- `instance init: tyres_thermal_<n> "<name>" -> normal init` for the first rows.

## R&D > Performance

- Five tiles (3+2); the Tyres tile has the tyre icon, the orange ring and **0/15** fitted upgrades on a fresh save.
- The pie has five sectors; focusing the Tyres tile highlights the orange sector. Pressing it opens the Tyres page.
- Log: `pie5 slice: code 4 name error_department -> tyres` and `pie5 perf: dept 4 published as department_performance.tyres`. Focusing the Tyres tile: `pie5 focus: department_focused "tyres"`; focusing a stock tile afterwards: `pie5 open: R&D > Performance focused stock department index N; Tyres focus cleared`.
- A Tyres upgrade's Develop Upgrade screen lets you start research while fewer Tyres upgrades are in research than Durability has research slots. Log: `capacity check: team 0x… dept 4 slots=N in-progress=M gate=1 -> research-limit=0`.

## The Tyres department page

Four category rows, in this order, each with its own icon and label:

| type | label | icon | upgrades (name, size) |
|---|---|---|---|
| 19 | Tyre Thermal | `art/out/tyre_thermal.png` | Tyre Thermal Model (Minor), Rim Heat Shielding (Minor), Wheel Rim Cooling Channels (Major), Carcass Heat Management (Ultimate) |
| 20 | Brake Cooling | `art/out/brake_cooling.png` | Brake Duct Inlet Revision (Minor), High-Density Disc Ventilation (Minor), Caliper Cooling Fins (Major), Brake Drum Airflow Package (Ultimate) |
| 21 | Tyre Life | `art/out/tyre_life.png` | Sidewall Load Analysis (Minor), Tyre Pressure Telemetry (Minor), Camber Wear Optimisation (Major), Low-Degradation Package (Ultimate) |
| 22 | Power Unit Cooling | `art/out/pu_cooling.png` | Radiator Core Density (Minor), Cooling Exit Louvres (Major), Integrated Thermal Management (Ultimate) |

That is 15 upgrades. Check:

- The category icons are the Tyres ones above, not the Chassis or Durability icons. Log: `dept tag: department category row icon "error_department_tyre_thermal" -> "tyres_tyre_thermal"`.
- The row labels read Tyre Thermal, Brake Cooling, Tyre Life, Power Unit Cooling, never "INVALID UPGRADE". Log: `upgrade types: department category row 0x141c82470 label type 19 -> "tyre_thermal"`.
- Every card shows its name and its own art (below), and the Tyres orange.
- The descriptions match `assets/text.json` (Develop Upgrade screen).

### Prices and days

Research cost / development cost, research days / development days, before team modifiers:

| size | cost | days | weighting |
|---|---|---|---|
| Minor | 500 / 1000 | 20 / 25 | 1 |
| Major | 1000 / 1000 | 30 / 25 | 2 |
| Ultimate | 1500 / 1000 | 40 / 25 | 3 |

The game scales these by Durability's cost and time modifiers, so compare a Tyres Minor with a stock Durability Minor on the same save: they should match. The Ultimates (A4, B4, C4, D3) unlock at facility level 3 like Durability's; if they stay locked at a higher facility level, report it.

### The tree

Unlocks run top to bottom in four columns: A1 → A2 → A3 → A4, B1 → B2 → B3 → B4, C1 → C2 → C3 → C4, and D1 → D2 → D3 hanging off B2. A1, B1 and C1 are available from the start; D1 becomes available once B2 is complete.

```
        x=6   x=7   x=8   x=9
  y=5   A1    B1    .     C1        A  Tyre Thermal
  y=6   A2    B2 -> D1    C2        B  Brake Cooling
  y=7   A3    B3    D2    C3        C  Tyre Life
  y=8   A4    B4    D3    C4        D  Power Unit Cooling
```

If the tree map is shown, the block sits between the Chassis and Durability trees, clear of both.

## Card art in each state

Compare with a stock Chassis or Durability card in the same state (`art/preview.html` shows ours next to stock):

| state | how to get it | what the card shows | log (first 32 rewrites) |
|---|---|---|---|
| available or locked | any card you have not started | grey wireframe of the part | `card art: site N image "tt_thermal_model" -> own "om_xm_tt_thermal_model"` |
| in progress | start researching or developing one | the same wireframe in white | `-> own "om_xm_tt_thermal_model_progress"` |
| fitted | finish research and development | the full-colour render | `-> own "om_xm_tt_thermal_model_complete"` |
| mask | any card | the card's highlight hugs the part's outline | `card art: site N mask "tt_thermal_model" -> own "om_xm_tt_thermal_model_m"` |

Check one card of each branch in each state, and the Develop Upgrade screen's large card. A blank card means a frame name the game did not find: send the `card art:` lines.

## Other surfaces

- R&D > In Progress: a Tyres upgrade shows the Tyres category icon, its type label and the orange strip.
- Team HQ popups (research or development complete): Tyres category icon and orange.
- Back from a Tyres upgrade's Develop Upgrade screen returns to the Tyres page.
- The vehicle performance comparison on the Develop Upgrade screen fills in (Chassis for Tyre Thermal, Brake Cooling and Tyre Life, Powertrain for Power Unit Cooling).

## A fitted upgrade reaches the car

1. Complete a Tyres upgrade (research and develop it until it is fitted).
2. Start any session (practice is enough). Leaving Team HQ for the session picks the weekend objectives, which leave every Available Tyres upgrade out of the upgrade-discount types (0.9.0 crashed here before the fix):

```
[rnd-tyres] upgrade types: weekend objectives 0x14295c0a0 leaves Available type 19 (TyreThermal) out of the discount type bitset
```

   The game then builds the car. Each Tyres type the car's upgrade map holds is named to its own block (first 8 lines):

```
[rnd-tyres] upgrade types: apply 0x14fa63050 type 19 names block radUpgradeTyreThermal
```

The addon has no apply log of its own. With the game's `VEHICLE_UPGRADES` log channel on, the game prints `Applying radUpgradeTyreThermal upgrade to vehicle N. (perc0to1: x)`, where x is the fitted weighting of that type over the type's total: Tyre Thermal, Brake Cooling and Tyre Life total 7 (a Minor gives 0.1429, a Major 0.2857, an Ultimate 0.4286), Power Unit Cooling totals 6 (0.1667, 0.3333, 0.5). Otherwise check the effect on track (`native/HOOKS.md`, "Power Unit Cooling" and the Brake note).

## Only the game can confirm

- That the 2026 card lookups find our frames under their stock-style names (`om_xm_<name>_progress`, `om_xm_<name>_m`). The game builds `<name>_in_progress` and an unsuffixed mask; the hook renames them, and the frames are in the lists at those names.
- How close the BC7 encoding of the fitted renders looks to the stock cards next to them (mode 6 only: fine detail such as radiator fins loses some contrast).
- Whether the Ultimates unlock at facility level 3.
- Whether Brake Cooling and Power Unit Cooling change anything on track (`native/HOOKS.md`, "Power Unit Cooling" and the Brake note).

## My Team is blocked (0.11.0)

1. Career > My Team, new or continue: a "My Team not available" popup with one OK button shows, and the Career menu stays open and usable after OK.
2. The core log has the block and each cancelled route:

```
[career] swofty.rnd-tyres-thermal blocks my_team careers; the start gate is live
[career] career.starting: my_team (pick, route 'career_dlc_select_my_team'); blocked by swofty.rnd-tyres-thermal
[popup] shown for swofty.rnd-tyres-thermal as 'swofty_message': "My Team not available"
```

3. Career > Driver Career, new and continue, opens as before and the Tyres department page shows in R&D.
