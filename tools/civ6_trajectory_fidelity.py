#!/usr/bin/env python3
"""Does the simulator play the game the live seat plays? A per-subsystem ledger.

★★★★ THE TWO INSTRUMENTS HAVE NEVER BEEN COMPARED LIKE WITH LIKE.

`civ6_fidelity.py` answers a different and already-solved question: whether
`data/*.json` carries the same RULES CONSTANTS as the shipped game database.
It says nothing about whether a game played under those constants goes the same
way. Nothing here did.

The repository already knows the two regimes diverge, and by how much on one
axis: `docs/` records a headless soak fielding **30-66 units** by era 8 against a
live ladder that ENDED with **one warrior** at turn 229 — about sixty to one. It
also records why that number is so easy to misread: a 6-player 24x16 `ai_eval`
produces empires SMALLER than live (1.74 cities against 3.0) while a 4-player
soak produces ones much LARGER, so "headless" is not one regime and the
configuration chooses which one is measured. A change measured null on one is
close to uninformative about the other.

This tool makes that a standing, per-subsystem number instead of an anecdote.

WHAT IT REFUSES TO DO, WHICH IS THE POINT
=========================================
It will not pool two corpora that played different games. Both sides are keyed
by CONFIGURATION -- difficulty, speed and map size -- and a subsystem is only
ever reported inside a cell both corpora actually populate. Every unmatched cell
is NAMED, with its own count, rather than dropped: a corpus that cannot be
compared at all is the finding, not an empty table.

That is not a hypothetical caution. On the corpus this was written against,
248 of 320 deep live runs are Settler on a Small map and every recorded screen
seat is Prince -- so the honest answer to "does the simulator reproduce the live
game" is that nobody has ever been in a position to ask.

THE RUNG IS NOT THE WHOLE CONFIGURATION: WHO CARRIES THE HANDICAP IS TOO
======================================================================
A rung above Prince hands the AI a yield, experience and free-unit bonus, and
the rungs below Prince hand the HUMAN an experience bonus. `gene_screen` decides
who receives it, and its DEFAULT (`--handicap all`, i.e. the flag absent) gives
it to every seat -- our measured seats included. That is symmetric, so it
cancels, and it is not the deployment: a live seat meets an Emperor field
without an Emperor bonus of its own.

The first version of this ledger did not read that axis, and its Emperor row
said so without knowing why -- live standing 0.34 against a simulated 0.66, a
1.93x divergence, which reads as an engine defect. It is not one. The tell was
already in the table: the simulated standing is **0.66 at Prince and 0.66 at
Emperor**, identical to two decimals, because a handicap given to everyone
changes nothing. Every Emperor seat ever screened played a symmetric field.

So the handicap mode is part of the configuration key. The live seat is always
`rivals` -- the rung's bonus goes to the rivals and not to us. A screen is
whatever its header records, defaulting to `all`. At a rung that confers nothing
either way the distinction is meaningless, so both sides collapse to `n/a` and
the cell still matches; `data/difficulties.json` says which rungs those are and
it is read rather than restated.

Usage::

    python3 tools/civ6_trajectory_fidelity.py                  # markdown ledger
    python3 tools/civ6_trajectory_fidelity.py --json out.json   # machine-readable
    python3 tools/civ6_trajectory_fidelity.py --check --max 0   # the ratchet

``--live`` and ``--sim`` override the two corpora. ``--check`` compares each
matched subsystem against the tolerance recorded in
``docs/trajectory_fidelity.json`` and exits 1 when one is outside it; a
tolerance may only ever TIGHTEN, written by ``--write``, so fidelity cannot
silently regress. On a machine with no live corpus -- every hosted runner --
``--check`` prints a named skip and passes, the same shape `civ6_fidelity.py`
uses, so the ratchet is only ever a real number where the recorded runs live.
"""

from __future__ import annotations

import argparse
import json
import re
import statistics
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
#: The committed live corpus. Deliberately the one inside the repository
#: rather than the machine-local `~/civvis-civ6-runs/civvis_ladder.jsonl`: a
#: ledger whose evidence only exists on one disk cannot be a ratchet, and this
#: one travels to every clone and every runner. `--live` takes either.
LIVE_DEFAULT = REPO / "docs" / "civ6_ladder.json"
SIM_DEFAULT = REPO / "docs" / "gene_screens"
TOLERANCES = REPO / "docs" / "trajectory_fidelity.json"

#: A live run shallower than this never left the opening and says nothing about
#: a trajectory. The live corpus's own reports use the same 100-turn floor.
MIN_TURNS = 100


class Subsystem:
    """One quantity both corpora can report, and the name each calls it.

    ⚠ The pair is the whole definition. A subsystem whose field is missing from
    either side is reported as UNAVAILABLE by name; it is never silently
    dropped, because a field that quietly disappears from one schema is exactly
    how a ledger starts measuring nothing.
    """

    def __init__(self, name: str, live: str, sim: str, about: str) -> None:
        self.name = name
        self.live = live
        self.sim = sim
        self.about = about


SUBSYSTEMS = [
    Subsystem(
        "standing", "_score_ratio", "_score_ratio", "own score over the best rival's"
    ),
    Subsystem(
        "research_pace", "techs_at_150", "techs_150", "own techs at Standard turn 150"
    ),
    Subsystem(
        "rival_research_pace",
        "rival_techs_at_150",
        "_rival_techs_150",
        "the best rival's techs at Standard turn 150",
    ),
    Subsystem(
        "boost_coverage",
        "_boost_share",
        "_boost_share",
        "share of researched techs that arrived boosted",
    ),
]


def map_sizes() -> dict[tuple[int, int], str]:
    """The shipped map-size table, read out of `src/setup.rs`.

    Discovered rather than listed: a size added or resized there reaches this
    tool without anyone remembering to come here, and a table that stops
    parsing fails loudly below instead of quietly matching nothing.
    """
    source = (REPO / "src" / "setup.rs").read_text(encoding="utf-8")
    sizes: dict[tuple[int, int], str] = {}
    for block in re.finditer(
        r'id:\s*"(\w+)"\s*,\s*name:\s*"[^"]*"\s*,\s*width:\s*(\d+)\s*,\s*height:\s*(\d+)',
        source,
    ):
        name, width, height = block.group(1), int(block.group(2)), int(block.group(3))
        sizes.setdefault((width, height), name)
    if not sizes:
        raise SystemExit(
            "src/setup.rs parsed to no map sizes; the table moved and this "
            "tool would silently match nothing"
        )
    return sizes


#: What the live seat always is: the rung's bonus goes to the rivals, never to
#: us. `civ6_play.py` sets the difficulty on the host and CIVVIS plays the
#: local player, so there is no configuration in which a live seat carries it.
LIVE_HANDICAP = "rivals"

#: What a screen is when its header records nothing, which is `gene_screen`'s
#: own default for the flag: every seat receives the rung's bonus.
SIM_HANDICAP_DEFAULT = "all"

#: A rung that confers nothing on either side; the handicap mode cannot matter.
NEUTRAL_HANDICAP = "n/a"


def neutral_rungs() -> set[str]:
    """Difficulties that hand nothing to either side, from the shipped table.

    Read out of `data/difficulties.json` rather than restated, so a rung whose
    bonuses change reaches this tool on its own. A rung is neutral when it
    carries no AI bonus of any kind AND no human bonus: on the stock ladder
    that is Prince alone, since Settler, Chieftain and Warlord all hand the
    HUMAN an experience bonus and every rung above Prince hands the AI yields,
    experience and free units.
    """
    body = json.loads((REPO / "data" / "difficulties.json").read_text(encoding="utf-8"))
    rows = body.items() if isinstance(body, dict) else [(r.get("name"), r) for r in body]
    neutral = set()
    for name, spec in rows:
        if not isinstance(spec, dict) or not name:
            continue
        tilts = any(
            spec.get(key)
            for key in ("ai_yield_pct", "ai_bonus_units", "ai_xp_pct", "human_xp_pct")
        )
        if not tilts:
            neutral.add(str(name).lower())
    if not neutral:
        raise SystemExit(
            "data/difficulties.json parsed to no neutral rung; the table moved "
            "and every cell would be split on a distinction that cannot matter"
        )
    return neutral


def handicap_for(difficulty: str | None, mode: str, neutral: set[str]) -> str:
    """`mode`, unless the rung confers nothing and the mode cannot matter."""
    if difficulty and difficulty.lower() in neutral:
        return NEUTRAL_HANDICAP
    return mode


def strip_prefix(value: str | None, prefix: str) -> str | None:
    """`DIFFICULTY_KING` -> `king`, and `None` stays `None`."""
    if not isinstance(value, str):
        return None
    stripped = value[len(prefix) :] if value.startswith(prefix) else value
    return stripped.lower() or None


def live_attempts(path: Path) -> list[dict]:
    """The raw attempt rows, from either recorded shape.

    `docs/civ6_ladder.json` is an object with an `attempts` list; the
    machine-local ladder is JSON Lines. Both are accepted, so a fleet Mac can
    point `--live` at its own deeper corpus without a second tool.
    """
    if not path.exists():
        return []
    text = path.read_text(encoding="utf-8")
    try:
        body = json.loads(text)
    except json.JSONDecodeError:
        rows = []
        for line in text.splitlines():
            line = line.strip()
            if line:
                try:
                    rows.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
        return rows
    if isinstance(body, dict):
        return list(body.get("attempts") or [])
    return list(body) if isinstance(body, list) else []


def live_records(path: Path) -> list[dict]:
    """Deep live runs, projected onto the common schema."""
    neutral = neutral_rungs()
    out = []
    for row in live_attempts(path):
        turns = row.get("turns")
        if turns is None:
            turns = row.get("last_turn")
        if (turns or 0) < MIN_TURNS:
            continue
        difficulty = strip_prefix(row.get("difficulty"), "DIFFICULTY_")
        cell = (
            difficulty,
            strip_prefix(row.get("speed"), "GAMESPEED_"),
            strip_prefix(row.get("map_size"), "MAPSIZE_"),
            handicap_for(difficulty, LIVE_HANDICAP, neutral),
        )
        if None in cell:
            continue
        record = {"_cell": cell}
        for key in ("techs_at_150", "rival_techs_at_150"):
            value = row.get(key)
            if isinstance(value, (int, float)):
                record[key] = float(value)
        own = row.get("score")
        if own is None:
            own = row.get("last_score")
        best = row.get("rival_best")
        if isinstance(own, (int, float)) and isinstance(best, (int, float)) and best > 0:
            record["_score_ratio"] = float(own) / float(best)
        boosts = row.get("boosts")
        if isinstance(boosts, dict):
            share = boosts.get("techs_boosted_share")
            if isinstance(share, (int, float)):
                record["_boost_share"] = float(share)
        out.append(record)
    return out


def sim_records(path: Path, sizes: dict[tuple[int, int], str]) -> list[dict]:
    """Screen seats, projected onto the same schema.

    A screen file's header carries the configuration; each `game` row is one
    seat. The best rival's score is the best OTHER seat in the same game, which
    is what the live row's `rival_best` means.
    """
    files = sorted(path.glob("*.jsonl")) if path.is_dir() else [path]
    neutral = neutral_rungs()
    out: list[dict] = []
    for file in files:
        header: dict = {}
        seats: dict[int, list[dict]] = {}
        for line in file.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            if row.get("kind") == "game":
                seats.setdefault(row.get("game"), []).append(row)
            elif "width" in row and "height" in row:
                header = row
        if not header:
            continue
        difficulty = (header.get("difficulty") or "").lower() or None
        # `gene_screen` records the handicap inside its rival-mix block, and
        # skips that block entirely when no `--rivals` was asked for — so an
        # absent value is the flag's own default, every seat handicapped.
        mode = str(header.get("handicap") or SIM_HANDICAP_DEFAULT).lower()
        cell = (
            difficulty,
            (header.get("speed") or "").lower() or None,
            sizes.get((header.get("width"), header.get("height"))),
            handicap_for(difficulty, mode, neutral),
        )
        if None in cell:
            continue
        for game in seats.values():
            scores = [
                float(seat["score"])
                for seat in game
                if isinstance(seat.get("score"), (int, float))
            ]
            techs = [
                float(seat["techs_150"])
                for seat in game
                if isinstance(seat.get("techs_150"), (int, float))
            ]
            for seat in game:
                record = {"_cell": cell}
                value = seat.get("techs_150")
                if isinstance(value, (int, float)):
                    record["techs_150"] = float(value)
                    # The best OTHER seat in this game, which is what the live
                    # row's `rival_*` fields mean.
                    others = [t for t in techs if t is not value]
                    if others:
                        record["_rival_techs_150"] = max(others)
                own = seat.get("score")
                others = [s for s in scores if s != own] or scores
                if isinstance(own, (int, float)) and others and max(others) > 0:
                    record["_score_ratio"] = float(own) / max(others)
                researched = seat.get("techs_researched")
                boosted = seat.get("techs_boosted")
                if (
                    isinstance(researched, (int, float))
                    and isinstance(boosted, (int, float))
                    and researched > 0
                ):
                    record["_boost_share"] = float(boosted) / float(researched)
                out.append(record)
    return out


def median_of(records: list[dict], field: str) -> float | None:
    values = [r[field] for r in records if field in r]
    return statistics.median(values) if values else None


def cells(records: list[dict]) -> dict[tuple, list[dict]]:
    out: dict[tuple, list[dict]] = {}
    for record in records:
        out.setdefault(record["_cell"], []).append(record)
    return out


def divergence(live: float, sim: float) -> float:
    """How far apart two medians are, as a ratio that does not care which is
    larger: 1.0 is identical, 2.0 is one twice the other, 60.0 is the army gap.
    """
    if live <= 0 and sim <= 0:
        return 1.0
    if live <= 0 or sim <= 0:
        return float("inf")
    return max(live / sim, sim / live)


def ledger(live: list[dict], sim: list[dict]) -> dict:
    """The whole comparison: matched cells, and every cell that is not."""
    live_cells, sim_cells = cells(live), cells(sim)
    shared = sorted(set(live_cells) & set(sim_cells))
    report = {
        "live_runs": len(live),
        "sim_seats": len(sim),
        "matched_cells": [],
        "live_only": [
            {"cell": list(cell), "runs": len(rows)}
            for cell, rows in sorted(live_cells.items())
            if cell not in sim_cells
        ],
        "sim_only": [
            {"cell": list(cell), "seats": len(rows)}
            for cell, rows in sorted(sim_cells.items())
            if cell not in live_cells
        ],
    }
    for cell in shared:
        entry = {
            "cell": list(cell),
            "live_runs": len(live_cells[cell]),
            "sim_seats": len(sim_cells[cell]),
            "subsystems": {},
        }
        for subsystem in SUBSYSTEMS:
            live_median = median_of(live_cells[cell], subsystem.live)
            sim_median = median_of(sim_cells[cell], subsystem.sim)
            if live_median is None or sim_median is None:
                entry["subsystems"][subsystem.name] = {
                    "available": False,
                    "why": "live" if live_median is None else "sim",
                }
                continue
            entry["subsystems"][subsystem.name] = {
                "available": True,
                "live": round(live_median, 4),
                "sim": round(sim_median, 4),
                "divergence": round(divergence(live_median, sim_median), 4),
            }
        report["matched_cells"].append(entry)
    return report


def worst_divergences(report: dict) -> dict[str, float]:
    """The largest divergence each subsystem shows across every matched cell."""
    worst: dict[str, float] = {}
    for cell in report["matched_cells"]:
        for name, body in cell["subsystems"].items():
            if body.get("available"):
                worst[name] = max(worst.get(name, 0.0), body["divergence"])
    return worst


def render(report: dict) -> str:
    lines = [
        "# Trajectory fidelity: the simulator against the live seat",
        "",
        f"{report['live_runs']} live runs of at least {MIN_TURNS} turns, "
        f"{report['sim_seats']} screen seats.",
        "",
    ]
    if not report["matched_cells"]:
        lines += [
            "## Nothing is comparable",
            "",
            "**No configuration is populated by both corpora**, so no subsystem "
            "can be compared at all. Every number below is a corpus talking to "
            "itself.",
            "",
        ]
    for cell in report["matched_cells"]:
        difficulty, speed, size, handicap = cell["cell"]
        lines += [
            f"## {difficulty} / {speed} / {size} / handicap {handicap}",
            "",
            f"{cell['live_runs']} live runs against {cell['sim_seats']} screen seats.",
            "",
            "| subsystem | live | simulator | divergence |",
            "|---|---:|---:|---:|",
        ]
        for subsystem in SUBSYSTEMS:
            body = cell["subsystems"][subsystem.name]
            if not body.get("available"):
                lines.append(
                    f"| {subsystem.name} | — | — | unavailable "
                    f"({body['why']} side has no value) |"
                )
                continue
            lines.append(
                f"| {subsystem.name} | {body['live']:.2f} | {body['sim']:.2f} "
                f"| {body['divergence']:.2f}× |"
            )
        lines.append("")
    for title, key, unit in (
        ("Live configurations the simulator has never run", "live_only", "runs"),
        ("Simulator configurations the live seat has never played", "sim_only", "seats"),
    ):
        if not report[key]:
            continue
        lines += [f"## {title}", ""]
        for entry in report[key]:
            difficulty, speed, size, handicap = entry["cell"]
            lines.append(
                f"- {difficulty} / {speed} / {size} / handicap {handicap}"
                f" — {entry[unit]} {unit}"
            )
        lines.append("")
    return "\n".join(lines)


#: How far past a recorded divergence the check tolerates before failing.
#:
#: The recorded number is a point estimate over a few dozen live runs, so it
#: carries real sampling error, and a ratchet that fires on a 2% wobble is an
#: alarm nobody keeps. This is a REGRESSION alarm: the recorded value stays the
#: honest observation, and the bar to clear is that value plus this margin.
FIDELITY_SLACK = 0.25


def load_tolerances() -> dict[str, float]:
    if not TOLERANCES.exists():
        return {}
    body = json.loads(TOLERANCES.read_text(encoding="utf-8"))
    return {k: float(v) for k, v in body.get("tolerances", {}).items()}


def check(report: dict, tolerances: dict[str, float], most: int) -> tuple[int, list[str]]:
    """Subsystems outside their recorded tolerance, and why."""
    notes = []
    over = 0
    for name, worst in sorted(worst_divergences(report).items()):
        allowed = tolerances.get(name)
        if allowed is None:
            notes.append(f"{name}: {worst:.2f}× — no tolerance recorded yet")
            continue
        bar = allowed * (1.0 + FIDELITY_SLACK)
        if worst > bar:
            over += 1
            notes.append(
                f"{name}: {worst:.2f}× exceeds the recorded {allowed:.2f}× "
                f"by more than the {FIDELITY_SLACK:.0%} regression margin"
            )
        else:
            notes.append(f"{name}: {worst:.2f}× within {allowed:.2f}× +{FIDELITY_SLACK:.0%}")
    return (1 if over > most else 0), notes


def write_tolerances(report: dict) -> dict[str, float]:
    """Record each subsystem's current worst divergence, tightening only.

    A tolerance never loosens: the ratchet's whole job is that fidelity already
    achieved cannot be given back quietly.
    """
    existing = load_tolerances()
    updated = dict(existing)
    for name, worst in worst_divergences(report).items():
        if name not in updated or worst < updated[name]:
            updated[name] = round(worst, 4)
    TOLERANCES.write_text(
        json.dumps(
            {
                "note": (
                    "Worst per-subsystem divergence between the live corpus and "
                    "the simulator, in matched configurations — a configuration "
                    "being difficulty, speed, map size AND which side carries "
                    "the rung's handicap. Written only by "
                    "`civ6_trajectory_fidelity.py --write`; a value may tighten "
                    "and never loosens, and `--check` fails only past it plus a "
                    "regression margin (see FIDELITY_SLACK)."
                ),
                "tolerances": updated,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    return updated


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--live", type=Path, default=LIVE_DEFAULT)
    parser.add_argument("--sim", type=Path, default=SIM_DEFAULT)
    parser.add_argument("--json", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--max", type=int, default=0, dest="most")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args(argv)

    live = live_records(args.live)
    if args.check and not live:
        print(
            f"trajectory-fidelity: no live corpus at {args.live} — "
            "skipped on a machine with no recorded runs"
        )
        return 0
    sim = sim_records(args.sim, map_sizes())
    report = ledger(live, sim)

    if args.json:
        args.json.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if args.write:
        write_tolerances(report)

    if not args.check:
        print(render(report))
        return 0

    status, notes = check(report, load_tolerances(), args.most)
    for note in notes:
        print(f"trajectory-fidelity: {note}")
    if not report["matched_cells"]:
        print(
            "trajectory-fidelity: no configuration is populated by both corpora, "
            "so no subsystem was compared"
        )
    return status


if __name__ == "__main__":
    sys.exit(main())
