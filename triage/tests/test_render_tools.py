import subprocess

import pytest

from swofty_triage import render, source
from swofty_triage.config import Repo, parse_repos
from swofty_triage.source import SourceError, Sources
from swofty_triage.tools import SUBMIT, Toolbox, validate_diagnosis

DIAGNOSIS = {
    "title": "Offline play staging fails on non-ASCII user names",
    "category": "launch-failure",
    "confidence": "high",
    "summary": "Staging aborts.",
    "root_cause": "The tool reads ANSI argv.",
    "evidence": ["state/wrapper-run.log:12 staging failed"],
    "fixed_on_main": "no",
    "fixed_by": "",
    "duplicate_of": "",
    "user_reply": "Set SWOFTY_DATA_DIR to an ASCII folder.",
    "fix_repo": "launcher",
    "fix_prompt": "## Problem\nStaging fails.\n\n```text\nerror\n```\n",
}


def test_split_keeps_messages_under_the_limit_and_fences_balanced():
    body = "\n".join(f"line {i} " + "x" * 50 for i in range(200))
    text = f"intro\n```rust\n{body}\n```\noutro"
    chunks = render.split(text, 500)
    assert len(chunks) > 5
    for chunk in chunks:
        assert len(chunk) <= 520
        assert chunk.count("```") % 2 == 0
    assert chunks[0].startswith("intro")
    assert chunks[-1].endswith("outro")


def test_split_cuts_overlong_lines():
    chunks = render.split("a" * 1200, 500)
    assert [len(c) for c in chunks] == [500, 500, 200]


def test_messages_are_plain_text_under_discord_limit():
    out = render.messages(DIAGNOSIS, 0.29, 7, "https://discord.com/channels/1/2/3")
    assert out[0].startswith("## Offline play staging fails")
    assert any(m.startswith("**Reply to the player**\n>>> ") for m in out)
    assert any("Fix prompt" in m for m in out)
    assert all(len(m) <= 2000 for m in out)


def test_messages_without_fix_prompt():
    d = dict(DIAGNOSIS, fix_prompt="", fix_repo="")
    assert not any("Fix prompt" in m for m in render.messages(d, 0.1, 2, "u"))


def test_validate_diagnosis():
    assert validate_diagnosis(dict(DIAGNOSIS), ["launcher"])["title"] == DIAGNOSIS["title"]
    with pytest.raises(Exception, match="category"):
        validate_diagnosis(dict(DIAGNOSIS, category="nope"), ["launcher"])
    with pytest.raises(Exception, match="missing"):
        validate_diagnosis({k: v for k, v in DIAGNOSIS.items() if k != "summary"}, ["launcher"])
    with pytest.raises(Exception, match="unknown"):
        validate_diagnosis(dict(DIAGNOSIS, extra="x"), ["launcher"])
    assert set(SUBMIT["input_schema"]["required"]) == set(DIAGNOSIS)


@pytest.mark.parametrize("ref", ["--output=/tmp/x", "-n", "a..b", "v1 1", "HEAD:secret", ""])
def test_bad_refs(ref):
    with pytest.raises(SourceError):
        source.check_ref(ref)


@pytest.mark.parametrize("path", ["../x", "a/../../b", "-rf"])
def test_bad_paths(path):
    with pytest.raises(SourceError):
        source.check_path(path)


def test_parse_repos():
    repos = parse_repos("launcher=https://x/a.git, offlineplay=git@github.com:o/b.git")
    assert repos == (Repo("launcher", "https://x/a.git"), Repo("offlineplay", "git@github.com:o/b.git"))
    with pytest.raises(ValueError):
        parse_repos("other=https://x")


@pytest.fixture
def toolbox(tmp_path):
    work = tmp_path / "work"
    work.mkdir()
    subprocess.run(["git", "init", "-q", "-b", "main", str(work)], check=True)
    (work / "src").mkdir()
    (work / "src" / "a.rs").write_text("fn main() {\n    stage();\n}\n")
    subprocess.run(["git", "-C", str(work), "add", "."], check=True)
    subprocess.run(
        ["git", "-C", str(work), "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "init"],
        check=True,
    )
    subprocess.run(["git", "-C", str(work), "tag", "v1.0.0"], check=True)
    sources = Sources((Repo("launcher", str(work)),), tmp_path / "repos")
    assert sources.update() == []
    logs = tmp_path / "logs"
    (logs / "state").mkdir(parents=True)
    (logs / "state" / "wrapper-run.log").write_text("ok\nstaging failed here\n")
    (tmp_path / "secret.env").write_text("DISCORD_TOKEN=x")
    return Toolbox(logs, sources, lambda n: [])


def test_log_tools_stay_inside_the_bundle(toolbox):
    assert "staging failed" in toolbox.run("read_log", {"path": "state/wrapper-run.log"}).text
    assert toolbox.run("read_log", {"path": "../secret.env"}).is_error
    assert toolbox.run("read_log", {"path": "/etc/passwd"}).is_error
    hit = toolbox.run("grep_logs", {"pattern": "staging"})
    assert "state/wrapper-run.log:2:" in hit.text


def test_source_tools(toolbox):
    assert (
        "stage();"
        in toolbox.run("read_source", {"repo": "launcher", "ref": "v1.0.0", "path": "src/a.rs"}).text
    )
    assert (
        "src/a.rs:2:"
        in toolbox.run("grep_source", {"repo": "launcher", "ref": "HEAD", "pattern": "stage"}).text
    )
    assert toolbox.run("read_source", {"repo": "launcher", "ref": "--help", "path": "x"}).is_error
    assert toolbox.run("read_source", {"repo": "nope", "ref": "HEAD", "path": "x"}).is_error
    assert "no commits" in toolbox.run("git_log", {"repo": "launcher", "base": "v1.0.0", "head": "HEAD"}).text
    assert toolbox.run("submit_diagnosis", dict(DIAGNOSIS)).diagnosis is not None


def test_logs_request():
    text = render.logs_request("<@123>")
    assert text.startswith("Hey <@123>!")
    assert "**Send Logs to Developers**" in text
    assert "<https://f1.swofty.net/using/send-logs>" in text
    assert len(text) <= 2000
    assert render.logs_request(None).startswith("Hey! Please press")
