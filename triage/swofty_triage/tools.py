import json
import re
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from . import source
from .source import SourceError, Sources

CATEGORIES = [
    "crash",
    "hang",
    "launch-failure",
    "install-or-update",
    "mod-or-track-package",
    "addon-bug",
    "launcher-bug",
    "game-bug",
    "user-setup",
    "feature-request",
    "not-enough-info",
]
CONFIDENCE = ["high", "medium", "low"]
FIXED = ["yes", "partly", "no", "unknown"]


def tool(name: str, description: str, properties: dict[str, Any], required: list[str]) -> dict[str, Any]:
    return {
        "name": name,
        "description": description,
        "eager_input_streaming": True,
        "input_schema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": False,
        },
    }


REPO = {"type": "string", "description": "Repository name from the briefing, such as launcher"}
REF = {
    "type": "string",
    "description": "A tag, branch or commit: the user's release tag (for example v1.1.0) or HEAD for the latest main",
}
PATH = {"type": "string", "description": "Path relative to the root, empty for the root"}
OFFSET = {"type": "integer", "description": "First line to show, 1-based"}
LIMIT = {"type": "integer", "description": "Number of lines to show, at most 2000"}
PATTERN = {"type": "string", "description": "Extended regular expression"}
IGNORE_CASE = {"type": "boolean"}
MAX_RESULTS = {"type": "integer", "description": "Most matching lines to return, at most 300"}

SUBMIT = tool(
    "submit_diagnosis",
    "Record the final diagnosis of this report. Call it exactly once, when the investigation is done.",
    {
        "title": {"type": "string", "description": "Under 90 characters, names the problem"},
        "category": {"type": "string", "enum": CATEGORIES},
        "confidence": {"type": "string", "enum": CONFIDENCE},
        "summary": {"type": "string", "description": "Two to four sentences for the developers"},
        "root_cause": {"type": "string", "description": "Markdown: what goes wrong and why, citing code"},
        "evidence": {
            "type": "array",
            "items": {"type": "string"},
            "description": "Each item cites a log file:line or repo path:line and what it shows",
        },
        "fixed_on_main": {"type": "string", "enum": FIXED},
        "fixed_by": {
            "type": "string",
            "description": "Commit hashes and subjects on main that fix it, or empty",
        },
        "duplicate_of": {
            "type": "string",
            "description": "Report id of an earlier report with the same cause, or empty",
        },
        "user_reply": {
            "type": "string",
            "description": "Plain-language reply a developer can paste to the user: workaround or next step",
        },
        "fix_repo": {"type": "string", "description": "Repository the fix belongs in, or empty"},
        "fix_prompt": {
            "type": "string",
            "description": "Self-contained markdown prompt for a coding agent working in fix_repo, or empty when no code change is needed",
        },
    },
    [
        "title",
        "category",
        "confidence",
        "summary",
        "root_cause",
        "evidence",
        "fixed_on_main",
        "fixed_by",
        "duplicate_of",
        "user_reply",
        "fix_repo",
        "fix_prompt",
    ],
)

TOOLS = [
    tool(
        "list_logs",
        "List the files in the user's log bundle with their sizes.",
        {"path": PATH},
        [],
    ),
    tool(
        "read_log",
        "Read a file from the user's log bundle, numbered by line.",
        {"path": PATH, "offset": OFFSET, "limit": LIMIT},
        ["path"],
    ),
    tool(
        "grep_logs",
        "Search the user's log bundle. Returns file:line:text for each match.",
        {
            "pattern": PATTERN,
            "path": {"type": "string", "description": "File or folder to search, empty for all"},
            "ignore_case": IGNORE_CASE,
            "context": {"type": "integer", "description": "Lines of context around each match, at most 5"},
            "max_results": MAX_RESULTS,
        },
        ["pattern"],
    ),
    tool(
        "list_source",
        "List a folder of a source repository at a ref.",
        {"repo": REPO, "ref": REF, "path": PATH},
        ["repo", "ref"],
    ),
    tool(
        "read_source",
        "Read a file of a source repository at a ref, numbered by line.",
        {"repo": REPO, "ref": REF, "path": PATH, "offset": OFFSET, "limit": LIMIT},
        ["repo", "ref", "path"],
    ),
    tool(
        "grep_source",
        "Search a source repository at a ref (git grep). Returns path:line:text for each match.",
        {
            "repo": REPO,
            "ref": REF,
            "pattern": PATTERN,
            "path": {"type": "string", "description": "Folder or file to limit the search to, empty for all"},
            "ignore_case": IGNORE_CASE,
            "max_results": MAX_RESULTS,
        },
        ["repo", "ref", "pattern"],
    ),
    tool(
        "git_log",
        "List the commits in base..head, newest first, optionally only those touching a path.",
        {
            "repo": REPO,
            "base": {"type": "string", "description": "Older ref, usually the user's release tag"},
            "head": {"type": "string", "description": "Newer ref, usually HEAD"},
            "path": PATH,
            "max_count": {"type": "integer"},
        },
        ["repo", "base", "head"],
    ),
    tool(
        "show_commit",
        "Show one commit's message, file stats and diff, optionally limited to a path.",
        {"repo": REPO, "commit": {"type": "string"}, "path": PATH},
        ["repo", "commit"],
    ),
    tool(
        "recent_reports",
        "List earlier reports and their diagnoses, newest first, to spot duplicates and patterns.",
        {"limit": {"type": "integer", "description": "At most 60"}},
        [],
    ),
    SUBMIT,
]


@dataclass
class Outcome:
    text: str
    is_error: bool = False
    diagnosis: dict[str, Any] | None = None


class ToolError(Exception):
    pass


def text_arg(args: dict[str, Any], key: str, default: str | None = None, limit: int = 400) -> str:
    value = args.get(key, default)
    if value is None:
        raise ToolError(f"missing {key}")
    if not isinstance(value, str):
        raise ToolError(f"{key} must be a string")
    if len(value) > limit:
        raise ToolError(f"{key} is longer than {limit} characters")
    return value


def int_arg(args: dict[str, Any], key: str, default: int, low: int, high: int) -> int:
    value = args.get(key, default)
    if isinstance(value, bool) or not isinstance(value, int):
        raise ToolError(f"{key} must be an integer")
    return max(low, min(value, high))


def bool_arg(args: dict[str, Any], key: str) -> bool:
    value = args.get(key, False)
    if not isinstance(value, bool):
        raise ToolError(f"{key} must be true or false")
    return value


class Toolbox:
    def __init__(self, logs: Path, sources: Sources, recent):
        self.logs = logs.resolve()
        self.sources = sources
        self.recent = recent

    def log_path(self, rel: str) -> Path:
        rel = rel.strip().lstrip("/")
        target = (self.logs / rel).resolve()
        if target != self.logs and not target.is_relative_to(self.logs):
            raise ToolError(f"{rel} is outside the log bundle")
        if not target.exists():
            raise ToolError(f"{rel} is not in the log bundle; use list_logs")
        return target

    def run(self, name: str, args: Any) -> Outcome:
        if not isinstance(args, dict):
            return Outcome("tool input must be a JSON object", True)
        try:
            handler = getattr(self, f"do_{name}", None)
            if handler is None:
                return Outcome(f"unknown tool {name}", True)
            return handler(args)
        except (ToolError, SourceError) as e:
            return Outcome(str(e), True)
        except subprocess.TimeoutExpired:
            return Outcome("the command timed out; narrow the request", True)

    def do_list_logs(self, args: dict[str, Any]) -> Outcome:
        root = self.log_path(text_arg(args, "path", ""))
        if root.is_file():
            return Outcome(f"{root.relative_to(self.logs).as_posix()}  ({root.stat().st_size} bytes)")
        rows = [
            f"{p.relative_to(self.logs).as_posix()}  ({p.stat().st_size} bytes)"
            for p in sorted(root.rglob("*"))
            if p.is_file()
        ]
        return Outcome("\n".join(rows) or "empty")

    def do_read_log(self, args: dict[str, Any]) -> Outcome:
        path = self.log_path(text_arg(args, "path"))
        if path.is_dir():
            return self.do_list_logs(args)
        offset = int_arg(args, "offset", 1, 1, 10_000_000)
        limit = int_arg(args, "limit", 400, 1, 2000)
        text = path.read_bytes().decode("utf-8", errors="replace")
        return Outcome(source.number(text, offset, limit, path.relative_to(self.logs).as_posix()))

    def do_grep_logs(self, args: dict[str, Any]) -> Outcome:
        pattern = text_arg(args, "pattern", limit=300)
        if not pattern:
            raise ToolError("pattern is empty")
        path = self.log_path(text_arg(args, "path", ""))
        context = int_arg(args, "context", 0, 0, 5)
        max_results = int_arg(args, "max_results", 80, 1, 300)
        ignore_case = bool_arg(args, "ignore_case")
        if shutil.which("rg"):
            out = ripgrep(pattern, path, self.logs, context, ignore_case)
        else:
            out = pygrep(pattern, path, self.logs, context, ignore_case)
        lines = out.splitlines()
        if not lines:
            return Outcome(f"no matches for /{pattern}/")
        shown = [line if len(line) <= 300 else line[:300] + " …" for line in lines[:max_results]]
        more = len(lines) - len(shown)
        return Outcome("\n".join(shown) + (f"\n[{more} more lines; narrow the search]" if more else ""))

    def repo(self, args: dict[str, Any]) -> str:
        name = text_arg(args, "repo", "launcher", 40)
        if not self.sources.available(name):
            raise ToolError(f"repository {name!r} is not available; known: {', '.join(self.sources.names())}")
        return name

    def do_list_source(self, args: dict[str, Any]) -> Outcome:
        return Outcome(self.sources.ls(self.repo(args), text_arg(args, "ref"), text_arg(args, "path", "")))

    def do_read_source(self, args: dict[str, Any]) -> Outcome:
        return Outcome(
            self.sources.read(
                self.repo(args),
                text_arg(args, "ref"),
                text_arg(args, "path"),
                int_arg(args, "offset", 1, 1, 10_000_000),
                int_arg(args, "limit", 400, 1, 2000),
            )
        )

    def do_grep_source(self, args: dict[str, Any]) -> Outcome:
        return Outcome(
            self.sources.grep(
                self.repo(args),
                text_arg(args, "ref"),
                text_arg(args, "pattern", limit=300),
                text_arg(args, "path", ""),
                bool_arg(args, "ignore_case"),
                int_arg(args, "max_results", 80, 1, 300),
            )
        )

    def do_git_log(self, args: dict[str, Any]) -> Outcome:
        return Outcome(
            self.sources.log(
                self.repo(args),
                text_arg(args, "base"),
                text_arg(args, "head", "HEAD"),
                text_arg(args, "path", ""),
                int_arg(args, "max_count", 60, 1, 200),
            )
        )

    def do_show_commit(self, args: dict[str, Any]) -> Outcome:
        return Outcome(
            self.sources.show_commit(self.repo(args), text_arg(args, "commit"), text_arg(args, "path", ""))
        )

    def do_recent_reports(self, args: dict[str, Any]) -> Outcome:
        rows = self.recent(int_arg(args, "limit", 30, 1, 60))
        return Outcome(json.dumps(rows, indent=1, ensure_ascii=False) if rows else "no earlier reports")

    def do_submit_diagnosis(self, args: dict[str, Any]) -> Outcome:
        diagnosis = validate_diagnosis(args, self.sources.names())
        return Outcome("recorded", diagnosis=diagnosis)


def validate_diagnosis(args: dict[str, Any], repos: list[str]) -> dict[str, Any]:
    schema = SUBMIT["input_schema"]
    missing = [k for k in schema["required"] if k not in args]
    if missing:
        raise ToolError(f"missing {', '.join(missing)}")
    extra = [k for k in args if k not in schema["properties"]]
    if extra:
        raise ToolError(f"unknown field(s) {', '.join(extra)}")
    out: dict[str, Any] = {}
    for key, spec in schema["properties"].items():
        value = args[key]
        if spec["type"] == "array":
            if not isinstance(value, list) or not all(isinstance(v, str) for v in value):
                raise ToolError(f"{key} must be a list of strings")
            out[key] = [v.strip() for v in value if v.strip()][:25]
            continue
        if not isinstance(value, str):
            raise ToolError(f"{key} must be a string")
        if "enum" in spec and value not in spec["enum"]:
            raise ToolError(f"{key} must be one of {', '.join(spec['enum'])}")
        out[key] = value.strip()
    if not out["title"]:
        raise ToolError("title is empty")
    out["title"] = out["title"][:120]
    if (
        out["fix_repo"]
        and out["fix_repo"] not in repos
        and not re.match(r"^[\w.-]+/[\w.-]+$", out["fix_repo"])
    ):
        raise ToolError("fix_repo must be a repository name from the briefing, owner/name, or empty")
    return out


def ripgrep(pattern: str, path: Path, root: Path, context: int, ignore_case: bool) -> str:
    args = ["rg", "--no-heading", "--line-number", "--with-filename", "--color=never", "--no-config"]
    args += ["--max-count=200", "--max-columns=400", "--text"]
    if context:
        args += ["-C", str(context)]
    if ignore_case:
        args.append("-i")
    args += ["-e", pattern, "--", str(path)]
    result = subprocess.run(args, capture_output=True, timeout=30, cwd=root)
    if result.returncode == 2:
        raise ToolError(result.stderr.decode("utf-8", errors="replace").strip()[-500:] or "bad pattern")
    prefix = str(root) + "/"
    return "\n".join(
        line.removeprefix(prefix) for line in result.stdout.decode("utf-8", errors="replace").splitlines()
    )


def pygrep(pattern: str, path: Path, root: Path, context: int, ignore_case: bool) -> str:
    try:
        rx = re.compile(pattern, re.IGNORECASE if ignore_case else 0)
    except re.error as e:
        raise ToolError(f"bad pattern: {e}") from e
    targets = [path] if path.is_file() else sorted(p for p in path.rglob("*") if p.is_file())
    out = []
    for target in targets:
        rel = target.relative_to(root).as_posix()
        lines = target.read_bytes().decode("utf-8", errors="replace").splitlines()
        hits = 0
        for i, line in enumerate(lines):
            if rx.search(line):
                lo, hi = max(0, i - context), min(len(lines), i + context + 1)
                for j in range(lo, hi):
                    sep = ":" if j == i else "-"
                    out.append(f"{rel}{sep}{j + 1}{sep}{lines[j]}")
                hits += 1
                if hits >= 200:
                    break
    return "\n".join(out)
