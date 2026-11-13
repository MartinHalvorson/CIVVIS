#!/usr/bin/env python3
"""WIP STAND-IN (PR #5236): the speed job's runner plays the war-gate driver's
Score recipe instead of the paired cost. Restored before the PR leaves draft."""
import os
import subprocess
import sys

env = dict(os.environ, CIVVIS_WAR_GATE_RECIPES="score")
cmd = ["cargo", "test", "--profile", "ci", "--locked", "--features", "developer-tools",
       "--bin", "gene_screen", "war_gate", "--", "--nocapture"]
print("speed_ab stand-in:", " ".join(cmd), "with CIVVIS_WAR_GATE_RECIPES=score", flush=True)
sys.exit(subprocess.call(cmd, env=env))
