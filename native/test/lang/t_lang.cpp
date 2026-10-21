// Test addon for run_lang_override_test.sh: fake_lang.exe calls t_lang_step(n) to drive the
// runtime string override API.
#include <swofty/lang.hpp>
#include <swofty/mixin.hpp>

namespace lg = swofty::lang;

extern "C" __declspec(dllexport) int t_lang_step(int n)
{
    switch (n) {
    case 0:
        return lg::available() ? 1 : 0;
    case 1: {
        int bad = 0;
        bad += lg::set("lng_mode_career", "1990 CAREER") != lg::Result::ok;
        bad += lg::set("lng_press_start", "ENG ONLY", "eng") != lg::Result::ok;
        bad += lg::set("lng_other", "FRE ONLY", "FRE") != lg::Result::ok;
        bad += lg::set("lng_new_key", "BRAND NEW") != lg::Result::ok;
        return bad;
    }
    case 2:
        return int(lg::clear("lng_mode_career"));
    case 3:
        return int(lg::refresh());
    case 4:
        return int(lg::clear_all());
    case 5:
        return int(lg::set("", "x"));
    case 6:
        return int(lg::set("lng_mode_career", "SAME TEXT"));
    case 7:
        return int(lg::clear("lng_mode_career")) + int(lg::set("lng_mode_career", "SAME TEXT"));
    }
    return -1;
}

SWOFTY_ADDON()
