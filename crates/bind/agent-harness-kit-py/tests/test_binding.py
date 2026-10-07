"""The Python binding over a built package (BND-1..3).

    python -m unittest discover -s crates/bind/agent-harness-kit-py/tests

With AHK set to an ahk binary, results are also compared with ahk's.
"""

import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import agent_harness_kit as ahk

MANIFEST = {"version": 1, "name": "t", "instructions": "Use t.\n", "allow_commands": ["t"]}


class Tree:
    def __enter__(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = self.tmp.name
        self.proj = os.path.join(self.dir, "proj")
        self.home = os.path.join(self.dir, "home")
        os.makedirs(self.proj)
        os.makedirs(self.home)
        return self

    def __exit__(self, *exc):
        self.tmp.cleanup()

    def manifest_file(self):
        path = os.path.join(self.dir, "t.harness.json")
        with open(path, "w", encoding="utf-8") as f:
            json.dump(MANIFEST, f)
        return path


class InstallStatus(unittest.TestCase):
    # @zen-test: BND-1_AC-1
    # @zen-test: BND-1_AC-2
    def test_install_then_status(self):
        with Tree() as t:
            out = ahk.install(MANIFEST, ["claude"], root=t.proj, home=t.home)
            self.assertEqual(out["harnesses"][0]["parts"][0]["action"], "created")
            with open(os.path.join(t.proj, "CLAUDE.md"), encoding="utf-8") as f:
                self.assertIn("Use t.", f.read())
            st = ahk.status(t.manifest_file(), root=t.proj, home=t.home)
            self.assertEqual([h["harness"] for h in st["harnesses"]], ["claude"])
            self.assertEqual(st["harnesses"][0]["parts"][0]["state"], "current")
            local = ahk.install(
                MANIFEST, ["all"], scope="local", root=t.proj, home=t.home,
                force=True, without=["permissions"],
            )
            self.assertEqual([h["harness"] for h in local["harnesses"]], ["claude", "copilot"])

    # @zen-test: BND-1_AC-3
    def test_refusals_raise_value_error(self):
        with Tree() as t:
            with self.assertRaisesRegex(ValueError, "no harness named `vim`"):
                ahk.install(MANIFEST, ["vim"], root=t.proj, home=t.home)
            with self.assertRaisesRegex(ValueError, "version 2"):
                ahk.status({"version": 2}, root=t.proj, home=t.home)
            with self.assertRaisesRegex(ValueError, "no scope named `global`"):
                ahk.status(MANIFEST, scope="global", root=t.proj, home=t.home)

    # BND_P-1: the same result as `ahk --json`.
    @unittest.skipUnless(os.environ.get("AHK"), "AHK is not set")
    def test_the_same_as_ahk(self):
        with Tree() as t:
            path = t.manifest_file()
            mine = ahk.install(path, ["claude", "codex"], root=t.proj, home=t.home)
            env = dict(os.environ, HOME=t.home, USERPROFILE=t.home)
            theirs = subprocess.run(
                [os.environ["AHK"], "status", "--manifest", path, "--root", t.proj,
                 "--harness", "claude,codex", "--json"],
                capture_output=True, text=True, env=env, check=True,
            )
            st = ahk.status(path, ["claude", "codex"], root=t.proj, home=t.home)
            self.assertEqual(json.loads(theirs.stdout), st)
            self.assertEqual(mine["root"], st["root"])


class Hooks(unittest.TestCase):
    # @zen-test: BND-2_AC-1
    def test_parse_hook(self):
        got = ahk.parse_hook("claude", "stop", '{"session_id":"s","stop_hook_active":true}')
        self.assertEqual((got["v"], got["session_id"], got["continuing"]), (1, "s", True))
        self.assertEqual(
            ahk.parse_hook("codex", "stop", "nope"),
            {"v": 1, "harness": "codex", "event": "stop"},
        )

    # @zen-test: BND-2_AC-2
    # @zen-test: BND-2_AC-3
    def test_answer_hook(self):
        out = ahk.answer_hook("claude", "stop", {"answer": "continue", "reason": "go"})
        self.assertEqual(
            out, {"stdout": '{"decision":"block","reason":"go"}\n', "stderr": None, "exit": 0}
        )
        with self.assertRaisesRegex(ValueError, "cannot answer deny at stop"):
            ahk.answer_hook("claude", "stop", {"answer": "deny", "reason": "r"})
        with self.assertRaisesRegex(ValueError, "no hook event named `later`"):
            ahk.parse_hook("claude", "later", "{}")

    # @zen-test: BND-2_AC-4
    def test_run_hook(self):
        seen = []

        def decide(hook):
            seen.append(hook)
            return {"answer": "allow", "stderr": "noted\n"}

        stdin = io.StringIO('{"session_id":"s"}')
        stdout, stderr = io.StringIO(), io.StringIO()
        with mock.patch.object(sys, "stdin", stdin), mock.patch.object(
            sys, "stdout", stdout
        ), mock.patch.object(sys, "stderr", stderr):
            code = ahk.run_hook("claude", "stop", decide)
        self.assertEqual(code, 0)
        self.assertEqual(seen[0]["session_id"], "s")
        self.assertEqual((stdout.getvalue(), stderr.getvalue()), ("{}\n", "noted\n"))


class Schemas(unittest.TestCase):
    # @zen-test: BND-3_AC-1
    # @zen-test: BND-3_AC-2
    def test_schemas_and_versions(self):
        for c in ["manifest", "hook-input", "hook-answer", "result"]:
            self.assertTrue(ahk.schema(c)["title"].startswith("AHK "))
        with self.assertRaisesRegex(ValueError, "no contract"):
            ahk.schema("nope")
        self.assertEqual((ahk.MANIFEST_VERSION, ahk.HOOK_VERSION), (1, 1))
        self.assertRegex(ahk.__version__, r"^\d+\.\d+\.\d+")


if __name__ == "__main__":
    unittest.main()
