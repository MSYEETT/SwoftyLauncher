// Host unit test for the era picker logic (era_logic.hpp): the box check, the default title and image,
// the box order, the press guard and the addon row's handle links. run_events_test.sh builds and runs it.
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

    expect(era::ADDON_PRESS == 0x8fb68dffu && era::SELECT_PRESS == 0x2b0357au && era::ADDON_TILES == 0xf2614e2bu,
           "djb2 of addon_select_pressed, select_pressed and addon_tiles");

    printf("%s\n", fails ? "era_logic_test: FAILED" : "era_logic_test: all passed");
    return fails ? 1 : 0;
}
