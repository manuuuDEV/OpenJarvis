"""Structured, approval-gated plans for the native Windows desktop operator.

The cloud model never receives a raw mouse, keyboard, shell, or browser-control
channel.  It may only propose a short declarative plan.  A local native broker
will validate the same plan again immediately before any UI Automation action.
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

_OPERATOR_ENABLED_ENV = "OPENJARVIS_ENABLE_CONTROLLED_DESKTOP_OPERATOR"
_PLAN_TTL_MINUTES = 10
_MAX_STEPS = 12
_MAX_TEXT_CHARS = 4_000
_MAX_FIELD_CHARS = 240
_ALLOWED_STEP_TYPES = frozenset(
    {
        "focus_window",
        "inspect_window",
        "read_accessible_text",
        "invoke_element",
        "set_text",
    }
)
_ALLOWED_CONTROL_TYPES = frozenset(
    {
        "Button",
        "Edit",
        "Hyperlink",
        "ListItem",
        "MenuItem",
        "TabItem",
        "Text",
    }
)
_WINDOWS_EXE_PATH = re.compile(r"^[A-Za-z]:\\[^\r\n]+\.exe$", re.IGNORECASE)
_SENSITIVE_TERMS = re.compile(
    r"\b("
    r"account|bank|banking|beneficiary|bonifico|card|checkout|credential|"
    r"credit\s*card|cvv|delete\s*account|fatturazione|iban|investment|"
    r"login|otp|password|pay(?:ment)?|pin|purchase|recovery|security\s*code|"
    r"sign\s*in|transfer|two[\s-]?factor|verifica|wallet"
    r")\b",
    re.IGNORECASE,
)


def _enabled() -> bool:
    return os.getenv(_OPERATOR_ENABLED_ENV, "").strip() == "1"


def _as_short_string(value: Any, field: str, *, allow_empty: bool = False) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{field} must be text.")
    cleaned = " ".join(value.split())
    if not cleaned and not allow_empty:
        raise ValueError(f"{field} cannot be empty.")
    if len(cleaned) > _MAX_FIELD_CHARS:
        raise ValueError(f"{field} exceeds the {_MAX_FIELD_CHARS}-character limit.")
    return cleaned


def _reject_sensitive_text(value: str, field: str) -> None:
    if _SENSITIVE_TERMS.search(value):
        raise ValueError(
            f"{field} indicates a login, financial, account, or credential flow. "
            "The desktop operator must stop and request manual user handling."
        )


def _validate_target(raw_target: Any) -> dict[str, str]:
    if not isinstance(raw_target, dict):
        raise ValueError("target must be an object describing one approved window.")
    application = _as_short_string(raw_target.get("application"), "target.application")
    if not _WINDOWS_EXE_PATH.fullmatch(application):
        raise ValueError(
            "target.application must be an absolute Windows .exe path for one "
            "user-authorized application."
        )
    window_title = _as_short_string(
        raw_target.get("window_title"), "target.window_title"
    )
    _reject_sensitive_text(application, "target.application")
    _reject_sensitive_text(window_title, "target.window_title")
    return {"application": application, "window_title": window_title}


def _validate_element(raw_element: Any, *, required: bool) -> dict[str, str] | None:
    if raw_element is None and not required:
        return None
    if not isinstance(raw_element, dict):
        raise ValueError("step.element must identify one accessible UI element.")
    element: dict[str, str] = {}
    for key in ("name", "automation_id", "control_type"):
        value = raw_element.get(key)
        if value is None:
            continue
        element[key] = _as_short_string(value, f"step.element.{key}")
    if not element:
        raise ValueError(
            "step.element must contain a name, automation_id, or control_type."
        )
    control_type = element.get("control_type")
    if control_type and control_type not in _ALLOWED_CONTROL_TYPES:
        raise ValueError("The requested UI control type is not allowed for automation.")
    for value in element.values():
        _reject_sensitive_text(value, "step.element")
    return element


def validate_desktop_plan(raw_plan: Any) -> dict[str, Any]:
    """Validate a short low-risk desktop plan before it reaches the approval UI."""

    if not isinstance(raw_plan, dict):
        raise ValueError("plan must be a structured object.")
    target = _validate_target(raw_plan.get("target"))
    steps = raw_plan.get("steps")
    if not isinstance(steps, list) or not steps:
        raise ValueError("plan.steps must contain at least one action.")
    if len(steps) > _MAX_STEPS:
        raise ValueError(f"A desktop plan may contain at most {_MAX_STEPS} steps.")

    normalized_steps: list[dict[str, Any]] = []
    for index, raw_step in enumerate(steps, start=1):
        if not isinstance(raw_step, dict):
            raise ValueError(f"Step {index} must be an object.")
        step_type = _as_short_string(raw_step.get("type"), f"step {index}.type")
        if step_type not in _ALLOWED_STEP_TYPES:
            raise ValueError(f"Step {index} uses an unsupported desktop action.")
        element_required = step_type not in {"focus_window", "inspect_window"}
        element = _validate_element(raw_step.get("element"), required=element_required)
        step: dict[str, Any] = {"type": step_type}
        if element is not None:
            step["element"] = element
        if step_type == "set_text":
            text = raw_step.get("text")
            if not isinstance(text, str) or not text.strip():
                raise ValueError(f"Step {index} requires non-empty text.")
            if len(text) > _MAX_TEXT_CHARS:
                raise ValueError(
                    f"Step {index} text exceeds the {_MAX_TEXT_CHARS}-character limit."
                )
            _reject_sensitive_text(text, f"step {index}.text")
            step["text"] = text
        normalized_steps.append(step)

    summary = _as_short_string(raw_plan.get("summary"), "plan.summary")
    _reject_sensitive_text(summary, "plan.summary")
    return {
        "version": 1,
        "summary": summary,
        "target": target,
        "steps": normalized_steps,
    }


def queue_desktop_plan(plan: dict[str, Any]) -> ToolResult:
    """Persist one approval-gated plan without executing any operating-system input."""

    action = ApprovalStore().queue_action(
        action_type="desktop_automation_plan",
        description=f"Desktop plan: {plan['summary']}",
        payload=plan,
        permission_key="desktop_automation_plan:always_ask",
        tier=TIER_HIGH,
        ttl_hours=_PLAN_TTL_MINUTES / 60,
    )
    return ToolResult(
        tool_name="controlled_desktop_plan",
        success=True,
        content=(
            f"Desktop plan {action.id} is queued for local review. It expires in "
            f"{_PLAN_TTL_MINUTES} minutes and cannot send input until the native "
            "desktop broker validates a UI-approved plan."
        ),
        metadata={
            "action_id": action.id,
            "status": action.status,
            "requires_ui_approval": True,
        },
    )


@ToolRegistry.register("controlled_desktop_result")
class ControlledDesktopResultTool(BaseTool):
    """Read a redacted result of one desktop plan after the native broker ran it."""

    tool_id = "controlled_desktop_result"

    @property
    def spec(self) -> ToolSpec:
        return ToolSpec(
            name=self.tool_id,
            description=(
                "Read the locally redacted outcome of one consumed desktop plan. "
                "Use only the action ID returned by controlled_desktop_plan."
            ),
            parameters={
                "type": "object",
                "properties": {"action_id": {"type": "string"}},
                "required": ["action_id"],
            },
            category="controlled-desktop",
            required_capabilities=["desktop:controlled"],
        )

    def execute(self, **params: Any) -> ToolResult:
        if not _enabled():
            return ToolResult(
                tool_name=self.tool_id,
                content="Controlled desktop automation is disabled in this profile.",
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
                content="Desktop result denied: invalid action ID.",
                success=False,
            )
        action = ApprovalStore().get_action(action_id)
        if action is None or action.action_type != "desktop_automation_plan":
            return ToolResult(
                tool_name=self.tool_id,
                content="Desktop result is unavailable.",
                success=False,
            )
        execution = action.payload.get("execution")
        if not isinstance(execution, dict):
            return ToolResult(
                tool_name=self.tool_id,
                content="Desktop plan has not completed yet.",
                success=False,
                metadata={"status": action.status},
            )
        success = bool(execution.get("success"))
        summary = sanitize_model_output(
            str(execution.get("summary", "")),
            max_chars=4_000,
        )
        return ToolResult(
            tool_name=self.tool_id,
            content=summary or "Desktop plan completed without readable output.",
            success=success,
            metadata={"action_id": action.id, "status": action.status},
        )


@ToolRegistry.register("controlled_desktop_plan")
class ControlledDesktopPlanTool(BaseTool):
    """Allow the model to propose, never directly execute, a desktop UI plan."""

    tool_id = "controlled_desktop_plan"

    @property
    def spec(self) -> ToolSpec:
        return ToolSpec(
            name=self.tool_id,
            description=(
                "Propose a short, structured, low-risk Windows desktop UI plan. "
                "Do not use this for credentials, logins, banking, payments, account "
                "changes, purchases, recovery flows, elevated prompts, or sending data "
                "to third parties. The plan is reviewed locally and never executes "
                "directly from the model."
            ),
            parameters={
                "type": "object",
                "properties": {
                    "plan": {
                        "type": "object",
                        "description": (
                            "A structured target, summary, and 1-12 bounded UI steps."
                        ),
                    }
                },
                "required": ["plan"],
            },
            category="controlled-desktop",
            requires_confirmation=True,
            required_capabilities=["desktop:controlled"],
        )

    def execute(self, **params: Any) -> ToolResult:
        if not _enabled():
            return ToolResult(
                tool_name=self.tool_id,
                content="Controlled desktop automation is disabled in this profile.",
                success=False,
            )
        try:
            return queue_desktop_plan(validate_desktop_plan(params.get("plan")))
        except ValueError as exc:
            return ToolResult(
                tool_name=self.tool_id,
                content=f"Desktop plan denied: {exc}",
                success=False,
            )


__all__ = [
    "ControlledDesktopPlanTool",
    "ControlledDesktopResultTool",
    "queue_desktop_plan",
    "validate_desktop_plan",
]
