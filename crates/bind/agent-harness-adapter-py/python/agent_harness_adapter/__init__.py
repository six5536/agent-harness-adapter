"""agent-harness-adapter for Python: one agent setup, every coding agent.

Install instructions, skills and hooks, kept once in ``AGENTS.md`` and
``.agents/`` (the config) or declared in a tool's manifest, into Claude Code,
Codex, Gemini CLI, GitHub Copilot, Cursor, Factory Droid, Pi, OpenCode, Kilo
Code, Qwen Code, Devin and any agent that reads AGENTS.md. Hooks speak one
format in every harness, Claude Code's (``parse_hook`` / ``answer_hook`` /
``run_hook``). Errors raise ValueError with the library's message.
"""


import json
import os
from typing import Any, Dict, Iterable, List, Optional, Union

from . import _native

__all__ = [
    "install",
    "status",
    "uninstall",
    "edits",
    "keep_instructions",
    "follow",
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


def _path(p):
    return None if p is None else os.fspath(p)


def _options(harnesses, scope, root, home, force=False) -> str:
    return json.dumps(
        {
            "harnesses": list(harnesses or []),
            "scope": scope,
            "root": _path(root),
            "home": _path(home),
            "force": force if isinstance(force, bool) else list(force),
        }
    )


def install(
    manifest: Optional[Manifest],
    harnesses: Iterable[str],
    *,
    scope: str = "project",
    root: PathLike = None,
    home: PathLike = None,
    force: Union[bool, Iterable[str]] = False,
) -> Dict[str, Any]:
    """Install the manifest's integration for ``harnesses`` (ids, or "all").

    ``manifest`` is a manifest file's path (TOML, or JSON when it ends in
    ``.json``), a manifest as a dict (its text files and sources resolve
    against the working directory), or ``None``: the config
    ``.agents/agent-harness-adapter.toml``, found as the command finds it.
    ``root`` is the project directory (default: the config's, else the
    working directory); ``home`` the home directory. ``force`` overwrites
    parts edited by hand: ``True`` for every part not declined, or the
    part names. A part is declined in the config, as
    ``[harness.<id>] without = [...]``. Returns the result, as
    ``agent-harness-adapter install --json`` prints it.
    """
    path, text = _source(manifest)
    options = _options(harnesses, scope, root, home, force)
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
    # sokf-implements: BND-1_AC-4
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


def edits(
    manifest: Optional[Manifest],
    harnesses: Optional[Iterable[str]] = None,
    *,
    scope: str = "project",
    root: PathLike = None,
    home: PathLike = None,
) -> List[Dict[str, Any]]:
    """Each file of a part edited by hand, for ``harnesses`` or the
    installed ones: ``{"harness", "part", "path", "expected", "found"}``,
    the text install would write and the text found."""
    # sokf-implements: BND-1_AC-5
    path, text = _source(manifest)
    return json.loads(_native.edits(path, text, _options(harnesses, scope, root, home)))


def keep_instructions(
    manifest: Optional[Manifest],
    text: str,
    *,
    scope: str = "project",
    root: PathLike = None,
    home: PathLike = None,
) -> Dict[str, Any]:
    """Write ``text`` as the project's own: plain text right after the
    tool's block in each file of an installed harness's instructions, once
    each, or in ``AGENTS.md`` when no file holds the block (project scope
    only; elsewhere it raises). A file that already has the text there is
    not written. Returns ``{"report"}``, the report to print as it
    stands."""
    # sokf-implements: BND-1_AC-6
    path, json_text = _source(manifest)
    return json.loads(_native.keep_instructions(path, json_text, text, _options(None, scope, root, home)))


def follow(
    tool: str,
    instructions: Optional[str] = None,
    *,
    keep: Optional[str] = None,
    block_order: str = "installed",
    scope: str = "project",
    root: PathLike = None,
    home: PathLike = None,
) -> Dict[str, Any]:
    """Hand over the tool's ``instructions`` (``None`` takes its block out
    of every harness), a text to ``keep`` as the project's own, and whether
    its block comes ``"last"``; the adapter decides every harness and file.
    Returns ``{"report", "exit"}``: the report to print as it stands and the
    command's exit code (0 done, 1 an edited block left, 2 a refusal). It
    never raises for a refusal."""
    # sokf-implements: BND-1_AC-7
    if not isinstance(tool, str):
        return {"report": "error: give the tool's name as a string\n", "exit": 2}
    options = {
        "instructions": instructions,
        "keep": keep,
        "block_order": block_order,
        "scope": scope,
        "root": _path(root),
        "home": _path(home),
    }
    return json.loads(_native.follow(tool, json.dumps(options)))


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
    # sokf-implements: BND-2_AC-4
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
