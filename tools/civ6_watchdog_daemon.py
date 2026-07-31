"""Run the watchdogs against every attempt as it finishes, unattended.

    python3 tools/civ6_watchdog_daemon.py --seconds 40000 &

⚠ THE DETECTORS ARE NOT THE POINT — RUNNING THEM UNPROMPTED IS. Both failures
`civ6_watchdogs.py` measures went a full night without being noticed while
`civ6_civvis_status.py` printed green, because nothing ran a check that could go red.
A tool that has to be remembered is a tool that reports what was already suspected.

Watches the run directory, and for every run whose events have stopped growing (so
the attempt is over) writes one line to `watchdogs.jsonl` and prints any verdict.
Idempotent: a run already in the ledger is never re-checked.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"
REPORT = Path.home() / "civvis-civ6-runs" / "watchdogs.jsonl"


def done_runs(seen: dict[str, int], quiet_for: float) -> list[Path]:
    """Runs whose event file has not grown for `quiet_for` seconds.

    ⚠ KEYED ON SIZE, NOT ON NAME. A live attempt whose turn rate collapses under
    machine load goes quiet for minutes at a time — this daemon checked one such run
    mid-flight, wrote it down as done, and would then never have looked at it again.
    A run is re-checked whenever its stream has grown since the last look, so a
    premature verdict is replaced by the final one instead of standing.
    """
    now = time.time()
    out = []
    for run in sorted(RUN_ROOT.iterdir()):
        events = run / "events.jsonl"
        if not events.exists():
            continue
        stat = events.stat()
        if seen.get(run.name) == stat.st_size:
            continue
        if now - stat.st_mtime < quiet_for:
            continue
        out.append(run)
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seconds", type=float, default=36000.0)
    ap.add_argument("--poll", type=float, default=60.0)
    ap.add_argument("--quiet-for", type=float, default=180.0,
                    help="seconds of no new events before a run counts as finished")
    args = ap.parse_args()

    seen: dict[str, int] = {}
    if REPORT.exists():
        for line in REPORT.read_text(errors="replace").splitlines():
            try:
                record = json.loads(line)
                seen[record["run"]] = record.get("events_bytes", -1)
            except (ValueError, KeyError):
                continue
    # ⚠ Everything already on disk when this starts is checked once unless the report
    # already holds it at its current size, so starting the daemon late does not lose
    # the night's earlier attempts.
    deadline = time.time() + args.seconds
    while time.time() < deadline:
        for run in done_runs(seen, args.quiet_for):
            seen[run.name] = (run / "events.jsonl").stat().st_size
            proc = subprocess.run(
                [sys.executable, str(HERE / "civ6_watchdogs.py"),
                 "--run", run.name, "--json", str(REPORT)],
                capture_output=True, text=True, timeout=900,
            )
            print(proc.stdout.rstrip(), flush=True)
            if proc.stderr.strip():
                print(f"  (stderr) {proc.stderr.strip()[:300]}", flush=True)
        time.sleep(args.poll)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
