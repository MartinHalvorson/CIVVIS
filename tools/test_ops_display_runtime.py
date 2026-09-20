"""Exercise display recovery without launching Chrome or touching live services."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).parent / "ops" / "civvis-overnight-audit.sh"


@unittest.skipUnless(shutil.which("zsh"), "requires zsh")
class DisplayRuntimeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="display runtime ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bindir = self.root / "bin"
        self.bindir.mkdir()
        self.script = self.root / "test.zsh"
        # Load the real recovery functions, without executing the host audit.
        self.prefix = SCRIPT.read_text().split("typeset -a warnings actions", 1)[0]

    def node(self, path, compatible=True):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            '#!/bin/sh\n'
            'if [ "$1" = -e ]; then\n'
            f'  exit {0 if compatible else 1}\n'
            'fi\n'
            'printf "%s\\n" "$1" > "$DISPLAY_TEST_LAUNCH"\n'
        )
        path.chmod(0o755)
        return path

    def run_shell(self, body, override=None):
        self.script.write_text(self.prefix + '\nBASE=$1\nAUDIT_LOG=$BASE/audit.log\n' + body)
        env = dict(os.environ, PATH=str(self.bindir), DISPLAY_TEST_LAUNCH=str(self.root / "launched"))
        env.pop("CIVVIS_DISPLAY_NODE", None)
        if override is not None:
            env["CIVVIS_DISPLAY_NODE"] = str(override)
        return subprocess.run(
            [shutil.which("zsh"), str(self.script), str(self.root)],
            env=env, capture_output=True, text=True, timeout=10,
        )

    def test_path_runtime(self):
        node = self.node(self.bindir / "node")
        result = self.run_shell("display_node\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), str(node))

    def test_local_runtime_under_minimal_launchd_path(self):
        node = self.node(self.root / ".local/bin/node")
        result = self.run_shell("display_node\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), str(node))

    def test_old_path_runtime_falls_back_to_compatible_local_runtime(self):
        self.node(self.bindir / "node", compatible=False)
        node = self.node(self.root / ".local/bin/node")
        result = self.run_shell("display_node\n")
        self.assertEqual(result.stdout.strip(), str(node))

    def test_explicit_incompatible_runtime_does_not_silently_fall_back(self):
        self.node(self.bindir / "node")
        old = self.node(self.root / "old-node", compatible=False)
        result = self.run_shell("display_node\n", override=old)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")
        self.assertIn("no Node runtime", (self.root / "audit.log").read_text())

    def test_missing_runtime_refuses_launch(self):
        result = self.run_shell(
            'verification_intent_running() { return 0; }\nstart_display_keeper\n',
            override=self.root / "missing-node",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / "launched").exists())

    def test_stopped_intent_never_probes_or_launches(self):
        result = self.run_shell(
            'verification_intent_running() { return 1; }\n'
            'display_node() { print probed; return 0; }\nstart_display_keeper\n',
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")

    def test_launch_uses_resolved_runtime_and_preserves_spaces(self):
        node = self.node(self.root / "custom node")
        keeper = self.root / "display keeper.mjs"
        keeper.write_text("// fixture\n")
        (self.root / "civvis-civ6-mirror").mkdir()
        result = self.run_shell(
            'verification_intent_running() { return 0; }\n'
            'DISPLAY_KEEPER="$BASE/display keeper.mjs"\nstart_display_keeper\nwait\n',
            override=node,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "launched").read_text().strip(), str(keeper))


if __name__ == "__main__":
    unittest.main()
