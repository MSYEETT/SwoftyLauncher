import json
import sqlite3
import threading
import time
from pathlib import Path
from typing import Any

SCHEMA = """
create table if not exists reports (
    id text primary key,
    stem text not null,
    name text not null,
    launcher text not null,
    message text not null,
    received real not null,
    status text not null,
    signature text not null default '',
    thread_id text,
    diagnosis text,
    error text,
    cost_usd real not null default 0,
    turns integer not null default 0,
    updated real not null
);
create index if not exists reports_received on reports(received);
create table if not exists parts (
    stem text not null,
    part integer not null,
    total integer not null,
    message_id text not null,
    channel_id text not null,
    received real not null,
    primary key (stem, part)
);
create table if not exists meta (key text primary key, value text not null);
"""

DONE = ("done", "skipped")


class Store:
    def __init__(self, path: Path):
        path.parent.mkdir(parents=True, exist_ok=True)
        self.db = sqlite3.connect(path, check_same_thread=False, isolation_level=None)
        self.db.row_factory = sqlite3.Row
        self.lock = threading.Lock()
        with self.lock:
            self.db.executescript(SCHEMA)

    def get(self, report_id: str) -> dict[str, Any] | None:
        with self.lock:
            row = self.db.execute("select * from reports where id = ?", (report_id,)).fetchone()
        return dict(row) if row else None

    def begin(
        self, report_id: str, stem: str, name: str, launcher: str, message: str, received: float
    ) -> None:
        now = time.time()
        with self.lock:
            self.db.execute(
                """insert into reports (id, stem, name, launcher, message, received, status, updated)
                   values (?, ?, ?, ?, ?, ?, 'running', ?)
                   on conflict(id) do update set status = 'running', error = null, updated = excluded.updated""",
                (report_id, stem, name, launcher, message, received, now),
            )

    def finish(
        self,
        report_id: str,
        status: str,
        *,
        signature: str = "",
        diagnosis: dict[str, Any] | None = None,
        error: str | None = None,
        cost_usd: float = 0.0,
        turns: int = 0,
        thread_id: str | None = None,
    ) -> None:
        with self.lock:
            self.db.execute(
                """update reports set status = ?, signature = ?, diagnosis = ?, error = ?,
                   cost_usd = cost_usd + ?, turns = ?, thread_id = coalesce(?, thread_id), updated = ?
                   where id = ?""",
                (
                    status,
                    signature,
                    json.dumps(diagnosis, ensure_ascii=False) if diagnosis else None,
                    error,
                    cost_usd,
                    turns,
                    thread_id,
                    time.time(),
                    report_id,
                ),
            )

    def set_thread(self, report_id: str, thread_id: str) -> None:
        with self.lock:
            self.db.execute("update reports set thread_id = ? where id = ?", (thread_id, report_id))

    def handled(self, report_id: str) -> bool:
        row = self.get(report_id)
        return bool(row and row["status"] in DONE)

    def interrupted(self) -> list[str]:
        with self.lock:
            rows = self.db.execute(
                "select id from reports where status = 'running' order by received"
            ).fetchall()
        return [r[0] for r in rows]

    def recent(self, limit: int, exclude: str | None = None) -> list[dict[str, Any]]:
        with self.lock:
            rows = self.db.execute(
                """select id, received, name, launcher, signature, diagnosis from reports
                   where status = 'done' and id != coalesce(?, '') order by received desc limit ?""",
                (exclude, limit),
            ).fetchall()
        out = []
        for row in rows:
            d = json.loads(row["diagnosis"] or "{}")
            out.append(
                {
                    "id": row["id"],
                    "received": time.strftime("%Y-%m-%d %H:%M", time.gmtime(row["received"])),
                    "name": row["name"],
                    "launcher": row["launcher"],
                    "signature": row["signature"],
                    "title": d.get("title", ""),
                    "category": d.get("category", ""),
                    "summary": d.get("summary", "")[:400],
                    "fixed_on_main": d.get("fixed_on_main", ""),
                }
            )
        return out

    def count_since(self, since: float, name: str | None = None) -> int:
        query = "select count(*) from reports where received >= ? and status in ('done', 'running', 'failed')"
        args: list[Any] = [since]
        if name is not None:
            query += " and name = ?"
            args.append(name)
        with self.lock:
            return self.db.execute(query, args).fetchone()[0]

    def add_part(
        self, stem: str, part: int, total: int, message_id: str, channel_id: str, received: float
    ) -> None:
        with self.lock:
            self.db.execute(
                "insert or ignore into parts values (?, ?, ?, ?, ?, ?)",
                (stem, part, total, message_id, channel_id, received),
            )

    def parts(self, stem: str) -> list[dict[str, Any]]:
        with self.lock:
            rows = self.db.execute("select * from parts where stem = ? order by part", (stem,)).fetchall()
        return [dict(r) for r in rows]

    def waiting_stems(self, older_than: float) -> list[str]:
        with self.lock:
            rows = self.db.execute(
                "select stem from parts group by stem having max(received) < ?", (older_than,)
            ).fetchall()
        return [r[0] for r in rows]

    def drop_parts(self, stem: str) -> None:
        with self.lock:
            self.db.execute("delete from parts where stem = ?", (stem,))

    def meta(self, key: str) -> str | None:
        with self.lock:
            row = self.db.execute("select value from meta where key = ?", (key,)).fetchone()
        return row[0] if row else None

    def set_meta(self, key: str, value: str) -> None:
        with self.lock:
            self.db.execute(
                "insert into meta values (?, ?) on conflict(key) do update set value = excluded.value",
                (key, value),
            )
