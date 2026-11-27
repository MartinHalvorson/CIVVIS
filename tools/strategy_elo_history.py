#!/usr/bin/env python3
"""Every published Elo fit, in order: the record the ELO Rankings page draws
over time.

    strategy_elo_history.py append [--at ISO]   add each published fit as its
                                                 history's newest point, when
                                                 it is not that already
    strategy_elo_history.py backfill            rebuild each history from
                                                 every commit of its published
                                                 file on origin/main
    strategy_elo_history.py check               exit 1 when a history's newest
                                                 point is not its published file

There are two fits, each with its history (`FITS`): the stock game's, every
victory on (`data/strategy_elo.json`, `data/strategy_elo_history.json`), and
the one played with the Diplomatic victory switched off
(`strategy_elo --no-diplo`: `data/strategy_elo_no_diplo.json`,
`data/strategy_elo_no_diplo_history.json`; operator 2026-11-24: "add a second
elo for games with diplo off. we should maintain this because diplo on or off
makes a big difference"). A revision from before the second fit has neither of
its files, and that is no fault.

Then one fit for each named setup Tournaments shows a column for
(`strategy_elo --setup <name>`: `data/strategy_elo_<name>.json` and
`data/strategy_elo_<name>_history.json`; operator 2026-11-27: "6p Continents
Online (Diplo off, Diplo on, science, culture, religion, diplomacy,
domination, score) , 2P Pangaea (religion off, religion on). we can elo for
each"). Each is checked once it is published, like the diplo-off fit.

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
NO_DIPLO_PUBLISHED = pathlib.Path("data/strategy_elo_no_diplo.json")
NO_DIPLO_HISTORY = pathlib.Path("data/strategy_elo_no_diplo_history.json")

#: The named setups (`strategy_elo --setup`, its `SETUPS`), in the order
#: Tournaments lists their columns.
SETUPS = [
    "continents6_no_diplo", "continents6", "continents6_science", "continents6_culture",
    "continents6_religion", "continents6_diplomacy", "continents6_domination", "continents6_score",
    "pangaea2_no_religion", "pangaea2",
]

#: Each fit's published file and its history: the stock game's first, then the
#: one played with the Diplomatic victory off, then each named setup's.
FITS = [(PUBLISHED, HISTORY), (NO_DIPLO_PUBLISHED, NO_DIPLO_HISTORY)] + [
    (pathlib.Path(f"data/strategy_elo_{name}.json"), pathlib.Path(f"data/strategy_elo_{name}_history.json"))
    for name in SETUPS
]

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


def read(root: pathlib.Path, history: pathlib.Path = HISTORY) -> dict:
    try:
        return json.loads((root / history).read_text(encoding="utf-8"))
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


def append(root: pathlib.Path, at: str | None = None, published: pathlib.Path = PUBLISHED,
           history: pathlib.Path = HISTORY) -> bool:
    """Add the published file as its history's newest point; `False` when it
    already is, or when the tree has no such file."""
    if not (root / published).is_file():
        return False
    kept = read(root, history)
    fresh = point(json.loads((root / published).read_text(encoding="utf-8")), at or now())
    if kept["points"] and same_fit(kept["points"][-1], fresh):
        return False
    kept["points"].append(fresh)
    (root / history).write_text(written(kept), encoding="utf-8")
    return True


def backfill(root: pathlib.Path, ref: str = "origin/main", published: pathlib.Path = PUBLISHED,
             history: pathlib.Path = HISTORY) -> int:
    """Rebuild the history from every commit of the published file on `ref`."""
    log = subprocess.run(
        ["git", "-C", str(root), "log", "--reverse", "--format=%H %cI", ref, "--", str(published)],
        capture_output=True, text=True, check=True,
    ).stdout.split("\n")
    points: list[dict] = []
    for line in filter(None, log):
        sha, stamp = line.split()
        shown = subprocess.run(
            ["git", "-C", str(root), "show", f"{sha}:{published}"], capture_output=True, text=True, check=False
        )
        try:
            fresh = point(json.loads(shown.stdout), utc(stamp))
        except (ValueError, KeyError, TypeError):
            continue
        if not points or not same_fit(points[-1], fresh):
            points.append(fresh)
    if points:
        (root / history).write_text(written({"points": points}), encoding="utf-8")
    return len(points)


def check(root: pathlib.Path, published: pathlib.Path = PUBLISHED, history: pathlib.Path = HISTORY,
          required: bool = True) -> str | None:
    """What is wrong with the history against the published file, or `None`.
    A fit that is not `required` may be missing altogether (a revision from
    before it), but once published it wants its history."""
    if not required and not (root / published).is_file():
        return None
    kept = read(root, history)
    if not kept["points"]:
        return f"{history} has no points"
    fit = json.loads((root / published).read_text(encoding="utf-8"))
    if not same_fit(kept["points"][-1], point(fit, "")):
        return (f"{history}'s newest point is not {published}: run "
                "`python3 tools/strategy_elo_history.py append` after a rating run")
    stamps = [p["at"] for p in kept["points"]]
    if stamps != sorted(stamps):
        return f"{history}'s points are not in time order"
    return None


def check_all(root: pathlib.Path) -> str | None:
    """What is wrong with any history, or `None`: the stock fit must be
    there, every other fit only once it is published."""
    for at, (published, history) in enumerate(FITS):
        problem = check(root, published, history, required=at == 0)
        if problem:
            return problem
    return None


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "append":
        at = argv[argv.index("--at") + 1] if "--at" in argv else None
        for published, history in FITS:
            if not (ROOT / published).is_file():
                continue
            added = append(ROOT, at, published, history)
            print(f"strategy_elo_history: {history}: "
                  + ("appended the published fit" if added else "already the newest point"))
        return 0
    if len(argv) == 2 and argv[1] == "backfill":
        for published, history in FITS:
            print(f"strategy_elo_history: {history}: {backfill(ROOT, published=published, history=history)} points")
        return 0
    if len(argv) == 2 and argv[1] == "check":
        problem = check_all(ROOT)
        print(problem or "strategy_elo_history: each history's newest point is its published fit")
        return 1 if problem else 0
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
