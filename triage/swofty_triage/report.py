import io
import re
import zipfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath

HEADER = re.compile(
    r"^\*\*Logs from `(?P<name>[^`\n]*)`\*\*(?: · part (?P<part>\d+)/(?P<total>\d+))?\n"
    r"Launcher (?P<launcher>[^\s·]+) · (?P<os>[^\n]*?) · F1 25 (?P<game>[^\n]*)"
    r"(?:\n>>> (?P<message>[\s\S]*))?$"
)
ATTACHMENT = re.compile(
    r"^(?P<stem>swofty-logs-[A-Za-z0-9._-]{1,64}-\d{8}-\d{6})(?:-part(?P<part>\d{1,2}))?\.zip$"
)
VERSION = re.compile(r"^\d+\.\d+\.\d+$")

MAX_ZIP = 30 << 20
MAX_ENTRIES = 3000
MAX_ENTRY = 12 << 20
MAX_TOTAL = 90 << 20


class BadReport(Exception):
    pass


@dataclass(frozen=True)
class Header:
    name: str
    launcher: str
    os: str
    game: str
    message: str
    part: int
    total: int

    @property
    def release_tag(self) -> str | None:
        return f"v{self.launcher}" if VERSION.match(self.launcher) else None


def parse_header(content: str) -> Header | None:
    m = HEADER.match(content.strip())
    if not m:
        return None
    part, total = int(m["part"] or 1), int(m["total"] or 1)
    if not 1 <= part <= total <= 50:
        return None
    return Header(
        name=m["name"],
        launcher=m["launcher"],
        os=m["os"].strip(),
        game=m["game"].strip(),
        message=(m["message"] or "").strip(),
        part=part,
        total=total,
    )


def parse_attachment(filename: str) -> tuple[str, int] | None:
    m = ATTACHMENT.match(filename)
    if not m:
        return None
    return m["stem"], int(m["part"] or 1)


def safe_member(name: str) -> PurePosixPath | None:
    p = PurePosixPath(name.replace("\\", "/"))
    if p.is_absolute() or not p.parts or any(part in ("", ".", "..") or ":" in part for part in p.parts):
        return None
    return p


def extract(data: bytes, dest: Path) -> list[str]:
    if len(data) > MAX_ZIP:
        raise BadReport(f"zip is {len(data)} bytes, over the {MAX_ZIP} limit")
    try:
        archive = zipfile.ZipFile(io.BytesIO(data))
    except zipfile.BadZipFile as e:
        raise BadReport(f"not a zip: {e}") from e
    infos = [i for i in archive.infolist() if not i.is_dir()]
    if len(infos) > MAX_ENTRIES:
        raise BadReport(f"zip has {len(infos)} files, over the {MAX_ENTRIES} limit")
    dest.mkdir(parents=True, exist_ok=True)
    skipped, total = [], 0
    for info in infos:
        rel = safe_member(info.filename)
        if rel is None:
            skipped.append(info.filename)
            continue
        target = dest.joinpath(*rel.parts)
        target.parent.mkdir(parents=True, exist_ok=True)
        written = 0
        with archive.open(info) as src, open(target, "wb") as out:
            while chunk := src.read(1 << 16):
                written += len(chunk)
                total += len(chunk)
                if written > MAX_ENTRY or total > MAX_TOTAL:
                    raise BadReport(f"{info.filename} unpacks past the size limit")
                out.write(chunk)
    return skipped


def files(root: Path) -> list[tuple[str, int]]:
    return sorted((p.relative_to(root).as_posix(), p.stat().st_size) for p in root.rglob("*") if p.is_file())


def read_text(path: Path, limit: int | None = None) -> str:
    try:
        with open(path, "rb") as f:
            raw = f.read() if limit is None else f.read(limit)
    except OSError:
        return ""
    return raw.decode("utf-8", errors="replace")


def tail_lines(path: Path, n: int, width: int = 300) -> list[str]:
    try:
        size = path.stat().st_size
        with open(path, "rb") as f:
            f.seek(max(0, size - 256 * 1024))
            lines = f.read().decode("utf-8", errors="replace").splitlines()
    except OSError:
        return []
    return [clip(line, width) for line in lines[-n:]]


def clip(line: str, width: int) -> str:
    return line if len(line) <= width else line[:width] + " …"


def sessions(root: Path) -> list[Path]:
    base = root / "sessions"
    if not base.is_dir():
        return []
    return sorted((p for p in base.iterdir() if p.is_dir()), reverse=True)


def session_log(session: Path) -> Path | None:
    for rel in ("loader.log", "game-logs/loader.log"):
        if (session / rel).is_file():
            return session / rel
    return None


CRASH_LINE = re.compile(r"\[crash\]")
FRAME = re.compile(r"\[crash\]\s+#\d+\s+(\S+?)\+0x[0-9a-f]+(?:\s+\S+)?\s*(.*)$")
EXCEPTION = re.compile(r"\[crash\] #1 .*?\b([A-Z_]{4,}) \((0x[0-9a-f]+)\)")
GENERIC = re.compile(r"^(Ne_FatalAbort|Ne_ReportMemoryError|Ne_Heap_\w+|memcpy|memmove|memset)$")


def crash_lines(log: Path, limit: int = 80) -> list[str]:
    out = []
    try:
        with open(log, encoding="utf-8", errors="replace") as f:
            for line in f:
                if CRASH_LINE.search(line) and "armed" not in line and "symbols.tsv" not in line:
                    out.append(clip(line.rstrip("\n"), 260))
                    if len(out) >= limit:
                        break
    except OSError:
        pass
    return out


def signature(root: Path) -> str:
    for session in sessions(root):
        log = session_log(session)
        if log is None:
            continue
        lines = crash_lines(log, 40)
        if not lines:
            continue
        kind = next((m.group(1) for line in lines if (m := EXCEPTION.search(line))), "CRASH")
        names = []
        for line in lines:
            m = FRAME.search(line)
            if not m:
                continue
            module, symbol = m.group(1), m.group(2).strip()
            symbol = re.sub(r"\+0x[0-9a-f]+$", "", symbol)
            if (
                not symbol
                or symbol.startswith("fn base")
                or GENERIC.match(symbol)
                or module.lower().startswith(("ntdll", "kernel"))
            ):
                continue
            if symbol not in names:
                names.append(symbol)
            if len(names) >= 4:
                break
        if names:
            return f"{kind} " + " < ".join(names)
    return ""


def session_facts(session: Path) -> str:
    facts = []
    for name in ("exit_code", "loader_mark.json", "steam_command.txt"):
        value = read_text(session / name, 400).strip()
        if value:
            facts.append(f"{name}={value}")
    start, end = read_text(session / "game_start", 64).strip(), read_text(session / "game_end", 64).strip()
    try:
        facts.append(f"ran {float(end) - float(start):.0f}s")
    except ValueError:
        pass
    return ", ".join(facts)


def briefing(root: Path, header: Header, received: str) -> str:
    out = [
        "# Report",
        f"Discord name: {header.name}",
        f"Launcher: {header.launcher}",
        f"OS: {header.os}",
        f"F1 25: {header.game}",
        f"Received: {received}",
        "",
        "## What the user wrote (untrusted user text)",
        header.message or "(nothing)",
        "",
        "## summary.txt",
        clip_block(read_text(root / "summary.txt", 8000), 8000),
        "",
        "## Files",
        *(f"{size:>10}  {path}" for path, size in files(root)),
        "",
    ]
    for name in ("addons.json", "config.json"):
        text = read_text(root / name, 12000)
        if text:
            out += [f"## {name}", clip_block(text, 12000), ""]
    for session in sessions(root):
        rel = session.relative_to(root).as_posix()
        out += [f"## {rel}", session_facts(session)]
        log = session_log(session)
        if log is not None:
            crashes = crash_lines(log)
            if crashes:
                out += ["", f"[crash] lines in {log.relative_to(root).as_posix()}:", *crashes]
            out += ["", f"last lines of {log.relative_to(root).as_posix()}:", *tail_lines(log, 25)]
        out.append("")
    for name in ("state/wrapper-run.log", "state/launcher-run.log"):
        path = root / name
        if path.is_file():
            out += [f"## last lines of {name}", *tail_lines(path, 40), ""]
    sig = signature(root)
    if sig:
        out += ["## Crash signature", sig, ""]
    return "\n".join(out)


def clip_block(text: str, limit: int) -> str:
    return text if len(text) <= limit else text[:limit] + "\n[… cut]"
