"""Tests for the Groq Whisper cloud speech backend."""

from __future__ import annotations

import pytest

from openjarvis.core.registry import SpeechRegistry
from openjarvis.speech._stubs import TranscriptionResult
from openjarvis.speech.groq_whisper import GroqWhisperBackend


@pytest.fixture(autouse=True)
def _register_groq_whisper():
    """Re-register the backend after registry-mutating test modules."""
    if not SpeechRegistry.contains("groq-whisper"):
        SpeechRegistry.register_value("groq-whisper", GroqWhisperBackend)


class _FakeResponse:
    def raise_for_status(self) -> None:
        return None

    def json(self) -> dict[str, object]:
        return {"text": "Ciao", "language": "it", "duration": 1.25}


class _FakeClient:
    def __init__(self, **kwargs) -> None:
        self.kwargs = kwargs
        self.request: dict[str, object] | None = None

    def __enter__(self):
        return self

    def __exit__(self, *_args) -> None:
        return None

    def post(self, url: str, **kwargs):
        self.request = {"url": url, **kwargs}
        return _FakeResponse()


def test_groq_whisper_registers() -> None:
    assert SpeechRegistry.contains("groq-whisper")


def test_groq_whisper_transcribes_with_openai_compatible_endpoint(monkeypatch) -> None:
    fake_client = _FakeClient()
    monkeypatch.setattr(
        "openjarvis.speech.groq_whisper.httpx.Client",
        lambda **_kwargs: fake_client,
    )
    backend = GroqWhisperBackend(api_key="test-only-value")

    result = backend.transcribe(b"audio", format="webm", language="it")

    assert isinstance(result, TranscriptionResult)
    assert result.text == "Ciao"
    assert result.language == "it"
    assert result.duration_seconds == 1.25
    assert fake_client.request is not None
    assert fake_client.request["url"] == "https://api.groq.com/openai/v1/audio/transcriptions"
    assert fake_client.request["data"] == {
        "model": "whisper-large-v3-turbo",
        "response_format": "verbose_json",
        "temperature": "0",
        "language": "it",
    }


def test_groq_whisper_rejects_unsupported_format() -> None:
    backend = GroqWhisperBackend(api_key="test-only-value")

    with pytest.raises(RuntimeError, match="Unsupported audio format"):
        backend.transcribe(b"audio", format="exe")


def test_groq_whisper_requires_a_key() -> None:
    backend = GroqWhisperBackend(api_key="")

    assert backend.health() is False
    with pytest.raises(RuntimeError, match="not configured"):
        backend.transcribe(b"audio")
