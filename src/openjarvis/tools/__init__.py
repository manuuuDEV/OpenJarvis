"""Tools primitive — tool system with ABC interface and built-in tools."""

from __future__ import annotations

import os

from openjarvis.tools._stubs import BaseTool, ToolExecutor, ToolSpec


def _controlled_local_actions_enabled() -> bool:
    """Return whether the desktop explicitly enables bounded local actions."""

    return os.getenv("OPENJARVIS_ENABLE_CONTROLLED_LOCAL_ACTIONS", "").strip() == "1"


def _controlled_desktop_operator_enabled() -> bool:
    """Return whether the secure Windows desktop enables its local UI broker."""

    return os.getenv("OPENJARVIS_ENABLE_CONTROLLED_DESKTOP_OPERATOR", "").strip() == "1"


def _controlled_android_adb_enabled() -> bool:
    """Return whether native desktop explicitly enables the bounded ADB broker."""

    return os.getenv("OPENJARVIS_ENABLE_CONTROLLED_ANDROID_ADB", "").strip() == "1"


def _dangerous_tools_enabled() -> bool:
    """Return whether a trusted local operator explicitly enabled risky tools.

    The secure desktop profile is fail-closed: cloud inference may not expose
    local command execution, code evaluation, container access, or write tools
    unless the operator starts the process with this dedicated opt-in.
    """

    value = os.getenv("OPENJARVIS_ENABLE_DANGEROUS_TOOLS", "").strip().lower()
    return value in {"1", "true", "yes"}


# Import built-in tools to trigger @ToolRegistry.register() decorators.
# Each is wrapped in try/except so the package loads even before the
# individual tool modules are created.
try:
    import openjarvis.tools.calculator  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.think  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.retrieval  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.llm_tool  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.file_read  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.web_search  # noqa: F401
except ImportError:
    pass

if _dangerous_tools_enabled():
    try:
        import openjarvis.tools.code_interpreter  # noqa: F401
        import openjarvis.tools.code_interpreter_docker  # noqa: F401
        import openjarvis.tools.repl  # noqa: F401
    except ImportError:
        pass

try:
    import openjarvis.tools.storage_tools  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.mcp_adapter  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.channel_tools  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.http_request  # noqa: F401
except ImportError:
    pass

if _dangerous_tools_enabled():
    try:
        import openjarvis.tools.docker_shell_exec  # noqa: F401
        import openjarvis.tools.shell_exec  # noqa: F401
    except ImportError:
        pass

try:
    import openjarvis.tools.memory_manage  # noqa: F401
except ImportError:
    pass
try:
    import openjarvis.tools.user_profile_manage  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.skill_manage  # noqa: F401
except ImportError:
    pass

if _controlled_local_actions_enabled():
    try:
        import openjarvis.tools.controlled_local  # noqa: F401
    except ImportError:
        pass

if _controlled_desktop_operator_enabled():
    try:
        import openjarvis.tools.controlled_desktop  # noqa: F401
    except ImportError:
        pass

if _controlled_android_adb_enabled():
    try:
        import openjarvis.tools.controlled_android_adb  # noqa: F401
    except ImportError:
        pass

if _dangerous_tools_enabled():
    try:
        import openjarvis.tools.apply_patch  # noqa: F401
        import openjarvis.tools.file_write  # noqa: F401
        import openjarvis.tools.git_tool  # noqa: F401
    except ImportError:
        pass

try:
    import openjarvis.tools.db_query  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.pdf_tool  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.image_tool  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.audio_tool  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.knowledge_tools  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.text_to_speech  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.digest_collect  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.scan_chunks  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.knowledge_sql  # noqa: F401
except ImportError:
    pass

try:
    import openjarvis.tools.apple_calendar  # noqa: F401
except ImportError:
    pass

__all__ = ["BaseTool", "ToolExecutor", "ToolSpec"]
