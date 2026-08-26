"""Approval-gated Android ADB software diagnostics for the secure desktop profile.

The cloud model never receives an ADB shell. It can only queue one bounded,
read-only diagnostic request for the Android device selected locally by the user.
"""

from __future__ import annotations

import os
import re
from typing import Any

from openjarvis.core.registry import ToolRegistry
from openjarvis.core.types import ToolResult
from openjarvis.security.output_safety import sanitize_model_output
from openjarvis.tools._stubs import BaseTool, ToolSpec
from openjarvis.tools.approval_store import TIER_HIGH, ApprovalStore

_ENABLED_ENV = "OPENJARVIS_ENABLE_CONTROLLED_ANDROID_ADB"
_ACTION_TYPE = "android_adb_software_diagnostic"
_PERMISSION_KEY = "android_adb_software_diagnostic:always_ask"
_PLAN_TTL_MINUTES = 10
_SENSITIVE_TERMS = re.compile(
    r"\b("
    r"account|bank|banking|card|checkout|credential|credit\s*card|cvv|"
    r"iban|login|otp|password|pay(?:ment)?|pin|purchase|recovery|"
    r"sign\s*in|transfer|two[\s-]?factor|wallet"
    r")\b",
    re.IGNORECASE,
)


def _enabled() -> bool:
    return os.getenv(_ENABLED_ENV, "").strip() == "1"


def _queue_diagnostic(summary: str) -> ToolResult:
    action = ApprovalStore().queue_action(
        action_type=_ACTION_TYPE,
        description="Android ADB software diagnostic",
        payload={"version": 1, "summary": summary},
        permission_key=_PERMISSION_KEY,
        tier=TIER_HIGH,
        ttl_hours=_PLAN_TTL_MINUTES / 60,
    )
    return ToolResult(
        tool_name="controlled_android_adb_diagnostic",
        success=True,
        content=(
            f"Android diagnostic {action.id} is queued for local review. It expires "
            f"in {_PLAN_TTL_MINUTES} minutes. The native ADB broker can run only its "
            "fixed read-only software checks on the device selected in Settings."
        ),
        metadata={
            "action_id": action.id,
            "status": action.status,
            "requires_ui_approval": True,
        },
    )


@ToolRegistry.register("controlled_android_adb_diagnostic")
class ControlledAndroidAdbDiagnosticTool(BaseTool):
    """Queue a bounded software diagnostic for a user-selected Android device."""

    tool_id = "controlled_android_adb_diagnostic"

    @property
    def spec(self) -> ToolSpec:
        return ToolSpec(
            name=self.tool_id,
            description=(
                "Propose a read-only Android software diagnostic through ADB for the "
                "single device the user has selected locally in Settings. It cannot "
                "execute arbitrary ADB commands, open apps, tap or type on Android, "
                "install or remove apps, copy files, connect devices, access logs, "
                "use root, handle credentials, or modify the phone. Local one-time "
                "approval is always required before the native broker can run it."
            ),
            parameters={
                "type": "object",
                "properties": {
                    "summary": {
                        "type": "string",
                        "description": (
                            "Short, non-sensitive reason for the requested software "
                            "diagnostic, maximum 240 characters."
                        ),
                    }
                },
                "required": ["summary"],
            },
            category="controlled-android-adb",
            requires_confirmation=True,
            required_capabilities=["android:adb-controlled"],
        )

    def execute(self, **params: Any) -> ToolResult:
        if not _enabled():
            return ToolResult(
                tool_name=self.tool_id,
                content=(
                    "Controlled Android ADB diagnostics are disabled in this profile."
                ),
                success=False,
            )
        summary = params.get("summary")
        if not isinstance(summary, str):
            return ToolResult(
                tool_name=self.tool_id,
                content="Android diagnostic denied: summary must be text.",
                success=False,
            )
        summary = " ".join(summary.split())
        if not summary or len(summary) > 240:
            return ToolResult(
                tool_name=self.tool_id,
                content=(
                    "Android diagnostic denied: summary must contain "
                    "1 to 240 characters."
                ),
                success=False,
            )
        safe_summary = sanitize_model_output(summary, max_chars=240)
        if (
            _SENSITIVE_TERMS.search(summary)
            or safe_summary != summary
            or "[REDACTED" in safe_summary
        ):
            return ToolResult(
                tool_name=self.tool_id,
                content=(
                    "Android diagnostic denied: the request contains sensitive content."
                ),
                success=False,
            )
        return _queue_diagnostic(summary)


@ToolRegistry.register("controlled_android_adb_result")
class ControlledAndroidAdbResultTool(BaseTool):
    """Return only the redacted summary emitted by the native ADB broker."""

    tool_id = "controlled_android_adb_result"

    @property
    def spec(self) -> ToolSpec:
        return ToolSpec(
            name=self.tool_id,
            description=(
                "Read the locally redacted result of an approved Android ADB software "
                "diagnostic. Use only an action ID returned by "
                "controlled_android_adb_diagnostic."
            ),
            parameters={
                "type": "object",
                "properties": {"action_id": {"type": "string"}},
                "required": ["action_id"],
            },
            category="controlled-android-adb",
            required_capabilities=["android:adb-controlled"],
        )

    def execute(self, **params: Any) -> ToolResult:
        if not _enabled():
            return ToolResult(
                tool_name=self.tool_id,
                content=(
                    "Controlled Android ADB diagnostics are disabled in this profile."
                ),
                success=False,
            )
        action_id = params.get("action_id")
        if (
            not isinstance(action_id, str)
            or not action_id.isalnum()
            or len(action_id) > 32
        ):
            return ToolResult(
                tool_name=self.tool_id,
                content="Android diagnostic result denied: invalid action ID.",
                success=False,
            )
        action = ApprovalStore().get_action(action_id)
        if action is None or action.action_type != _ACTION_TYPE:
            return ToolResult(
                tool_name=self.tool_id,
                content="Android diagnostic result is unavailable.",
                success=False,
            )
        execution = action.payload.get("execution")
        if not isinstance(execution, dict):
            return ToolResult(
                tool_name=self.tool_id,
                content="Android diagnostic has not completed yet.",
                success=False,
                metadata={"status": action.status},
            )
        summary = sanitize_model_output(
            str(execution.get("summary", "")), max_chars=4_000
        )
        return ToolResult(
            tool_name=self.tool_id,
            content=summary or "Android diagnostic completed without readable output.",
            success=bool(execution.get("success")),
            metadata={"action_id": action.id, "status": action.status},
        )


__all__ = [
    "ControlledAndroidAdbDiagnosticTool",
    "ControlledAndroidAdbResultTool",
]
