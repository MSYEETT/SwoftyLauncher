// Host unit test for the events dispatcher. Compiles and runs natively (no game, no Wine): the
// clock is injected as a plain seconds value, so dt, the frame counter and the "seconds since
// attach" time are deterministic. run_events_test.sh builds and runs it.
#include "../src/core/events_dispatch.hpp"

#include <cstdio>
#include <string>
#include <vector>

using runtime::ev::Dispatcher;

static int fails;

static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

static std::vector<SwoftyEvent> g_events;
static void on_event(const SwoftyEvent *ev, void *user)
{
    if (user)
        *static_cast<int *>(user) += 1;
    g_events.push_back(*ev);
}

static std::vector<double> g_dts;
static void on_frame(double dt, void *user)
{
    if (user)
        *static_cast<int *>(user) += 1;
    g_dts.push_back(dt);
}

// A callback that unsubscribes itself and subscribes a fresh handler mid-dispatch, to prove the
// dispatcher does not deadlock or iterate a mutated vector.
static Dispatcher *g_reentrant_disp;
static uint32_t g_reentrant_token;
static int g_reentrant_added;
static void reentrant_frame(double, void *)
{
    g_reentrant_disp->unsubscribe(g_reentrant_token);
    uint32_t t = 0;
    g_reentrant_disp->on_frame(9, on_frame, &g_reentrant_added, &t);
}

int main()
{
    // Registration and result codes.
    {
        Dispatcher d;
        uint32_t t1 = 0, t2 = 0, t3 = 0;
        expect(d.subscribe(1, SWOFTY_EVENT_SESSION_START, on_event, nullptr, &t1) == SWOFTY_ASSET_OK && t1 != 0,
               "subscribe to a known event returns ok and a token");
        expect(d.on_frame(1, on_frame, nullptr, &t2) == SWOFTY_ASSET_OK && t2 != 0 && t2 != t1,
               "on_frame returns ok and a distinct token");
        expect(d.subscribe(1, "not.an.event", on_event, nullptr, &t3) == SWOFTY_ASSET_INVALID,
               "subscribe to an unknown event name is invalid");
        expect(d.subscribe(1, SWOFTY_EVENT_LAP, nullptr, nullptr, &t3) == SWOFTY_ASSET_INVALID,
               "subscribe with a null callback is invalid");
        expect(d.on_frame(1, nullptr, nullptr, &t3) == SWOFTY_ASSET_INVALID, "on_frame with a null callback is invalid");
        expect(d.unsubscribe(t1) == SWOFTY_ASSET_OK, "unsubscribe of a live token returns ok");
        expect(d.unsubscribe(t1) == SWOFTY_ASSET_INVALID, "unsubscribe of an already-dropped token is invalid");
        expect(d.unsubscribe(0xdead) == SWOFTY_ASSET_INVALID, "unsubscribe of an unknown token is invalid");
        expect(d.count() == 1, "one subscription remains after the drops");
    }

    // The flow and career events are known names and carry their payload pointer through.
    {
        Dispatcher d;
        g_events.clear();
        int hits = 0;
        uint32_t t = 0;
        for (const char *n : {SWOFTY_EVENT_FLOW_STATE, SWOFTY_EVENT_CAREER_LOADED, SWOFTY_EVENT_CAREER_UNLOADED,
                              SWOFTY_EVENT_CAREER_HUB})
            expect(d.subscribe(2, n, on_event, &hits, &t) == SWOFTY_ASSET_OK, std::string("subscribe ") + n);
        SwoftyCareerState s{};
        s.mode = SWOFTY_CAREER_DRIVER;
        expect(d.fire(SWOFTY_EVENT_CAREER_LOADED, SWOFTY_CAREER_DRIVER, 0, &s, 1.0) == 1 && hits == 1 &&
                   g_events.back().a == SWOFTY_CAREER_DRIVER && g_events.back().data == &s,
               "career.loaded reaches its subscriber with mode and state");
        expect(d.fire(SWOFTY_EVENT_CAREER_HUB, 1, SWOFTY_CAREER_DRIVER, &s, 1.5) == 1 && g_events.back().a == 1,
               "career.hub entered");
        expect(d.drop_owner(2) == 4, "the four subscriptions drop with their module");
    }

    // dt computation across flow ticks.
    {
        Dispatcher d;
        int frames = 0;
        d.on_frame(1, on_frame, &frames, nullptr);
        g_dts.clear();
        expect(d.tick(100.0) == 0.0, "the first flow tick reports dt 0 (no previous frame)");
        expect(d.tick(100.5) == 0.5, "the next tick reports the elapsed seconds");
        expect(d.tick(100.75) == 0.25, "and the one after reports the new delta");
        expect(d.frame() == 3, "three ticks advanced the frame counter to 3");
        expect(frames == 3 && g_dts.size() == 3, "the frame callback fired once per tick");
        expect(d.tick(100.70) == 0.0, "a non-monotonic clock reading clamps dt to 0 rather than going negative");
    }

    // Event payload and selective delivery.
    {
        Dispatcher d;
        int starts = 0;
        d.subscribe(1, SWOFTY_EVENT_SESSION_START, on_event, &starts, nullptr);
        d.subscribe(1, SWOFTY_EVENT_RACE_START, on_event, nullptr, nullptr);
        d.tick(10.0);
        d.tick(10.0); // frame counter now 2
        g_events.clear();
        int payload = 0xabc;
        size_t n = d.fire(SWOFTY_EVENT_SESSION_START, 7, 8, &payload, 12.0);
        expect(n == 1 && starts == 1, "fire delivers only to subscribers of that event name");
        expect(g_events.size() == 1 && g_events[0].a == 7 && g_events[0].b == 8 && g_events[0].data == &payload,
               "the event carries the a/b integers and the data pointer");
        expect(g_events[0].frame == 2 && g_events[0].time == 2.0,
               "the event carries the current frame and seconds since the first clock reading");
        expect(d.fire("not.an.event", 0, 0, nullptr, 13.0) == 0, "firing an unknown event name delivers to no one");
    }

    // Per-owner (per-addon) removal.
    {
        Dispatcher d;
        d.subscribe(100, SWOFTY_EVENT_SESSION_START, on_event, nullptr, nullptr);
        d.on_frame(100, on_frame, nullptr, nullptr);
        d.subscribe(200, SWOFTY_EVENT_SESSION_START, on_event, nullptr, nullptr);
        expect(d.count() == 3, "three subscriptions across two owners");
        expect(d.drop_owner(100) == 2, "dropping an owner removes exactly its subscriptions");
        expect(d.count() == 1, "the other owner's subscription survives");
        g_events.clear();
        d.fire(SWOFTY_EVENT_SESSION_START, 0, 0, nullptr, 1.0);
        expect(g_events.size() == 1, "only the surviving owner's handler fires");
        expect(d.drop_owner(999) == 0, "dropping an owner with no subscriptions removes nothing");
    }

    // Re-entrancy: a callback mutates the dispatcher while it is dispatching.
    {
        Dispatcher d;
        g_reentrant_disp = &d;
        g_reentrant_added = 0;
        d.on_frame(5, reentrant_frame, nullptr, &g_reentrant_token);
        d.tick(1.0); // reentrant_frame runs, drops itself, adds a new on_frame handler
        expect(d.count() == 1, "the self-replacing handler left exactly one subscription");
        d.tick(2.0); // the newly added handler runs this time
        expect(g_reentrant_added == 1, "the handler added mid-dispatch fires on the following tick, not the current one");
    }

    printf("%s\n", fails ? "events dispatcher: FAILED" : "events dispatcher: all checks passed");
    return fails ? 1 : 0;
}
