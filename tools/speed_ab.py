#!/usr/bin/env python3
"""WIP stand-in (removed before ready): build gene_screen on the runner and run
the culture-road-builds-its-triggers fires probe, printing both artifacts."""
import subprocess
import sys

TAG = "culture-road-builds-its-triggers"


def run(cmd):
    print("+", " ".join(cmd), flush=True)
    return subprocess.run(cmd).returncode


rc = run(["cargo", "build", "--profile", "ci", "--features", "developer-tools",
          "--bin", "gene_screen", "--locked"])
if rc:
    sys.exit(rc)
rc = run(["target/ci/gene_screen", "--games", "4", "--jobs", "4", "--difficulty", "deity",
          "--speed", "online", "--turns", "160", "--target-mix", "culture",
          "--genes", TAG, "--start-seed", "33600000", "--quiet", "--out", f"/tmp/{TAG}.jsonl"])
print("probe exit", rc, flush=True)
rc2 = run(["target/ci/gene_screen", "--analyze", f"/tmp/{TAG}.jsonl", "--json", f"/tmp/{TAG}.json"])
for path in (f"/tmp/{TAG}.jsonl", f"/tmp/{TAG}.json"):
    print(f"=====BEGIN {path}", flush=True)
    try:
        print(open(path).read(), flush=True)
    except OSError as exc:
        print("missing", exc)
    print(f"=====END {path}", flush=True)
sys.exit(0)
