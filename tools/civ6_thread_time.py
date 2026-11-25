#!/usr/bin/env python3
"""What a running Civilization VI spends its time on, thread by thread.

    tools/civ6_thread_time.py record PID [--every 1.0] > threads.jsonl
    tools/civ6_thread_time.py read threads.jsonl RUN_DIR
    tools/civ6_thread_time.py stacks SAMPLE.txt [SAMPLE.txt ...]

Operator, 2026-11-24: "what is the main reason civvis is so much faster than
firaxis civ 6? please try to explain that gap in the chart/timeline".

The speed timeline has a real Civilization VI game with Firaxis's AI in every
seat at one stop and a simulator game at the next, about ten times faster.
A stopwatch says how long each takes. It does not say what the real game's
time is spent on, and the game's source is not ours to read. This tool reads
it from outside, from the process as it plays:

- **record** asks the kernel, once a second, how much processor time each
  thread of the process has used so far, and writes each answer as a line.
  The figures are the kernel's own running totals (`proc_pidinfo`), exact
  whatever the interval, and reading them does not stop the game. Start it
  when the game launches and it ends when the process does.
- **read** takes those lines and a run folder of `tools/civ6_play.py`
  (`events.jsonl` holds the stamp of every turn) and gives the play, from
  the first turn to the last: how long it was by the clock, how much
  processor time the process used in it, and how much of that each named
  thread used. The game names its own threads: `Game Core` plays the rules
  and the AI, `WinMain` is the window's (it draws the frame and runs the
  interface), and the `TBB` threads are workers that either hands jobs to.
- **stacks** reads the report of macOS's `sample PID SECONDS 1` and says, for
  each of those threads, in what share of the samples it was parked waiting
  to be woken (a condition variable) and in what share it was passing its
  turn while it waited on a job (`sched_yield`). The second counts as
  processor time in **read** and is not work: the thread is spinning.

`data/performance/speedup_timeline.json` holds what these gave for a Firaxis
self-play game, as its reading's `threads`, and `docs/SPEEDUP_TIMELINE.md`
says how the page draws it. macOS only: `proc_pidinfo` and `sample` are its.
"""

from __future__ import annotations

import argparse
import ctypes
import ctypes.util
import datetime
import json
import pathlib
import re
import sys
import time

PROC_PIDTHREADINFO = 5
PROC_PIDLISTTHREADS = 6

#: The threads the game names that a reading is about, by the name the page's
#: ledger keeps each under.
NAMED = {"Game Core": "game", "WinMain": "window", "TBB": "workers"}


class ThreadInfo(ctypes.Structure):
    """`struct proc_threadinfo` of `<sys/proc_info.h>`: the two times are
    nanoseconds."""

    _fields_ = [
        ("pth_user_time", ctypes.c_uint64),
        ("pth_system_time", ctypes.c_uint64),
        ("pth_cpu_usage", ctypes.c_int32),
        ("pth_policy", ctypes.c_int32),
        ("pth_run_state", ctypes.c_int32),
        ("pth_flags", ctypes.c_int32),
        ("pth_sleep_time", ctypes.c_int32),
        ("pth_curpri", ctypes.c_int32),
        ("pth_priority", ctypes.c_int32),
        ("pth_maxpriority", ctypes.c_int32),
        ("pth_name", ctypes.c_char * 64),
    ]


def snapshot(pid: int, libproc=None) -> dict | None:
    """Every thread of a process with the processor time it has used so far,
    in seconds, under the thread's handle: `{"at": clock, "threads": {handle:
    {"name", "seconds"}}}`. `None` once the process is gone."""
    libproc = libproc or ctypes.CDLL(ctypes.util.find_library("proc"), use_errno=True)
    handles = (ctypes.c_uint64 * 4096)()
    size = libproc.proc_pidinfo(pid, PROC_PIDLISTTHREADS, 0, handles, ctypes.sizeof(handles))
    if size <= 0:
        return None
    threads = {}
    for handle in handles[: size // 8]:
        info = ThreadInfo()
        got = libproc.proc_pidinfo(pid, PROC_PIDTHREADINFO, ctypes.c_uint64(handle), ctypes.byref(info), ctypes.sizeof(info))
        if got != ctypes.sizeof(info):
            continue
        threads[f"{handle:x}"] = {
            "name": info.pth_name.decode("utf-8", "replace"),
            "seconds": (info.pth_user_time + info.pth_system_time) / 1e9,
        }
    return {"at": time.time(), "threads": threads}


def named(name: str) -> str:
    """A thread's name without the number the game gives each of a kind:
    `TBB (WinID 4)` is `TBB`. A thread without a name is `(unnamed)`."""
    return re.sub(r"\s*\(WinID \d+\)$", "", name).strip() or "(unnamed)"


def spent(snapshots: list[dict], start: float, end: float) -> dict | None:
    """The processor time used between two moments, from the snapshots
    nearest each: the seconds between those two (`seconds`), the processor
    seconds every thread used in them (`cpu_seconds`) and the same for each
    name (`threads`), a name's threads added together. A thread that began
    after the first snapshot counts from nothing; one that ended before the
    last is not in it and is not counted. `None` without two snapshots a
    moment apart."""
    if len(snapshots) < 2:
        return None
    first = min(snapshots, key=lambda snap: abs(snap["at"] - start))
    last = min(snapshots, key=lambda snap: abs(snap["at"] - end))
    seconds = last["at"] - first["at"]
    if seconds <= 0:
        return None
    by_name: dict[str, float] = {}
    for handle, thread in last["threads"].items():
        before = first["threads"].get(handle, {}).get("seconds", 0.0)
        by_name[named(thread["name"])] = by_name.get(named(thread["name"]), 0.0) + thread["seconds"] - before
    return {
        "seconds": round(seconds, 1),
        "cpu_seconds": round(sum(by_name.values()), 1),
        "threads": {name: round(used, 1) for name, used in sorted(by_name.items(), key=lambda item: -item[1]) if used >= 0.05},
    }


def stamp(text: str) -> float:
    return datetime.datetime.fromisoformat(text.replace("Z", "+00:00")).timestamp()


def turns(run: pathlib.Path) -> list[tuple[int, float]]:
    """The turns of a run and the moment each began, from its `events.jsonl`."""
    found = []
    for line in (run / "events.jsonl").read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if event.get("kind") == "turn" and "turn" in event and "utc" in event:
            found.append((int(event["turn"]), stamp(event["utc"])))
    return found


def reading(snapshots: list[dict], played: list[tuple[int, float]]) -> dict | None:
    """What a ledger's reading keeps of a game's threads (`threads`): the
    play by the clock (`play_seconds`), the processor seconds the process
    used in it (`cpu_seconds`), and those of the game's own thread, the
    window's and the workers' (`game_seconds`, `window_seconds`,
    `workers_seconds`). The play is from the first turn the snapshots cover
    to the last (`from_turn`, `to_turn`): a recording begun after the game
    did is over the turns it saw, and says which."""
    covered = [(turn, at) for turn, at in played if snapshots and snapshots[0]["at"] <= at <= snapshots[-1]["at"]]
    if len(covered) < 2:
        return None
    used = spent(snapshots, covered[0][1], covered[-1][1])
    if used is None:
        return None
    row = {"from_turn": covered[0][0], "to_turn": covered[-1][0], "play_seconds": used["seconds"], "cpu_seconds": used["cpu_seconds"]}
    for name, key in NAMED.items():
        row[f"{key}_seconds"] = used["threads"].get(name, 0.0)
    return row


#: A thread parked until it is woken, and a thread passing its turn while it
#: waits on a job: the frames `sample` shows at the bottom of each.
PARKED = ("__psynch_cvwait", "semaphore_wait_trap", "__semwait_signal", "__ulock_wait", "mach_msg2_trap", "kevent")
YIELDING = ("swtch_pri", "sched_yield")
THREAD = re.compile(r"^\s{4}(\d+) Thread_\w+(.*)$")
FRAME = re.compile(r"^([\s+!:|]*?)(\d+) (\S+)")


def stacks(report: str) -> dict[str, dict]:
    """A `sample` report as each named thread's samples: how many were taken
    of it (`samples`), in how many it was parked waiting to be woken
    (`parked`) and in how many it was passing its turn (`yielding`). A
    name's threads are added together. A frame is counted where it is the
    last of its branch, so a sample is counted once."""
    found: dict[str, dict] = {}
    name = None
    graph = report.split("Call graph:", 1)[-1].split("Total number in stack", 1)[0]
    for line in graph.splitlines():
        head = THREAD.match(line)
        if head:
            # `Thread_N: its name`, or `Thread_N   DispatchQueue_N: a queue`
            # for a thread the game did not name.
            name = named(re.sub(r"\s+DispatchQueue.*$", "", head[2][1:]).strip() if head[2].startswith(":") else "")
            row = found.setdefault(name, {"samples": 0, "parked": 0, "yielding": 0})
            row["samples"] += int(head[1])
            continue
        frame = FRAME.match(line)
        if not frame or name is None:
            continue
        if frame[3] in PARKED:
            found[name]["parked"] += int(frame[2])
        elif frame[3] in YIELDING:
            found[name]["yielding"] += int(frame[2])
    return found


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    record = commands.add_parser("record", help="write a process's threads' processor time, a line a snapshot")
    record.add_argument("pid", type=int)
    record.add_argument("--every", type=float, default=1.0, help="seconds between snapshots")
    read = commands.add_parser("read", help="the play's processor time, from the snapshots and the run's turns")
    read.add_argument("snapshots", type=pathlib.Path)
    read.add_argument("run", type=pathlib.Path)
    sampled = commands.add_parser("stacks", help="how each thread waits, from `sample` reports")
    sampled.add_argument("reports", type=pathlib.Path, nargs="+")
    args = parser.parse_args(argv[1:])
    if args.command == "record":
        while True:
            taken = snapshot(args.pid)
            if taken is None:
                return 0
            print(json.dumps(taken), flush=True)
            time.sleep(args.every)
    if args.command == "read":
        snapshots = [json.loads(line) for line in args.snapshots.read_text(encoding="utf-8").splitlines() if line.strip()]
        row = reading(snapshots, turns(args.run))
        if row is None:
            print("civ6_thread_time: no play between two snapshots", file=sys.stderr)
            return 1
        print(json.dumps(row, indent=2))
        return 0
    total: dict[str, dict] = {}
    for report in args.reports:
        for name, row in stacks(report.read_text(encoding="utf-8", errors="replace")).items():
            kept = total.setdefault(name, {"samples": 0, "parked": 0, "yielding": 0})
            for key in kept:
                kept[key] += row[key]
    print(json.dumps({name: row for name, row in total.items() if name in NAMED}, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
