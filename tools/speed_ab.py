#!/usr/bin/env python3
"""TEMPORARY, WIP commit only: run the last-civ probe matrix on the CI runner.

The real paired-timing harness is restored before this pull request is marked
ready. The speed workflow hands this script a built candidate binary; here it
plays stock worlds past their first result under different experiment
switches and prints each run's report.
"""
import os
import subprocess
import sys
import time

candidate = sys.argv[sys.argv.index("--candidate") + 1]
START = time.time()
DEADLINE = START + 34 * 60
FULL = "bodies=4,patience=8,noabort=1,bill=1,nomuster=1,norecovery=1,arm=1"
RUNS = [
    ("full", FULL, 13, 1200),
    ("full", FULL, 14, 1200),
    ("full", FULL, 22, 1200),
    ("full", FULL, 23, 1200),
    ("norecovery", "norecovery=1", 13, 3600),
    ("economy", "bodies=4,norecovery=1,arm=1", 13, 3600),
    ("siege", "patience=8,noabort=1,bill=1,nomuster=1", 13, 3600),
    ("full", FULL, 19, 3600),
    ("full", FULL, 11, 3600),
    ("full", FULL, 12, 3600),
]
os.makedirs("/tmp/lcs", exist_ok=True)
pending = list(RUNS)
running = []
slots = os.cpu_count() or 4


def show(label, knobs, seed, path, note):
    print(f'LCSREPORT seed={seed} knobs="{knobs}" label={label} {note}', flush=True)
    with open(path) as handle:
        sys.stdout.write(handle.read())
    sys.stdout.flush()


while pending or running:
    now = time.time()
    while pending and len(running) < slots and DEADLINE - now > 180:
        label, knobs, seed, cap = pending.pop(0)
        wall = int(min(cap, DEADLINE - now - 30))
        path = f"/tmp/lcs/{label}_{seed}.log"
        env = dict(os.environ, LCS=knobs)
        proc = subprocess.Popen(
            [candidate, "lcs-probe", "--seed", str(seed), "--extra", "400",
             "--wall", str(wall), "--dumps", "2"],
            stdout=open(path, "w"), stderr=subprocess.STDOUT, env=env,
        )
        running.append((proc, label, knobs, seed, path))
        print(f"started {label} seed={seed} wall={wall} at +{int(now - START)}s", flush=True)
    if pending and not running:
        break
    time.sleep(5)
    still = []
    for entry in running:
        proc, label, knobs, seed, path = entry
        if proc.poll() is not None:
            show(label, knobs, seed, path, f"exit={proc.returncode} at +{int(time.time() - START)}s")
        elif time.time() > DEADLINE + 20:
            proc.kill()
            proc.wait()
            show(label, knobs, seed, path, "killed at the deadline")
        else:
            still.append(entry)
    running = still
for label, knobs, seed, _ in pending:
    print(f"not started: {label} seed={seed}", flush=True)
print("lcs matrix done", flush=True)
