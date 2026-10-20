import io
import zipfile

import pytest

from swofty_triage import report

CONTENT = (
    "**Logs from `nicov.r11`**\n"
    "Launcher 1.1.0 · Windows 11 26H2 (10.0.26300.9711) · F1 25 1.0.141.7865 (Steam build 25251167)\n"
    ">>> I cant load into my team career\nsecond line"
)


def make_zip(entries: dict[str, bytes]) -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        for name, data in entries.items():
            z.writestr(name, data)
    return buf.getvalue()


def test_header_with_message():
    h = report.parse_header(CONTENT)
    assert h is not None
    assert h.name == "nicov.r11"
    assert h.launcher == "1.1.0"
    assert h.release_tag == "v1.1.0"
    assert h.os == "Windows 11 26H2 (10.0.26300.9711)"
    assert h.game == "1.0.141.7865 (Steam build 25251167)"
    assert h.message == "I cant load into my team career\nsecond line"
    assert (h.part, h.total) == (1, 1)


def test_header_without_message_and_with_parts():
    h = report.parse_header(
        "**Logs from `game doesn't launch`** · part 2/3\nLauncher 1.1.0-dev · Windows · F1 25 not found"
    )
    assert h is not None
    assert h.name == "game doesn't launch"
    assert (h.part, h.total) == (2, 3)
    assert h.message == ""
    assert h.release_tag is None


@pytest.mark.parametrize("content", ["hello", "**Logs from `x`**\nLauncher 1.0 · Windows", ""])
def test_header_rejects_other_messages(content):
    assert report.parse_header(content) is None


def test_attachment_names():
    assert report.parse_attachment("swofty-logs-nicov.r11-20261020-222044.zip") == (
        "swofty-logs-nicov.r11-20261020-222044",
        1,
    )
    assert report.parse_attachment("swofty-logs-a_b-20261020-222044-part3.zip") == (
        "swofty-logs-a_b-20261020-222044",
        3,
    )
    assert report.parse_attachment("../evil.zip") is None
    assert report.parse_attachment("swofty-logs-x-1-2.zip") is None


def test_extract_skips_unsafe_entries(tmp_path):
    data = make_zip({"summary.txt": b"ok", "../escape.txt": b"no", "/abs.txt": b"no", "a/../../b": b"no"})
    skipped = report.extract(data, tmp_path / "logs")
    assert sorted(skipped) == ["../escape.txt", "/abs.txt", "a/../../b"]
    assert (tmp_path / "logs" / "summary.txt").read_text() == "ok"
    assert not (tmp_path / "escape.txt").exists()


def test_extract_rejects_bombs(tmp_path, monkeypatch):
    monkeypatch.setattr(report, "MAX_ENTRY", 1000)
    with pytest.raises(report.BadReport):
        report.extract(make_zip({"big.log": b"x" * 5000}), tmp_path / "logs")


def test_extract_rejects_non_zip(tmp_path):
    with pytest.raises(report.BadReport):
        report.extract(b"not a zip", tmp_path / "logs")


LOADER = """\
2026-10-20 18:55:48.498 [15556] [crash] crash log armed: fatal exceptions
2026-10-20 18:58:22.330 [26908] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 26908, 153 s after load, at F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4
2026-10-20 18:58:22.330 [26908] [crash]   stack:
2026-10-20 18:58:22.330 [26908] [crash]   #0  F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4
2026-10-20 18:58:22.330 [26908] [crash]   #1  F1_25.exe+0x395fa23 .xtls Ne_ReportMemoryError+0x343
2026-10-20 18:58:22.330 [26908] [crash]   #2  F1_25.exe+0x39658d8 .xtls fn base+0x39657d0+0x108
2026-10-20 18:58:22.330 [26908] [crash]   #3  F1_25.exe+0x3963f5f .xtls Ne_Heap_Free+0xdf
2026-10-20 18:58:22.330 [26908] [crash]   #4  F1_25.exe+0xa72744d .data1 Ne_RobinHoodU8_IncreaseSize_Body+0x1dd
2026-10-20 18:58:22.330 [26908] [crash]   #5  F1_25.exe+0x2e909f3 .xtls Sess_Veh_UpdateWheelMap+0x493
2026-10-20 18:58:22.330 [26908] [crash]   #6  KERNEL32.DLL+0x2dda7
"""


def test_signature_and_briefing(tmp_path):
    root = tmp_path / "logs"
    session = root / "sessions" / "20261020-165545"
    session.mkdir(parents=True)
    (session / "loader.log").write_text(LOADER)
    (session / "exit_code").write_text("-1")
    (session / "game_start").write_text("100.0")
    (session / "game_end").write_text("257.5")
    (root / "summary.txt").write_text("Discord username x")
    assert (
        report.signature(root)
        == "ACCESS_VIOLATION Ne_RobinHoodU8_IncreaseSize_Body < Sess_Veh_UpdateWheelMap"
    )
    brief = report.briefing(root, report.parse_header(CONTENT), "now")
    assert "exit_code=-1" in brief
    assert "ran 158s" in brief
    assert "Sess_Veh_UpdateWheelMap" in brief
    assert "crash log armed" not in brief.split("[crash] lines")[1].split("last lines")[0]
