#!/usr/bin/env python3
"""Does the same seed now produce the same world? The check that unlocks measurement.

`MapRandomSeed` and `GameRandomSeed` ship with NO DefaultValue, which is why the
fixed-seed flag was inert: four runs launched with `--seed 425255 --fixed-seed` drew
capitals at (44,20), (19,30) and (13,27) and civs AMERICA / SWEDEN / KOREA / SWEDEN. The
setup-defaults `UpdateDatabase` now writes both seed rows (verified in the live config
cache), so this asks whether that reaches world generation.

# Why this is worth more than the map type

Without a reproducible world, **no A/B on real Civ 6 can be paired** — map luck dominates
and swamps any treatment, which is why `tools/civ6_ab.py` and `tools/civ6_near_ab.py` are
both invalid as written. Every finding today had to be argued as a MECHANISM claim
("0 buildings became a monument", "loyalty -23/turn became +17") because outcome claims
were unavailable. Pinning the world restores outcome comparison, and that is the
foundation under every future measurement here — including "did this change win more
games", which is the only question the operator's goal actually cares about.

# Why it is cheap

The capital is in the first `state` export, so each arm needs about two turns, not two
hundred. ⚠ Deliberately NOT a full game: a long run would confound the answer with
everything that happens after turn 2.

# ⚠⚠ IT HAS ALREADY RUN, AND THE ANSWER WAS NO (2026-07-30)

    seed 777001, arm 1: capital (51,13)  SCOTLAND  Continents.lua
    seed 777001, arm 2: capital (19,11)  ARABIA    Continents.lua

Both seed rows were verified written in the live config cache at the time, and the world was
still freshly random. So `Parameters.DefaultValue` is not the source of the map seed at
generation time — just as it is not the source of MAP_SCRIPT. The UpdateDatabase channel
reaches the DATABASE and not WORLD GENERATION.

Keep this tool: re-run it after any change that claims to pin the map, because the claim is
cheap to make and this is the only thing that settles it. But do not re-run it expecting a
different answer from another edit to `Parameters`.

⚠ `--max-turns` sets the GAME's turn limit and does NOT stop the harness early: an arm asked
for 4 turns played to 38. The capital is in the first `state` export, so read it and kill the
arm rather than waiting for the arm to finish.

    python3 tools/civ6_seed_check.py                 # two arms on one seed
    python3 tools/civ6_seed_check.py --seed 777001 --arms 3
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUNS = Path.home() / "civvis-civ6-runs" / "control"


def first_world(run_dir: Path) -> dict | None:
    """Capital, civ and map script from the earliest events — the world's identity."""
    events = run_dir / "events.jsonl"
    if not events.is_file():
        return None
    seat, capital, turn = None, None, None
    for line in events.read_text(errors="replace").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except Exception:
            continue
        if event.get("kind") == "seat" and seat is None:
            seat = event
        elif event.get("kind") == "state" and capital is None:
            for city in event.get("cities") or []:
                if city.get("capital"):
                    capital = (city.get("x"), city.get("y"))
                    turn = event.get("turn")
                    break
    if seat is None:
        return None
    return {
        "run": run_dir.name,
        "map": seat.get("map"),
        "civ": seat.get("civ"),
        "leader": seat.get("leader"),
        "capital": capital,
        "capital_seen_turn": turn,
    }


def run_arm(seed: int, turns: int, timeout: int) -> dict | None:
    before = {p.name for p in RUNS.glob("settler-*")} if RUNS.is_dir() else set()
    cmd = [
        sys.executable, str(HERE / "civ6_climb.py"),
        "--only", "DIFFICULTY_SETTLER",
        "--map", "Pangaea.lua", "--map-size", "MAPSIZE_TINY",
        "--speed", "GAMESPEED_ONLINE",
        "--seed", str(seed), "--fixed-seed",
        "--max-turns", str(turns), "--attempts", "1",
        "--timeout", str(timeout), "--lock-wait", "120",
        "--report-every", "1", "--export-state",
        "--window-side", "right", "--window-frac", "0.5", "--window-vfrac", "0.5",
    ]
    print(f"\n=== seed {seed}, {turns} turns ===", flush=True)
    subprocess.run(cmd, check=False)
    after = {p.name for p in RUNS.glob("settler-*")} if RUNS.is_dir() else set()
    fresh = sorted(after - before)
    if not fresh:
        print("  no run directory appeared", flush=True)
        return None
    return first_world(RUNS / fresh[-1])


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=777001)
    ap.add_argument("--arms", type=int, default=2)
    ap.add_argument("--turns", type=int, default=4,
                    help="only needs to reach the first state export")
    ap.add_argument("--timeout", type=int, default=1500)
    args = ap.parse_args()

    worlds = []
    for _ in range(args.arms):
        world = run_arm(args.seed, args.turns, args.timeout)
        if world:
            worlds.append(world)
            print(f"  -> {world}", flush=True)
        # ⚠ Quit between arms: killing a harness leaves Civ 6 holding the run lock,
        # and civ6_climb counts a lock collision as a spent attempt.
        subprocess.run(["osascript", "-e", 'tell application "Civ6" to quit'],
                       capture_output=True)
        time.sleep(6)
        subprocess.run(["pkill", "-f", "Civ6_Exe"], capture_output=True)
        time.sleep(3)

    print()
    if len(worlds) < 2:
        print(f"only {len(worlds)} arm(s) produced a world — cannot compare")
        return 1

    print(f"{'run':>10} {'map':>16} {'civ':>26} {'capital':>12}")
    for w in worlds:
        print(f"{w['run'][-7:]:>10} {str(w['map']):>16} {str(w['civ']):>26} "
              f"{str(w['capital']):>12}")
    print()
    caps = {w["capital"] for w in worlds}
    civs = {w["civ"] for w in worlds}
    maps = {w["map"] for w in worlds}
    # ⚠ All three must match. The capital alone is the strongest single signal, but a
    # matching capital with a differing civ would mean the world repeats while the seat
    # does not — still unusable for pairing, and worth seeing rather than glossing.
    if len(caps) == 1 and None not in caps:
        print(f"→ CAPITAL IS REPRODUCIBLE at {caps.pop()}: the seed reaches world gen.")
        if len(civs) == 1 and len(maps) == 1:
            print("  civ and map also identical — PAIRED EXPERIMENTS ARE AVAILABLE.")
            print("  ⚠ Un-invalidate civ6_ab.py / civ6_near_ab.py before trusting them: "
                  "they were written against a seed that did nothing.")
        else:
            print(f"  ⚠ but civ varies {civs} and/or map varies {maps} — the world "
                  "repeats and the SEAT does not, so pairing is still not sound.")
        return 0
    if None in caps:
        print("⚠ At least one arm never reported a capital — inconclusive, not a "
              "negative result. Re-run before concluding.")
        return 1
    print(f"→ CAPITALS DIFFER {caps}: the seed still does not reach world generation.")
    print("  So the seed rows being set in the config database is NOT sufficient, and")
    print("  pairing remains unavailable. Keep stating findings as MECHANISM claims.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
