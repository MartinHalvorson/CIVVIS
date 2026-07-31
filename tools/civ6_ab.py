#!/usr/bin/env python3
"""Does war pay? A paired war / no-war experiment on identical maps.

Every run on 2026-07-30 that reached a war showed the same shape: AHEAD before it
(lead +19, +25, +10, +11 across four runs) and collapsing after (cities 4→1, army
14→3, lead −200 to −440). Every combat mechanism now works — rams built and
positioned, a ranged floor, six-wide assaults, a mass gate at twelve, war deferred to
turn 55 — and **no city has ever been captured**. So the question is no longer how to
assault better. It is whether declaring at all is net-negative for this agent.

This runs both arms on the SAME seeds and reports what each did with the same map.

# Why paired, and why on a fixed seed

CIVVIS's own history is full of unpaired results that did not survive. A single run's
score says almost nothing here: map luck dominates, one run reached five cities and
another one city on the same settings. Pairing removes the map as a variable — each
seed is played twice, once with war enabled and once without — so the difference is
attributable to the treatment rather than to terrain.

⚠ AN EARLIER PEACE-VS-WAR COMPARISON EXISTS AND IS WORTHLESS. Both of its arms ran
through the truncated unit roster (`eachUnit` wrapped one pcall around the whole walk,
so the first throwing callback abandoned the rest), which means neither arm had the army
it thought it had. Do not cite it, and do not treat this as a re-run of it: this is the
first version of the question asked with working units.

⚠ WHAT A NULL RESULT WOULD MEAN, decided in advance so the answer cannot be
rationalised afterwards:

* war arm clearly better  → the combat work pays; keep tuning the assault.
* peace arm clearly better → war is a trap at this level of play; the agent should
  develop, and domination is off the table until the army is a different thing
  entirely.
* both lose badly         → NEITHER victory route is reachable at this standard, which
  is the most useful of the three answers and the one that should redirect the work.
  Score needs us ahead and the AI outscores us roughly 5:1 late, so peace probably
  cannot win either — it would merely lose more slowly.

    python3 tools/civ6_ab.py --seeds 3 --turns 250
    python3 tools/civ6_ab.py --report            # summarise whatever has finished
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUNS = Path.home() / "civvis-civ6-runs" / "control"
LEDGER = Path.home() / "civvis-civ6-runs" / "war_ab.jsonl"


def run_arm(seed: int, make_war: bool, turns: int, timeout: int) -> dict | None:
    """One game. Returns the last turn's telemetry, or None if it never started."""
    before = {p.name for p in RUNS.glob("settler-*")} if RUNS.is_dir() else set()
    cmd = [
        sys.executable, str(HERE / "civ6_climb.py"),
        "--only", "DIFFICULTY_SETTLER",
        "--map-size", "MAPSIZE_TINY", "--speed", "GAMESPEED_ONLINE",
        "--seed", str(seed), "--fixed-seed",
        "--city-target", "6", "--settlers-in-flight", "1",
        "--garrison-per-city", "3", "--military-per-city", "5",
        "--explore-until-turn", "25", "--assault-width", "6",
        "--war-from-turn", "55", "--war-army", "12",
        "--max-turns", str(turns), "--attempts", "1",
        "--timeout", str(timeout), "--report-every", "25", "--lock-wait", "90",
        "--export-state",
        "--window-side", "right", "--window-frac", "0.5", "--window-vfrac", "0.5",
    ]
    if not make_war:
        cmd.append("--no-war")
    print(f"\n=== seed {seed}  war={make_war} ===", flush=True)
    subprocess.run(cmd, check=False)

    after = {p.name for p in RUNS.glob("settler-*")} if RUNS.is_dir() else set()
    fresh = sorted(after - before)
    if not fresh:
        print(f"  seed {seed} war={make_war}: no run directory appeared", flush=True)
        return None
    return summarise(RUNS / fresh[-1], seed, make_war)


def summarise(run_dir: Path, seed: int, make_war: bool) -> dict | None:
    events = run_dir / "events.jsonl"
    if not events.is_file():
        return None
    turns, wars, victory, defeat = [], 0, None, None
    for line in events.read_text(errors="replace").splitlines():
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except Exception:
            continue
        kind = event.get("kind")
        if kind == "turn":
            turns.append(event)
        elif kind == "war":
            wars += 1
        elif kind == "victory":
            victory = event
        elif kind == "defeat" and event.get("ours"):
            defeat = event
    if not turns:
        return None
    last = turns[-1]
    peak_cities = max((t.get("cities") or 0) for t in turns)
    return {
        "seed": seed,
        "war": make_war,
        "run": run_dir.name,
        "last_turn": last.get("turn"),
        "cities": last.get("cities"),
        "peak_cities": peak_cities,
        "score": last.get("score"),
        "rival_best": last.get("rival_best"),
        "lead": last.get("lead"),
        "wars_declared": wars,
        # ⚠ The outcome, not a proxy for it. A run that merely survived to the turn
        # limit has not won; `VICTORY_SCORE` needs us AHEAD.
        "won": bool(victory),
        "eliminated": bool(defeat),
    }


def report() -> int:
    if not LEDGER.is_file():
        print("no results yet")
        return 1
    rows = [json.loads(l) for l in LEDGER.read_text().splitlines() if l.strip()]
    by_seed: dict[int, dict] = {}
    for row in rows:
        by_seed.setdefault(row["seed"], {})[row["war"]] = row

    print(f"{'seed':>8}  {'arm':<6} {'turn':>5} {'cities':>7} {'peak':>5} "
          f"{'score':>6} {'rival':>6} {'lead':>6}  outcome")
    complete = 0
    war_better = 0
    for seed in sorted(by_seed):
        pair = by_seed[seed]
        for war in (True, False):
            row = pair.get(war)
            if row is None:
                print(f"{seed:>8}  {'war' if war else 'peace':<6} {'—':>5}  (not run)")
                continue
            outcome = ("WON" if row["won"] else
                       "eliminated" if row["eliminated"] else "survived, behind")
            print(f"{seed:>8}  {'war' if war else 'peace':<6} {row['last_turn']:>5} "
                  f"{row['cities']:>7} {row['peak_cities']:>5} {row['score']:>6} "
                  f"{str(row['rival_best']):>6} {str(row['lead']):>6}  {outcome}")
        if len(pair) == 2:
            complete += 1
            if (pair[True]["score"] or 0) > (pair[False]["score"] or 0):
                war_better += 1

    print()
    if complete == 0:
        print("No complete pair yet. A single arm says nothing — map luck dominates.")
        return 0
    print(f"complete pairs: {complete}   war scored higher in {war_better}")
    if complete < 3:
        print("⚠ Fewer than three pairs. Report the count, not a conclusion: this "
              "project has repeatedly been misled by two-cell results.")
    elif war_better == 0:
        print("→ War lost every pair. Treat domination as unreachable at this "
              "standard of play and redirect to development.")
    elif war_better == complete:
        print("→ War won every pair. The combat work pays; keep tuning the assault.")
    else:
        print("→ Mixed. Not enough signal to redirect; run more pairs.")
    if all(not r["won"] for r in rows):
        print("⚠ NO ARM HAS WON A GAME. The interesting comparison may be between "
              "two ways of losing, which is worth saying out loud rather than "
              "reporting the less-bad one as progress.")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seeds", type=int, default=3)
    ap.add_argument("--base-seed", type=int, default=770001)
    ap.add_argument("--turns", type=int, default=250)
    ap.add_argument("--timeout", type=int, default=9000)
    ap.add_argument("--report", action="store_true")
    args = ap.parse_args()

    if args.report:
        return report()

    LEDGER.parent.mkdir(parents=True, exist_ok=True)
    for index in range(args.seeds):
        seed = args.base_seed + index * 977
        for make_war in (True, False):
            result = run_arm(seed, make_war, args.turns, args.timeout)
            if result is None:
                continue
            with LEDGER.open("a") as handle:
                handle.write(json.dumps(result, sort_keys=True) + "\n")
            print(f"  -> {result}", flush=True)
    return report()


if __name__ == "__main__":
    sys.exit(main())
