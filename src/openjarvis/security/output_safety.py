"""Fail-closed sanitization for model output, approval previews, and local logs.

The cloud model is untrusted for presentation purposes: it can accidentally
repeat a key present in context or render hostile control characters.  This
module redacts common credential shapes before text leaves the local backend.
It is not a substitute for keeping secrets out of prompts.
"""

from __future__ import annotations

import hashlib
import re
from typing import Any

_MAX_OUTPUT_CHARS = 64_000
_STREAM_HOLD_BACK_CHARS = 512
_REDACTED = "[REDACTED]"
_SENSITIVE_FIELD_NAMES = frozenset(
    {
        "api_key",
        "apikey",
        "authorization",
        "credential",
        "credentials",
        "content",
        "date_of_birth",
        "email",
        "fiscal_code",
        "iban",
        "national_id",
        "password",
        "secret",
        "tax_id",
        "token",
        "phone",
        "phone_number",
    }
)

_PRIVATE_KEY = re.compile(
    (
        r"-----BEGIN(?: [A-Z0-9]+)? PRIVATE KEY-----.*?"
        r"-----END(?: [A-Z0-9]+)? PRIVATE KEY-----"
    ),
    re.DOTALL,
)
_SECRET_PATTERNS: tuple[tuple[re.Pattern[str], str], ...] = (
    (_PRIVATE_KEY, "[REDACTED_PRIVATE_KEY]"),
    (re.compile(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{16,}\b"), "[REDACTED_OPENAI_KEY]"),
    (re.compile(r"\bsk-ant-[A-Za-z0-9_-]{16,}\b"), "[REDACTED_ANTHROPIC_KEY]"),
    (re.compile(r"\bAIza[0-9A-Za-z_-]{20,}\b"), "[REDACTED_GOOGLE_KEY]"),
    (
        re.compile(r"\b(?:ghp|github_pat)_[A-Za-z0-9_]{16,}\b"),
        "[REDACTED_GITHUB_TOKEN]",
    ),
    (re.compile(r"\bglpat-[A-Za-z0-9_-]{16,}\b"), "[REDACTED_GITLAB_TOKEN]"),
    (re.compile(r"\bxox[baprs]-[A-Za-z0-9-]{16,}\b"), "[REDACTED_SLACK_TOKEN]"),
    (re.compile(r"\bAKIA[0-9A-Z]{16}\b"), "[REDACTED_AWS_ACCESS_KEY]"),
    (
        re.compile(r"(?i)\b[A-Z]{2}\d{2}(?:[ ]?[A-Z0-9]){11,30}\b"),
        "[REDACTED_IBAN]",
    ),
    (
        re.compile(r"\b[A-Z]{6}\d{2}[A-Z]\d{2}[A-Z]\d{3}[A-Z]\b", re.I),
        "[REDACTED_FISCAL_CODE]",
    ),
    (
        re.compile(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b"),
        "[REDACTED_EMAIL]",
    ),
    (
        re.compile(r"\b(?:\d[ -]?){13,19}\d\b"),
        "[REDACTED_PAYMENT_NUMBER]",
    ),
    (
        re.compile(r"(?<!\w)(?:\+?\d[ .-]?){9,15}\d(?!\w)"),
        "[REDACTED_PHONE]",
    ),
    (
        re.compile(
            r"(?i)\b(api[_-]?key|access[_-]?token|auth(?:orization)?|password|secret)"
            r"\s*[:=]\s*(['\"]?)[^\s,'\";]{8,}\2"
        ),
        r"\1=[REDACTED]",
    ),
)
_CONTROL_CHARS = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]")


def sanitize_model_output(value: str, *, max_chars: int = _MAX_OUTPUT_CHARS) -> str:
    """Return display-safe model text with credentials redacted and bounded."""

    text = str(value or "")
    text = _CONTROL_CHARS.sub("", text)
    for pattern, replacement in _SECRET_PATTERNS:
        text = pattern.sub(replacement, text)
    if len(text) > max_chars:
        text = text[:max_chars] + "\n[OUTPUT_TRUNCATED_FOR_SAFETY]"
    return text


def sanitize_payload_for_display(payload: Any) -> Any:
    """Return an approval-safe payload while retaining only reviewable metadata.

    Stored actions keep their full payload for the already approved local
    executor. The UI and API preview receive this transformed structure, so a
    write request cannot disclose its complete content or a credential.
    """

    if isinstance(payload, dict):
        display: dict[str, Any] = {}
        for key, value in payload.items():
            normalized = str(key).lower().replace("-", "_")
            if normalized in _SENSITIVE_FIELD_NAMES:
                encoded = str(value).encode("utf-8", errors="replace")
                display[key] = {
                    "redacted": True,
                    "length": len(encoded),
                    "sha256": hashlib.sha256(encoded).hexdigest(),
                }
            else:
                display[key] = sanitize_payload_for_display(value)
        return display
    if isinstance(payload, list):
        return [sanitize_payload_for_display(item) for item in payload]
    if isinstance(payload, str):
        return sanitize_model_output(payload, max_chars=2_000)
    return payload


class StreamingOutputSanitizer:
    """Sanitize an SSE response while retaining a tail for multi-token secrets."""

    def __init__(self, hold_back_chars: int = _STREAM_HOLD_BACK_CHARS) -> None:
        self._hold_back_chars = max(64, hold_back_chars)
        self._pending = ""

    def push(self, token: str) -> str:
        """Accept one raw token and return only the safe portion ready to emit."""

        self._pending += str(token or "")
        if len(self._pending) <= self._hold_back_chars:
            return ""
        ready = self._pending[: -self._hold_back_chars]
        self._pending = self._pending[-self._hold_back_chars :]
        return sanitize_model_output(ready)

    def finalize(self) -> str:
        """Return the final redacted tail once upstream streaming completes."""

        ready, self._pending = self._pending, ""
        return sanitize_model_output(ready)


__all__ = [
    "StreamingOutputSanitizer",
    "sanitize_model_output",
    "sanitize_payload_for_display",
]
