#pragma once

#include <cstddef>
#include <functional>
#include <mutex>
#include <string>
#include <vector>

namespace swofty_core::apply::db {

struct Row {
    std::string addon, registry, path, type, text;
};

class Staging {
public:
    int set(const std::string &addon, const std::string &registry, const std::string &path, const std::string &type,
            const std::string &text)
    {
        std::lock_guard g(mu_);
        if (!open_)
            return 0;
        rows_.push_back({addon, registry, path, type, text});
        return 1;
    }

    bool available() const
    {
        std::lock_guard g(mu_);
        return open_;
    }

    bool exists(const std::string &path) const
    {
        std::function<bool(const std::string &)> fn;
        {
            std::lock_guard g(mu_);
            fn = exists_;
        }
        return fn && fn(path);
    }

    void open(std::function<bool(const std::string &)> exists)
    {
        std::lock_guard g(mu_);
        open_ = true;
        exists_ = std::move(exists);
    }

    std::vector<Row> close()
    {
        std::lock_guard g(mu_);
        open_ = false;
        exists_ = nullptr;
        std::vector<Row> out;
        out.swap(rows_);
        return out;
    }

    std::size_t size() const
    {
        std::lock_guard g(mu_);
        return rows_.size();
    }

private:
    mutable std::mutex mu_;
    bool open_ = false;
    std::function<bool(const std::string &)> exists_;
    std::vector<Row> rows_;
};

inline Staging &staging()
{
    static Staging s;
    return s;
}

}
