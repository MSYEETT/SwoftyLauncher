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

}
