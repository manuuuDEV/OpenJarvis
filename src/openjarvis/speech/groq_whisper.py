"""Groq Whisper speech-to-text backend (cloud, OpenAI-compatible)."""

from __future__ import annotations

from typing import Optional

import httpx

from openjarvis.core.registry import SpeechRegistry
from openjarvis.speech._stubs import SpeechBackend, TranscriptionResult

_GROQ_TRANSCRIPTION_URL = "https://api.groq.com/openai/v1/audio/transcriptions"
_SUPPORTED_FORMATS = {"flac", "m4a", "mp3", "mp4", "mpeg", "mpga", "ogg", "wav", "webm"}


@SpeechRegistry.register("groq-whisper")
class GroqWhisperBackend(SpeechBackend):
    """Cloud transcription through Groq Whisper with no local speech model."""

    backend_id = "groq-whisper"

    def __init__(self, *, api_key: str, model: str = "whisper-large-v3-turbo") -> None:
        self._api_key = api_key.strip()
        self._model = model.strip() or "whisper-large-v3-turbo"
        self._last_error: Optional[str] = None

    def transcribe(
        self,
        audio: bytes,
        *,
        format: str = "wav",
        language: Optional[str] = None,
    ) -> TranscriptionResult:
        if not self._api_key:
            raise RuntimeError("Groq speech key is not configured")
        normalized_format = format.lower().lstrip(".")
        if normalized_format not in _SUPPORTED_FORMATS:
            raise RuntimeError("Unsupported audio format for Groq transcription")
        if not audio:
            raise RuntimeError("Audio is empty")
        if len(audio) > 25 * 1024 * 1024:
            raise RuntimeError("Audio exceeds the configured Groq upload limit")

        form: dict[str, str] = {
            "model": self._model,
            "response_format": "verbose_json",
            "temperature": "0",
        }
        if language:
            form["language"] = language
        try:
            with httpx.Client(timeout=90) as client:
                response = client.post(
                    _GROQ_TRANSCRIPTION_URL,
                    headers={"Authorization": f"Bearer {self._api_key}"},
                    data=form,
                    files={"file": (f"recording.{normalized_format}", audio)},
                )
                response.raise_for_status()
                data = response.json()
        except httpx.HTTPError as exc:
            self._last_error = exc.__class__.__name__
            raise RuntimeError("Groq transcription request failed") from exc

        self._last_error = None
        return TranscriptionResult(
            text=str(data.get("text", "")).strip(),
            language=data.get("language"),
            duration_seconds=float(data.get("duration", 0.0) or 0.0),
        )

    def health(self) -> bool:
        return bool(self._api_key)

    def last_error(self) -> Optional[str]:
        return self._last_error

    def supported_formats(self) -> list[str]:
        return sorted(_SUPPORTED_FORMATS)
