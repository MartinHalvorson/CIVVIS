#!/usr/bin/env python3
"""Climb the Civilization VI difficulty ladder, one rung at a time, unattended.

``tools/civ6_play.py`` plays one game. This runs the campaign: take the lowest
difficulty not yet beaten, play it until it is won or the attempt budget for
that rung is spent, record the result, and move up.

It stops rather than skipping when a rung will not fall. A ladder with a hole
in it says nothing -- "beaten on Emperor" is only a claim about the controller
if every rung below it was also beaten by the same controller -- so a rung that
runs out of attempts ends the campaign and says which rung and why.

Each attempt uses a different map seed. Repeating the same seed measures the
same game over and over, and a controller that cannot beat one particular map
is a different problem from one that cannot beat the difficulty.

Usage::

    python tools/civ6_climb.py                       # from the lowest unbeaten rung
    python tools/civ6_climb.py --attempts 5 --max-turns 200
    python tools/civ6_climb.py --only DIFFICULTY_SETTLER
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import civ6_ladder as ladder  # noqa: E402

PLAY = HERE / "civ6_play.py"


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def run_attempt(difficulty: str, seed: int, args: argparse.Namespace) -> dict | None:
    tag = f"{difficulty.replace('DIFFICULTY_', '').lower()}-{utc_stamp()}"
    command = [
        sys.executable, "-u", str(PLAY),
        "--tag", tag,
        "--difficulty", difficulty,
        "--map", args.map,
        "--map-size", args.map_size,
        "--speed", args.speed,
        "--seed", str(seed),
        "--max-turns", str(args.max_turns),
        "--timeout", str(args.timeout),
        "--lock-wait", str(args.lock_wait),
        "--report-every", str(args.report_every),
        "--city-target", str(args.city_target),
        "--war-from-turn", str(args.war_from_turn),
        "--war-army", str(args.war_army),
        "--military-per-city", str(args.military_per_city),
        "--explore-until-turn", str(args.explore_until_turn),
        *([] if args.make_war else ["--no-war"]),
        "--assault-width", str(args.assault_width),
        "--settlers-in-flight", str(args.settlers_in_flight),
        "--plan-near-window", str(args.plan_near_window),
        "--garrison-per-city", str(args.garrison_per_city),
        *(["--export-state"] if args.export_state else []),
        *(["--settle-plan", args.settle_plan] if args.settle_plan else []),
        # The operator's layout: CIVVIS owns the left half, the real game the
        # upper right, a terminal beneath it. Threaded through because a ladder
        # attempt relaunches Civ 6 and it comes back wherever it last was.
        "--window-side", args.window_side,
        "--window-frac", str(args.window_frac),
        "--window-vfrac", str(args.window_vfrac),
    ]
    print(f"\n=== {difficulty} seed {seed} -> {tag} ===", flush=True)
    subprocess.run(command, check=False)
    summary = Path.home() / "civvis-civ6-runs" / "control" / tag / "summary.json"
    if not summary.is_file():
        print(f"no summary written for {tag}", file=sys.stderr)
        return None
    ladder.record(summary)
    return json.loads(summary.read_text())


def climb(args: argparse.Namespace) -> int:
    rungs = [args.only] if args.only else [key for key, _ in ladder.LADDER]
    for difficulty in rungs:
        state = ladder.load()
        if difficulty in state["wins"] and not args.only:
            continue
        for attempt in range(1, args.attempts + 1):
            # ⚠ `--fixed-seed` REPLAYS THE SAME WORLD every attempt, which is what
            # makes a CIVVIS settle plan meaningful: the plan describes one map, and
            # the map is a function of the seed. Without it each attempt is a
            # different world and a plan from the last one is describing terrain
            # that no longer exists.
            #
            # Varying seeds remains the default, because a ladder rung claimed on
            # one lucky map is not a rung.
            seed = (
                args.seed
                if args.fixed_seed
                else args.seed + attempt * 1013 + rungs.index(difficulty) * 7919
            )
            summary = run_attempt(difficulty, seed, args)
            if summary and ladder.is_win(summary) and summary.get("configured"):
                print(f"*** {difficulty} beaten on attempt {attempt} ***", flush=True)
                break
            print(f"{difficulty}: attempt {attempt} did not win "
                  f"({(summary or {}).get('reason', 'no summary')})", flush=True)
            time.sleep(args.between)
        else:
            print(f"stopping: {difficulty} not beaten in {args.attempts} attempts; "
                  f"the rungs above it would be meaningless", file=sys.stderr)
            return 1
    print(ladder.LEDGER.read_text())
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--attempts", type=int, default=3,
                    help="attempts per rung before the campaign stops")
    ap.add_argument("--only", help="play one rung rather than climbing")
    # Pangaea, not Continents: on a Duel Continents map the rival can start on
    # its own landmass, and `reachable()` is a land query — an unreachable
    # capital makes domination impossible no matter how good the army is.
    ap.add_argument("--map", default="Pangaea.lua")
    ap.add_argument("--city-target", type=int, default=3)
    ap.add_argument("--war-from-turn", type=int, default=25)
    ap.add_argument("--war-army", type=int, default=4)
    ap.add_argument("--military-per-city", type=float, default=1.5)
    ap.add_argument("--explore-until-turn", type=int, default=12)
    ap.add_argument("--make-war", dest="make_war", action="store_true", default=True)
    ap.add_argument("--no-war", dest="make_war", action="store_false")
    ap.add_argument("--assault-width", type=int, default=2)
    ap.add_argument("--settlers-in-flight", type=int, default=1)
    ap.add_argument("--plan-near-window", type=int, default=6)
    ap.add_argument("--garrison-per-city", type=int, default=2)
    ap.add_argument("--export-state", action="store_true", default=False)
    ap.add_argument("--settle-plan", default=None)
    ap.add_argument("--fixed-seed", action="store_true", default=False,
                    help="replay the same world every attempt, so a CIVVIS settle "
                         "plan still describes the map it was planned on")
    ap.add_argument("--window-side", default="left")
    ap.add_argument("--window-frac", type=float, default=0.5)
    ap.add_argument("--window-vfrac", type=float, default=1.0)
    ap.add_argument("--map-size", default="MAPSIZE_DUEL")
    ap.add_argument("--speed", default="GAMESPEED_ONLINE")
    ap.add_argument("--max-turns", type=int, default=120)
    ap.add_argument("--timeout", type=float, default=7200.0)
    ap.add_argument("--report-every", type=int, default=10)
    ap.add_argument("--seed", type=int, default=424242)
    ap.add_argument("--between", type=float, default=15.0)
    ap.add_argument("--lock-wait", type=float, default=3600.0,
                    help="seconds each attempt waits for another run to finish")
    return climb(ap.parse_args(argv))


if __name__ == "__main__":
    sys.exit(main())
