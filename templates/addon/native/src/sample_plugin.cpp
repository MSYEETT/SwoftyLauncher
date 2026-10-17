#include <swofty/mixin.hpp>

SWOFTY_SETTING(bool, verbose, false);
SWOFTY_SETTING(int, greeting_count, 1);
SWOFTY_SETTING(std::string_view, greeting, "hello");

SWOFTY_ON_CHANGE(verbose, bool on)
{
    swofty::log("[sample] verbose logging %s", on ? "on" : "off");
}

SWOFTY_ON_CHANGE(greeting, std::string_view text)
{
    if (verbose)
        swofty::log("[sample] greeting is now \"%.*s\"", int(text.size()), text.data());
}

SWOFTY_ON_LOAD
{
    std::string_view text = greeting;
    for (int i = 0; i < greeting_count; i++)
        swofty::log("[sample] %.*s from the sample mod", int(text.size()), text.data());
    uintptr_t hit = 0;
    int n = swofty::api->scan(".xtls", "48 89 5C 24 ?? 57 48 83 EC 20", &hit);
    if (verbose)
        swofty::log("[sample] example signature: %d hit(s), first at %p", n, reinterpret_cast<void *>(hit));
}

SWOFTY_ADDON()
