// Validates every example and must-fail case under a registries folder with the core's schema check and
// prints the same JSON report as `python3 tools/std/validate.py --json` (check_example and run_examples
// there), so run_schema_test.sh can diff the verdicts and error pointers of the two.
//
//   schema_agree <registries folder> <display prefix>
//   schema_agree --patterns < cases.json    [[pattern, [text, ...]], ...] -> [[compiled, [matched, ...]], ...]

#include "../../src/core/registry/schema.hpp"

#include <algorithm>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <map>
#include <sstream>

namespace fs = std::filesystem;
using swofty_core::registry::check_data;
using swofty_core::registry::check_record;
using swofty_core::registry::DataOptions;
using swofty_core::registry::DataReport;
using swofty_core::registry::DataSchemas;
using swofty_core::registry::Schema;
using Value = swofty::json::Value;

namespace {

using Version = std::pair<int, int>;
std::map<std::string, std::map<Version, Schema>> g_schemas;
int g_compile_failures = 0;

bool read_file(const fs::path &p, std::string &out)
{
    std::ifstream f(p, std::ios::binary);
    if (!f)
        return false;
    std::ostringstream s;
    s << f.rdbuf();
    out = s.str();
    return true;
}

bool number(std::string_view t, int &out, bool zero_ok = true)
{
    if (t.empty() || t.size() > 9 || !std::all_of(t.begin(), t.end(), [](char c) { return c >= '0' && c <= '9'; }))
        return false;
    if (t.size() > 1 && t[0] == '0')
        return false;
    if (!zero_ok && t == "0")
        return false;
    out = std::stoi(std::string(t));
    return true;
}

bool version_file(const std::string &name, Version &v)
{
    if (name.size() < 5 || name.substr(name.size() - 5) != ".json")
        return false;
    std::string_view stem(name.data(), name.size() - 5);
    auto dot = stem.find('.');
    return dot != std::string_view::npos && number(stem.substr(0, dot), v.first) && number(stem.substr(dot + 1), v.second);
}

std::string version_name(Version v)
{
    return std::to_string(v.first) + "." + std::to_string(v.second);
}

struct Pin {
    int major = 0;
    int minor = -1;
};

std::optional<Pin> file_pin(const std::string &name)
{
    auto at = name.rfind('@');
    if (at == std::string::npos || at == 0 || name.size() < 5 || name.substr(name.size() - 5) != ".json")
        return std::nullopt;
    std::string_view rest(name.data() + at + 1, name.size() - at - 1 - 5);
    Pin p;
    auto dot = rest.find('.');
    if (!number(rest.substr(0, dot), p.major, false))
        return std::nullopt;
    if (dot != std::string_view::npos && !number(rest.substr(dot + 1), p.minor))
        return std::nullopt;
    return p;
}

bool is_layers_name(const std::string &name)
{
    std::string stem = name.substr(0, name.find('@'));
    if (stem.size() >= 5 && stem.substr(stem.size() - 5) == ".json")
        stem.resize(stem.size() - 5);
    auto ends = [&](std::string_view s) { return stem.size() >= s.size() && stem.compare(stem.size() - s.size(), s.size(), s) == 0; };
    return stem == "layers" || ends("layer") || ends("layers");
}

bool data_key_name(std::string_view key, std::string &name)
{
    if (!key.empty() && key.back() == '\n')
        key.remove_suffix(1);
    auto at = key.find('@');
    std::string_view n = key.substr(0, at);
    if (n.empty() || !((n[0] >= 'a' && n[0] <= 'z') || (n[0] >= '0' && n[0] <= '9')))
        return false;
    for (char c : n)
        if (!((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '_' || c == '.' || c == '-'))
            return false;
    if (at != std::string_view::npos) {
        int m = 0;
        if (!number(key.substr(at + 1), m, false))
            return false;
    }
    name = std::string(n);
    return true;
}

bool is_data_file(const Value &doc, const std::string &name)
{
    auto *o = doc.if_object();
    if (!o || o->empty())
        return false;
    for (auto &[k, v] : *o) {
        (void)v;
        std::string n;
        if (!data_key_name(k, n) || n != name)
            return false;
    }
    return true;
}

std::string quoted(const std::string &s)
{
    return "'" + s + "'";
}

DataSchemas pick_versions(const std::string &name, int major, const std::optional<Pin> &pin)
{
    DataSchemas out;
    auto it = g_schemas.find(name);
    if (it == g_schemas.end() || it->second.empty()) {
        out.missing = "no SDK schema for registry " + quoted(name);
        return out;
    }
    auto &versions = it->second;
    if (pin) {
        if (pin->major != major && major != 0) {
            out.missing = "the file name pins major " + std::to_string(pin->major) + " but the data names major " + std::to_string(major);
            return out;
        }
        major = pin->major;
        if (pin->minor >= 0) {
            auto v = versions.find({pin->major, pin->minor});
            if (v == versions.end()) {
                out.missing = name + " has no schema " + std::to_string(pin->major) + "." + std::to_string(pin->minor);
                return out;
            }
            out.versions.emplace_back(version_name(v->first), &v->second);
            return out;
        }
    }
    if (major == 0)
        major = 1;
    for (auto &[v, s] : versions)
        if (v.first == major)
            out.versions.emplace_back(version_name(v), &s);
    if (out.versions.empty())
        out.missing = name + " has no major " + std::to_string(major);
    return out;
}

Value pairs(const std::vector<Schema::Error> &errors)
{
    Value out = Value::array();
    for (auto &e : errors)
        out.push_back(Value::array({e.pointer, e.message}));
    return out;
}

Value check_example(const fs::path &file, const std::string &display, const std::string &name, bool valid)
{
    Value entry = Value::object();
    entry["file"] = display;
    entry["registry"] = name;
    entry["expect"] = valid ? "valid" : "invalid";
    entry["kind"] = nullptr;
    entry["versions"] = Value::object();
    entry["problems"] = Value::array();
    std::string text;
    swofty::json::Error perr;
    Value doc;
    if (!read_file(file, text))
        perr.message = "unreadable";
    else
        doc = swofty::json::parse(text, &perr);
    if (!perr.ok()) {
        entry["problems"].push_back(Value::array({"/", "not JSON: " + perr.message}));
        entry["ok"] = false;
        return entry;
    }
    std::string base = file.filename().string();
    auto pin = file_pin(base);
    DataReport report;
    if (is_data_file(doc, name)) {
        bool layers = is_layers_name(base);
        entry["kind"] = layers ? "layers" : "records";
        DataOptions o;
        o.layers = layers;
        o.only = name;
        o.max_errors = 100000;
        report = check_data(doc, [&](std::string_view n, int major) { return pick_versions(std::string(n), major, pin); }, o);
    } else {
        entry["kind"] = "record";
        DataOptions o;
        o.max_errors = 100000;
        report = check_record(doc, name, pick_versions(name, 0, pin), o);
    }
    Value versions = Value::object();
    for (auto &[v, errs] : report.by_version)
        versions[v] = pairs(errs);
    entry["versions"] = versions;
    for (auto &m : report.missing)
        entry["problems"].push_back(Value::array({m.pointer, m.message}));
    bool any_errors = false;
    for (auto &[v, errs] : report.by_version)
        any_errors = any_errors || !errs.empty();
    if (valid) {
        for (auto &p : report.problems)
            entry["problems"].push_back(Value::array({p.pointer, p.message}));
        entry["ok"] = !report.by_version.empty() && entry["problems"].size() == 0 && !any_errors;
    } else {
        Value accepted = Value::array();
        for (auto &[v, errs] : report.by_version)
            if (errs.empty())
                accepted.push_back(v);
        entry["accepted_by"] = report.problems.empty() ? accepted : Value::array();
        entry["rejections"] = pairs(report.problems);
        bool rejected = !report.problems.empty() || (!report.by_version.empty() && accepted.size() == 0);
        entry["ok"] = report.missing.empty() && rejected;
    }
    return entry;
}

std::vector<fs::path> sorted_entries(const fs::path &dir, bool dirs)
{
    std::vector<fs::path> out;
    std::error_code ec;
    if (!fs::is_directory(dir, ec))
        return out;
    for (auto &e : fs::directory_iterator(dir, ec)) {
        if (dirs ? e.is_directory() : (e.is_regular_file() && e.path().extension() == ".json"))
            out.push_back(e.path());
    }
    std::sort(out.begin(), out.end(), [](auto &a, auto &b) { return a.filename().string() < b.filename().string(); });
    return out;
}

void load_schemas(const fs::path &root)
{
    for (auto &folder : sorted_entries(root, true)) {
        auto &versions = g_schemas[folder.filename().string()];
        for (auto &f : sorted_entries(folder, false)) {
            Version v;
            if (!version_file(f.filename().string(), v))
                continue;
            std::string text;
            swofty::json::Error perr;
            if (!read_file(f, text))
                continue;
            auto doc = swofty::json::parse(text, &perr);
            if (!perr.ok())
                continue;
            Schema::Error err;
            auto s = Schema::compile(doc, &err);
            if (!s) {
                std::cerr << "schema_agree: " << f.string() << " " << err.pointer << ": " << err.message << "\n";
                ++g_compile_failures;
                continue;
            }
            versions.emplace(v, std::move(*s));
        }
    }
}

} // namespace

int patterns()
{
    std::ostringstream in;
    in << std::cin.rdbuf();
    swofty::json::Error perr;
    Value cases = swofty::json::parse(in.str(), &perr);
    if (!perr.ok() || !cases.is_array()) {
        std::cerr << "schema_agree: --patterns reads a JSON list of [pattern, [text, ...]]\n";
        return 2;
    }
    Value out = Value::array();
    for (auto &c : *cases.if_array()) {
        Value schema = Value::object({{"type", "string"}, {"pattern", c[0]}});
        auto s = Schema::compile(schema);
        Value results = Value::array();
        if (s)
            for (auto &t : *c[1].if_array())
                results.push_back(s->valid(t));
        out.push_back(Value::array({s.has_value(), std::move(results)}));
    }
    std::cout << swofty::json::dump(out) << "\n";
    return 0;
}

int main(int argc, char **argv)
{
    if (argc == 2 && std::string_view(argv[1]) == "--patterns")
        return patterns();
    if (argc < 3) {
        std::cerr << "usage: schema_agree <registries folder> <display prefix>\n";
        return 2;
    }
    fs::path root = argv[1];
    std::string display = argv[2];
    while (display.size() > 1 && display.back() == '/')
        display.pop_back();
    load_schemas(root);
    Value examples = Value::array();
    int failed = 0;
    for (auto &folder : sorted_entries(root, true)) {
        std::string name = folder.filename().string();
        auto run = [&](const fs::path &dir, bool valid, const std::string &rel) {
            for (auto &f : sorted_entries(dir, false)) {
                auto entry = check_example(f, display + "/" + name + "/" + rel + f.filename().string(), name, valid);
                failed += !entry["ok"].as_bool();
                examples.push_back(std::move(entry));
            }
        };
        run(folder / "examples", true, "examples/");
        run(folder / "examples" / "must-fail", false, "examples/must-fail/");
    }
    Value report = Value::object();
    report["examples"] = examples;
    report["failed"] = failed;
    std::cout << swofty::json::dump(report, 2) << "\n";
    return g_compile_failures ? 3 : (failed ? 1 : 0);
}
