"""Mirror recovery follows the selected runtime, including paths with spaces."""
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

SOURCE = Path(__file__).parent / "ops/civvis-mirror-keeper.sh"


@unittest.skipUnless(shutil.which("zsh"), "mirror keeper requires zsh")
class MirrorKeeperRuntimeTest(unittest.TestCase):
    @staticmethod
    def complete_runtime(repo):
        (repo / "tools").mkdir(parents=True, exist_ok=True)
        (repo / "tools/follow.py").write_text("# test follower\n")
        binary = repo / "target/release/civvis"
        binary.parent.mkdir(parents=True, exist_ok=True)
        binary.write_text("#!/bin/sh\n")
        binary.chmod(0o755)

    def resolve(self, pin=None, head=None):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder) / "verification runtime"
            script = root / "tools/ops/civvis-mirror-keeper.sh"
            script.parent.mkdir(parents=True)
            # Execute the actual initializers and resolver, stopping before
            # the keeper's process management and loop are defined or run.
            prefix = SOURCE.read_text().split("find_follower() {", 1)[0]
            script.write_text(prefix + "\nexpected_repo\n")
            pinfile = Path(folder) / "pin"
            if pin is not None:
                pinfile.write_text(pin + "\n")
            env = dict(os.environ, CIVVIS_PINFILE=str(pinfile),
                       CIVVIS_MIRROR_HOME=str(Path(folder) / "mirror"),
                       CIVVIS_HEAD_REPO=head or "")
            result = subprocess.run(["zsh", str(script)], env=env, capture_output=True,
                                    text=True, check=True, timeout=5)
            return result.stdout.strip(), str(root.resolve())

    def test_head_uses_the_runtime_that_owns_the_keeper(self):
        actual, root = self.resolve("head")
        self.assertEqual(actual, root)

    def test_missing_pin_uses_the_runtime_that_owns_the_keeper(self):
        actual, root = self.resolve()
        self.assertEqual(actual, root)

    def test_empty_pin_uses_the_runtime_that_owns_the_keeper(self):
        actual, root = self.resolve("")
        self.assertEqual(actual, root)

    def test_explicit_head_runtime_is_honored(self):
        actual, _ = self.resolve("head", "/tmp/explicit verification runtime")
        self.assertEqual(actual, "/tmp/explicit verification runtime")

    def test_explicit_pin_wins_over_the_head_runtime(self):
        actual, _ = self.resolve("/tmp/pinned game", "/tmp/head runtime")
        self.assertEqual(actual, "/tmp/pinned game")

    def resolve_follower(self, pin_complete, head_complete):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            head = root / "complete head runtime"
            pinned = root / "pinned game"
            script = head / "tools/ops/civvis-mirror-keeper.sh"
            script.parent.mkdir(parents=True)
            prefix = SOURCE.read_text().split("find_follower() {", 1)[0]
            script.write_text(prefix + "\nfollower_runtime\n")
            if head_complete:
                self.complete_runtime(head)
            if pin_complete:
                self.complete_runtime(pinned)
            pinfile = root / "pin"
            pinfile.write_text(str(pinned) + "\n")
            env = dict(os.environ, CIVVIS_PINFILE=str(pinfile),
                       CIVVIS_MIRROR_HOME=str(root / "mirror"),
                       CIVVIS_HEAD_REPO=str(head))
            result = subprocess.run(["zsh", str(script)], env=env,
                                    capture_output=True, text=True, timeout=5)
            return result.returncode, result.stdout.strip(), str(head), str(pinned)

    def test_complete_pin_is_the_follower_runtime(self):
        status, actual, _, pinned = self.resolve_follower(True, True)
        self.assertEqual(status, 0)
        self.assertEqual(actual, pinned)

    def test_incomplete_pin_uses_complete_head_runtime(self):
        status, actual, head, _ = self.resolve_follower(False, True)
        self.assertEqual(status, 0)
        self.assertEqual(actual, head)

    def test_no_complete_runtime_refuses_to_launch(self):
        status, actual, _, _ = self.resolve_follower(False, False)
        self.assertNotEqual(status, 0)
        self.assertEqual(actual, "")


if __name__ == "__main__":
    unittest.main()
