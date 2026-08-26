"""Controlled local operations for the privacy-first desktop profile.

This module deliberately separates *proposal* from *execution*: the cloud model
can only queue an action.  The local desktop UI must approve it through the
local approval endpoint before :func:`execute_approved_local_action` performs
one bounded side effect.
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import time
import uuid
from pathlib import Path
from typing import Any

from openjarvis.core.paths import get_config_dir
from openjarvis.core.registry import ToolRegistry
from openjarvis.core.types import ToolResult
from openjarvis.security.file_policy import is_sensitive_file
from openjarvis.security.output_safety import sanitize_model_output
from openjarvis.tools._stubs import BaseTool, ToolSpec
from openjarvis.tools.approval_store import TIER_HIGH, ApprovalStore

_CONTROLLED_ACTIONS_ENV = "OPENJARVIS_ENABLE_CONTROLLED_LOCAL_ACTIONS"
_CONTROLLED_FOLDERS_FILE = "controlled-local-folders.json"
_MAX_APPROVED_FOLDERS = 8
_MAX_FILE_BYTES = 1_048_576
_MAX_READ_BYTES = 262_144
_MAX_DIRECTORY_ENTRIES = 200
_APPROVAL_TTL_HOURS = 1
_BLOCKED_DIRECTORY_NAMES = frozenset(
    {
        ".aws",
        ".gnupg",
        ".openjarvis",
        ".ssh",
        "appdata",
        "program files",
        "program files (x86)",
        "system32",
        "windows",
    }
)
_PROTECTED_WORKSPACE_DIRECTORY_NAMES = frozenset(
    {
        ".agents",
        ".claude",
        ".codex",
        ".cursor",
        ".git",
        ".hg",
        ".idea",
        ".svn",
        ".vscode",
    }
)
_PROTECTED_WORKSPACE_FILE_NAMES = frozenset(
    {"agents.md", "claude.md", "gemini.md", "instructions.md", "mcp.json"}
)

_BLOCKED_APPLICATION_NAMES = frozenset(
    {
        "bash.exe",
        "cmd.exe",
        "cscript.exe",
        "git.exe",
        "installutil.exe",
        "mshta.exe",
        "msiexec.exe",
        "node.exe",
        "powershell.exe",
        "pwsh.exe",
        "python.exe",
        "pythonw.exe",
        "regsvr32.exe",
        "rundll32.exe",
        "wscript.exe",
    }
)
_OPENABLE_DOCUMENT_SUFFIXES = frozenset(
    {
        ".csv",
        ".docx",
        ".json",
        ".md",
        ".pdf",
        ".pptx",
        ".rtf",
        ".tsv",
        ".txt",
        ".xlsx",
    }
)
_BLOCKED_SUFFIXES = frozenset(
    {
        ".bat",
        ".cmd",
        ".com",
        ".dll",
        ".exe",
        ".jar",
        ".js",
        ".msi",
        ".ps1",
        ".py",
        ".scr",
        ".sys",
        ".vbs",
    }
)


def _enabled() -> bool:
    return os.getenv(_CONTROLLED_ACTIONS_ENV, "").strip() == "1"


def _workspace() -> Path:
    configured = os.getenv("OPENJARVIS_CONTROLLED_WORKSPACE", "").strip()
    if configured:
        root = Path(configured).expanduser()
    else:
        root = Path.home() / "OpenJarvis-Workspace"
    root.mkdir(parents=True, exist_ok=True)
    return root.resolve()


def _controlled_folders_path() -> Path:
    """Return the user-managed policy file without storing file contents."""

    return get_config_dir() / _CONTROLLED_FOLDERS_FILE


def _is_safe_additional_root(candidate: Path) -> bool:
    """Reject broad, private, or system roots even if a local file is edited."""

    if not candidate.is_absolute() or not candidate.is_dir():
        return False
    if candidate.parent == candidate or candidate == Path.home().resolve():
        return False
    if candidate == get_config_dir().resolve():
        return False
    lowered_parts = {part.casefold() for part in candidate.parts}
    return not bool(lowered_parts & _BLOCKED_DIRECTORY_NAMES)


def _authorized_roots() -> tuple[Path, ...]:
    """Return the workspace plus the small user-approved external folder set."""

    roots = [_workspace()]
    try:
        raw = json.loads(_controlled_folders_path().read_text(encoding="utf-8"))
        folders = raw.get("folders", []) if isinstance(raw, dict) else []
    except (OSError, ValueError, json.JSONDecodeError):
        folders = []

    for folder in folders[:_MAX_APPROVED_FOLDERS]:
        if not isinstance(folder, str):
            continue
        try:
            candidate = Path(folder).expanduser().resolve()
        except OSError:
            continue
        if _is_safe_additional_root(candidate) and candidate not in roots:
            roots.append(candidate)
    return tuple(roots)


def list_authorized_folders() -> list[str]:
    """Expose only canonical root paths selected in local desktop settings."""

    return [str(root) for root in _authorized_roots()]


def replace_authorized_folders(raw_folders: list[str]) -> list[str]:
    """Save a user-selected external folder allowlist after strict validation.

    This function is intended for the local desktop settings command, not for
    model tools. A model can use a folder only after the person adds it in the
    Settings page; it can never grant itself a new root.
    """

    if len(raw_folders) > _MAX_APPROVED_FOLDERS:
        raise ValueError(
            f"At most {_MAX_APPROVED_FOLDERS} external folders are allowed."
        )
    workspace = _workspace()
    roots: list[Path] = []
    for raw in raw_folders:
        if not isinstance(raw, str) or not raw.strip():
            raise ValueError(
                "Each approved folder must be an absolute existing directory."
            )
        candidate = Path(raw).expanduser()
        if not candidate.is_absolute():
            raise ValueError("Each approved folder must use an absolute path.")
        resolved = candidate.resolve()
        if resolved == workspace:
            continue
        if not _is_safe_additional_root(resolved):
            raise ValueError(
                "The selected folder is too broad, private, or a system location."
            )
        if resolved not in roots:
            roots.append(resolved)
    path = _controlled_folders_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps({"version": 1, "folders": [str(root) for root in roots]}, indent=2)
        + "\n",
        encoding="utf-8",
    )
    return [str(root) for root in roots]


def _is_protected_workspace_path(candidate: Path, root: Path) -> bool:
    """Return whether a path is VCS or agent-control metadata under one root.

    These files can modify repository state, inject instructions into an agent,
    or carry credentials. The controlled desktop channel never exposes them to
    the cloud model, even for a read-only request.
    """

    try:
        relative = candidate.relative_to(root)
    except ValueError:
        return True
    lowered_parts = {part.casefold() for part in relative.parts}
    return bool(lowered_parts & _PROTECTED_WORKSPACE_DIRECTORY_NAMES) or (
        candidate.name.casefold() in _PROTECTED_WORKSPACE_FILE_NAMES
    )


def _resolve_workspace_path(
    raw_path: str, *, allow_workspace_root: bool = False
) -> Path:
    """Resolve one path within the workspace or a user-approved external root."""

    if not raw_path or not isinstance(raw_path, str):
        raise ValueError("A file or directory path is required.")
    roots = _authorized_roots()
    workspace = roots[0]
    requested = Path(raw_path).expanduser()
    candidate = (
        requested.resolve()
        if requested.is_absolute()
        else (workspace / requested).resolve()
    )
    root = next(
        (
            allowed
            for allowed in roots
            if candidate == allowed or candidate.is_relative_to(allowed)
        ),
        None,
    )
    if root is None:
        raise ValueError(
            "Access is limited to the workspace and user-approved folders."
        )
    if candidate == root and not allow_workspace_root:
        raise ValueError("An approved folder root itself cannot be modified.")
    if _is_protected_workspace_path(candidate, root):
        raise ValueError(
            "Version-control and agent-configuration metadata cannot be accessed."
        )
    if is_sensitive_file(candidate):
        raise ValueError("Sensitive files and credential material cannot be accessed.")
    if candidate.suffix.lower() in _BLOCKED_SUFFIXES:
        raise ValueError(
            "Executable, script, library, and installer files cannot be modified."
        )
    return candidate


def _is_blocked_application_path(path: Path) -> bool:
    """Return whether an executable is a shell, installer, or script host."""

    return path.name.casefold() in _BLOCKED_APPLICATION_NAMES


def _resolve_application_path(raw_path: str) -> Path:
    """Validate one installed Windows executable without accepting arguments.

    The model can nominate any installed ``.exe`` by absolute path, but cannot
    append flags, shell metacharacters, scripts, or installers. The desktop UI
    still displays and requires approval of the exact resolved executable.
    """

    if os.name != "nt":
        raise ValueError(
            "Application control is available only in the Windows desktop build."
        )
    if not raw_path or not isinstance(raw_path, str):
        raise ValueError(
            "An absolute path to an installed .exe application is required."
        )
    candidate = Path(raw_path).expanduser()
    if not candidate.is_absolute() or candidate.suffix.lower() != ".exe":
        raise ValueError("Only an absolute path to one .exe application is accepted.")
    resolved = candidate.resolve()
    if not resolved.is_file():
        raise ValueError("The selected application executable does not exist.")
    if _is_blocked_application_path(resolved):
        raise ValueError(
            "Command shells, script hosts, installers, and administrative "
            "utilities cannot be launched by the controlled application channel."
        )
    return resolved


def _process_file() -> Path:
    return get_config_dir() / "controlled-local-processes.json"


def _load_processes() -> dict[str, list[int]]:
    path = _process_file()
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        if isinstance(data, dict):
            return {
                str(app): [int(pid) for pid in pids if isinstance(pid, int)]
                for app, pids in data.items()
                if isinstance(pids, list)
            }
    except (OSError, json.JSONDecodeError, ValueError):
        pass
    return {}


def _save_processes(data: dict[str, list[int]]) -> None:
    path = _process_file()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, sort_keys=True), encoding="utf-8")


def _audit(action: str, *, success: bool, **metadata: Any) -> None:
    """Append an audit event without storing file content or cloud credentials."""

    record = {
        "timestamp": int(time.time()),
        "action": action,
        "success": success,
        **metadata,
    }
    path = get_config_dir() / "controlled-local-audit.jsonl"
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(record, sort_keys=True, default=str) + "\n")


def _queue_action(
    action_type: str, description: str, payload: dict[str, Any]
) -> ToolResult:
    store = ApprovalStore()
    action = store.queue_action(
        action_type=action_type,
        description=description,
        payload=payload,
        permission_key=f"{action_type}:always_ask",
        tier=TIER_HIGH,
        ttl_hours=_APPROVAL_TTL_HOURS,
    )
    return ToolResult(
        tool_name="controlled_local_action",
        success=True,
        content=(
            f"Action {action.id} is queued for local approval. It will expire in "
            f"{_APPROVAL_TTL_HOURS} hour and cannot execute until approved "
            "in the desktop UI."
        ),
        metadata={
            "action_id": action.id,
            "status": action.status,
            "requires_ui_approval": True,
        },
    )


@ToolRegistry.register("controlled_workspace_read")
class ControlledWorkspaceReadTool(BaseTool):
    """Read a bounded, non-sensitive text file only inside the dedicated workspace."""

    tool_id = "controlled_workspace_read"

    @property
    def spec(self) -> ToolSpec:
        return ToolSpec(
            name="controlled_workspace_read",
            description=(
                "Read a non-sensitive text file inside the approved "
                "OpenJarvis workspace."
            ),
            parameters={
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
            },
            category="filesystem",
            required_capabilities=["file:read"],
        )

    def execute(self, **params: Any) -> ToolResult:
        if not _enabled():
            return ToolResult(
                tool_name=self.tool_id,
                content="Controlled local actions are disabled.",
                success=False,
            )
        try:
            path = _resolve_workspace_path(str(params.get("path", "")))
            if not path.is_file():
                raise ValueError("Only existing regular files can be read.")
            if path.stat().st_size > _MAX_READ_BYTES:
                raise ValueError(
                    f"File is too large to read safely (max {_MAX_READ_BYTES} bytes)."
                )
            raw_content = path.read_text(encoding="utf-8")
            content = sanitize_model_output(raw_content, max_chars=_MAX_READ_BYTES)
            _audit(
                "workspace_read",
                success=True,
                path=str(path),
                size_bytes=len(raw_content.encode("utf-8")),
                content_redacted=content != raw_content,
            )
            return ToolResult(
                tool_name=self.tool_id,
                content=content,
                success=True,
                metadata={"path": str(path)},
            )
        except (OSError, UnicodeDecodeError, ValueError) as exc:
            _audit("workspace_read", success=False, error=str(exc))
            return ToolResult(
                tool_name=self.tool_id, content=f"Read denied: {exc}", success=False
            )


@ToolRegistry.register("controlled_workspace_list")
class ControlledWorkspaceListTool(BaseTool):
    """List bounded, non-sensitive metadata inside a user-approved folder."""

    tool_id = "controlled_workspace_list"

    @property
    def spec(self) -> ToolSpec:
        return ToolSpec(
            name=self.tool_id,
            description=(
                "List up to 200 non-sensitive entries in the approved workspace "
                "or a user-approved external folder."
            ),
            parameters={
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
            },
            category="filesystem",
            required_capabilities=["file:read"],
        )

    def execute(self, **params: Any) -> ToolResult:
        if not _enabled():
            return ToolResult(
                tool_name=self.tool_id,
                content="Controlled local actions are disabled.",
                success=False,
            )
        try:
            path = _resolve_workspace_path(
                str(params.get("path", "")), allow_workspace_root=True
            )
            if not path.is_dir():
                raise ValueError("Only an existing approved directory can be listed.")
            entries: list[dict[str, Any]] = []
            for child in sorted(path.iterdir(), key=lambda item: item.name.casefold()):
                if len(entries) >= _MAX_DIRECTORY_ENTRIES:
                    break
                if (
                    _is_protected_workspace_path(child, path)
                    or is_sensitive_file(child)
                    or child.suffix.lower() in _BLOCKED_SUFFIXES
                ):
                    continue
                entries.append({"name": child.name, "directory": child.is_dir()})
            _audit("workspace_list", success=True, path=str(path), count=len(entries))
            return ToolResult(
                tool_name=self.tool_id,
                content=json.dumps(entries, ensure_ascii=False),
                success=True,
                metadata={"path": str(path), "count": len(entries)},
            )
        except (OSError, ValueError) as exc:
            _audit("workspace_list", success=False, error=str(exc))
            return ToolResult(
                tool_name=self.tool_id,
                content=(
                    f"List denied: {sanitize_model_output(str(exc), max_chars=512)}"
                ),
                success=False,
            )


@ToolRegistry.register("controlled_local_action")
class ControlledLocalActionTool(BaseTool):
    """Queue a bounded local side effect for a mandatory desktop approval."""

    tool_id = "controlled_local_action"

    @property
    def spec(self) -> ToolSpec:
        return ToolSpec(
            name="controlled_local_action",
            description=(
                "Queue a high-risk local action for explicit approval "
                "in the desktop UI. The action never executes directly from the model."
            ),
            parameters={
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": [
                            "write_text",
                            "append_text",
                            "create_directory",
                            "open_app",
                            "open_document",
                            "close_app",
                        ],
                    },
                    "path": {"type": "string"},
                    "content": {"type": "string"},
                    "application": {
                        "type": "string",
                        "description": (
                            "Absolute path to one installed Windows .exe application."
                        ),
                    },
                    "pid": {"type": "integer"},
                },
                "required": ["action"],
            },
            category="controlled-local",
            requires_confirmation=True,
            required_capabilities=["local:controlled"],
        )

    def execute(self, **params: Any) -> ToolResult:
        if not _enabled():
            return ToolResult(
                tool_name=self.tool_id,
                content=(
                    "Controlled local actions are disabled in this desktop profile."
                ),
                success=False,
            )
        try:
            action = str(params.get("action", ""))
            if action in {"write_text", "append_text"}:
                content = params.get("content")
                if not isinstance(content, str):
                    raise ValueError("Text content is required for this action.")
                sanitized_content = sanitize_model_output(
                    content, max_chars=_MAX_FILE_BYTES
                )
                if sanitized_content != content:
                    raise ValueError(
                        "Content containing a credential-like secret cannot be written."
                    )
                encoded = content.encode("utf-8")
                if len(encoded) > _MAX_FILE_BYTES:
                    raise ValueError(
                        f"Content exceeds the {_MAX_FILE_BYTES}-byte limit."
                    )
                path = _resolve_workspace_path(str(params.get("path", "")))
                if not path.parent.exists():
                    raise ValueError(
                        "The parent directory does not exist; create it "
                        "in a separate approved action."
                    )
                digest = hashlib.sha256(encoded).hexdigest()
                return _queue_action(
                    "local_file_write",
                    f"{action.replace('_', ' ')}: {path.name}",
                    {
                        "operation": action,
                        "path": str(path),
                        "content": content,
                        "content_sha256": digest,
                    },
                )
            if action == "create_directory":
                path = _resolve_workspace_path(str(params.get("path", "")))
                return _queue_action(
                    "local_directory_create",
                    f"Create directory: {path.name}",
                    {"path": str(path)},
                )
            if action == "open_app":
                application = _resolve_application_path(
                    str(params.get("application", ""))
                )
                return _queue_action(
                    "local_app_open",
                    f"Open application: {application.name}",
                    {"application": str(application)},
                )
            if action == "open_document":
                path = _resolve_workspace_path(str(params.get("path", "")))
                if (
                    not path.is_file()
                    or path.suffix.lower() not in _OPENABLE_DOCUMENT_SUFFIXES
                ):
                    raise ValueError(
                        "Only an existing, non-macro document in an approved "
                        "folder can be opened."
                    )
                return _queue_action(
                    "local_document_open",
                    f"Open document: {path.name}",
                    {"path": str(path)},
                )
            if action == "close_app":
                application = _resolve_application_path(
                    str(params.get("application", ""))
                )
                pid = params.get("pid")
                app_key = str(application)
                if not isinstance(pid, int) or pid not in _load_processes().get(
                    app_key, []
                ):
                    raise ValueError(
                        "Only a tracked application launched by this controlled "
                        "profile can be closed."
                    )
                return _queue_action(
                    "local_app_close",
                    f"Close application: {application.name} (PID {pid})",
                    {"application": app_key, "pid": pid},
                )
            raise ValueError("Unsupported controlled local action.")
        except ValueError as exc:
            _audit("local_action_queued", success=False, error=str(exc))
            return ToolResult(
                tool_name=self.tool_id, content=f"Action denied: {exc}", success=False
            )


def execute_approved_local_action(
    action_type: str, payload: dict[str, Any]
) -> tuple[bool, str]:
    """Execute one already-approved action from the localhost approval endpoint."""

    if not _enabled():
        return False, "Controlled local actions are disabled."
    try:
        if action_type == "local_file_write":
            path = _resolve_workspace_path(str(payload.get("path", "")))
            content = payload.get("content")
            operation = payload.get("operation")
            if not isinstance(content, str) or operation not in {
                "write_text",
                "append_text",
            }:
                raise ValueError("Invalid approved file action payload.")
            if sanitize_model_output(content, max_chars=_MAX_FILE_BYTES) != content:
                raise ValueError(
                    "Approved content contains a credential-like secret "
                    "and was blocked."
                )
            encoded = content.encode("utf-8")
            if len(encoded) > _MAX_FILE_BYTES:
                raise ValueError("Approved file content exceeds the size limit.")
            if operation == "write_text":
                temp = path.with_name(f".{path.name}.{uuid.uuid4().hex}.tmp")
                temp.write_text(content, encoding="utf-8")
                os.replace(temp, path)
            else:
                with path.open("a", encoding="utf-8") as handle:
                    handle.write(content)
            _audit(
                action_type,
                success=True,
                path=str(path),
                content_sha256=hashlib.sha256(encoded).hexdigest(),
            )
            return True, f"Updated {path.name} in the approved workspace."
        if action_type == "local_directory_create":
            path = _resolve_workspace_path(str(payload.get("path", "")))
            path.mkdir(parents=False, exist_ok=True)
            _audit(action_type, success=True, path=str(path))
            return True, f"Created directory {path.name}."
        if action_type == "local_app_open":
            application = _resolve_application_path(str(payload.get("application", "")))
            # No command-line arguments are accepted or derived from model output.
            process = subprocess.Popen(
                [str(application)], cwd=str(_workspace()), shell=False
            )
            processes = _load_processes()
            app_key = str(application)
            processes.setdefault(app_key, []).append(process.pid)
            _save_processes(processes)
            _audit(action_type, success=True, application=app_key, pid=process.pid)
            return True, f"Opened {application.name} (PID {process.pid})."
        if action_type == "local_document_open":
            if os.name != "nt":
                raise ValueError(
                    "Document opening is available only in the Windows desktop build."
                )
            path = _resolve_workspace_path(str(payload.get("path", "")))
            if (
                not path.is_file()
                or path.suffix.lower() not in _OPENABLE_DOCUMENT_SUFFIXES
            ):
                raise ValueError("Invalid approved document action payload.")
            start_file = getattr(os, "startfile", None)
            if start_file is None:
                raise ValueError("Windows document opener is unavailable.")
            # The user controls the associated application. No argument, macro,
            # shell text, or simulated input is accepted from the model.
            start_file(str(path))
            _audit(action_type, success=True, path=str(path))
            return (
                True,
                f"Opened {path.name} in its user-configured default application.",
            )
        if action_type == "local_app_close":
            application = _resolve_application_path(str(payload.get("application", "")))
            pid = payload.get("pid")
            app_key = str(application)
            if not isinstance(pid, int):
                raise ValueError("Invalid approved application close payload.")
            processes = _load_processes()
            if pid not in processes.get(app_key, []):
                raise ValueError(
                    "The application was not launched by this controlled profile."
                )
            os.kill(pid, 15)
            processes[app_key] = [
                tracked for tracked in processes[app_key] if tracked != pid
            ]
            _save_processes(processes)
            _audit(action_type, success=True, application=app_key, pid=pid)
            return True, f"Closed {application.name} (PID {pid})."
        return False, f"Unsupported controlled local action type: {action_type}"
    except (OSError, ValueError) as exc:
        _audit(action_type, success=False, error=str(exc))
        return False, str(exc)


__all__ = [
    "ControlledLocalActionTool",
    "ControlledWorkspaceReadTool",
    "ControlledWorkspaceListTool",
    "execute_approved_local_action",
    "list_authorized_folders",
    "replace_authorized_folders",
]
