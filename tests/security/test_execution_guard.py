from __future__ import annotations

from pathlib import Path

from openjarvis.security import execution_guard


def test_disabled_guard_blocks_before_any_defender_call(tmp_path: Path, monkeypatch) -> None:
    target = tmp_path / "safe.exe"
    target.write_bytes(b"test executable")
    monkeypatch.delenv("OPENJARVIS_ENABLE_EXECUTION_GUARD", raising=False)

    report = execution_guard.preflight_open(target, action_type="local_app_open")

    assert report.allowed is False
    assert report.decision == "blocked"
    assert report.defender_scan == "not-run"
    assert "not enabled" in report.summary


def test_enabled_executable_requires_scan_and_trust(tmp_path: Path, monkeypatch) -> None:
    target = tmp_path / "tool.exe"
    target.write_bytes(b"test executable")
    mpcmdrun = tmp_path / "MpCmdRun.exe"
    mpcmdrun.write_bytes(b"defender")
    calls: list[list[str]] = []

    monkeypatch.setenv("OPENJARVIS_ENABLE_EXECUTION_GUARD", "1")
    monkeypatch.setattr(execution_guard.os, "name", "nt")
    monkeypatch.setattr(execution_guard, "_find_mpcmdrun", lambda: mpcmdrun)

    def fake_run(command: list[str], timeout: int) -> tuple[bool, str]:
        calls.append(command)
        return True, "verified"

    monkeypatch.setattr(execution_guard, "_run_defender", fake_run)

    report = execution_guard.preflight_open(target, action_type="local_app_open")

    assert report.allowed is True
    assert report.defender_scan == "clean"
    assert report.reputation == "Defender trusted"
    assert calls == [
        [str(mpcmdrun), "-Scan", "-ScanType", "3", "-File", str(target)],
        [str(mpcmdrun), "-TrustCheck", "-File", str(target)],
    ]


def test_scan_failure_blocks_without_running_trust_check(tmp_path: Path, monkeypatch) -> None:
    target = tmp_path / "tool.exe"
    target.write_bytes(b"test executable")
    mpcmdrun = tmp_path / "MpCmdRun.exe"
    mpcmdrun.write_bytes(b"defender")
    calls: list[list[str]] = []

    monkeypatch.setenv("OPENJARVIS_ENABLE_EXECUTION_GUARD", "1")
    monkeypatch.setattr(execution_guard.os, "name", "nt")
    monkeypatch.setattr(execution_guard, "_find_mpcmdrun", lambda: mpcmdrun)

    def failed_scan(command: list[str], timeout: int) -> tuple[bool, str]:
        calls.append(command)
        return False, "Defender returned code 2"

    monkeypatch.setattr(execution_guard, "_run_defender", failed_scan)

    report = execution_guard.preflight_open(target, action_type="local_app_open")

    assert report.allowed is False
    assert report.defender_scan == "failed"
    assert report.reputation == "not-run"
    assert len(calls) == 1
    assert "scan did not verify" in report.summary


def test_document_needs_successful_scan_but_not_executable_trust(tmp_path: Path, monkeypatch) -> None:
    target = tmp_path / "notes.pdf"
    target.write_bytes(b"not a real pdf")
    mpcmdrun = tmp_path / "MpCmdRun.exe"
    mpcmdrun.write_bytes(b"defender")
    calls: list[list[str]] = []

    monkeypatch.setenv("OPENJARVIS_ENABLE_EXECUTION_GUARD", "1")
    monkeypatch.setattr(execution_guard.os, "name", "nt")
    monkeypatch.setattr(execution_guard, "_find_mpcmdrun", lambda: mpcmdrun)

    def successful_scan(command: list[str], timeout: int) -> tuple[bool, str]:
        calls.append(command)
        return True, "verified"

    monkeypatch.setattr(execution_guard, "_run_defender", successful_scan)

    report = execution_guard.preflight_open(target, action_type="local_document_open")

    assert report.allowed is True
    assert report.reputation == "not-applicable-to-document"
    assert len(calls) == 1
    assert "-Scan" in calls[0]


def test_windows_security_status_is_read_only_on_unsupported_platform(monkeypatch) -> None:
    monkeypatch.setattr(execution_guard.os, "name", "posix")

    status = execution_guard.windows_security_status()

    assert status["platform"] == "unsupported"
    assert status["smart_screen"].startswith("os-managed")
