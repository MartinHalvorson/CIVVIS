#!/usr/bin/env python3
"""Which of the shipped live-bridge treatments can change what the seat does.

`AdvancedAi::enable_live_bridge` turns on 74 treatments and `docs/EVAL_STATUS.md`
reports that 34 of the withholdable ones have never been named in any evaluation
round. The obvious way to price one is to withhold it and play -- but a live
Civilization VI attempt is 40 to 70 minutes of wall clock and one machine's
GPU, so pricing 74 arms that way is months of seat time, and the arms that turn
out to change nothing would consume the same months as the arms that matter.

`gene_census` already answers this shape of question for the 40 genes, and its
lesson transfers: *does this control change anything at all* is far cheaper to
establish than a win rate, and it is a precondition for the win rate meaning
anything. A gene that left 398 of 400 paired maps outcome-identical could not be
selected on. A live treatment whose absence changes no order on a real board
cannot be worth a live game either.

The difference is the regime, and the regime is the point. `docs/AI_GAPS.md`
2026-08-10 measured the whole live bundle at **-108 Elo confirmed** against
`advanced` in native play, and concluded that a repair validated in one engine
does not transfer to the other. So a native `ai_eval` census would answer a
question nobody asked. This replays **recorded Civilization VI games** through
the same decider the seat runs, with one treatment withheld, and counts the
turns whose orders move.

    python3 tools/civ6_treatment_census.py ~/civvis-civ6-runs/control/civvis-...Z

What it reports, per treatment: how many replayed turns produced a different
order set than the control, which turn moved first, and how many orders differed.
A treatment reported INERT is inert **on this run's board**, which is a claim
about one recorded game and not about the rule the treatment names -- run it on
two or three unlike runs before believing any single verdict, exactly as
`gene_census` demands re-probing anything it calls inert.

⚠ THE CONTROL IS RUN TWICE AND THE TWO MUST AGREE. A census whose control cannot
reproduce itself measures its own noise: every "difference" would be
indistinguishable from replay jitter, and the tool would confidently report a
treatment as live because a process started at a different microsecond. If the
two control passes disagree the run is refused rather than reported, and the
first disagreeing turn is printed so the non-determinism can be chased.

⚠ It replays through the PERSISTENT decider (`--serve`), one turn number per
line, because the per-turn mode is a different agent: a fresh process holds no
plan, no book position and an empty `ours`, so it takes the cheap route and
reproduces neither the live plan nor each other. The persistent process is what
the seat actually runs.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Iterable, Sequence

REPO = Path(__file__).resolve().parent.parent
DEFAULT_BIN = REPO / "target" / "release" / "civvis_orders"


class CensusError(RuntimeError):
    """A refusal that names its cause, rather than a partial table."""


def discover_treatments(binary: Path, mirror: Path) -> list[str]:
    """Ask the binary which treatments it can withhold.

    Deliberately not a list in this file. A hand-written one is complete the day
    it is written and silently shrinks afterwards -- the repository has paid for
    that three times (`AGENTS.md`, "Discover, never list"), most recently when
    `civvis_orders` carried 57 hand-written `--without` arms against 68 live
    treatments and eleven shipped with no control at all.

    `--without <unknown>` is a hard error whose message enumerates the real
    table, so the binary under test is the source of truth.
    """
    probe = subprocess.run(
        [str(binary), "--mirror", str(mirror), "--without", "__census_probe__"],
        capture_output=True,
        text=True,
    )
    match = re.search(r"this binary can withhold:\s*(.+)", probe.stderr, re.S)
    if not match:
        raise CensusError(
            "the decider did not enumerate its treatments; expected the "
            f"`--without` error to list them. stderr was:\n{probe.stderr.strip()[:800]}"
        )
    names = [name.strip() for name in match.group(1).split(",")]
    return [name for name in names if name]


def played_as(mirror: Path) -> dict[str, str]:
    """The seat identity this run was actually played with.

    ★★★ A CENSUS THAT REPLAYS A DIFFERENT AGENT THAN THE ONE THAT PLAYED
    MEASURES A DIFFERENT AGENT. The first version of this tool defaulted to
    `--victory civvis --strategy auto --civ Rome` whatever the run held, and the
    two censuses recorded in #2018 were run that way against games played on
    `strategy=WildCard9` with `victory_target=diplomatic`. Control and arm still
    shared those flags, so the comparison was internally sound -- but a
    treatment that only fires under the plan the run was played on would read
    inert, and the report claimed to replay "the decider the seat actually
    runs".

    The run records both. `brain.log` prints the decider's own line
    (`strategy=… civ=…`) and `summary.json` carries `victory_target`. Read them,
    say which were found, and let a flag override. `--civ` takes the host's
    `CIVILIZATION_ROME` unchanged: `civvis_orders` maps it through CIVVIS's own
    roster rather than by string surgery.
    """
    found: dict[str, str] = {}
    brain = mirror / "brain.log"
    if brain.exists():
        text = brain.read_text(errors="ignore")
        for key, pattern in (("strategy", r"strategy=(\S+)"), ("civ", r"civ=(\S+?)[,\s]")):
            match = re.search(pattern, text)
            if match:
                found[key] = match.group(1).rstrip(",")
    summary = mirror / "summary.json"
    if summary.exists():
        try:
            row = json.loads(summary.read_text())
        except json.JSONDecodeError:
            row = {}
        if isinstance(row.get("victory_target"), str):
            found["victory"] = row["victory_target"]
    return found


def replay(
    binary: Path,
    mirror: Path,
    turns: Sequence[int],
    *,
    without: str | None,
    civ: str,
    victory: str,
    strategy: str,
    timeout: float,
) -> dict[int, str]:
    """One persistent replay; returns {turn: canonical order set}.

    The agent lives for the whole pass, which is what gives it a plan that spans
    turns. One turn number per line in, one line of orders JSON out.
    """
    argv = [
        str(binary),
        "--mirror",
        str(mirror),
        "--serve",
        "--fresh-board",
        "--victory",
        victory,
        "--strategy",
        strategy,
        "--civ",
        civ,
    ]
    if without:
        argv += ["--without", without]
    stdin = "".join(f"{turn}\n" for turn in turns)
    done = subprocess.run(
        argv, input=stdin, capture_output=True, text=True, timeout=timeout
    )
    if done.returncode != 0:
        raise CensusError(
            f"decider exited {done.returncode} for "
            f"{'--without ' + without if without else 'the control'}: "
            f"{done.stderr.strip()[-400:]}"
        )
    out: dict[int, str] = {}
    lines = [line for line in done.stdout.splitlines() if line.strip()]
    for turn, line in zip(turns, lines):
        out[turn] = canonical(line)
    return out


def canonical(line: str) -> str:
    """The orders of one turn, in a form two runs can be compared by.

    Sorted, because the order channel's own sequence is not part of what a
    treatment decides, and a reordering would otherwise read as a decision
    change. Everything else in the row is kept: a treatment that moves one unit
    one hex differently has changed what the seat does.
    """
    try:
        reply = json.loads(line)
    except json.JSONDecodeError:
        return f"UNPARSEABLE:{line[:200]}"
    orders = reply.get("orders") or []
    rows = sorted(json.dumps(order, sort_keys=True) for order in orders)
    return "\n".join(rows)


def turn_window(mirror: Path, spec: str | None, cap: int) -> list[int]:
    """The turns this run actually recorded, narrowed by `--turns LO:HI`."""
    events = mirror / "events.jsonl"
    if not events.exists():
        raise CensusError(f"{events} does not exist; --mirror wants a run directory")
    seen: set[int] = set()
    with events.open(errors="ignore") as handle:
        for line in handle:
            if '"state"' not in line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            if row.get("kind") == "state" and isinstance(row.get("turn"), int):
                seen.add(row["turn"])
    turns = sorted(seen)
    if spec:
        lo_text, _, hi_text = spec.partition(":")
        lo = int(lo_text) if lo_text else turns[0] if turns else 0
        hi = int(hi_text) if hi_text else turns[-1] if turns else 0
        turns = [turn for turn in turns if lo <= turn <= hi]
    if not turns:
        raise CensusError("no recorded state turns in that window")
    if cap and len(turns) > cap:
        # Evenly spaced rather than the first N: a treatment that only acts in
        # the endgame would be invisible in a prefix, and the war and settlement
        # arms act at opposite ends of a game.
        step = len(turns) / cap
        turns = [turns[int(index * step)] for index in range(cap)]
    return turns


def compare(control: dict[int, str], arm: dict[int, str]) -> dict:
    """How far apart two passes are, per turn."""
    shared = sorted(set(control) & set(arm))
    moved = [turn for turn in shared if control[turn] != arm[turn]]
    return {
        "turns_compared": len(shared),
        "turns_moved": len(moved),
        "first_moved": moved[0] if moved else None,
        "share": (len(moved) / len(shared)) if shared else 0.0,
        "missing": sorted(set(control) - set(arm)),
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("mirror", help="a recorded run directory (holds events.jsonl)")
    parser.add_argument("--turns", help="LO:HI window of recorded turns")
    parser.add_argument(
        "--max-turns",
        type=int,
        default=40,
        help="cap the replayed turns, spread evenly over the window (default 40)",
    )
    parser.add_argument(
        "--treatments",
        help="comma-separated subset; default is every treatment the binary lists",
    )
    parser.add_argument("--bin", default=str(DEFAULT_BIN))
    # Default `None`, not a value: the run's own record is the default, and a
    # flag exists to override it rather than to restate it.
    parser.add_argument("--civ", help="override the civilization the run recorded")
    parser.add_argument("--victory", help="override the victory target the run recorded")
    parser.add_argument("--strategy", help="override the strategy the run recorded")
    parser.add_argument("--jobs", type=int, default=max(1, (os.cpu_count() or 4) // 2))
    parser.add_argument("--timeout", type=float, default=1800.0)
    parser.add_argument("--json", help="write the full result to this path")
    parser.add_argument(
        "--skip-control-check",
        action="store_true",
        help="do not run the control twice (you are then reporting unvalidated noise)",
    )
    args = parser.parse_args(argv)

    binary = Path(args.bin)
    if not binary.exists():
        raise CensusError(
            f"{binary} does not exist; build it with "
            f"`cargo build --release --locked --bin civvis_orders`"
        )
    mirror = Path(args.mirror).expanduser()
    turns = turn_window(mirror, args.turns, args.max_turns)
    # The run's own identity wins over the defaults; an explicit flag wins over
    # both. What was found and what was assumed is printed, because a census run
    # against the wrong seat identity is not distinguishable from a right one in
    # the table it prints.
    played = played_as(mirror)
    seat = {
        "victory": args.victory or played.get("victory") or "civvis",
        "strategy": args.strategy or played.get("strategy") or "auto",
        "civ": args.civ or played.get("civ") or "Rome",
    }
    assumed = [key for key in seat if key not in played and not getattr(args, key)]
    print(
        "seat: " + " ".join(f"{k}={v}" for k, v in sorted(seat.items()))
        + (f"  (not recorded in the run, assumed: {', '.join(sorted(assumed))})" if assumed else "")
        + "  [read from the run]",
        file=sys.stderr,
    )
    print(
        f"census over {len(turns)} recorded turns "
        f"({turns[0]}..{turns[-1]}) of {mirror.name}",
        file=sys.stderr,
    )

    def run(without: str | None) -> dict[int, str]:
        return replay(
            binary,
            mirror,
            turns,
            without=without,
            civ=seat["civ"],
            victory=seat["victory"],
            strategy=seat["strategy"],
            timeout=args.timeout,
        )

    control = run(None)
    if not args.skip_control_check:
        second = run(None)
        drift = [turn for turn in turns if control.get(turn) != second.get(turn)]
        if drift:
            raise CensusError(
                "the control does not reproduce itself: two identical passes "
                f"disagree on {len(drift)} of {len(turns)} turns, first at turn "
                f"{drift[0]}. Every difference this tool would report is then "
                "indistinguishable from that jitter, so nothing is reported."
            )
        print("control reproduced itself exactly", file=sys.stderr)

    names = (
        [name.strip() for name in args.treatments.split(",") if name.strip()]
        if args.treatments
        else discover_treatments(binary, mirror)
    )
    print(f"{len(names)} treatments to withhold", file=sys.stderr)

    results: dict[str, dict] = {}

    def one(name: str) -> tuple[str, dict]:
        try:
            return name, compare(control, run(name))
        except (CensusError, subprocess.TimeoutExpired) as exc:
            return name, {"error": str(exc)[:300]}

    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for name, result in pool.map(one, names):
            results[name] = result
            print(".", end="", flush=True, file=sys.stderr)
    print("", file=sys.stderr)

    ranked = sorted(
        results.items(),
        key=lambda pair: (-(pair[1].get("share") or 0.0), pair[0]),
    )
    inert = [name for name, row in ranked if not row.get("error") and row["turns_moved"] == 0]
    print(f"\n{'treatment':38} {'turns moved':>12} {'share':>7}  first")
    for name, row in ranked:
        if row.get("error"):
            print(f"{name:38} {'ERROR':>12}          {row['error'][:60]}")
            continue
        print(
            f"{name:38} {row['turns_moved']:>5}/{row['turns_compared']:<6} "
            f"{row['share']:>6.1%}  {row['first_moved'] if row['first_moved'] else '-'}"
        )
    print(
        f"\n{len(inert)} of {len(names)} treatments changed no order on this board.\n"
        "That is a screen on ONE recorded game, not a verdict on the treatment: "
        "re-run on an unlike run before withholding one in a live attempt."
    )

    if args.json:
        Path(args.json).write_text(
            json.dumps(
                {
                    "mirror": str(mirror),
                    "seat": seat,
                    "turns": turns,
                    "results": results,
                },
                indent=2,
            )
            + "\n"
        )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except CensusError as exc:
        print(f"civ6_treatment_census: {exc}", file=sys.stderr)
        raise SystemExit(2)
