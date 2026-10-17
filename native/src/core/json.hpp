#pragma once
#include <cstdlib>
#include <cstring>
#include <string>
#include <tuple>
#include <utility>
#include <vector>

namespace json {

struct Value {
    enum Type { Null, Bool, Num, Str, Arr, Obj } type = Null;
    bool b = false;
    double n = 0;
    std::string s;
    std::vector<Value> a;
    std::vector<std::pair<std::string, Value>> o;

    const Value &operator[](const char *key) const
    {
        static const Value none;
        for (auto &[k, v] : o)
            if (k == key)
                return v;
        return none;
    }
    const Value &field(const char *key, const char *legacy) const
    {
        const Value &v = (*this)[key];
        return v.type == Null ? (*this)[legacy] : v;
    }
    std::string str(const char *def = "") const { return type == Str ? s : def; }
    long long num(long long def = 0) const
    {
        return type == Num ? (long long)n : type == Str ? strtoll(s.c_str(), nullptr, 0) : def;
    }
};

struct Parser {
    const char *p;

    void ws()
    {
        while (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r')
            p++;
    }

    bool string(std::string &out)
    {
        if (*p++ != '"')
            return false;
        while (*p && *p != '"') {
            char c = *p++;
            if (c == '\\') {
                if (!*p)
                    return false;
                c = *p++;
                if (c == 'n')
                    c = '\n';
                else if (c == 't')
                    c = '\t';
                else if (c == 'u') {
                    if (strnlen(p, 4) < 4)
                        return false;
                    c = (char)strtol(std::string(p, 4).c_str(), nullptr, 16);
                    p += 4;
                }
            }
            out += c;
        }
        return *p++ == '"';
    }

    bool value(Value &v)
    {
        ws();
        if (*p == '{') {
            v.type = Value::Obj;
            p++;
            ws();
            if (*p == '}')
                return p++, true;
            do {
                ws();
                std::string k;
                Value x;
                if (!string(k))
                    return false;
                ws();
                if (*p++ != ':' || !value(x))
                    return false;
                v.o.emplace_back(std::move(k), std::move(x));
                ws();
            } while (*p == ',' && p++);
            return *p++ == '}';
        }
        if (*p == '[') {
            v.type = Value::Arr;
            p++;
            ws();
            if (*p == ']')
                return p++, true;
            do {
                Value x;
                if (!value(x))
                    return false;
                v.a.push_back(std::move(x));
                ws();
            } while (*p == ',' && p++);
            return *p++ == ']';
        }
        if (*p == '"') {
            v.type = Value::Str;
            return string(v.s);
        }
        for (auto [word, type, b] : {std::tuple{"true", Value::Bool, true}, {"false", Value::Bool, false}, {"null", Value::Null, false}})
            if (!strncmp(p, word, strlen(word))) {
                p += strlen(word);
                v.type = type;
                v.b = b;
                return true;
            }
        char *end;
        v.n = strtod(p, &end);
        if (end == p)
            return false;
        v.type = Value::Num;
        p = end;
        return true;
    }
};

inline bool parse(const std::string &text, Value &out)
{
    Parser ps{text.c_str()};
    if (!ps.value(out))
        return false;
    ps.ws();
    return !*ps.p;
}

}
