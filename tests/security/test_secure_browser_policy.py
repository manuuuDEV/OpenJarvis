"""Regression tests for the secure desktop browser interaction boundary."""

from __future__ import annotations

from openjarvis.security.browser_policy import (
    blocked_click_reason,
    blocked_navigation_reason,
    blocked_suspicious_url_reason,
    blocked_text_entry_reason,
)


def test_secure_browser_policy_allows_https_reading(monkeypatch) -> None:
    monkeypatch.setenv("OPENJARVIS_SECURE_DESKTOP_PROFILE", "1")

    assert blocked_navigation_reason("https://www.example.com/docs") is None
    assert blocked_click_reason("Read documentation") is None
    assert blocked_text_entry_reason("input[name='q']", "come fare una pizza") is None


def test_secure_browser_policy_blocks_non_https_navigation(monkeypatch) -> None:
    monkeypatch.setenv("OPENJARVIS_SECURE_DESKTOP_PROFILE", "1")

    reason = blocked_navigation_reason("http://example.com")

    assert reason is not None
    assert "HTTPS" in reason


def test_secure_browser_policy_blocks_structurally_risky_links(monkeypatch) -> None:
    monkeypatch.setenv("OPENJARVIS_SECURE_DESKTOP_PROFILE", "1")

    assert blocked_suspicious_url_reason("https://user:secret@example.com") is not None
    assert (
        blocked_suspicious_url_reason("https://example.com/?access_token=value")
        is not None
    )
    assert blocked_suspicious_url_reason("https://xn--exampl-ova.example") is not None
    assert blocked_suspicious_url_reason("https://bit.ly/example") is not None
    assert (
        blocked_suspicious_url_reason("https://www.example.com/documentation") is None
    )


def test_secure_browser_policy_blocks_sensitive_browser_actions(monkeypatch) -> None:
    monkeypatch.setenv("OPENJARVIS_SECURE_DESKTOP_PROFILE", "1")

    assert blocked_click_reason("Sign in") is not None
    assert blocked_click_reason("Complete payment") is not None
    assert blocked_click_reason("Publish post") is not None
    assert blocked_text_entry_reason("input[type='password']", "secret") is not None
    assert (
        blocked_text_entry_reason("input[name='email']", "user@example.com") is not None
    )


def test_secure_browser_policy_is_not_applied_outside_desktop_profile(
    monkeypatch,
) -> None:
    monkeypatch.delenv("OPENJARVIS_SECURE_DESKTOP_PROFILE", raising=False)

    assert blocked_navigation_reason("http://example.com") is None
    assert blocked_click_reason("Sign in") is None
    assert blocked_text_entry_reason("input[name='email']", "user@example.com") is None
