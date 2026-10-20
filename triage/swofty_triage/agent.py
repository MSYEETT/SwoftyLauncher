import json
import logging
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import anthropic

from .config import Config
from .report import Header
from .source import Sources
from .tools import TOOLS, Toolbox

log = logging.getLogger("swofty_triage.agent")

SYSTEM = Path(__file__).with_name("prompt.md").read_text(encoding="utf-8")
FALLBACK_BETA = "server-side-fallback-2026-07-01"
MAX_TOKENS = 32000
MAX_NUDGES = 2

PRICES = {
    "claude-opus-5-5": (4.0, 20.0),
    "claude-sonnet-5-5": (2.0, 10.0),
    "claude-fable-5-1": (10.0, 50.0),
    "claude-haiku-4-5": (1.0, 5.0),
}


class Declined(Exception):
    pass


@dataclass
class Run:
    diagnosis: dict[str, Any] | None = None
    turns: int = 0
    cost_usd: float = 0.0
    tool_calls: list[str] = field(default_factory=list)


def cost_of(model: str, usage: Any) -> float:
    input_price, output_price = PRICES.get(model, (5.0, 25.0))
    fresh = getattr(usage, "input_tokens", 0) or 0
    written = getattr(usage, "cache_creation_input_tokens", 0) or 0
    read = getattr(usage, "cache_read_input_tokens", 0) or 0
    output = getattr(usage, "output_tokens", 0) or 0
    return (
        fresh * input_price + written * input_price * 1.25 + read * input_price * 0.1 + output * output_price
    ) / 1e6


def git_context(sources: Sources, header: Header) -> str:
    lines = [
        "# Source",
        "Repositories you can read: " + ", ".join(n for n in sources.names() if sources.available(n)),
    ]
    tag = header.release_tag
    if tag and sources.resolve("launcher", tag):
        lines.append(f"launcher at the user's release: {sources.describe('launcher', tag)}")
        lines.append(f"launcher latest main: {sources.describe('launcher', 'HEAD')}")
        lines += [
            "",
            f"Commits on launcher main since {tag}:",
            sources.log("launcher", tag, "HEAD", max_count=80),
        ]
    else:
        lines.append(f"Launcher {header.launcher} has no release tag; read launcher at HEAD.")
        lines.append(f"launcher latest main: {sources.describe('launcher', 'HEAD')}")
    return "\n".join(lines)


def opening(case_id: str, brief: str, context: str) -> str:
    return (
        f"Report id: {case_id}\n\n{brief}\n\n{context}\n\n"
        "Investigate this report with the tools, then call submit_diagnosis."
    )


class Agent:
    def __init__(self, cfg: Config, sources: Sources, client: anthropic.Anthropic | None = None):
        self.cfg = cfg
        self.sources = sources
        self.client = client or anthropic.Anthropic(max_retries=4)

    def request(self, messages: list[dict[str, Any]]):
        with self.client.beta.messages.stream(
            model=self.cfg.model,
            max_tokens=MAX_TOKENS,
            system=[{"type": "text", "text": SYSTEM}],
            tools=TOOLS,
            messages=messages,
            thinking={"type": "adaptive"},
            output_config={"effort": self.cfg.effort},
            cache_control={"type": "ephemeral"},
            betas=[FALLBACK_BETA],
            extra_body={"fallbacks": "default"},
        ) as stream:
            return stream.get_final_message()

    def run(self, case_id: str, logs: Path, header: Header, brief: str, recent) -> Run:
        toolbox = Toolbox(logs, self.sources, recent)
        messages: list[dict[str, Any]] = [
            {"role": "user", "content": opening(case_id, brief, git_context(self.sources, header))}
        ]
        run = Run()
        nudges = 0
        warned = False
        while run.turns < self.cfg.max_turns:
            message = self.request(messages)
            run.turns += 1
            run.cost_usd += cost_of(self.cfg.model, message.usage)
            messages.append({"role": "assistant", "content": message.content})
            if message.stop_reason == "refusal":
                details = getattr(message, "stop_details", None)
                raise Declined(getattr(details, "explanation", None) or "the model declined this report")
            calls = [b for b in message.content if b.type == "tool_use"]
            if not calls:
                if nudges >= MAX_NUDGES:
                    break
                nudges += 1
                messages.append(
                    {"role": "user", "content": "Call submit_diagnosis now with what you have found."}
                )
                continue
            results: list[dict[str, Any]] = []
            for call in calls:
                run.tool_calls.append(call.name)
                if message.stop_reason == "max_tokens":
                    results.append(error_result(call.id, "INVALID_JSON: the input was cut off; call again"))
                    continue
                outcome = toolbox.run(call.name, call.input)
                log.info("%s %s %s", case_id, call.name, summarize(call.input))
                if outcome.diagnosis is not None:
                    run.diagnosis = outcome.diagnosis
                results.append(
                    {
                        "type": "tool_result",
                        "tool_use_id": call.id,
                        "content": outcome.text,
                        **({"is_error": True} if outcome.is_error else {}),
                    }
                )
            if run.diagnosis is not None:
                return run
            if not warned and run.cost_usd > self.cfg.max_cost_usd * 0.75:
                warned = True
                results.append(
                    {"type": "text", "text": "The budget for this report is nearly spent. Submit next."}
                )
            elif run.cost_usd > self.cfg.max_cost_usd:
                results.append({"type": "text", "text": "Budget spent. Call submit_diagnosis now."})
            messages.append({"role": "user", "content": results})
            if run.cost_usd > self.cfg.max_cost_usd * 1.5:
                break
        return run


def error_result(tool_use_id: str, text: str) -> dict[str, Any]:
    return {"type": "tool_result", "tool_use_id": tool_use_id, "content": text, "is_error": True}


def summarize(args: Any) -> str:
    try:
        text = json.dumps(args, ensure_ascii=False)
    except (TypeError, ValueError):
        text = str(args)
    return text if len(text) <= 160 else text[:160] + "…"
