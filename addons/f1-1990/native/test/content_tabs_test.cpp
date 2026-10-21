// SPDX-License-Identifier: AGPL-3.0-or-later
#include <cstdio>
#include <string>

#include "content_tabs.hpp"

static int failures;

static void check(const char *name, bool ok)
{
    std::printf("%s %s\n", ok ? "ok  " : "FAIL", name);
    failures += !ok;
}

template <class T> static void put(void *p, size_t off, T v)
{
    std::memcpy(static_cast<uint8_t *>(p) + off, &v, sizeof v);
}

int main()
{
    const char *names[] = {"header_f1", "header_selected_team_icon", "header_f2", "header_icon"};
    std::vector<uint8_t> strings[4], entries(4 * f1990::kContentTabSize), group(0x20);
    for (int i = 0; i < 4; i++) {
        strings[i].assign(9, 0);
        strings[i].insert(strings[i].end(), names[i], names[i] + std::strlen(names[i]) + 1);
        put(entries.data() + i * f1990::kContentTabSize, 0x08, strings[i].data());
        entries[i * f1990::kContentTabSize + 0x28] = uint8_t(i);
    }
    put(group.data(), 0x08, entries.data());
    put(group.data(), 0x10, entries.data() + entries.size());

    const char *const drop[] = {"header_f2", "header_icon"};
    std::vector<uint8_t> out;
    int n = f1990::drop_content_tabs(group.data(), drop, out);
    check("drops the F2 and Icons tabs", n == 2 && out.size() == 2 * f1990::kContentTabSize);
    check("keeps the F1 tab first", !std::strcmp(f1990::content_tab_name(out.data()), "header_f1"));
    check("keeps the selected team tab second",
          !std::strcmp(f1990::content_tab_name(out.data() + f1990::kContentTabSize), "header_selected_team_icon"));
    check("copies whole entries", out[0x28] == 0 && out[f1990::kContentTabSize + 0x28] == 1);

    const char *const none[] = {"header_mp"};
    check("drops nothing that isn't there", f1990::drop_content_tabs(group.data(), none, out) == 0 &&
                                                out.size() == entries.size());
    put(group.data(), 0x10, entries.data() + entries.size() - 1);
    check("rejects a group that is not whole entries", f1990::drop_content_tabs(group.data(), drop, out) == -1);

    std::printf("%s (%d failure(s))\n", failures ? "FAILED" : "ALL PASSED", failures);
    return failures != 0;
}
