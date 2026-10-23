import argparse
import json
import logging
import sys
import time
from pathlib import Path

from . import config, report


def diagnose(args: argparse.Namespace) -> int:
    from .pipeline import Triage

    cfg = config.from_env(need_discord=False)
    content = args.content
    if args.message_json:
        content = json.loads(Path(args.message_json).read_text(encoding="utf-8"))["content"]
    if not content:
        raise SystemExit("swofty-triage: give --content or --message-json")
    header = report.parse_header(content)
    if header is None:
        raise SystemExit("swofty-triage: the content is not a Send Logs to Developers message")
    zips = [(Path(z).name, Path(z).read_bytes()) for z in args.zip]
    report_id = args.id or report.parse_attachment(zips[0][0])[0]
    result = Triage(cfg).run(report_id, header, zips, args.received or time.time())
    print(result.markdown)
    return 0


def update_sources(_: argparse.Namespace) -> int:
    from .source import Sources

    cfg = config.from_env(need_discord=False)
    problems = Sources(cfg.repos, cfg.repos_dir).update()
    for problem in problems:
        print(problem, file=sys.stderr)
    return 1 if problems else 0


def release_preview(args: argparse.Namespace) -> int:
    import anthropic

    from . import releases
    from .source import Sources

    cfg = config.from_env(need_discord=False)
    gh = releases.GitHub(cfg.releases_repo, cfg.github_token or None)
    found = gh.releases()
    release = next((r for r in found if r.tag == args.tag), None)
    if release is None:
        raise SystemExit(f"swofty-triage: no published release {args.tag} in {cfg.releases_repo}")
    sources = Sources(cfg.repos, cfg.repos_dir)
    text = releases.compose(
        gh, sources, anthropic.Anthropic(), cfg.model, cfg.releases_project, cfg.releases_docs, release, found
    )
    print(text)
    return 0


def usage(args: argparse.Namespace) -> int:
    import os

    from .relay import Relay
    from .usage import serve

    webhook = os.environ.get("SWOFTY_LOG_WEBHOOK", "").strip()
    serve(Path(args.db), args.host, args.port, Relay(webhook) if webhook else None)
    return 0


def run(_: argparse.Namespace) -> int:
    from .bot import TriageBot
    from .pipeline import Triage

    cfg = config.from_env()
    TriageBot(Triage(cfg)).run(cfg.discord_token, log_handler=None)
    return 0


def main() -> int:
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(name)s: %(message)s")
    logging.getLogger("discord").setLevel(logging.WARNING)
    logging.getLogger("httpx2").setLevel(logging.WARNING)
    parser = argparse.ArgumentParser(prog="swofty-triage")
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("run", help="watch the Discord channel and diagnose new reports").set_defaults(func=run)
    p = sub.add_parser("diagnose", help="diagnose one report from local zips and print the result")
    p.add_argument("zip", nargs="+")
    p.add_argument("--content", help="the webhook message text")
    p.add_argument("--message-json", help="a Discord message object saved as JSON")
    p.add_argument("--id", help="report id, the stem by default")
    p.add_argument("--received", type=float, help="Unix time the report arrived")
    p.set_defaults(func=diagnose)
    p = sub.add_parser("release-preview", help="print the announcement for a release without posting it")
    p.add_argument("tag")
    p.set_defaults(func=release_preview)
    p = sub.add_parser("usage", help="serve the usage API and the stats page")
    p.add_argument("--db", default="/var/lib/swofty-usage/usage.db")
    p.add_argument("--host", default="127.0.0.1")
    p.add_argument("--port", type=int, default=8471)
    p.set_defaults(func=usage)
    sub.add_parser("update-sources", help="clone or fetch the source mirrors").set_defaults(
        func=update_sources
    )
    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
