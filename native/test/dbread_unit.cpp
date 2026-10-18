// Host-side unit test for the build-independent db-read logic in dbread_logic.hpp:
// fnv1a64 key hashing, incremental continuation, table/row/cell key construction, the row
// stem derivation, and the typed record decode. No Windows, no Wine, no live game.

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>

#include "../src/core/dbread_logic.hpp"

static int failures;

static void ok(bool cond, const char *what)
{
    printf("%s: %s\n", cond ? "ok  " : "FAIL", what);
    if (!cond)
        failures++;
}

static uint64_t full(const std::string &s)
{
    return dbread::fnv64(s);
}

int main()
{
    // fnv1a64 against the hashes recorded in re/db/db_keys.tsv.
    ok(full("entrants.teams._team_instancecount_") == 0x3f0b53f9d57b748eull, "fnv64 teams instancecount");
    ok(full("entrants.vehicles._vehicle_instancecount_") == 0x7b6eb7739e186f54ull, "fnv64 vehicles instancecount");
    ok(full("entrants.drivers._driver_instancecount_") == 0x11ff62da5ecc0d64ull, "fnv64 drivers instancecount");
    ok(full("entrants.teams.team_28.stat_id") == 0x83a0da0da7de0118ull, "fnv64 team_28.stat_id");
    ok(full("ai.general_ai_settings.max_test_time") == 0x2273a5bae867b23bull, "fnv64 max_test_time");

    // Incremental continuation of a prefix state equals hashing the whole string.
    uint64_t pref = dbread::fnv64("entrants.teams");
    ok(dbread::fnv_cont(pref, ".team_28.stat_id") == full("entrants.teams.team_28.stat_id"), "incremental continuation");

    // Row stem derivation strips one trailing 's' from the last path segment.
    ok(dbread::row_stem("entrants.teams") == "team", "stem teams->team");
    ok(dbread::row_stem("entrants.vehicles") == "vehicle", "stem vehicles->vehicle");
    ok(dbread::row_stem("entrants.entrant_lists") == "entrant_list", "stem entrant_lists->entrant_list");
    ok(dbread::row_stem("entrants.entrant_setup_modes") == "entrant_setup_mode", "stem setup_modes");

    // table_keys builds the same hashes as the full dotted strings.
    dbread::TableKeys tk = dbread::table_keys("entrants.teams");
    ok(tk.ok, "table_keys ok");
    ok(tk.count_key == full("entrants.teams._team_instancecount_"), "table_keys count key");
    ok(dbread::row_key(tk.row_prefix, 0) == full("entrants.teams.team_0"), "row_key 0");
    ok(dbread::row_key(tk.row_prefix, 28) == full("entrants.teams.team_28"), "row_key 28");
    ok(dbread::cell_key(tk.row_prefix, 28, "stat_id") == full("entrants.teams.team_28.stat_id"), "cell_key 28.stat_id");

    dbread::TableKeys bad = dbread::table_keys("");
    ok(!bad.ok, "empty table rejected");

    // Typed decode over synthetic value records: vtable at +0, scalar at +8, string text at +8,
    // fnv32 metadata at +0x30.
    uint8_t rec[0x40];
    SwoftyDbValue v;

    memset(rec, 0, sizeof rec);
    *reinterpret_cast<int32_t *>(rec + 8) = -1234;
    ok(dbread::read_i32(rec) == -1234, "read_i32");
    dbread::decode(rec, SWOFTY_DB_INT, &v);
    ok(v.tag == SWOFTY_DB_INT && v.i == -1234 && v.f == -1234.0, "decode int");

    memset(rec, 0, sizeof rec);
    *reinterpret_cast<float *>(rec + 8) = 0.5f;
    ok(dbread::read_f32(rec) == 0.5f, "read_f32");
    dbread::decode(rec, SWOFTY_DB_FLOAT, &v);
    ok(v.tag == SWOFTY_DB_FLOAT && v.f == 0.5 && v.i == 0, "decode float");

    memset(rec, 0, sizeof rec);
    rec[8] = 1;
    ok(dbread::read_u8(rec) == 1, "read_u8");
    dbread::decode(rec, SWOFTY_DB_BOOL, &v);
    ok(v.tag == SWOFTY_DB_BOOL && v.i == 1 && v.f == 1.0, "decode bool true");
    rec[8] = 0;
    dbread::decode(rec, SWOFTY_DB_BOOL, &v);
    ok(v.tag == SWOFTY_DB_BOOL && v.i == 0, "decode bool false");

    memset(rec, 0, sizeof rec);
    const char *text = "Lewis";
    *reinterpret_cast<const char **>(rec + 8) = text;
    *reinterpret_cast<uint32_t *>(rec + 0x30) = 0xdeadbeefu;
    ok(dbread::read_cstr(rec) == text && dbread::read_meta(rec) == 0xdeadbeefu, "read_cstr/read_meta");
    dbread::decode(rec, SWOFTY_DB_STRING, &v);
    ok(v.tag == SWOFTY_DB_STRING && v.s == text && uint32_t(v.i) == 0xdeadbeefu, "decode string");
    // ENUM and ID share the string record class; decode reports them as string.
    dbread::decode(rec, SWOFTY_DB_ENUM, &v);
    ok(v.tag == SWOFTY_DB_STRING && v.s == text, "decode enum as string family");

    printf("%s: %d failure(s)\n", failures ? "FAIL" : "ok  ", failures);
    return failures ? 1 : 0;
}
