"""REST endpoints for the proactive-agent approval queue."""

from __future__ import annotations

import hmac
import logging
import os
from typing import Any, Dict, Optional

from pydantic import BaseModel, Field

from openjarvis.security.output_safety import (
    sanitize_model_output,
    sanitize_payload_for_display,
)
from openjarvis.tools.approval_store import (
    STATUS_APPROVED,
    STATUS_DENIED,
    STATUS_EXECUTED,
    STATUS_EXECUTING,
    STATUS_PENDING,
    ApprovalStore,
    PendingAction,
)

try:
    from fastapi import APIRouter, Depends, Header, HTTPException
except ImportError:
    raise ImportError("fastapi is required for approval routes")

logger = logging.getLogger(__name__)

router = APIRouter()


class DesktopPlanCompletion(BaseModel):
    """Bounded native-broker completion body; never placed in a URL."""

    success: bool
    summary: str = Field(default="", max_length=4_000)


# Singleton that shares the same DB file as ProactiveAgent (WAL mode is safe)
_store: Optional[ApprovalStore] = None


def _get_store() -> ApprovalStore:
    global _store
    if _store is None:
        _store = ApprovalStore()
    return _store


def _require_desktop_broker_token(
    x_openjarvis_desktop_broker: Optional[str] = Header(default=None),
) -> None:
    """Authenticate only the native desktop broker using a per-launch secret."""

    expected = os.getenv("OPENJARVIS_DESKTOP_BROKER_TOKEN", "")
    if not expected or not x_openjarvis_desktop_broker:
        raise HTTPException(
            status_code=403,
            detail="Desktop broker authentication required",
        )
    if not hmac.compare_digest(expected, x_openjarvis_desktop_broker):
        raise HTTPException(
            status_code=403,
            detail="Desktop broker authentication failed",
        )


def _require_android_adb_broker_token(
    x_openjarvis_android_adb_broker: Optional[str] = Header(default=None),
) -> None:
    """Authenticate only the native ADB diagnostic broker for this launch."""

    expected = os.getenv("OPENJARVIS_ANDROID_ADB_BROKER_TOKEN", "")
    if not expected or not x_openjarvis_android_adb_broker:
        raise HTTPException(
            status_code=403, detail="Android ADB broker authentication required"
        )
    if not hmac.compare_digest(expected, x_openjarvis_android_adb_broker):
        raise HTTPException(
            status_code=403, detail="Android ADB broker authentication failed"
        )


def _serialize(action: PendingAction) -> Dict[str, Any]:
    return {
        "id": action.id,
        "action_type": sanitize_model_output(action.action_type, max_chars=120),
        "description": sanitize_model_output(action.description, max_chars=1_000),
        "payload": sanitize_payload_for_display(action.payload),
        "permission_key": sanitize_model_output(action.permission_key, max_chars=240),
        "tier": action.tier,
        "status": action.status,
        "created_at": action.created_at,
        "expires_at": action.expires_at,
    }


@router.get("/v1/approvals/pending")
async def list_pending_approvals() -> Dict[str, Any]:
    store = _get_store()
    store.expire_stale()
    actions = store.list_pending()
    return {"actions": [_serialize(a) for a in actions], "count": len(actions)}


@router.post("/v1/approvals/{action_id}/approve")
async def approve_action(action_id: str) -> Dict[str, Any]:
    store = _get_store()
    store.expire_stale()
    action = store.get_action(action_id)
    if action is None:
        raise HTTPException(status_code=404, detail="Action not found")
    if action.status != STATUS_PENDING:
        raise HTTPException(status_code=409, detail="Action is no longer pending")

    local_action_types = {
        "local_file_write",
        "local_directory_create",
        "local_app_open",
        "local_document_open",
        "local_app_close",
    }
    if action.action_type in local_action_types:
        from openjarvis.tools.controlled_local import execute_approved_local_action

        success, message = execute_approved_local_action(
            action.action_type, action.payload
        )
        # A local approval is single-use even if execution fails, preventing a
        # stale consent from being replayed after a path, process, or payload changes.
        store.update_status(action_id, STATUS_EXECUTED)
        logger.info(
            "Controlled local action %s consumed via UI: %s", action_id, success
        )
        return {
            "status": "executed",
            "id": action_id,
            "success": success,
            "message": sanitize_model_output(message, max_chars=1_000),
        }

    store.update_status(action_id, STATUS_APPROVED)
    if action.action_type == "desktop_automation_plan":
        logger.info("Desktop plan %s approved; awaiting native broker claim", action_id)
        return {"status": "awaiting_desktop_broker", "id": action_id}
    if action.action_type == "android_adb_software_diagnostic":
        logger.info(
            "Android ADB diagnostic %s approved; awaiting native broker claim",
            action_id,
        )
        return {"status": "awaiting_android_adb_broker", "id": action_id}

    logger.info("Action %s approved via UI", action_id)
    return {"status": "approved", "id": action_id}


@router.get("/v1/approvals/desktop-plans/approved")
async def list_approved_desktop_plan_ids(
    _: None = Depends(_require_desktop_broker_token),
) -> Dict[str, Any]:
    """List only approved plan identifiers for the native broker polling loop."""

    store = _get_store()
    store.expire_stale()
    ids = [
        action.id
        for action in store.list_approved()
        if action.action_type == "desktop_automation_plan"
    ]
    return {"ids": ids}


@router.post("/v1/approvals/{action_id}/desktop-plan/claim")
async def claim_desktop_plan(
    action_id: str,
    _: None = Depends(_require_desktop_broker_token),
) -> Dict[str, Any]:
    """Return one UI-approved desktop plan to the authenticated native broker."""

    store = _get_store()
    store.expire_stale()
    action = store.get_action(action_id)
    if action is None:
        raise HTTPException(status_code=404, detail="Action not found")
    if action.action_type != "desktop_automation_plan":
        raise HTTPException(status_code=400, detail="Action is not a desktop plan")
    claimed = store.claim_approved_action(action_id)
    if claimed is None:
        raise HTTPException(
            status_code=409,
            detail="Desktop plan is no longer claimable",
        )
    logger.info("Desktop plan %s claimed by native broker", action_id)
    return {"id": claimed.id, "plan": claimed.payload}


@router.post("/v1/approvals/{action_id}/desktop-plan/complete")
async def complete_desktop_plan(
    action_id: str,
    completion: DesktopPlanCompletion,
    _: None = Depends(_require_desktop_broker_token),
) -> Dict[str, Any]:
    """Consume a claimed plan regardless of its native execution outcome."""

    store = _get_store()
    action = store.get_action(action_id)
    if action is None:
        raise HTTPException(status_code=404, detail="Action not found")
    if (
        action.action_type != "desktop_automation_plan"
        or action.status != STATUS_EXECUTING
    ):
        raise HTTPException(status_code=409, detail="Desktop plan is not executing")
    sanitized_summary = sanitize_model_output(completion.summary, max_chars=4_000)
    store.record_execution_result(
        action_id,
        success=completion.success,
        summary=sanitized_summary,
    )
    store.update_status(action_id, STATUS_EXECUTED)
    logger.info(
        "Desktop plan %s completed by native broker: %s", action_id, completion.success
    )
    return {
        "status": "executed",
        "id": action_id,
        "success": bool(completion.success),
    }


@router.get("/v1/approvals/android-adb/approved")
async def list_approved_android_adb_diagnostic_ids(
    _: None = Depends(_require_android_adb_broker_token),
) -> Dict[str, Any]:
    """List approved Android diagnostic IDs, never device settings or payloads."""

    store = _get_store()
    store.expire_stale()
    ids = [
        action.id
        for action in store.list_approved()
        if action.action_type == "android_adb_software_diagnostic"
    ]
    return {"ids": ids}


@router.post("/v1/approvals/{action_id}/android-adb/claim")
async def claim_android_adb_diagnostic(
    action_id: str,
    _: None = Depends(_require_android_adb_broker_token),
) -> Dict[str, Any]:
    """Claim one user-approved Android diagnostic for the native broker."""

    store = _get_store()
    store.expire_stale()
    action = store.get_action(action_id)
    if action is None:
        raise HTTPException(status_code=404, detail="Action not found")
    if action.action_type != "android_adb_software_diagnostic":
        raise HTTPException(
            status_code=400, detail="Action is not an Android ADB diagnostic"
        )
    claimed = store.claim_approved_action(action_id)
    if claimed is None:
        raise HTTPException(
            status_code=409, detail="Android ADB diagnostic is no longer claimable"
        )
    logger.info("Android ADB diagnostic %s claimed by native broker", action_id)
    return {"id": claimed.id, "plan": claimed.payload}


@router.post("/v1/approvals/{action_id}/android-adb/complete")
async def complete_android_adb_diagnostic(
    action_id: str,
    completion: DesktopPlanCompletion,
    _: None = Depends(_require_android_adb_broker_token),
) -> Dict[str, Any]:
    """Persist a redacted result from one claimed native Android diagnostic."""

    store = _get_store()
    action = store.get_action(action_id)
    if action is None:
        raise HTTPException(status_code=404, detail="Action not found")
    if (
        action.action_type != "android_adb_software_diagnostic"
        or action.status != STATUS_EXECUTING
    ):
        raise HTTPException(
            status_code=409, detail="Android ADB diagnostic is not executing"
        )
    sanitized_summary = sanitize_model_output(completion.summary, max_chars=4_000)
    store.record_execution_result(
        action_id,
        success=completion.success,
        summary=sanitized_summary,
    )
    store.update_status(action_id, STATUS_EXECUTED)
    logger.info(
        "Android ADB diagnostic %s completed by native broker: %s",
        action_id,
        completion.success,
    )
    return {"status": "executed", "id": action_id, "success": bool(completion.success)}


@router.post("/v1/approvals/{action_id}/deny")
async def deny_action(action_id: str) -> Dict[str, Any]:
    store = _get_store()
    action = store.get_action(action_id)
    if action is None:
        raise HTTPException(status_code=404, detail="Action not found")
    if action.status != STATUS_PENDING:
        raise HTTPException(status_code=409, detail="Action is no longer pending")
    store.update_status(action_id, STATUS_DENIED)
    logger.info("Action %s denied via UI", action_id)
    return {"status": "denied", "id": action_id}


__all__ = ["router"]
