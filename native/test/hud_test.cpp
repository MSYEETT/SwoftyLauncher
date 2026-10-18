// Host-native unit test for the portable overlay draw-list core (native/src/core/hud_drawlist.hpp).
// The D3D12 present hook and GPU renderer can only run in game; this covers everything that does
// not: the draw-list building, the per-addon paint registration and token lifecycle, text
// measurement and the image id registry. Built and run by native/test/run_hud_test.sh.

#include "hud_drawlist.hpp"

#include <cstdio>
#include <string>

static int failures;

#define CHECK(cond, msg)                                                                                                \
    do {                                                                                                                \
        if (cond) {                                                                                                     \
            printf("ok  : %s\n", msg);                                                                                  \
        } else {                                                                                                        \
            printf("FAIL: %s\n", msg);                                                                                  \
            failures++;                                                                                                 \
        }                                                                                                               \
    } while (0)

static void paint_box(SwoftyPaint *paint, void *user)
{
    auto &f = *reinterpret_cast<hud::Frame *>(paint);
    int tag = int(reinterpret_cast<intptr_t>(user));
    hud::rect(f, 10, 20, 30, 40, 0x112233ff);
    hud::text(f, 1, 2, 16, 0xffffffff, tag == 7 ? "seven" : "x");
}

static void paint_nothing(SwoftyPaint *paint, void *user)
{
    (void)paint;
    (void)user;
}

static void test_measure()
{
    CHECK(hud::glyphs("abc") == 3, "glyphs counts ascii");
    CHECK(hud::glyphs("") == 0, "glyphs of empty is zero");
    // "café" is 5 bytes (é = 2 bytes) but 4 codepoints.
    CHECK(hud::glyphs("caf\xc3\xa9") == 4, "glyphs counts utf-8 codepoints, not bytes");
    float w = -1, h = -1;
    hud::measure(16, "abcd", &w, &h);
    CHECK(w == 4 * 16 * (hud::FONT_ADVANCE / hud::FONT_CELL), "measure width is glyphs * advance");
    CHECK(h == 16, "measure height is the font size");
    hud::measure(16, "", &w, &h);
    CHECK(w == 0 && h == 16, "measure of empty text is zero wide");
}

static void test_culling()
{
    hud::Frame f;
    hud::rect(f, 0, 0, 0, 10, 0xff0000ff);
    CHECK(f.cmds.empty(), "zero-width rect is dropped");
    hud::rect(f, 0, 0, 10, 10, 0x00000000);
    CHECK(f.cmds.empty(), "fully transparent rect is dropped");
    hud::line(f, 5, 5, 5, 5, 1, 0xffffffff);
    CHECK(f.cmds.empty(), "zero-length line is dropped");
    hud::text(f, 0, 0, 16, 0xffffffff, "");
    CHECK(f.cmds.empty(), "empty text is dropped");
    hud::rect_outline(f, 0, 0, 10, 10, 0, 0xffffffff);
    CHECK(f.cmds.empty(), "zero-thickness outline is dropped");
}

static void test_commands()
{
    hud::Frame f;
    hud::rect(f, 10, 20, 30, 40, 0x11223344);
    hud::line(f, 1, 2, 3, 4, 2.5f, 0xaabbccdd);
    hud::text(f, 7, 8, 18, 0xffffffff, "hi");
    hud::image(f, 3, 100, 100, 64, 64, 0x80808080);
    CHECK(f.cmds.size() == 4, "four visible draw calls appended four commands");
    CHECK(f.cmds[0].op == hud::Op::rect && f.cmds[0].x == 10 && f.cmds[0].h == 40 && f.cmds[0].color == 0x11223344,
          "rect command keeps its coordinates and colour");
    CHECK(f.cmds[1].op == hud::Op::line && f.cmds[1].thickness == 2.5f, "line command keeps its thickness");
    CHECK(f.cmds[2].op == hud::Op::text && f.cmds[2].text == "hi" && f.cmds[2].font_size == 18,
          "text command keeps its string and size");
    CHECK(f.cmds[3].op == hud::Op::image && f.cmds[3].image == 3 && f.cmds[3].color == 0x80808080,
          "image command keeps its slot and tint");
}

static void test_registry()
{
    hud::Registry reg;
    CHECK(reg.add(nullptr, nullptr, "a") == 0, "on_paint with a null callback yields no token");
    CHECK(reg.count() == 0, "a refused registration is not stored");
    uint32_t a = reg.add(paint_box, nullptr, "a");
    uint32_t b = reg.add(paint_nothing, nullptr, "b");
    CHECK(a != 0 && b != 0 && a != b, "two registrations get distinct non-zero tokens");
    CHECK(!reg.remove(9999), "removing an unknown token fails");
    CHECK(reg.remove(a), "removing a known token succeeds");
    CHECK(reg.count() == 1, "remove drops exactly one subscription");
    CHECK(!reg.remove(a), "a token cannot be removed twice");
}

static void test_build_dispatch()
{
    hud::Registry reg;
    reg.add(paint_box, reinterpret_cast<void *>(intptr_t(7)), "a");
    reg.add(paint_box, reinterpret_cast<void *>(intptr_t(1)), "b");
    hud::Frame f;
    hud::build(reg.subs(), f, 1920, 1080);
    CHECK(f.width == 1920 && f.height == 1080, "build records the drawable size for size()");
    CHECK(f.cmds.size() == 4, "both callbacks ran and each drew a rect and text");
    CHECK(f.cmds[1].op == hud::Op::text && f.cmds[1].text == "seven",
          "the first subscriber's user pointer reached its callback");
    CHECK(f.cmds[3].text == "x", "subscribers run in registration order");
    // A second frame starts clean.
    hud::build(reg.subs(), f, 800, 600);
    CHECK(f.cmds.size() == 4 && f.width == 800, "each frame rebuilds the command list from scratch");
}

static void test_image_registry()
{
    hud::Registry reg;
    int first = reg.intern_image("logo");
    int again = reg.intern_image("logo");
    int other = reg.intern_image("flag");
    CHECK(first == 0 && again == 0, "interning the same id twice returns the same slot");
    CHECK(other == 1, "a new id gets a new slot");
    CHECK(reg.find_image("logo") == 0, "find_image returns a registered slot");
    CHECK(reg.find_image("missing") == -1, "find_image returns -1 for an unknown id");
    CHECK(reg.image_count() == 2, "two distinct images registered");
}

int main()
{
    test_measure();
    test_culling();
    test_commands();
    test_registry();
    test_build_dispatch();
    test_image_registry();
    if (failures)
        printf("\n%d check(s) FAILED\n", failures);
    else
        printf("\nall hud draw-list checks passed\n");
    return failures ? 1 : 0;
}
