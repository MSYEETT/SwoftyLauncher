// SPDX-License-Identifier: AGPL-3.0-or-later
//
// F1 1990 addon DLL entry. A refused init leaves every hook out, including the save separation, so
// the profile would read and write the real profile save: that is logged loudly.

#include <swofty/mixin.hpp>

extern "C" __declspec(dllexport) const uint32_t SwoftyAddonApi = SWOFTY_API_VERSION;

extern "C" __declspec(dllexport) int SwoftyAddonInit(const SwoftyApi *a)
{
    int ok = ::swofty::detail::init(a, nullptr);
    if (!ok && a && a->log)
        for (int i = 0; i < 3; i++)
            a->log("[f1-1990] SAVE SEPARATION OFF: the f1-1990 hooks did not install, so this profile uses the real "
                   "profile save (savegame@profile). Quit before opening any career.");
    return ok;
}
