"""Integration tests for UI-approved controlled local operations."""

from __future__ import annotations

import asyncio
from pathlib import Path

import pytest
from fastapi import HTTPException

from openjarvis.server import approval_routes
from openjarvis.tools.approval_store import STATUS_EXECUTED, TIER_HIGH, ApprovalStore


def test_ui_approval_consumes_controlled_write_once(
    monkeypatch, tmp_path: Path
) -> None:
    workspace = tmp_path / "OpenJarvis-Workspace"
    workspace.mkdir()
    monkeypatch.setenv("OPENJARVIS_ENABLE_CONTROLLED_LOCAL_ACTIONS", "1")
    monkeypatch.setenv("OPENJARVIS_CONTROLLED_WORKSPACE", str(workspace))

    store = ApprovalStore(str(tmp_path / "approvals.db"))
    previous_store = approval_routes._store
    approval_routes._store = store
    try:
        target = workspace / "approved.txt"
        action = store.queue_action(
            action_type="local_file_write",
            description="Write approved.txt",
            payload={
                "operation": "write_text",
                "path": str(target),
                "content": "approved",
            },
            permission_key="local_file_write:always_ask",
            tier=TIER_HIGH,
            ttl_hours=1,
        )

        response = asyncio.run(approval_routes.approve_action(action.id))

        assert response["status"] == "executed"
        assert response["success"] is True
        assert target.read_text(encoding="utf-8") == "approved"
        assert store.get_action(action.id).status == STATUS_EXECUTED

        with pytest.raises(HTTPException) as exc:
            asyncio.run(approval_routes.approve_action(action.id))
        assert exc.value.status_code == 409
    finally:
        approval_routes._store = previous_store
        store.close()


def test_approval_serialization_redacts_pending_write_content(tmp_path: Path) -> None:
    store = ApprovalStore(str(tmp_path / "approvals.db"))
    previous_store = approval_routes._store
    approval_routes._store = store
    try:
        action = store.queue_action(
            action_type="local_file_write",
            description="Write a secret",
            payload={"path": "C:/safe.txt", "content": "top secret"},
            permission_key="local_file_write:always_ask",
            tier=TIER_HIGH,
            ttl_hours=1,
        )

        response = asyncio.run(approval_routes.list_pending_approvals())

        preview = response["actions"][0]["payload"]
        assert preview["content"]["redacted"] is True
        assert preview["content"]["length"] == len("top secret")
        assert "top secret" not in str(response)
        assert response["actions"][0]["id"] == action.id
    finally:
        approval_routes._store = previous_store
        store.close()


def test_desktop_plan_claim_is_authenticated_and_single_use(
    monkeypatch, tmp_path: Path
) -> None:
    monkeypatch.setenv("OPENJARVIS_DESKTOP_BROKER_TOKEN", "broker-test-token")
    store = ApprovalStore(str(tmp_path / "approvals.db"))
    previous_store = approval_routes._store
    approval_routes._store = store
    try:
        action = store.queue_action(
            action_type="desktop_automation_plan",
            description="Read public recipe page",
            payload={
                "version": 1,
                "summary": "Read public recipe page",
                "target": {
                    "application": (
                        r"C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"
                    ),
                    "window_title": "Recipe",
                },
                "steps": [{"type": "inspect_window"}],
            },
            permission_key="desktop_automation_plan:always_ask",
            tier=TIER_HIGH,
            ttl_hours=1,
        )

        approved = asyncio.run(approval_routes.approve_action(action.id))
        assert approved["status"] == "awaiting_desktop_broker"

        with pytest.raises(HTTPException) as rejected:
            approval_routes._require_desktop_broker_token("wrong-token")
        assert rejected.value.status_code == 403
        approval_routes._require_desktop_broker_token("broker-test-token")

        claimed = asyncio.run(approval_routes.claim_desktop_plan(action.id, None))
        assert claimed["id"] == action.id
        assert claimed["plan"]["summary"] == "Read public recipe page"
        assert store.get_action(action.id).status == "executing"

        with pytest.raises(HTTPException) as duplicate_claim:
            asyncio.run(approval_routes.claim_desktop_plan(action.id, None))
        assert duplicate_claim.value.status_code == 409

        completed = asyncio.run(
            approval_routes.complete_desktop_plan(
                action.id,
                approval_routes.DesktopPlanCompletion(
                    success=True,
                    summary="Read result with sk-test-key-should-not-persist",
                ),
                None,
            )
        )
        assert completed == {"status": "executed", "id": action.id, "success": True}
        stored = store.get_action(action.id)
        assert stored.status == STATUS_EXECUTED
        summary = stored.payload["execution"]["summary"]
        assert "sk-test-key-should-not-persist" not in summary
        assert "[REDACTED" in summary
    finally:
        approval_routes._store = previous_store
        store.close()


def test_android_adb_diagnostic_is_authenticated_and_single_use(
    monkeypatch, tmp_path: Path
) -> None:
    monkeypatch.setenv("OPENJARVIS_ANDROID_ADB_BROKER_TOKEN", "adb-broker-test-token")
    store = ApprovalStore(str(tmp_path / "approvals.db"))
    previous_store = approval_routes._store
    approval_routes._store = store
    try:
        action = store.queue_action(
            action_type="android_adb_software_diagnostic",
            description="Check Android software health",
            payload={"version": 1, "summary": "Check Android software health"},
            permission_key="android_adb_software_diagnostic:always_ask",
            tier=TIER_HIGH,
            ttl_hours=1,
        )

        approved = asyncio.run(approval_routes.approve_action(action.id))
        assert approved["status"] == "awaiting_android_adb_broker"

        with pytest.raises(HTTPException) as rejected:
            approval_routes._require_android_adb_broker_token("wrong-token")
        assert rejected.value.status_code == 403
        approval_routes._require_android_adb_broker_token("adb-broker-test-token")

        claimed = asyncio.run(
            approval_routes.claim_android_adb_diagnostic(action.id, None)
        )
        assert claimed["id"] == action.id
        assert claimed["plan"] == {
            "version": 1,
            "summary": "Check Android software health",
        }
        assert store.get_action(action.id).status == "executing"

        with pytest.raises(HTTPException) as duplicate_claim:
            asyncio.run(approval_routes.claim_android_adb_diagnostic(action.id, None))
        assert duplicate_claim.value.status_code == 409

        completed = asyncio.run(
            approval_routes.complete_android_adb_diagnostic(
                action.id,
                approval_routes.DesktopPlanCompletion(
                    success=True,
                    summary="Android version 15; token sk-test-key-should-not-persist",
                ),
                None,
            )
        )
        assert completed == {"status": "executed", "id": action.id, "success": True}
        summary = store.get_action(action.id).payload["execution"]["summary"]
        assert "sk-test-key-should-not-persist" not in summary
        assert "[REDACTED" in summary
    finally:
        approval_routes._store = previous_store
        store.close()


def test_identical_pending_actions_are_deduplicated(tmp_path: Path) -> None:
    store = ApprovalStore(str(tmp_path / "approvals.db"))
    try:
        first = store.queue_action(
            action_type="local_file_write",
            description="Write an approved note",
            payload={"operation": "write_text", "path": "note.txt", "content": "safe"},
            permission_key="local_file_write:always_ask",
            tier=TIER_HIGH,
        )
        retry = store.queue_action(
            action_type="local_file_write",
            description="Write an approved note",
            payload={"content": "safe", "path": "note.txt", "operation": "write_text"},
            permission_key="local_file_write:always_ask",
            tier=TIER_HIGH,
        )
        different_description = store.queue_action(
            action_type="local_file_write",
            description="Write a separately reviewed note",
            payload={"operation": "write_text", "path": "note.txt", "content": "safe"},
            permission_key="local_file_write:always_ask",
            tier=TIER_HIGH,
        )
        different = store.queue_action(
            action_type="local_file_write",
            description="Write a different note",
            payload={"operation": "write_text", "path": "other.txt", "content": "safe"},
            permission_key="local_file_write:always_ask",
            tier=TIER_HIGH,
        )

        assert retry.id == first.id
        assert different_description.id != first.id
        assert different.id != first.id
        assert [action.id for action in store.list_pending()] == [
            first.id,
            different_description.id,
            different.id,
        ]
    finally:
        store.close()
