import asyncio
import json
import logging
import re
import sqlite3
import time
from collections import OrderedDict
from pathlib import Path
from typing import Any

from aiohttp import web

log = logging.getLogger("swofty_triage.usage")

ONLINE_S = 600
STEP_S = 300
HEARTBEAT_GAP_S = 50
BODY_MAX = 2048
RANGES = {"24h": (86400, 300), "7d": (7 * 86400, 1800), "30d": (30 * 86400, 7200), "90d": (90 * 86400, 21600)}

INSTALL = re.compile(r"^[0-9a-f]{32}$")
SESSION = re.compile(r"^[0-9a-f]{16,32}$")
VERSION = re.compile(r"^\d{1,3}\.\d{1,3}\.\d{1,3}(-[0-9A-Za-z.-]{1,20})?$")
OSES = {"windows", "linux", "steamos", "macos", "other"}
KINDS = {"launcher", "session_start", "session", "session_end"}

SCHEMA = """
create table if not exists installs (
    id text primary key,
    first_seen real not null,
    last_seen real not null,
    version text not null,
    os text not null,
    in_game_seen real not null default 0
);
create table if not exists sessions (
    install text not null,
    id text not null,
    version text not null,
    os text not null,
    started real not null,
    ended real,
    exit_code integer,
    seconds integer,
    primary key (install, id)
);
create index if not exists sessions_ended on sessions(ended);
create table if not exists samples (
    ts integer not null,
    version text not null,
    online integer not null,
    in_game integer not null,
    primary key (ts, version)
);
create table if not exists days (
    day text not null,
    install text not null,
    version text not null,
    primary key (day, install)
);
"""


class Invalid(Exception):
    pass


def version_key(v: str) -> tuple:
    head = v.split("-", 1)[0]
    return tuple(int(p) for p in head.split(".")) + ((1,) if "-" not in v else (0,))


def validate(body: Any) -> dict[str, Any]:
    if not isinstance(body, dict):
        raise Invalid("body must be a JSON object")
    kind, install = body.get("kind"), body.get("install")
    version, os_name = body.get("version"), body.get("os")
    if kind not in KINDS:
        raise Invalid("kind")
    if not isinstance(install, str) or not INSTALL.match(install):
        raise Invalid("install")
    if not isinstance(version, str) or not VERSION.match(version):
        raise Invalid("version")
    if os_name not in OSES:
        raise Invalid("os")
    event = {"kind": kind, "install": install, "version": version, "os": os_name}
    if kind != "launcher":
        session = body.get("session")
        if not isinstance(session, str) or not SESSION.match(session):
            raise Invalid("session")
        event["session"] = session
    if kind == "session_end":
        code, seconds = body.get("exit_code"), body.get("seconds", 0)
        if isinstance(code, bool) or not isinstance(code, int) or not -(2**31) <= code < 2**32:
            raise Invalid("exit_code")
        if isinstance(seconds, bool) or not isinstance(seconds, int) or not 0 <= seconds <= 10**7:
            raise Invalid("seconds")
        event["exit_code"] = code
        event["seconds"] = seconds
    return event


def crashed(code: int) -> bool:
    return code != 0


class Usage:
    def __init__(self, path: Path, new_per_hour: int = 600):
        path.parent.mkdir(parents=True, exist_ok=True)
        self.db = sqlite3.connect(path, isolation_level=None)
        self.db.row_factory = sqlite3.Row
        self.db.execute("pragma journal_mode=wal")
        self.db.executescript(SCHEMA)
        self.new_per_hour = new_per_hour
        self.recent: OrderedDict[str, float] = OrderedDict()
        self.new_window: list[float] = []

    def throttled(self, event: dict[str, Any], now: float) -> bool:
        key = f"{event['install']}:{event['kind']}"
        if event["kind"] in ("launcher", "session"):
            last = self.recent.get(key)
            if last is not None and now - last < HEARTBEAT_GAP_S:
                return True
            self.recent[key] = now
            self.recent.move_to_end(key)
            while len(self.recent) > 200_000:
                self.recent.popitem(last=False)
        return False

    def admit_new(self, now: float) -> bool:
        self.new_window = [t for t in self.new_window if now - t < 3600]
        if len(self.new_window) >= self.new_per_hour:
            return False
        self.new_window.append(now)
        return True

    def record(self, event: dict[str, Any], now: float | None = None) -> bool:
        now = time.time() if now is None else now
        if self.throttled(event, now):
            return False
        install, version, os_name, kind = event["install"], event["version"], event["os"], event["kind"]
        known = self.db.execute("select 1 from installs where id = ?", (install,)).fetchone()
        if not known and not self.admit_new(now):
            return False
        in_game = now if kind in ("session_start", "session") else None
        self.db.execute(
            """insert into installs (id, first_seen, last_seen, version, os, in_game_seen)
               values (?, ?, ?, ?, ?, ?)
               on conflict(id) do update set last_seen = excluded.last_seen, version = excluded.version,
                 os = excluded.os,
                 in_game_seen = case when ? = 'session_end' then 0
                                     when excluded.in_game_seen > 0 then excluded.in_game_seen
                                     else installs.in_game_seen end""",
            (install, now, now, version, os_name, in_game or 0, kind),
        )
        day = time.strftime("%Y-%m-%d", time.gmtime(now))
        self.db.execute(
            "insert into days values (?, ?, ?) on conflict(day, install) do update set version = excluded.version",
            (day, install, version),
        )
        if kind == "session_start":
            self.db.execute(
                "insert or ignore into sessions (install, id, version, os, started) values (?, ?, ?, ?, ?)",
                (install, event["session"], version, os_name, now),
            )
        elif kind == "session_end":
            self.db.execute(
                """insert into sessions (install, id, version, os, started, ended, exit_code, seconds)
                   values (?, ?, ?, ?, ?, ?, ?, ?)
                   on conflict(install, id) do update set ended = excluded.ended,
                     exit_code = excluded.exit_code, seconds = excluded.seconds""",
                (
                    install,
                    event["session"],
                    version,
                    os_name,
                    now - event["seconds"],
                    now,
                    event["exit_code"],
                    event["seconds"],
                ),
            )
        return True

    def online(self, now: float) -> list[dict[str, Any]]:
        rows = self.db.execute(
            """select version, count(*) as online, sum(in_game_seen >= ?) as in_game
               from installs where last_seen >= ? group by version""",
            (now - ONLINE_S, now - ONLINE_S),
        ).fetchall()
        return sorted(
            ({"version": r["version"], "online": r["online"], "in_game": r["in_game"] or 0} for r in rows),
            key=lambda r: version_key(r["version"]),
            reverse=True,
        )

    def sample(self, now: float | None = None) -> int:
        now = time.time() if now is None else now
        ts = int(now // STEP_S * STEP_S)
        rows = self.online(now)
        self.db.execute("delete from samples where ts = ?", (ts,))
        for r in rows:
            self.db.execute(
                "insert into samples values (?, ?, ?, ?)", (ts, r["version"], r["online"], r["in_game"])
            )
        self.db.execute(
            "insert into samples values (?, '*', ?, ?)",
            (ts, sum(r["online"] for r in rows), sum(r["in_game"] for r in rows)),
        )
        return ts

    def prune(self, now: float) -> None:
        self.db.execute("delete from sessions where started < ?", (now - 120 * 86400,))
        self.db.execute(
            "delete from days where day < ?", (time.strftime("%Y-%m-%d", time.gmtime(now - 400 * 86400)),)
        )

    def series(self, now: float, span: str) -> dict[str, Any]:
        seconds, bucket = RANGES.get(span, RANGES["7d"])
        start = now - seconds
        rows = self.db.execute(
            """select (ts / ?) * ? as b, version, sum(online) as online, sum(in_game) as in_game
               from samples where ts >= ? group by b, version order by b""",
            (bucket, bucket, start),
        ).fetchall()
        points: dict[int, dict[str, Any]] = {}
        versions: set[str] = set()
        counts = {
            r["b"]: r["n"]
            for r in self.db.execute(
                "select (ts / ?) * ? as b, count(*) as n from samples where ts >= ? and version = '*' group by b",
                (bucket, bucket, start),
            )
        }
        for r in rows:
            n = counts.get(r["b"]) or 1
            p = points.setdefault(r["b"], {"ts": r["b"], "total": 0, "in_game": 0, "versions": {}})
            if r["version"] == "*":
                p["total"], p["in_game"] = round(r["online"] / n), round(r["in_game"] / n)
            else:
                p["versions"][r["version"]] = round(r["online"] / n)
                versions.add(r["version"])
        return {
            "range": span if span in RANGES else "7d",
            "bucket": bucket,
            "versions": sorted(versions, key=version_key, reverse=True),
            "points": [points[k] for k in sorted(points)],
        }

    def crashes(self, now: float) -> list[dict[str, Any]]:
        out: dict[str, dict[str, Any]] = {}
        for label, window in (("24h", 86400), ("7d", 7 * 86400)):
            rows = self.db.execute(
                """select version, count(*) as sessions, sum(exit_code != 0) as crashed
                   from sessions where ended >= ? group by version""",
                (now - window,),
            ).fetchall()
            for r in rows:
                entry = out.setdefault(r["version"], {"version": r["version"]})
                entry[f"sessions_{label}"] = r["sessions"]
                entry[f"crashed_{label}"] = r["crashed"] or 0
        for entry in out.values():
            for label in ("24h", "7d"):
                entry.setdefault(f"sessions_{label}", 0)
                entry.setdefault(f"crashed_{label}", 0)
        return sorted(out.values(), key=lambda e: version_key(e["version"]), reverse=True)

    def daily(self, now: float, days: int = 30) -> list[dict[str, Any]]:
        since = time.strftime("%Y-%m-%d", time.gmtime(now - days * 86400))
        rows = self.db.execute(
            "select day, count(*) as installs from days where day > ? group by day order by day", (since,)
        ).fetchall()
        return [{"day": r["day"], "installs": r["installs"]} for r in rows]

    def active(self, now: float, window: int) -> int:
        return self.db.execute(
            "select count(*) from installs where last_seen >= ?", (now - window,)
        ).fetchone()[0]

    def stats(self, span: str = "7d", now: float | None = None) -> dict[str, Any]:
        now = time.time() if now is None else now
        online = self.online(now)
        return {
            "now": int(now),
            "online": {
                "total": sum(r["online"] for r in online),
                "in_game": sum(r["in_game"] for r in online),
                "by_version": online,
            },
            "active": {"24h": self.active(now, 86400), "7d": self.active(now, 7 * 86400)},
            "series": self.series(now, span),
            "crashes": self.crashes(now),
            "daily": self.daily(now),
        }


PAGE = Path(__file__).with_name("stats.html")


def app(usage: Usage) -> web.Application:
    async def events(request: web.Request) -> web.Response:
        if request.content_length is not None and request.content_length > BODY_MAX:
            return web.Response(status=413)
        raw = await request.content.read(BODY_MAX + 1)
        if len(raw) > BODY_MAX:
            return web.Response(status=413)
        try:
            event = validate(json.loads(raw))
        except (ValueError, Invalid) as e:
            return web.json_response({"error": str(e) or "invalid"}, status=400)
        usage.record(event)
        return web.Response(status=204)

    async def stats(request: web.Request) -> web.Response:
        span = request.query.get("range", "7d")
        return web.json_response(
            usage.stats(span),
            headers={"Cache-Control": "public, max-age=60", "Access-Control-Allow-Origin": "*"},
        )

    async def index(request: web.Request) -> web.StreamResponse:
        if request.host.split(":")[0].startswith("stats."):
            return web.FileResponse(PAGE, headers={"Cache-Control": "public, max-age=300"})
        return web.Response(text="Swofty Launcher API\n")

    async def health(_: web.Request) -> web.Response:
        return web.Response(text="ok\n")

    application = web.Application(client_max_size=BODY_MAX)
    application.router.add_post("/v1/events", events)
    application.router.add_get("/v1/stats", stats)
    application.router.add_get("/healthz", health)
    application.router.add_get("/", index)
    return application


async def sampler(usage: Usage) -> None:
    while True:
        await asyncio.sleep(STEP_S - time.time() % STEP_S + 1)
        try:
            usage.sample()
            if time.gmtime().tm_hour == 3 and time.gmtime().tm_min < 6:
                usage.prune(time.time())
        except sqlite3.Error:
            log.exception("sampling failed")


def serve(db_path: Path, host: str, port: int) -> None:
    usage = Usage(db_path)
    application = app(usage)

    async def start(app_: web.Application) -> None:
        app_["sampler"] = asyncio.create_task(sampler(usage))

    async def stop(app_: web.Application) -> None:
        app_["sampler"].cancel()

    application.on_startup.append(start)
    application.on_cleanup.append(stop)
    web.run_app(application, host=host, port=port, access_log=None, print=None)
