import json
import logging
import re
import ssl
import urllib.error
import urllib.request
from collections import Counter
from dataclasses import dataclass
from datetime import datetime
from typing import Any

import anthropic
import certifi

from .source import Sources

log = logging.getLogger("swofty_triage.releases")

API = "https://api.github.com"
PR_SUFFIX = re.compile(r"\s*\(#(\d+)\)\s*$")
MERGE = re.compile(r"^Merge pull request #(\d+)")
TYPE_PREFIX = re.compile(r"^[a-z]+(\([^)]*\))?!?:\s*")
VERSION_SUFFIX = re.compile(r"\s*\(\d+\.\d+\.\d+\)\s*$")
TAG = re.compile(r"^v?(\d+)\.(\d+)\.(\d+)$")
FALLBACK_BETA = "server-side-fallback-2026-07-01"
TLS = ssl.create_default_context(cafile=certifi.where())

LINES_SCHEMA = {
    "type": "object",
    "properties": {
        "lines": {
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "prs": {"type": "array", "items": {"type": "integer"}},
                },
                "required": ["text", "prs"],
                "additionalProperties": False,
            },
        }
    },
    "required": ["lines"],
    "additionalProperties": False,
}

WRITER = """You write the changelog of a release announcement for Swofty Launcher for F1 25, a launcher and mod loader for the game F1 25. It is posted to the project's Discord for players.

Write one line per user-facing change. Each line is a single sentence in the past tense that starts with a verb such as Added, Fixed, Made, Improved, Removed or Updated, in plain words a player understands, with no trailing full stop, no markdown links and no code formatting. Name addons and features the way the release notes do. Fold minor related details into one line rather than listing each. Leave out CI, release preparation, documentation-only and internal changes, and the notes aimed only at addon authors unless they are the main change.

For each line, list the pull request numbers it comes from, taken only from the commit list. A change from the release notes that no pull request in the list covers gets an empty list."""


@dataclass(frozen=True)
class Asset:
    name: str
    url: str
    size: int


@dataclass(frozen=True)
class Release:
    tag: str
    name: str
    url: str
    published_at: str
    assets: tuple[Asset, ...]

    @property
    def version(self) -> str:
        return self.tag.removeprefix("v")

    @property
    def published(self) -> datetime:
        return datetime.fromisoformat(self.published_at.replace("Z", "+00:00"))


@dataclass(frozen=True)
class Commit:
    sha: str
    subject: str
    login: str | None
    name: str

    @property
    def pr(self) -> int | None:
        m = PR_SUFFIX.search(self.subject) or MERGE.match(self.subject)
        return int(m.group(1)) if m else None


class GitHub:
    def __init__(self, repo: str, token: str | None = None):
        self.repo = repo
        self.token = token
        self.etag: str | None = None
        self.cached: list[Release] = []

    def request(self, path: str, etag: str | None = None) -> tuple[int, Any, str | None]:
        req = urllib.request.Request(API + path)
        req.add_header("Accept", "application/vnd.github+json")
        req.add_header("User-Agent", "swofty-triage")
        req.add_header("X-GitHub-Api-Version", "2022-11-28")
        if self.token:
            req.add_header("Authorization", f"Bearer {self.token}")
        if etag:
            req.add_header("If-None-Match", etag)
        try:
            with urllib.request.urlopen(req, timeout=30, context=TLS) as r:
                return r.status, json.loads(r.read()), r.headers.get("ETag")
        except urllib.error.HTTPError as e:
            if e.code == 304:
                return 304, None, etag
            raise RuntimeError(f"GitHub {path}: HTTP {e.code} {e.read()[:300]!r}") from e

    def releases(self) -> list[Release]:
        status, data, etag = self.request(f"/repos/{self.repo}/releases?per_page=30", self.etag)
        if status == 304:
            return self.cached
        self.etag = etag
        self.cached = [
            Release(
                tag=r["tag_name"],
                name=r.get("name") or r["tag_name"],
                url=r["html_url"],
                published_at=r["published_at"],
                assets=tuple(
                    Asset(a["name"], a["browser_download_url"], a["size"]) for a in r.get("assets", [])
                ),
            )
            for r in data
            if not r.get("draft") and not r.get("prerelease") and r.get("published_at")
        ]
        return self.cached

    def compare(self, base: str, head: str) -> list[Commit]:
        _, data, _ = self.request(f"/repos/{self.repo}/compare/{base}...{head}")
        return [
            Commit(
                sha=c["sha"],
                subject=c["commit"]["message"].split("\n", 1)[0],
                login=(c.get("author") or {}).get("login"),
                name=c["commit"]["author"]["name"],
            )
            for c in data.get("commits", [])
        ]


def previous(release: Release, releases: list[Release]) -> Release | None:
    older = [r for r in releases if r.published < release.published]
    return max(older, key=lambda r: r.published) if older else None


def version_word(tag: str) -> str:
    m = TAG.match(tag)
    if m and m.group(3) != "0":
        return "Minor Version"
    return "Major Version"


def contributors(commits: list[Commit]) -> list[tuple[str, str]]:
    counts: Counter[str] = Counter()
    first: dict[str, int] = {}
    links: dict[str, str] = {}
    for i, c in enumerate(commits):
        if c.login and c.login.endswith("[bot]"):
            continue
        key = c.login or c.name
        counts[key] += 1
        first.setdefault(key, i)
        links[key] = f"https://github.com/{c.login}/" if c.login else ""
    order = sorted(counts, key=lambda k: (-counts[k], first[k]))
    return [(k, links[k]) for k in order]


def plain_title(subject: str) -> str:
    text = VERSION_SUFFIX.sub("", PR_SUFFIX.sub("", TYPE_PREFIX.sub("", subject))).strip()
    return text[:1].upper() + text[1:]


def fallback_lines(commits: list[Commit]) -> list[dict[str, Any]]:
    return [
        {"text": plain_title(c.subject), "prs": [c.pr]}
        for c in commits
        if c.pr is not None and not re.match(r"^(chore|ci|build|test|style|docs)\b", c.subject)
    ]


def release_notes(sources: Sources, tag: str) -> str | None:
    version = tag.removeprefix("v")
    try:
        text = sources.git("launcher", "cat-file", "blob", f"{tag}:docs/releases/{version}.md", check=True)
    except Exception:
        return None
    return re.sub(r"^---\n.*?\n---\n", "", text, flags=re.S).strip() or None


def addon_names(sources: Sources, tag: str) -> dict[str, str]:
    names: dict[str, str] = {}
    try:
        paths = sources.git("launcher", "ls-tree", "-r", "--name-only", tag, "--", "addons").split()
    except Exception:
        return names
    for path in paths:
        if path.count("/") != 2 or not path.endswith("/swofty.json"):
            continue
        try:
            manifest = json.loads(sources.git("launcher", "cat-file", "blob", f"{tag}:{path}"))
        except Exception:
            continue
        if isinstance(manifest, dict) and manifest.get("id") and manifest.get("name"):
            names[manifest["id"]] = manifest["name"]
    return names


def write_lines(
    client: anthropic.Anthropic, model: str, release: Release, notes: str | None, commits: list[Commit]
) -> list[dict[str, Any]]:
    known = {c.pr for c in commits if c.pr is not None}
    commit_list = "\n".join(f"- {c.subject}" for c in commits) or "(none)"
    prompt = (
        f"Release {release.tag}.\n\n<release_notes>\n{notes or '(no release notes)'}\n</release_notes>\n\n"
        f"<commits>\n{commit_list}\n</commits>"
    )
    try:
        response = client.beta.messages.create(
            model=model,
            max_tokens=8000,
            system=WRITER,
            messages=[{"role": "user", "content": prompt}],
            output_config={"effort": "medium", "format": {"type": "json_schema", "schema": LINES_SCHEMA}},
            betas=[FALLBACK_BETA],
            extra_body={"fallbacks": "default"},
        )
    except anthropic.APIError as e:
        log.warning("%s: changelog writer failed (%s); using commit titles", release.tag, e)
        return fallback_lines(commits)
    if response.stop_reason in ("refusal", "max_tokens"):
        log.warning(
            "%s: changelog writer stopped with %s; using commit titles", release.tag, response.stop_reason
        )
        return fallback_lines(commits)
    text = next((b.text for b in response.content if b.type == "text"), "")
    try:
        lines = json.loads(text)["lines"]
    except (ValueError, KeyError, TypeError):
        return fallback_lines(commits)
    out = []
    for line in lines:
        words = str(line.get("text", "")).strip().rstrip(".")
        if words:
            out.append({"text": words, "prs": sorted({p for p in line.get("prs", []) if p in known})})
    return out or fallback_lines(commits)


def download_label(asset: Asset, addons: dict[str, str]) -> str | None:
    name = asset.name
    if name == "SHA256SUMS":
        return None
    if "windows" in name and name.endswith(".zip"):
        return "Windows"
    if "linux" in name and name.endswith((".tar.gz", ".zip")):
        return "Linux and Steam Deck"
    if name.endswith(".zip"):
        addon_id = name.removesuffix(".zip")
        return f"{addons[addon_id]} addon" if addon_id in addons else name
    return name


def announcement(
    project: str,
    repo: str,
    release: Release,
    lines: list[dict[str, Any]],
    addons: dict[str, str],
    people: list[tuple[str, str]],
    docs_url: str | None,
) -> str:
    repo_url = f"https://github.com/{repo}"
    where = f"available on our [GitHub](<{release.url}>)"
    if docs_url:
        where += f" and documented on our [Docs](<{docs_url}>)"
    out = [
        f"**{project} Release {release.tag}** :tada:",
        "",
        f"Today we announce the release of {version_word(release.tag)} {release.version}, {where}.",
        "",
        "Changelog:",
    ]
    for line in lines:
        cites = ", ".join(f"[#{n}](<{repo_url}/pull/{n}>)" for n in line["prs"])
        out.append(f"- {line['text']}" + (f" (PR {cites})" if cites else ""))
    downloads = sorted(
        ((label, a.url) for a in release.assets if (label := download_label(a, addons))),
        key=lambda d: {"Windows": 0, "Linux and Steam Deck": 1}.get(d[0], 2),
    )
    if downloads:
        out += ["", "Downloads:", *(f"- [{label}](<{url}>)" for label, url in downloads)]
    if people:
        out += ["", "Contributors:", *(f"- [{who}](<{url}>)" if url else f"- {who}" for who, url in people)]
    return "\n".join(out)


def compose(
    gh: GitHub,
    sources: Sources,
    client: anthropic.Anthropic,
    model: str,
    project: str,
    docs_base: str,
    release: Release,
    releases: list[Release],
) -> str:
    sources.update()
    before = previous(release, releases)
    commits = gh.compare(before.tag, release.tag) if before else []
    notes = release_notes(sources, release.tag)
    lines = write_lines(client, model, release, notes, commits)
    docs_url = f"{docs_base}/releases/{release.version}" if notes and docs_base else None
    return announcement(
        project, gh.repo, release, lines, addon_names(sources, release.tag), contributors(commits), docs_url
    )
