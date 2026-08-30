#!/usr/bin/env python3
"""Reclaim the screenshots of finished runs, and nothing else.

★★★★★ SCREENSHOTS ARE 97% OF THE RUN STORE. Measured 2026-08-30 over 760 runs
under `~/civvis-civ6-runs/control`:

    total        157.7 GB
    .png         153.2 GB   31738 files     <- this
    .jsonl         4.0 GB    1394 files
    everything else < 0.6 GB

They are full-desktop captures (3456x2234, 5-10 MB each) and they cannot simply
be shrunk: the OCR that drives the menus reads these exact files, so downscaling
or recompressing them would degrade the thing they exist for. What they can be
is *deleted once the run they diagnose is over* — which is the run-retention gap
already on record ("179GB, no run retention").

⚠⚠ THE EVIDENCE THAT MATTERS IS NOT THE PNG. A finished run's value lives in
`events.jsonl`, `summary.json`, `orders.sqlite` and `why.log` — every analysis in
this repo reads those. The screenshots earn their keep only while a failure is
being diagnosed, and this tool is deliberately unable to touch anything else.

⚠ Dry-run by default. `--apply` is required to delete, and even then a run is
skipped when a live process names it, when its tag cannot be read, or when it is
too recent — three days finished, seven unfinished, because a run with no
`summary.json` is the one whose pictures a diagnosis is most likely to want.
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import time
from pathlib import Path

RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"

# A run must be at least this old before its pictures go. Three days is well past
# the window in which anyone is still reading a failure — the live diagnoses in
# this repo all happened within minutes of the run, and the ledger keeps the
# numbers forever.
DEFAULT_AGE_DAYS = 3

# An UNFINISHED run — no `summary.json` — waits longer. Before #2807 a run the
# wedge watchdog killed wrote no summary at all, so "unfinished" describes 315 of
# the runs on this host (23.8 GB of pictures) and most of them are weeks old and
# will never gain one. They are still the runs whose pictures a live diagnosis is
# most likely to want, so they keep a wider margin rather than an exemption.
DEFAULT_UNFINISHED_AGE_DAYS = 7

# ⚠ Only these. A glob of `*.png` would also take `wedge-sample.txt`'s neighbours
# on a future rename; naming the suffix keeps the blast radius at one file type.
SCREENSHOT_SUFFIX = ".png"

# The run directory name the harness writes: `civvis-<UTC>Z`, optionally with a
# `-contN` continuation suffix. ⚠ Matching this rather than `civvis-*` keeps the
# July runs (`civvis-clean-…`, `civvis-duel-…`) and any hand-made directory out
# of reach.
RUN_NAME = re.compile(r"^civvis-\d{8}T\d{6}Z(-cont\d+)?$")


def run_age_days(run: Path, now: float) -> float:
    """How old the run is, by its TAG rather than its mtime.

    ⚠⚠ NOT `getmtime`. A directory's mtime moves whenever an entry is created in
    it, and a read-only `sqlite3` connect writes `-shm`/`-wal` beside the db — so
    merely *analysing* a run makes it look new. The tag is the start time and
    cannot be disturbed by reading.
    """
    stamp = run.name[len("civvis-"):len("civvis-") + 16]
    try:
        started = time.mktime(time.strptime(stamp, "%Y%m%dT%H%M%SZ")) - time.timezone
    except ValueError:
        return -1.0
    return (now - started) / 86400.0


def live_run_tags() -> set[str]:
    """Tags any running harness process names, so a live run is never touched."""
    try:
        out = subprocess.run(["ps", "-eo", "args="], capture_output=True,
                             text=True, timeout=20).stdout
    except (OSError, subprocess.SubprocessError):
        # Unreadable process table means "assume everything is live".
        return {"*"}
    return {tag for tag in re.findall(r"civvis-\d{8}T\d{6}Z(?:-cont\d+)?", out)}


def prunable(run: Path, now: float, age_days: float, live: set[str],
             unfinished_age_days: float) -> str | None:
    """Why this run may NOT be pruned, or None when it may."""
    if not RUN_NAME.match(run.name):
        return "not a harness run directory"
    if "*" in live or run.name in live:
        return "a live process names it"
    age = run_age_days(run, now)
    if age < 0:
        return "unreadable tag"
    finished = (run / "summary.json").exists()
    limit = age_days if finished else unfinished_age_days
    if age < limit:
        state = "" if finished else ", unfinished"
        return f"only {age:.1f} days old{state}"
    return None


def screenshots(run: Path) -> list[Path]:
    return sorted(p for p in run.iterdir()
                  if p.is_file() and p.suffix == SCREENSHOT_SUFFIX)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--run-root", type=Path, default=RUN_ROOT)
    ap.add_argument("--older-than-days", type=float, default=DEFAULT_AGE_DAYS,
                    help=f"leave runs younger than this alone (default {DEFAULT_AGE_DAYS})")
    ap.add_argument("--unfinished-after-days", type=float,
                    default=DEFAULT_UNFINISHED_AGE_DAYS,
                    help="a run with no summary.json waits this long instead "
                         f"(default {DEFAULT_UNFINISHED_AGE_DAYS})")
    ap.add_argument("--apply", action="store_true",
                    help="actually delete; without it nothing is removed")
    args = ap.parse_args(argv)

    root = args.run_root
    if not root.is_dir():
        print(f"no run root at {root}", file=sys.stderr)
        return 2

    now = time.time()
    live = live_run_tags()
    freed = 0
    files = 0
    pruned_runs = 0
    kept: dict[str, int] = {}
    for run in sorted(root.iterdir()):
        if not run.is_dir():
            continue
        why = prunable(run, now, args.older_than_days, live,
                       args.unfinished_after_days)
        if why is not None:
            kept[why] = kept.get(why, 0) + 1
            continue
        shots = screenshots(run)
        if not shots:
            continue
        pruned_runs += 1
        for shot in shots:
            try:
                size = shot.stat().st_size
            except OSError:
                continue
            freed += size
            files += 1
            if args.apply:
                try:
                    shot.unlink()
                except OSError as exc:  # noqa: PERF203 - one bad file must not stop the sweep
                    print(f"could not remove {shot}: {exc}", file=sys.stderr)

    verb = "removed" if args.apply else "would remove"
    print(f"{verb} {files} screenshot(s), {freed / 1e9:.1f} GB, "
          f"across {pruned_runs} finished run(s)")
    for why, count in sorted(kept.items(), key=lambda kv: -kv[1]):
        print(f"  kept {count} run(s): {why}")
    if not args.apply:
        print("dry run — pass --apply to delete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
