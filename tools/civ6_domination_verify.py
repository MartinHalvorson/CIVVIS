"""Keep running native King-5 Gran Colombia Domination verification games.

Launch from a GUI-authorized Terminal after Civ VI is closed:
    python3 -u tools/civ6_domination_verify.py

Each game runs in its own process so launcher cleanup cannot act on a later
game. The shared game lock makes this wait for any other verification run.
"""

from __future__ import annotations

import argparse
import datetime as dt
import shlex
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = Path(__file__).resolve()


def game_args() -> list[str]:
    forced = [
        gene
        for gene in (ROOT / "deploy/live-force-on.txt").read_text().strip().split(",")
        if gene
    ]
    if "one-war-at-a-time" not in forced:
        forced.append("one-war-at-a-time")
    tag = "civvis-domination-combined-" + dt.datetime.now(
        dt.timezone.utc
    ).strftime("%Y%m%dT%H%M%SZ")
    args = [
        "--tag", tag,
        "--difficulty", "DIFFICULTY_KING",
        "--ruleset", "RULESET_EXPANSION_2",
        "--leader", "LEADER_SIMON_BOLIVAR",
        "--map", "Pangaea.lua",
        "--map-size", "MAPSIZE_TINY",
        "--speed", "GAMESPEED_ONLINE",
        "--max-turns", "650",
        "--civvis-decides",
        "--peace-deal-sessions",
        "--civvis-victory", "domination",
        "--civvis-bin", str(ROOT / "target/release/civvis_orders"),
        "--civvis-refresh-seconds", "0",
        "--lock-wait", "7200",
    ]
    for gene in forced:
        args.extend(("--civvis-with", gene))
    return args


def run_one() -> int:
    subprocess.run(
        ["cargo", "build", "--release", "--bin", "civvis_orders"],
        cwd=ROOT,
        check=True,
    )
    sys.path.insert(0, str(ROOT / "tools"))
    import civ6_play  # noqa: E402

    original_play = civ6_play._play

    def play_after_previous_game_exits(args: argparse.Namespace) -> int:
        print("Game lock acquired; allowing 120 seconds for shutdown", flush=True)
        time.sleep(120)
        return original_play(args)

    civ6_play._play = play_after_previous_game_exits
    args = game_args()
    print("Native verification:", shlex.join(args), flush=True)
    return civ6_play.main(args)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--once", action="store_true", help="play one game")
    parser.add_argument("--dry-run", action="store_true", help="show the game command")
    parser.add_argument("--retry-seconds", type=float, default=120)
    parser.add_argument("--child-once", action="store_true", help=argparse.SUPPRESS)
    options = parser.parse_args()
    if options.dry_run:
        print(shlex.join(game_args()))
        return 0
    if options.child_once:
        return run_one()

    while True:
        try:
            result = subprocess.run([sys.executable, "-u", str(SCRIPT), "--child-once"])
            print(f"Native verification exited {result.returncode}", flush=True)
            if options.once:
                return result.returncode
            time.sleep(max(30.0, options.retry_seconds))
        except KeyboardInterrupt:
            return 130


if __name__ == "__main__":
    raise SystemExit(main())
