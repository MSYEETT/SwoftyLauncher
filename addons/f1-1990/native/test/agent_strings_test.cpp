// SPDX-License-Identifier: AGPL-3.0-or-later
#include <cstdio>
#include <fstream>
#include <map>
#include <sstream>
#include <string>

#include "agent_strings.hpp"

static int failures;

static void check(const char *name, bool ok)
{
    std::printf("%s %s\n", ok ? "ok  " : "FAIL", name);
    failures += !ok;
}

static const std::string *text(const std::vector<f1990::AgentStrings> &langs, const char *code, const char *key)
{
    for (auto &l : langs)
        if (l.code == code)
            for (auto &[k, t] : l.texts)
                if (k == key)
                    return &t;
    return nullptr;
}

int main(int argc, char **argv)
{
    std::vector<f1990::AgentStrings> langs;
    size_t at = 0;

    check("parses an empty object", f1990::parse_agent_strings(" {} \n", langs) && langs.empty());
    check("parses languages and escapes",
          f1990::parse_agent_strings(R"({"eng": {"a": "x\r\ny \"q\" \\ \/ \t", "b": ""}, "jap": {}, "zhs": {"c": "弗😀"}})",
                                     langs) &&
              langs.size() == 3 && langs[0].texts.size() == 2 && *text(langs, "eng", "a") == "x\r\ny \"q\" \\ / \t" &&
              text(langs, "eng", "b")->empty() && langs[1].texts.empty() &&
              *text(langs, "zhs", "c") == "\xe5\xbc\x97\xf0\x9f\x98\x80");
    check("keeps raw UTF-8", f1990::parse_agent_strings("{\"jap\": {\"k\": \"\xe3\x83\x95\"}}", langs) &&
                                 *text(langs, "jap", "k") == "\xe3\x83\x95");
    check("rejects a number value", !f1990::parse_agent_strings(R"({"eng": {"a": 1}})", langs, &at) && at == 14 &&
                                        langs.empty());
    check("rejects a lone low surrogate", !f1990::parse_agent_strings(R"({"eng": {"a": "\udc00"}})", langs));
    check("rejects a trailing comma", !f1990::parse_agent_strings(R"({"eng": {"a": "b",}})", langs));
    check("rejects trailing data", !f1990::parse_agent_strings(R"({} x)", langs, &at) && at == 3);
    check("rejects an unterminated string", !f1990::parse_agent_strings(R"({"eng": {"a": "b)", langs));

    if (argc > 1) {
        std::ifstream f(argv[1], std::ios::binary);
        std::stringstream ss;
        ss << f.rdbuf();
        bool ok = f1990::parse_agent_strings(ss.str(), langs, &at);
        check("parses data/flavio-strings.json", ok);
        size_t n = 0;
        for (auto &l : langs)
            n += l.texts.size();
        std::printf("     %zu languages, %zu texts\n", langs.size(), n);
        check("has the eleven game languages", langs.size() == 11 && langs[0].code == "bra" && langs.back().code == "zhs");
        const std::string *eng = text(langs, "eng", "lng_elena_name"), *jap = text(langs, "jap", "lng_elena_name");
        check("names the agent Flavio in eng", eng && *eng == "Flavio");
        check("names the agent in jap", jap && *jap == "\xe3\x83\x95\xe3\x83\xa9\xe3\x83\x93\xe3\x82\xaa");
        const std::string *mail = text(langs, "eng", "lng_email_championship_points_tier_1_completed_message");
        check("keeps CR LF in email bodies", mail && mail->find("\r\n\r\nFlavio") != std::string::npos);
    }

    std::printf("%s (%d failure(s))\n", failures ? "FAILED" : "ALL PASSED", failures);
    return failures != 0;
}
