import hashlib
import json
import logging
import secrets
import time
from collections import defaultdict, deque
from dataclasses import dataclass

import aiohttp
from aiohttp import web

from .report import ATTACHMENT, HEADER

log = logging.getLogger("swofty_triage.relay")

CONTENT_MAX = 2000
FILE_MAX = 10 << 20
SUPPRESS_EMBEDS = 1 << 2
TAG_SALT = secrets.token_bytes(16)


@dataclass(frozen=True)
class Limit:
    count: int
    window: float


IP_LIMITS = (Limit(12, 600), Limit(40, 86400))
IP_BYTES = Limit(120 << 20, 86400)
GLOBAL_LIMITS = (Limit(25, 60), Limit(400, 3600))


def client_ip(request: web.Request) -> str:
    peer = request.remote or ""
    forwarded = request.headers.get("X-Real-IP")
    if forwarded and peer in ("127.0.0.1", "::1"):
        return forwarded.strip()
    return peer


def refuse(
    status: int, message: str, retry_after: float | None = None, code: int | None = None
) -> web.Response:
    body: dict = {"message": message}
    if retry_after is not None:
        body["retry_after"] = round(retry_after, 1)
    if code is not None:
        body["code"] = code
    headers = {"Retry-After": str(int(retry_after) + 1)} if retry_after is not None else None
    return web.json_response(body, status=status, headers=headers)


class Window:
    def __init__(self) -> None:
        self.events: dict[str, deque[tuple[float, int]]] = defaultdict(deque)

    def wait(self, key: str, limit: Limit, now: float, weight: int = 1) -> float:
        q = self.events[key]
        while q and q[0][0] <= now - limit.window:
            q.popleft()
        used = sum(w for _, w in q)
        if used + weight <= limit.count:
            return 0.0
        total = used + weight
        for at, w in q:
            total -= w
            if total <= limit.count:
                return at + limit.window - now
        return limit.window

    def add(self, key: str, now: float, weight: int = 1) -> None:
        self.events[key].append((now, weight))

    def prune(self, now: float, horizon: float) -> None:
        for key in [k for k, q in self.events.items() if not q or q[-1][0] <= now - horizon]:
            del self.events[key]


class Relay:
    def __init__(self, webhook: str):
        self.webhook = webhook
        self.counts = Window()
        self.bytes = Window()
        self.sent = 0

    def limited(self, ip: str, size: int, now: float) -> float:
        waits = [self.counts.wait("*", limit, now) for limit in GLOBAL_LIMITS]
        waits += [self.counts.wait(ip, limit, now) for limit in IP_LIMITS]
        waits.append(self.bytes.wait(ip, IP_BYTES, now, size))
        return max(waits)

    def record(self, ip: str, size: int, now: float) -> None:
        self.counts.add("*", now)
        self.counts.add(ip, now)
        self.bytes.add(ip, now, size)
        if self.sent % 100 == 0:
            self.counts.prune(now, 86400)
            self.bytes.prune(now, 86400)
        self.sent += 1

    async def post(self, payload: dict, filename: str, data: bytes) -> tuple[int, dict]:
        form = aiohttp.FormData()
        form.add_field("payload_json", json.dumps(payload), content_type="application/json")
        form.add_field("files[0]", data, filename=filename, content_type="application/zip")
        timeout = aiohttp.ClientTimeout(total=120)
        async with aiohttp.ClientSession(timeout=timeout) as http:
            async with http.post(self.webhook, params={"wait": "true"}, data=form) as r:
                try:
                    body = await r.json(content_type=None)
                except ValueError:
                    body = {}
                return r.status, body if isinstance(body, dict) else {}


async def read_report(request: web.Request) -> tuple[dict, str, bytes] | web.Response:
    if not request.content_type.startswith("multipart/"):
        return refuse(400, "send multipart/form-data with payload_json and files[0]")
    reader = await request.multipart()
    payload_text, filename, data = None, None, None
    while (part := await reader.next()) is not None:
        if part.name == "payload_json" and payload_text is None:
            payload_text = (await part.read(decode=False))[: 16 << 10].decode("utf-8", "replace")
        elif part.name == "files[0]" and data is None:
            filename = part.filename or ""
            chunks, size = [], 0
            while chunk := await part.read_chunk(1 << 20):
                size += len(chunk)
                if size > FILE_MAX:
                    return refuse(413, "the logs part is larger than 10 MB", code=40005)
                chunks.append(chunk)
            data = b"".join(chunks)
        else:
            return refuse(400, f"unexpected form field {part.name!r}")
    if payload_text is None or data is None or filename is None:
        return refuse(400, "send payload_json and files[0]")
    try:
        payload = json.loads(payload_text)
    except ValueError:
        return refuse(400, "payload_json is not JSON")
    return payload, filename, data


def clean(payload: object, filename: str, data: bytes) -> dict | web.Response:
    content = payload.get("content") if isinstance(payload, dict) else None
    if not isinstance(content, str) or len(content) > CONTENT_MAX or not HEADER.match(content.strip()):
        return refuse(400, "the message is not a Swofty Launcher logs report")
    if not ATTACHMENT.match(filename):
        return refuse(400, "the attachment is not a Swofty Launcher logs zip")
    if not data.startswith(b"PK\x03\x04"):
        return refuse(400, "the attachment is not a zip")
    return {
        "content": content,
        "allowed_mentions": {"parse": []},
        "flags": SUPPRESS_EMBEDS,
        "attachments": [{"id": 0, "filename": filename}],
    }


def routes(app: web.Application, relay: Relay | None) -> None:
    async def send(request: web.Request) -> web.Response:
        if relay is None:
            return refuse(503, "sending logs is not set up on this server")
        ip = client_ip(request)
        ip_tag = hashlib.sha256(TAG_SALT + ip.encode()).hexdigest()[:10]
        started = time.monotonic()
        if (wait := relay.limited(ip, request.content_length or 0, time.time())) > 0:
            log.info("logs from %s limited for %.0fs", ip_tag, wait)
            return refuse(429, "too many logs sent from this network; try again later", retry_after=wait)
        got = await read_report(request)
        if isinstance(got, web.Response):
            return got
        payload, filename, data = got
        cleaned = clean(payload, filename, data)
        if isinstance(cleaned, web.Response):
            log.info("logs from %s refused: %s", ip_tag, cleaned.text)
            return cleaned
        now = time.time()
        if (wait := relay.limited(ip, len(data), now)) > 0:
            return refuse(429, "too many logs sent from this network; try again later", retry_after=wait)
        relay.record(ip, len(data), now)
        try:
            status, body = await relay.post(cleaned, filename, data)
        except (aiohttp.ClientError, TimeoutError) as e:
            log.warning("logs from %s could not reach Discord: %s", ip_tag, type(e).__name__)
            return refuse(502, "the log server could not reach Discord; try again in a minute")
        log.info(
            "logs from %s: %s (%d KB) -> Discord %d in %.1fs",
            ip_tag,
            filename,
            len(data) >> 10,
            status,
            time.monotonic() - started,
        )
        if 200 <= status < 300:
            return web.json_response({"ok": True, "id": body.get("id")})
        if status == 413 or body.get("code") == 40005:
            return refuse(413, "the logs part is too big for Discord", code=40005)
        if status == 429:
            return refuse(
                429, "Discord is limiting uploads right now", retry_after=float(body.get("retry_after", 5))
            )
        return refuse(502, f"Discord answered HTTP {status}")

    app.router.add_post("/v1/logs", send)
