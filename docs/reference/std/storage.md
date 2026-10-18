---
title: Storage
description: Read and write an addon's own files under swofty/addons/<id>/ from a native plugin with swofty/storage.hpp or the C ABI.
---

# Storage

A native plugin reads and writes its own files under `swofty/addons/<id>/` in the game folder. The calling DLL is identified the way the [assets API](/reference/addons/asset-api) identifies it, from the `dlls` its addon lists in `swofty.json`, so an addon reaches only its own folder.

Paths are relative to the addon folder. The core rejects an absolute path, a drive letter and any path that climbs above the folder with `..`. A `write` is atomic: it goes to a temp file and renames over the target, creating any missing parent folders first.

The library needs API version 5 and the `storage` [capability](/reference/addons/native-plugins). It is always on.

## C++

```cpp
#include <swofty/storage.hpp>
namespace storage = swofty::storage;
```

| Call | Returns |
|---|---|
| `available()` | `true` when the loader is version 5 or later and the core implements storage |
| `read(path, buf, cap, len)` | Reads the whole file into `buf`. Sets `*len` to the file size |
| `read(path, std::string &out)` | Reads the whole file into `out` |
| `write(path, data)` | Replaces the file atomically. `data` is a `std::string_view` or `std::span<const std::byte>` |
| `append(path, data)` | Appends to the file, creating it when absent |
| `remove(path)` | Deletes the file |
| `size(path, out)` | Sets `*out` to the file size in bytes |
| `list(dir, buf, cap, len)` | Writes the folder's entries into `buf` as newline-separated names. A sub-folder's name ends with `/`. Sets `*len` to the bytes written |

Each call returns a `storage::Result`: `ok`, `unavailable` (the loader is older than version 5, or the core does not implement storage) or `invalid`. `storage::name(result)` gives the name. A `list` with no `dir`, or `"."`, lists the addon folder itself.

```cpp
storage::write("save/progress.json", text);
std::string back;
if (storage::read("save/progress.json", back) == storage::Result::ok)
    swofty::log("[sample] %zu bytes", back.size());
```

`invalid` means a bad or escaping path, a missing file for `read`, `size` or `remove`, or a file or listing larger than `cap` for `read` and `list`. `read` and `list` set `*len` to the size they need, so a caller can grow the buffer and retry. The `std::string` `read` overload sizes its buffer from `size` and never hits that case.

## C ABI

```c
typedef struct SwoftyStorage {
    int (*available)(void);
    int (*read)(const char *path, void *buf, size_t cap, size_t *len);
    int (*write)(const char *path, const void *data, size_t len);
    int (*append)(const char *path, const void *data, size_t len);
    int (*remove)(const char *path);
    int (*size)(const char *path, uint64_t *out);
    int (*list)(const char *dir, char *buf, size_t cap, size_t *len);
} SwoftyStorage;
```

Reach the table through `api->storage` when `api->version >= 5`. Each function returns a `SWOFTY_ASSET_*` code: `SWOFTY_ASSET_OK`, `SWOFTY_ASSET_UNAVAILABLE` or `SWOFTY_ASSET_INVALID`.

## Logging

The core logs every call to `loader.log`, with the addon, the DLL, the call and the result:

```
[storage] api: sample.my-addon (my_plugin.dll) write save/progress.json (128 byte(s)) -> ok
[storage] api: sample.my-addon (my_plugin.dll) read save/progress.json (128 byte(s)) -> ok
```
