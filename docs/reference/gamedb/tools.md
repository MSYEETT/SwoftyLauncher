# Tools

## The formats crate

`swofty-formats` (`packages/formats`) reads and writes the database.

`swofty_formats::gvdb`:

| Item | Purpose |
|---|---|
| `Gvdb::parse(&[u8]) -> Result<Gvdb>` | Parse a file. Fails on a missing terminator or trailing bytes |
| `Gvdb::to_bytes()` | Write, sorted stably by `key_hi`, re-masked. An unchanged database writes back byte for byte |
| `Gvdb::to_bytes_unsorted()` | Write in the current record order |
| `Gvdb::get(path)`, `get_key(u64)`, `get_text(path)`, `position(u64)` | Look up a record |
| `Gvdb::set(path, value, tag)` | Replace a value, or add a key when `tag` is given. Returns whether the key existed |
| `Gvdb::remove(path)` | Remove a record |
| `Gvdb::instance_count(container, row)` | Read `<container>._<row>_instancecount_` |
| `instance_count_path(container, row)` | Build that path |
| `Record { key, tag, value }`, `Record::new(path, tag, value)` | One record; `value` is the unmasked text |
| `ValueType::from_tag`, `ValueType::tag` | Tag numbers and names |
| `MASK`, `mask(plain, stream)`, `unmask(enc, stream)` | The value mask |

`swofty_formats::hash`: `fnv1a64(path)`, `fnv1a64_from(h, suffix)` (continue a hash, as in `{hash}` placeholders), `fnv1a64_unwind(h, suffix)` (peel a suffix off), `fnv1a32` (label lookups), `djb2`.

## The launcher's database stage

`swofty build` and `swofty apply` (see [the command line](/reference/cli)) write a new database when a profile has maps or database operations:

1. Read the pristine database from the store.
2. For each enabled map, `activation::db::activate` adds the track's rows (registry: `packages/core/src/activation/track_tables.json`).
3. For each addon in load order, `ops::apply_db_ops` applies its [database operations](/reference/ops/database), after `settings::render_ops` fills <span v-pre>`{{option}}`</span> placeholders and `when` conditions.
4. Write `game_package/gamedatabases/gamedatabases.2025.bin` into the build's overlay folder.

The build hash covers the pristine database, the registry and every rendered operation file. The journal installs the overlay file and restores the original.

## Python tools

`tools/` holds the research tools. They are the byte-identity oracle for the Rust code and are not shipped.

| Tool | Use |
|---|---|
| `tools/gamedb.py` | `roundtrip <db>`, `get <db> <path>`, `set <db> <path> <value> -o <out> [--type N]`, `dump <db> [-o json] [--names tsv] [--tree]` |
| `tools/gdb_hashsim.py <db>…` | Replays the game's hash-map load and prints the missing-key sentinel |
| `tools/track_keys.py names <exe>` | Prints the 8,286 names the exe's database readers use, stack-built ones included |
| `tools/track_keys.py keys <exe> <db> <track>` | Prints the keys a track's rows hold, found with those names |
| `tools/map_check.py <overlay> --game <dir>` | Checks a build with maps against the game it was built from |
| `tools/gamedb/catalog.py --game <dir>` | Writes the [table and key catalogue](/reference/gamedb#table-catalogue) |

`track_keys.py`, `catalog.py` and the reader-key part of `map_check.py` need `numpy`, `pefile` and `capstone`. On NixOS:

```sh
nix-shell -p 'python3.withPackages(ps: [ps.numpy ps.pefile ps.capstone])'
```

### How `catalog.py` names keys

1. It reads the names the exe's database readers use (`track_keys.exe_names`) and the seed list `tools/gamedb/names.txt`.
2. It finds tables bottom-up: for every key it peels `._metadata_` and a row digit; a hash reached from both row 0 and row 1 is a table prefix.
3. It walks top-down from every name: `<node>._<word>_instancecount_` finds a table and its row count, and `<node>.<name>` finds a field. New rows are walked again, which finds nested tables.
4. It names unresolved table prefixes by trying every row word against the count key, and attaches them to a known parent where a name links them.
5. It scans the exe's code for 64-bit immediates equal to a key.
6. It gives every table and key a meaning: the most specific pattern in `tools/gamedb/meanings.tsv`, or else one built from the name, the unit suffix (`_metres`, `_deg`, `Factor`) and the values (a field whose values are all labels of one table is a reference to it). It also records each numeric key's range.

Names it cannot reach stay unnamed. Adding names to `names.txt` and rerunning names more of the file.
