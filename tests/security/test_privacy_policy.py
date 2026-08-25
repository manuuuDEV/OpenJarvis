"""Tests for the outbound inference privacy boundary."""

from __future__ import annotations

import pytest

from openjarvis.security.privacy import (
    PrivacyPolicy,
    PrivacyPolicyError,
    is_loopback_endpoint,
)


class TestPrivacyPolicy:
    def test_local_only_blocks_external_provider(self) -> None:
        policy = PrivacyPolicy()
        with pytest.raises(PrivacyPolicyError, match="local_only"):
            policy.require_provider("openai")

    def test_local_only_allows_loopback_endpoint(self) -> None:
        PrivacyPolicy().require_endpoint("ollama", "http://127.0.0.1:11434")

    def test_explicit_external_requires_allowlisted_provider(self) -> None:
        policy = PrivacyPolicy(
            mode="explicit_external",
            approved_external_providers=("openai",),
        )
        policy.require_provider("openai")
        with pytest.raises(PrivacyPolicyError, match="not in"):
            policy.require_provider("anthropic")

    def test_explicit_external_requires_https(self) -> None:
        policy = PrivacyPolicy(
            mode="explicit_external",
            approved_external_providers=("openai",),
        )
        with pytest.raises(PrivacyPolicyError, match="HTTPS"):
            policy.require_endpoint("openai", "http://api.example.test/v1")

    def test_confidential_compute_fails_closed_for_generic_api(self) -> None:
        policy = PrivacyPolicy(
            mode="confidential_compute",
            approved_external_providers=("openai",),
        )
        with pytest.raises(PrivacyPolicyError, match="remote attestation"):
            policy.require_provider("openai")

    @pytest.mark.parametrize(
        ("endpoint", "expected"),
        [
            ("http://localhost:11434", True),
            ("http://127.0.0.1:8000", True),
            ("http://[::1]:8000", True),
            ("https://api.openai.com/v1", False),
            ("http://192.168.1.10:8000", False),
        ],
    )
    def test_loopback_detection(self, endpoint: str, expected: bool) -> None:
        assert is_loopback_endpoint(endpoint) is expected
