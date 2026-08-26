"""Regression coverage for the secure desktop tool-registration boundary."""

from __future__ import annotations

import importlib
import sys


def test_dangerous_tools_are_not_registered_without_opt_in(monkeypatch) -> None:
    """The default process must not expose local execution or write tools."""

    monkeypatch.delenv("OPENJARVIS_ENABLE_DANGEROUS_TOOLS", raising=False)
    for module_name in list(sys.modules):
        if module_name == "openjarvis.tools" or module_name.startswith(
            "openjarvis.tools."
        ):
            sys.modules.pop(module_name, None)

    import openjarvis.tools  # noqa: F401
    from openjarvis.core.registry import ToolRegistry

    blocked = {
        "shell_exec",
        "docker_shell_exec",
        "code_interpreter",
        "code_interpreter_docker",
        "repl",
        "file_write",
        "apply_patch",
        "git_tool",
    }
    assert blocked.isdisjoint(set(ToolRegistry.keys()))


def test_dangerous_tools_require_explicit_environment_opt_in(monkeypatch) -> None:
    """The deliberate opt-in remains narrow and auditable."""

    monkeypatch.delenv("OPENJARVIS_ENABLE_DANGEROUS_TOOLS", raising=False)
    tools_module = importlib.import_module("openjarvis.tools")
    assert tools_module._dangerous_tools_enabled() is False

    monkeypatch.setenv("OPENJARVIS_ENABLE_DANGEROUS_TOOLS", "1")
    assert tools_module._dangerous_tools_enabled() is True
