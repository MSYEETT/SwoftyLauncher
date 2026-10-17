#include "wine_test.hpp"

SWOFTY_WRAP_PRIORITY(100, FeHome_Handle_Command, uint64_t, FEHomeViewModel *, NeStr **)
{
    return 888;
}

SWOFTY_ON_LOAD
{
    probe()[5] = Fake_Missing() + 1;
}

SWOFTY_ADDON()
