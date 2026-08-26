"""Security tests for bounded, approval-gated local desktop actions."""

from __future__ import annotations

from pathlib import Path

import pytest

from openjarvis.tools import controlled_local


def _enable_workspace(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> Path:
    workspace = tmp_path / "OpenJarvis-Workspace"
    monkeypatch.setenv("OPENJARVIS_ENABLE_CONTROLLED_LOCAL_ACTIONS", "1")
    monkeypatch.setenv("OPENJARVIS_CONTROLLED_WORKSPACE", str(workspace))
    return workspace


def test_write_is_only_queued_until_desktop_approval(monkeypatch, tmp_path) -> None:
    workspace = _enable_workspace(monkeypatch, tmp_path)
    tool = controlled_local.ControlledLocalActionTool()

    result = tool.execute(action="write_text", path="note.txt", content="hello")

    assert result.success
    assert result.metadata["requires_ui_approval"] is True
    assert not (workspace / "note.txt").exists()


def test_approved_write_is_atomic_and_limited_to_workspace(
    monkeypatch, tmp_path
) -> None:
    workspace = _enable_workspace(monkeypatch, tmp_path)
    workspace.mkdir(parents=True)
    target = workspace / "note.txt"

    success, message = controlled_local.execute_approved_local_action(
        "local_file_write",
        {"operation": "write_text", "path": str(target), "content": "approved"},
    )

    assert success, message
    assert target.read_text(encoding="utf-8") == "approved"


def test_outside_workspace_and_executable_writes_are_rejected(
    monkeypatch, tmp_path
) -> None:
    workspace = _enable_workspace(monkeypatch, tmp_path)
    workspace.mkdir(parents=True)
    tool = controlled_local.ControlledLocalActionTool()

    outside = tool.execute(
        action="write_text", path=str(tmp_path / "outside.txt"), content="no"
    )
    executable = tool.execute(action="write_text", path="danger.ps1", content="no")

    assert not outside.success
    assert "workspace" in outside.content.lower()
    assert not executable.success
    assert "executable" in executable.content.lower()


def test_controlled_actions_are_disabled_without_explicit_desktop_opt_in(
    monkeypatch,
) -> None:
    monkeypatch.delenv("OPENJARVIS_ENABLE_CONTROLLED_LOCAL_ACTIONS", raising=False)
    result = controlled_local.ControlledLocalActionTool().execute(
        action="create_directory", path="safe"
    )
    assert not result.success
    assert "disabled" in result.content.lower()


def test_application_control_exposes_no_command_argument_parameter() -> None:
    """The model may name an executable, but cannot supply flags or shell text."""

    tool = controlled_local.ControlledLocalActionTool()
    properties = tool.spec.parameters["properties"]
    assert "application" in properties
    assert "args" not in properties
    assert "command" not in properties
    assert "shell" not in properties


def test_workspace_read_redacts_detected_credential(monkeypatch, tmp_path) -> None:
    workspace = _enable_workspace(monkeypatch, tmp_path)
    workspace.mkdir(parents=True)
    path = workspace / "credential.txt"
    path.write_text("sk-proj-abcdefghijklmnopqrstuvwxyz123456", encoding="utf-8")

    tool = controlled_local.ControlledWorkspaceReadTool()
    result = tool.execute(path="credential.txt")

    assert result.success
    assert "sk-proj-" not in result.content
    assert "REDACTED_OPENAI_KEY" in result.content


def test_credential_like_content_is_rejected_before_approval(
    monkeypatch, tmp_path
) -> None:
    workspace = _enable_workspace(monkeypatch, tmp_path)
    workspace.mkdir(parents=True)

    result = controlled_local.ControlledLocalActionTool().execute(
        action="write_text",
        path="credential.txt",
        content="sk-proj-abcdefghijklmnopqrstuvwxyz123456",
    )

    assert not result.success
    assert "credential-like" in result.content.lower()


def test_user_approved_external_folder_can_be_used_after_approval(
    monkeypatch, tmp_path
) -> None:
    workspace = _enable_workspace(monkeypatch, tmp_path)
    workspace.mkdir(parents=True)
    monkeypatch.setenv("OPENJARVIS_HOME", str(tmp_path / "app-state"))
    documents = tmp_path / "Documents"
    documents.mkdir()

    saved = controlled_local.replace_authorized_folders([str(documents)])
    assert saved == [str(documents.resolve())]

    queued = controlled_local.ControlledLocalActionTool().execute(
        action="write_text",
        path=str(documents / "approved-note.txt"),
        content="approved external folder",
    )

    assert queued.success
    success, message = controlled_local.execute_approved_local_action(
        "local_file_write",
        queued.metadata
        and {
            "operation": "write_text",
            "path": str(documents / "approved-note.txt"),
            "content": "approved external folder",
        },
    )
    assert success, message
    assert (documents / "approved-note.txt").read_text(encoding="utf-8") == (
        "approved external folder"
    )


def test_folder_listing_filters_sensitive_and_executable_entries(
    monkeypatch, tmp_path
) -> None:
    workspace = _enable_workspace(monkeypatch, tmp_path)
    workspace.mkdir(parents=True)
    (workspace / "note.txt").write_text("safe", encoding="utf-8")
    (workspace / ".env").write_text("SECRET=value", encoding="utf-8")
    (workspace / "script.ps1").write_text("ignored", encoding="utf-8")

    result = controlled_local.ControlledWorkspaceListTool().execute(path=str(workspace))

    assert result.success
    assert "note.txt" in result.content
    assert ".env" not in result.content
    assert "script.ps1" not in result.content


def test_high_risk_windows_executables_are_rejected(tmp_path) -> None:
    executable = tmp_path / "powershell.exe"
    executable.write_text("not executed", encoding="utf-8")

    assert controlled_local._is_blocked_application_path(executable)
    assert not controlled_local._is_blocked_application_path(tmp_path / "notepad.exe")


def test_document_open_is_queued_and_rejects_macro_or_executable_files(
    monkeypatch, tmp_path
) -> None:
    workspace = _enable_workspace(monkeypatch, tmp_path)
    workspace.mkdir(parents=True)
    document = workspace / "report.csv"
    document.write_text("name,value\nA,1\n", encoding="utf-8")
    macro = workspace / "unsafe.docm"
    macro.write_text("not accepted", encoding="utf-8")

    tool = controlled_local.ControlledLocalActionTool()
    queued = tool.execute(action="open_document", path=str(document))
    rejected = tool.execute(action="open_document", path=str(macro))

    assert queued.success
    assert queued.metadata["requires_ui_approval"] is True
    assert not rejected.success
    assert "non-macro" in rejected.content.lower()


def test_workspace_metadata_and_agent_configuration_are_protected(
    monkeypatch, tmp_path
) -> None:
    workspace = _enable_workspace(monkeypatch, tmp_path)
    workspace.mkdir(parents=True)
    git_dir = workspace / ".git"
    git_dir.mkdir()
    (git_dir / "config").write_text("[remote]", encoding="utf-8")
    (workspace / "AGENTS.md").write_text("untrusted instructions", encoding="utf-8")
    (workspace / "note.txt").write_text("safe", encoding="utf-8")

    reader = controlled_local.ControlledWorkspaceReadTool()
    git_read = reader.execute(path=".git/config")
    agent_read = reader.execute(path="AGENTS.md")
    listing = controlled_local.ControlledWorkspaceListTool().execute(
        path=str(workspace)
    )
    ordinary_read = reader.execute(path="note.txt")

    assert not git_read.success
    assert not agent_read.success
    assert "metadata" in git_read.content.lower()
    assert "metadata" in agent_read.content.lower()
    assert listing.success
    assert ".git" not in listing.content
    assert "AGENTS.md" not in listing.content
    assert "note.txt" in listing.content
    assert ordinary_read.success
    assert ordinary_read.content == "safe"


def test_workspace_metadata_write_is_rejected_before_approval(
    monkeypatch, tmp_path
) -> None:
    workspace = _enable_workspace(monkeypatch, tmp_path)
    workspace.mkdir(parents=True)
    (workspace / ".vscode").mkdir()

    result = controlled_local.ControlledLocalActionTool().execute(
        action="write_text", path=".vscode/settings.json", content="{}"
    )

    assert not result.success
    assert "metadata" in result.content.lower()
