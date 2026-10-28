#include "moments.hpp"

#include "../hooks/hooks.hpp"
#include "../registry/table.hpp"
#include "../runtime.hpp"
#include "database.hpp"
#include "scheduler.hpp"

#include <swofty/mixin.hpp>

#include <windows.h>

#include <atomic>
#include <cstdio>
#include <ctime>
#include <map>
#include <mutex>

namespace swofty_core::apply::moments {

namespace {

namespace reg = swofty_core::registry;
using swofty::json::Value;

std::once_flag attached;
std::mutex status_mu;
std::map<std::string, std::vector<std::string>> status_lines;
std::atomic<bool> loaded_from_flow{false};
std::mutex ids_mu;

std::string registry_dir()
{
    std::wstring w = runtime::swofty_root();
    while (!w.empty() && (w.back() == L'\\' || w.back() == L'/'))
        w.pop_back();
    w += L"\\registry";
    int n = WideCharToMultiByte(CP_UTF8, 0, w.c_str(), -1, nullptr, 0, nullptr, nullptr);
    std::string out(n > 0 ? n - 1 : 0, '\0');
    if (n > 1)
        WideCharToMultiByte(CP_UTF8, 0, w.c_str(), -1, out.data(), n, nullptr, nullptr);
    return out;
}

std::string today()
{
    std::time_t t = std::time(nullptr);
    std::tm tm{};
    localtime_s(&tm, &t);
    char b[16];
    std::strftime(b, sizeof b, "%Y-%m-%d", &tm);
    return b;
}

bool write_file(const std::string &path, const std::string &text)
{
    CreateDirectoryA(registry_dir().c_str(), nullptr);
    std::string tmp = path + ".tmp";
    FILE *f = std::fopen(tmp.c_str(), "wb");
    if (!f)
        return false;
    bool ok = std::fwrite(text.data(), 1, text.size(), f) == text.size();
    ok = std::fclose(f) == 0 && ok;
    std::remove(path.c_str());
    return ok && std::rename(tmp.c_str(), path.c_str()) == 0;
}

std::string read_file(const std::string &path)
{
    std::string out;
    if (FILE *f = std::fopen(path.c_str(), "rb")) {
        char b[65536];
        for (size_t n; (n = std::fread(b, 1, sizeof b, f));)
            out.append(b, n);
        std::fclose(f);
    }
    return out;
}

void save_ids()
{
    std::lock_guard g(ids_mu);
    if (!ids::store().dirty())
        return;
    if (write_file(registry_dir() + "\\ids.json", ids::store().dump()))
        ids::store().saved();
    else
        swofty::log("[registry] stat ids: ids.json could not be written in %s", registry_dir().c_str());
}

std::string owner_of(const std::string &id)
{
    auto s = id.find('/');
    return s == std::string::npos ? std::string() : id.substr(0, s);
}

void facts(const reg::Context &c)
{
    hooks::update_facts([&](hooks::Facts &f) {
        if (!c.mode.empty())
            f.mode = c.mode;
        f.content = c.content.empty() ? std::string() : c.content.front();
        f.content_registry = c.content.empty() ? std::string() : std::string("content.package");
        f.content_owner = owner_of(f.content);
        f.chain = c.content;
        f.owners.clear();
        for (auto &id : c.content)
            f.owners.push_back(owner_of(id));
        if (!c.career.empty()) {
            if (!f.career)
                f.career.emplace();
            f.career->id = c.career;
            f.career->data_group = c.data_group;
            f.career->year = c.has_year ? c.year : 0;
        } else {
            f.career.reset();
        }
        if (!c.session_type.empty() || !c.track.empty()) {
            if (!f.session)
                f.session.emplace();
            f.session->type = c.session_type;
            f.session->track = c.track;
            f.session->setup_mode = c.setup_mode;
        }
        f.vehicle_packages.clear();
        if (!c.vehicle_package.empty())
            f.vehicle_packages.push_back(c.vehicle_package);
    });
}

uint64_t pe_timestamp()
{
    uintptr_t base = swofty::api ? swofty::api->module_base() : 0;
    if (!base)
        return 0;
    auto dos = reinterpret_cast<const IMAGE_DOS_HEADER *>(base);
    auto nt = reinterpret_cast<const IMAGE_NT_HEADERS *>(base + dos->e_lfanew);
    return nt->FileHeader.TimeDateStamp;
}

const char *mode_name(unsigned m)
{
    return m == SWOFTY_CAREER_MY_TEAM ? "my_team" : m == SWOFTY_CAREER_DRIVER ? "driver_career" : "";
}

void extend(Value &, Value &conflicts)
{
    Value &list = conflicts["registry"];
    if (!list.is_array())
        list = Value::array();
    for (auto &f : hooks::faults())
        list.push_back(Value::object({{"kind", Value("handler_faulted")},
                                      {"point", Value(f.point)},
                                      {"phase", Value(f.phase)},
                                      {"addon", Value(f.addon)},
                                      {"error", Value(f.what)}}));
    std::lock_guard g(status_mu);
    if (status_lines.empty())
        return;
    Value st = Value::object();
    for (auto &[addon, lines] : status_lines) {
        Value a = Value::array();
        for (auto &l : lines)
            a.push_back(Value(l));
        st[addon] = a;
    }
    conflicts["status"] = st;
}

void on_stage(int stage)
{
    if (stage == runtime::STAGE_WINDOW)
        window();
}

}

void attach()
{
    std::call_once(attached, [] {
        Scheduler &s = scheduler();
        s.set_log([](const std::string &line) { swofty::log("%s", line.c_str()); });
        s.set_facts(facts);
        std::string text = read_file(registry_dir() + "\\ids.json");
        std::string why;
        if (!text.empty() && !ids::store().load(text, &why))
            swofty::log("[registry] stat ids: ids.json unreadable (%s); it is rewritten from the ids this game start "
                        "allocates",
                        why.c_str());
        else if (!text.empty())
            swofty::log("[registry] stat ids: ids.json holds %zu id(s)", ids::store().size());
        reg::set_stat_id_source([](const std::string &registry, const std::string &id, int64_t *out) {
            std::string line;
            int r = stat_id(reg::store(), ids::store(), registry, id, out, today(), &line);
            if (!line.empty())
                swofty::log("%s", line.c_str());
            save_ids();
            return r;
        });
        runtime::on_stage(on_stage);
    });
}

std::size_t boot_plan()
{
    attach();
    return scheduler().plan_boot();
}

bool boot_work()
{
    return scheduler().boot_work();
}

void status(const std::string &addon, const std::string &line)
{
    std::lock_guard g(status_mu);
    status_lines[addon.empty() ? std::string("swofty.core") : addon].push_back(line);
}

void write_room(const room::Report &report)
{
    room::Report r = report;
    r.pe_timestamp = pe_timestamp();
    if (r.profile.empty())
        r.profile = reg::store().context().profile;
    std::string path = registry_dir() + "\\room.json";
    bool ok = write_file(path, swofty::json::dump(room::to_json(r), 1));
    swofty::log("%s", room::line(r).c_str());
    swofty::log("[registry] room report %s%s", ok ? "written to " : "NOT written to ", path.c_str());
}

void boot_load(room::Report &report)
{
    (void)report;
    attach();
    scheduler().run_boot();
    save_ids();
}

void boot_without_database(room::Report report)
{
    attach();
    scheduler().run_boot();
    auto rows = db::staging().close();
    std::map<std::string, std::size_t> refused;
    for (auto &row : rows)
        refused[row.addon]++;
    for (auto &[addon, n] : refused) {
        status(addon, std::to_string(n) + " database record(s) not applied: " +
                          (report.reason.empty() ? std::string("the database was not reachable") : report.reason));
        swofty::log("[registry] register: %s %zu database record(s) refused (late): %s", addon.c_str(), n,
                    report.reason.c_str());
    }
    report.registry_asked = scheduler().planned_keys();
    save_ids();
    write_room(report);
}

void career_loaded(const char *key, unsigned mode)
{
    attach();
    char group[128] = {};
    if (auto t = runtime::career_content_table())
        if (t->data_group && t->data_group(nullptr, group, sizeof group) != SWOFTY_ASSET_OK)
            group[0] = 0;
    scheduler().career_loaded(key ? key : "", mode_name(mode), group);
}

void career_unloaded()
{
    attach();
    scheduler().career_unloaded();
}

void career_season(int year)
{
    attach();
    scheduler().career_season(year);
}

void box_picked(const char *entrant_list)
{
    attach();
    scheduler().content_picked(entrant_list ? entrant_list : "");
}

void session_load(const char *source)
{
    attach();
    std::string from = source ? source : "";
    bool flow = from == "StateLoadSession";
    if (!flow && scheduler().in_session() && loaded_from_flow.load())
        return;
    loaded_from_flow = flow;
    scheduler().session_load("", "");
    write_reports(from.c_str());
}

void session_end(const char *source)
{
    attach();
    loaded_from_flow = false;
    scheduler().session_end(source ? source : "the session ended");
}

void window()
{
    attach();
    scheduler().install_read();
}

int write_reports(const char *when)
{
    save_ids();
    int r = reg::write_reports(registry_dir(), extend);
    swofty::log("[registry] effective.json and conflicts.json %s (%s)", r == SWOFTY_ASSET_OK ? "written" : "NOT written",
                when ? when : "");
    return r;
}

namespace {

DWORD WINAPI crash_writer(void *)
{
    write_reports("crash");
    return 0;
}

}

void crash_reports()
{
    HANDLE t = CreateThread(nullptr, 0, crash_writer, nullptr, 0, nullptr);
    if (!t)
        return;
    if (WaitForSingleObject(t, 3000) != WAIT_OBJECT_0)
        swofty::log("[registry] effective.json and conflicts.json not written on the crash: the registry stayed busy");
    CloseHandle(t);
}

ids::CareerMap map_for_career(const std::string &career)
{
    attach();
    for (auto *a : appliers())
        ids::store().set_live(a->registry(), reg::store().list(a->registry(), ""));
    ids::CareerMap m = ids::store().map_for_career(career);
    save_ids();
    return m;
}

std::vector<ids::Clash> adopt(const std::string &career, const ids::CareerMap &map)
{
    attach();
    auto clashes = ids::store().adopt(career, map, today());
    for (auto &c : clashes)
        swofty::log("[guards] career.load: %s", c.line.c_str());
    save_ids();
    return clashes;
}

}
