#!/usr/bin/env python3
"""Refuse a PR that silently deletes freshly merged work.

On 2026-08-05 the operator asked what had been overwriting what across the
fleet. Blaming every line deleted on `main` over the preceding week answered
it: sequential rewrites of the same hot regions — machines redesigning the
map-lens panel and the volcano tiles hours apart (#1109 → #1138 → #1191 →
#1199, #1153 → #1185 → #1226), and wide purges sweeping along fresh unrelated
lines (#969, #1194, #1155). Every one of those PRs merged green: nothing in CI
asked "who wrote the lines you are deleting, and how recently?" The damage was
only visible afterwards, in `Restore ...` commits (#1124, #1146).

This check asks that question before the merge. It blames every line the PR
deletes, at the PR's merge base. Deleting *old* lines is ordinary maintenance
and passes silently. Deleting lines that landed on `main` within the last
KEEP_DAYS is either deliberate supersession — in which case saying so costs
one line of the PR body — or exactly the accident this repository kept having.

A deletion of young work is acknowledged by naming its source anywhere in the
PR body: the victim PR's `#number` (a `Supersedes: #N` or `Coordinated with:
#N` line both qualify) or the victim commit's short sha. An acknowledged
deletion always passes; the check never judges whether replacing the work is
wise, only whether it is stated. `overwrite-guard: allow` in the body waives
the whole check for genuinely bulk rewrites — grep-able, so a waiver is a
recorded decision, not a silence.

Thresholds, tuned on the week that motivated this: a victim is reported when
the PR deletes >= 25 of its young lines, or when the PR deletes >= 50 young
lines across all victims combined (that aggregate is what caught #1155, which
shaved 7-14 lines from each of four fresh PRs). Adjacent-hunk churn of a few
lines never trips it. `AI_PLAYER_ELO_RANKINGS.md` is exempt: league refreshes
replace that whole snapshot several times a day by design.

    ./tools/overwrite_guard.py --base <merge-base> --head <sha> [--body-file F]

Exit 0 = clean or acknowledged. Exit 1 = unacknowledged young deletions, with
one `::error` line per victim naming the exact acknowledgment to add.
"""

from __future__ import annotations

import argparse
import collections
import os
import re
import subprocess
import sys
import time

KEEP_DAYS = 7
PER_VICTIM = 25
AGGREGATE = 50
MAX_RANGES_PER_FILE = 400
SNAPSHOT_PATHS = {"AI_PLAYER_ELO_RANKINGS.md"}
WAIVER = "overwrite-guard: allow"


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], capture_output=True, text=True, encoding="utf-8",
        errors="replace", check=False,
    ).stdout


def deleted_ranges(base: str, head: str) -> dict[str, list[tuple[int, int]]]:
    """Old-side line ranges the PR deletes, per file, renames followed.

    Rename detection stays ON: with it off, a renamed file reads as a total
    deletion of young lines and a plain workflow rename (482215ef5) would be
    the loudest overwrite of the week.
    """
    out = git("diff", "-U0", "-M", "--no-color", base, head)
    ranges: dict[str, list[tuple[int, int]]] = collections.defaultdict(list)
    current = None
    for line in out.splitlines():
        if line.startswith("--- "):
            name = line[4:]
            current = None if name == "/dev/null" else name[2:] if name.startswith("a/") else name
        elif line.startswith("@@ ") and current:
            m = re.match(r"@@ -(\d+)(?:,(\d+))? \+", line)
            if not m:
                continue
            start = int(m.group(1))
            count = int(m.group(2)) if m.group(2) is not None else 1
            if count > 0:
                ranges[current].append((start, start + count - 1))
    return ranges


def young_victims(base: str, head: str, now: float) -> dict[str, int]:
    """Deleted-young-line counts keyed by the commit that wrote those lines."""
    victims: collections.Counter[str] = collections.Counter()
    cutoff = now - KEEP_DAYS * 86400
    for path, ranges in deleted_ranges(base, head).items():
        if path in SNAPSHOT_PATHS:
            continue
        args = ["blame", "--line-porcelain", base]
        for start, end in ranges[:MAX_RANGES_PER_FILE]:
            args += ["-L", f"{start},{end}"]
        args += ["--", path]
        blame = git(*args)
        if not blame:
            continue  # binary, unreadable, or path absent at base
        origin = None
        for row in blame.splitlines():
            m = re.match(r"^([0-9a-f]{40}) \d+ \d+", row)
            if m:
                origin = m.group(1)
            elif row.startswith("committer-time ") and origin:
                if int(row.split()[1]) >= cutoff:
                    victims[origin] += 1
    return dict(victims)


def acknowledged(body: str, sha: str, subject: str) -> bool:
    """A victim is acknowledged by its PR number or its commit sha prefix."""
    if sha[:7] in body:
        return True
    m = re.search(r"\(#(\d+)\)\s*$", subject)
    return bool(m) and f"#{m.group(1)}" in body


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True, help="merge base of the PR")
    parser.add_argument("--head", required=True, help="PR head sha")
    parser.add_argument("--body-file", help="file holding the PR body")
    parser.add_argument("--now", type=float, default=None,
                        help="override the clock (tests)")
    args = parser.parse_args(argv)

    body = ""
    if args.body_file and os.path.exists(args.body_file):
        with open(args.body_file, encoding="utf-8", errors="replace") as handle:
            body = handle.read()
    if WAIVER in body:
        print(f"waived: the body carries '{WAIVER}'")
        return 0

    victims = young_victims(args.base, args.head, args.now or time.time())
    if not victims:
        print("no young deletions; nothing to acknowledge")
        return 0

    unacknowledged = []
    for sha, count in sorted(victims.items(), key=lambda kv: -kv[1]):
        subject = git("log", "-1", "--format=%s", sha).strip()
        if acknowledged(body, sha, subject):
            print(f"    ok   -{count:<5} {sha[:9]} {subject[:70]} (acknowledged)")
        else:
            unacknowledged.append((sha, count, subject))

    flagged = [v for v in unacknowledged if v[1] >= PER_VICTIM]
    total = sum(count for _, count, _ in unacknowledged)
    if not flagged and total < AGGREGATE:
        for sha, count, subject in unacknowledged:
            print(f"    ok   -{count:<5} {sha[:9]} {subject[:70]} (below threshold)")
        return 0

    reportable = flagged or unacknowledged
    print(
        f"\nThis PR deletes {total} lines merged to main within the last "
        f"{KEEP_DAYS} days without saying so.",
        file=sys.stderr,
    )
    for sha, count, subject in reportable:
        m = re.search(r"\(#(\d+)\)\s*$", subject)
        name = f"#{m.group(1)}" if m else sha[:9]
        print(
            f"::error title=overwrite-guard::deletes {count} recent lines from "
            f"{name} ({subject[:80]}). If replacing that work is intended, add "
            f"'Supersedes: {name}' to the PR body; otherwise rebuild on top of it.",
        )
    return 1


if __name__ == "__main__":
    sys.exit(main())
