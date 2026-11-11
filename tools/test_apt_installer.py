#!/usr/bin/env python3
"""Exercise bounded package-install failures and real mirror-file rewrites."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
INSTALLER = ROOT / ".github/scripts/install-apt-packages.sh"
MIRRORS = """http://azure.archive.ubuntu.com/ubuntu priority:1
https://archive.ubuntu.com/ubuntu priority:2
https://security.ubuntu.com/ubuntu priority:3
https://custom.example/ubuntu priority:4
"""

APT = r'''#!/usr/bin/env python3
import json, os, pathlib, sys
args = sys.argv[1:]
command = next(arg for arg in args if arg in ("update", "install"))
mirrors = pathlib.Path(os.environ["CIVVIS_APT_MIRROR_LIST"]).read_text()
host = mirrors.splitlines()[0].split("/")[2]
with open(os.environ["APT_TEST_LOG"], "a") as log:
    log.write(json.dumps({"command": command, "host": host, "args": args}) + "\n")
mode = os.environ["APT_TEST_MODE"]
failed = mode == "all_fail"
failed |= mode == "fallback" and host == "archive.ubuntu.com"
failed |= mode == "install_fallback" and command == "install" and host == "archive.ubuntu.com"
failed |= mode == "relaxed" and "--error-on=any" in args
sys.exit(100 if failed else 0)
'''


class AptInstallerTests(unittest.TestCase):
    def run_installer(self, mode, packages=("binaryen", "brotli")):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "bin"
            binary.mkdir()
            scripts = {
                "sudo": '#!/bin/sh\nexec "$@"\n',
                "timeout": '#!/bin/sh\nshift\nshift\nexec "$@"\n',
                "sleep": "#!/bin/sh\nexit 0\n",
                "apt-get": APT,
            }
            for name, content in scripts.items():
                path = binary / name
                path.write_text(content)
                path.chmod(0o755)
            mirror = root / "mirrors.txt"
            mirror.write_text(MIRRORS)
            log = root / "apt.jsonl"
            env = dict(os.environ, PATH=str(binary) + os.pathsep + os.environ["PATH"])
            env.update(CIVVIS_APT_MIRROR_LIST=str(mirror), APT_TEST_LOG=str(log), APT_TEST_MODE=mode)
            result = subprocess.run(
                ["bash", str(INSTALLER), *packages], env=env,
                capture_output=True, text=True, timeout=10,
            )
            rows = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            return result, rows, mirror.read_text()

    def test_primary_success_preserves_other_mirrors_and_exact_packages(self):
        result, rows, mirrors = self.run_installer("success")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([row["command"] for row in rows], ["update", "install"])
        self.assertTrue(all(row["host"] == "archive.ubuntu.com" for row in rows))
        self.assertEqual(rows[-1]["args"][-2:], ["binaryen", "brotli"])
        self.assertNotIn("azure.archive", mirrors)
        self.assertIn("https://security.ubuntu.com/ubuntu priority:3", mirrors)
        self.assertIn("https://custom.example/ubuntu priority:4", mirrors)

    def test_unreachable_primary_switches_to_kernel_mirror(self):
        result, rows, mirrors = self.run_installer("fallback")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([row["host"] for row in rows], [
            "archive.ubuntu.com", "archive.ubuntu.com",
            "mirrors.edge.kernel.org", "mirrors.edge.kernel.org",
        ])
        self.assertEqual(rows[-1]["command"], "install")
        self.assertIn("https://mirrors.edge.kernel.org/ubuntu priority:1", mirrors)

    def test_package_download_failure_also_switches_mirrors(self):
        result, rows, _ = self.run_installer("install_fallback", ("lua5.1",))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([row["command"] for row in rows], ["update", "install", "update", "install"])
        self.assertEqual(rows[-1]["host"], "mirrors.edge.kernel.org")
        self.assertEqual(rows[-1]["args"][-1], "lua5.1")

    def test_total_outage_is_bounded_and_returns_failure(self):
        result, rows, _ = self.run_installer("all_fail")
        self.assertEqual(result.returncode, 1)
        self.assertEqual([row["host"] for row in rows], [
            "archive.ubuntu.com", "archive.ubuntu.com",
            "mirrors.edge.kernel.org", "mirrors.edge.kernel.org",
            "archive.ubuntu.com", "archive.ubuntu.com",
        ])
        self.assertTrue(all(row["command"] == "update" for row in rows))
        self.assertIn("after 3 bounded attempts", result.stderr)

    def test_usable_indexes_still_allow_the_existing_relaxed_update(self):
        result, rows, _ = self.run_installer("relaxed")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([row["command"] for row in rows], ["update", "update", "install"])
        self.assertTrue(all(row["host"] == "archive.ubuntu.com" for row in rows))

    def test_missing_package_arguments_do_not_touch_sources(self):
        result, rows, mirrors = self.run_installer("success", ())
        self.assertEqual(result.returncode, 64)
        self.assertEqual(rows, [])
        self.assertEqual(mirrors, MIRRORS)


if __name__ == "__main__":
    unittest.main()
