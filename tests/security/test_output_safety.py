"""Regression tests for display-safe model output and approval previews."""

from __future__ import annotations

from openjarvis.security.output_safety import (
    StreamingOutputSanitizer,
    sanitize_model_output,
    sanitize_payload_for_display,
)


def test_model_output_redacts_common_provider_credentials() -> None:
    raw = (
        "OpenAI sk-proj-abcdefghijklmnopqrstuvwxyz123456 and "
        "GitHub ghp_abcdefghijklmnopqrstuvwxyz123456"
    )

    safe = sanitize_model_output(raw)

    assert "sk-proj-" not in safe
    assert "ghp_" not in safe
    assert "REDACTED_OPENAI_KEY" in safe
    assert "REDACTED_GITHUB_TOKEN" in safe


def test_model_output_removes_private_keys_and_control_characters() -> None:
    raw = (
        "before\x00-----BEGIN PRIVATE KEY-----\nsecret\n"
        "-----END PRIVATE KEY-----\x1bafter"
    )

    safe = sanitize_model_output(raw)

    assert "secret" not in safe
    assert "REDACTED_PRIVATE_KEY" in safe
    assert "\x00" not in safe
    assert "\x1b" not in safe


def test_approval_preview_redacts_content_but_retains_review_metadata() -> None:
    payload = {
        "path": "C:/Users/Alice/OpenJarvis-Workspace/note.txt",
        "content": "top secret",
    }

    safe = sanitize_payload_for_display(payload)

    assert safe["path"].endswith("note.txt")
    assert safe["content"]["redacted"] is True
    assert safe["content"]["length"] == len("top secret")
    assert "top secret" not in str(safe)


def test_streaming_sanitizer_holds_output_until_safe_tail_is_finalized() -> None:
    sanitizer = StreamingOutputSanitizer(hold_back_chars=64)

    assert sanitizer.push("sk-proj-abcdefghijklmnopqrstuvwxyz123456") == ""
    final = sanitizer.finalize()

    assert "sk-proj-" not in final
    assert "REDACTED_OPENAI_KEY" in final


def test_model_output_redacts_common_personal_data() -> None:
    raw = (
        "email alice.rossi@example.it, telefono +39 333 123 4567, "
        "IBAN IT60X0542811101000000123456, "
        "codice fiscale RSSMRA85T10A562S, "
        "carta 4111 1111 1111 1111"
    )

    safe = sanitize_model_output(raw)

    assert "alice.rossi@example.it" not in safe
    assert "+39 333 123 4567" not in safe
    assert "IT60X0542811101000000123456" not in safe
    assert "RSSMRA85T10A562S" not in safe
    assert "4111 1111 1111 1111" not in safe
    assert "[REDACTED_EMAIL]" in safe
    assert "[REDACTED_PHONE]" in safe
    assert "[REDACTED_IBAN]" in safe
    assert "[REDACTED_FISCAL_CODE]" in safe
    assert "[REDACTED_PAYMENT_NUMBER]" in safe


def test_approval_preview_redacts_personal_data_fields() -> None:
    safe = sanitize_payload_for_display(
        {"email": "alice.rossi@example.it", "phone": "+393331234567"}
    )

    assert safe["email"]["redacted"] is True
    assert safe["phone"]["redacted"] is True
    assert "alice.rossi@example.it" not in str(safe)
    assert "+393331234567" not in str(safe)
