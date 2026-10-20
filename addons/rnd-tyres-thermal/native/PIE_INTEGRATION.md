# rnd_fifth_pie - integration notes

Native half of the 5-sector development-potential pie (Tyres = department code 4, short name
`tyres`, colour ABGR 0xff0078ff). Every hook is a mixin on a KB binding resolved by signature at
load (build d873a1c8 in the KB). Site proofs and the binding list: `PIE_SITES.md`. These notes
were written for v0.7.0, when the pie code joined the addon; later pie changes (v0.7.1 slot
construction, v0.7.2 tile counts and tile route) are in `HOOKS.md`.

## Files

| file | |
|---|---|
| `src/rnd_fifth_pie.hpp` / `src/rnd_fifth_pie.cpp` | the translation unit |
| `../tools/sites/fdis.py`, `xjump.py`, `sigscan.py`, `sitebytes.py` | the offline checks behind `PIE_SITES.md` (run with `re/tools/py`, `EXE=<F1_25.exe>`) |

## Wiring

```cpp
// native/src/rnd_departments.cpp, in SWOFTY_ON_LOAD
    install_fifth_pie(base);
    rnd_fifth_pie::install(base);   // right after install_fifth_pie
```
`SWOFTY_SITE` / `SWOFTY_REDIRECT` work from a second TU: the mixin and binding registries are
`inline` lists appended at static init, and `SWOFTY_NS` lives in an anonymous namespace, so only
`SWOFTY_ADDON()` must stay single (it stays in rnd_departments.cpp).

## Pieces

The file has no options; all four pieces are always on.

| piece | does |
|---|---|
| slice | populate: code 4 -> `tyres` name / description key / VM child / `department_name`, colour 0xff0078ff, wheel icon borrows `maintenance` |
| focus | `department_focused "tyres"` focuses the `tyres` child and raises `highlight_pie_sector`; `g_Career_RnDOverviewSelectedDept` untouched |
| open | the Tyres focus flag (cleared by a stock focus on either page), read by the box-grid first-tab route in rnd_departments.cpp, and the populate focus restore |
| performance | publisher: `department_performance.tyres` per team (orange `dept_colour`, `lng_rnd_dept_tyres`, `rnd_goto_tyres_department`, derived `perf_fraction` / `potential_perf_fraction`, `max_perf_fraction` = 1 like the stock four) |

The slice and focus pieces rely on install_fifth_pie's populate changes (code guard relaxed to 4,
buffer 0xb40 with slot 4). Until `install()` runs every handler is a no-op. The bindings resolve
all-or-nothing with the rest of the addon.

## Sites (only when the department code is 4)

| binding | action |
|---|---|
| `site_Career_RnDPieSliceNameJoin` | rdi `error_department` -> `tyres` |
| `site_Career_RnDPieSliceDescFormat` (redirect of the Ne_Format_String call) | name argument `error_department` -> `tyres` (description key) |
| `site_Career_RnDPieSliceDefaultColour` | eax 0xffffffff -> 0xff0078ff |
| `site_Career_RnDPieSliceIconArgs` | [rsp+0x20] `tyres` -> `maintenance` |
| `site_Career_RnDPieFocusNameCmp` | data "tyres": rdi = `tyres`, rip = `site_Career_RnDPieFocusJoin`; a stock name clears the Tyres flag |
| `site_Career_RnDResearchFocusNameCmp` | a stock name clears the Tyres flag |
| `site_Career_RnDPieFocusRestore` | Tyres flag set: al = 4 for code 4, 0xff for codes 0..3 |
| `site_Career_RnDPerfDeptLoopEnd` | 4 absent: list[count] = 4, rax = count+1 (stored count unchanged) |
| `site_Career_RnDPerfDeptName` | r8 -> `tyres`; derive [rsp+0x158] perf / [rsp+0x154] potential |
| `site_Career_RnDPerfChildName` | rax -> `tyres` (child name) |
| `site_Career_RnDPerfDeptColour` | eax white -> 0xff0078ff (`dept_colour`) |
| `site_Career_RnDPerfGotoName` | r8 -> `tyres` (`rnd_goto_%s_department`) |
| `k_Career_RnDErrorDeptName` | the `error_department` string every name check compares against |

## The Tyres value (performance piece)

Stat entries = `Upgrades+0x1a38..+0x1a40`, 0x20 each (+0 attribute type, +1 department, +0xc
current, +0x10 max); F = FUN_143518f50 = the field-wide best department max (xmm0 at P1).
- If this team has stat entries with department 4 (sum max != 0), the engine's own values are
  kept (logged once).
- Otherwise T = the +0x70 types of this team's dept-4 instances with row flag +0x7c == 0xff,
  hub (+0x58 == 0xbd057d45) excluded (tree vectors at Upgrades+0xe0+k*0xc40 +0x10/+0x18, 0xe0
  stride, k < Upgrades+0x1960, exactly as the addon's v0.6.0 diagnostic walks them), and
  `perf = engine_perf + sum cur(first entry of each t) / F`,
  `potential = sum max(first entry of each t) / F`, clamped `0 <= perf <= potential <= 1`.
  engine_perf is the engine's own dept-4 pending gain from completed Tyres upgrades
  (FUN_143518660(ctx, 4)). No stat match -> `potential = perf`, logged.
The tyre attributes are also counted in their stock department (Durability), and a subset of a
department's attributes gives a smaller wedge than a full department.

## Naming contract (for the .ui)

- Tile / populate VM child: `tyres` (djb2 0x107666dc) under the same parent as `aero` etc.;
  `department_name` = `tyres`; name `lng_rnd_dept_tyres`; description
  `lng_rnd_dept_tyres_overview_description` (both in the addon's `assets/text.json`; the
  description is `eng` only); colour leaf 0xff0078ff; `progress_wheel_bg_icon_tex` =
  `overrides.career.rnd_wheel_icon.maintenance`.
- Focus: the 5th tile must send `department_focused` with data `tyres` (`tools/build_pie.py`
  rebinds the 5th-box tile from `fake` to `tyres`). `focused_department.department_name` becomes
  `tyres`, so `highlight_department` / `focused_department` MVCDs need a `tyres` case.
- Pie data: `focused_team_perf_root.department_performance.tyres.{perf_fraction,
  potential_perf_fraction, max_perf_fraction, dept_colour, name, dept_colour_anim}`.
- `dept_colour_anim` = `rnd_goto_tyres_department`; no stock anim with that name was found
  (the `error_department` variant had none either).

## Expected log

```
[rnd-tyres] pie5 perf: hooked department_performance loop bound (rax) @0x1418945b6 (base+0x18945b6)   (x5 sites)
[rnd-tyres] pie5 slice: hooked populate name join (rdi) @0x141f482a0 ...                                (x4 sites)
[rnd-tyres] pie5 focus: hooked focus handler name compare @0x141f4a9e6 ...
[rnd-tyres] pie5 open: hooked R&D > Performance stock focus @0x141f4a9e6 ...                           (x3 sites)
[rnd-tyres] pie5 perf INSTALLED: department_performance.tyres published per team
[rnd-tyres] pie5 slice INSTALLED: code 4 populates as tyres / 0xff0078ff / icon maintenance
[rnd-tyres] pie5 focus INSTALLED: data "tyres" resumes at 0x141f4aa4f
[rnd-tyres] pie5 open INSTALLED: the Tyres focus is tracked for the departments screen's first tab and restored on the Performance page
-- on R&D > Performance --
[rnd-tyres] pie5 slice: code 4 name error_department -> tyres (VM child tyres, lng_rnd_dept_tyres)
[rnd-tyres] pie5 slice: code 4 description key -> lng_rnd_dept_tyres_overview_description
[rnd-tyres] pie5 slice: code 4 colour white -> 0xff0078ff
[rnd-tyres] pie5 slice: code 4 wheel icon -> rnd_wheel_icon.maintenance (borrowed, no tyres frame)
[rnd-tyres] pie5 perf: department_performance loop gets dept 4 (4 -> 5 entries; the stored list is unchanged)
[rnd-tyres] pie5 perf: team 0x.. dept 4 from N Tyres type(s) [..] (M stat match): F=.. engine perf=.. cur=.. max=.. -> perf=.. potential=..
[rnd-tyres] pie5 perf: dept 4 published as department_performance.tyres
[rnd-tyres] pie5 perf: dept 4 dept_colour white -> 0xff0078ff
[rnd-tyres] pie5 perf: dept 4 dept_colour_anim -> rnd_goto_tyres_department
-- focusing the Tyres tile --
[rnd-tyres] pie5 focus: department_focused "tyres" -> focused_department = tyres, highlight_pie_sector raised (selection index untouched)
```
The `hooked` lines print each binding's resolved address. `install()` has no refusal path; a
binding that does not resolve drops the whole addon (`swofty/loader.log`, `[core] ... needs <binding>, which does not resolve`).

## Live checks (gdb under Wine: `handle all nostop noprint pass` then `handle SIGTRAP stop noprint nopass`)

The hook sites start with a `jmp` once installed, so break on the instruction after each window,
with hardware breakpoints (max 4 at a time):
- `hbreak *0x141f482bb if $r14==4` -> `x/s $r8` = `tyres` (lng_rnd_dept_%s arg).
- `hbreak *0x141f48629 if $r14==4` -> `p/x $eax` = 0xff0078ff.
- `hbreak *0x141f48a6b if $r14==4` -> `x/s *(char**)($rsp+0x20)` = `maintenance`.
- `hbreak *0x141f4aa4f` then focus the Tyres tile -> `x/s $rdi` = `tyres`; `x/bx 0x145f93c08`
  (`g_Career_RnDOverviewSelectedDept`) unchanged from before the focus.
- `hbreak *0x1418946e0 if *(unsigned char*)$rbp==4` -> `x/s $r8` = `tyres`,
  `x/2fw $rsp+0x154` = potential, perf (both in 0..1, potential >= perf).
- `hbreak *0x141894cfb if *(unsigned char*)$rbp==4` -> `x/wx $rsp+0x9c` = 0xff0078ff (the table
  0x14571bce0 itself stays stock unless the v0.6.1 card colour write changed it).
