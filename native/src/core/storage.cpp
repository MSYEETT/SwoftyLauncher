#include "runtime.hpp"

#include <swofty/mixin.hpp>

#include "perf.hpp"

#include <cstdio>
#include <unordered_map>

std::string settings_addon(const void *from, std::string &dll);

namespace {

const char *const RESULTS[] = {"ok", "late", "unavailable", "invalid"};

struct Caller {
    std::string id, dll;
};

Caller caller(const void *from)
{
    Caller c;
    c.id = settings_addon(from, c.dll);
    return c;
}

constexpr uint32_t QUIET_AFTER = 3;
SRWLOCK seen_lock = SRWLOCK_INIT;
std::unordered_map<std::string, uint32_t> seen;

bool log_every_call()
{
    static const bool all = [] {
        char v[8] = {};
        DWORD n = GetEnvironmentVariableA("SWOFTY_STORAGE_LOG", v, sizeof v);
        return n && n < sizeof v && !_stricmp(v, "all");
    }();
    return all;
}

int done(const Caller &c, const std::string &call, int r, const std::string &why = {})
{
    const char *id = c.id.empty() ? "no addon" : c.id.c_str();
    if (r == SWOFTY_ASSET_OK && !log_every_call()) {
        std::string key = std::string(id) + '\n' + call.substr(0, call.find(" ("));
        AcquireSRWLockExclusive(&seen_lock);
        uint32_t n = ++seen[key];
        ReleaseSRWLockExclusive(&seen_lock);
        if (n > QUIET_AFTER) {
            if (n == QUIET_AFTER + 1)
                swofty::log("[storage] api: %s (%s) %s -> ok; further successful calls like this are not logged "
                            "(failures still are; SWOFTY_STORAGE_LOG=all logs every call)",
                            id, c.dll.c_str(), call.c_str());
            return r;
        }
    }
    swofty::log("[storage] api: %s (%s) %s -> %s%s%s", id, c.dll.c_str(), call.c_str(), RESULTS[r],
                why.empty() ? "" : ": ", why.c_str());
    return r;
}

uint32_t probe(const Caller &c, const char *op)
{
    return perf::live() ? perf::probe(c.id.empty() ? c.dll : c.id, std::string("api storage ") + op) : 0;
}

std::string bytes(size_t n)
{
    char b[48];
    snprintf(b, sizeof b, " (%zu byte(s))", n);
    return b;
}

std::string addon_rel(const Caller &c, const char *path, bool allow_root, std::string &why)
{
    if (c.id.empty())
        return why = "no addon in settings.json lists " + c.dll + ", so it has no folder", std::string();
    std::string f = path ? path : "";
    std::replace(f.begin(), f.end(), '\\', '/');
    if (allow_root && (f.empty() || f == "."))
        return "addons/" + c.id;
    if (f.empty() || f[0] == '/' || f.find(':') != std::string::npos || ("/" + f + "/").find("/../") != std::string::npos)
        return why = "'" + f + "' is not a relative path inside swofty/addons/" + c.id + "/", std::string();
    return "addons/" + c.id + "/" + f;
}

void make_parents(const std::wstring &full)
{
    for (size_t i = full.find_first_of(L"\\/", runtime::swofty_root().size()); i != std::wstring::npos;
         i = full.find_first_of(L"\\/", i + 1))
        CreateDirectoryW(full.substr(0, i).c_str(), nullptr);
}

int st_available()
{
    return runtime::enabled("storage", nullptr) ? 1 : 0;
}

int st_read(const char *path, void *buf, size_t cap, size_t *len)
{
    Caller c = caller(__builtin_return_address(0));
    perf::Scope p(probe(c, "read"));
    std::string call = std::string("read ") + (path ? path : "(null)");
    std::string why, rel = addon_rel(c, path, false, why);
    if (rel.empty())
        return done(c, call, SWOFTY_ASSET_INVALID, why);
    std::wstring full = runtime::under_swofty(rel);
    uint64_t n = 0;
    if (!runtime::file_size(full, &n))
        return done(c, call, SWOFTY_ASSET_INVALID, "swofty/" + rel + " is missing");
    if (len)
        *len = size_t(n);
    if (n > cap)
        return done(c, call, SWOFTY_ASSET_INVALID, "the file is larger than the buffer");
    if (n && !buf)
        return done(c, call, SWOFTY_ASSET_INVALID, "no buffer");
    std::string data = runtime::read_file(full, size_t(n));
    if (data.size() != n)
        return done(c, call, SWOFTY_ASSET_INVALID, "the file could not be read");
    if (n)
        memcpy(buf, data.data(), size_t(n));
    return done(c, call + bytes(size_t(n)), SWOFTY_ASSET_OK);
}

int st_write(const char *path, const void *data, size_t len)
{
    Caller c = caller(__builtin_return_address(0));
    perf::Scope p(probe(c, "write"));
    std::string call = std::string("write ") + (path ? path : "(null)") + bytes(len);
    if (len && !data)
        return done(c, call, SWOFTY_ASSET_INVALID, "no data");
    std::string why, rel = addon_rel(c, path, false, why);
    if (rel.empty())
        return done(c, call, SWOFTY_ASSET_INVALID, why);
    std::wstring full = runtime::under_swofty(rel);
    make_parents(full);
    if (!runtime::write_file(full, len ? std::string(static_cast<const char *>(data), len) : std::string()))
        return done(c, call, SWOFTY_ASSET_INVALID, "the file could not be written");
    return done(c, call, SWOFTY_ASSET_OK);
}

int st_append(const char *path, const void *data, size_t len)
{
    Caller c = caller(__builtin_return_address(0));
    perf::Scope p(probe(c, "append"));
    std::string call = std::string("append ") + (path ? path : "(null)") + bytes(len);
    if (len && !data)
        return done(c, call, SWOFTY_ASSET_INVALID, "no data");
    std::string why, rel = addon_rel(c, path, false, why);
    if (rel.empty())
        return done(c, call, SWOFTY_ASSET_INVALID, why);
    std::wstring full = runtime::under_swofty(rel);
    make_parents(full);
    HANDLE f = CreateFileW(full.c_str(), FILE_APPEND_DATA, FILE_SHARE_READ, nullptr, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL,
                           nullptr);
    if (f == INVALID_HANDLE_VALUE)
        return done(c, call, SWOFTY_ASSET_INVALID, "the file could not be opened");
    bool ok = true;
    for (size_t off = 0; ok && off < len;) {
        DWORD put = 0, want = DWORD(std::min<size_t>(len - off, 1u << 30));
        ok = WriteFile(f, static_cast<const char *>(data) + off, want, &put, nullptr) && put;
        off += put;
    }
    CloseHandle(f);
    return done(c, call, ok ? SWOFTY_ASSET_OK : SWOFTY_ASSET_INVALID, ok ? "" : "the write failed");
}

int st_remove(const char *path)
{
    Caller c = caller(__builtin_return_address(0));
    perf::Scope p(probe(c, "remove"));
    std::string call = std::string("remove ") + (path ? path : "(null)");
    std::string why, rel = addon_rel(c, path, false, why);
    if (rel.empty())
        return done(c, call, SWOFTY_ASSET_INVALID, why);
    if (DeleteFileW(runtime::under_swofty(rel).c_str()))
        return done(c, call, SWOFTY_ASSET_OK);
    DWORD e = GetLastError();
    bool gone = e == ERROR_FILE_NOT_FOUND || e == ERROR_PATH_NOT_FOUND;
    return done(c, call, SWOFTY_ASSET_INVALID, gone ? "swofty/" + rel + " is missing" : "the file could not be removed");
}

int st_size(const char *path, uint64_t *out)
{
    Caller c = caller(__builtin_return_address(0));
    perf::Scope p(probe(c, "size"));
    std::string call = std::string("size ") + (path ? path : "(null)");
    std::string why, rel = addon_rel(c, path, false, why);
    if (rel.empty())
        return done(c, call, SWOFTY_ASSET_INVALID, why);
    uint64_t n = 0;
    if (!runtime::file_size(runtime::under_swofty(rel), &n))
        return done(c, call, SWOFTY_ASSET_INVALID, "swofty/" + rel + " is missing");
    if (out)
        *out = n;
    return done(c, call, SWOFTY_ASSET_OK);
}

int st_list(const char *dir, char *buf, size_t cap, size_t *len)
{
    Caller c = caller(__builtin_return_address(0));
    perf::Scope p(probe(c, "list"));
    std::string call = std::string("list ") + (dir && *dir ? dir : ".");
    std::string why, rel = addon_rel(c, dir, true, why);
    if (rel.empty())
        return done(c, call, SWOFTY_ASSET_INVALID, why);
    WIN32_FIND_DATAW fd{};
    HANDLE h = FindFirstFileW((runtime::under_swofty(rel) + L"\\*").c_str(), &fd);
    if (h == INVALID_HANDLE_VALUE)
        return done(c, call, SWOFTY_ASSET_INVALID, "swofty/" + rel + " is not a folder");
    std::string out;
    do {
        std::wstring n = fd.cFileName;
        if (n == L"." || n == L"..")
            continue;
        out += runtime::narrow(n);
        if (fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)
            out += '/';
        out += '\n';
    } while (FindNextFileW(h, &fd));
    FindClose(h);
    if (len)
        *len = out.size();
    if (out.size() > cap)
        return done(c, call, SWOFTY_ASSET_INVALID, "the listing is larger than the buffer");
    if (!out.empty() && buf)
        memcpy(buf, out.data(), out.size());
    return done(c, call, SWOFTY_ASSET_OK);
}

const SwoftyStorage table = {st_available, st_read, st_write, st_append, st_remove, st_size, st_list};

}

namespace runtime {

const SwoftyStorage *storage_table()
{
    return &table;
}

}
