from __future__ import annotations

from openjarvis.tools import controlled_desktop


def _safe_plan() -> dict[str, object]:
    return {
        "summary": "Read the first public recipe result",
        "target": {
            "application": (
                r"C:\Program Files\Google\Chrome\Application\chrome.exe"
            ),
            "window_title": "Recipe search",
        },
        "steps": [
            {"type": "focus_window"},
            {
                "type": "invoke_element",
                "element": {
                    "name": "First recipe result",
                    "control_type": "Hyperlink",
                },
            },
            {
                "type": "read_accessible_text",
                "element": {"name": "Article", "control_type": "Text"},
            },
        ],
    }


def test_validate_desktop_plan_accepts_bounded_low_risk_ui_steps() -> None:
    plan = controlled_desktop.validate_desktop_plan(_safe_plan())

    assert plan["version"] == 1
    assert len(plan["steps"]) == 3
    assert plan["target"]["application"].endswith("chrome.exe")


def test_validate_desktop_plan_rejects_non_executable_window_identity() -> None:
    plan = _safe_plan()
    plan["target"]["application"] = "Google Chrome"

    try:
        controlled_desktop.validate_desktop_plan(plan)
    except ValueError as exc:
        assert "absolute Windows .exe path" in str(exc)
    else:
        raise AssertionError("Expected an unbound window target to be rejected")


def test_validate_desktop_plan_rejects_password_and_bank_flows() -> None:
    plan = _safe_plan()
    plan["summary"] = "Log in to my bank account"

    try:
        controlled_desktop.validate_desktop_plan(plan)
    except ValueError as exc:
        assert "login, financial, account, or credential" in str(exc)
    else:
        raise AssertionError("Expected a sensitive desktop plan to be rejected")


def test_validate_desktop_plan_rejects_password_like_text_input() -> None:
    plan = _safe_plan()
    plan["steps"] = [
        {
            "type": "set_text",
            "element": {"name": "Password", "control_type": "Edit"},
            "text": "not-a-real-secret",
        }
    ]

    try:
        controlled_desktop.validate_desktop_plan(plan)
    except ValueError as exc:
        assert "login, financial, account, or credential" in str(exc)
    else:
        raise AssertionError("Expected a password plan to be rejected")


def test_validate_desktop_plan_rejects_unbounded_input_actions() -> None:
    plan = _safe_plan()
    plan["steps"] = [{"type": "move_mouse"}]

    try:
        controlled_desktop.validate_desktop_plan(plan)
    except ValueError as exc:
        assert "unsupported desktop action" in str(exc)
    else:
        raise AssertionError("Expected global mouse input to be rejected")


def test_plan_tool_is_disabled_without_desktop_operator_opt_in(monkeypatch) -> None:
    monkeypatch.delenv("OPENJARVIS_ENABLE_CONTROLLED_DESKTOP_OPERATOR", raising=False)
    tool = controlled_desktop.ControlledDesktopPlanTool()

    result = tool.execute(plan=_safe_plan())

    assert not result.success
    assert "disabled" in result.content.lower()


def test_result_tool_returns_only_redacted_completed_output(
    monkeypatch,
    tmp_path,
) -> None:
    monkeypatch.setenv("OPENJARVIS_ENABLE_CONTROLLED_DESKTOP_OPERATOR", "1")
    store = controlled_desktop.ApprovalStore(str(tmp_path / "approvals.db"))
    monkeypatch.setattr(controlled_desktop, "ApprovalStore", lambda: store)
    try:
        action = store.queue_action(
            action_type="desktop_automation_plan",
            description="Read a public result",
            payload={
                "execution": {
                    "success": True,
                    "summary": "Found sk-test-token-should-be-redacted",
                }
            },
            permission_key="desktop_automation_plan:always_ask",
            tier="high",
            ttl_hours=1,
        )
        tool = controlled_desktop.ControlledDesktopResultTool()

        result = tool.execute(action_id=action.id)

        assert result.success
        assert "sk-test-token-should-be-redacted" not in result.content
        assert "[REDACTED" in result.content
    finally:
        store.close()
