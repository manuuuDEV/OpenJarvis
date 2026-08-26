from __future__ import annotations

from openjarvis.tools import controlled_android_adb


def test_adb_diagnostic_tool_is_disabled_without_native_opt_in(monkeypatch) -> None:
    monkeypatch.delenv("OPENJARVIS_ENABLE_CONTROLLED_ANDROID_ADB", raising=False)

    result = controlled_android_adb.ControlledAndroidAdbDiagnosticTool().execute(
        summary="Check Android software health"
    )

    assert not result.success
    assert "disabled" in result.content.lower()


def test_adb_diagnostic_queues_only_a_bounded_read_only_request(
    monkeypatch, tmp_path
) -> None:
    monkeypatch.setenv("OPENJARVIS_ENABLE_CONTROLLED_ANDROID_ADB", "1")
    store = controlled_android_adb.ApprovalStore(str(tmp_path / "approvals.db"))
    monkeypatch.setattr(controlled_android_adb, "ApprovalStore", lambda: store)
    try:
        result = controlled_android_adb.ControlledAndroidAdbDiagnosticTool().execute(
            summary="Check available storage and Android software health"
        )

        assert result.success
        action = store.get_action(result.metadata["action_id"])
        assert action is not None
        assert action.action_type == "android_adb_software_diagnostic"
        assert action.payload == {
            "version": 1,
            "summary": "Check available storage and Android software health",
        }
        assert action.tier == "high"
    finally:
        store.close()


def test_adb_diagnostic_rejects_sensitive_request_content(monkeypatch) -> None:
    monkeypatch.setenv("OPENJARVIS_ENABLE_CONTROLLED_ANDROID_ADB", "1")

    result = controlled_android_adb.ControlledAndroidAdbDiagnosticTool().execute(
        summary="Inspect my password manager"
    )

    assert not result.success
    assert "sensitive" in result.content.lower()


def test_adb_result_tool_returns_only_redacted_completed_output(
    monkeypatch, tmp_path
) -> None:
    monkeypatch.setenv("OPENJARVIS_ENABLE_CONTROLLED_ANDROID_ADB", "1")
    store = controlled_android_adb.ApprovalStore(str(tmp_path / "approvals.db"))
    monkeypatch.setattr(controlled_android_adb, "ApprovalStore", lambda: store)
    try:
        action = store.queue_action(
            action_type="android_adb_software_diagnostic",
            description="Android diagnostic",
            payload={
                "execution": {
                    "success": True,
                    "summary": (
                        "Device report includes sk-test-token-should-be-redacted"
                    ),
                }
            },
            permission_key="android_adb_software_diagnostic:always_ask",
            tier="high",
            ttl_hours=1,
        )

        result = controlled_android_adb.ControlledAndroidAdbResultTool().execute(
            action_id=action.id
        )

        assert result.success
        assert "sk-test-token-should-be-redacted" not in result.content
        assert "[REDACTED" in result.content
    finally:
        store.close()
