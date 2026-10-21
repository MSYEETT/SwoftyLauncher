import asyncio

import pytest
from aiohttp.test_utils import TestClient, TestServer

from swofty_triage import usage
from swofty_triage.usage import Invalid, Usage, validate

A = "a" * 32
B = "b" * 32
S1 = "1" * 16
NOW = 1_800_000_000.0


def event(kind="launcher", install=A, version="1.2.1", os="windows", **extra):
    return {"kind": kind, "install": install, "version": version, "os": os, **extra}


@pytest.fixture
def u(tmp_path):
    return Usage(tmp_path / "usage.db")


def test_validate_accepts_each_kind():
    assert validate(event())["kind"] == "launcher"
    assert validate(event("session_start", session=S1))["session"] == S1
    end = validate(event("session_end", session=S1, exit_code=-1, seconds=120))
    assert (end["exit_code"], end["seconds"]) == (-1, 120)


@pytest.mark.parametrize(
    "body",
    [
        [],
        event(kind="hello"),
        event(install="xyz"),
        event(version="latest"),
        event(os="amiga"),
        event("session_start"),
        event("session_end", session=S1, exit_code="1", seconds=1),
        event("session_end", session=S1, exit_code=True, seconds=1),
        event("session_end", session=S1, exit_code=0, seconds=-5),
    ],
)
def test_validate_rejects(body):
    with pytest.raises(Invalid):
        validate(body)


def test_heartbeats_are_throttled_per_install(u):
    assert u.record(validate(event()), NOW)
    assert not u.record(validate(event()), NOW + 10)
    assert u.record(validate(event(install=B)), NOW + 10)
    assert u.record(validate(event("session", session=S1)), NOW + 10)
    assert u.record(validate(event()), NOW + 60)


def test_online_counts_and_in_game(u):
    u.record(validate(event()), NOW)
    u.record(validate(event("session_start", install=B, version="1.2.0", session=S1)), NOW)
    rows = {r["version"]: r for r in u.online(NOW + 60)}
    assert rows["1.2.1"] == {"version": "1.2.1", "online": 1, "in_game": 0}
    assert rows["1.2.0"] == {"version": "1.2.0", "online": 1, "in_game": 1}
    u.record(
        validate(event("session_end", install=B, version="1.2.0", session=S1, exit_code=0, seconds=60)),
        NOW + 70,
    )
    assert {r["version"]: r["in_game"] for r in u.online(NOW + 80)}["1.2.0"] == 0
    assert u.online(NOW + usage.ONLINE_S + 100) == []


def test_samples_and_series(u):
    u.record(validate(event()), NOW)
    u.record(validate(event(install=B, version="1.2.0")), NOW)
    ts = u.sample(NOW + 1)
    s = u.series(NOW + 2, "24h")
    assert s["versions"] == ["1.2.1", "1.2.0"]
    assert s["points"] == [
        {"ts": ts // 300 * 300, "total": 2, "in_game": 0, "versions": {"1.2.1": 1, "1.2.0": 1}}
    ]


def test_crash_rate(u):
    for i, code in enumerate([0, -1, 0, 1]):
        session = f"{i:016x}"
        u.record(validate(event("session_start", install=f"{i:032x}", session=session)), NOW)
        u.record(
            validate(event("session_end", install=f"{i:032x}", session=session, exit_code=code, seconds=30)),
            NOW,
        )
    [row] = u.crashes(NOW + 10)
    assert row == {"version": "1.2.1", "sessions_24h": 4, "crashed_24h": 2, "sessions_7d": 4, "crashed_7d": 2}


def test_new_install_cap(tmp_path):
    capped = Usage(tmp_path / "capped.db", new_per_hour=2)
    assert capped.record(validate(event(install="1" * 32)), NOW)
    assert capped.record(validate(event(install="2" * 32)), NOW)
    assert not capped.record(validate(event(install="3" * 32)), NOW)
    assert capped.record(validate(event(install="1" * 32)), NOW + 100)


def test_stats_shape(u):
    u.record(validate(event()), NOW)
    s = u.stats("7d", NOW + 1)
    assert s["online"]["total"] == 1
    assert s["active"] == {"24h": 1, "7d": 1}
    assert s["daily"][-1]["installs"] == 1


def test_http(tmp_path):
    async def run():
        client = TestClient(TestServer(usage.app(Usage(tmp_path / "http.db"))))
        await client.start_server()
        try:
            r = await client.post("/v1/events", json=event())
            assert r.status == 204
            r = await client.post("/v1/events", json={"kind": "nope"})
            assert r.status == 400
            r = await client.post(
                "/v1/events", data=b"x" * 5000, headers={"Content-Type": "application/json"}
            )
            assert r.status == 413
            r = await client.get("/v1/stats?range=24h")
            assert (await r.json())["online"]["total"] == 1
            r = await client.get("/", headers={"Host": "stats.f1.swofty.net"})
            assert "Swofty Launcher for F1 25" in await r.text()
            r = await client.get("/", headers={"Host": "api.f1.swofty.net"})
            assert (await r.text()).startswith("Swofty Launcher API")
        finally:
            await client.close()

    asyncio.run(run())
