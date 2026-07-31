#!/usr/bin/env python3
"""Does settling NEARER build a bigger empire? A paired near-window experiment.

The real-Civ-6 seat founds a median of 3 cities while CIVVIS offers it 116-603 legal
sites, and that one number explains the whole failure chain: 3 cities makes
`wantArmy = MilitaryPerCity x cities` unfillable, so the army never reaches 12, so
`declareWar` returns early — war is declared in only 19 of 47 runs — so no capital is
ever taken. Score is no escape either at 5:1 behind.

Localised with distinct `UNIT_SETTLER` ids: **484 `move_to_site` orders against 48
`found_city` — about ten turns of walking per city.** `planSite` returned the FIRST
unoccupied site in CIVVIS's ranking, and that ranking is computed FROM THE CAPITAL, so
every settler was sent to the globally best plot however far away it was.

`PlanNearWindow` lets the settler take the NEAREST of CIVVIS's top-N sites instead.
**Window 1 reproduces the old behaviour exactly**, so the control arm is the shipped
code and the treatment is one flag.

# How this is scored, and why not at turn 250

⚠ AT TURN 60, NOT AT THE END. No run in 45 has ever reached turn 250 (median 106), so
an experiment scored "at the turn limit" is scored on a turn the run will not see, and
the comparison silently degrades into "which arm survived longer". Turn 60 is inside
every arm that starts.

⚠ CITIES ARE NOT THE ONLY OUTCOME. Settling competes with the army for production, and
expansion has already starved defence to one unit at turn 40 once — the
one-defender-per-city floor exists because of that. An arm that buys 5 cities behind a
4-unit army has MOVED the constraint, not removed it, so units are reported beside
cities and neither is read alone.

⚠ Prints the mechanism as well as the outcome. `near_rank` is how far down CIVVIS's
ranking the choices sat and `near_dist` is the walk they faced: the trade is only
working if dist falls FASTER than rank rises. Rank rising with dist flat means the
window is purchasing nothing and giving up site quality to do it.

    python3 tools/civ6_near_ab.py --seeds 3
    python3 tools/civ6_near_ab.py --report
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUNS = Path.home() / "civvis-civ6-runs" / "control"
LEDGER = Path.home() / "civvis-civ6-runs" / "near_ab.jsonl"
SCORE_AT = 60


def run_arm(seed: int, window: int, turns: int, timeout: int) -> dict | None:
    before = {p.name for p in RUNS.glob("settler-*")} if RUNS.is_dir() else set()
    cmd = [
        sys.executable, str(HERE / "civ6_climb.py"),
        "--only", "DIFFICULTY_SETTLER",
        "--map", "Pangaea.lua", "--map-size", "MAPSIZE_TINY",
        "--speed", "GAMESPEED_ONLINE",
        "--seed", str(seed), "--fixed-seed",
        "--city-target", "6", "--settlers-in-flight", "1",
        "--plan-near-window", str(window),
        "--garrison-per-city", "3", "--military-per-city", "5",
        "--explore-until-turn", "25", "--assault-width", "6",
        "--war-from-turn", "55", "--war-army", "12",
        "--max-turns", str(turns), "--attempts", "1",
        "--timeout", str(timeout), "--report-every", "20", "--lock-wait", "90",
        "--export-state",
        "--window-side", "right", "--window-frac", "0.5", "--window-vfrac", "0.5",
    ]
    print(f"\n=== seed {seed}  near_window={window} ===", flush=True)
    subprocess.run(cmd, check=False)
    after = {p.name for p in RUNS.glob("settler-*")} if RUNS.is_dir() else set()
    fresh = sorted(after - before)
    if not fresh:
        print(f"  seed {seed} window {window}: no run directory appeared", flush=True)
        return None
    return summarise(RUNS / fresh[-1], seed, window)


def summarise(run_dir: Path, seed: int, window: int) -> dict | None:
    events = run_dir / "events.jsonl"
    if not events.is_file():
        return None
    at_score, last = None, None
    settlers, founds, walks = set(), 0, 0
    near_rank = near_dist = near_n = 0
    for line in events.read_text(errors="replace").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except Exception:
            continue
        kind = event.get("kind")
        if kind == "state":
            for unit in event.get("units") or []:
                if unit.get("kind") == "UNIT_SETTLER":
                    settlers.add(unit.get("id"))
        elif kind == "turn":
            last = event
            turn = event.get("turn") or 0
            if turn <= SCORE_AT:
                at_score = event
            for action, count in (event.get("actions") or {}).items():
                if "found_city" in action:
                    founds += count
                elif "move_to_site" in action:
                    walks += count
            near_rank = event.get("near_rank") or near_rank
            near_dist = event.get("near_dist") or near_dist
            near_n = event.get("near_n") or near_n
    if last is None:
        return None
    # ⚠ `reached_score_turn` is recorded so a pair where one arm died before turn
    # 60 is visible as such rather than being compared on different turns.
    return {
        "seed": seed,
        "window": window,
        "run": run_dir.name,
        "reached_score_turn": bool(at_score),
        "cities_at_60": (at_score or {}).get("cities"),
        "units_at_60": (at_score or {}).get("units"),
        "last_turn": last.get("turn"),
        "cities_last": last.get("cities"),
        "units_last": last.get("units"),
        "settlers_made": len(settlers),
        "found_city": founds,
        "move_to_site": walks,
        "walk_per_found": round(walks / founds, 1) if founds else None,
        "near_rank_mean": round(near_rank / near_n, 2) if near_n else None,
        "near_dist_mean": round(near_dist / near_n, 2) if near_n else None,
    }


def report() -> int:
    if not LEDGER.is_file():
        print("no results yet")
        return 1
    rows = [json.loads(l) for l in LEDGER.read_text().splitlines() if l.strip()]
    by_seed: dict[int, dict] = {}
    for row in rows:
        by_seed.setdefault(row["seed"], {})[row["window"]] = row

    print(f"{'seed':>8} {'window':>7} {'t60?':>5} {'cities60':>9} {'units60':>8} "
          f"{'walk/found':>11} {'rank':>6} {'dist':>6}")
    complete = better = 0
    for seed in sorted(by_seed):
        pair = by_seed[seed]
        for window in sorted(pair):
            r = pair[window]
            print(f"{seed:>8} {window:>7} {'y' if r['reached_score_turn'] else 'NO':>5} "
                  f"{str(r['cities_at_60']):>9} {str(r['units_at_60']):>8} "
                  f"{str(r['walk_per_found']):>11} {str(r['near_rank_mean']):>6} "
                  f"{str(r['near_dist_mean']):>6}")
        if len(pair) >= 2:
            windows = sorted(pair)
            ctrl, treat = pair[windows[0]], pair[windows[-1]]
            if ctrl["reached_score_turn"] and treat["reached_score_turn"]:
                complete += 1
                if (treat["cities_at_60"] or 0) > (ctrl["cities_at_60"] or 0):
                    better += 1

    print()
    if complete == 0:
        print("No pair where BOTH arms reached turn 60 — nothing comparable yet.")
        return 0
    print(f"comparable pairs: {complete}   near-window had more cities at t60 "
          f"in {better}")
    if complete < 3:
        print("⚠ Fewer than three pairs. Report the count, not a conclusion.")
    elif better == complete:
        print("→ Settling nearer wins every pair. Check units did not collapse "
              "to buy it before adopting the default.")
    elif better == 0:
        print("→ No gain. The walk was not the binding constraint; look at "
              "settler PRODUCTION rate and at the 40% of settlers that never "
              "become a city.")
    else:
        print("→ Mixed. Run more pairs.")
    print("⚠ Read `rank` against `dist`: the trade only pays if dist fell faster "
          "than rank rose. Rank up with dist flat means value given away for nothing.")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seeds", type=int, default=3)
    ap.add_argument("--base-seed", type=int, default=880011)
    ap.add_argument("--turns", type=int, default=120,
                    help="no run has ever passed 250; 120 covers the scoring turn "
                         "and the war window without burning hours per arm")
    ap.add_argument("--timeout", type=int, default=5400)
    ap.add_argument("--windows", type=int, nargs=2, default=[1, 6],
                    metavar=("CONTROL", "TREATMENT"))
    ap.add_argument("--report", action="store_true")
    args = ap.parse_args()

    if args.report:
        return report()

    LEDGER.parent.mkdir(parents=True, exist_ok=True)
    for index in range(args.seeds):
        seed = args.base_seed + index * 1009
        for window in args.windows:
            result = run_arm(seed, window, args.turns, args.timeout)
            if result is None:
                continue
            with LEDGER.open("a") as handle:
                handle.write(json.dumps(result, sort_keys=True) + "\n")
            print(f"  -> {result}", flush=True)
    return report()


if __name__ == "__main__":
    sys.exit(main())
