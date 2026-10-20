import json
import logging
import shutil
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from . import render, report
from .agent import Agent, Declined
from .config import Config
from .report import Header
from .source import Sources
from .store import Store

log = logging.getLogger("swofty_triage.pipeline")


class Limited(Exception):
    pass


@dataclass
class Result:
    report_id: str
    header: Header
    diagnosis: dict[str, Any]
    cost_usd: float
    turns: int
    signature: str
    markdown: str


class Triage:
    def __init__(self, cfg: Config, agent: Agent | None = None):
        self.cfg = cfg
        self.store = Store(cfg.db_path)
        self.sources = Sources(cfg.repos, cfg.repos_dir)
        self.agent = agent or Agent(cfg, self.sources)

    def check_limits(self, header: Header, received: float) -> None:
        if self.store.count_since(received - 86400) >= self.cfg.daily_limit:
            raise Limited(f"daily limit of {self.cfg.daily_limit} reports reached")
        if self.store.count_since(received - 3600, header.name) >= self.cfg.per_name_hourly:
            raise Limited(f"{header.name} sent more than {self.cfg.per_name_hourly} reports in an hour")

    def unpack(self, report_id: str, zips: list[tuple[str, bytes]]) -> Path:
        case = self.cfg.cases_dir / report_id
        logs = case / "logs"
        if logs.exists():
            shutil.rmtree(logs)
        (case / "zips").mkdir(parents=True, exist_ok=True)
        skipped = []
        for name, data in zips:
            (case / "zips" / name).write_bytes(data)
            skipped += report.extract(data, logs)
        if skipped:
            log.warning("%s: left out unsafe zip entries %s", report_id, skipped[:5])
        return logs

    def run(self, report_id: str, header: Header, zips: list[tuple[str, bytes]], received: float) -> Result:
        stem = report.parse_attachment(zips[0][0])
        self.store.begin(
            report_id, stem[0] if stem else zips[0][0], header.name, header.launcher, header.message, received
        )
        signature = ""
        try:
            problems = self.sources.update()
            for problem in problems:
                log.warning("source update: %s", problem)
            if not self.sources.available("launcher"):
                raise RuntimeError("the launcher repository is not available")
            logs = self.unpack(report_id, zips)
            signature = report.signature(logs)
            stamp = time.strftime("%Y-%m-%d %H:%M UTC", time.gmtime(received))
            brief = report.briefing(logs, header, stamp)
            run = self.agent.run(
                report_id, logs, header, brief, lambda n: self.store.recent(n, exclude=report_id)
            )
        except Declined as e:
            self.store.finish(report_id, "failed", signature=signature, error=f"declined: {e}")
            raise
        except Exception as e:
            self.store.finish(report_id, "failed", signature=signature, error=str(e)[:2000])
            raise
        if run.diagnosis is None:
            message = f"no diagnosis after {run.turns} turns (${run.cost_usd:.2f})"
            self.store.finish(
                report_id,
                "failed",
                signature=signature,
                error=message,
                cost_usd=run.cost_usd,
                turns=run.turns,
            )
            raise RuntimeError(message)
        d = run.diagnosis
        markdown = render.diagnosis_markdown(d, header, report_id, run.cost_usd, run.turns)
        case = self.cfg.cases_dir / report_id
        (case / "diagnosis.json").write_text(json.dumps(d, indent=2, ensure_ascii=False), encoding="utf-8")
        (case / "diagnosis.md").write_text(markdown, encoding="utf-8")
        self.store.finish(
            report_id, "done", signature=signature, diagnosis=d, cost_usd=run.cost_usd, turns=run.turns
        )
        log.info(
            "%s: %s (%s, $%.2f, %d turns)", report_id, d["title"], d["category"], run.cost_usd, run.turns
        )
        return Result(report_id, header, d, run.cost_usd, run.turns, signature, markdown)
