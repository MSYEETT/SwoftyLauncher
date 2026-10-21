from typing import Any

from .report import Header

LIMIT = 1900
LOGS_DOCS = "https://f1.swofty.net/using/send-logs"
FIXED = {
    "yes": "fixed on main",
    "partly": "partly fixed on main",
    "no": "not fixed on main",
    "unknown": "fix status unknown",
}


def cut(text: str, limit: int) -> str:
    text = text.strip()
    return text if len(text) <= limit else text[: limit - 1].rstrip() + "…"


def logs_request(mention: str | None) -> str:
    greeting = f"Hey {mention}!" if mention else "Hey!"
    return (
        f"{greeting} Please press **Send Logs to Developers** at the bottom of the launcher's sidebar, "
        "enter your Discord username and what went wrong, then press **Send**. That sends us your logs "
        "so we can fix your issue as soon as possible. Don't see the button? Update the launcher first. "
        f"<{LOGS_DOCS}>"
    )


def thread_name(diagnosis: dict[str, Any] | None, header: Header) -> str:
    title = diagnosis["title"] if diagnosis else "Triage"
    return cut(f"{header.name}: {title}", 100)


def status_line(d: dict[str, Any], cost_usd: float, turns: int) -> str:
    parts = [d["category"], f"{d['confidence']} confidence", FIXED[d["fixed_on_main"]]]
    if d["duplicate_of"]:
        parts.append(f"same as {d['duplicate_of']}")
    parts.append(f"${cost_usd:.2f} · {turns} turns")
    return " · ".join(parts)


def split(text: str, limit: int = LIMIT) -> list[str]:
    out: list[str] = []
    lines: list[str] = []
    size = 0
    fence: str | None = None
    for line in text.strip().split("\n"):
        while len(line) > limit:
            head, line = line[:limit], line[limit:]
            out += flush(lines, fence) + [head]
            lines, size = ([fence] if fence else []), len(fence or "")
        if size + len(line) + 1 > limit and lines:
            out += flush(lines, fence)
            lines, size = ([fence] if fence else []), len(fence or "")
        lines.append(line)
        size += len(line) + 1
        if line.lstrip().startswith("```"):
            fence = None if fence else line.strip()
    out += flush(lines, fence)
    return [
        c for c in out if any(line.strip() and not line.strip().startswith("```") for line in c.split("\n"))
    ]


def flush(lines: list[str], fence: str | None) -> list[str]:
    if not lines:
        return []
    return ["\n".join(lines + (["```"] if fence else []))]


def messages(d: dict[str, Any], cost_usd: float, turns: int, report_url: str) -> list[str]:
    out = split(
        f"## {d['title']}\n-# {status_line(d, cost_usd, turns)} · [report]({report_url})\n\n"
        f"{d['summary']}\n\n**Cause**\n{d['root_cause']}"
    )
    tail = []
    if d["evidence"]:
        tail.append("**Evidence**\n" + "\n".join(f"- {e}" for e in d["evidence"]))
    if d["fixed_by"]:
        tail.append(f"**Fixed by**\n{d['fixed_by']}")
    if tail:
        out += split("\n\n".join(tail))
    if d["user_reply"]:
        out += [f"**Reply to the player**\n>>> {chunk}" for chunk in split(d["user_reply"], LIMIT - 40)]
    if d["fix_prompt"]:
        repo = f" for `{d['fix_repo']}`" if d["fix_repo"] else ""
        out.append(f"**Fix prompt**{repo}, copy it into a coding agent:")
        out += split(d["fix_prompt"])
    return out


def diagnosis_markdown(d: dict[str, Any], header: Header, report_id: str, cost_usd: float, turns: int) -> str:
    lines = [
        f"# {d['title']}",
        "",
        f"Report {report_id} from `{header.name}`, launcher {header.launcher}, {header.os}, F1 25 {header.game}.",
        "",
        status_line(d, cost_usd, turns),
        "",
        "## Summary",
        d["summary"],
        "",
        "## Cause",
        d["root_cause"],
        "",
        "## Evidence",
        *(f"- {e}" for e in d["evidence"]),
        "",
    ]
    if d["fixed_by"]:
        lines += ["## Fixed by", d["fixed_by"], ""]
    lines += ["## Reply to the player", d["user_reply"], ""]
    if d["fix_prompt"]:
        lines += [f"## Fix prompt ({d['fix_repo'] or 'repository not named'})", "", d["fix_prompt"], ""]
    return "\n".join(lines)
