"""agent-harness-adapter for Python: one agent setup, every coding agent.

Install instructions, skills and hooks, kept once in ``AGENTS.md`` and
``.agents/`` (the config) or declared in a tool's manifest, into Claude Code,
Codex, Gemini CLI, GitHub Copilot, Cursor, Factory Droid, Pi, OpenCode, Kilo
Code, Qwen Code, Devin and any agent that reads AGENTS.md. Hooks speak one
format in every harness, Claude Code's (``parse_hook`` / ``answer_hook`` /
``run_hook``). Errors raise ValueError with the library's message.
"""

# @zen-component: BND-Python

import json
import os
from typing import Any, Dict, Iterable, Optional, Union

from . import _native

__all__ = [
    "install",
    "status",
    "uninstall",
    "parse_hook",
    "answer_hook",
    "run_hook",
    "schema",
    "MANIFEST_VERSION",
]

__version__: str = _native.__version__
MANIFEST_VERSION: int = _native.MANIFEST_VERSION

Manifest = Union[str, "os.PathLike[str]", Dict[str, Any]]
PathLike = Union[str, "os.PathLike[str]", None]


def _source(manifest: Optional[Manifest]):
    """(path, json) for the native call: a path, a manifest as a dict, or
    neither: the config."""
    if manifest is None:
        return None, None
    if isinstance(manifest, dict):
        return None, json.dumps(manifest)
    return os.fspath(manifest), None


def _options(harnesses, scope, root, home, force=False, without=None) -> str:
    def path(p):
        return None if p is None else os.fspath(p)

    return json.dumps(
        {
            "harnesses": list(harnesses or []),
            "scope": scope,
            "root": path(root),
            "home": path(home),
            "force": force,
            "without": None if without is None else list(without),
        }
    )


def install(
    manifest: Optional[Manifest],
    harnesses: Iterable[str],
    *,
    scope: str = "project",
    root: PathLike = None,
    home: PathLike = None,
    force: bool = False,
    without: Optional[Iterable[str]] = None,
) -> Dict[str, Any]:
    """Install the manifest's integration for ``harnesses`` (ids, or "all").

    ``manifest`` is a manifest file's path (TOML, or JSON when it ends in
    ``.json``), a manifest as a dict (its text files and sources resolve
    against the working directory), or ``None``: the config
    ``.agents/agent-harness-adapter.toml``, found as the command finds it.
    ``root`` is the project directory (default: the config's, else the
    working directory); ``home`` the home directory. Returns the result,
    as ``agent-harness-adapter install --json`` prints it.
    """
    path, text = _source(manifest)
    options = _options(harnesses, scope, root, home, force, without)
    return json.loads(_native.install(path, text, options))


def uninstall(
    manifest: Optional[Manifest],
    harnesses: Iterable[str],
    *,
    scope: str = "project",
    root: PathLike = None,
    home: PathLike = None,
    force: bool = False,
) -> Dict[str, Any]:
    """Take the manifest's integration back out of ``harnesses`` (ids, or
    "all": the installed ones). A part edited by hand stays unless
    ``force``; what another installed harness reads stays. Returns the
    result, as ``agent-harness-adapter uninstall --json`` prints it.
    """
    # @zen-impl: BND-1_AC-4
    path, text = _source(manifest)
    return json.loads(_native.uninstall(path, text, _options(harnesses, scope, root, home, force)))


def status(
    manifest: Optional[Manifest],
    harnesses: Optional[Iterable[str]] = None,
    *,
    scope: str = "project",
    root: PathLike = None,
    home: PathLike = None,
) -> Dict[str, Any]:
    """The state of each part, for ``harnesses`` or the installed ones."""
    path, text = _source(manifest)
    return json.loads(_native.status(path, text, _options(harnesses, scope, root, home)))


def parse_hook(harness: str, event: str, text: str) -> Dict[str, Any]:
    """The hook contract's input for what ``harness`` sent at ``event``:
    Claude Code's input (``hook_event_name``, ``tool_name``, ``tool_input``,
    …), with ``harness`` and ``tool_kind``."""
    return json.loads(_native.parse_hook(harness, event, text))


def answer_hook(harness: str, event: str, answer: Dict[str, Any]) -> Dict[str, Any]:
    """What the hook command writes for ``answer``, Claude Code's answer
    JSON (e.g. ``{"decision": "block", "reason": "..."}``), as ``harness``
    expects it: ``{"stdout", "stderr", "exit"}``.
    """
    return json.loads(_native.answer_hook(harness, event, json.dumps(answer)))


def run_hook(harness: str, event: str, decide) -> int:
    # @zen-impl: BND-2_AC-4
    """Be a hook command: read the harness's input on stdin, call
    ``decide(input) -> answer`` (Claude Code's answer JSON as a dict, or
    ``None`` to allow), write what the harness expects, and return the exit
    code (``sys.exit(run_hook(...))``).
    """
    import sys

    hook_input = parse_hook(harness, event, sys.stdin.read())
    out = answer_hook(harness, event, decide(hook_input) or {})
    if out["stderr"]:
        sys.stderr.write(out["stderr"])
    sys.stdout.write(out["stdout"])
    sys.stdout.flush()
    return out["exit"]


def schema(contract: str) -> Dict[str, Any]:
    """The JSON Schema of "manifest", "hooks", "hook-input", "hook-answer" or
    "result"."""
    return json.loads(_native.schema(contract))
