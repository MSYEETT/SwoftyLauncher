---
title: Characters
description: Per-career character overrides with swofty/characters.hpp — swap a character's portrait and name, mute a speaker and set career-only strings, for one career data group.
---

# Characters

Use the characters library to change who a character is in one kind of career: in careers of one data group, show another character's portrait, names and job on a character's ref, play a speaker's voice lines silently, and set string overrides that hold only while such a career is loaded. Careers of every other data group, and every other mode, stay stock.

| | |
|---|---|
| Header | `swofty/characters.hpp` |
| Namespace | `swofty::characters` |
| Table | `SwoftyApi::characters` (`SwoftyCharacters`), null on cores without it |
| API version | 8 (the table member), feature-detected: the table is null on cores without it |
| Compile-time check | `#ifdef SWOFTY_API_CHARACTERS` |
| Feature | `career` — stable |
| Thread | registration: addon init or `SWOFTY_ON_LOAD`, then any thread. `refresh`: the game thread |

Every call returns a `swofty::characters::Result` with the [result codes](./#result-codes). A core without the table returns `unavailable` from every call.

## When to register

The core installs its hooks only once an addon needs them: the ref hooks after the first `swap`, the voice hook after the first `mute`. A registration from the check function you pass to `SWOFTY_ADDON` is installed with that commit's addon mixins: an addon's own wrap of the same function chains with the core's. A registration from `SWOFTY_ON_LOAD` is installed right after that commit's `SWOFTY_ON_LOAD` handlers. Until then a registration returns `ok`; after the last mixin commit, one that needs hooks that were never installed returns `unavailable` and logs `registered after the last mixin commit`. A game where no addon swaps or mutes runs none of these hooks, and an addon that wraps `Game_SpeechSystem_PostLine` or the character ref functions itself keeps its wraps.

Register from an addon with `"early": true` to have the ref hooks in place before the game's first character list build; otherwise `refresh()` and the career events rebuild the list only after the game has built it once more.

## Data groups

Every entry is keyed by a career data group: the data group of the career's `CareerSetupComponent`, the name the career content tile carries (`f1_career_2025`, `f1_career_2026`, an addon's own group). `career_group()` returns the loaded career's data group, or `""` with no career loaded. The core works it out when the core's `career.loaded` event fires, clears it on `career.unloaded`, and reads it again on `refresh()`.

## Calls

| Call | Does |
|---|---|
| `available()` | `true` when the core has the table, the `career` feature is on, and its character or voice hooks are installed or not needed yet |
| `career_group()` | the loaded career's data group, or `""` |
| `swap(group, character, replacement)` | in careers of `group`, the ref of the `entrants.characters` row `character` is built from the row `replacement` (names, `job_role`, the `message_<name>` textures), under `character`'s `stat_id` |
| `restore(group, character)` | drops that swap |
| `mute(group, speaker, on = true)` | in careers of `group`, lines of `speaker` (`agent`, `agent_cutscene`, ...) play at speech volume 0 |
| `string(group, key, text, language = "*")` | in careers of `group`, `key` shows `text` (a [string override](./strings)) |
| `clear_string(group, key, language = "*")` | drops that string |
| `strings_json(group, json)` | every string of a JSON object of language objects, `{"eng": {"lng_key": "text"}, "fre": {...}}` |
| `clear(group)` | drops every swap, mute and string your addon registered for `group` |
| `refresh()` | reads the loaded career's data group again and rebuilds the game's character list now |

Several addons can register for the same or different groups. Entries belong to the module that registered them, so `clear` drops only yours; when two addons swap the same character in the same group, the last one wins and the core logs it.

`swap` returns `late` when the group's career is already loaded: the swap applies at the next character list build, or at once after `refresh()`.

## How it applies

- **Refs.** The UI keeps one VM node per character row under `character_list`, filled by `Game_EntrantsUi_BuildCharacterRef`; the subtitle box, emails and contacts bind those nodes. The game builds the list in `Game_EntrantsUi_OnCharacterListLoaded` (the `GameCharacterListLoaded` handler) when it loads its characters. The core keeps that character vector, and when a career load or unload changes the active group for a swapped character, it runs the handler again, so the list is rebuilt for the new group. The replacement row has to be in the character table (add it with a database operation); without it the core logs `no '<name>' row in the character list` and the ref stays stock.
- **Voices.** `Game_SpeechSystem_PostLine` posts each line on the line's emitter. For a muted speaker the core sets the emitter's `vol_speech` RTPC to 0 before the line is posted, so its subtitles and end-of-line callbacks still run. An emitter that is muted once stays muted; when it speaks for another speaker the core logs `muted earlier, also speaks <speaker>`.
- **Strings.** A group's strings are set through the [string overrides](./strings) when the group becomes active and cleared when it stops being active. The core does not call `swofty::lang::refresh()`.

The refs and the voices are core mixin sets of their own (`swofty_core.dll (character refs)`, `swofty_core.dll (character voices)`), so a binding that does not resolve turns off only that part.

The data group is read the way the driver-career save-slot predicate (0x1438fb390) reads it: each championship of the career entity (`+0x1040..+0x1048`), its `CareerSetupComponent` through the championship's component list (`+0x208`, vtable slot `+0x10`), the component's data-group index (virtual `+0x268`), the group list (`*(component +0x290) +0x560`) and the group name (`*(group +0x28) +0x10`). These offsets are not KB struct rows yet.

## Example

```cpp
#include <swofty/characters.hpp>
#include <swofty/storage.hpp>

namespace ch = swofty::characters;

SWOFTY_ON_LOAD
{
    ch::swap("f1_career_1990", "elena", "flavio");
    ch::mute("f1_career_1990", "agent");
    ch::mute("f1_career_1990", "agent_cutscene");
    std::string json;
    if (swofty::storage::read("data/flavio-strings.json", json) == swofty::storage::Result::ok)
        ch::strings_json("f1_career_1990", json);
}
```

To build one plugin against cores with and without the library, guard the calls with `#ifdef SWOFTY_API_CHARACTERS` and check `ch::available()` at runtime.

## Log

`[characters] setup: character refs armed|off|not used, voices armed|off|not used` after each commit that installs a part; `[characters] swap ...`, `mute ...`, `<n> string(s) in <m> language(s) for careers of '<group>'` per registration; `[characters] career loaded: active group '<group>'`; `[characters] <why>; character list rebuilt (<n> stand-in ref(s))`; `[characters] <speaker> line muted (emitter ...)`; `[characters] strings: cleared <n>, set <m> for '<group>'`.
