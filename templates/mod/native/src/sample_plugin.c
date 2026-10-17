#include <stdint.h>
#include "swofty_api.h"

SWOFTY_EXPORT int SwoftyModInit(const SwoftyApi *api)
{
    if (!api || api->version < 1)
        return 0;
    api->log("[sample] loaded, game image at %p", (void *)api->module_base());
    uintptr_t hit = 0;
    int n = api->scan(".xtls", "48 89 5C 24 ?? 57 48 83 EC 20", &hit);
    api->log("[sample] example signature: %d hit(s), first at %p", n, (void *)hit);
    return 1;
}
