#!/usr/bin/env python3
"""What the merged live repairs actually did, replayed over every recorded run.

## Why this exists

Three repairs merged on 2026-08-22 against the live Civilization VI bridge, and
every one of them was justified by **one live run**:

| PR | commit | the evidence it shipped with |
|---|---|---|
| #2278 | `c8a90523` | nine Great People and four Traders idle in run `civvis-20260822T020434Z`, turn 231 |
| #2316 | `d55b03f3` | the Cumae-style loyalty trap, one run |
| #2319 | `90bc9a09` | the operator watching a lost position play out |

`docs/EVAL_INTEGRITY.md` and this repository's own memory say the same thing in
four different places: **one seed is never a result.** None of the three can be
priced by `ai_eval`, because none of them runs in a headless game — #2278 lives
in the mirror and the order bridge, #2316 is behind two live-bridge-only flags,
and #2319 is the ladder's own restart policy. So the instrument cannot be the
simulator. It has to be the **recorded corpus**: ~560 finished live runs under
`~/civvis-civ6-runs/control/`, each with the exact `events.jsonl` the live
harness itself consumed.

    python3 tools/live_repair_census.py                       # every recorded run
    python3 tools/live_repair_census.py --since 20260818
    python3 tools/live_repair_census.py --section traders --json out.json

## ⚠ What each section can and cannot answer

Conflating these two questions is the most expensive recurring error in this
repository's history, so every section is labelled with which one it answers:

- **BEHAVIOUR** — did the predicate change what the agent does? A census over
  recorded frames answers this exactly, because the frames are the same input
  the live decider saw.
- **VALUE** — did it help? A census **never** answers this. Only a paired
  win/score contrast does, and for a live-bridge repair that means live games.

Every section here is BEHAVIOUR except `restart`, whose counterfactual half is
labelled where it starts.

## ⚠ It reads and prints

It starts no game, changes no controller, writes nothing into a run directory,
and asks nothing of the host. It is safe under the operator halt and safe
against a game that is still being played.

## ⚠ Where the numbers come from

The harness's early stop is not re-implemented here: `below_leader_score_reading`
(the one rule left after 2026-08-26 — under 70 % of the leader's score after
turn 100; it replaced #2319's three-axis reading) is imported from
`tools/civ6_play.py` and fed the recorded events in file order, which is
exactly what the live loop does. That section is a **check**.

`#2278`'s Great Person section transcribes `StateGreatPerson::slot_starved`
(`src/mirror.rs`) into Python, because the Rust predicate is not reachable from
a script. A transcription is a claim, so it is pinned: the truth table in
`tools/test_live_repair_census.py` is the same case table the Rust unit test
`the_rome_stack_is_starved_even_though_the_empire_owns_empty_slots`
(`src/bin/civvis_orders.rs`) asserts, including the three cases the predicate
must NOT change. Re-run both when either moves.
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import re
import statistics
import sys
import tempfile
from pathlib import Path
from typing import Any, Iterable, Iterator

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_play  # noqa: E402
from civ6_play import below_leader_score_reading  # noqa: E402

DEFAULT_CORPUS = Path.home() / "civvis-civ6-runs" / "control"

#: The line for `--restart-below-leader-ratio`: the harness's own default since
#: 2026-08-26 (`civ6_play.DEFAULT_LEADER_SCORE_RATIO`), so a census at this
#: value replays exactly what a live game would have done.
RESTART_RATIO = 0.70

#: Firaxis's three city-defence buildings, in escalation order. The live report
#: names the second and third by their in-game labels, Castle and Star Fort.
WALL_CHAIN = ("BUILDING_WALLS", "BUILDING_CASTLE", "BUILDING_STAR_FORT")

#: Great Person classes that consume a Great Work slot. These are the ones
#: `slot_starved` was written for; a Scientist or Merchant is a different
#: failure and is counted separately so the two cannot be confused.
CULTURAL_CLASSES = (
    "GREAT_PERSON_CLASS_WRITER",
    "GREAT_PERSON_CLASS_ARTIST",
    "GREAT_PERSON_CLASS_MUSICIAN",
)

SECTIONS = ("great-people", "traders", "restart", "settlers", "vetoes", "army")

#: Units that cost production but are not army. `army_reading` must not count
#: a Builder or a Spy as military: 36% of city-turns "building a UNIT" reads
#: like an army problem and is mostly Builders, Settlers and Traders.
CIVILIAN_UNITS = (
    "UNIT_SETTLER", "UNIT_BUILDER", "UNIT_TRADER", "UNIT_SPY",
    "UNIT_MISSIONARY", "UNIT_APOSTLE", "UNIT_INQUISITOR", "UNIT_GURU",
    "UNIT_ARCHAEOLOGIST", "UNIT_NATURALIST", "UNIT_ROCK_BAND",
    "UNIT_MEDIC", "UNIT_MILITARY_ENGINEER",
)

#: `civvis_orders --explain` writes one of these into every settle-site veto.
#: The first two are FOG — a site refused for what the seat has not seen — and
#: the third is an arithmetic loyalty forecast on ground it has seen. Telling
#: them apart is the whole question of whether the settle veto is a loyalty
#: problem or an exploration problem.
VETO_UNEXPLORED = "has not explored"
VETO_UNSEEN_CITY = "has never seen"
VETO_LOYALTY_RATE = "Loyalty a turn beside its neighbours"


# --------------------------------------------------------------------------
# The corpus
# --------------------------------------------------------------------------


def run_dirs(corpus: Path, since: str | None, until: str | None) -> list[Path]:
    """Every recorded run under `corpus` that has the event stream we read.

    ⚠ Discovered, never listed. A hand-written list of runs is complete on the
    day it is written and silently shrinks afterwards; this repository has paid
    for that mistake three times (`AGENTS.md`, "Discover, never list").
    """
    found = []
    for path in sorted(glob.glob(str(corpus / "civvis-*" / "events.jsonl"))):
        run = os.path.basename(os.path.dirname(path))
        stamp = run.removeprefix("civvis-")[:8]
        if since and stamp < since:
            continue
        if until and stamp > until:
            continue
        found.append(Path(path))
    return found


def events(path: Path) -> Iterator[dict]:
    """Every parseable record of one run, in the order the harness saw them.

    A truncated final line is normal — these files are tailed while a game is
    still running and a killed harness leaves half a record behind. Skipping it
    is right; failing on it would make the census unable to read exactly the
    runs that ended badly, which are the ones worth reading.
    """
    try:
        handle = path.open(errors="replace")
    except OSError:
        return
    with handle:
        for line in handle:
            try:
                record = json.loads(line)
            except (ValueError, TypeError):
                continue
            if isinstance(record, dict):
                yield record


def summary(path: Path) -> dict:
    """The harness's own verdict on a run, from `summary.json` beside the stream.

    ⚠ The event stream is not enough to say who won. A Civilization VI game
    emits a `victory` record for EVERY player's victory and a `defeat` for
    every elimination — 209 `victory` records in this corpus carry
    `won: false`, and 209 `defeat` records carry `ours: false`. Reading the
    stream naively counts a rival's win as ours. `summary.json` carries the
    harness's resolved `outcome`, `last_score` and `rival_best`, and
    `civ6_play` writes it from the same distinction the mod makes.
    """
    try:
        return json.loads((path.parent / "summary.json").read_text(errors="replace"))
    except (OSError, ValueError):
        return {}


def _number(value: object) -> float | None:
    """A finite numeric reading, or None for anything else."""
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    return float(value)


# --------------------------------------------------------------------------
# #2278, half one: the Great Person predicate
# --------------------------------------------------------------------------


def starved_before(person: dict) -> bool:
    """The predicate every escape hatch used before `c8a90523`.

    `!can_activate && empty_slots == Some(0)` — an empire-wide slot tally, and
    the wrong question. A missing `empty_slots` (an older control mod) is an
    absence and was never read as a zero.
    """
    return not person.get("can_activate") and person.get("empty_slots") == 0


def starved_after(person: dict) -> bool:
    """`StateGreatPerson::slot_starved`, transcribed from `src/mirror.rs`.

    Can this person REACH a slot? `empty_slots == Some(0)` stays sufficient;
    beyond it, a non-empty plot list whose every entry says `slot_open: false`
    is the host answering "no" tile by tile. No plots at all is a missing
    DISTRICT, deliberately not a missing slot. `slot_open: null` (an older mod)
    keeps its benefit of the doubt and is never read as either claim.
    """
    if person.get("can_activate"):
        return False
    if person.get("empty_slots") == 0:
        return True
    plots = person.get("activation_plots") or []
    return bool(plots) and all(plot.get("slot_open") is False for plot in plots)


def great_person_reading(records: Iterable[dict]) -> dict:
    """BEHAVIOUR. What #2278's predicate change does to one recorded run."""
    frames = unit_frames = before = after = flips = 0
    cultural_flips = 0
    flipped: set[tuple] = set()
    exports_empty_slots = exports_slot_open = False
    gp_idle_peak = gp_idle_final = 0
    gp_actions: dict[str, int] = {}
    cultural_present = 0

    for record in records:
        kind = record.get("kind")
        if kind == "orders":
            idle = record.get("gp_idle")
            if isinstance(idle, int) and not isinstance(idle, bool):
                gp_idle_peak = max(gp_idle_peak, idle)
                gp_idle_final = idle
            continue
        if kind == "gp":
            action = str(record.get("action") or "unknown")
            klass = str(record.get("class") or "unknown")
            gp_actions[f"{klass}:{action}"] = gp_actions.get(f"{klass}:{action}", 0) + 1
            continue
        if kind != "state" or record.get("ctx") != "agent":
            continue
        seen_person = False
        for unit in record.get("units") or []:
            person = unit.get("great_person")
            if not isinstance(person, dict):
                continue
            seen_person = True
            unit_frames += 1
            klass = person.get("class")
            if klass in CULTURAL_CLASSES:
                cultural_present += 1
            if person.get("empty_slots") is not None:
                exports_empty_slots = True
            plots = person.get("activation_plots") or []
            if any("slot_open" in plot for plot in plots):
                exports_slot_open = True
            was, now = starved_before(person), starved_after(person)
            before += was
            after += now
            if was != now:
                flips += 1
                flipped.add((unit.get("id"), klass))
                if klass in CULTURAL_CLASSES:
                    cultural_flips += 1
        if seen_person:
            frames += 1

    # `gp_frames` counts state frames holding at least one Great Person;
    # `gp_unit_frames` counts person-observations, which is what every rate
    # below is a fraction of. A person standing for forty turns is forty
    # unit-frames, deliberately: the failure being measured is exactly that
    # standing, so weighting by how long it lasted is the point.
    return {
        "gp_frames": frames,
        "gp_unit_frames": unit_frames,
        "starved_before": before,
        "starved_after": after,
        "flips": flips,
        "cultural_unit_frames": cultural_present,
        "cultural_flips": cultural_flips,
        "distinct_people_flipped": len(flipped),
        "exports_empty_slots": exports_empty_slots,
        "exports_slot_open": exports_slot_open,
        "gp_idle_peak": gp_idle_peak,
        "gp_idle_final": gp_idle_final,
        "gp_actions": gp_actions,
    }


# --------------------------------------------------------------------------
# #2278, half two: the trade-route block ledger
# --------------------------------------------------------------------------


def trade_reading(records: Iterable[dict]) -> dict:
    """BEHAVIOUR. What corroboration frees in one recorded run.

    `blocked_trade_routes` was extended and never cleared, so the FIRST refusal
    of an origin/destination pairing retired it for the rest of the game. Under
    corroboration a pairing is condemned only on its second refusal. The
    pairings refused exactly once are therefore exactly the ones the repair
    hands back.
    """
    pairs: dict[tuple, int] = {}
    capacity = routes = 0
    traders = 0
    for record in records:
        kind = record.get("kind")
        if kind == "trade_route_refused":
            key = (record.get("from_x"), record.get("from_y"),
                   record.get("x"), record.get("y"))
            pairs[key] = pairs.get(key, 0) + 1
        elif kind == "state" and record.get("ctx") == "agent":
            value = _number(record.get("trade_capacity"))
            if value is not None:
                capacity = int(value)
            running = record.get("trade_routes")
            if isinstance(running, list):
                routes = len(running)
            units = record.get("units")
            if isinstance(units, list):
                traders = sum(1 for u in units if u.get("kind") == "UNIT_TRADER")
    once = sum(1 for count in pairs.values() if count == 1)
    return {
        "refusals": sum(pairs.values()),
        "distinct_pairings": len(pairs),
        "pairings_refused_once": once,
        "pairings_refused_twice_or_more": len(pairs) - once,
        "final_trade_capacity": capacity,
        "final_running_routes": routes,
        "final_idle_capacity": max(0, capacity - routes),
        "final_traders_alive": traders,
    }


# --------------------------------------------------------------------------
# The early stop (below the leader's score after turn 100), replayed through
# the harness's own function
# --------------------------------------------------------------------------


def restart_reading(records: Iterable[dict], ratio: float, closing: dict) -> dict:
    """BEHAVIOUR, then VALUE-adjacent. Would the early stop have ended this run?

    The verdict half is a **check**, not a transcription: the recorded events
    are fed to `civ6_play.below_leader_score_reading` in file order, which is
    what `_play`'s `finished()` does with the live stream.

    The counterfactual half — what the run went on to do after the turn the
    rule would have stopped it — is the closest a recorded corpus can get to
    value, and it is still not a win/loss contrast: a restart replaces the rest
    of that game with a *different* game whose result is unrecorded. What it
    can say exactly is the **risk**: how many stopped games later climbed back
    over the score line, and how many were won.
    """
    state: dict[str, Any] = {}
    verdict = None
    last_turn = 0
    recovered_after = False
    best_ratio_after = 0.0
    for record in records:
        kind = record.get("kind")
        if kind == "turn" and record.get("ctx") == "agent":
            turn = record.get("turn")
            if isinstance(turn, int) and not isinstance(turn, bool):
                last_turn = max(last_turn, turn)
        fired = below_leader_score_reading(state, record, ratio)
        if fired is not None and verdict is None:
            verdict = fired
        if verdict is not None and kind == "turn" and record.get("ctx") == "agent":
            score = _number(record.get("score"))
            rival = _number(record.get("rival_best"))
            if score is not None and rival and rival > 0:
                best_ratio_after = max(best_ratio_after, score / rival)
                if score / rival >= ratio:
                    recovered_after = True
    outcome = closing.get("outcome") if isinstance(closing.get("outcome"), dict) else {}
    won = bool(outcome.get("won")) if outcome.get("kind") == "victory" else False
    final_score = _number(closing.get("last_score"))
    final_rival = _number(closing.get("rival_best"))
    return {
        "fired": verdict is not None,
        "fire_turn": verdict.get("turn") if verdict else None,
        "fire_score_ratio": verdict.get("score_ratio") if verdict else None,
        "last_turn": last_turn or int(closing.get("last_turn") or 0),
        "turns_after_fire": (last_turn - verdict["turn"]) if verdict else 0,
        "won": won,
        "final_score_ratio": (round(final_score / final_rival, 4)
                              if final_score is not None and final_rival else None),
        "recovered_after_fire": recovered_after,
        "best_score_ratio_after_fire": round(best_ratio_after, 4),
    }


# --------------------------------------------------------------------------
# #2316 and the settler lane
# --------------------------------------------------------------------------


def settler_reading(records: Iterable[dict]) -> dict:
    """BEHAVIOUR. How long Settlers stand around, and what refuses them."""
    first_seen: dict[int, int] = {}
    last_seen: dict[int, int] = {}
    founded: set[int] = set()
    found_refused = 0
    settler_city_turns = 0
    city_turns = 0
    settlers_per_frame: list[int] = []
    for record in records:
        kind = record.get("kind")
        if kind == "found":
            unit = record.get("unit")
            if isinstance(unit, int):
                founded.add(unit)
            continue
        if kind == "found_refused":
            found_refused += 1
            continue
        if kind != "state" or record.get("ctx") != "agent":
            continue
        turn = record.get("turn")
        if not isinstance(turn, int) or isinstance(turn, bool):
            continue
        alive = 0
        for unit in record.get("units") or []:
            if unit.get("kind") != "UNIT_SETTLER":
                continue
            alive += 1
            uid = unit.get("id")
            if not isinstance(uid, int):
                continue
            first_seen.setdefault(uid, turn)
            last_seen[uid] = turn
        settlers_per_frame.append(alive)
        for city in record.get("cities") or []:
            city_turns += 1
            if city.get("producing") == "UNIT_SETTLER":
                settler_city_turns += 1

    lifetimes = [last_seen[uid] - first_seen[uid]
                 for uid in first_seen if uid not in founded]
    return {
        "settlers_seen": len(first_seen),
        "settlers_that_founded": len(founded & set(first_seen)),
        "settlers_that_never_founded": len(lifetimes),
        "never_founded_turns_alive": sorted(lifetimes),
        "never_founded_median_turns_alive": (
            statistics.median(lifetimes) if lifetimes else 0),
        "never_founded_max_turns_alive": max(lifetimes) if lifetimes else 0,
        "found_refused": found_refused,
        "city_turns": city_turns,
        "settler_city_turns": settler_city_turns,
        "mean_settlers_alive": (
            round(statistics.mean(settlers_per_frame), 2) if settlers_per_frame else 0),
    }


# --------------------------------------------------------------------------
# The settle-site veto, and who ends up on the ground it refused
# --------------------------------------------------------------------------

#: `[why] t200 Expansion/Detail Settler refuses (51, 11) before walking there
#:  | <reason>; the site is retired ... [civ6 (56,11) = axial (51,11)]`
VETO_LINE = re.compile(
    r"^\[why\] t(?P<turn>\d+) .*Settler refuses \((?P<ax>-?\d+), (?P<ay>-?\d+)\)"
    r" before walking there \| (?P<why>[^|]*)")


def offset_to_axial(x: int, y: int) -> tuple[int, int]:
    """Civilization VI speaks odd-r OFFSET; CIVVIS stores AXIAL.

    ⚠ Mixing them is silent and has already put a capital on no tile at all
    (`src/bin/civvis_orders.rs`). The journal prints both — `[civ6 (56,11) =
    axial (51,11)]` — and this inverse reproduces every one of those pairs, so
    a rival city exported in offset can be measured against a site the journal
    named in axial.
    """
    return x - ((y - (y & 1)) // 2), y


def hex_distance(a: tuple[int, int], b: tuple[int, int]) -> int:
    """Axial hex distance. Chebyshev on offset coordinates is not this."""
    dq, dr = a[0] - b[0], a[1] - b[1]
    return (abs(dq) + abs(dq + dr) + abs(dr)) // 2


def veto_reading(path: Path, records: list[dict], radius: int) -> dict:
    """BEHAVIOUR. Why the seat refuses a settle site, and who takes it after.

    `docs/AI_GAPS.md` records the conclusion that the fix for the settle veto
    is mid-game EXPLORATION rather than a softer veto, because the ground it
    refuses is settled by rivals who are simply closer and can see it. Both
    halves of that are measurable from a finished run: the journal says which
    refusals are about fog, and the state stream says where rival cities
    subsequently appeared.
    """
    log = path.parent / "why.log"
    reasons = {"unexplored": 0, "unseen_rival_city": 0,
               "loyalty_rate": 0, "other": 0}
    sites: dict[tuple[int, int], int] = {}
    try:
        handle = log.open(errors="replace")
    except OSError:
        return {"veto_log": False, **reasons, "distinct_sites": 0,
                "sites_taken_by_a_rival": 0, "total": 0}
    with handle:
        for line in handle:
            match = VETO_LINE.match(line)
            if not match:
                continue
            why = match.group("why")
            if VETO_UNEXPLORED in why:
                reasons["unexplored"] += 1
            elif VETO_UNSEEN_CITY in why:
                reasons["unseen_rival_city"] += 1
            elif VETO_LOYALTY_RATE in why:
                reasons["loyalty_rate"] += 1
            else:
                reasons["other"] += 1
            site = (int(match.group("ax")), int(match.group("ay")))
            turn = int(match.group("turn"))
            sites[site] = min(sites.get(site, turn), turn)

    # Where rival cities stood, by the turn they were first seen there.
    rival_cities: dict[tuple[int, int], int] = {}
    for record in records:
        if record.get("kind") != "state" or record.get("ctx") != "agent":
            continue
        turn = record.get("turn")
        if not isinstance(turn, int) or isinstance(turn, bool):
            continue
        for rival in record.get("rivals") or []:
            if not isinstance(rival, dict):
                continue
            for city in rival.get("cities") or []:
                x, y = city.get("x"), city.get("y")
                if isinstance(x, int) and isinstance(y, int):
                    here = offset_to_axial(x, y)
                    rival_cities[here] = min(rival_cities.get(here, turn), turn)

    taken = 0
    for site, vetoed_at in sites.items():
        if any(hex_distance(site, city) <= radius and seen >= vetoed_at
               for city, seen in rival_cities.items()):
            taken += 1
    return {
        "veto_log": True,
        **reasons,
        "total": sum(reasons.values()),
        "distinct_sites": len(sites),
        "sites_taken_by_a_rival": taken,
    }


# --------------------------------------------------------------------------
# The peacetime army and the wall chain
# --------------------------------------------------------------------------


def army_reading(records: Iterable[dict]) -> dict:
    """BEHAVIOUR. Our military against the best rival's, split by war state."""
    peace_ratios: list[float] = []
    war_ratios: list[float] = []
    late_peace_ratios: list[float] = []
    walls: dict[str, int] = {name: 0 for name in WALL_CHAIN}
    wall_city_turns = 0
    city_turns = 0
    military_city_turns = 0
    civilian_city_turns = 0
    wall_city_builds = 0
    army_units: list[int] = []
    army_per_city: list[float] = []
    final = {"military": 0.0, "rival_best": 0.0, "turn": 0, "at_war": False}
    for record in records:
        if record.get("kind") != "state" or record.get("ctx") != "agent":
            continue
        turn = record.get("turn")
        ours = _number(record.get("military"))
        rivals = record.get("rivals")
        if not isinstance(turn, int) or ours is None or not isinstance(rivals, list):
            continue
        strengths = [_number(r.get("military")) for r in rivals
                     if isinstance(r, dict)]
        strengths = [s for s in strengths if s is not None and s > 0]
        at_war = any(isinstance(r, dict) and r.get("at_war") for r in rivals)
        units = record.get("units")
        cities = record.get("cities")
        if isinstance(units, list) and isinstance(cities, list) and cities:
            fielded = sum(1 for u in units
                          if isinstance(u.get("kind"), str)
                          and u["kind"].startswith("UNIT_")
                          and u["kind"] not in CIVILIAN_UNITS
                          and not u["kind"].startswith("UNIT_GREAT_")
                          and not u.get("great_person"))
            army_units.append(fielded)
            army_per_city.append(fielded / len(cities))
        if strengths:
            ratio = ours / max(strengths)
            (war_ratios if at_war else peace_ratios).append(ratio)
            if not at_war and turn >= 100:
                late_peace_ratios.append(ratio)
            final = {"military": ours, "rival_best": max(strengths),
                     "turn": turn, "at_war": at_war}
        for city in record.get("cities") or []:
            city_turns += 1
            held = [name for name in WALL_CHAIN
                    if name in (city.get("buildings") or [])]
            for name in held:
                walls[name] += 1
            if held:
                wall_city_turns += 1
            item = city.get("producing")
            if not isinstance(item, str):
                continue
            if item in WALL_CHAIN:
                wall_city_builds += 1
            if not item.startswith("UNIT_"):
                continue
            if item in CIVILIAN_UNITS or item.startswith("UNIT_GREAT_"):
                civilian_city_turns += 1
            else:
                military_city_turns += 1

    def mean(values: list[float]) -> float:
        return round(statistics.mean(values), 3) if values else 0.0

    return {
        "peace_frames": len(peace_ratios),
        "war_frames": len(war_ratios),
        "mean_peace_army_ratio": mean(peace_ratios),
        "mean_war_army_ratio": mean(war_ratios),
        "mean_late_peace_army_ratio": mean(late_peace_ratios),
        "max_peace_army_ratio": round(max(peace_ratios), 3) if peace_ratios else 0.0,
        "final_military": final["military"],
        "final_rival_best_military": final["rival_best"],
        "final_turn": final["turn"],
        "wall_city_turns": wall_city_turns,
        "city_turns": city_turns,
        "military_city_turns": military_city_turns,
        "civilian_city_turns": civilian_city_turns,
        "wall_city_builds": wall_city_builds,
        "mean_army_units": (round(statistics.mean(army_units), 2)
                            if army_units else 0.0),
        "mean_army_per_city": (round(statistics.mean(army_per_city), 3)
                               if army_per_city else 0.0),
        "max_army_per_city": (round(max(army_per_city), 3)
                              if army_per_city else 0.0),
        **{f"held_{name.removeprefix('BUILDING_').lower()}": count
           for name, count in walls.items()},
    }


# --------------------------------------------------------------------------
# One pass per run
# --------------------------------------------------------------------------


def run_reading(path: Path, ratio: float, radius: int,
                sections: tuple[str, ...]) -> dict:
    """Every requested section of one run, from a single read of its stream."""
    records = list(events(path))
    reading = {"run": path.parent.name}
    if "great-people" in sections:
        reading["great_people"] = great_person_reading(records)
    if "traders" in sections:
        reading["traders"] = trade_reading(records)
    if "restart" in sections:
        reading["restart"] = restart_reading(records, ratio, summary(path))
    if "settlers" in sections:
        reading["settlers"] = settler_reading(records)
    if "vetoes" in sections:
        reading["vetoes"] = veto_reading(path, records, radius)
    if "army" in sections:
        reading["army"] = army_reading(records)
    return reading


# --------------------------------------------------------------------------
# The paired replay: two deciders, the same recorded turns
# --------------------------------------------------------------------------

#: The counters `civvis_orders` writes into the `note` field of every answer.
#: `no_empty_slot` is the readout of `StateGreatPerson::slot_starved` itself —
#: the branch that stands a Great Person still — so it says directly how often
#: #2278's predicate fired.
NOTE_COUNTERS = re.compile(
    r"great_people_orders=(?P<gp_orders>\d+)"
    r"|great_people_without_activation_target=(?P<gp_stalled>\d+)"
    r" \(cooldown=(?P<cooldown>\d+) no_plot=(?P<no_plot>\d+)"
    r" no_empty_slot=(?P<no_empty_slot>\d+)\)")

#: Journal lines that exist only because of one of the repairs under test.
#: `--explain` prints them, and their presence or absence in an arm is the
#: repair firing or not firing on that exact recorded turn.
JOURNAL_MARKERS = {
    "gp_activation_path": "activation path for an idle Great Person",
    "loyalty_doomed_fallback": "loyalty-doomed fallback",
}

#: ★ THE AGENT'S OWN ARMY ARITHMETIC, in its own words, on every production
#: decision it explains: "the empire holds 8 military for 9 cities against a
#: target of 1.0 each". Reading the army this way needs no hand-written list of
#: which unit kinds are naval — a list that is complete the day it is written
#: and shrinks afterwards — and it is the exact quantity
#: `enemy_weighted_army_target` compares, which a headcount off the state
#: export is not.
ARMY_LINE = re.compile(
    r"the empire holds (?P<held>\d+) military for (?P<cities>\d+) cities"
    r" against a target of (?P<target>[\d.]+) each")


def replay_turn(binary: Path, mirror: Path, turn: int, victory: str,
                timeout: float) -> dict:
    """One recorded turn answered by one decider, exactly as the brain asks it.

    ⚠ The argument list is `civ6_brain.civvis_orders`'s, plus `--explain`. In
    particular it does NOT pass `--players`: the live brain does not either,
    so a replay that did would be answering a different board than the seat
    ever saw.
    """
    import subprocess
    try:
        done = subprocess.run(
            [str(binary), "--mirror", str(mirror), "--turn", str(turn),
             "--victory", victory, "--explain"],
            capture_output=True, text=True, timeout=timeout, check=False)
    except (OSError, subprocess.SubprocessError) as exc:
        return {"ok": False, "why": str(exc)}
    if done.returncode != 0:
        return {"ok": False, "why": done.stderr.strip()[-200:]}
    try:
        answer = json.loads(done.stdout)
    except ValueError:
        return {"ok": False, "why": "stdout was not JSON"}
    orders = answer.get("orders") or []
    note = str(answer.get("note") or "")
    counters = {"gp_orders": 0, "gp_stalled": 0, "cooldown": 0,
                "no_plot": 0, "no_empty_slot": 0}
    for match in NOTE_COUNTERS.finditer(note):
        for key, value in match.groupdict().items():
            if value is not None:
                counters[key] = int(value)
    markers = {name: done.stderr.count(text)
               for name, text in JOURNAL_MARKERS.items()}
    army = ARMY_LINE.search(done.stderr)
    return {
        "ok": True,
        "army_held": int(army.group("held")) if army else None,
        "army_cities": int(army.group("cities")) if army else None,
        "army_target_each": float(army.group("target")) if army else None,
        "orders": [(o.get("kind"), o.get("subject"), o.get("verb"),
                    o.get("x"), o.get("y")) for o in orders],
        "found_city": sum(1 for o in orders if o.get("verb") == "FOUND_CITY"),
        "deal": sum(1 for o in orders if o.get("kind") == "deal"),
        **counters,
        **markers,
    }


def replay(arms: dict[str, Path], paths: list[Path], turns: range,
           victory: str, jobs: int, timeout: float) -> list[dict]:
    """Every arm answers every requested turn of every requested run.

    ⚠ The mirror directory handed to the decider is a temporary directory
    holding a SYMLINK to the archived `events.jsonl`, never the archive itself.
    `civvis_orders` only reads, but a measurement must not be the thing that
    finds out otherwise about 179 GB of irreplaceable recordings.
    """
    import concurrent.futures
    rows: list[dict] = []
    with tempfile.TemporaryDirectory(prefix="live-repair-replay-") as tmp:
        jobs_out = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as pool:
            for path in paths:
                mirror = Path(tmp) / path.parent.name
                mirror.mkdir(exist_ok=True)
                link = mirror / "events.jsonl"
                if not link.exists():
                    link.symlink_to(path.resolve())
                lane = recorded_victory(path) or victory
                for turn in turns:
                    for arm, binary in arms.items():
                        jobs_out.append((
                            path.parent.name, turn, arm,
                            pool.submit(replay_turn, binary, mirror, turn,
                                        lane, timeout)))
            for run, turn, arm, future in jobs_out:
                rows.append({"run": run, "turn": turn, "arm": arm,
                             **future.result()})
    return rows


def recorded_victory(path: Path) -> str | None:
    """The victory lane the run itself was played with, from its `why.log`.

    The decider's startup line names it. Replaying a run on today's default
    lane instead would compare two arms on a game neither of them played.
    """
    try:
        handle = (path.parent / "why.log").open(errors="replace")
    except OSError:
        return None
    with handle:
        for line in handle:
            if '"kind":"genome"' not in line and '"kind": "genome"' not in line:
                continue
            try:
                lane = json.loads(line).get("victory")
            except ValueError:
                continue
            if isinstance(lane, str) and lane:
                return lane
    return None


def report_replay(rows: list[dict], arms: list[str]) -> list[str]:
    """BEHAVIOUR. What the two deciders answered differently, turn by turn."""
    ok = [r for r in rows if r.get("ok")]
    failed = len(rows) - len(ok)
    by_key: dict[tuple, dict] = {}
    for row in ok:
        by_key.setdefault((row["run"], row["turn"]), {})[row["arm"]] = row
    paired = [v for v in by_key.values() if len(v) == len(arms)]
    differing = [v for v in paired if len({tuple(v[a]["orders"]) for a in arms}) > 1]
    lines = [
        "=== paired replay of the recorded turns  [BEHAVIOUR] ===",
        f"  arms                                 {', '.join(arms)}",
        f"  recorded turns answered by both      {len(paired)}"
        f"  (failed invocations: {failed})",
        f"  turns whose ORDER SET differs        {len(differing)}"
        f"  ({100 * len(differing) / len(paired) if paired else 0:.1f}%)",
    ]
    for key in ("gp_orders", "no_empty_slot", "no_plot", "found_city", "deal",
                "gp_activation_path", "loyalty_doomed_fallback"):
        totals = " ".join(
            f"{arm}={sum(v[arm].get(key, 0) for v in paired)}" for arm in arms)
        lines.append(f"  {key:<34} {totals}")

    # The decider's own army arithmetic, averaged over the turns it explained
    # one. `held / cities` against `target` is the only comparison that uses
    # the same quantity on both sides.
    for arm in arms:
        held = [v[arm]["army_held"] / v[arm]["army_cities"] for v in paired
                if v[arm].get("army_cities")]
        target = [v[arm]["army_target_each"] for v in paired
                  if v[arm].get("army_target_each") is not None]
        if not held:
            continue
        lines.append(
            f"  army held/city [{arm}]  mean {_mean(held):.2f}"
            f"  median {statistics.median(held):.2f}"
            f"  vs its own target/city mean {_mean(target):.2f}"
            f"  ({len(held)} explained turns)")

    # Where the two arms spent the production they took away from each other.
    tally: dict[str, dict[str, int]] = {}
    for arm in arms:
        for pair in paired:
            for kind, _subject, verb, _x, _y in pair[arm]["orders"]:
                key = f"{kind}:{verb}"
                tally.setdefault(key, {a: 0 for a in arms})[arm] += 1
    moved = sorted(
        ((key, counts) for key, counts in tally.items()
         if len(set(counts.values())) > 1),
        key=lambda row: -abs(row[1][arms[-1]] - row[1][arms[0]]))
    if moved:
        lines.append("  what moved (top 12 by absolute change):")
        for key, counts in moved[:12]:
            delta = counts[arms[-1]] - counts[arms[0]]
            body = "  ".join(f"{arm}={counts[arm]}" for arm in arms)
            lines.append(f"    {key:<44} {body}  ({delta:+d})")
    return lines


# --------------------------------------------------------------------------
# Reporting
# --------------------------------------------------------------------------


def _sum(rows: list[dict], section: str, key: str) -> float:
    return sum(row[section].get(key, 0) or 0 for row in rows if section in row)


def _mean(values: list[float]) -> float:
    return round(statistics.mean(values), 3) if values else 0.0


def _quantile(sorted_values: list[float], fraction: float) -> float:
    """A nearest-rank quantile of an already sorted list; 0.0 when it is empty.

    `statistics.quantiles` needs at least two points and interpolates, which
    makes a small corpus report a number no run produced. Nearest rank always
    names an observation.
    """
    if not sorted_values:
        return 0.0
    index = min(len(sorted_values) - 1,
                max(0, int(round(fraction * (len(sorted_values) - 1)))))
    return float(sorted_values[index])


def report_great_people(rows: list[dict]) -> list[str]:
    live = [r for r in rows if r["great_people"]["gp_unit_frames"]]
    modern = [r for r in live if r["great_people"]["exports_slot_open"]]
    changed = [r for r in live if r["great_people"]["flips"]]
    frames = _sum(live, "great_people", "gp_unit_frames")
    before = _sum(live, "great_people", "starved_before")
    after = _sum(live, "great_people", "starved_after")
    flips = _sum(live, "great_people", "flips")
    modern_frames = _sum(modern, "great_people", "gp_unit_frames")
    modern_before = _sum(modern, "great_people", "starved_before")
    modern_flips = _sum(modern, "great_people", "flips")
    cultural = _sum(live, "great_people", "cultural_unit_frames")
    cultural_flips = _sum(live, "great_people", "cultural_flips")
    idle_peaks = [r["great_people"]["gp_idle_peak"] for r in live]
    cultural_orders = sum(
        count for r in live
        for key, count in r["great_people"]["gp_actions"].items()
        if key.split(":")[0] in CULTURAL_CLASSES)
    return [
        "=== #2278 half one: the Great Person slot predicate  [BEHAVIOUR] ===",
        f"  runs exporting a Great Person        {len(live)}",
        f"  of those, exporting `slot_open`      {len(modern)}"
        "   (an older control mod sends none, and is never read as a claim)",
        f"  runs whose verdict CHANGES           {len(changed)}",
        f"  Great-Person unit-frames             {frames:.0f}",
        f"    starved under the OLD predicate    {before:.0f}"
        f"  ({100 * before / frames if frames else 0:.1f}%)",
        f"    starved under `slot_starved`       {after:.0f}"
        f"  ({100 * after / frames if frames else 0:.1f}%)",
        f"    verdict flips                      {flips:.0f}",
        "  restricted to runs whose mod exports `slot_open` — the only ones where",
        "  the repair can fire at all:",
        f"    unit-frames                        {modern_frames:.0f}",
        f"    starved under the OLD predicate    {modern_before:.0f}"
        f"  ({100 * modern_before / modern_frames if modern_frames else 0:.1f}%)",
        f"    verdict flips                      {modern_flips:.0f}"
        f"  ({100 * modern_flips / modern_frames if modern_frames else 0:.1f}%)",
        f"  cultural (Writer/Artist/Musician) unit-frames {cultural:.0f},"
        f" of which flipped {cultural_flips:.0f}",
        f"  cultural Great People given ANY order, whole corpus: {cultural_orders}",
        f"  `gp_idle` peak per run: mean {_mean(idle_peaks):.2f},"
        f" max {max(idle_peaks) if idle_peaks else 0}",
    ]


def report_traders(rows: list[dict]) -> list[str]:
    live = [r for r in rows if r["traders"]["refusals"]]
    pairings = _sum(live, "traders", "distinct_pairings")
    once = _sum(live, "traders", "pairings_refused_once")
    gaps = [r["traders"]["final_idle_capacity"] for r in rows
            if r["traders"]["final_trade_capacity"]]
    idle_traders = [r["traders"]["final_traders_alive"] for r in rows
                    if r["traders"]["final_trade_capacity"]]
    return [
        "=== #2278 half two: the trade-route block ledger  [BEHAVIOUR] ===",
        f"  runs with at least one refusal       {len(live)}",
        f"  distinct origin/destination pairings {pairings:.0f}",
        f"  refused EXACTLY ONCE                 {once:.0f}"
        f"  ({100 * once / pairings if pairings else 0:.1f}%)"
        "  <- condemned for the game by the old rule, handed back by corroboration",
        f"  refused twice or more                {pairings - once:.0f}"
        "   <- condemned by both rules",
        f"  final unused trade capacity, mean    {_mean(gaps):.2f}"
        f"  over {len(gaps)} runs",
        f"  Traders alive at the end, mean       {_mean(idle_traders):.2f}",
    ]


def report_restart(rows: list[dict], ratio: float) -> list[str]:
    fired = [r for r in rows if r["restart"]["fired"]]
    recovered = [r for r in fired if r["restart"]["recovered_after_fire"]]
    won = [r for r in fired if r["restart"]["won"]]
    saved = sum(r["restart"]["turns_after_fire"] for r in fired)
    turns = [r["restart"]["fire_turn"] for r in fired]
    all_won = [r for r in rows if r["restart"]["won"]]
    finals = [r["restart"]["final_score_ratio"] for r in fired
              if r["restart"]["final_score_ratio"] is not None]
    return [
        f"=== #2319: the three-signal restart, replayed at ratio {ratio}"
        "  [BEHAVIOUR, then RISK] ===",
        f"  runs replayed                        {len(rows)}",
        f"  runs the rule would have RESTARTED   {len(fired)}"
        f"  ({100 * len(fired) / len(rows) if rows else 0:.1f}%)",
        f"  median turn it fires                 "
        f"{statistics.median(turns) if turns else 0:.0f}",
        f"  turns of play it would have skipped  {saved}"
        f"  (mean {saved / len(fired) if fired else 0:.0f} per stopped run)",
        "  RISK — what the stopped games went on to do:",
        f"    later reached the score ratio again  {len(recovered)}"
        f"  ({100 * len(recovered) / len(fired) if fired else 0:.1f}% of stopped runs)",
        f"    later WON                            {len(won)}",
        f"    median FINAL score ratio             "
        f"{statistics.median(finals) if finals else 0:.2f}"
        "   (where the stopped game actually ended up)",
        f"  for scale, wins anywhere in the corpus {len(all_won)}"
        f" of {len(rows)}",
        "  ⚠ NOT a value reading. A restart replaces the rest of that game with a",
        "    different game whose result is unrecorded; this bounds the RISK only.",
    ]


def report_settlers(rows: list[dict]) -> list[str]:
    seen = _sum(rows, "settlers", "settlers_seen")
    never = _sum(rows, "settlers", "settlers_that_never_founded")
    refused = _sum(rows, "settlers", "found_refused")
    city_turns = _sum(rows, "settlers", "city_turns")
    settler_turns = _sum(rows, "settlers", "settler_city_turns")
    lifetimes = sorted(t for r in rows
                       for t in r["settlers"]["never_founded_turns_alive"])
    long_idle = sum(1 for t in lifetimes if t >= 50)
    return [
        "=== #2316 and the settler lane  [BEHAVIOUR] ===",
        f"  Settlers seen                        {seen:.0f}",
        f"  never founded a city                 {never:.0f}"
        f"  ({100 * never / seen if seen else 0:.1f}%)",
        "  of those, turns alive:"
        f"  median {statistics.median(lifetimes) if lifetimes else 0:.0f}"
        f"  p90 {_quantile(lifetimes, 0.90):.0f}"
        f"  p99 {_quantile(lifetimes, 0.99):.0f}"
        f"  max {max(lifetimes) if lifetimes else 0}",
        f"    alive 50+ turns without founding   {long_idle}"
        f"  ({100 * long_idle / len(lifetimes) if lifetimes else 0:.1f}% of them)",
        f"  `found_refused` events, mean per run {refused / len(rows) if rows else 0:.1f}",
        f"  city-turns producing a Settler       {settler_turns:.0f}"
        f" of {city_turns:.0f}"
        f"  ({100 * settler_turns / city_turns if city_turns else 0:.1f}%)",
    ]


def report_army(rows: list[dict]) -> list[str]:
    peace = [r["army"]["mean_peace_army_ratio"] for r in rows
             if r["army"]["peace_frames"]]
    late = [r["army"]["mean_late_peace_army_ratio"] for r in rows
            if r["army"]["mean_late_peace_army_ratio"]]
    war = [r["army"]["mean_war_army_ratio"] for r in rows
           if r["army"]["war_frames"]]
    peaks = sorted(r["army"]["max_peace_army_ratio"] for r in rows
                   if r["army"]["peace_frames"])
    over_two = sum(1 for r in peace if r >= 2.0)
    city_turns = _sum(rows, "army", "city_turns")
    military_turns = _sum(rows, "army", "military_city_turns")
    civilian_turns = _sum(rows, "army", "civilian_city_turns")
    wall_builds = _sum(rows, "army", "wall_city_builds")
    per_city = [r["army"]["mean_army_per_city"] for r in rows
                if r["army"]["mean_army_per_city"]]
    return [
        "=== the peacetime army and the wall chain  [BEHAVIOUR] ===",
        f"  runs with a peacetime reading        {len(peace)}",
        f"  our military / best rival's, AT PEACE      mean {_mean(peace):.2f}"
        f"  median {statistics.median(peace) if peace else 0:.2f}"
        f"  p90 {_quantile(sorted(peace), 0.90):.2f}",
        f"    runs whose PEACETIME MEAN is 2x or more  {over_two}"
        f"  ({100 * over_two / len(peace) if peace else 0:.1f}%)",
        f"    peak peacetime ratio within a run: median "
        f"{statistics.median(peaks) if peaks else 0:.2f}"
        f"  p90 {_quantile(peaks, 0.90):.2f}",
        f"  same, at peace from turn 100 onwards       mean {_mean(late):.2f}",
        f"  same, AT WAR                               mean {_mean(war):.2f}"
        f"  over {len(war)} runs",
        "  ⚠ `military` is the host's STRENGTH aggregate, not a unit count. The",
        "    two say different things and only one of them is a spending decision:",
        f"  military units fielded per city, mean      {_mean(per_city):.2f}"
        f"  median {statistics.median(per_city) if per_city else 0:.2f}"
        f"  p90 {_quantile(sorted(per_city), 0.90):.2f}",
        f"  city-turns building a MILITARY unit  {military_turns:.0f}"
        f" of {city_turns:.0f}"
        f"  ({100 * military_turns / city_turns if city_turns else 0:.1f}%)",
        f"  city-turns building a CIVILIAN unit  {civilian_turns:.0f}"
        f"  ({100 * civilian_turns / city_turns if city_turns else 0:.1f}%)",
        f"  city-turns building a WALL           {wall_builds:.0f}"
        f"  ({100 * wall_builds / city_turns if city_turns else 0:.1f}%)",
        f"  city-turns holding Walls             "
        f"{_sum(rows, 'army', 'held_walls'):.0f}"
        f"  ({100 * _sum(rows, 'army', 'held_walls') / city_turns if city_turns else 0:.1f}%)",
        f"  city-turns holding a Castle          "
        f"{_sum(rows, 'army', 'held_castle'):.0f}"
        f"  ({100 * _sum(rows, 'army', 'held_castle') / city_turns if city_turns else 0:.1f}%)",
        f"  city-turns holding a Star Fort       "
        f"{_sum(rows, 'army', 'held_star_fort'):.0f}"
        f"  ({100 * _sum(rows, 'army', 'held_star_fort') / city_turns if city_turns else 0:.1f}%)",
    ]


def report_vetoes(rows: list[dict]) -> list[str]:
    # ⚠ TWO DENOMINATORS, AND THEY ARE NOT THE SAME ONE. A run can carry a
    # decider journal and never veto a site; quoting a per-run rate against
    # the journal count understates it by 4.7x, and quoting it against the
    # vetoing count while calling them "runs with a journal" is the mislabel
    # this line used to carry. Print both and say which the rate is over.
    journalled = [r for r in rows if r["vetoes"]["veto_log"]]
    live = [r for r in journalled if r["vetoes"]["total"]]
    total = _sum(live, "vetoes", "total")
    fog = _sum(live, "vetoes", "unexplored") + _sum(live, "vetoes", "unseen_rival_city")
    sites = _sum(live, "vetoes", "distinct_sites")
    taken = _sum(live, "vetoes", "sites_taken_by_a_rival")
    return [
        "=== the settle-site veto: is it loyalty, or is it fog?  [BEHAVIOUR] ===",
        f"  runs with a decider journal          {len(journalled)}",
        f"    of those, vetoing at least one site  {len(live)}",
        f"  settle-site vetoes                   {total:.0f}"
        f"  (mean {total / len(live) if live else 0:.0f} per VETOING run,"
        f" {total / len(journalled) if journalled else 0:.0f} per journalled run)",
        f"    ground the seat HAS NOT EXPLORED   "
        f"{_sum(live, 'vetoes', 'unexplored'):.0f}"
        f"  ({100 * _sum(live, 'vetoes', 'unexplored') / total if total else 0:.1f}%)",
        f"    a rival city it has NEVER SEEN     "
        f"{_sum(live, 'vetoes', 'unseen_rival_city'):.0f}"
        f"  ({100 * _sum(live, 'vetoes', 'unseen_rival_city') / total if total else 0:.1f}%)",
        f"    an arithmetic Loyalty forecast     "
        f"{_sum(live, 'vetoes', 'loyalty_rate'):.0f}"
        f"  ({100 * _sum(live, 'vetoes', 'loyalty_rate') / total if total else 0:.1f}%)",
        f"    anything else                      {_sum(live, 'vetoes', 'other'):.0f}",
        f"  ★ FOG accounts for                   {fog:.0f} of {total:.0f}"
        f"  ({100 * fog / total if total else 0:.1f}%)",
        f"  distinct sites vetoed                {sites:.0f}",
        f"    a rival city later stood within reach of one  {taken:.0f}"
        f"  ({100 * taken / sites if sites else 0:.1f}%)",
    ]


REPORTERS = {
    "great-people": report_great_people,
    "traders": report_traders,
    "restart": report_restart,
    "settlers": report_settlers,
    "vetoes": report_vetoes,
    "army": report_army,
}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS,
                        help=f"directory of recorded runs (default {DEFAULT_CORPUS})")
    parser.add_argument("--since", help="earliest run stamp, YYYYMMDD")
    parser.add_argument("--until", help="latest run stamp, YYYYMMDD")
    parser.add_argument("--section", action="append", choices=SECTIONS,
                        help="repeatable; default is every section")
    parser.add_argument("--restart-ratio", type=float, default=RESTART_RATIO,
                        help=f"score ratio for the early stop (default {RESTART_RATIO},"
                             " the operator value; the flag's own default of 0.0"
                             " disables the policy)")
    parser.add_argument("--replay", action="append", metavar="NAME=BINARY",
                        help="repeatable; answer the recorded turns again with"
                             " this `civvis_orders` build and diff the arms")
    parser.add_argument("--replay-turns", default="60:250:5",
                        help="first:last:step of recorded turns to answer"
                             " (default 60:250:5)")
    parser.add_argument("--replay-victory", default="diplomatic",
                        help="fallback lane when a run's own why.log does not"
                             " name one (default diplomatic)")
    parser.add_argument("--jobs", type=int, default=4,
                        help="parallel decider invocations (default 4; this is"
                             " a shared machine)")
    parser.add_argument("--replay-timeout", type=float, default=120.0)
    parser.add_argument("--veto-radius", type=int, default=3,
                        help="hex distance at which a later rival city counts as"
                             " having taken a vetoed site (default 3)")
    parser.add_argument("--json", type=Path, help="also write the per-run rows here")
    args = parser.parse_args(argv)

    sections = tuple(args.section or SECTIONS)
    paths = run_dirs(args.corpus, args.since, args.until)
    if not paths:
        print(f"no recorded runs under {args.corpus}", file=sys.stderr)
        return 1

    if args.replay:
        arms = {}
        for spec in args.replay:
            name, _, binary = spec.partition("=")
            if not name or not binary or not Path(binary).is_file():
                print(f"--replay wants NAME=BINARY; {spec!r} is not one",
                      file=sys.stderr)
                return 2
            arms[name] = Path(binary)
        first, last, step = (int(part) for part in args.replay_turns.split(":"))
        rows = replay(arms, paths, range(first, last + 1, step),
                      args.replay_victory, args.jobs, args.replay_timeout)
        print(f"live-repair replay: {len(paths)} runs, turns {args.replay_turns}")
        print()
        for line in report_replay(rows, list(arms)):
            print(line)
        if args.json:
            args.json.write_text(json.dumps(rows, indent=2, sort_keys=True) + "\n")
            print(f"\nper-invocation rows written to {args.json}")
        return 0

    rows = [run_reading(path, args.restart_ratio, args.veto_radius, sections)
            for path in paths]
    print(f"live-repair census: {len(rows)} recorded runs under {args.corpus}")
    if args.since or args.until:
        print(f"  window: {args.since or 'start'}..{args.until or 'end'}")
    print()
    for name in SECTIONS:
        if name not in sections:
            continue
        reporter = REPORTERS[name]
        lines = (reporter(rows, args.restart_ratio) if name == "restart"
                 else reporter(rows))
        for line in lines:
            print(line)
        print()

    if args.json:
        args.json.write_text(json.dumps(rows, indent=2, sort_keys=True) + "\n")
        print(f"per-run rows written to {args.json}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
