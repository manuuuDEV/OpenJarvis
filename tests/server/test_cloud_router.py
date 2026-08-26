"""Regression tests for OpenRouter model ID normalization."""

from __future__ import annotations

import pytest

from openjarvis.core.types import Message
from openjarvis.server import cloud_router


def test_get_provider_detects_bare_openrouter_id():
    assert cloud_router.get_provider("anthropic/claude-haiku-4.5") == "openrouter"


def test_get_provider_detects_litellm_prefixed_openrouter_id():
    model = "openrouter/anthropic/claude-haiku-4.5"
    assert cloud_router.get_provider(model) == "openrouter"


@pytest.mark.parametrize(
    "requested_model,expected_forwarded_model",
    [
        ("anthropic/claude-haiku-4.5", "anthropic/claude-haiku-4.5"),
        ("openrouter/anthropic/claude-haiku-4.5", "anthropic/claude-haiku-4.5"),
        ("openrouter/auto", "openrouter/auto"),
    ],
)
@pytest.mark.asyncio
async def test_stream_cloud_normalizes_openrouter_model_before_forwarding(
    monkeypatch, requested_model, expected_forwarded_model
):
    monkeypatch.setenv("OPENROUTER_API_KEY", "test-key")
    captured: dict[str, str] = {}

    async def fake_stream_openai(model, messages, temperature, max_tokens, **kwargs):
        captured["model"] = model
        yield "ok"

    monkeypatch.setattr(cloud_router, "_stream_openai", fake_stream_openai)

    tokens = [
        token
        async for token in cloud_router.stream_cloud(
            requested_model, [Message(role="user", content="hi")]
        )
    ]

    assert tokens == ["ok"]
    assert captured["model"] == expected_forwarded_model


def test_secure_desktop_never_infers_provider_from_model(monkeypatch):
    monkeypatch.setenv("OPENJARVIS_SECURE_DESKTOP_PROFILE", "1")
    monkeypatch.delenv("OPENJARVIS_CLOUD_PROVIDER", raising=False)

    assert cloud_router.get_provider("gemini-3.1-flash") is None
    assert cloud_router.get_provider("meta-llama/llama-4") is None


def test_secure_desktop_accepts_only_visible_explicit_provider(monkeypatch):
    monkeypatch.setenv("OPENJARVIS_SECURE_DESKTOP_PROFILE", "1")
    monkeypatch.setenv("OPENJARVIS_CLOUD_PROVIDER", "pollinations")
    assert cloud_router.get_provider("any-model-id") == "pollinations"

    monkeypatch.setenv("OPENJARVIS_CLOUD_PROVIDER", "anthropic")
    assert cloud_router.get_provider("claude-sonnet") is None


@pytest.mark.asyncio
async def test_pollinations_uses_canonical_https_endpoint(monkeypatch):
    monkeypatch.setenv("OPENJARVIS_SECURE_DESKTOP_PROFILE", "1")
    monkeypatch.setenv("OPENJARVIS_CLOUD_PROVIDER", "pollinations")
    captured: dict[str, str] = {}

    async def fake_stream_openai(model, messages, temperature, max_tokens, **kwargs):
        captured["base_url"] = kwargs["base_url"]
        captured["api_key_name"] = kwargs["api_key_name"]
        yield "ok"

    monkeypatch.setattr(cloud_router, "_stream_openai", fake_stream_openai)
    tokens = [
        token
        async for token in cloud_router.stream_cloud(
            "openai", [Message(role="user", content="hi")]
        )
    ]

    assert tokens == ["ok"]
    assert captured == {
        "base_url": "https://gen.pollinations.ai",
        "api_key_name": "POLLINATIONS_API_KEY",
    }
