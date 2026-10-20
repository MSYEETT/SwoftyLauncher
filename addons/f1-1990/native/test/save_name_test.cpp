// SPDX-License-Identifier: AGPL-3.0-or-later
#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "save_name.hpp"

static int failures;

static void expect(const char *in, const char *want)
{
    const char *got = f1990::save_name_for(in);
    bool ok = (!got && !want) || (got && want && !std::strcmp(got, want));
    std::printf("%s %s -> %s\n", ok ? "ok  " : "FAIL", in ? in : "(null)", got ? got : "(null)");
    failures += !ok;
}

int main()
{
    expect("profile", "f1990");
    expect("highlights001", "highlights001");
    expect("profile1", "profile1");
    expect("Profile", "Profile");
    expect("", "");
    expect(nullptr, nullptr);
    const char *same = "highlights002";
    if (f1990::save_name_for(same) != same) {
        std::printf("FAIL other names keep their pointer\n");
        failures++;
    }
    for (const char *c = f1990::kProfileSave1990; *c; c++)
        if (!((*c >= 'a' && *c <= 'z') || (*c >= '0' && *c <= '9'))) {
            std::printf("FAIL 1990 save name is not lowercase alphanumeric\n");
            failures++;
        }
    if (!std::strncmp(f1990::kProfileSave1990, "profile", 7) || !std::strncmp(f1990::kProfileSave1990, "highlights", 10)) {
        std::printf("FAIL 1990 save name shares a stock prefix\n");
        failures++;
    }
    if (failures)
        std::printf("%d failure(s)\n", failures);
    return failures ? EXIT_FAILURE : EXIT_SUCCESS;
}
