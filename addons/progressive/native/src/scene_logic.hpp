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
//   route     what ContractNegotiationsDecision's link leads to: 0x5e3c3c2d when he has no contract
//             yet (or it runs past this season), the scene from the agent intro; 0xd2985448 when his
//             contract ends this season, the scene from the team select; 0x7c9b1ec4 (no contracts
//             component) no scene.
//   tried     a scene is signed for at most once: the decision, or the first scene step entered
//             without it (team select, agent intro), whichever comes first.

#pragma once
#include <cstdint>
#include <cstdio>
#include <string>

namespace progressive::scene {

constexpr uint32_t kLinkNoContract = 0x5e3c3c2d;
constexpr uint32_t kLinkContractEnds = 0xd2985448;

enum class Route { none, no_contract, contract_ends };

inline Route route(uint32_t link)
{
    return link == kLinkNoContract ? Route::no_contract : link == kLinkContractEnds ? Route::contract_ends : Route::none;
}

inline const char *route_name(Route r)
{
    switch (r) {
    case Route::no_contract:
        return "no contract yet: the scene starts at the agent intro";
    case Route::contract_ends:
        return "his contract ends: the scene starts at the team select";
    default:
        return "no contract scene follows";
    }
}

enum Step : unsigned {
    kTeamSelect = 1u << 0,
    kIntro = 1u << 1,
    kTerms = 1u << 2,
    kOutro = 1u << 3,
};

struct Skip {
    bool on = false;
    bool tried = false;
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
        tried = true;
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
