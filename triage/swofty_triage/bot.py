import asyncio
import json
import logging
import shutil
import time
from dataclasses import dataclass
from datetime import datetime, timezone

import aiohttp
import discord

from . import releases, render, report
from .agent import Declined
from .pipeline import Limited, Result, Triage
from .report import Header

log = logging.getLogger("swofty_triage.bot")

WORKING = "🔍"
DONE = "✅"
FAILED = "⚠️"
SKIPPED = "⏭️"
RETRY = "🔁"
CASE_DAYS = 30
STATS_NAME = "🏎️ F1 Online: {:,}"
STATS_EVERY_S = 600


@dataclass(frozen=True)
class Job:
    anchor_id: int
    stem: str
    part_ids: tuple[int, ...]
    force: bool = False


class TriageBot(discord.Client):
    def __init__(self, triage: Triage):
        intents = discord.Intents.none()
        intents.guilds = True
        intents.guild_messages = True
        intents.guild_reactions = True
        intents.message_content = True
        super().__init__(intents=intents, allowed_mentions=discord.AllowedMentions.none())
        self.triage = triage
        self.cfg = triage.cfg
        self.queue: asyncio.Queue[Job] = asyncio.Queue()
        self.pending: set[int] = set()
        self.ready_once = asyncio.Event()

    async def setup_hook(self) -> None:
        self.tasks = [
            asyncio.create_task(self.worker()),
            asyncio.create_task(self.sweeper()),
            asyncio.create_task(self.release_watcher()),
            asyncio.create_task(self.stats_channel()),
        ]

    async def channel(self) -> discord.TextChannel:
        channel = self.get_channel(self.cfg.channel_id) or await self.fetch_channel(self.cfg.channel_id)
        if not isinstance(channel, discord.TextChannel):
            raise RuntimeError(f"{self.cfg.channel_id} is not a text channel")
        return channel

    async def on_ready(self) -> None:
        log.info("connected as %s", self.user)
        await asyncio.to_thread(self.triage.sources.update)
        self.ready_once.set()
        channel = await self.channel()
        for report_id in self.triage.store.interrupted():
            try:
                await self.consider(await channel.fetch_message(int(report_id)))
            except discord.HTTPException as e:
                log.warning("interrupted report %s: %s", report_id, e)
        seen = int(self.triage.store.meta("last_seen") or 0)
        start = seen or (self.cfg.backfill_from - 1 if self.cfg.backfill_from else 0)
        if not start:
            last = [m async for m in channel.history(limit=1)]
            if last:
                self.triage.store.set_meta("last_seen", str(last[0].id))
            return
        async for message in channel.history(limit=None, after=discord.Object(id=start), oldest_first=True):
            await self.consider(message)

    async def on_message(self, message: discord.Message) -> None:
        if message.channel.id == self.cfg.channel_id:
            await self.consider(message)

    async def on_raw_reaction_add(self, event: discord.RawReactionActionEvent) -> None:
        if event.channel_id != self.cfg.channel_id or str(event.emoji) != RETRY:
            return
        if self.user and event.user_id == self.user.id:
            return
        if event.member is not None and event.member.bot:
            return
        channel = await self.channel()
        try:
            message = await channel.fetch_message(event.message_id)
        except discord.HTTPException:
            return
        await self.consider(message, force=True)

    async def consider(self, message: discord.Message, force: bool = False) -> None:
        if message.webhook_id != self.cfg.webhook_id:
            return
        header = report.parse_header(message.content)
        attachment = next((a for a in message.attachments if report.parse_attachment(a.filename)), None)
        if header is None or attachment is None:
            return
        seen = int(self.triage.store.meta("last_seen") or 0)
        if message.id > seen:
            self.triage.store.set_meta("last_seen", str(message.id))
        stem, _ = report.parse_attachment(attachment.filename)
        if header.total > 1:
            self.triage.store.add_part(
                stem,
                header.part,
                header.total,
                str(message.id),
                str(message.channel.id),
                message.created_at.timestamp(),
            )
            parts = self.triage.store.parts(stem)
            if len(parts) < header.total and not force:
                return
            ids = tuple(int(p["message_id"]) for p in parts)
            anchor = next((int(p["message_id"]) for p in parts if p["part"] == 1), ids[0])
        else:
            ids, anchor = (message.id,), message.id
        if anchor in self.pending or (not force and self.triage.store.handled(str(anchor))):
            return
        self.pending.add(anchor)
        await self.queue.put(Job(anchor, stem, ids, force))

    async def worker(self) -> None:
        await self.ready_once.wait()
        while True:
            job = await self.queue.get()
            try:
                await self.process(job)
            except Exception:
                log.exception("job %s failed", job.anchor_id)
            finally:
                self.pending.discard(job.anchor_id)
                self.queue.task_done()

    async def react(self, message: discord.Message, add: str | None, remove: str | None = None) -> None:
        try:
            if remove and self.user:
                await message.remove_reaction(remove, self.user)
            if add:
                await message.add_reaction(add)
        except discord.HTTPException as e:
            log.warning("reaction on %s: %s", message.id, e)

    async def process(self, job: Job) -> None:
        channel = await self.channel()
        messages = [await channel.fetch_message(i) for i in job.part_ids]
        anchor = next(m for m in messages if m.id == job.anchor_id)
        header = report.parse_header(anchor.content)
        if header is None:
            return
        received = anchor.created_at.timestamp()
        if not job.force:
            try:
                self.triage.check_limits(header, received)
            except Limited as e:
                log.warning("skipping %s: %s", anchor.id, e)
                self.triage.store.begin(
                    str(anchor.id), job.stem, header.name, header.launcher, header.message, received
                )
                self.triage.store.finish(str(anchor.id), "skipped", error=str(e))
                await self.react(anchor, SKIPPED)
                return
        await self.react(anchor, WORKING, FAILED)
        zips = []
        for m in messages:
            for a in m.attachments:
                if report.parse_attachment(a.filename):
                    zips.append((a.filename, await a.read()))
        try:
            result = await asyncio.to_thread(self.triage.run, str(anchor.id), header, zips, received)
        except Exception as e:
            log.exception("triage of %s failed", anchor.id)
            reason = f"declined: {e}" if isinstance(e, Declined) else f"{type(e).__name__}: {e}"
            thread = await self.thread_for(anchor, header, None)
            await thread.send(
                f"Triage failed — {render.cut(reason, 1500)}\nReact {RETRY} on the report to try again."
            )
            await self.react(anchor, FAILED, WORKING)
            return
        thread = await self.thread_for(anchor, header, result.diagnosis)
        await self.post(thread, anchor, result)
        self.triage.store.set_thread(str(anchor.id), str(thread.id))
        self.triage.store.drop_parts(job.stem)
        await self.react(anchor, DONE, WORKING)

    async def thread_for(self, anchor: discord.Message, header: Header, diagnosis) -> discord.abc.Messageable:
        name = render.thread_name(diagnosis, header)
        thread = anchor.thread
        if thread is None:
            try:
                return await anchor.create_thread(name=name, auto_archive_duration=10080)
            except discord.HTTPException as e:
                log.warning("thread on %s: %s", anchor.id, e)
                return anchor.channel
        if diagnosis is not None and thread.name != name:
            try:
                await thread.edit(name=name, archived=False)
            except discord.HTTPException:
                pass
        return thread

    async def post(self, thread: discord.abc.Messageable, anchor: discord.Message, result: Result) -> None:
        for text in render.messages(result.diagnosis, result.cost_usd, result.turns, anchor.jump_url):
            await thread.send(text, suppress_embeds=True)

    async def sweeper(self) -> None:
        await self.ready_once.wait()
        while True:
            await asyncio.sleep(600)
            try:
                for stem in self.triage.store.waiting_stems(time.time() - self.cfg.part_wait_s):
                    parts = self.triage.store.parts(stem)
                    if not parts:
                        continue
                    channel = await self.channel()
                    first = await channel.fetch_message(int(parts[0]["message_id"]))
                    await self.consider(first, force=True)
                await asyncio.to_thread(prune_cases, self.cfg.cases_dir, CASE_DAYS)
            except Exception:
                log.exception("sweep failed")

    async def release_watcher(self) -> None:
        await self.ready_once.wait()
        if not self.cfg.releases_channel_id:
            return
        gh = releases.GitHub(self.cfg.releases_repo, self.cfg.github_token or None)
        while True:
            try:
                await self.announce_releases(gh)
            except Exception:
                log.exception("release check failed")
            await asyncio.sleep(self.cfg.releases_poll_s)

    async def announce_releases(self, gh: releases.GitHub) -> None:
        store = self.triage.store
        found = await asyncio.to_thread(gh.releases)
        raw = store.meta("releases_seen")
        if raw is None:
            since = since_time(self.cfg.releases_since)
            seen = {r.tag for r in found if r.published < since}
            store.set_meta("releases_seen", json.dumps(sorted(seen)))
        else:
            seen = set(json.loads(raw))
        new = sorted((r for r in found if r.tag not in seen), key=lambda r: r.published)
        if not new:
            return
        channel = self.get_channel(self.cfg.releases_channel_id) or await self.fetch_channel(
            self.cfg.releases_channel_id
        )
        for release in new:
            key = f"release_text:{release.tag}"
            text = store.meta(key)
            if text is None:
                text = await asyncio.to_thread(
                    releases.compose,
                    gh,
                    self.triage.sources,
                    self.triage.agent.client,
                    self.cfg.model,
                    self.cfg.releases_project,
                    self.cfg.releases_docs,
                    release,
                    found,
                )
                store.set_meta(key, text)
            for chunk in render.split(text):
                message = await channel.send(chunk, suppress_embeds=True)
                if isinstance(channel, discord.TextChannel) and channel.is_news():
                    try:
                        await message.publish()
                    except discord.HTTPException as e:
                        log.warning("publishing %s: %s", release.tag, e)
            seen.add(release.tag)
            store.set_meta("releases_seen", json.dumps(sorted(seen)))
            log.info("announced %s in %s", release.tag, self.cfg.releases_channel_id)

    async def stats_channel(self) -> None:
        await self.ready_once.wait()
        if not self.cfg.stats_channel_id:
            return
        url = f"{self.cfg.usage_url}/v1/stats?range=24h"
        async with aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=20)) as http:
            while True:
                try:
                    async with http.get(url) as r:
                        online = (await r.json())["online"]["total"]
                    channel = self.get_channel(self.cfg.stats_channel_id) or await self.fetch_channel(
                        self.cfg.stats_channel_id
                    )
                    name = STATS_NAME.format(online)
                    if channel.name != name:
                        await channel.edit(name=name, reason="F1 players online")
                except Exception as e:
                    log.warning("stats channel update failed: %s", e)
                await asyncio.sleep(STATS_EVERY_S)


def since_time(raw: str) -> datetime:
    if raw:
        return datetime.fromisoformat(raw.replace("Z", "+00:00"))
    return datetime.now(timezone.utc)


def prune_cases(cases_dir, days: int) -> None:
    if not cases_dir.is_dir():
        return
    cutoff = time.time() - days * 86400
    for case in cases_dir.iterdir():
        for sub in ("logs", "zips"):
            path = case / sub
            if path.is_dir() and path.stat().st_mtime < cutoff:
                shutil.rmtree(path, ignore_errors=True)
