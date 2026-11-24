#!/usr/bin/env python3
"""WIP STAND-IN (PR #5770): the speed job's runner measures the reduced game
model with `tools/core_probe.py --quick`. Restored from main before this PR
leaves draft."""
import os
import subprocess
import sys

args = sys.argv[1:]
candidate = args[args.index("--candidate") + 1] if "--candidate" in args else "target/ci/civvis"
here = os.path.dirname(os.path.abspath(__file__))
sys.exit(
    subprocess.call(
        [
            sys.executable,
            "-u",
            os.path.join(here, "core_probe.py"),
            "--civvis",
            candidate,
            "--quick",
            os.path.join(here, "..", "docs", "core_state_classes.json"),
        ]
    )
)
