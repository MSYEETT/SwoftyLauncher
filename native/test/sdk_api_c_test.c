/* swofty_api.h compiles as C and keeps the version 9 layout: the 1.6 tables are appended after every
   existing SwoftyApi field. */
#include <stddef.h>
#include <stdio.h>
#include <swofty_api.h>

_Static_assert(SWOFTY_API_VERSION == 9, "API version 9");
_Static_assert(offsetof(SwoftyApi, era) == 168, "era stays where version 8 put it");
_Static_assert(offsetof(SwoftyApi, registry) == 176, "registry appended after era");
_Static_assert(offsetof(SwoftyApi, hooks) == 184, "hooks after registry");
_Static_assert(offsetof(SwoftyApi, services) == 192, "services after hooks");
_Static_assert(sizeof(SwoftyApi) == 200, "SwoftyApi version 9 size");
_Static_assert(offsetof(SwoftyRegistry, migrate) == 8 * 23, "the documented registry table comes first");
_Static_assert(offsetof(SwoftyRegistry, layer_ex) == 8 * 24, "layer_ex appended after migrate");
_Static_assert(offsetof(SwoftyRegistry, stat_id) == 8 * 31, "stat_id is the last version 1 field");
_Static_assert(offsetof(SwoftyHooks, fire) == 48 && offsetof(SwoftyHooks, context_in) == 80, "hooks table layout");
_Static_assert(sizeof(SwoftyServices) == 32, "services table layout");
_Static_assert(offsetof(SwoftyHookCall, context) == 32 && sizeof(SwoftyHookCall) == 40, "hook call layout");
_Static_assert(sizeof(SwoftyLayerDesc) == 128, "layer descriptor layout");
_Static_assert(sizeof(SwoftyText) == 16, "text layout");
_Static_assert(SWOFTY_TOO_SMALL == 4 && SWOFTY_SCOPE_CAREER == 2 && SWOFTY_PHASE_AFTER == 2, "codes");

static int phase_count(void) { return SWOFTY_PHASE_BEFORE + SWOFTY_PHASE_REPLACE + SWOFTY_PHASE_AFTER; }

int main(void)
{
    SwoftyApi api = {0};
    SwoftyCoreRegistryFn r = 0;
    SwoftyCoreHooksFn h = 0;
    SwoftyCoreServicesFn s = 0;
    SwoftyAddonRegisterFn reg = 0;
    api.version = SWOFTY_API_VERSION;
    if (api.registry || api.hooks || api.services || r || h || s || reg || phase_count() != 3)
        return 1;
    printf("sdk_api_c_test: ok\n");
    return 0;
}
