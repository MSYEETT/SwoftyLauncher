// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include <cstring>

namespace f1990 {

inline constexpr char kProfileSave[] = "profile";
inline constexpr char kProfileSave1990[] = "f1990";

inline const char *save_name_for(const char *name)
{
    return name && !std::strcmp(name, kProfileSave) ? kProfileSave1990 : name;
}

// A remote storage name is <title dir>@<container>(#|@)<file>; the device also keeps
// <container>_backup next to it. The stock profile container and its backup are hidden.
inline bool hidden_remote_name(const char *name)
{
    if (!name)
        return false;
    const char *at = std::strchr(name, '@');
    if (!at)
        return false;
    const char *c = at + 1;
    size_t n = std::strcspn(c, "#@");
    return (n == 7 && !std::strncmp(c, "profile", 7)) || (n == 14 && !std::strncmp(c, "profile_backup", 14));
}

}
