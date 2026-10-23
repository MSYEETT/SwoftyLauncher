import asyncio
import io
import json
import zipfile

import pytest
from aiohttp import FormData, web
from aiohttp.test_utils import TestClient, TestServer

from swofty_triage import relay as relay_mod
from swofty_triage import usage
from swofty_triage.relay import Limit, Relay, Window

CONTENT = "**Logs from `racer`**\nLauncher 1.3.7 · Windows 11 · F1 25 1.0.141 <@&123> https://example.com\n>>> it crashed @everyone"
NAME = "swofty-logs-racer-20261024-010203.zip"


def zip_data() -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        z.writestr("launcher.log", "hello")
    return buf.getvalue()


def form(content=CONTENT, name=NAME, data=None, extra=None):
    f = FormData()
    payload = {
        "content": content,
        "allowed_mentions": {"parse": ["everyone"]},
        "username": "Swofty",
        **(extra or {}),
    }
    f.add_field("payload_json", json.dumps(payload), content_type="application/json")
    f.add_field(
        "files[0]", data if data is not None else zip_data(), filename=name, content_type="application/zip"
    )
    return f


def test_window_wait():
    w = Window()
    limit = Limit(2, 10)
    assert w.wait("a", limit, 0) == 0
    w.add("a", 0)
    w.add("a", 5)
    assert w.wait("a", limit, 6) == pytest.approx(4)
    assert w.wait("a", limit, 10.5) == 0
    assert w.wait("b", limit, 6) == 0


def run(handler, check, tmp):
    async def go():
        hook = web.Application()
        seen = []

        async def receive(request):
            reader = await request.multipart()
            parts = {}
            while (part := await reader.next()) is not None:
                parts[part.name] = (part.filename, await part.read())
            seen.append((dict(request.query), json.loads(parts["payload_json"][1]), parts["files[0]"]))
            return await handler(request)

        hook.router.add_post("/api/webhooks/1/token", receive)
        hook_server = TestServer(hook)
        await hook_server.start_server()
        relay = Relay(str(hook_server.make_url("/api/webhooks/1/token")))
        client = TestClient(TestServer(usage.app(usage.Usage(tmp / "u.db"), relay)))
        await client.start_server()
        try:
            await check(client, seen)
        finally:
            await client.close()
            await hook_server.close()

    asyncio.run(go())


def test_relays_a_clean_report(tmp_path):
    async def ok(_):
        return web.json_response({"id": "42"})

    async def check(client, seen):
        r = await client.post("/v1/logs", data=form(), headers={"X-Real-IP": "203.0.113.9"})
        assert r.status == 200 and (await r.json())["id"] == "42"
        query, payload, (filename, data) = seen[0]
        assert query == {"wait": "true"}
        assert payload == {
            "content": CONTENT,
            "allowed_mentions": {"parse": []},
            "flags": 4,
            "attachments": [{"id": 0, "filename": NAME}],
        }
        assert filename == NAME and data == zip_data()

    run(ok, check, tmp_path)


@pytest.mark.parametrize(
    "kwargs,status",
    [
        ({"content": "hello"}, 400),
        ({"name": "evil.exe"}, 400),
        ({"data": b"MZ not a zip"}, 400),
        ({"data": b"PK\x03\x04" + b"0" * (relay_mod.FILE_MAX + 1)}, 413),
    ],
)
def test_refuses_non_reports(kwargs, status, tmp_path):
    async def never(_):
        raise AssertionError("posted to Discord")

    async def check(client, seen):
        r = await client.post("/v1/logs", data=form(**kwargs))
        assert r.status == status
        assert not seen

    run(never, check, tmp_path)


def test_rate_limits_per_ip(monkeypatch, tmp_path):
    monkeypatch.setattr(relay_mod, "IP_LIMITS", (Limit(2, 600),))

    async def ok(_):
        return web.json_response({"id": "1"})

    async def check(client, seen):
        for _ in range(2):
            assert (
                await client.post("/v1/logs", data=form(), headers={"X-Real-IP": "198.51.100.1"})
            ).status == 200
        r = await client.post("/v1/logs", data=form(), headers={"X-Real-IP": "198.51.100.1"})
        assert r.status == 429 and (await r.json())["retry_after"] > 60
        assert (
            await client.post("/v1/logs", data=form(), headers={"X-Real-IP": "198.51.100.2"})
        ).status == 200
        assert len(seen) == 3

    run(ok, check, tmp_path)


def test_maps_discord_errors(tmp_path):
    answers = iter(
        [
            web.json_response({"message": "Request entity too large", "code": 40005}, status=413),
            web.json_response({"message": "You are being rate limited.", "retry_after": 3.5}, status=429),
            web.json_response({"message": "Unknown Webhook", "code": 10015}, status=404),
        ]
    )

    async def handler(_):
        return next(answers)

    async def check(client, seen):
        r = await client.post("/v1/logs", data=form())
        assert r.status == 413 and (await r.json())["code"] == 40005
        r = await client.post("/v1/logs", data=form())
        assert r.status == 429 and (await r.json())["retry_after"] == 3.5
        r = await client.post("/v1/logs", data=form())
        assert r.status == 502 and "token" not in await r.text()

    run(handler, check, tmp_path)


def test_unconfigured_relay_is_503(tmp_path):
    async def go():
        client = TestClient(TestServer(usage.app(usage.Usage(tmp_path / "u.db"))))
        await client.start_server()
        try:
            assert (await client.post("/v1/logs", data=form())).status == 503
        finally:
            await client.close()

    asyncio.run(go())
