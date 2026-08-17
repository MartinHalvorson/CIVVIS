#!/usr/bin/env python3
"""Kill a single runaway process before macOS jetsam kills everything.

Background
----------
On 2026-08-10 08:24 two `civvis` benchmark processes reached a 206 GB and a
205 GB physical footprint each on this 128 GB machine. The kernel responded
with a system-wide jetsam that terminated 14818 processes. macOS enforces
neither `ulimit -v` nor `ulimit -d` (verified on macOS 26.5.2 / arm64), so a
process here has no memory ceiling of its own.

This guard supplies the missing ceiling. It samples every process's *physical
footprint* -- not RSS. That distinction is the whole point: at the moment of
the jetsam those civvis processes held only 20 GB resident, with the other
186 GB parked in the compressor. An RSS-based limit would never have fired.

Triggers
--------
hard      one process exceeds --hard-gb, regardless of system state
pressure  system available memory falls below --pressure-pct while some
          process exceeds --soft-gb; the largest such process is killed

Only processes owned by the invoking user are ever eligible, and the names in
PROTECTED are never killed -- terminating Terminal or WindowServer would take
down every session on the machine, which is the outcome this guard exists to
prevent.
"""

from __future__ import annotations

import argparse
import os
import re
import signal
import subprocess
import sys
import time
from datetime import datetime

PAGE_BYTES = 16384

# Killing any of these takes down the whole session or the window server, which
# is worse than the runaway. They are reported but never signalled.
PROTECTED = {
    "kernel_task", "launchd", "WindowServer", "loginwindow", "Finder", "Dock",
    "SystemUIServer", "Terminal", "iTerm2", "logd", "opendirectoryd",
    "securityd", "configd", "powerd", "notifyd", "distnoted", "cfprefsd",
    "coreaudiod", "fseventsd", "diskarbitrationd", "memguard.py",
}

MEM_UNITS = {"B": 1, "K": 1024, "M": 1024**2, "G": 1024**3, "T": 1024**4}


def log(path: str, message: str) -> None:
    line = f"{datetime.now().isoformat(timespec='seconds')} {message}"
    print(line, flush=True)
    try:
        with open(path, "a") as handle:
            handle.write(line + "\n")
    except OSError:
        pass


def available_fraction() -> tuple[float, dict[str, int]]:
    """Fraction of RAM reclaimable without paging out anonymous memory.

    Measured against the jetsam report: this metric stood at 3.35% when the
    kernel started killing, versus ~90% on an idle machine, so it separates the
    two states by more than an order of magnitude.
    """
    out = subprocess.run(["vm_stat"], capture_output=True, text=True, timeout=10).stdout
    stats: dict[str, int] = {}
    for line in out.splitlines():
        match = re.match(r"(.+?):\s+(\d+)\.", line)
        if match:
            stats[match.group(1).strip()] = int(match.group(2))

    total_bytes = int(subprocess.run(
        ["sysctl", "-n", "hw.memsize"], capture_output=True, text=True, timeout=10
    ).stdout.strip())
    total_pages = total_bytes // PAGE_BYTES

    avail = (
        stats.get("Pages free", 0)
        + stats.get("Pages speculative", 0)
        + stats.get("Pages purgeable", 0)
        + stats.get("File-backed pages", 0)
    )
    return (avail / total_pages if total_pages else 1.0), stats


def parse_mem(token: str) -> int:
    match = re.match(r"^([\d.]+)([BKMGT])", token)
    if not match:
        return 0
    return int(float(match.group(1)) * MEM_UNITS[match.group(2)])


def sample_processes(limit: int = 25) -> list[tuple[int, int, str]]:
    """(pid, footprint_bytes, command) for the heaviest processes.

    top's MEM column is the physical footprint, which counts compressed pages;
    ps rss does not. Costs ~0.09s per sample.
    """
    out = subprocess.run(
        ["top", "-l", "1", "-o", "mem", "-n", str(limit), "-stats", "pid,mem,command"],
        capture_output=True, text=True, timeout=60,
    ).stdout

    rows: list[tuple[int, int, str]] = []
    started = False
    for line in out.splitlines():
        fields = line.split(None, 2)
        if not started:
            if fields[:2] == ["PID", "MEM"]:
                started = True
            continue
        if len(fields) < 3 or not fields[0].isdigit():
            continue
        rows.append((int(fields[0]), parse_mem(fields[1]), fields[2].strip()))
    return rows


def process_uid(pid: int) -> int | None:
    out = subprocess.run(
        ["ps", "-o", "uid=", "-p", str(pid)], capture_output=True, text=True, timeout=10
    ).stdout.strip()
    return int(out) if out.isdigit() else None


def full_command(pid: int) -> str:
    out = subprocess.run(
        ["ps", "-o", "command=", "-p", str(pid)], capture_output=True, text=True, timeout=10
    ).stdout.strip()
    return out or "<exited>"


def eligible(pid: int, name: str, me: int) -> tuple[bool, str]:
    if pid <= 1:
        return False, "system pid"
    if pid == os.getpid() or pid == os.getppid():
        return False, "guard itself"
    if name in PROTECTED or os.path.basename(name) in PROTECTED:
        return False, "protected name"
    uid = process_uid(pid)
    if uid is None:
        return False, "exited"
    if uid != me:
        return False, f"owned by uid {uid}"
    return True, "eligible"


def notify(title: str, message: str) -> None:
    script = (
        f'display notification {message!r} with title {title!r} sound name "Basso"'
    )
    subprocess.run(["osascript", "-e", script], capture_output=True, timeout=15)


def terminate(pid: int, logfile: str, grace: float) -> str:
    """SIGTERM, then SIGKILL if the process is still alive after `grace`."""
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        return "already exited"
    except PermissionError:
        return "permission denied"

    deadline = time.time() + grace
    while time.time() < deadline:
        time.sleep(0.25)
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return "exited on SIGTERM"
    try:
        os.kill(pid, signal.SIGKILL)
        return "SIGKILL sent"
    except ProcessLookupError:
        return "exited on SIGTERM"
    except PermissionError:
        return "permission denied on SIGKILL"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--hard-gb", type=float, default=48.0,
                        help="kill any one process above this footprint (default 48)")
    parser.add_argument("--soft-gb", type=float, default=16.0,
                        help="minimum footprint to be killable under pressure (default 16)")
    parser.add_argument("--pressure-pct", type=float, default=12.0,
                        help="available-memory %% that counts as pressure (default 12)")
    parser.add_argument("--grace", type=float, default=5.0,
                        help="seconds between SIGTERM and SIGKILL (default 5)")
    parser.add_argument("--log", default=os.path.expanduser("~/Library/Logs/memguard.log"))
    parser.add_argument("--dry-run", action="store_true",
                        help="report what would be killed, kill nothing")
    parser.add_argument("--report", action="store_true",
                        help="print current state and exit without acting")
    args = parser.parse_args()

    me = os.getuid()
    frac, _ = available_fraction()
    avail_pct = frac * 100
    procs = sample_processes()

    if args.report:
        print(f"available memory: {avail_pct:.1f}%  "
              f"(pressure threshold {args.pressure_pct}%)")
        print(f"{'PID':>8} {'FOOTPRINT':>12}  {'STATUS':<20} COMMAND")
        for pid, mem, name in procs[:12]:
            ok, why = eligible(pid, name, me)
            print(f"{pid:>8} {mem/1024**3:>9.2f} GB  "
                  f"{('killable' if ok else why):<20} {name}")
        return 0

    under_pressure = avail_pct < args.pressure_pct
    hard_bytes = int(args.hard_gb * 1024**3)
    soft_bytes = int(args.soft_gb * 1024**3)

    victim = None
    for pid, mem, name in procs:
        if mem >= hard_bytes:
            reason = (f"footprint {mem/1024**3:.1f} GB exceeds hard cap "
                      f"{args.hard_gb} GB")
        elif under_pressure and mem >= soft_bytes:
            reason = (f"available memory {avail_pct:.1f}% below "
                      f"{args.pressure_pct}% and footprint {mem/1024**3:.1f} GB "
                      f"exceeds {args.soft_gb} GB")
        else:
            continue

        ok, why = eligible(pid, name, me)
        if not ok:
            log(args.log, f"SKIP pid {pid} ({name}) {reason} -- {why}")
            continue
        victim = (pid, mem, name, reason)
        break

    if victim is None:
        if under_pressure:
            log(args.log, f"PRESSURE available={avail_pct:.1f}% but no eligible "
                          f"process above {args.soft_gb} GB")
        return 0

    pid, mem, name, reason = victim
    argv = full_command(pid)
    log(args.log, f"RUNAWAY pid={pid} name={name} footprint={mem/1024**3:.2f} GB "
                  f"available={avail_pct:.1f}% reason=({reason})")
    log(args.log, f"  argv: {argv[:500]}")

    if args.dry_run:
        log(args.log, f"  DRY RUN -- would terminate pid {pid}")
        return 0

    outcome = terminate(pid, args.log, args.grace)
    log(args.log, f"  terminated pid {pid}: {outcome}")
    notify("Memory guard killed a runaway",
           f"{name} (pid {pid}) reached {mem/1024**3:.0f} GB. See memguard.log.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
