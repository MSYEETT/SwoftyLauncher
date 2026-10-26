// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career: the contract scene a follow_driver career skips (progressive_follow.dll). Pure
// logic, built on the host by native/test.
//
//   Skip      armed by the negotiation decision once follow.cpp has signed the contract: where it
//             happened (the career's start, with no contract yet, or the end of his contract), the
//             team, year and target signed, and which steps of the scene passed through (team
//             select, the agent intro, the contract terms, the outro).
//   summary   the text of the log line the outro writes.

#pragma once
#include <cstdint>
#include <cstdio>
#include <string>

namespace progressive::scene {

enum Step : unsigned {
    kTeamSelect = 1u << 0,
    kIntro = 1u << 1,
    kTerms = 1u << 2,
    kOutro = 1u << 3,
};

struct Skip {
    bool on = false;
    bool update_skipped = false;
    bool contract_end = false;
    uint32_t team = 0;
    int year = 0;
    int target = 0;
    unsigned steps = 0;

    void arm(bool end, uint32_t team_key, int season_year, int target_rating)
    {
        *this = Skip{};
        on = true;
        contract_end = end;
        team = team_key;
        year = season_year;
        target = target_rating;
    }

    void mark(Step s)
    {
        steps |= s;
    }

    void clear()
    {
        *this = Skip{};
    }

    std::string where() const
    {
        return contract_end ? "end of his contract" : "career start, no contract yet";
    }

    std::string passed() const
    {
        static constexpr struct {
            Step step;
            const char *name;
        } kNames[] = {{kTeamSelect, "team select"}, {kIntro, "agent intro"}, {kTerms, "contract terms"}, {kOutro, "outro"}};
        std::string out;
        for (auto &n : kNames)
            if (steps & n.step) {
                if (!out.empty())
                    out += ", ";
                out += n.name;
            }
        return out.empty() ? "no step reached" : out;
    }

    std::string summary() const
    {
        char b[160];
        if (year > 0)
            std::snprintf(b, sizeof b, "), signed team %u for %d (one season, target %d)", team >> 16, year, target);
        else
            std::snprintf(b, sizeof b, "), signed team %u (one season, target %d)", team >> 16, target);
        return "contract scene skipped (" + where() + ": " + passed() + b;
    }
};

}
