#!/usr/bin/env python3
"""Every published Elo fit, in order: the record the ELO Rankings page draws
over time.

    strategy_elo_history.py append [--at ISO]   add data/strategy_elo.json as
                                                 the newest point, when it is
                                                 not the newest point already
    strategy_elo_history.py backfill            rebuild the file from every
                                                 commit of data/strategy_elo.json
                                                 on origin/main
    strategy_elo_history.py check               exit 1 when the newest point
                                                 is not the published file

`data/strategy_elo.json` holds one fit, the newest; each rating run rewrites
it (`docs/ELO.md`). The operator wanted the ratings over time (2026-11-23:
"we should have a running graph over time of the top 10 elo civvis ai agents,
as well as our elo estimations benchmarks for the lv 4 - 8 firaxis civ ai"),
so each fit is also kept here, as a point: when it was published, the tables
and runs it was fitted on, and every entrant's Elo, 95% margin and bound,
with the ladder a person is shown.

The scheduled rating run (`elo_cadence.py` on the tournament host) appends
its point before it commits, and `check` (run by
`tools/test_strategy_elo_history.py`) fails a revision whose published file
is not the history's newest point, so a fit cannot ship without its point.

The Firaxis levels measured by the benchmark games are not points here: they
are read off the run ledger whenever the site is built (`beta/elo_page.py`),
so they move with every finished game rather than with a rating run.
"""

from __future__ import annotations

import datetime
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PUBLISHED = pathlib.Path("data/strategy_elo.json")
HISTORY = pathlib.Path("data/strategy_elo_history.json")

DOC = (
    "Every published strategy Elo fit, oldest first (tools/strategy_elo_history.py). A point: `at`, when it "
    "was published (the commit's time, UTC); `tables` and `runs`, what it was fitted on; `ratings`, each "
    "entrant as [elo, margin95, bound] (bound '' when the fit places it from both sides); `ladder`, the rungs "
    "as a person is shown them. Drawn over time by Charts/ELO Rankings (beta/elo_page.py)."
)


def point(published: dict, at: str) -> dict:
    """One fit as the history keeps it."""
    ratings = {}
    for key, rating in sorted((published.get("ratings") or {}).items()):
        margin = rating.get("margin95")
        if margin is None:
            margin = round(1.96 * float(rating.get("sd") or 0.0))
        ratings[key] = [round(float(rating["elo"]), 1), round(float(margin)), rating.get("bound") or ""]
    return {
        "at": at,
        "tables": published.get("tables"),
        "runs": [run.get("run") or "" for run in published.get("runs") or [] if isinstance(run, dict)],
        "ratings": ratings,
        "ladder": {rung: round(float(elo), 1) for rung, elo in sorted((published.get("ladder") or {}).items())},
    }


def same_fit(a: dict, b: dict) -> bool:
    """Whether two points are one fit (everything but when it was published)."""
    return {k: v for k, v in a.items() if k != "at"} == {k: v for k, v in b.items() if k != "at"}


def read(root: pathlib.Path) -> dict:
    try:
        return json.loads((root / HISTORY).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {"_doc": DOC, "points": []}


def written(history: dict) -> str:
    """The file, a point to a line so a run's diff is one line."""
    lines = [json.dumps(p, ensure_ascii=False, separators=(",", ":")) for p in history["points"]]
    return '{\n "_doc": ' + json.dumps(DOC) + ',\n "points": [\n  ' + ",\n  ".join(lines) + "\n ]\n}\n"


def now() -> str:
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def utc(stamp: str) -> str:
    """A git `%cI` time as UTC."""
    at = datetime.datetime.fromisoformat(stamp).astimezone(datetime.timezone.utc)
    return at.strftime("%Y-%m-%dT%H:%M:%SZ")


def append(root: pathlib.Path, at: str | None = None) -> bool:
    """Add the published file as the newest point; `False` when it already is."""
    history = read(root)
    published = json.loads((root / PUBLISHED).read_text(encoding="utf-8"))
    fresh = point(published, at or now())
    if history["points"] and same_fit(history["points"][-1], fresh):
        return False
    history["points"].append(fresh)
    (root / HISTORY).write_text(written(history), encoding="utf-8")
    return True


def backfill(root: pathlib.Path, ref: str = "origin/main") -> int:
    """Rebuild the history from every commit of the published file on `ref`."""
    log = subprocess.run(
        ["git", "-C", str(root), "log", "--reverse", "--format=%H %cI", ref, "--", str(PUBLISHED)],
        capture_output=True, text=True, check=True,
    ).stdout.split("\n")
    points: list[dict] = []
    for line in filter(None, log):
        sha, stamp = line.split()
        shown = subprocess.run(
            ["git", "-C", str(root), "show", f"{sha}:{PUBLISHED}"], capture_output=True, text=True, check=False
        )
        try:
            fresh = point(json.loads(shown.stdout), utc(stamp))
        except (ValueError, KeyError, TypeError):
            continue
        if not points or not same_fit(points[-1], fresh):
            points.append(fresh)
    (root / HISTORY).write_text(written({"points": points}), encoding="utf-8")
    return len(points)


def check(root: pathlib.Path) -> str | None:
    """What is wrong with the history against the published file, or `None`."""
    history = read(root)
    if not history["points"]:
        return f"{HISTORY} has no points"
    published = json.loads((root / PUBLISHED).read_text(encoding="utf-8"))
    if not same_fit(history["points"][-1], point(published, "")):
        return (f"{HISTORY}'s newest point is not {PUBLISHED}: run "
                "`python3 tools/strategy_elo_history.py append` after a rating run")
    stamps = [p["at"] for p in history["points"]]
    if stamps != sorted(stamps):
        return f"{HISTORY}'s points are not in time order"
    return None


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "append":
        at = argv[argv.index("--at") + 1] if "--at" in argv else None
        added = append(ROOT, at)
        print("strategy_elo_history: " + ("appended the published fit" if added else "already the newest point"))
        return 0
    if len(argv) == 2 and argv[1] == "backfill":
        print(f"strategy_elo_history: {backfill(ROOT)} points")
        return 0
    if len(argv) == 2 and argv[1] == "check":
        problem = check(ROOT)
        print(problem or "strategy_elo_history: the newest point is the published fit")
        return 1 if problem else 0
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
