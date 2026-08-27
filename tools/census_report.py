#!/usr/bin/env python3
"""Run the crate's censuses and publish what they measured.

## Why

Twenty-eight of the crate's censuses carry `#[ignore]`, most with the same note:
*"census, not an assertion; run explicitly with --nocapture."* The reasoning is
right — a census is a reading, not a pass/fail, and asserting an exact number
would fail on every legitimate change. The consequence is that the project's
best diagnostic surface is invisible unless a human remembers to look, and this
week alone that pattern cost twice:

- a tactics baseline went stale because nothing re-read it, and a "21.7-point
  regression" was measured against a 40-game number instead of a 480-game one;
- the ladder loop stopped for 14.3 hours with a correct detector wired to
  nothing.

A reading nobody takes is not an instrument.

## What this does instead of asserting

It runs them, records what they printed, and makes the DRIFT the signal. The
absolute number stays free to move — that is what a census is for — but it
cannot move silently: `--check` fails when today's output differs from the
committed one, and the remedy is to look at the diff and, if the change is
intended, `--write` it.

That keeps them censuses and makes them visible, which is the whole complaint.

⚠ One exception, and it is the reason the gate could not be green even with a
fresh baseline: a census whose note says *microbenchmark* prints wall-clock
nanoseconds, which differ between two runs on one machine and certainly between
macOS and a hosted Linux runner. It is recorded and rendered like the rest —
`--check` just compares whether it still ran and passed rather than what the
stopwatch said. See `STOPWATCH_NOTE`.

## Why this is not a per-PR gate

Measured 2026-08-18: the full set takes over ten minutes here and, on the
2026-08-20 hosted runner, 75m43s for 22 of them. One plays 24 games to a
result. That is a scheduled job, not something to put in front of every pull
request — the `#[ignore]` is not laziness, it is a correct call about where
this work belongs.

    tools/census_report.py --list
    tools/census_report.py --write          # take the readings, record them
    tools/census_report.py --check          # fail on drift
    tools/census_report.py --check --jobs 4 # ... four at a time
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from concurrent import futures
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
LEDGER = REPO / "docs" / "census.json"
MARKDOWN = REPO / "docs" / "CENSUS.md"

# The note a census carries. `#[ignore]` with no reason is an ordinary skipped
# test and none of this applies to it.
CENSUS_NOTE = re.compile(r'#\[ignore\s*=\s*"([^"]*(?:census|microbenchmark)[^"]*)"\s*\]')
TEST_NAME = re.compile(r"\s*(?:async\s+)?fn\s+([A-Za-z_][\w]*)")

#: ⚠⚠ A STOPWATCH IS NOT A DETERMINISM READING, AND ONE OF THESE IS A STOPWATCH.
#:
#: `.github/workflows/census.yml` compares this ledger on Linux against a
#: baseline recorded on macOS, and calls a difference a determinism break,
#: because `docs/FLOAT_DETERMINISM.md` makes identical readings the contract.
#: That contract is about what the engine *computes*. It says nothing about how
#: long a CPU took, and `sphere_distance_cache_order_benchmark` prints
#: `median_elapsed_ns` straight off `Instant::elapsed()` — 26,225,791 ns in the
#: committed baseline. That number differs between a hosted runner and an M5
#: Max, and differs between two runs on the same machine. Pinned, it is drift
#: on every single run forever: a red X that means nothing, next to twenty-seven
#: that would mean something.
#:
#: So a timing reading is recorded and rendered like any other — the numbers
#: stay visible, and `docs/closed/SPHERE_PERFORMANCE.md` is where their
#: conclusion lives — but the drift gate compares only whether it still RAN AND
#: PASSED. The benchmark asserts its own invariant (eight distinct long queries
#: admit the reused source row), so a real regression in what it exists to watch
#: still turns this red.
#:
#: Read off the `#[ignore]` note, so `src/` stays the single source of truth and
#: nothing in the generated ledger can disagree with it. `run_one` already drops
#: harness durations for exactly this reason; this is the same judgement applied
#: to a census whose whole output is one.
STOPWATCH_NOTE = re.compile(r"microbenchmark", re.IGNORECASE)


def is_stopwatch(note: str) -> bool:
    """True when a reading's numbers are wall-clock time on the measuring host."""
    return bool(STOPWATCH_NOTE.search(note or ""))


def censuses() -> list[dict[str, str]]:
    """Every ignored test whose reason says it is a census, with where it lives."""
    found = []
    for path in sorted((REPO / "src").rglob("*.rs")):
        lines = path.read_text(encoding="utf-8", errors="replace").split("\n")
        for index, line in enumerate(lines):
            note = CENSUS_NOTE.search(line)
            if not note:
                continue
            for ahead in lines[index + 1:index + 4]:
                name = TEST_NAME.match(ahead)
                if name:
                    found.append({
                        "test": name.group(1),
                        "file": str(path.relative_to(REPO)),
                        "line": index + 1,
                        "note": note.group(1).strip(),
                    })
                    break
    return found


def run_one(test: str, timeout: float) -> dict[str, object]:
    """One census, run alone so its output is unambiguously its own."""
    done = subprocess.run(
        # ⚠ NOT `--exact`. That wants the full module path
        # (`ai::advanced::tests::belief_pressure_census`), and passing the bare
        # function name to it matches nothing at all — cargo then reports
        # "running 0 tests" and exits 0, so the census records an empty reading
        # and looks like a census that printed nothing. Substring matching on a
        # unique test name is what is wanted here.
        ["cargo", "test", "--profile", "ci", "--locked", "--lib", "--",
         "--ignored", "--nocapture", "--test-threads=1", test],
        cwd=REPO, capture_output=True, text=True, timeout=timeout, check=False,
    )
    ran = re.search(r"^running (\d+) tests?$", done.stdout, re.M)
    if ran and ran.group(1) == "0":
        return {"ok": False, "output": [f"no test matched {test!r}"]}
    body = []
    for line in done.stdout.split("\n"):
        # ⚠ THE CENSUS PRINTS ON THE HARNESS'S OWN LINE. With `--nocapture` the
        # runner writes `test path::name ... ` WITHOUT a newline and the test's
        # output lands on the end of it, so a filter that drops lines beginning
        # "test " drops exactly the reading this exists to record. Strip the
        # prefix and keep what follows.
        if line.startswith("test ") and " ... " in line:
            tail = line.split(" ... ", 1)[1].strip()
            if (tail and tail not in ("ok", "FAILED", "ignored")
                    and not re.search(r"\bin \d+(\.\d+)?s\b", tail)):
                body.append(tail)
            continue
        # The harness's remaining chatter, and anything carrying a duration,
        # which changes on every machine and would report as drift forever.
        if line.startswith(("running ", "test result:", "   Compiling",
                            "    Finished", "     Running", "warning:", "ok")):
            continue
        if re.search(r"\bin \d+(\.\d+)?s\b", line):
            continue
        if line.strip():
            body.append(line.rstrip())
    return {"ok": done.returncode == 0, "output": body}


def heaviest_first(entries: list[dict]) -> list[dict]:
    """Start the long ones first: a batch ends when its slowest member does.

    Nothing here knows a duration, and measuring one to schedule it would cost
    the run it is trying to shorten. The name is the honest proxy available —
    the deployment-scale censuses play full games to a result and are an order
    of magnitude heavier than the rest (980s against 19s on the 2026-08-22
    runner). Ordering is a scheduling hint only; it changes no reading.
    """
    return sorted(entries, key=lambda entry: (
        "deployment_scale" not in entry["test"], entry["test"]))


def believe(entry: dict, timeout: float) -> dict[str, object]:
    """One census, retried once before a failure is believed.

    ⚠ A FAILURE IS RETRIED ONCE BEFORE IT IS BELIEVED. These run for minutes on
    a machine that is also playing Civilization VI, and a cargo invocation that
    loses a build lock or gets starved returns nonzero without the census having
    failed at all. Recording that transient bakes `ok: false` into the baseline,
    and every later run then reports drift when the census simply passes again —
    measured on the first full run here, where
    `expansion_funnel_blocker_census` recorded a failure and passed on every
    attempt afterwards.
    """
    for attempt in (1, 2):
        try:
            reading = run_one(entry["test"], timeout)
        except subprocess.TimeoutExpired:
            reading = {"ok": False, "output": [f"timed out after {timeout:g}s"]}
        if reading["ok"] or attempt == 2:
            return reading
        print(f"    ({entry['test']} failed; retrying once before believing it)",
              flush=True)
    return reading


def take(timeout: float, only: str | None, jobs: int = 1) -> dict[str, object]:
    """Every census, or the ones matching `only`, keyed by test name.

    ⚠⚠ `jobs` IS WHY THE SCHEDULED JOB CAN STILL FINISH. Sequentially this set
    outgrew its runner: 22 censuses took 75m43s on the 2026-08-20 hosted runner,
    six more landed within five days, and the 08-19 and 08-22 runs were killed
    mid-reading at a ceiling that cannot be raised past GitHub's own six-hour
    job cap. Raising the ceiling buys weeks; using the cores does not run out.

    Running them concurrently is safe in a way that parallelising most things is
    not, and the reason is worth stating: **each census is a separate `cargo
    test` process replaying fixed seeds**. Nothing is shared, nothing races, and
    a count is a function of the simulation rather than of how many other
    processes are on the machine. The one reading that *is* a function of the
    machine is the microbenchmark, and `STOPWATCH_NOTE` already excludes it from
    the comparison — so contention cannot manufacture drift here.

    Sequential stays the default so a local run behaves exactly as before; the
    workflow opts in.
    """
    entries = [entry for entry in censuses()
               if not only or only in entry["test"]]
    readings: dict[str, object] = {}
    if jobs <= 1:
        for entry in entries:
            print(f"  {entry['test']} ...", flush=True)
            readings[entry["test"]] = {**entry, **believe(entry, timeout)}
        return readings

    entries = heaviest_first(entries)
    print(f"  {len(entries)} censuses, {jobs} at a time", flush=True)
    with futures.ThreadPoolExecutor(max_workers=jobs) as pool:
        submitted = {pool.submit(believe, entry, timeout): entry
                     for entry in entries}
        for done in futures.as_completed(submitted):
            entry = submitted[done]
            readings[entry["test"]] = {**entry, **done.result()}
            print(f"  {entry['test']} ... {len(readings)}/{len(entries)}",
                  flush=True)
    return readings


def render(readings: dict) -> str:
    parts = [
        "# Census readings",
        "",
        "_Generated by `tools/census_report.py --write`. These are the crate's",
        "`#[ignore]`d censuses: readings, not assertions. The numbers are free to",
        "move — that is what a census is for — but they cannot move silently, and",
        "`--check` fails on any change so the diff gets read._",
        "",
    ]
    for name in sorted(readings):
        row = readings[name]
        parts.append(f"## {name}")
        parts.append("")
        parts.append(f"`{row['file']}:{row['line']}` — {row['note']}")
        parts.append("")
        if is_stopwatch(row.get("note", "")):
            parts.append("_Wall-clock time on whichever host took the reading, so"
                         " `--check` compares only that it still ran and passed._")
            parts.append("")
        parts.append("```")
        parts.extend(row["output"] or ["(printed nothing)"])
        parts.append("```")
        parts.append("")
    return "\n".join(parts)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--list", action="store_true", help="name the censuses and exit")
    mode.add_argument("--write", action="store_true", help="take the readings and record them")
    mode.add_argument("--check", action="store_true", help="fail when a reading has drifted")
    parser.add_argument("--timeout", type=float, default=1800.0,
                        help="seconds allowed per census (default 1800)")
    parser.add_argument("--only", help="substring filter, for working on one")
    parser.add_argument("--jobs", type=int, default=1,
                        help="censuses to run at a time (default 1). Each is a "
                             "separate process replaying fixed seeds, so a "
                             "count cannot depend on how many run together")
    args = parser.parse_args(argv)

    if args.list:
        for entry in censuses():
            print(f"{entry['file']}:{entry['line']}\t{entry['test']}\t{entry['note'][:60]}")
        return 0

    readings = take(args.timeout, args.only, args.jobs)
    if not readings:
        print("no censuses matched", file=sys.stderr)
        return 1

    if args.write:
        # ⚠ `--only` narrows what was RUN, so it has to narrow what is
        # REPLACED too: a filtered run written on its own dropped the other 28
        # readings from both files (2026-08-27, #2653), and the overwrite guard
        # was what caught it. Merge the readings taken into the ledger on disk.
        if args.only and LEDGER.is_file():
            merged = json.loads(LEDGER.read_text())
            merged.update(readings)
            readings = merged
        LEDGER.write_text(json.dumps(readings, indent=2, sort_keys=True) + "\n")
        MARKDOWN.write_text(render(readings))
        print(f"recorded {len(readings)} census reading(s) to {LEDGER.name} "
              f"and {MARKDOWN.name}")
        return 0

    if not LEDGER.is_file():
        print(f"{LEDGER} does not exist yet; run --write first", file=sys.stderr)
        return 1
    before = json.loads(LEDGER.read_text())
    if args.only:
        # ⚠ `--only` narrows what was RUN, so it has to narrow what is compared
        # too. Comparing a filtered run against the whole ledger reports every
        # census that was not run as "recorded but no longer present", which is
        # 21 false alarms and one real answer.
        before = {k: v for k, v in before.items() if args.only in k}
    drifted = []
    timed = []
    for name, row in sorted(readings.items()):
        was = before.get(name)
        if was is None:
            drifted.append(f"{name}: new census, never recorded")
            continue
        # A new one is still reported above, so a timing census cannot slip in
        # unrecorded; what it skips is only the comparison of its numbers.
        if is_stopwatch(row.get("note", "")):
            timed.append(name)
            if was.get("ok") != row.get("ok"):
                drifted.append(
                    f"{name}: now {'passes' if row['ok'] else 'fails'}")
            continue
        if was.get("output") != row.get("output"):
            drifted.append(f"{name}: reading changed")
        elif was.get("ok") != row.get("ok"):
            drifted.append(f"{name}: now {'passes' if row['ok'] else 'fails'}")
    for name in sorted(set(before) - set(readings)):
        drifted.append(f"{name}: recorded but no longer present")

    for line in drifted:
        print(f"CENSUS DRIFT {line}")
    if drifted:
        print("\nA census is a reading, so a change here is not automatically a "
              "defect — read the diff. If the new number is the intended one, "
              "`--write` it and say why in the commit.")
        return 1
    print(f"{len(readings) - len(timed)} census reading(s) unchanged")
    if timed:
        print(f"{len(timed)} timing reading(s) recorded but not compared "
              f"(wall clock is not a determinism reading): " + ", ".join(timed))
    return 0


if __name__ == "__main__":
    sys.exit(main())
