import os
from dataclasses import dataclass
from pathlib import Path

DEFAULT_REPOS = "launcher=https://github.com/Swofty-Developments/SwoftyLauncher.git"


@dataclass(frozen=True)
class Repo:
    name: str
    url: str


@dataclass(frozen=True)
class Config:
    discord_token: str
    channel_id: int
    webhook_id: int
    data_dir: Path
    repos: tuple[Repo, ...]
    model: str = "claude-opus-5-5"
    effort: str = "high"
    max_turns: int = 40
    max_cost_usd: float = 4.0
    daily_limit: int = 40
    per_name_hourly: int = 3
    backfill_from: int = 0
    part_wait_s: int = 1800

    @property
    def cases_dir(self) -> Path:
        return self.data_dir / "cases"

    @property
    def repos_dir(self) -> Path:
        return self.data_dir / "repos"

    @property
    def db_path(self) -> Path:
        return self.data_dir / "triage.db"


def parse_repos(raw: str) -> tuple[Repo, ...]:
    repos = []
    for item in raw.replace("\n", ",").split(","):
        item = item.strip()
        if not item:
            continue
        name, sep, url = item.partition("=")
        if not sep or not name.strip().isidentifier() or not url.strip():
            raise ValueError(f"TRIAGE_REPOS entry {item!r} is not name=url")
        repos.append(Repo(name.strip(), url.strip()))
    if not repos or repos[0].name != "launcher":
        raise ValueError("TRIAGE_REPOS must start with launcher=<url>")
    return tuple(repos)


def from_env(env: dict[str, str] | None = None, need_discord: bool = True) -> Config:
    env = dict(os.environ if env is None else env)

    def need(key: str) -> str:
        value = env.get(key, "").strip()
        if not value:
            raise SystemExit(f"swofty-triage: set {key}")
        return value

    if not env.get("ANTHROPIC_API_KEY") and not env.get("ANTHROPIC_AUTH_TOKEN"):
        raise SystemExit("swofty-triage: set ANTHROPIC_API_KEY")
    return Config(
        discord_token=need("DISCORD_TOKEN") if need_discord else env.get("DISCORD_TOKEN", ""),
        channel_id=int(need("TRIAGE_CHANNEL_ID")) if need_discord else int(env.get("TRIAGE_CHANNEL_ID", "0")),
        webhook_id=int(need("TRIAGE_WEBHOOK_ID")) if need_discord else int(env.get("TRIAGE_WEBHOOK_ID", "0")),
        data_dir=Path(env.get("TRIAGE_DATA_DIR", "/var/lib/swofty-triage")),
        repos=parse_repos(env.get("TRIAGE_REPOS", DEFAULT_REPOS)),
        model=env.get("TRIAGE_MODEL", "claude-opus-5-5"),
        effort=env.get("TRIAGE_EFFORT", "high"),
        max_turns=int(env.get("TRIAGE_MAX_TURNS", "40")),
        max_cost_usd=float(env.get("TRIAGE_MAX_COST_USD", "4")),
        daily_limit=int(env.get("TRIAGE_DAILY_LIMIT", "40")),
        per_name_hourly=int(env.get("TRIAGE_PER_NAME_HOURLY", "3")),
        backfill_from=int(env.get("TRIAGE_BACKFILL_FROM") or 0),
        part_wait_s=int(env.get("TRIAGE_PART_WAIT_S", "1800")),
    )
