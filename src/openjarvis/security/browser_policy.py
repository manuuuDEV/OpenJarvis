"""Fail-closed browser interaction policy for the secure desktop profile.

This module intentionally classifies only the high-risk browser operations that
must never be delegated to a cloud model.  It does not replace the existing
SSRF check, and it does not grant approval: sensitive interactions are blocked
rather than retried through a different browser path.
"""

from __future__ import annotations

import os
import re
from urllib.parse import urlsplit

_SENSITIVE_BROWSER_TERMS = re.compile(
    r"\b("
    r"login|log[ -]?in|sign[ -]?in|sign[ -]?up|password|passcode|"
    r"otp|one[ -]?time[ -]?code|verification[ -]?code|two[ -]?factor|mfa|"
    r"account[ -]?recovery|recover[ -]?account|credential|"
    r"bank|banking|wallet|payment|pay|checkout|purchase|buy[ -]?now|"
    r"order[ -]?now|transfer|withdraw|deposit|billing|card[ -]?number|"
    r"cvv|cvc|iban|swift|send|submit|publish|post|share|delete|remove"
    r")\b",
    re.IGNORECASE,
)

_SUSPICIOUS_URL_QUERY_TERMS = re.compile(
    r"(?:^|[?&])(token|access_token|api[_-]?key|password|passcode|otp|"
    r"verification[_-]?code|session|credential)=",
    re.IGNORECASE,
)

_LINK_SHORTENER_HOSTS = frozenset(
    {"bit.ly", "t.co", "tinyurl.com", "goo.gl", "is.gd", "cutt.ly", "rebrand.ly"}
)

_SENSITIVE_FIELD_TERMS = re.compile(
    r"\b("
    r"password|passcode|otp|verification|two[ -]?factor|mfa|"
    r"email|e-mail|username|user[ -]?name|account|credential|"
    r"card|billing|payment|bank|iban|swift|address|phone|ssn|tax"
    r")\b",
    re.IGNORECASE,
)


def secure_desktop_browser_policy_enabled() -> bool:
    """Return whether browser interactions are constrained for secure desktop."""

    return os.getenv("OPENJARVIS_SECURE_DESKTOP_PROFILE", "").strip() == "1"


def blocked_navigation_reason(url: str) -> str | None:
    """Return a reason when secure desktop forbids navigation to *url*."""

    if not secure_desktop_browser_policy_enabled():
        return None
    parsed = urlsplit(url.strip())
    if parsed.scheme.lower() != "https" or not parsed.netloc:
        return "Secure desktop browser navigation requires a public HTTPS URL."
    return None


def blocked_suspicious_url_reason(url: str) -> str | None:
    """Return a reason for a structurally risky URL in secure desktop mode.

    This is a local preflight, not a malware or reputation verdict. It never
    sends the URL to a third party; SSRF validation remains a separate guard.
    """

    if not secure_desktop_browser_policy_enabled():
        return None
    parsed = urlsplit(url.strip())
    host = (parsed.hostname or "").lower().rstrip(".")
    if parsed.username or parsed.password:
        return "URLs containing embedded credentials are blocked."
    if _SUSPICIOUS_URL_QUERY_TERMS.search(parsed.query):
        return "URLs containing token, password, or session query values are blocked."
    if host.startswith("xn--") or ".xn--" in host:
        return "Internationalized/punycode hostnames require manual verification."
    if host in _LINK_SHORTENER_HOSTS:
        return "Link shorteners require manual verification before navigation."
    return None


def blocked_click_reason(selector: str) -> str | None:
    """Return a reason for an unsafe user-visible click target, if any."""

    if not secure_desktop_browser_policy_enabled():
        return None
    if _SENSITIVE_BROWSER_TERMS.search(selector):
        return (
            "Secure desktop browser policy blocks login, credential, payment, "
            "submission, publishing, deletion, and account actions."
        )
    return None


def blocked_text_entry_reason(selector: str, text: str) -> str | None:
    """Return a reason for an unsafe form entry, if any.

    The guard deliberately blocks identity-bearing field names as well as
    credential and payment fields. Search, documentation, and ordinary public
    form fields remain usable.
    """

    if not secure_desktop_browser_policy_enabled():
        return None
    if _SENSITIVE_FIELD_TERMS.search(selector) or _SENSITIVE_BROWSER_TERMS.search(text):
        return (
            "Secure desktop browser policy blocks entry into account, identity, "
            "credential, verification, and payment fields."
        )
    return None
