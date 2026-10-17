---
title: Settings
description: Declare addon options in swofty.json and read them from a native plugin with swofty/mixin.hpp or the C ABI.
---

# Settings

An addon's settings are its [`options`](/modding/manifest#options) in `swofty.json`. The launcher shows them on the **Addons** page, the game shows them in **ADDON SETTINGS**, and the profile stores the values. The [tutorial](/tutorials/addon-settings) builds one of each type.

## Declaring

| Field | Meaning |
|---|---|
| `id` | Lowercase letters, digits and `_`, starting with a letter, at most 40 characters |
| `type` | `toggle`, `slider` or `choice` |
| `label`, `description` | Text shown to players. `label` defaults to the `id` |
| `default` | `true` or `false`, a number on the slider's steps, or one of the choice values |
| `min`, `max`, `step` | Slider range. `max - min` is a whole number of steps, at most 100 |
| `values` | Choice values, 1 to 16: strings, or `{ "value", "label" }` |
| `binds` | `native`: read by the plugin while the game runs. `db`: built into the game files, applied after a rebuild |

`db` settings reach operation files through <span v-pre>`{{id}}`</span> and `when`. See [database operations](/modding/database-ops#settings).

## C++

```cpp
SWOFTY_SETTING(type, id, fallback);
SWOFTY_ON_CHANGE(id, type value) { ... }
```

`SWOFTY_SETTING` declares `inline const swofty::setting<type> id`. `id` is the option's `id`.

| `type` | Option type | Conversion |
|---|---|---|
| `bool` | `toggle` | |
| An integer type | `slider` | Rounded to the nearest whole number |
| A floating-point type | `slider` | |
| `std::string_view` | `choice` | The choice's value |

```cpp
template <class T> struct swofty::setting {
    using type = T;
    const char *id;
    T fallback;
    setting(const char *id, T fallback);
    T get() const;
    operator T() const;
    friend bool operator==(const setting &s, T v);
};
```

- `get()`, or using the setting as a `T`, returns the profile's value. It returns `fallback` when the API is older than version 3 or `swofty.json` has no option of that `id` and type.
- Construct `swofty::setting<T>{"id", fallback}` directly for a variable whose name differs from the option.
- `SWOFTY_ON_CHANGE` runs when the player changes the value in ADDON SETTINGS, with the new value.
- At init, `SWOFTY_ADDON()` checks every declared setting and logs each one that `swofty.json` does not declare with the same type.

## C ABI

```c
enum { SWOFTY_SETTING_TOGGLE = 1, SWOFTY_SETTING_SLIDER = 2, SWOFTY_SETTING_CHOICE = 3 };

typedef struct SwoftySettingValue {
    uint32_t type;
    int32_t toggle;
    double slider;
    const char *choice;
} SwoftySettingValue;

typedef void (*SwoftySettingFn)(const char *id, const SwoftySettingValue *value, void *user);

int (*setting)(const char *id, uint32_t type, SwoftySettingValue *out);
int (*watch_setting)(const char *id, SwoftySettingFn fn, void *user);
```

`setting` fills `out` with the calling addon's value and returns non-zero, or returns 0 when the addon has no option `id` of that `type`. `watch_setting` calls `fn` with `user` on every change. Both identify the addon by the calling DLL and need API version 3.

## Files

| File | Written by | Contents |
|---|---|---|
| `$DATA/profiles/<name>.json` | The launcher | `options`: each addon's values that differ from the default |
| `swofty/settings.json` | The build | Every enabled addon with its options, values and the commands ADDON SETTINGS sends |
| The requests file | SwoftyMod | Changes made in game, merged into the profile by the launcher |
