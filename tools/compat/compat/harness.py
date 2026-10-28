import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

from . import report

PIN = ["taskset", "-c", "2-7,10,11,14-31", "nice", "-n", "10"]
DEFAULT_LOCK = "/mnt/work/.f1-game.lock.d"
DEFAULT_QUEUE = "/mnt/work/.f1-game.queue"
WHO = "std16-gate2"
PROFILE = "compat"
CATEGORIES = ("dll", "drop", "chain", "conflict", "unresolved", "register", "start", "run")
CORE_DLL = "swofty_core.dll"

STAMP = re.compile(r"^\d{4}-\d\d-\d\d \d\d:\d\d:\d\d(?:\.\d+)? \[\s*\d+\] ?")
POINTER = re.compile(r"\b0x[0-9a-fA-F]{9,}\b|\b[0-9A-F]{16}\b")
DURATION = re.compile(r"\b\d+ ?ms\b")
LOADED = re.compile(r"^addon (\S+): loaded at \S+, (.*)$")
ALSO = re.compile(r"^addon (\S+): (also in|settings\.json lists|listed twice|loaded at \S+ but exports no|too many|exports SwoftyCoreAttach but did not)(.*)$")
DROPPED = re.compile(r"^\[core\] addon (\S+) dropped: (.*)$")
INSTALLED = re.compile(r"^\[core\] (\w+) (\S+) at (?:0x)?\S+ \(base\+0x[0-9a-f]+\) installed: (.*)$")
OWNER = re.compile(r"^(\S+?)\((-?\d+)(, overwrite)?\)$")
CONFLICT = re.compile(r"^\[core\] (conflict|warning): ")
NEEDS = re.compile(r"^\[core\] addon (\S+) (needs .*|waits: .*)$")
REGISTERED = re.compile(r"^\[core\] addon (\S+): registered (\d+) mixin\(s\)")
REFUSED = re.compile(r"^\[core\] addon (\S+): (.*(?:refused|malformed).*)$")
NOISE = re.compile(
    r"^(\[core\] (bind |binding |scan|caches|hook pool|build |bindings|session\.json|binding resolution|core mixin set)|"
    r"\[bind\] |scan |swofty loader|exe: |\[core\]   )"
)
SECTION_END = re.compile(r"^(addons:|stage:|early stage:|\[core\] (commit|.* pass:|bindings:|hook pool|conflict|warning)|"
                         r"trigger|fallback)")


def clean(line, roots=()):
    line = STAMP.sub("", line.rstrip("\r\n"))
    for root in roots:
        for v in (root, "Z:" + root.replace("/", "\\"), "Z:" + root.replace("/", "\\\\")):
            line = line.replace(v, "<game>")
    line = POINTER.sub("<ptr>", line)
    return DURATION.sub("<ms>", line)


def parse_log(text, roots=()):
    out = {"dlls": [], "drops": [], "chains": {}, "conflicts": [], "unresolved": {}, "registered": {}, "start": {}}
    stage = "window"
    current = None
    for raw in text.splitlines():
        if not raw.strip():
            continue
        line = clean(raw, roots)
        if "(early stage)" in line:
            stage = "early"
        elif "loading the addons without" in line:
            stage = "window"
        m = LOADED.match(line)
        if m:
            kind = "v1" if "API v1 plugin" in m.group(2) else "v2+"
            out["dlls"].append({"dll": m.group(1), "stage": stage, "api": kind})
            current = m.group(1)
            out["start"].setdefault(current, [])
            continue
        m = ALSO.match(line)
        if m:
            out["drops"].append(f"{m.group(1)}: {m.group(2)}{m.group(3)}".strip())
            continue
        m = DROPPED.match(line)
        if m:
            out["drops"].append(f"{m.group(1)}: {m.group(2)}")
            continue
        m = INSTALLED.match(line)
        if m:
            owners = []
            for part in m.group(3).split(" > "):
                o = OWNER.match(part.strip())
                owners.append({"addon": o.group(1), "priority": int(o.group(2)), "overwrite": bool(o.group(3))}
                              if o else {"addon": part.strip(), "priority": None, "overwrite": False})
            out["chains"][f"{m.group(1)} {m.group(2)}"] = owners
            current = None
            continue
        if CONFLICT.match(line):
            out["conflicts"].append(line)
            current = None
            continue
        m = NEEDS.match(line)
        if m:
            out["unresolved"].setdefault(m.group(1), []).append(m.group(2))
            continue
        m = REGISTERED.match(line)
        if m:
            out["registered"][m.group(1)] = int(m.group(2))
            continue
        m = REFUSED.match(line)
        if m:
            out["drops"].append(f"{m.group(1)}: {m.group(2)}")
            continue
        if NOISE.match(line) or SECTION_END.match(line):
            current = None
            continue
        if current:
            out["start"][current].append(line)
    return out


def _diff(category, what, a, b, addon=None):
    d = {"category": category, "what": what, "a": a, "b": b}
    if addon:
        d["addon"] = addon
    return d


def _only_core(owners):
    return all(o["addon"] == CORE_DLL for o in owners)


def _without_core(owners):
    return [o for o in owners if o["addon"] != CORE_DLL]


def _fmt(owners):
    if owners is None:
        return None
    return " > ".join(f"{o['addon']}({o['priority']}{', overwrite' if o['overwrite'] else ''})" for o in owners)


def compare_logs(a, b, allow=()):
    diffs, allowed = [], []

    def explain(d, why):
        d["explained"] = why
        allowed.append(d)

    la = [(x["dll"], x["stage"]) for x in a["dlls"]]
    lb = [(x["dll"], x["stage"]) for x in b["dlls"]]
    core_b = [x for x in lb if x[0] == CORE_DLL and x not in la]
    lb_cmp = [x for x in lb if x not in core_b]
    la_cmp = [x for x in la if x[0] != CORE_DLL or x in lb]
    if la_cmp != lb_cmp:
        only_a = [x for x in la_cmp if x not in lb_cmp]
        only_b = [x for x in lb_cmp if x not in la_cmp]
        what = "DLLs loaded" if only_a or only_b else "DLLs loaded in a different order or stage"
        diffs.append(_diff("dll", what, [f"{n} ({s})" for n, s in (only_a or la_cmp)],
                           [f"{n} ({s})" for n, s in (only_b or lb_cmp)]))
    apis_a = {x["dll"]: x["api"] for x in a["dlls"]}
    for x in b["dlls"]:
        if x["dll"] in apis_a and apis_a[x["dll"]] != x["api"]:
            diffs.append(_diff("dll", f"{x['dll']}: API level", apis_a[x["dll"]], x["api"], x["dll"]))

    new_drops = [d for d in b["drops"] if d not in a["drops"]]
    gone = [d for d in a["drops"] if d not in b["drops"]]
    if new_drops:
        diffs.append(_diff("drop", "DLLs dropped on b and not on a", gone, new_drops))
    elif gone:
        diffs.append(_diff("drop", "DLLs dropped on a and not on b", gone, []))

    for key in sorted(set(a["chains"]) | set(b["chains"])):
        ca, cb = a["chains"].get(key), b["chains"].get(key)
        if ca == cb:
            continue
        d = _diff("chain", f"mixin chain {key}", _fmt(ca), _fmt(cb))
        if ca is None and cb is not None and _only_core(cb):
            explain(d, "a mixin chain only the 1.6 core installs")
        elif ca is not None and cb is not None and _without_core(ca) == _without_core(cb):
            explain(d, "the addons' order is the same; the 1.6 core's own handler differs (a hook point subscriber)")
        elif "chain" in allow:
            allowed.append(d)
        else:
            diffs.append(d)
    ka, kb = [c for c in a["conflicts"]], [c for c in b["conflicts"]]
    if ka != kb:
        diffs.append(_diff("conflict", "conflict pass output", sorted(set(ka) - set(kb)) or ka, sorted(set(kb) - set(ka)) or kb))
    for addon in sorted(set(a["unresolved"]) | set(b["unresolved"])):
        ua, ub = a["unresolved"].get(addon, []), b["unresolved"].get(addon, [])
        if ua != ub:
            diffs.append(_diff("unresolved", f"{addon}: bindings it needs that do not resolve", ua, ub, addon))
    for addon in sorted(set(a["registered"]) | set(b["registered"])):
        if a["registered"].get(addon) != b["registered"].get(addon):
            diffs.append(_diff("register", f"{addon}: mixins registered", a["registered"].get(addon),
                               b["registered"].get(addon), addon))
    for dll in sorted(set(a["start"]) | set(b["start"])):
        sa, sb = a["start"].get(dll, []), b["start"].get(dll, [])
        if sa != sb:
            diffs.append(_diff("start", f"{dll}: start lines", [l for l in sa if l not in sb] or sa,
                               [l for l in sb if l not in sa] or sb, dll))
    allowed_cats = set(allow)
    kept = []
    for d in diffs:
        (allowed if d["category"] in allowed_cats else kept).append(d)
    return {"same": not kept, "differences": kept, "allowed": allowed}


class LockWait(Exception):
    pass


class Lock:
    def __init__(self, path, queue, who, wait, hold, sleep=time.sleep, now=time.time):
        self.path, self.queue, self.who = Path(path), Path(queue), who
        self.wait, self.hold, self.sleep, self.now = wait, hold, sleep, now
        self.entry = None
        self.since = None

    def _queued(self):
        try:
            return [l.strip() for l in self.queue.read_text().splitlines() if l.strip()]
        except OSError:
            return []

    def _enqueue(self):
        self.entry = f"{self.who} {time.strftime('%H:%M')}"
        with open(self.queue, "a") as f:
            f.write(self.entry + "\n")

    def _dequeue(self):
        if self.entry is None:
            return
        keep = [l for l in self._queued() if l != self.entry]
        self.queue.write_text("".join(l + "\n" for l in keep))
        self.entry = None

    def take(self):
        if not self.path.parent.is_dir():
            self.since = self.now()
            return
        started = self.now()
        while True:
            line_up = [l for l in self._queued() if l != self.entry]
            first = (self._queued() or [None])[0]
            my_turn = self.entry is not None and first == self.entry
            if (not line_up or my_turn) and self._mkdir():
                self._dequeue()
                (self.path / "owner").write_text(f"gate2 wine {time.strftime('%H:%M')}\n")
                self.since = self.now()
                return
            if self.entry is None and (line_up or self.path.exists()):
                self._enqueue()
            if self.now() - started >= self.wait:
                self._dequeue()
                raise LockWait(f"the game lock is held by {self.owner()}; queue: {self._queued() or 'empty'}")
            self.sleep(5)

    def _mkdir(self):
        try:
            self.path.mkdir()
            return True
        except FileExistsError:
            return False

    def owner(self):
        try:
            return (self.path / "owner").read_text().strip() or "unknown"
        except OSError:
            return "unknown"

    def expired(self):
        return self.since is not None and self.now() - self.since >= self.hold

    def release(self):
        if self.since is not None and self.path.parent.is_dir():
            shutil.rmtree(self.path, ignore_errors=True)
        self.since = None


def remap(path, mapping):
    for old, new in mapping:
        if path == old or path.startswith(old + "/"):
            return new + path[len(old):]
    return path


def side_mapping(out, side, index):
    work = f"{out}/work/s{index:04d}/data"
    kept = f"{out}/{side}/s{index:04d}/data"
    return [(work, kept)]


def load_plan(out, side, index, buildhash):
    data = Path(out) / side / f"s{index:04d}" / "data"
    path = data / "cache" / "overlay" / PROFILE / buildhash / "plan.json"
    return data, json.loads(path.read_text())


def prepare_game(plan, mapping, dest, exe, max_copy=512 << 20):
    dest = Path(dest)
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    copied, skipped, missing = 0, [], []
    for kind in ("replace", "add", "loader"):
        for rel, src in sorted((plan.get(kind) or {}).items()):
            if not isinstance(src, str):
                continue
            src = remap(src, mapping)
            target = dest / rel
            try:
                size = os.stat(src).st_size
            except OSError:
                missing.append(rel)
                continue
            if size > max_copy and kind != "loader":
                skipped.append(rel)
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src, target)
            copied += 1
    shutil.copyfile(exe, dest / "F1_25.exe")
    return {"copied": copied, "skipped": skipped, "missing": missing}


def wine_env(prefix):
    env = dict(os.environ)
    env.update(WINEPREFIX=str(prefix), WINEDLLOVERRIDES="version=n,b;mscoree,mshtml=;winemenubuilder.exe,winedbg.exe=d",
               WINEDEBUG="-all", SWOFTY_USAGE="0")
    return env


def run_wine(game, prefix, mode, timeout, pin=True):
    cmd = (PIN if pin else []) + ["xvfb-run", "-a", "wine", str(Path(game) / "F1_25.exe"), mode]
    try:
        p = subprocess.run(cmd, cwd=game, env=wine_env(prefix), capture_output=True, text=True, timeout=timeout)
        rc, stdout = p.returncode, p.stdout + p.stderr
    except subprocess.TimeoutExpired as e:
        rc, stdout = "timeout", (e.stdout or b"").decode(errors="replace") if isinstance(e.stdout, bytes) else (e.stdout or "")
        subprocess.run(["wineserver", "-k"], env=wine_env(prefix), capture_output=True)
    log = Path(game) / "swofty" / "loader.log"
    text = log.read_text(errors="replace") if log.is_file() else ""
    return rc, text, stdout


def build_fake_exe(repo, dest):
    dest = Path(dest)
    if dest.is_file():
        return dest
    dest.parent.mkdir(parents=True, exist_ok=True)
    cache = dest.parent / "zig-cache"
    env = dict(os.environ, ZIG_GLOBAL_CACHE_DIR=str(cache), ZIG_LOCAL_CACHE_DIR=str(cache))
    src = Path(repo) / "native" / "test" / "fake_game.c"
    cmd = PIN + ["zig", "cc", "-target", "x86_64-windows-gnu", "-O2", "-o", str(dest), str(src), "-lversion", "-luser32"]
    p = subprocess.run(cmd, env=env, capture_output=True, text=True)
    if p.returncode:
        raise RuntimeError(f"building the synthetic F1_25.exe failed:\n{p.stderr[-2000:]}")
    return dest


def harness_sets(rep):
    out = []
    for i, r in enumerate(rep["results"]):
        out.append((i, r))
    return out


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.replace(tmp, path)
