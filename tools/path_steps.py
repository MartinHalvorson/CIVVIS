#!/usr/bin/env python3
"""The path through one game: how many steps it is, and how big.

    tools/path_steps.py --binary B                 measure and print
    tools/path_steps.py --binary B --record        and append the reading
    tools/path_steps.py --summarise DIR [DIR ...]  count recorded games again

Operator, 2026-11-24: "we should try to reduce the path of civvis through civ
6 down to the computatoinal irreducability and should have a chart tracking
the size/num of steps."

A game is a function `next(world, action) -> world` applied over and over.
The **path** is the list of actions it was applied to, from the first turn to
the end: the engine's own action log. This tool plays whole games with
`civvis core-probe record` (the stock agents on every seat, six players on
Continents at Online speed, 250 turns at most: the standard self-play game)
and counts the path each one took:

- **steps**: the actions in the log, and how many of each kind;
- **bytes**: the log written down, each action as compact JSON;
- **instructions**: what the processor retired to play the game, agents and
  rules together (macOS `/usr/bin/time -l`; left out where the platform does
  not count them). Unlike seconds it does not move with how busy the host is;
- **replay instructions**: the same path applied to the same opening world
  with no agent in the loop (`civvis core-probe strip` with nothing stripped,
  which replays the window twice, so half of its count). This is what the
  path costs once nobody has to decide it;
- **walks**: a unit's consecutive one-hex `move` steps inside one seat's turn
  are one walk. `merged` is the steps there would be if every walk were one
  step, which is the first reduction the log itself shows.

A reading is the mean over the games played, on fixed seeds, so two builds
are compared on the same worlds. The path is deterministic: the same build
and seed give the same log on any machine, so only the instructions and the
seconds belong to the host.

The readings are `data/performance/path_steps.json`, newest last, drawn on
the CIVVIS Performance page. `docs/PATH_STEPS.md` is the write-up.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import subprocess
import sys
import tempfile
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
LEDGER = ROOT / "data" / "performance" / "path_steps.json"

#: The standard self-play game, as `civvis core-probe record` sets it up.
PLAYERS = 6
TURNS = 250
SETUP = "6 players, Continents, Online speed, 250 turns at most, the stock agent on every seat"

#: The seeds are the games-per-minute benchmark's: worker i plays this plus
#: 100000 * i (`tools/games_per_minute.py`).
SEED_BASE = 20261122000000
SEED_STEP = 100000

RETIRED = re.compile(r"^\s*(\d+)\s+instructions retired\s*$", re.M)


def seeds(games: int, base: int = SEED_BASE) -> list[int]:
    return [base + SEED_STEP * index for index in range(games)]


def counted(command: list[str], cwd: Path) -> tuple[str, int | None]:
    """Run a command and return what it printed with the instructions it
    retired, where the platform counts them (`None` elsewhere)."""
    timer = Path("/usr/bin/time")
    if platform.system() == "Darwin" and timer.exists():
        done = subprocess.run([str(timer), "-l", *command], cwd=cwd, capture_output=True, text=True, check=True)
        found = RETIRED.search(done.stderr)
        return done.stdout, int(found.group(1)) if found else None
    done = subprocess.run(command, cwd=cwd, capture_output=True, text=True, check=True)
    return done.stdout, None


def path_of(actions: list[Any]) -> dict[str, Any]:
    """Count one game's path. `actions` is the action log as the probe writes
    it: `[seat, action]` pairs, an action an object with its `type`."""
    kinds: Counter[str] = Counter()
    size = hops = walks = 0
    walking: tuple[int, Any] | None = None
    for seat, action in actions:
        kind = str(action["type"])
        kinds[kind] += 1
        size += len(json.dumps(action, separators=(",", ":"), ensure_ascii=False).encode("utf-8"))
        if kind == "move":
            hops += 1
            unit = (seat, action.get("unit"))
            if unit != walking:
                walks += 1
            walking = unit
        else:
            walking = None
    steps = sum(kinds.values())
    return {
        "steps": steps,
        "bytes": size,
        "hops": hops,
        "walks": walks,
        "merged": steps - hops + walks,
        "kinds": dict(kinds.most_common()),
    }


def game_of(directory: Path) -> dict[str, Any]:
    """One recorded game, read from the folder `core-probe record` filled."""
    meta = json.loads((directory / "meta.json").read_text(encoding="utf-8"))
    row = path_of(json.loads((directory / "actions_all.json").read_text(encoding="utf-8")))
    row.update(seed=meta["seed"], turns=meta["end_turn"], seats=meta["seats"], won=meta.get("winner") is not None)
    return row


def play(binary: Path, seed: int, directory: Path, log) -> dict[str, Any]:
    """Play one whole game, count its path, and replay it with no agent."""
    directory.mkdir(parents=True, exist_ok=True)
    out, retired = counted([str(binary), "core-probe", "record", str(PLAYERS), str(seed), "1", str(TURNS), str(directory)], directory)
    meta = json.loads(out.strip().splitlines()[-1])
    row = game_of(directory)
    row["seconds"] = round(float(meta["window_s"]), 2)
    if retired:
        row["instructions"] = retired
    nothing = directory / "nothing.txt"
    nothing.write_text("", encoding="utf-8")
    out, retired = counted([str(binary), "core-probe", "strip", str(directory), str(nothing)], directory)
    replay = json.loads(out.strip().splitlines()[-1])
    row["replay_identical"] = bool(
        replay.get("restored") and not replay.get("diverged_count") and not replay.get("refused") and not replay.get("baseline_failed")
    )
    if retired:
        # `strip` replays the window twice, once as its reference.
        row["replay_instructions"] = retired // 2
    log(
        f"seed {seed}: {row['steps']:,} steps over {row['turns']} turns, {row['bytes']:,} bytes"
        + (f", {row['instructions'] / 1e9:.1f} billion instructions" if row.get("instructions") else "")
        + ("" if row["replay_identical"] else "  ** the replay did NOT come out the same **")
    )
    return row


def summarise(games: list[dict[str, Any]]) -> dict[str, Any]:
    """The reading: the mean of each count over the games."""
    if not games:
        raise ValueError("no games to summarise")
    n = len(games)

    def mean(key: str) -> float | None:
        values = [game[key] for game in games if game.get(key) is not None]
        return sum(values) / len(values) if len(values) == n else None

    kinds: Counter[str] = Counter()
    for game in games:
        kinds.update(game["kinds"])
    reading: dict[str, Any] = {
        "games": n,
        "seeds": [game["seed"] for game in games],
        "turns": round(mean("turns"), 1),
        "steps": round(mean("steps")),
        "bytes": round(mean("bytes")),
        "hops": round(mean("hops")),
        "walks": round(mean("walks")),
        "kinds": {kind: round(count / n) for kind, count in kinds.most_common()},
    }
    # From the rounded means, so the sum a reader does comes out to the figure.
    reading["merged"] = reading["steps"] - reading["hops"] + reading["walks"]
    for key in ("instructions", "replay_instructions"):
        value = mean(key)
        if value is not None:
            reading[key] = round(value)
    seconds = mean("seconds")
    if seconds is not None:
        reading["seconds"] = round(seconds, 1)
    if all("replay_identical" in game for game in games):
        reading["replay_identical"] = all(game["replay_identical"] for game in games)
    return reading


def source_tree(binary: Path) -> Path | None:
    """`<tree>/target/<profile>/civvis` was built from `<tree>`."""
    for parent in binary.parents:
        if parent.name == "target" and (parent.parent / "Cargo.toml").exists():
            return parent.parent
    return None


def commit_of(binary: Path) -> str | None:
    tree = source_tree(binary)
    if tree is None:
        return None
    done = subprocess.run(["git", "-C", str(tree), "rev-parse", "HEAD"], capture_output=True, text=True)
    return done.stdout.strip() or None if done.returncode == 0 else None


def record(reading: dict[str, Any], ledger: Path = LEDGER) -> None:
    book = json.loads(ledger.read_text(encoding="utf-8")) if ledger.exists() else {
        "about": (
            "The path through one game: the actions of one standard self-play game from its first turn to its end, "
            "counted by `tools/path_steps.py --record`. The write-up is `docs/PATH_STEPS.md`. Newest reading last."
        ),
        "readings": [],
    }
    book["readings"].append(reading)
    ledger.write_text(json.dumps(book, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")


def report(reading: dict[str, Any]) -> str:
    lines = [
        f"the path through one game: {reading['steps']:,} steps over {reading['turns']:g} turns"
        f"  (mean of {reading['games']} games)",
        f"  written down: {reading['bytes']:,} bytes",
        f"  walks: {reading['hops']:,} one-hex moves in {reading['walks']:,} walks;"
        f" with a walk as one step the path is {reading['merged']:,} steps",
    ]
    if reading.get("instructions"):
        lines.append(f"  instructions: {reading['instructions'] / 1e9:,.1f} billion to play it")
    if reading.get("replay_instructions"):
        lines.append(
            f"  the same path with no agent deciding: {reading['replay_instructions'] / 1e9:,.1f} billion"
            + ("" if reading.get("replay_identical") else "  ** NOT the same world **")
        )
    top = list(reading["kinds"].items())[:6]
    lines.append("  most steps: " + ", ".join(f"{kind} {count:,}" for kind, count in top))
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="The path through one game: its steps and its size.")
    parser.add_argument("--binary", help="a release `civvis`")
    parser.add_argument("--games", type=int, default=3, help="games to play (default 3)")
    parser.add_argument("--seed-base", type=int, default=SEED_BASE)
    parser.add_argument("--commit", help="the commit the binary was built from (default: its source tree's HEAD)")
    parser.add_argument("--out", help="where the games are kept (default: a temporary folder)")
    parser.add_argument("--record", action="store_true", help="append the reading to path_steps.json")
    parser.add_argument("--json", action="store_true", help="print the reading as JSON")
    parser.add_argument("--summarise", nargs="+", metavar="DIR", help="count games already recorded in these folders")
    args = parser.parse_args(argv)

    def log(text: str) -> None:
        print(text, file=sys.stderr, flush=True)

    if args.summarise:
        reading = summarise([game_of(Path(directory)) for directory in args.summarise])
    else:
        if not args.binary:
            parser.error("--binary is needed to play games")
        binary = Path(args.binary).resolve()
        if not os.access(binary, os.X_OK):
            parser.error(f"{binary} is not an executable")
        commit = args.commit or commit_of(binary)
        if args.record and not commit:
            parser.error("--record needs the build's commit: pass --commit")
        if args.record and args.seed_base != SEED_BASE:
            parser.error("a reading on other seeds cannot be recorded")
        out = Path(args.out) if args.out else Path(tempfile.mkdtemp(prefix="path-steps-"))
        games = [play(binary, seed, out / str(seed), log) for seed in seeds(args.games, args.seed_base)]
        reading = {"taken": datetime.now(timezone.utc).strftime("%Y-%m-%d"), "build": commit, "setup": SETUP, **summarise(games)}
        if args.record:
            if reading.get("replay_identical") is False:
                log("not recorded: a replay of the path did not come out as the game did")
                return 1
            record(reading)
            log(f"recorded in {LEDGER.relative_to(ROOT)}")
    print(json.dumps(reading, ensure_ascii=False, indent=1) if args.json else report(reading))
    return 0


if __name__ == "__main__":
    sys.exit(main())
