"""The native build policy must reach nested builds without stale shell settings."""

import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import unittest


LAUNCHER = Path(__file__).resolve().parent / "ops/civvis-verified-head-launcher.sh"
BUILD_KEYS = ("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "CARGO_PROFILE_RELEASE_LTO",
              "CARGO_INCREMENTAL")


@unittest.skipUnless(shutil.which("zsh"), "zsh is not installed")
class VerificationBuildTests(unittest.TestCase):
    def run_policy(self, mode=None):
        source = LAUNCHER.read_text()
        defaults_start = source.index("policy=(")
        defaults = source[defaults_start:source.index("\n)", defaults_start) + 2]
        case = source[source.index('    case "$key" in'):
                      source.index('    policy[$key]=$value')]
        exports_start = source.index("unset CIVVIS_WITH ")
        exports = source[exports_start:source.index('if [[ -f "$POLICY" ]]; then',
                                                    exports_start)]
        # One immediate and one nested child stand in for the supervisor's
        # cargo build and the Python climb's refresh subprocess. Neither
        # child receives a hand-built environment: export inheritance is what
        # the actual launcher must provide to both of them.
        reader = "import os,json;print(json.dumps({k:os.environ.get(k) for k in " + repr(
            BUILD_KEYS + ("CIVVIS_BUILD_MODE",)) + "}))"
        children = ("import subprocess,sys; "
                    "subprocess.run([sys.executable,'-c'," + repr(reader) + "],check=True); "
                    "subprocess.run([sys.executable,'-c'," + repr(
                        "import subprocess,sys;subprocess.run([sys.executable,'-c',"
                        + repr(reader) + "],check=True)") + "],check=True)")
        script = "\n".join([
            'set -u', 'typeset -A policy', defaults,
            'refuse() { print -ru2 -- "$*"; exit 64; }', 'say() { :; }',
            'HEAD_REPO=/test/native; PIN=/test/pin; POLICY=/test/policy; lineno=1',
            'if (( $# )); then', 'key=CIVVIS_BUILD_MODE; value=$1',
            'for item in 1; do', case, 'policy[$key]=$value', 'done', 'fi',
            exports,
            shlex.quote(sys.executable) + ' -c ' + shlex.quote(children),
        ])
        return subprocess.run(
            ["zsh", "-c", script, "test"] + ([] if mode is None else [mode]),
            env={**os.environ, "CIVVIS_BUILD_MODE": "fast",
                 **{k: "stale" for k in BUILD_KEYS}},
            capture_output=True, text=True,
        )

    def test_fast_mode_is_inherited_by_both_build_generations(self):
        done = self.run_policy("fast")
        self.assertEqual(done.returncode, 0, done.stderr)
        expected = dict(zip(BUILD_KEYS, ("16", "false", "1")))
        expected["CIVVIS_BUILD_MODE"] = "fast"
        self.assertEqual([json.loads(row) for row in done.stdout.splitlines()],
                         [expected, expected])

    def test_default_and_explicit_release_clear_stale_fast_settings(self):
        for mode in (None, "release"):
            with self.subTest(mode=mode):
                done = self.run_policy(mode)
                self.assertEqual(done.returncode, 0, done.stderr)
                expected = {k: None for k in BUILD_KEYS}
                expected["CIVVIS_BUILD_MODE"] = "release"
                self.assertEqual([json.loads(row) for row in done.stdout.splitlines()],
                                 [expected, expected])

    def test_invalid_modes_are_rejected_before_children_start(self):
        for mode in ("", "debug", "fast;echo injected"):
            with self.subTest(mode=mode):
                done = self.run_policy(mode)
                self.assertEqual(done.returncode, 64, done.stderr)
                self.assertEqual(done.stdout, "")

    def test_launcher_parses(self):
        done = subprocess.run(["zsh", "-n", str(LAUNCHER)], capture_output=True, text=True)
        self.assertEqual(done.returncode, 0, done.stderr)


if __name__ == "__main__":
    unittest.main()
