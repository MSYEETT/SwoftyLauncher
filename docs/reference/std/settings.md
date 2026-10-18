---
title: Settings
description: Declare an addon's options in swofty.json and read them from a plugin with SWOFTY_SETTING or the C ABI.
---

# Settings

To give players control over your addon, declare its settings under [`options`](/reference/addons/manifest#options) in `swofty.json`. They'll appear on the launcher's **Addons** page and in the game's **ADDON SETTINGS**, with values stored in the profile. Your plugin reads each one by its option `id`. For an example of each type, follow the [settings tutorial](/reference/addons/settings).

| | |
|---|---|
| Header | `swofty/mixin.hpp` |
| Members | `SwoftyApi::setting`, `SwoftyApi::watch_setting` |
| Feature | `options` |
| API version | 3 |
| Thread | Flow thread (for `SWOFTY_ON_CHANGE` and `watch_setting`) |

## Declare in the manifest

| Field | Meaning |
|---|---|
| `id` | Lowercase letters, digits and `_`, starting with a letter, at most 40 characters |
| `type` | `toggle`, `slider` or `choice` |
| `label`, `description` | Text shown to players; `label` defaults to the `id` |
| `default` | `true`/`false`, a number on the slider's steps, or one of the choice values |
| `min`, `max`, `step` | Slider range. `max - min` is a whole number of steps, at most 100 |
| `values` | Choice values, 1 to 16: strings, or `{ "value", "label" }` |
| `binds` | Use `native` for a value your plugin reads while the game runs, or `db` for one built into the game files, where a change needs a rebuild |

To use a `db` option in your operation files, reference it through <span v-pre>`{{id}}`</span> and `when`; see [database operations](/reference/ops/database#settings).

## Read from C++

Declare each setting with its type, the option `id` and a fallback. If you need to react when the player changes it, add `SWOFTY_ON_CHANGE`:

```cpp [native/src/my_plugin.cpp]
#include <swofty/mixin.hpp>

SWOFTY_SETTING(bool, show_panel, true);
SWOFTY_SETTING(int, ai_offset, 0);
SWOFTY_SETTING(std::string_view, team_livery, "titlesponsor");

SWOFTY_ON_CHANGE(show_panel, bool on)
{
    swofty::log("[my-addon] panel %s", on ? "on" : "off");
}

SWOFTY_ON_LOAD
{
    if (show_panel && team_livery == "replica")
        swofty::log("[my-addon] offset %d", int(ai_offset));
}

SWOFTY_ADDON()
```

| `type` | Option `type` | Conversion |
|---|---|---|
| `bool` | `toggle` | |
| An integer type | `slider` | Rounded to the nearest whole number |
| A floating-point type | `slider` | |
| `std::string_view` | `choice` | The choice's value |

`SWOFTY_SETTING(type, id, fallback)` declares `inline const swofty::setting<type> id`. You can read it as a `T`, as in the example above, or call `get()`:

```cpp
template <class T> struct swofty::setting {
    using type = T;
    const char *id;
    T fallback;
    T get() const;
    operator T() const;
    friend bool operator==(const setting &s, T v);
};
```

- `get()` gives you the profile's value for the session. You'll get `fallback` if the loader is older than version 3 or `swofty.json` has no option with that `id` and type.
- To name your variable differently from the option `id`, construct `swofty::setting<T>{"id", fallback}` directly.
- When the player changes a value in **ADDON SETTINGS**, `SWOFTY_ON_CHANGE` runs with the new value.
- At init, `SWOFTY_ADDON()` checks every setting you've declared. It logs each one that `swofty.json` doesn't declare with the same type.

## Read from C

```c
enum { SWOFTY_SETTING_TOGGLE = 1, SWOFTY_SETTING_SLIDER = 2, SWOFTY_SETTING_CHOICE = 3 };

typedef struct SwoftySettingValue {
    uint32_t type;
    int32_t toggle;
    double slider;
    const char *choice;
} SwoftySettingValue;

typedef void (*SwoftySettingFn)(const char *id, const SwoftySettingValue *value, void *user);
```

- To read a value, call `api->setting(id, type, &out)`. It fills `out` with the calling addon's value and returns non-zero. If the addon has no option `id` of that `type`, it returns 0.
- To react to changes, call `api->watch_setting(id, fn, user)`. It calls `fn` with `user` on every change.

Both calls need API version 3 and identify your addon by the calling DLL.

## Files

| File | Written by | Holds |
|---|---|---|
| `$DATA/profiles/<name>.json` | The launcher | `options`: each addon's values that differ from the default |
| `swofty/settings.json` | The build | Every enabled addon with its options, values and the commands ADDON SETTINGS sends |
| The requests file | swofty_core | Changes made in game, merged into the profile by the launcher |
