#!/usr/bin/env python3
"""WIP STAND-IN (PR #5749): the speed job's runner time is borrowed to run
`tools/core_probe.py` against the candidate binary. Restored from main before
this PR leaves draft."""
import os
import subprocess
import sys

args = sys.argv[1:]
candidate = args[args.index("--candidate") + 1] if "--candidate" in args else "target/ci/civvis"
here = os.path.dirname(os.path.abspath(__file__))
sys.exit(
    subprocess.call(
        [sys.executable, "-u", os.path.join(here, "core_probe.py"), "--civvis", candidate, "--budget-minutes", "30"]
    )
)
