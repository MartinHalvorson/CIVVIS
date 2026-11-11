#!/usr/bin/env python3
"""Does a shortcut cost a site seat its game? Seats that take it against seats that do not.

A cache is checked by comparing saves: the same game, faster. A *shortcut*
changes what a seat decides, so there is no same game to compare, and the
only question left is the one that matters to somebody watching: do the seats
that take it lose to the seats that do not?

`civvis site-bench --shortcuts NAME --shortcut-seats IDS` plays the world
civvis.ai plays with the named `SiteShortcuts` on for some major seats and off
for the rest. This plays every seed twice with the chairs swapped — the first
half of the majors take the shortcut in one game, the second half in the
other — and reads the pair:

    +1   the shortcut's seats won both games
     0   one each, which is what a shortcut that changes nothing gives:
         the same civilization wins both, and it took the shortcut once
    -1   the seats without it won both

A game nobody won counts half to each side. The verdict is on the mean of
that pair difference, against a margin fixed before the first game:

    NOT WORSE    the 95% interval's lower end is above -margin
    WORSE        its upper end is below -margin
    UNRESOLVED   anything else: play more pairs or drop the shortcut

By hand, and it is a lot of games: 150 pairs is 300 complete games.

    tools/site_pair.py --binary target/ci/civvis \\
        --shortcuts envelopes-across-own-moves --pairs 150 --jobs 4

## Why the margin is not zero, and what it is

No number of games shows a shortcut costs *nothing*. The bar is that it does
not cost more than `--margin` pairs per pair, 0.15 by default: seats that take
it winning about 42.5% of decided games against 50% would sit exactly on it.
`BasicAi::envelope_cache_across_own_moves` was priced once before, on the gene
screen's world, and lost at 43.8% (`docs/SIMULATOR_PERFORMANCE.md`,
2026-08-19); a bar that would have passed that reading is not a bar.

## What it also prints

The seconds each side's agents spent, from the same games: the shortcut's
seats beside the others in one process on one host, so the ratio is the
saving and does not move with the load. It is the saving to those seats'
own turns, not to a whole frame.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import math
import re
import statistics
import subprocess
import sys
from pathlib import Path
from typing import Dict, List, Optional, Sequence

#: The bar, fixed before any game: see the header.
MARGIN = 0.15

WINNER = re.compile(r"^Winner: .* \(player (\d+)\) by (.+) on turn (\d+)", re.MULTILINE)
UNDECIDED = re.compile(r"^(?:Draw: turn limit reached on turn|No winner: turn) (\d+)",
                       re.MULTILINE)
SEATS = re.compile(r"^\[major seats, player id and seconds: (.*?) \|", re.MULTILINE)


def game_command(binary: Path, seed: int, turns: int, shortcuts: str,
                 seats: Sequence[int]) -> List[str]:
    return [str(binary), "site-bench", "--seed", str(seed), "--turns", str(turns),
            "--no-documents", "--shortcuts", shortcuts,
            "--shortcut-seats", ",".join(str(seat) for seat in seats)]


def parse_game(text: str) -> dict:
    """What one `site-bench` report says: who won, when, and each major's seconds."""
    seats = SEATS.search(text)
    if not seats:
        raise ValueError(
            "no `[major seats, player id and seconds: …]` line in the report; "
            "this binary predates `site-bench --shortcuts`")
    seconds: Dict[int, float] = {}
    for entry in seats.group(1).split():
        seat, _, spent = entry.partition("=")
        seconds[int(seat)] = float(spent)
    won = WINNER.search(text)
    undecided = UNDECIDED.search(text)
    return {
        "winner": int(won.group(1)) if won else None,
        "victory": won.group(2) if won else None,
        "turn": int(won.group(3)) if won else (int(undecided.group(1)) if undecided else None),
        "seconds": seconds,
    }


def split_majors(majors: Sequence[int]) -> tuple[List[int], List[int]]:
    """The two sets of chairs: the first half of the majors, and the rest."""
    ordered = sorted(majors)
    half = len(ordered) // 2
    if half == 0:
        raise ValueError("a paired test needs at least two major seats")
    return ordered[:half], ordered[half:]


def treated_score(game: dict, treated: Sequence[int]) -> float:
    """1 if a seat that took the shortcut won, 0 if another did, half if nobody."""
    if game["winner"] is None:
        return 0.5
    return 1.0 if game["winner"] in treated else 0.0


def time_ratio(game: dict, treated: Sequence[int]) -> Optional[float]:
    """Seconds per seat for the shortcut's seats over the same for the others."""
    with_it = [spent for seat, spent in game["seconds"].items() if seat in treated]
    without = [spent for seat, spent in game["seconds"].items() if seat not in treated]
    if not with_it or not without or min(without) <= 0:
        return None
    return statistics.fmean(with_it) / statistics.fmean(without)


def summarise(differences: Sequence[float], margin: float = MARGIN) -> dict:
    """The mean pair difference, its 95% interval, and what the bar makes of it."""
    count = len(differences)
    if count < 2:
        raise ValueError("a verdict needs at least two pairs")
    mean = statistics.fmean(differences)
    error = statistics.stdev(differences) / math.sqrt(count)
    low, high = mean - 1.96 * error, mean + 1.96 * error
    if low > -margin:
        verdict = "NOT WORSE"
    elif high < -margin:
        verdict = "WORSE"
    else:
        verdict = "UNRESOLVED"
    return {
        "pairs": count, "mean": mean, "low": low, "high": high, "margin": margin,
        "up": sum(1 for d in differences if d > 0),
        "level": sum(1 for d in differences if d == 0),
        "down": sum(1 for d in differences if d < 0),
        "verdict": verdict,
    }


def describe(summary: dict, shortcuts: str) -> str:
    return (
        f"{shortcuts}: {summary['verdict']} — mean pair difference "
        f"{summary['mean']:+.3f}, 95% interval [{summary['low']:+.3f}, "
        f"{summary['high']:+.3f}] over {summary['pairs']} pairs "
        f"({summary['up']} up, {summary['level']} level, {summary['down']} down), "
        f"against a margin of -{summary['margin']:.2f}")


def play(command: Sequence[str]) -> dict:
    done = subprocess.run(command, capture_output=True, text=True, check=False)
    if done.returncode != 0:
        raise SystemExit(f"{' '.join(command)} exited {done.returncode}:\n"
                         f"{done.stderr[-2000:]}")
    return parse_game(done.stdout)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__.splitlines()[0],
        formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--shortcuts", required=True,
                        help="comma-separated `SiteShortcuts` names")
    parser.add_argument("--pairs", type=int, default=150)
    parser.add_argument("--seeds", type=int, default=77_200_000,
                        help="the first seed; each pair takes the next")
    parser.add_argument("--turns", type=int, default=400,
                        help="a ceiling only: the stock world ends itself")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--margin", type=float, default=MARGIN)
    parser.add_argument("--out", type=Path, default=None,
                        help="write every game as a JSON line")
    args = parser.parse_args(argv)
    if not args.binary.is_file():
        raise SystemExit(f"not a binary: {args.binary}")

    print(f"bar, fixed before the first game: the lower end of the 95% interval "
          f"of the mean pair difference is above -{args.margin:.2f}; "
          f"{args.pairs} pairs from seed {args.seeds}")
    # One short game says which player ids are the majors.
    probe = play(game_command(args.binary, args.seeds, 1, args.shortcuts, []))
    first, second = split_majors(list(probe["seconds"]))

    jobs = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        for offset in range(args.pairs):
            seed = args.seeds + offset
            for chairs in (first, second):
                command = game_command(args.binary, seed, args.turns,
                                       args.shortcuts, chairs)
                jobs.append((seed, chairs, pool.submit(play, command)))
        games = [(seed, chairs, job.result()) for seed, chairs, job in jobs]

    by_seed: Dict[int, List[tuple]] = {}
    for seed, chairs, game in games:
        by_seed.setdefault(seed, []).append((chairs, game))
    differences, ratios, decided, treated_wins = [], [], 0, 0
    rows = []
    for seed in sorted(by_seed):
        scores = [treated_score(game, chairs) for chairs, game in by_seed[seed]]
        differences.append(sum(scores) - 1.0)
        for chairs, game in by_seed[seed]:
            ratio = time_ratio(game, chairs)
            if ratio is not None:
                ratios.append(ratio)
            if game["winner"] is not None:
                decided += 1
                treated_wins += int(game["winner"] in chairs)
            rows.append({"seed": seed, "shortcut_seats": list(chairs), **game})
    if args.out:
        args.out.write_text("".join(json.dumps(row) + "\n" for row in rows),
                            encoding="utf-8")

    summary = summarise(differences, args.margin)
    print(describe(summary, args.shortcuts))
    if decided:
        print(f"  the shortcut's seats won {treated_wins} of {decided} decided games "
              f"({100.0 * treated_wins / decided:.1f}%)")
    if ratios:
        print(f"  their agents' seconds per seat were {statistics.median(ratios):.3f} "
              f"of the other seats' (median of {len(ratios)} games; "
              f"{min(ratios):.3f} to {max(ratios):.3f})")
    return 0 if summary["verdict"] == "NOT WORSE" else 1


if __name__ == "__main__":
    raise SystemExit(main())
