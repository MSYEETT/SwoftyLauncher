from datetime import datetime, timezone

from swofty_triage import releases
from swofty_triage.bot import since_time
from swofty_triage.releases import Asset, Commit, Release

BASE = "https://github.com/Swofty-Developments/SwoftyLauncher/releases/download/v1.1.0/"
RELEASE = Release(
    tag="v1.1.0",
    name="v1.1.0",
    url="https://github.com/Swofty-Developments/SwoftyLauncher/releases/tag/v1.1.0",
    published_at="2026-10-20T13:04:05Z",
    assets=(
        Asset("SHA256SUMS", BASE + "SHA256SUMS", 217),
        Asset("swofty-launcher-v1.1.0-linux-x86_64.tar.gz", BASE + "linux.tar.gz", 1),
        Asset("swofty-launcher-v1.1.0-windows-x86_64.zip", BASE + "windows.zip", 1),
        Asset("swofty.career-editor.zip", BASE + "swofty.career-editor.zip", 1),
        Asset("unknown.zip", BASE + "unknown.zip", 1),
    ),
)
COMMITS = [
    Commit("a", "perf: faster play and auto-start steam (1.1.0) (#179)", "Swofty-Developments", "Yakov"),
    Commit("b", "chore: prepare the 1.1.0 release", "Swofty-Developments", "Jacob"),
    Commit("c", "fix(core): accept numbers as track override values (#178)", "someone", "Someone"),
    Commit("d", "Merge pull request #12 from x/y", None, "Outside Name"),
    Commit("e", "build(deps): bump zip (#180)", "dependabot[bot]", "dependabot"),
]


def test_version_word():
    assert releases.version_word("v1.1.0") == "Major Version"
    assert releases.version_word("v1.0.13") == "Minor Version"
    assert releases.version_word("v2.0.0") == "Major Version"


def test_pr_numbers():
    assert [c.pr for c in COMMITS] == [179, None, 178, 12, 180]


def test_contributors_by_commit_count_without_bots():
    assert releases.contributors(COMMITS) == [
        ("Swofty-Developments", "https://github.com/Swofty-Developments/"),
        ("someone", "https://github.com/someone/"),
        ("Outside Name", ""),
    ]


def test_fallback_lines_use_pr_titles():
    assert releases.fallback_lines(COMMITS) == [
        {"text": "Faster play and auto-start steam", "prs": [179]},
        {"text": "Accept numbers as track override values", "prs": [178]},
        {"text": "Merge pull request #12 from x/y", "prs": [12]},
    ]


def test_previous_release():
    older = Release("v1.0.13", "v1.0.13", "u", "2026-10-20T12:16:30Z", ())
    oldest = Release("v1.0.12", "v1.0.12", "u", "2026-10-20T11:13:07Z", ())
    assert releases.previous(RELEASE, [RELEASE, oldest, older]) == older
    assert releases.previous(oldest, [RELEASE, oldest, older]) is None


def test_announcement_matches_the_moj_layout():
    text = releases.announcement(
        "Swofty Launcher for F1 25",
        "Swofty-Developments/SwoftyLauncher",
        RELEASE,
        [{"text": "Made Play faster", "prs": [179]}, {"text": "Updated the docs", "prs": []}],
        {"swofty.career-editor": "Career Mode Editor"},
        [("Swofty-Developments", "https://github.com/Swofty-Developments/")],
        "https://f1.swofty.net/releases/1.1.0",
    )
    assert text.splitlines()[:5] == [
        "**Swofty Launcher for F1 25 Release v1.1.0** :tada:",
        "",
        "Today we announce the release of Major Version 1.1.0, available on our "
        "[GitHub](<https://github.com/Swofty-Developments/SwoftyLauncher/releases/tag/v1.1.0>) and documented "
        "on our [Docs](<https://f1.swofty.net/releases/1.1.0>).",
        "",
        "Changelog:",
    ]
    assert (
        "- Made Play faster (PR [#179](<https://github.com/Swofty-Developments/SwoftyLauncher/pull/179>))"
        in text
    )
    assert "- Updated the docs\n" in text
    downloads = text.split("Downloads:\n")[1].split("\n\n")[0].splitlines()
    assert downloads == [
        f"- [Windows](<{BASE}windows.zip>)",
        f"- [Linux and Steam Deck](<{BASE}linux.tar.gz>)",
        f"- [Career Mode Editor addon](<{BASE}swofty.career-editor.zip>)",
        f"- [unknown.zip](<{BASE}unknown.zip>)",
    ]
    assert text.endswith("Contributors:\n- [Swofty-Developments](<https://github.com/Swofty-Developments/>)")
    assert "SHA256SUMS" not in text


def test_announcement_without_docs():
    text = releases.announcement("P", "o/r", RELEASE, [], {}, [], None)
    assert "available on our [GitHub](<" + RELEASE.url + ">)." in text
    assert "Contributors" not in text


def test_since_time():
    assert since_time("2026-10-20T20:00:00Z") == datetime(2026, 10, 20, 20, tzinfo=timezone.utc)
    assert since_time("").tzinfo is not None
