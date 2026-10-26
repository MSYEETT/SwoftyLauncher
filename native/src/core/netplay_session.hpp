#pragma once

#include <atomic>

// The launcher arms netplay by launching the game with SWOFTY_NETPLAY set (hooks installed, pass-through)
// and configures a room at runtime through the netplay command channel; the bridge hooks read this.
namespace runtime::netplay_session {

inline std::atomic<int> self_member{0};
inline std::atomic<int> base_port{0};

inline bool configured()
{
    int s = self_member.load(std::memory_order_acquire);
    int b = base_port.load(std::memory_order_acquire);
    return s >= 1 && s <= 254 && b > 0 && b + 254 <= 65535;
}

inline void configure(int self, int base)
{
    base_port.store(base, std::memory_order_release);
    self_member.store(self, std::memory_order_release);
}

inline void clear()
{
    self_member.store(0, std::memory_order_release);
    base_port.store(0, std::memory_order_release);
}

}
