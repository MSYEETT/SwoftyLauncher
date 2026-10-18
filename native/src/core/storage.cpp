#include "runtime.hpp"

#include <swofty/mixin.hpp>

// Stub for the storage lane. Every call returns unavailable until the lane identifies the
// calling DLL (as the assets API does) and reads/writes under swofty/addons/<id>/.

namespace {

int st_available()
{
    return 0;
}

int st_read(const char *, void *, size_t, size_t *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int st_write(const char *, const void *, size_t)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int st_append(const char *, const void *, size_t)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int st_remove(const char *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int st_size(const char *, uint64_t *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int st_list(const char *, char *, size_t, size_t *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

const SwoftyStorage table = {st_available, st_read, st_write, st_append, st_remove, st_size, st_list};

}

namespace runtime {

const SwoftyStorage *storage_table()
{
    return &table;
}

}
