// Host unit test for the era picker logic (era_logic.hpp): the box check, the default title and image,
// the box order, the Time Trial list and car limit, the press guard, the addon row's handle links and the DRS
// group swap. run_events_test.sh builds and runs it.
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
    return era::Box{list, title, image, order, "test", 0, {}};
}

// A game setup mode object as Sess_Parse_GameSetupMode builds it: string fields are holders whose +0x10 is the label.
struct Mode {
    alignas(8) uint8_t mem[0x200] = {};
    era::DrsHolder own;
    explicit Mode(const char *group)
    {
        own.text = group;
        *reinterpret_cast<void **>(mem + era::MODE_DRS_GROUP) = &own;
    }
    void *field() const { return *reinterpret_cast<void *const *>(mem + era::MODE_DRS_GROUP); }
    std::string group() const { return era::holder_text(field()) ? era::holder_text(field()) : ""; }
};

static void drs_swap_tests()
{
    era::DrsSwaps swaps;
    Mode a("f1_2024"), b("f1_2024");
    era::DrsHolder *never = swaps.holder("swofty_drs_never_1a2b3c4d");
    expect(never && era::holder_text(never) == std::string("swofty_drs_never_1a2b3c4d") &&
               swaps.holder("swofty_drs_never_1a2b3c4d") == never,
           "one holder per label, at a stable address, its +0x10 the label");
    expect(!swaps.holder("") && !swaps.holder(std::string(64, 'x')), "an empty label or one past 63 characters has no holder");

    swaps.swap(a.mem, never);
    swaps.swap(b.mem, never);
    expect(a.group() == "swofty_drs_never_1a2b3c4d" && b.group() == "swofty_drs_never_1a2b3c4d" && swaps.active() == 2,
           "a press points every host mode's DRS group at the rule's group");

    era::DrsHolder *late = swaps.holder("swofty_drs_late_00ff00ff");
    swaps.swap(a.mem, late);
    expect(a.group() == "swofty_drs_late_00ff00ff" && swaps.active() == 2, "a second swap without a put back keeps the game's holder once");
    expect(swaps.put_back() == 2 && a.field() == &a.own && b.field() == &b.own && swaps.active() == 0,
           "put back restores the game's holder in every swapped mode");
    expect(swaps.put_back() == 0 && a.group() == "f1_2024", "a second put back changes nothing");

    swaps.swap(a.mem, never);
    era::DrsHolder other;
    other.text = "f2_2023";
    *reinterpret_cast<void **>(a.mem + era::MODE_DRS_GROUP) = &other;
    expect(swaps.put_back() == 0 && a.group() == "f2_2023", "put back leaves a field the game changed since the swap");

    swaps.swap(nullptr, never);
    swaps.swap(b.mem, nullptr);
    expect(swaps.active() == 0 && b.field() == &b.own, "no mode or no holder: nothing is swapped");
    expect(era::MODE_DRS_GROUP == 0x1e0 && era::MODE_NAME == 0xd8 && era::HOLDER_TEXT == 0x10,
           "GameSetupMode +0x1e0 DRS group and +0xd8 name holders, text at holder +0x10 (re/db structs/GameSetupMode.h)");
}

// A VM node as the game constructs a reference node (0x14083c570): its own handle at +0x8 heads its holder list.
struct Node {
    alignas(8) uint8_t mem[0x30] = {};
    Node()
    {
        self() = {mem, nullptr, nullptr};
        holders() = &self();
        refs() = 2;
    }
    era::Handle &self() { return *reinterpret_cast<era::Handle *>(mem + 8); }
    void *&holders() { return *reinterpret_cast<void **>(mem + era::NODE_HOLDERS); }
    int16_t &refs() { return *reinterpret_cast<int16_t *>(mem + era::NODE_REFS); }
};

static Node &node_of(uint8_t *mem)
{
    return *reinterpret_cast<Node *>(mem);
}

struct Set {
    era::Handle list, selected;
};

// The handle part of FeTile_Clear_EraTileSet_Thunk (0x14bd31910): unlink from the holder list, release.
static void game_unlink(era::Handle &h)
{
    if (!h.node)
        return;
    auto *next = static_cast<era::Handle *>(h.next), *prev = static_cast<era::Handle *>(h.prev);
    if (prev)
        prev->next = next;
    if (next)
        next->prev = prev;
    if (node_of(h.node).holders() == &h)
        node_of(h.node).holders() = next;
    node_of(h.node).refs()--;
    h = {};
}

// The holder walk of the game's node teardown (0x1442b3e30): every holder but the node's own is cut loose
// and releases the node it holds.
static void game_teardown(Node &n)
{
    for (void *p = n.holders(); p;) {
        auto *h = static_cast<era::Handle *>(p);
        p = h->next;
        h->next = h->prev = nullptr;
        if (h == &n.self() || !h->node)
            continue;
        if (node_of(h->node).holders() == h)
            node_of(h->node).holders() = nullptr;
        node_of(h->node).refs()--;
        h->node = nullptr;
    }
}

static std::vector<era::Handle *> chain(Node &n)
{
    std::vector<era::Handle *> out;
    for (void *p = n.holders(); p && out.size() < 16; p = static_cast<era::Handle *>(p)->next)
        out.push_back(static_cast<era::Handle *>(p));
    return out;
}

static bool consistent(Node &n)
{
    void *prev = nullptr;
    for (era::Handle *h : chain(n)) {
        if (h->prev != prev || h->node != n.mem)
            return false;
        prev = h;
    }
    return chain(n).size() < 16;
}

static bool holds(Node &n, const era::Handle &h)
{
    for (era::Handle *p : chain(n))
        if (p == &h)
            return true;
    return false;
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

    {
        Node a_list, a_sel, b_list, b_sel;
        era::Handle a_ui{}, b_ui{};
        era::link(a_ui, a_list.mem);
        era::link(b_ui, b_list.mem);
        int16_t a0 = a_list.refs(), as0 = a_sel.refs(), b0 = b_list.refs(), bs0 = b_sel.refs();
        Set set{};
        int clears = 0;
        auto clear = [&](Set &s) {
            game_unlink(s.list);
            game_unlink(s.selected);
            clears++;
        };
        auto build = [&](Node &list, Node &sel) {
            era::relink(set, list.mem, clear);
            era::link(set.selected, sel.mem);
        };

        build(a_list, a_sel);
        expect(!clears && a_list.holders() == &set.list && a_sel.holders() == &set.selected && consistent(a_list) &&
                   consistent(a_sel) && a_list.refs() == a0 + 1,
               "the first build links the set at the head of A's holder lists");

        build(b_list, b_sel);
        expect(clears == 1, "a build for view model B clears the set that A's nodes still hold");
        expect(!holds(a_list, set.list) && !holds(a_sel, set.selected) && consistent(a_list) && consistent(a_sel) &&
                   chain(a_list).size() == 2 && a_list.refs() == a0 && a_sel.refs() == as0,
               "A's nodes no longer reference the set and have their references back");
        expect(b_list.holders() == &set.list && b_sel.holders() == &set.selected && consistent(b_list) &&
                   consistent(b_sel) && b_list.refs() == b0 + 1 && b_sel.refs() == bs0 + 1,
               "B's nodes hold the set once, with consistent holder lists");

        game_teardown(a_list);
        game_teardown(a_sel);
        expect(set.list.node == b_list.mem && set.selected.node == b_sel.mem && b_sel.self().node == b_sel.mem &&
                   consistent(b_list) && consistent(b_sel) && b_list.refs() == b0 + 1 && b_sel.refs() == bs0 + 1,
               "tearing A down leaves B's nodes and the set alone");

        build(b_list, b_sel);
        expect(clears == 2 && chain(b_list).size() == 3 && consistent(b_list) && b_list.refs() == b0 + 1,
               "a rebuild of B links the set once");

        game_teardown(b_list);
        game_teardown(b_sel);
        expect(!set.list.node && !set.list.next && !set.selected.node && !set.selected.prev,
               "tearing B down unlinks the set and releases its nodes");
        Node c_list, c_sel;
        build(c_list, c_sel);
        expect(clears == 2 && consistent(c_list) && consistent(c_sel) && c_sel.holders() == &set.selected,
               "a build after the game tore the set's nodes down links them without a clear");

        Node old_a, old_b;
        Set old{};
        era::link(old.selected, old_a.mem);
        old = {};
        era::link(old.selected, old_b.mem);
        int16_t ob = old_b.refs();
        game_teardown(old_a);
        expect(old_b.refs() < ob && !old_b.self().node,
               "1.5.4's reset left A's holder list running into B's, so tearing A down released B's node");
    }

    {
        std::vector<era::Box> none;
        era::Box tt = box("bdx_2012", "lng_bdx_2012", "", 2012);
        tt.time_trial = "bdx_2012_tt";
        expect(era::check(tt, none).empty() && tt.time_trial == "bdx_2012_tt", "a box with a Time Trial list is accepted");
        expect(era::list_for(tt, true) == "bdx_2012_tt" && era::list_for(tt, false) == "bdx_2012",
               "the Time Trial picker uses the box's Time Trial list, the Grand Prix picker its own");
        era::Box plain = box("cs_f1_2016");
        expect(era::check(plain, none).empty() && era::list_for(plain, true) == "cs_f1_2016",
               "a box without a Time Trial list uses its own list in Time Trial");
        era::Box same = box("cs_f1_2017");
        same.time_trial = "cs_f1_2017";
        expect(era::check(same, none).empty() && same.time_trial.empty(), "a Time Trial list equal to the box's list is dropped");
        era::Box bad = box("cs_f1_2018");
        bad.time_trial = "has space";
        expect(!era::check(bad, none).empty(), "a Time Trial list that is not a plain name is refused");
        std::vector<era::Box> shared = {tt};
        era::Box other = box("bdx_2012_alt");
        other.time_trial = "bdx_2012_tt";
        expect(era::check(other, shared).empty(), "two boxes may share a Time Trial list");

        expect(era::TT_MAX_CARS == 23, "Time Trial holds 23 cars of a list (26 slots, 3 the game adds)");
        expect(era::time_trial_refusal(plain, 22).empty() && era::time_trial_refusal(plain, 23).empty(),
               "a list of 22 or 23 cars is offered in Time Trial");
        std::string why = era::time_trial_refusal(plain, 24);
        expect(why.find("cs_f1_2016 holds 24 cars") != std::string::npos && why.find("at most 23") != std::string::npos &&
                   why.find("Give the box a Time Trial entrant list") != std::string::npos,
               "a 24-car list without a Time Trial list is refused, naming the fix (" + why + ")");
        why = era::time_trial_refusal(tt, 24);
        expect(why.find("bdx_2012_tt holds 24 cars") != std::string::npos && why.find("Give the box") == std::string::npos,
               "an oversized Time Trial list is refused by its own name (" + why + ")");
        expect(era::time_trial_refusal(plain, -1).empty(), "an unread car count refuses nothing");

        expect(era::entrant_count(0x1000, 0x1000 + 24 * 0x88) == 24, "24 entrant records of 0x88 bytes count 24");
        expect(era::entrant_count(0x1000, 0x1000) == 0, "an empty list counts 0");
        expect(era::entrant_count(0, 0x88) < 0 && era::entrant_count(0x2000, 0x1000) < 0 &&
                   era::entrant_count(0x1000, 0x1000 + 0x89) < 0 && era::entrant_count(0x1000, 0x1000 + 65 * 0x88) < 0,
               "a null, reversed, misaligned or over-long entrant vector is unread");

        std::vector<era::ListCars> host = {{"cs_f1_2016", 22}, {"cs_f1_2017", 20}, {"bdx_s5_2012", 24}, {"unread", -1}};
        expect(!era::oversized(23) && era::oversized(24) && !era::oversized(-1), "past 23 cars is oversized, an unread count is not");
        auto keep = era::time_trial_keep(host);
        expect(keep == std::vector<size_t>{0, 1, 3},
               "a stock press in Time Trial keeps swofty_f1's lists of at most 23 cars and the unread ones, and drops the 24-car list");
        std::vector<era::ListCars> fine = {{"cs_f1_2016", 22}, {"cs_f1_2017", 20}};
        expect(era::time_trial_keep(fine).size() == fine.size(), "lists that all fit are all kept");
        std::vector<era::ListCars> big = {{"a", 24}, {"b", 30}};
        expect(era::time_trial_keep(big).empty(), "lists that all overflow keep nothing (the core then leaves them offered)");

        std::vector<era::ListCars> esm = {{"bdx_s5_2012", 24}, {"cs_f1_2016", 22}, {"f1_2025", 20}, {"tt_list", 12}};
        expect(era::time_trial_fallback(esm, {"f1_2025"}) == 2, "an oversized session list falls back to the box pressed");
        expect(era::time_trial_fallback(esm, {"bdx_s5_2012", "tt_list"}) == 3,
               "an oversized box pressed falls back to its Time Trial list");
        expect(era::time_trial_fallback(esm, {"missing"}) == 1, "with no preferred list, the first list that fits");
        expect(era::time_trial_fallback(esm, {}) == 1, "with no box pressed, the first list that fits");
        std::vector<era::ListCars> none_fit = {{"bdx_s5_2012", 24}, {"unread", -1}};
        expect(era::time_trial_fallback(none_fit, {"unread"}) == -1, "an unread list is never a fallback, nor is an oversized one");

        std::string swap = era::time_trial_swap("bdx_s5_2012", 24, "swofty_f1", "cs_f1_2016", 0);
        expect(swap.find("bdx_s5_2012 (setup mode swofty_f1) holds 24 cars") != std::string::npos &&
                   swap.find("at most 23") != std::string::npos && swap.find("loading cs_f1_2016 instead") != std::string::npos &&
                   swap.find("max_entrants") == std::string::npos,
               "the swap line names the list, its setup mode, its count and the list loaded instead (" + swap + ")");
        swap = era::time_trial_swap("bdx_s5_2012", 24, "bdx_2012_mode", "", 24);
        expect(swap.find("offers no list of at most 23 cars") != std::string::npos &&
                   swap.find("max_entrants 24 -> 23 until the session ends") != std::string::npos,
               "with nothing to load instead, the line says the session draws at most 23 cars (" + swap + ")");

        expect(era::time_trial_limit(24) == 23 && era::time_trial_limit(-1) == 23 && era::time_trial_limit(0) == 23,
               "an entrant setup mode allowing more than 23 cars, or any number, is held to 23 for a Time Trial session");
        expect(era::time_trial_limit(23) == 23 && era::time_trial_limit(20) == 20 && era::time_trial_limit(5) == 5,
               "a limit of at most 23 is left as it is");

        uintptr_t b = 0x10000;
        expect(era::session_vehicles(b, b + 27 * 8) == 27 && era::session_vehicles(b, b + 25 * 8) == 25 &&
                   era::session_vehicles(b, b) == 0 && era::session_vehicles(b + 8, b) == 0,
               "the session's vehicle count is its pointer vector's length, 0 when empty or reversed");
        expect(era::vehicles_end_within_table(b, b + 27 * 8) == b + 26 * 8,
               "27 vehicles (a 24-car list plus the game's 3) are handed over as the first 26, the table's size");
        expect(era::vehicles_end_within_table(b, b + 26 * 8) == b + 26 * 8 && era::vehicles_end_within_table(b, b + 25 * 8) == b + 25 * 8,
               "26 vehicles or fewer are handed over as they are");

        for (const char *m : {"time_trial", "solo_testing"})
            expect(era::time_trial_mode(m), std::string(m) + " is a Time Trial picker");
        for (const char *m : {"grand_prix_23", "grand_prix_splitscreen", "", "my_team_career_25"})
            expect(!era::time_trial_mode(m), std::string("'") + m + "' is not a Time Trial picker");
    }

    expect(era::ADDON_PRESS == 0x8fb68dffu && era::SELECT_PRESS == 0x2b0357au && era::ADDON_TILES == 0xf2614e2bu,
           "djb2 of addon_select_pressed, select_pressed and addon_tiles");

    drs_swap_tests();

    printf("%s\n", fails ? "era_logic_test: FAILED" : "era_logic_test: all passed");
    return fails ? 1 : 0;
}
