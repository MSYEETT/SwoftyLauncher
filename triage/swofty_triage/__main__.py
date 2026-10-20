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
    sub.add_parser("update-sources", help="clone or fetch the source mirrors").set_defaults(
        func=update_sources
    )
    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
