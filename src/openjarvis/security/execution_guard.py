"""Fail-safe execution guard for controlled Windows launch actions.

This module is deliberately *not* an antivirus engine and never claims to replace
Microsoft Defender or SmartScreen.  It is a local preflight for actions proposed
by the cloud agent: app and document launches do not occur until a locally
verified Microsoft Defender scan succeeds.  A missing scanner, timeout, or
ambiguous result is a denial, not a bypass.

The guard never uploads file contents.  It computes a SHA-256 locally for audit
and report correlation.  Windows-wide protection while OpenJarvis is closed
remains the responsibility of Windows Defender/SmartScreen or an installed
endpoint security product.
"""

from __future__ import annotations

import ctypes
import hashlib
import os
import re
import subprocess
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

_GUARD_ENV = "OPENJARVIS_ENABLE_EXECUTION_GUARD"
_SCAN_TIMEOUT_SECONDS = 120
_TRUST_TIMEOUT_SECONDS = 20
_MAX_DIAGNOSTIC_CHARS = 320
_EXECUTABLE_SUFFIXES = frozenset({".exe", ".com", ".scr", ".msi"})


@dataclass(frozen=True)
class ExecutionSecurityReport:
    """A redacted, serializable outcome for one local launch preflight."""

    allowed: bool
    decision: str
    summary: str
    file_name: str
    sha256: str
    defender_scan: str
    reputation: str
    source_zone: str
    details: tuple[str, ...]

    def as_dict(self) -> dict[str, Any]:
        return asdict(self)


def execution_guard_enabled() -> bool:
    """Return whether the secure desktop explicitly enables this guard."""

    return os.getenv(_GUARD_ENV, "").strip() == "1"


def _truncate(value: str) -> str:
    compact = " ".join(value.split())
    return compact[:_MAX_DIAGNOSTIC_CHARS]


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1_048_576):
            digest.update(chunk)
    return digest.hexdigest()


def _zone_identifier(path: Path) -> str:
    """Read only the local Mark-of-the-Web zone, if it exists.

    The alternate data stream is local metadata and is never sent to a cloud
    provider.  Failures intentionally produce ``unknown`` rather than a clean
    result because ADS availability differs by volume and file system.
    """

    if os.name != "nt":
        return "unsupported"
    try:
        raw = Path(f"{path}:Zone.Identifier").read_text(encoding="utf-8", errors="replace")
    except OSError:
        return "unknown"
    matched = re.search(r"(?im)^ZoneId\s*=\s*(\d+)$", raw)
    if not matched:
        return "unknown"
    zone_id = matched.group(1)
    return {
        "0": "local-machine",
        "1": "local-intranet",
        "2": "trusted-site",
        "3": "internet",
        "4": "restricted",
    }.get(zone_id, f"zone-{zone_id}")


def _find_mpcmdrun() -> Path | None:
    """Locate the Defender utility without accepting a model-controlled path."""

    if os.name != "nt":
        return None
    program_data = Path(os.environ.get("ProgramData", r"C:\ProgramData"))
    candidates: list[Path] = []
    platform_root = program_data / "Microsoft" / "Windows Defender" / "Platform"
    try:
        candidates.extend(
            sorted(
                platform_root.glob("*/MpCmdRun.exe"),
                key=lambda item: item.parent.name,
                reverse=True,
            )
        )
    except OSError:
        pass
    candidates.append(Path(r"C:\Program Files\Windows Defender\MpCmdRun.exe"))
    for candidate in candidates:
        try:
            if candidate.is_file():
                return candidate.resolve()
        except OSError:
            continue
    return None


def _run_defender(command: list[str], timeout_seconds: int) -> tuple[bool, str]:
    """Run a fixed Defender command without a shell or arbitrary arguments."""

    try:
        completed = subprocess.run(
            command,
            check=False,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
            shell=False,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
    except subprocess.TimeoutExpired:
        return False, "Defender verification timed out."
    except OSError as exc:
        return False, f"Defender could not start: {_truncate(str(exc))}"
    output = _truncate(f"{completed.stdout}\n{completed.stderr}")
    if completed.returncode != 0:
        suffix = f" ({output})" if output else ""
        return False, f"Defender returned code {completed.returncode}{suffix}"
    return True, "verified"


def _blocked_report(
    *,
    file_name: str,
    sha256: str,
    scan: str,
    reputation: str,
    source_zone: str,
    reason: str,
) -> ExecutionSecurityReport:
    return ExecutionSecurityReport(
        allowed=False,
        decision="blocked",
        summary=f"Blocked before opening: {reason}",
        file_name=file_name,
        sha256=sha256,
        defender_scan=scan,
        reputation=reputation,
        source_zone=source_zone,
        details=(reason,),
    )


def preflight_open(path: Path, *, action_type: str) -> ExecutionSecurityReport:
    """Run the mandatory local checks before a controlled launch.

    A controlled action is allowed only after a custom Microsoft Defender scan
    completes successfully.  Executables additionally need a successful
    Defender trust/reputation check.  This conservative policy makes unknown
    tooling unavailable through OpenJarvis rather than silently falling back.
    """

    file_name = path.name
    source_zone = _zone_identifier(path)
    try:
        digest = _sha256(path)
    except OSError as exc:
        return _blocked_report(
            file_name=file_name,
            sha256="",
            scan="not-run",
            reputation="not-run",
            source_zone=source_zone,
            reason=f"The file could not be read safely: {_truncate(str(exc))}",
        )

    if not execution_guard_enabled():
        return _blocked_report(
            file_name=file_name,
            sha256=digest,
            scan="not-run",
            reputation="not-run",
            source_zone=source_zone,
            reason="The mandatory execution guard is not enabled.",
        )
    if os.name != "nt":
        return _blocked_report(
            file_name=file_name,
            sha256=digest,
            scan="unsupported",
            reputation="unsupported",
            source_zone=source_zone,
            reason="Windows Defender verification is available only in the Windows desktop build.",
        )
    if action_type not in {"local_app_open", "local_document_open"}:
        return _blocked_report(
            file_name=file_name,
            sha256=digest,
            scan="not-run",
            reputation="not-run",
            source_zone=source_zone,
            reason="This action is not eligible for an execution preflight.",
        )

    defender = _find_mpcmdrun()
    if defender is None:
        return _blocked_report(
            file_name=file_name,
            sha256=digest,
            scan="unavailable",
            reputation="not-run",
            source_zone=source_zone,
            reason="Microsoft Defender command-line verification is unavailable.",
        )

    scan_ok, scan_detail = _run_defender(
        [str(defender), "-Scan", "-ScanType", "3", "-File", str(path)],
        _SCAN_TIMEOUT_SECONDS,
    )
    if not scan_ok:
        return _blocked_report(
            file_name=file_name,
            sha256=digest,
            scan="failed",
            reputation="not-run",
            source_zone=source_zone,
            reason=f"Microsoft Defender scan did not verify the file: {scan_detail}",
        )

    if path.suffix.casefold() in _EXECUTABLE_SUFFIXES:
        trust_ok, trust_detail = _run_defender(
            [str(defender), "-TrustCheck", "-File", str(path)],
            _TRUST_TIMEOUT_SECONDS,
        )
        if not trust_ok:
            return _blocked_report(
                file_name=file_name,
                sha256=digest,
                scan="clean",
                reputation="untrusted-or-unavailable",
                source_zone=source_zone,
                reason=(
                    "Microsoft Defender could not establish a trusted reputation for the executable: "
                    f"{trust_detail}"
                ),
            )
        reputation = "Defender trusted"
    else:
        reputation = "not-applicable-to-document"

    details = ["Microsoft Defender custom scan completed."]
    if source_zone in {"internet", "restricted"}:
        details.append(f"The file carries a {source_zone} origin marker.")
    return ExecutionSecurityReport(
        allowed=True,
        decision="allowed",
        summary="Security preflight completed; opening is allowed.",
        file_name=file_name,
        sha256=digest,
        defender_scan="clean",
        reputation=reputation,
        source_zone=source_zone,
        details=tuple(details),
    )


def windows_security_status() -> dict[str, Any]:
    """Return an OS-local, read-only Security Center health snapshot.

    SmartScreen does not offer a stable per-file status API to arbitrary apps;
    it is reported honestly as OS-managed.  No setting is modified, and no
    security event, file content, or credential is sent to the cloud.
    """

    base: dict[str, Any] = {
        "execution_guard": execution_guard_enabled(),
        "platform": "windows" if os.name == "nt" else "unsupported",
        "smart_screen": "os-managed; status is not independently queryable",
        "defender_health": "unknown",
        "details": [],
    }
    if os.name != "nt":
        base["details"] = ["Windows Security Center is available only on Windows."]
        return base
    try:
        health = ctypes.c_uint(0)
        # WSC_SECURITY_PROVIDER_ANTIVIRUS = 4.
        result = ctypes.windll.wscapi.WscGetSecurityProviderHealth(4, ctypes.byref(health))
        health_name = {
            0: "good",
            1: "not-monitored",
            2: "poor",
            3: "snoozed",
        }.get(health.value, "unknown")
        base["defender_health"] = health_name if result == 0 else "unavailable"
        base["details"] = [
            "Read-only Windows Security Center antivirus health.",
            "Microsoft Defender/SmartScreen settings are never modified by OpenJarvis.",
        ]
    except (AttributeError, OSError) as exc:
        base["defender_health"] = "unavailable"
        base["details"] = [f"Windows Security Center query failed: {_truncate(str(exc))}"]
    return base


__all__ = [
    "ExecutionSecurityReport",
    "execution_guard_enabled",
    "preflight_open",
    "windows_security_status",
]
