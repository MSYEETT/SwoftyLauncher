import os
import re
import subprocess
import threading
from pathlib import Path, PurePosixPath

from .config import Repo

REF = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._/^~-]{0,120}$")
MAX_FILE = 400_000
MAX_OUTPUT = 60_000


class SourceError(Exception):
    pass


def check_ref(ref: str) -> str:
    if not REF.match(ref) or ".." in ref or ref.endswith((".lock", "/")):
        raise SourceError(f"not a ref: {ref!r}")
    return ref


def check_path(path: str) -> str:
    path = path.strip().strip("/")
    if not path:
        return ""
    p = PurePosixPath(path)
    if p.is_absolute() or any(part in ("..", ".") for part in p.parts) or path.startswith("-"):
        raise SourceError(f"not a repository path: {path!r}")
    return p.as_posix()


class Sources:
    def __init__(self, repos: tuple[Repo, ...], root: Path):
        self.repos = {r.name: r for r in repos}
        self.root = root
        self.lock = threading.Lock()

    def names(self) -> list[str]:
        return list(self.repos)

    def dir(self, name: str) -> Path:
        if name not in self.repos:
            raise SourceError(f"no repository named {name!r}; known: {', '.join(self.repos)}")
        return self.root / f"{name}.git"

    def env(self, name: str) -> dict[str, str]:
        env = {k: v for k, v in os.environ.items() if not k.startswith(("ANTHROPIC_", "DISCORD_"))}
        env["GIT_TERMINAL_PROMPT"] = "0"
        key = self.root / f"{name}.key"
        if key.is_file():
            env["GIT_SSH_COMMAND"] = (
                f"ssh -i {key} -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new -o BatchMode=yes"
            )
        return env

    def git(self, name: str, *args: str, timeout: int = 60, check: bool = True) -> str:
        result = subprocess.run(
            ["git", "--git-dir", str(self.dir(name)), *args],
            capture_output=True,
            timeout=timeout,
            env=self.env(name),
        )
        out = result.stdout.decode("utf-8", errors="replace")
        if check and result.returncode != 0:
            err = result.stderr.decode("utf-8", errors="replace").strip()
            raise SourceError(err.splitlines()[-1] if err else f"git {args[0]} failed ({result.returncode})")
        return out

    def update(self) -> list[str]:
        problems = []
        with self.lock:
            self.root.mkdir(parents=True, exist_ok=True)
            for name, repo in self.repos.items():
                path = self.dir(name)
                try:
                    if not path.is_dir():
                        subprocess.run(
                            ["git", "clone", "--mirror", "--quiet", repo.url, str(path)],
                            check=True,
                            capture_output=True,
                            timeout=900,
                            env=self.env(name),
                        )
                    else:
                        self.git(name, "remote", "update", "--prune", timeout=300)
                except (subprocess.CalledProcessError, subprocess.TimeoutExpired, SourceError) as e:
                    problems.append(f"{name}: {e}")
        return problems

    def available(self, name: str) -> bool:
        return self.dir(name).is_dir()

    def resolve(self, name: str, ref: str) -> str | None:
        try:
            out = self.git(
                name, "rev-parse", "--verify", "--quiet", f"{check_ref(ref)}^{{commit}}", check=False
            )
        except SourceError:
            return None
        return out.strip() or None

    def describe(self, name: str, ref: str) -> str:
        sha = self.resolve(name, ref)
        if not sha:
            return f"{ref}: not found"
        line = self.git(name, "log", "-1", "--format=%h %cs %s", sha).strip()
        return f"{ref} = {line}"

    def ls(self, name: str, ref: str, path: str = "") -> str:
        ref, path = check_ref(ref), check_path(path)
        out = self.git(name, "ls-tree", "-l", ref, *(["--", path + "/"] if path else []))
        if not out and path:
            out = self.git(name, "ls-tree", "-l", ref, "--", path)
        if not out:
            return f"nothing at {path or '/'} in {ref}"
        lines = []
        for line in out.splitlines():
            meta, _, file = line.partition("\t")
            kind, size = meta.split()[1], meta.split()[3]
            lines.append(f"{file}/" if kind == "tree" else f"{file}  ({size} bytes)")
        return clip("\n".join(lines))

    def read(self, name: str, ref: str, path: str, offset: int = 1, limit: int = 400) -> str:
        ref, path = check_ref(ref), check_path(path)
        if not path:
            raise SourceError("give a file path")
        spec = f"{ref}:{path}"
        kind = self.git(name, "cat-file", "-t", spec, check=False).strip()
        if kind == "tree":
            return self.ls(name, ref, path)
        if kind != "blob":
            raise SourceError(f"{path} does not exist in {ref}")
        size = int(self.git(name, "cat-file", "-s", spec).strip())
        if size > MAX_FILE * 8:
            raise SourceError(f"{path} is {size} bytes; use grep_source on it instead")
        text = self.git(name, "cat-file", "blob", spec)
        return number(text, offset, limit, f"{name}:{ref}:{path}")

    def grep(
        self,
        name: str,
        ref: str,
        pattern: str,
        path: str = "",
        ignore_case: bool = False,
        max_results: int = 80,
    ) -> str:
        ref, path = check_ref(ref), check_path(path)
        if not pattern or len(pattern) > 300:
            raise SourceError("pattern must be 1-300 characters")
        args = ["grep", "-n", "-I", "-E", "--max-count=25", "--full-name"]
        if ignore_case:
            args.append("-i")
        args += ["-e", pattern, ref, "--"]
        if path:
            args.append(path)
        out = self.git(name, *args, timeout=60, check=False)
        prefix = f"{ref}:"
        lines = [line.removeprefix(prefix) for line in out.splitlines()]
        if not lines:
            return f"no matches for /{pattern}/ in {ref}" + (f" under {path}" if path else "")
        shown = [line if len(line) <= 300 else line[:300] + " …" for line in lines[:max_results]]
        more = len(lines) - len(shown)
        return clip(
            "\n".join(shown) + (f"\n[{more} more matches; narrow the path or pattern]" if more else "")
        )

    def log(self, name: str, base: str, head: str, path: str = "", max_count: int = 60) -> str:
        base, head, path = check_ref(base), check_ref(head), check_path(path)
        args = [
            "log",
            "--no-decorate",
            f"-n{max(1, min(max_count, 200))}",
            "--format=%h %cs %s",
            f"{base}..{head}",
        ]
        if path:
            args += ["--", path]
        out = self.git(name, *args).strip()
        return clip(out) if out else f"no commits in {base}..{head}" + (f" touching {path}" if path else "")

    def show_commit(self, name: str, commit: str, path: str = "") -> str:
        commit, path = check_ref(commit), check_path(path)
        args = ["show", "--no-color", "--stat", "--patch", "-M", "--format=%H%n%an %cs%n%n%B", commit]
        if path:
            args += ["--", path]
        return clip(self.git(name, *args))

    def tags(self, name: str) -> list[str]:
        return self.git(name, "tag", "--list", "v*", "--sort=-v:refname").split()


def number(text: str, offset: int, limit: int, label: str) -> str:
    lines = text.splitlines()
    offset = max(1, offset)
    limit = max(1, min(limit, 2000))
    chunk = lines[offset - 1 : offset - 1 + limit]
    if not chunk:
        return f"{label} has {len(lines)} lines; offset {offset} is past the end"
    body = "\n".join(f"{i:>6}\t{line[:400]}" for i, line in enumerate(chunk, offset))
    end = offset + len(chunk) - 1
    note = f"[{label}: lines {offset}-{end} of {len(lines)}]"
    return clip(f"{note}\n{body}")


def clip(text: str, limit: int = MAX_OUTPUT) -> str:
    return text if len(text) <= limit else text[:limit] + f"\n[… cut at {limit} characters]"
