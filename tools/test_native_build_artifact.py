"""Native artifacts must retain their exact source, binary and test evidence."""
import hashlib
import json
from pathlib import Path
import subprocess
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_build_artifact as artifact


class NativeBuildArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.repo, self.out = root / "source", root / "artifact"
        self.repo.mkdir()
        self.out.mkdir()
        self.git("init", "-q")
        (self.repo / "input.txt").write_text("source\n")
        self.git("add", "input.txt")
        self.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                 "commit", "-qm", "fixture")
        self.sha = self.git("rev-parse", "HEAD").strip()
        self.receipt = {"schema": 1, "status": "complete", "architecture": "arm64",
                        "profile": artifact.FAST_PROFILE.copy(), "rustc": "rustc 1.97.1 (fixture)",
                        "source": artifact.source_manifest(self.repo, self.sha),
                        "tests": [{"target": "lib", "filter": "fixture", "passed": 1}],
                        "binaries": {}}
        for name in ("civvis", "civvis_orders"):
            payload = struct.pack("<II", 0xfeedfacf, 0x0100000c) + name.encode()
            (self.out / name).write_bytes(payload)
            self.receipt["binaries"][name] = hashlib.sha256(payload).hexdigest()
        self.save()

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.repo), *args],
                                       text=True, stderr=subprocess.DEVNULL)

    def save(self):
        (self.out / "receipt.json").write_text(json.dumps(self.receipt))

    def test_a_complete_matching_receipt_verifies(self):
        self.assertEqual(artifact.verify(self.repo, self.out, self.sha), self.receipt)

    def test_dirty_and_untracked_source_inputs_are_rejected(self):
        (self.repo / "input.txt").write_text("changed\n")
        with self.assertRaisesRegex(ValueError, "dirty"):
            artifact.verify(self.repo, self.out, self.sha)
        (self.repo / "input.txt").write_text("source\n")
        (self.repo / "new.txt").write_text("untracked")
        with self.assertRaisesRegex(ValueError, "dirty"):
            artifact.verify(self.repo, self.out, self.sha)

    def test_wrong_source_identity_and_invalid_sha_are_rejected(self):
        for sha in ("0" * 40, "main", "x;exit", "A" * 40):
            with self.subTest(sha=sha), self.assertRaises(ValueError):
                artifact.verify(self.repo, self.out, sha)

    def test_changed_binary_bytes_are_rejected(self):
        (self.out / "civvis_orders").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "checksum"):
            artifact.verify(self.repo, self.out, self.sha)

    def test_a_matching_checksum_cannot_certify_another_architecture(self):
        payload = b"not a Mach-O executable"
        (self.out / "civvis_orders").write_bytes(payload)
        self.receipt["binaries"]["civvis_orders"] = hashlib.sha256(payload).hexdigest()
        self.save()
        with self.assertRaisesRegex(ValueError, "Mach-O"):
            artifact.verify(self.repo, self.out, self.sha)

    def test_corrupt_source_receipt_is_rejected(self):
        self.receipt["source"]["files"]["input.txt"] = "0" * 64
        self.save()
        with self.assertRaisesRegex(ValueError, "source inputs"):
            artifact.verify(self.repo, self.out, self.sha)

    def test_failed_empty_and_incomplete_artifacts_are_rejected(self):
        for key, value in (("status", "failed"), ("tests", []),
                           ("tests", [{"passed": 0}]), ("binaries", {}),
                           ("architecture", "x86_64")):
            old = self.receipt[key]
            self.receipt[key] = value
            self.save()
            with self.subTest(key=key), self.assertRaises(ValueError):
                artifact.verify(self.repo, self.out, self.sha)
            self.receipt[key] = old

    def test_filters_are_nonempty_structured_arguments(self):
        self.assertEqual(artifact.selection('{"lib":["siege_train::muster_tests"]}'),
                         [("lib", "siege_train::muster_tests")])
        for text in ('{}', '[]', '{"lib":[]}', '{"lib":["--ignored"]}',
                     '{"other":["fixture"]}', '{"lib":["x;touch x"]}'):
            with self.subTest(text=text), self.assertRaises(ValueError):
                artifact.selection(text)

    def test_requested_compiler_profile_and_scope_must_match(self):
        with self.assertRaisesRegex(ValueError, "compiler"):
            artifact.verify(self.repo, self.out, self.sha, toolchain="1.96.0")
        with self.assertRaisesRegex(ValueError, "test selections"):
            artifact.verify(self.repo, self.out, self.sha, tests='{"lib":["other_module"]}')
        self.receipt["profile"]["CARGO_INCREMENTAL"] = "0"
        self.save()
        with self.assertRaisesRegex(ValueError, "profile"):
            artifact.verify(self.repo, self.out, self.sha)


if __name__ == "__main__":
    unittest.main()
