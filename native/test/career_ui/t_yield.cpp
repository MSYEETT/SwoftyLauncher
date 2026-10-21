// Test addon for run_career_ui_test.sh: registers start_in_f1 from its init, so the core's start in
// F1 wrap joins this commit, and puts a site of its own on the same entry. The two overlap; only the
// core's set may be dropped.
#include <swofty_api.h>

namespace {

void entry(SwoftyContext *) {}

}

extern "C" __declspec(dllexport) const uint32_t SwoftyAddonApi = SWOFTY_API_VERSION;

extern "C" __declspec(dllexport) int SwoftyAddonInit(const SwoftyApi *a)
{
    if (!a || a->version < 8 || !a->register_mixins || !a->career_wizard)
        return 0;
    int r = a->career_wizard->starts_in_f1("f1_career_1990", 1);
    a->log("[t_yield] start_in_f1 at init -> %d", r);
    const SwoftyMixin m{"site_Test_StartsInF1Entry", SWOFTY_MIXIN_SITE, 0, (void *)&entry, nullptr};
    return a->register_mixins(&m, 1);
}
