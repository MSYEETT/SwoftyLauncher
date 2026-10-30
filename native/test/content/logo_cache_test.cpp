#include "../../src/core/apply/content/logo_cache.hpp"

#include <algorithm>
#include <cstdio>
#include <map>
#include <string>
#include <vector>

namespace lc = swofty_core::apply::content::logo_cache;

static int failures = 0;

static void check(bool ok, const std::string &what)
{
    std::printf("%s  %s\n", ok ? "ok  " : "FAIL", what.c_str());
    failures += !ok;
}

static uint64_t hash_of(const std::string &bytes, size_t chunk)
{
    lc::Bytes b;
    for (size_t at = 0; at < bytes.size(); at += chunk)
        b.add(bytes.data() + at, std::min(chunk, bytes.size() - at));
    return b.done();
}

struct Disk {
    struct File {
        std::string bytes;
        uint64_t time = 0;
    };
    std::map<std::string, File> files;
    int hashes = 0;

    lc::Env env()
    {
        return {
            [this](const std::string &rel, uint64_t &size, uint64_t &time) {
                auto f = files.find(rel);
                if (f == files.end())
                    return false;
                size = f->second.bytes.size();
                time = f->second.time;
                return true;
            },
            [this](const std::string &rel, uint64_t &hash) {
                auto f = files.find(rel);
                if (f == files.end())
                    return false;
                hashes++;
                hash = hash_of(f->second.bytes, 4096);
                return true;
            },
        };
    }

    std::vector<std::string> list() const
    {
        std::vector<std::string> out;
        for (auto &[rel, f] : files)
            out.push_back(rel);
        return out;
    }
};

struct Boot {
    uint64_t key;
    size_t hashed;
    std::string stamps;
};

static Boot boot(Disk &disk, const std::string &stamps)
{
    auto k = lc::key_archives(7, disk.list(), disk.env(), lc::parse(stamps));
    return {k.key, k.hashed, lc::text(k.stamps)};
}

int main()
{
    std::string data(100000, '\0');
    for (size_t i = 0; i < data.size(); i++)
        data[i] = char((i * 2654435761u) >> 13);
    uint64_t whole = hash_of(data, data.size());
    check(hash_of(data, 1) == whole && hash_of(data, 7) == whole && hash_of(data, 33) == whole && hash_of(data, 1 << 16) == whole,
          "the content hash is the same however the bytes arrive");
    std::string flipped = data;
    flipped[54321] ^= 1;
    check(hash_of(flipped, 4096) != whole, "one changed bit changes the hash");
    check(hash_of(data + std::string(1, '\0'), 4096) != whole, "a trailing zero byte changes the hash");
    check(hash_of("", 1) != hash_of(std::string(1, '\0'), 1), "an empty file and a one-byte file differ");

    lc::Stamps st;
    st["a/b.erp"] = {10, 20, 0xabcdef, true};
    st["c.erp"] = {1, 2, 0, false};
    auto back = lc::parse(lc::text(st) + "broken line\nx\t1\t2\n\t1\t2\t-\ny\tz\t2\t-\n");
    check(back.size() == 2 && back["a/b.erp"].hash == 0xabcdef && back["a/b.erp"].hashed && !back["c.erp"].hashed &&
              back["c.erp"].size == 1 && back["c.erp"].time == 2,
          "stamps read back what they wrote and skip broken lines");

    Disk disk;
    disk.files["ui/common.erp"] = {data, 100};
    disk.files["ui/flow.erp"] = {"flow bytes", 100};
    disk.files["ui/stock.erp"] = {"stock bytes", 50};

    Boot first = boot(disk, "");
    check(first.hashed == 3 && disk.hashes == 3, "a first boot hashes every archive");
    Boot same = boot(disk, first.stamps);
    check(same.key == first.key && same.hashed == 0 && disk.hashes == 3, "an untouched folder keeps its key without reading an archive");

    disk.files["ui/common.erp"].time = 200;
    Boot installed = boot(disk, same.stamps);
    check(installed.key == first.key && installed.hashed == 1,
          "the same bytes copied in again with a new write time keep the key (the cache is reused)");
    Boot quiet = boot(disk, installed.stamps);
    check(quiet.key == first.key && quiet.hashed == 0, "an archive hashed under its new write time is not read again");

    disk.files["ui/common.erp"] = {flipped, 300};
    Boot edited = boot(disk, quiet.stamps);
    check(edited.key != first.key && edited.hashed == 1, "changed bytes of the same size change the key");

    disk.files["ui/common.erp"] = {data, 400};
    Boot restored = boot(disk, edited.stamps);
    check(restored.key == first.key, "the earlier bytes copied back give the earlier key");

    disk.files["ui/common.erp"].time = 100;
    disk.files["ui/common.erp"].bytes = flipped;
    Boot sneaky = boot(disk, first.stamps);
    check(sneaky.key == first.key && sneaky.hashed == 0, "stamps are trusted while size and write time hold");

    Boot fresh = boot(disk, "");
    check(fresh.key == edited.key && fresh.hashed == 3, "without stamps every archive is read again");

    disk.files.erase("ui/flow.erp");
    Boot removed = boot(disk, fresh.stamps);
    check(removed.key != fresh.key && removed.hashed == 0 && removed.stamps.find("ui/flow.erp") == std::string::npos,
          "a removed archive changes the key and leaves the stamps");

    disk.files["ui/new.erp"] = {"new bytes", 10};
    Boot added = boot(disk, removed.stamps);
    check(added.key != removed.key && added.hashed == 1, "an added archive is hashed and changes the key");

    std::string unhashed = removed.stamps;
    auto at = unhashed.find("ui/stock.erp");
    unhashed = unhashed.substr(0, at) + "ui/stock.erp\t11\t50\t-\n" + unhashed.substr(unhashed.find('\n', at) + 1);
    Boot retry = boot(disk, unhashed);
    check(retry.key == added.key && retry.hashed == 2, "a stamp without a hash is hashed again");

    std::printf("%s\n", failures ? "FAILED" : "all logo cache checks passed");
    return failures ? 1 : 0;
}
