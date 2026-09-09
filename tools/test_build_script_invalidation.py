#!/usr/bin/env python3
"""Exercise the repository build script with Cargo's real change detection."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


@unittest.skipUnless(shutil.which("cargo"), "Cargo is required for build-cache checks")
class BuildScriptInvalidationTests(unittest.TestCase):
    def test_tools_stay_cached_but_compiler_inputs_still_rebuild(self):
        with tempfile.TemporaryDirectory(prefix="civvis-invalidation-") as tmp:
            root = Path(tmp)
            (root / "src").mkdir()
            (root / "Cargo.toml").write_text(
                '[package]\nname = "invalidation_probe"\nversion = "0.0.0"\n'
                'edition = "2021"\n')
            build = root / "build.rs"
            build.write_text((Path(__file__).resolve().parents[1] / "build.rs").read_text())
            source = root / "src/lib.rs"
            source.write_text(
                'pub const DATA: &str = include_str!("../data.txt");\n'
                'pub const BYTES: &[u8] = include_bytes!("../bytes.bin");\n')
            data = root / "data.txt"
            data.write_text("initial data\n")
            binary = root / "bytes.bin"
            binary.write_bytes(b"initial bytes")
            tool = root / "worker.py"
            tool.write_text("print('original')\n")
            env = dict(os.environ, CARGO_TARGET_DIR=str(root / "target"))

            def fresh():
                result = subprocess.run(
                    ["cargo", "build", "--offline", "--message-format=json"],
                    cwd=root, env=env, capture_output=True, text=True, timeout=60,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                artifacts = [row for line in result.stdout.splitlines()
                             if (row := json.loads(line)).get("reason") == "compiler-artifact"
                             and row["target"]["kind"] == ["lib"]]
                self.assertEqual(len(artifacts), 1, result.stdout)
                return artifacts[0]["fresh"]

            self.assertFalse(fresh(), "the initial crate must compile")
            self.assertTrue(fresh(), "an unchanged crate must remain cached")
            tool.write_text("print('updated Python automation')\n")
            self.assertTrue(fresh(), "a Python-only edit must not rebuild Rust")
            source.write_text(source.read_text() + "pub const NEW_VALUE: u8 = 7;\n")
            self.assertFalse(fresh(), "Rust source changes must rebuild")
            data.write_text("updated embedded data\n")
            self.assertFalse(fresh(), "include_str! inputs must still rebuild")
            binary.write_bytes(b"updated embedded bytes")
            self.assertFalse(fresh(), "include_bytes! inputs must still rebuild")
            build.write_text(build.read_text() + "// changed build script\n")
            self.assertFalse(fresh(), "build script changes must rebuild")
            self.assertTrue(fresh(), "the resulting build must remain cached")


if __name__ == "__main__":
    unittest.main()
