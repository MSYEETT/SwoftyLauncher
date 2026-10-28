import argparse
import concurrent.futures
import datetime
import json
import os
import shutil
import sys
from pathlib import Path

from . import catalog, compare, fetch, report, runner

DEFAULT_CACHE = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "swofty-compat"
DEFAULT_GAME = os.environ.get("F1_25_DIR", "/mnt/gaming/SteamLibrary/steamapps/common/F1 25")
DEFAULT_DATA = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share")) / "swofty"


def _write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.replace(tmp, path)


def load_catalog(args):
    path = Path(args.cache) / "catalog.json"
    if args.offline:
        if not path.is_file():
            raise runner.ToolError(f"--offline: no saved catalog at {path}")
        return json.loads(path.read_text())
    data = catalog.fetch(args.catalog_url)
    _write_json(path, data)
    return data


def _r2(args):
    return None if args.no_r2 else fetch.R2.from_env(args.r2_env)


def cmd_fetch(args):
    data = load_catalog(args)
    entries = catalog.entries(data)
    launcher = args.launcher_version
    picked = catalog.select(entries, launcher, latest=args.latest, only=catalog.parse_only(args.only))
    if args.with_requirements:
        extra = []
        for e in picked:
            extra += catalog.requirements(e, entries, launcher)[0]
        seen = {x.label for x in picked}
        picked += [x for x in extra if x.label not in seen and not seen.add(x.label)]
    r2 = _r2(args)
    total = sum(e.size for e in picked)
    print(f"{len(picked)} addon version(s), {total / 1e9:.2f} GB, into {Path(args.cache) / 'downloads'}")
    failed = 0
    for e in sorted(picked, key=lambda e: e.size):
        try:
            path = fetch.ensure(e, args.cache, r2, args.count_downloads, args.min_free_gb << 30)
            print(f"ok    {e.label:48} {e.size / 1e6:9.1f} MB  {path}")
        except (fetch.FetchError, OSError) as err:
            failed += 1
            print(f"error {e.label:48} {err}")
    return 2 if failed else 0


def _sets(args, entries, launcher):
    only = catalog.parse_only(args.only)
    picked = catalog.select(entries, launcher, latest=args.latest, only=only)
    sets = []

    def make(name, roots):
        notes, chosen = [], []
        for e in roots:
            deps, unresolved, outside = catalog.requirements(e, entries, launcher)
            for d in deps:
                if d.label not in {c.label for c in chosen} and d.id not in {r.id for r in roots}:
                    chosen.append(d)
            for addon_id, req in unresolved:
                notes.append(f"{e.label} requires {addon_id} {req}: no published version fits")
            for addon_id, req in outside:
                notes.append(f"{e.label} requires {addon_id} {req}: not on the Addon Browser (built in?)")
        ordered = []
        for c in chosen + list(roots):
            if c.id not in {o.id for o in ordered}:
                ordered.append(c)
        sets.append({"set": name, "entries": ordered, "notes": notes})

    if not args.no_per_version:
        for e in picked:
            make(e.label, [e])
    by_id = {}
    for e in picked:
        by_id.setdefault(e.id, e)
    if args.all_latest:
        make("profile:all-latest", list(by_id.values()))
    for spec in args.profile or []:
        name, _, items = spec.partition("=")
        roots = []
        for addon_id, version in catalog.parse_only([items]):
            if version:
                hit = [e for e in entries if e.id == addon_id and e.version == version]
            else:
                hit = [catalog.newest_matching(entries, addon_id, "*", launcher)]
            if not hit or hit[0] is None:
                raise runner.ToolError(f"--profile {name}: {addon_id}@{version or 'latest'} is not published")
            roots.append(hit[0])
        make(f"profile:{name}", roots)
    return sets


def _run_pair(args, a, b, index, item, seed, out, fetched, game):
    result = {"set": item["set"], "addons": [e.label for e in item["entries"]], "notes": list(item["notes"])}
    try:
        missing = [fetched[e.label] for e in item["entries"] if isinstance(fetched[e.label], str)]
        if missing:
            raise fetch.FetchError("; ".join(missing))
        zips = [fetched[e.label] for e in item["entries"]]
        addons = {"zips": zips, "enable": [e.id for e in item["entries"]]}
        free = shutil.disk_usage(out).free
        if free < args.min_free_gb << 30:
            raise runner.ToolError(f"only {free >> 30} GB free under {out}")
        before = runner.game_state(game)
        snaps = {}
        work = Path(out) / "work" / f"s{index:04d}"
        for side in (a, b):
            meta = {"set": item["set"], "side": side.name, "cli": str(side.cli), "version": side.version}
            snaps[side.name] = runner.build_set(
                side, addons, work / "data", game, seed=seed, cache=args.cache,
                timeout=args.timeout, pin=not args.no_pin, meta=meta,
            )
            _write_json(Path(out) / "snapshots" / f"{_safe(item['set'])}.{side.name}.json", snaps[side.name])
            kept = Path(out) / side.name / work.name
            if args.keep:
                shutil.rmtree(kept, ignore_errors=True)
                kept.parent.mkdir(parents=True, exist_ok=True)
                os.replace(work, kept)
            else:
                shutil.rmtree(work, ignore_errors=True)
        changed, n = runner.state_changes(before, runner.game_state(game))
        if n:
            result["notes"].append(
                f"the game folder changed during this set ({n} file(s), e.g. {', '.join(changed[:3])}); "
                "another session may have applied a build"
            )
        result["comparison"] = compare.compare(snaps[a.name], snaps[b.name], allow=set(args.allow or []))
        result["buildhash"] = {a.name: snaps[a.name]["buildhash"], b.name: snaps[b.name]["buildhash"]}
    except (runner.ToolError, fetch.FetchError, OSError) as e:
        result["error"] = str(e)
    return result


def _safe(name):
    return "".join(c if c.isalnum() or c in "._-@" else "_" for c in name)


def cmd_run(args):
    a = runner.Side("a", Path(args.cli_a).resolve(), Path(args.loader_a).resolve() if args.loader_a else None)
    b = runner.Side("b", Path(args.cli_b).resolve(), Path(args.loader_b).resolve() if args.loader_b else None)
    for side in (a, b):
        side.version = runner.cli_version(side.cli)
        if side.loader_dir() is None:
            raise runner.ToolError(f"{side.cli}: no loader found next to it; pass --loader-{side.name}")
    game = Path(args.game)
    if not (game / "F1_25.exe").is_file():
        raise runner.ToolError(f"{game}: no F1_25.exe; pass --game")
    launcher = args.launcher_version or a.version
    data = load_catalog(args)
    entries = catalog.entries(data)
    sets = _sets(args, entries, launcher)
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    out = Path(args.out) if args.out else Path(args.cache) / "runs" / stamp
    out.mkdir(parents=True, exist_ok=True)
    notes = []
    seed = None
    if args.baseline_from:
        seed = runner.seed_baseline(args.baseline_from, out / "baseline-seed")
        if seed is None:
            notes.append(f"no baseline under {args.baseline_from}; builds read the game folder as it is now")
        else:
            notes.append(f"originals come from the baseline in {args.baseline_from} (hard-linked)")
    meta = {
        "a": {"cli": str(a.cli), "version": a.version, "loader": str(a.loader_dir())},
        "b": {"cli": str(b.cli), "version": b.version, "loader": str(b.loader_dir())},
        "game": str(game),
        "launcher_version": launcher,
        "catalog_url": args.catalog_url,
        "allow": sorted(args.allow or []),
        "notes": notes,
        "started": stamp,
        "sets": len(sets),
    }
    print(f"{len(sets)} set(s); a = {a.version}, b = {b.version}; output in {out}", flush=True)
    r2 = _r2(args)
    fetched = {}
    for e in sorted({e.label: e for item in sets for e in item["entries"]}.values(), key=lambda e: e.size):
        try:
            fetched[e.label] = fetch.ensure(e, args.cache, r2, args.count_downloads, args.min_free_gb << 30)
        except (fetch.FetchError, OSError) as err:
            fetched[e.label] = str(err)
    results = [None] * len(sets)
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        futures = {
            pool.submit(_run_pair, args, a, b, i, item, seed, out, fetched, game): i for i, item in enumerate(sets)
        }
        for f in concurrent.futures.as_completed(futures):
            i = futures[f]
            results[i] = f.result()
            r = results[i]
            st = report.status_of(r)
            print(f"{st.upper():5} {r['set']}  {report.first_difference(r)}".rstrip(), flush=True)
    rep = report.build(meta, results)
    _write_json(out / "report.json", rep)
    text = report.summary(rep)
    (out / "summary.txt").write_text(text)
    if not args.keep:
        shutil.rmtree(out / "work", ignore_errors=True)
        shutil.rmtree(out / "baseline-seed", ignore_errors=True)
    print()
    print(text, end="")
    print(f"report: {out / 'report.json'}")
    return rep["exit"]


def cmd_compare(args):
    try:
        a = json.loads(Path(args.a).read_text())
        b = json.loads(Path(args.b).read_text())
    except (OSError, ValueError) as e:
        raise runner.ToolError(str(e)) from e
    result = {"set": a.get("set") or Path(args.a).name, "comparison": compare.compare(a, b, set(args.allow or []))}
    rep = report.build({"a": {"cli": args.a, "version": a.get("version")},
                        "b": {"cli": args.b, "version": b.get("version")},
                        "allow": sorted(args.allow or [])}, [result])
    if args.json:
        print(json.dumps(rep, indent=2, sort_keys=True))
    else:
        print(report.summary(rep), end="")
        for d in result["comparison"]["differences"]:
            print(f"  {d['category']}: {d['what']}\n    a: {d['a']}\n    b: {d['b']}")
    return rep["exit"]


def cmd_harness(args):
    print(
        "swofty-compat harness: the Wine half of the gate (boot each build to the main menu and compare "
        "the loaded DLLs, mixin chains and start lines) is not built yet; see tools/compat/README.md, "
        "\"The Wine harness\".",
        file=sys.stderr,
    )
    return 2


def parser():
    p = argparse.ArgumentParser(prog="tools/compat/run.sh", description="The 1.6 compatibility gate.")
    sub = p.add_subparsers(dest="command", required=True)

    def common(sp):
        sp.add_argument("--cache", default=str(DEFAULT_CACHE), help="download and run cache (default %(default)s)")
        sp.add_argument("--catalog-url", default=os.environ.get("SWOFTY_REGISTRY_URL", catalog.DEFAULT_URL))
        sp.add_argument("--offline", action="store_true", help="use the saved catalog instead of fetching it")
        sp.add_argument("--only", action="append", metavar="ID[@VERSION]", help="only these addons (repeatable)")
        sp.add_argument("--latest", action="store_true", help="only the newest version of each addon")
        sp.add_argument("--launcher-version", help="keep the versions whose launcher range accepts this version")
        sp.add_argument("--r2-env", default=str(fetch.DEFAULT_R2_ENV), help="R2 read credentials (KEY=VALUE file)")
        sp.add_argument("--no-r2", action="store_true", help="do not look the files up in the CDN bucket")
        sp.add_argument("--count-downloads", action="store_true",
                        help="fall back to the registry's download URL, which counts a download for the addon")
        sp.add_argument("--min-free-gb", type=int, default=10, help="keep this much disk free (default %(default)s)")

    f = sub.add_parser("fetch", help="download addon versions into the cache")
    common(f)
    f.add_argument("--with-requirements", action="store_true", help="also fetch what each version requires")
    f.set_defaults(func=cmd_fetch)

    r = sub.add_parser("run", help="build every set with CLI A and CLI B and compare")
    common(r)
    r.add_argument("--cli-a", required=True, help="the reference swofty binary (the 1.5 release)")
    r.add_argument("--cli-b", required=True, help="the swofty binary under test")
    r.add_argument("--loader-a", help="loader dir for CLI A (default: next to the binary)")
    r.add_argument("--loader-b", help="loader dir for CLI B (default: next to the binary)")
    r.add_argument("--game", default=DEFAULT_GAME, help="the game folder, read only (default %(default)s)")
    r.add_argument("--baseline-from", default=str(DEFAULT_DATA) if (DEFAULT_DATA / "baseline").is_dir() else None,
                   help="a launcher data dir whose baseline holds the stock originals (default %(default)s)")
    r.add_argument("--out", help="output dir (default: <cache>/runs/<time>)")
    r.add_argument("--profile", action="append", metavar="NAME=ID[@V],ID[@V]",
                   help="also build this whole profile (repeatable)")
    r.add_argument("--all-latest", action="store_true", help="also build one profile of every selected addon")
    r.add_argument("--no-per-version", action="store_true", help="skip the one-addon sets")
    r.add_argument("--jobs", type=int, default=2, help="sets built at once (default %(default)s)")
    r.add_argument("--timeout", type=int, default=3600, help="seconds per CLI call (default %(default)s)")
    r.add_argument("--keep", action="store_true", help="keep the scratch data dirs")
    r.add_argument("--no-pin", action="store_true", help="do not run the CLIs under taskset and nice")
    r.add_argument("--allow", action="append", choices=compare.CATEGORIES,
                   help="report this category of difference without failing (repeatable)")
    r.set_defaults(func=cmd_run)

    c = sub.add_parser("compare", help="compare two saved snapshots")
    c.add_argument("a")
    c.add_argument("b")
    c.add_argument("--allow", action="append", choices=compare.CATEGORIES)
    c.add_argument("--json", action="store_true")
    c.set_defaults(func=cmd_compare)

    h = sub.add_parser("harness", help="the Wine half of the gate (round 3; not built yet)")
    h.add_argument("rest", nargs=argparse.REMAINDER)
    h.set_defaults(func=cmd_harness)
    return p


def main(argv=None):
    args = parser().parse_args(argv)
    try:
        return args.func(args)
    except (runner.ToolError, fetch.FetchError) as e:
        print(f"swofty-compat: {e}", file=sys.stderr)
        return 2
    except OSError as e:
        print(f"swofty-compat: {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
