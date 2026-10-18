#include "runtime.hpp"

#include <swofty/mixin.hpp>

// Stub for the database-read lane. Every call returns unavailable until the lane resolves the
// loaded database and reads through Gdb_Lookup_Key, Gdb_Get_* and the table cursor.

namespace {

int db_available()
{
    return 0;
}

int db_get(const char *, SwoftyDbValue *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int db_get_hash(uint64_t, SwoftyDbValue *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int db_int_at(const char *, int64_t *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int db_float_at(const char *, double *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int db_string_at(const char *, const char **)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int db_bool_at(const char *, int32_t *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int db_open(const char *, SwoftyDbCursor *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int db_row(const char *, const char *, SwoftyDbCursor *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int db_step(SwoftyDbCursor *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int db_cell(const SwoftyDbCursor *, const char *, SwoftyDbValue *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

const char *db_row_name(const SwoftyDbCursor *)
{
    return nullptr;
}

const SwoftyDb table = {db_available, db_get,      db_get_hash, db_int_at, db_float_at, db_string_at,
                        db_bool_at,   db_open,     db_row,      db_step,   db_cell,     db_row_name};

}

namespace runtime {

const SwoftyDb *db_table()
{
    return &table;
}

}
