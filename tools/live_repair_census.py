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

`#2319`'s reading is not re-implemented here: `behind_all_metrics_reading` is
imported from `tools/civ6_play.py` and fed the recorded events in file order,
which is exactly what the live loop does. That section is a **check**.

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
import statistics
import sys
from pathlib import Path
from typing import Any, Iterable, Iterator

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_play import behind_all_metrics_reading  # noqa: E402

DEFAULT_CORPUS = Path.home() / "civvis-civ6-runs" / "control"

#: The operator value for `--restart-below-leader-ratio`, recorded in
#: `tools/civ6_play.py`'s own help text as "Operator request 2026-08-22: 0.70".
#: The flag's *default* is 0.0, which disables the policy entirely, so a census
#: run at the default would report nothing and prove nothing.
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

SECTIONS = ("great-people", "traders", "restart", "settlers", "army")


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
# #2319: the restart policy, replayed through its own function
# --------------------------------------------------------------------------


def restart_reading(records: Iterable[dict], ratio: float) -> dict:
    """BEHAVIOUR, then VALUE-adjacent. Would #2319 have ended this run?

    The verdict half is a **check**, not a transcription: the recorded events
    are fed to `civ6_play.behind_all_metrics_reading` in file order, which is
    what `_play`'s `finished()` does with the live stream.

    The counterfactual half — what the run went on to do after the turn the
    rule would have stopped it — is the closest a recorded corpus can get to
    value, and it is still not a win/loss contrast: a restart replaces the rest
    of that game with a *different* game whose result is unrecorded. What it
    can say exactly is the **risk**: how many stopped games later recovered on
    any of the three axes, and how many were won.
    """
    state: dict[str, Any] = {}
    verdict = None
    last_turn = 0
    outcome = None
    recovered_after = False
    best_ratio_after = 0.0
    for record in records:
        kind = record.get("kind")
        if kind == "turn" and record.get("ctx") == "agent":
            turn = record.get("turn")
            if isinstance(turn, int) and not isinstance(turn, bool):
                last_turn = max(last_turn, turn)
        if kind == "victory":
            outcome = "victory" if record.get("won") else "rival_victory"
        elif kind == "defeat" and record.get("ours"):
            outcome = "defeat"
        fired = behind_all_metrics_reading(state, record, ratio)
        if fired is not None and verdict is None:
            verdict = fired
        if verdict is not None and kind == "turn" and record.get("ctx") == "agent":
            score = _number(record.get("score"))
            rival = _number(record.get("rival_best"))
            if score is not None and rival and rival > 0:
                best_ratio_after = max(best_ratio_after, score / rival)
                if score / rival >= ratio:
                    recovered_after = True
    return {
        "fired": verdict is not None,
        "fire_turn": verdict.get("turn") if verdict else None,
        "fire_score_ratio": verdict.get("score_ratio") if verdict else None,
        "last_turn": last_turn,
        "turns_after_fire": (last_turn - verdict["turn"]) if verdict else 0,
        "outcome": outcome,
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
            if isinstance(item, str) and item.startswith("UNIT_"):
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
        "unit_city_turns": military_city_turns,
        **{f"held_{name.removeprefix('BUILDING_').lower()}": count
           for name, count in walls.items()},
    }


# --------------------------------------------------------------------------
# One pass per run
# --------------------------------------------------------------------------


def run_reading(path: Path, ratio: float, sections: tuple[str, ...]) -> dict:
    """Every requested section of one run, from a single read of its stream."""
    records = list(events(path))
    reading = {"run": path.parent.name}
    if "great-people" in sections:
        reading["great_people"] = great_person_reading(records)
    if "traders" in sections:
        reading["traders"] = trade_reading(records)
    if "restart" in sections:
        reading["restart"] = restart_reading(records, ratio)
    if "settlers" in sections:
        reading["settlers"] = settler_reading(records)
    if "army" in sections:
        reading["army"] = army_reading(records)
    return reading


# --------------------------------------------------------------------------
# Reporting
# --------------------------------------------------------------------------


def _sum(rows: list[dict], section: str, key: str) -> float:
    return sum(row[section].get(key, 0) or 0 for row in rows if section in row)


def _mean(values: list[float]) -> float:
    return round(statistics.mean(values), 3) if values else 0.0


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
    won = [r for r in fired if r["restart"]["outcome"] == "victory"]
    saved = sum(r["restart"]["turns_after_fire"] for r in fired)
    turns = [r["restart"]["fire_turn"] for r in fired]
    all_won = [r for r in rows if r["restart"]["outcome"] == "victory"]
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
        f"  for scale, wins anywhere in the corpus {len(all_won)}",
        "  ⚠ NOT a value reading. A restart replaces the rest of that game with a",
        "    different game whose result is unrecorded; this bounds the RISK only.",
    ]


def report_settlers(rows: list[dict]) -> list[str]:
    seen = _sum(rows, "settlers", "settlers_seen")
    never = _sum(rows, "settlers", "settlers_that_never_founded")
    refused = _sum(rows, "settlers", "found_refused")
    city_turns = _sum(rows, "settlers", "city_turns")
    settler_turns = _sum(rows, "settlers", "settler_city_turns")
    lifetimes = [r["settlers"]["never_founded_median_turns_alive"]
                 for r in rows if r["settlers"]["settlers_that_never_founded"]]
    return [
        "=== #2316 and the settler lane  [BEHAVIOUR] ===",
        f"  Settlers seen                        {seen:.0f}",
        f"  never founded a city                 {never:.0f}"
        f"  ({100 * never / seen if seen else 0:.1f}%)",
        f"  median turns alive without founding  "
        f"{statistics.median(lifetimes) if lifetimes else 0:.1f}",
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
    city_turns = _sum(rows, "army", "city_turns")
    unit_turns = _sum(rows, "army", "unit_city_turns")
    return [
        "=== the peacetime army and the wall chain  [BEHAVIOUR] ===",
        f"  runs with a peacetime reading        {len(peace)}",
        f"  our military / best rival's, AT PEACE      mean {_mean(peace):.2f}"
        f"  median {statistics.median(peace) if peace else 0:.2f}",
        f"  same, at peace from turn 100 onwards       mean {_mean(late):.2f}",
        f"  same, AT WAR                               mean {_mean(war):.2f}"
        f"  over {len(war)} runs",
        f"  city-turns building a UNIT           {unit_turns:.0f} of {city_turns:.0f}"
        f"  ({100 * unit_turns / city_turns if city_turns else 0:.1f}%)",
        f"  city-turns holding Walls             "
        f"{_sum(rows, 'army', 'held_walls'):.0f}",
        f"  city-turns holding a Castle          "
        f"{_sum(rows, 'army', 'held_castle'):.0f}",
        f"  city-turns holding a Star Fort       "
        f"{_sum(rows, 'army', 'held_star_fort'):.0f}",
    ]


REPORTERS = {
    "great-people": report_great_people,
    "traders": report_traders,
    "restart": report_restart,
    "settlers": report_settlers,
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
                        help=f"score ratio for #2319's rule (default {RESTART_RATIO},"
                             " the operator value; the flag's own default of 0.0"
                             " disables the policy)")
    parser.add_argument("--json", type=Path, help="also write the per-run rows here")
    args = parser.parse_args(argv)

    sections = tuple(args.section or SECTIONS)
    paths = run_dirs(args.corpus, args.since, args.until)
    if not paths:
        print(f"no recorded runs under {args.corpus}", file=sys.stderr)
        return 1

    rows = [run_reading(path, args.restart_ratio, sections) for path in paths]
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
