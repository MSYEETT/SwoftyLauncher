// Host unit test for the era picker logic (era_logic.hpp): the box check, the default title and image,
// the box order and the press guard. run_events_test.sh builds and runs it.
#include "../src/core/era_logic.hpp"

#include <cstdio>
#include <string>
#include <vector>

namespace era = runtime::era;

static int fails;

static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

static era::Box box(const char *list, const char *title = "", const char *image = "", int32_t order = 0)
{
    return era::Box{list, title, image, order, "test", 0};
}

int main()
{
    std::vector<era::Box> have;

    era::Box a = box("cs_f1_2016", "lng_cs_f1_2016", "", 2016);
    expect(era::check(a, have).empty(), "a box with a list, a title and an order is accepted");
    expect(era::image_of(a) == "#overrides.home_screen.tiles_solo_6x6[cs_f1_2016]", "an empty image is the list's tiles_solo_6x6 entry");
    a.seq = have.size();
    have.push_back(a);

    era::Box untitled = box("cs_f1_2017");
    expect(era::check(untitled, have).empty() && untitled.title == "lng_cs_f1_2017", "an empty title is lng_<entrant list>");

    era::Box dup = box("cs_f1_2016", "lng_other");
    expect(!era::check(dup, have).empty(), "a second box for the same entrant list is refused");

    for (const char *bad : {"", "has space", "dots.in.name", "slash/name", "0123456789012345678901234567890123456789012345678901234567890123"}) {
        era::Box b = box(bad);
        expect(!era::check(b, have).empty(), std::string("entrant list '") + bad + "' is refused");
    }

    era::Box image = box("cs_f1_2018", "lng_cs_f1_2018", "#overrides.home_screen.tiles_solo_6x6[f1]");
    expect(era::check(image, have).empty() && era::image_of(image) == "#overrides.home_screen.tiles_solo_6x6[f1]",
           "a given image is kept");

    era::Box nul = box("cs_f1_2019");
    nul.title = std::string("lng_a\0b", 7);
    expect(!era::check(nul, have).empty(), "a title with a NUL byte is refused");

    std::vector<era::Box> full;
    for (size_t i = 0; i < era::MAX_BOXES; i++)
        full.push_back(box(("l" + std::to_string(i)).c_str()));
    era::Box over = box("one_more");
    expect(!era::check(over, full).empty(), "a box past the 64th is refused");

    std::vector<era::Box> mixed = {box("y2018", "", "", 2018), box("y2016", "", "", 2016), box("late_a"), box("y2017", "", "", 2017),
                                   box("late_b")};
    for (size_t i = 0; i < mixed.size(); i++)
        mixed[i].seq = i;
    std::vector<era::Box> sorted = era::ordered(mixed);
    std::string got;
    for (const auto &b : sorted)
        got += b.entrant_list + " ";
    expect(got == "late_a late_b y2016 y2017 y2018 ", "boxes sort by order, then by registration (" + got + ")");

    expect(era::press_refusal(true, true, 13, 11, 2).empty(), "a press with the last build's stock and addon tiles goes through");
    expect(!era::press_refusal(false, true, 13, 11, 2).empty(), "a press from another view model is refused");
    expect(!era::press_refusal(true, false, 13, 11, 2).empty(), "a press without the addon nodes is refused");
    expect(!era::press_refusal(true, true, 11, 11, 0).empty(), "a press with no addon boxes is refused");
    expect(!era::press_refusal(true, true, 12, 11, 2).empty(), "a press with fewer tiles than the last build is refused");

    expect(era::ADDON_PRESS == 0x8fb68dffu && era::SELECT_PRESS == 0x2b0357au && era::ADDON_TILES == 0xf2614e2bu,
           "djb2 of addon_select_pressed, select_pressed and addon_tiles");

    printf("%s\n", fails ? "era_logic_test: FAILED" : "era_logic_test: all passed");
    return fails ? 1 : 0;
}
