from __future__ import annotations

from pathlib import Path

from openjarvis.core.config import MemoryFilesConfig, SystemPromptConfig


def test_base_agent_uses_builder(tmp_path: Path):
    soul = tmp_path / "SOUL.md"
    soul.write_text("I am Jarvis.")
    memory = tmp_path / "MEMORY.md"
    memory.write_text("- User likes Python")

    from openjarvis.prompt.builder import SystemPromptBuilder

    builder = SystemPromptBuilder(
        agent_template="You are a helpful assistant.",
        memory_files_config=MemoryFilesConfig(
            soul_path=str(soul),
            memory_path=str(memory),
            user_path=str(tmp_path / "USER.md"),
        ),
        system_prompt_config=SystemPromptConfig(),
    )
    prompt = builder.build()
    assert "Jarvis" in prompt
    assert "Python" in prompt
    assert "helpful assistant" in prompt


def test_builder_includes_non_optional_secure_desktop_policy() -> None:
    from openjarvis.prompt.builder import SystemPromptBuilder

    prompt = SystemPromptBuilder(agent_template="").build()

    assert "Secure Desktop Operating Rules" in prompt
    assert "Never bypass the local approval UI" in prompt
    assert "Do not use shell commands" in prompt


def test_secure_desktop_policy_limits_android_adb_to_read_only_diagnostics() -> None:
    from openjarvis.prompt.builder import SystemPromptBuilder

    prompt = SystemPromptBuilder(agent_template="").build()

    assert "controlled_android_adb_diagnostic" in prompt
    assert "arbitrary ADB shell commands" in prompt
    assert "one-time local approval" in prompt


def test_secure_desktop_policy_limits_browser_and_gemini_live() -> None:
    from openjarvis.prompt.builder import SystemPromptBuilder

    prompt = SystemPromptBuilder(agent_template="").build()

    assert "Browser reading is limited to public HTTPS navigation" in prompt
    assert "Gemini Live is a separate user-started audio session only" in prompt
    assert "it cannot call tools" in prompt
