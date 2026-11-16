#!/usr/bin/env python3
"""Check isolated SIMD options and the actual publisher's cargo arguments.

No build or game is started. The beta suite discovers this script in CI.
"""

import json
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('publish.sh')


class PublishSimdTests(unittest.TestCase):
    def test_unsafe_outputs_fail_before_building(self):
        with tempfile.TemporaryDirectory() as tmp:
            normal = SCRIPT.parent / 'dist'
            cases = [[], ['--out', tmp], ['--symbols-out', tmp],
                     ['--out', str(normal), '--symbols-out', tmp],
                     ['--out', str(normal / 'experiment'), '--symbols-out', tmp],
                     ['--out', tmp, '--symbols-out', tmp],
                     ['--out', tmp, '--symbols-out', str(Path(tmp) / 'symbols')],
                     ['--out', tmp, '--symbols-out', str(normal / 'symbols')]]
            for args in cases:
                with self.subTest(args=args):
                    done = subprocess.run(['bash', str(SCRIPT), '--experimental-simd128', *args],
                                          capture_output=True, text=True, timeout=10)
                    self.assertNotEqual(done.returncode, 0)
                    self.assertIn('experimental SIMD', done.stderr)
                    self.assertNotIn('building the engine', done.stdout)

    def test_actual_build_function_has_only_the_requested_extra_arguments(self):
        source = SCRIPT.read_text()
        start = source.index('build_engine() (\n')
        end = source.index('\n)\n', start) + 3
        function = source[start:end]
        # Execute the real function with a shell cargo stub, without executing
        # the publisher's PATH changes, rustup, worktrees, builds, or deletion.
        for enabled in (False, True):
            with self.subTest(enabled=enabled):
                harness = ('set -eu\nsource_tree=/\ncommit=fixture\ncommit_time=fixture-time\n'
                           'experimental_simd128=' + str(enabled).lower() + '\n'
                           'cargo() { python3 -c \'import json,os,sys; print(json.dumps(dict(args=sys.argv[1:],commit=os.environ["CIVVIS_COMMIT"],built=os.environ["CIVVIS_BUILT_AT"])))\' "$@"; }\n'
                           + function + '\nbuild_engine\n')
                done = subprocess.run(['bash', '-c', harness], check=True,
                                      capture_output=True, text=True, timeout=10)
                actual = json.loads(done.stdout)
                expected = ['rustc', '--lib', '--target', 'wasm32-unknown-unknown',
                            '--release', '--crate-type', 'cdylib']
                if enabled:
                    expected += ['--', '-C', 'target-feature=+simd128']
                self.assertEqual(actual, dict(args=expected, commit='fixture', built=''))


if __name__ == '__main__':
    unittest.main()
