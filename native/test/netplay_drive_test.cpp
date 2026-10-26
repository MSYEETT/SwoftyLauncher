// Host unit test for the netplay command logic (netplay_drive_logic.hpp): command parsing, the IPv4
// check, state gating and the reply JSON. run_events_test.sh builds and runs it.
#include "../src/core/netplay_drive_logic.hpp"

#include <cstdio>
#include <string>

namespace drive = runtime::drive;

static int fails;

static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

static drive::State main_menu(bool armed = true, bool configured = true)
{
    drive::State s;
    s.armed = armed;
    s.configured = configured;
    s.self = configured ? 2 : 0;
    s.base = configured ? 40000 : 0;
    s.frontend = true;
    s.home_screen = true;
    return s;
}

int main()
{
    auto c = drive::parse(R"({"seq": 12, "op": "host", "args": {"category": "f1-2026"}})");
    expect(c.error.empty() && c.op == drive::Op::host && c.seq == 12 && c.category == drive::Category::f1_2026,
           "host with a category parses");
    expect(drive::parse(R"({"seq":1,"op":"host","args":{"category":"f1-2025"}})").category == drive::Category::f1_2025,
           "f1-2025 parses");
    expect(!drive::parse(R"({"seq":1,"op":"host"})").error.empty(), "host without a category is an error");
    expect(drive::parse(R"({"seq":1,"op":"host","args":{"category":"f2"}})").error.find("unknown category") == 0,
           "an unknown category names itself");
    c = drive::parse(R"({"seq":3,"op":"join","args":{"ip":"10.77.0.1"}})");
    expect(c.error.empty() && c.op == drive::Op::join && c.ip == "10.77.0.1", "join with an IPv4 parses");
    expect(!drive::parse(R"({"seq":3,"op":"join","args":{"ip":"10.77.0"}})").error.empty(), "a short address is refused");
    expect(!drive::parse(R"({"seq":3,"op":"join"})").error.empty(), "join without an ip is an error");
    expect(drive::parse(R"({"seq":4,"op":"status"})").op == drive::Op::status, "status parses");
    expect(drive::parse(R"({"seq":5,"op":"leave"})").op == drive::Op::leave, "leave parses");
    expect(drive::parse(R"({"seq":5,"op":"dance"})").error == "unknown op 'dance'", "an unknown op is named");
    expect(drive::parse("[1,2]").error == "not a JSON object", "a non-object is refused");
    expect(drive::parse(R"({"op":"status"})").seq == 0, "a missing seq stays 0");

    expect(drive::valid_ipv4("192.168.1.20"), "a LAN address is valid");
    expect(drive::valid_ipv4("10.77.0.254"), "a virtual address is valid");
    expect(!drive::valid_ipv4("256.1.1.1"), "a part over 255 is invalid");
    expect(!drive::valid_ipv4("01.2.3.4"), "a leading zero is invalid");
    expect(!drive::valid_ipv4("1.2.3.4.5"), "five parts are invalid");
    expect(!drive::valid_ipv4("1.2.3.4 "), "trailing text is invalid");
    expect(!drive::valid_ipv4("255.255.255.255"), "the broadcast address is invalid");
    expect(!drive::valid_ipv4("0.0.0.0"), "the any address is invalid");
    expect(!drive::valid_ipv4("host.lan"), "a name is invalid");

    auto host = drive::parse(R"({"seq":1,"op":"host","args":{"category":"f1-2026"}})");
    auto join = drive::parse(R"({"seq":2,"op":"join","args":{"ip":"10.77.0.1"}})");
    auto leave = drive::parse(R"({"seq":3,"op":"leave"})");
    auto status = drive::parse(R"({"seq":4,"op":"status"})");
    expect(drive::refuse(host, main_menu()).empty(), "host runs from the main menu");
    expect(drive::refuse(join, main_menu()).empty(), "join runs from the main menu");
    expect(drive::refuse(host, main_menu(false)).find("not armed") != std::string::npos,
           "host is refused without SWOFTY_NETPLAY");
    expect(drive::refuse(join, main_menu(true, false)) == "no room is set; send configure first",
           "join is refused before configure");
    expect(drive::refuse(status, main_menu(false)).empty(), "status runs without SWOFTY_NETPLAY");
    drive::State sub = main_menu();
    sub.home_screen = false;
    expect(sub.screen() == drive::Screen::other, "a front end sub-screen is other");
    expect(drive::refuse(host, sub) == "the game is not on its main menu", "host is refused off the main menu");
    drive::State in_game;
    in_game.armed = true;
    in_game.home_screen = true;
    expect(in_game.screen() == drive::Screen::other, "fe_home without StateFrontend is other");
    drive::State lobby = main_menu();
    lobby.lobby = true;
    lobby.role = drive::Role::host;
    expect(lobby.screen() == drive::Screen::lan_lobby && lobby.in_session(), "StateOnlineLobby is the LAN lobby");
    expect(drive::refuse(join, lobby).find("already in a LAN session") == 0, "join is refused in a lobby");
    expect(drive::refuse(leave, lobby).empty(), "leave runs in a lobby");
    expect(drive::refuse(leave, main_menu()) == "not in a LAN session", "leave is refused outside a session");
    drive::State busy = main_menu();
    busy.busy = true;
    expect(drive::refuse(host, busy) == "a netplay command is still running", "a running command blocks the next");

    expect(drive::status_json(lobby) ==
               R"({"screen":"lan_lobby","in_session":true,"role":"host","armed":true,"configured":true,"self":2,"base":40000})",
           "status JSON in a hosted lobby");
    expect(drive::status_json(main_menu(false, false)) ==
               R"({"screen":"main_menu","in_session":false,"role":"none","armed":false,"configured":false,"self":0,"base":0})",
           "status JSON on the main menu");

    auto conf = drive::parse(R"({"seq":20,"op":"configure","args":{"self":3,"base":40000}})");
    expect(conf.error.empty() && conf.op == drive::Op::configure && conf.self == 3 && conf.base == 40000,
           "configure parses");
    expect(!drive::parse(R"({"seq":20,"op":"configure","args":{"self":0,"base":40000}})").error.empty(), "self 0 is refused");
    expect(!drive::parse(R"({"seq":20,"op":"configure","args":{"self":255,"base":40000}})").error.empty(), "self 255 is refused");
    expect(!drive::parse(R"({"seq":20,"op":"configure","args":{"self":2,"base":65282}})").error.empty(),
           "a base that leaves no room for 254 members is refused");
    expect(drive::parse(R"({"seq":20,"op":"configure","args":{"self":254,"base":65281}})").error.empty(), "the top values parse");
    expect(!drive::parse(R"({"seq":20,"op":"configure","args":{"self":2.5,"base":40000}})").error.empty(), "a fraction is refused");
    expect(!drive::parse(R"({"seq":20,"op":"configure","args":{"self":2}})").error.empty(), "configure needs a base");
    auto clear = drive::parse(R"({"seq":21,"op":"clear"})");
    expect(clear.op == drive::Op::clear && clear.error.empty(), "clear parses");
    expect(drive::refuse(conf, main_menu(true, false)).empty(), "configure runs on the main menu");
    expect(drive::refuse(conf, lobby).empty(), "configure runs in a LAN lobby");
    expect(drive::refuse(conf, sub) == "the room can be set only on the main menu or in a LAN lobby",
           "configure is refused elsewhere");
    expect(drive::refuse(conf, main_menu(false, false)).find("not armed") != std::string::npos,
           "configure needs SWOFTY_NETPLAY");
    expect(drive::refuse(clear, sub).empty() && drive::refuse(clear, main_menu(false, false)).empty(),
           "clear runs anywhere");
    std::string r = drive::reply_json(join, lobby, "already in a \"LAN\" session", 7);
    expect(r == R"({"seq":2,"op":"join","ok":false,"error":"already in a \"LAN\" session","status":{"screen":"lan_lobby","in_session":true,"role":"host","armed":true,"configured":true,"self":2,"base":40000},"t":7})",
           "a refused reply carries the escaped error and the status");
    json::Value v;
    expect(json::parse(drive::reply_json(status, main_menu(), "", 9), v) && v["ok"].type == json::Value::Bool && v["ok"].b,
           "an ok reply parses back");

    uint64_t seq = 0;
    expect(drive::inbox_name("1792467946485.json", &seq) && seq == 1792467946485ull, "a numeric inbox name is read");
    expect(!drive::inbox_name("12.json.tmp", &seq), "a temp file is ignored");
    expect(!drive::inbox_name("x1.json", &seq), "a non-numeric name is ignored");
    expect(!drive::inbox_name("0.json", &seq), "seq 0 is ignored");

    printf("%s\n", fails ? "FAILED" : "all ok");
    return fails ? 1 : 0;
}
