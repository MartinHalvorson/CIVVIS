"""Detectors for the two failures that ran for a whole night without being noticed.

    python3 tools/civ6_watchdogs.py                 # newest run
    python3 tools/civ6_watchdogs.py --run TAG
    python3 tools/civ6_watchdogs.py --all --json out.jsonl

⚠ THIS FILE EXISTS BECAUSE BOTH FAILURES BELOW ARE INVISIBLE IN EVERY EXISTING
REPORT. `civ6_civvis_status.py` reads green on both: `orders_source: civvis` on
every turn, `applied` in the 90s, `residual: none` — while the army stands in the
capital and the mirror describes a different map than the one on screen. A summary
that cannot go red for a failure is not a check for it.

Both detectors report a NUMERATOR AND A DENOMINATOR, never a boolean, for the reason
this project has now learned four times: a mechanism that works and a mechanism that
destroys itself both read "connected".

## 1. idle stack — units piling up in a city and never being used

The shape: a unit is built, is given no order it can act on, and stands on the city
centre. Civilization VI permits UNLIMITED stacking on a city centre, so the engine
never pushes back and nothing in the event stream complains. Measured as unit-turns,
because a snapshot of one turn cannot tell a garrison from a traffic jam:

  stuck_unit_turns / unit_turns   a unit-turn is stuck when the unit did not change
                                  plot since the previous state AND is standing on
                                  one of our own city centres. Bridge-managed Great
                                  People are counted separately, not in either side.
  worst_stack                     most own units sharing a single city-centre plot.
  frozen_units                    units that never moved at all, over their lifetime,
                                  and lived at least `--frozen-turns` turns.

⚠ Standing still is not by itself a fault — a garrison is supposed to stand still, and
one defender per city is a floor this project deliberately added. That is why the
report separates the first unit on a plot from the surplus, and why `worst_stack` is
printed beside the fraction. Read them together.

## 2. mirror disagreement — CIVVIS's board vs the one Civilization VI exported

The shape: the two sides use different coordinate systems (Civ 6 OFFSET, CIVVIS
AXIAL) and both are pairs of small integers, so a mix-up silently lands on a
different hex. It has already cost this project a capital that had NO TILE in the
reconstruction, and it is the standing explanation for improvement orders that are
re-issued and refused forever.

This half needs CIVVIS's own view, which `civvis_orders --dump-mirror` prints back in
OFFSET coordinates. Given both, the check is tile for tile:

  agree / compared      plots present on both sides that carry the same terrain.
  missing_in_mirror     plots Civ 6 exported that the mirror does not hold AT ALL.
                        ⚠ This is the coordinate bug's signature: not a wrong value,
                        an absent tile.
  disagree              per-field disagreement counts (terrain, hills, water, ...).

A run with `missing_in_mirror` above zero is not a valid measurement of CIVVIS's
judgement: it answered a different map.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"
HERE = Path(__file__).resolve().parent

sys.path.insert(0, str(HERE))
import _common  # noqa: E402
GREAT_PERSON_UNIQUE_TYPES = {"UNIT_COMANDANTE_GENERAL"}
GOVERNOR_BASE_PROMOTIONS = {
    "GOVERNOR_THE_AMBASSADOR": "GOVERNOR_PROMOTION_AMBASSADOR_MESSENGER",
    "GOVERNOR_THE_BUILDER": "GOVERNOR_PROMOTION_BUILDER_GUILDMASTER",
    "GOVERNOR_THE_CARDINAL": "GOVERNOR_PROMOTION_CARDINAL_BISHOP",
    "GOVERNOR_THE_DEFENDER": "GOVERNOR_PROMOTION_REDOUBT",
    "GOVERNOR_THE_EDUCATOR": "GOVERNOR_PROMOTION_EDUCATOR_LIBRARIAN",
    "GOVERNOR_THE_MERCHANT": "GOVERNOR_PROMOTION_MERCHANT_LAND_ACQUISITION",
    "GOVERNOR_THE_RESOURCE_MANAGER": (
        "GOVERNOR_PROMOTION_RESOURCE_MANAGER_GROUNDBREAKER"
    ),
}


def is_bridge_managed_great_person(unit: dict) -> bool:
    """Whether this physical unit uses the Great Person bridge instead of the model."""
    kind = unit.get("kind") or unit.get("type") or ""
    return kind.startswith("UNIT_GREAT_") or kind in GREAT_PERSON_UNIQUE_TYPES


def newest_run() -> Path | None:
    return _common.newest_run(RUN_ROOT, "events.jsonl")


def read_events(run: Path) -> list[dict]:
    out = []
    for line in (run / "events.jsonl").read_text(errors="replace").splitlines():
        try:
            out.append(json.loads(line))
        except ValueError:
            continue
    return out


# --------------------------------------------------------------------------- 1


def dropped_units(run: Path) -> dict:
    """Units the export named that never reached CIVVIS's board, from the decider's note.

    ★★★★★ THIS IS THE DETECTOR FOR THE NIGHT'S WORST FINDING. Six of twenty-one units
    were missing from the reconstruction on run `civvis-20260731T070956Z` — every
    barbarian (the `hostiles` list exports `type` and `StateUnit` read `kind`) and
    every embarked unit (`plant_unit` refused water). A unit the mirror drops gets no
    order and stands where it was built for the rest of the game, and nothing in the
    project's reporting could go red for it: `unmapped` was EMPTY, because an unread
    field is not a translation failure.

    Read from `civvis_notes.jsonl`, which the brain already writes — the decider's own
    per-turn note carries `dropped_units=N [...]` with a reason on each entry.
    """
    notes = run / "civvis_notes.jsonl"
    if not notes.exists():
        return {"checked": False}
    worst, reasons, turns_with = 0, Counter(), 0
    managed_great_people = 0
    for line in notes.read_text(errors="replace").splitlines():
        try:
            note = json.loads(line).get("note", "")
        except ValueError:
            continue
        found = re.search(r"dropped_units=(\d+) \[([^\]]*)\]", note)
        if not found:
            continue
        unmanaged_this_turn = 0
        for entry in found.group(2).split():
            reason = entry.rsplit(":", 1)[-1]
            # Physical Great People intentionally bypass CIVVIS's ordinary unit
            # model. `great_person_orders` still gives them host-validated move and
            # activation orders, including the Prophet-specific religion path. They
            # are bridge-managed, not units that receive no order and freeze.
            if reason == "great_person":
                managed_great_people += 1
                continue
            reasons[reason] += 1
            unmanaged_this_turn += 1
        if unmanaged_this_turn:
            turns_with += 1
            worst = max(worst, unmanaged_this_turn)
    return {
        "checked": True,
        "turns_with_drops": turns_with,
        "worst_on_one_turn": worst,
        "by_reason": dict(reasons.most_common(6)),
        "bridge_managed_great_person_observations": managed_great_people,
    }


def idle_stack(events: list[dict], frozen_turns: int = 20) -> dict:
    """Unit-turns spent motionless on our own city centres.

    Walks `state` events, which carry every own unit with an id and a plot, and every
    own city with its plot. Keyed by unit id across turns.

    ⚠ Unit ids are Civilization VI's, not the mirror's — the mirror reassigns them
    every turn, which is why this reads the EXPORT and not anything CIVVIS produced.
    An id that disappears is a dead unit, not a moved one.
    """
    states = [e for e in events if e.get("kind") == "state"]
    prev: dict[int, tuple[int, int]] = {}
    first_seen: dict[int, int] = {}
    last_seen: dict[int, int] = {}
    # ⚠ NAME THE KIND. "2 of 36 units never moved" is a number nobody can act on;
    # "a TRADER stood still for 113 turns" names a whole capability that does not
    # cross the bridge — Civ 6 trade routes are not actuated, so every trader CIVVIS
    # builds is dead weight for the rest of the game. The count alone read as noise.
    kinds: dict[int, str] = {}
    ever_moved: set[int] = set()
    unit_turns = 0
    stuck_turns = 0
    surplus_turns = 0
    worst_stack = 0
    worst_at: tuple | None = None
    per_turn: list[tuple[int, int, int, int]] = []
    managed_great_people = 0

    for state in states:
        turn = state.get("turn")
        centres = {(c["x"], c["y"]) for c in (state.get("cities") or [])}
        exported_units = state.get("units") or []
        managed_great_people += sum(
            is_bridge_managed_great_person(unit) for unit in exported_units
        )
        units = [
            unit for unit in exported_units
            if not is_bridge_managed_great_person(unit)
        ]
        occupancy: dict[tuple[int, int], list[int]] = defaultdict(list)
        turn_units = 0
        turn_stuck = 0
        for unit in units:
            uid = unit.get("id")
            if uid is None:
                continue
            plot = (unit.get("x"), unit.get("y"))
            kinds[uid] = unit.get("kind", "?")
            first_seen.setdefault(uid, turn)
            last_seen[uid] = turn
            occupancy[plot].append(uid)
            if uid in prev:
                turn_units += 1
                unit_turns += 1
                if prev[uid] != plot:
                    ever_moved.add(uid)
                elif plot in centres:
                    stuck_turns += 1
                    turn_stuck += 1
            prev_plot = prev.get(uid)
            if prev_plot is not None and prev_plot != plot:
                ever_moved.add(uid)
        # Surplus is everything beyond the first unit on a city centre: the first is a
        # garrison, the rest are the pile-up this detector is named for.
        turn_surplus = 0
        for plot, ids in occupancy.items():
            if plot in centres:
                turn_surplus += max(0, len(ids) - 1)
                if len(ids) > worst_stack:
                    worst_stack, worst_at = len(ids), (turn, plot)
        surplus_turns += turn_surplus
        per_turn.append((turn, turn_units, turn_stuck, turn_surplus))
        prev = {u["id"]: (u["x"], u["y"]) for u in units if u.get("id") is not None}

    # ★★★★ HOW FAR THE EMPIRE REACHES. A stack on the city centre is one way an army
    # goes unused; the other is an army that never leaves the neighbourhood, and the
    # idle-stack fraction cannot see it — every unit moves a tile each turn and none of
    # them goes anywhere. It is the failure behind `met` stalling at 1-2 of 3 rivals,
    # which makes domination unreachable no matter how good the siege code is: no
    # contact, no visible rival city, nothing for the army to attack.
    #
    # Measured on run civvis-20260731T075743Z at turn 77: TWELVE units alive, five of
    # them archers, and the FURTHEST from any of our cities was 2 tiles (mean 1.2).
    # ★★★★ CITIES LOST, WHICH CAPS THE EMPIRE EXACTLY AS HARD AS SETTLERS DYING.
    # A ladder that reads "peak 4, final 3" has founded four cities and been unable to
    # HOLD four, and those are different repairs. Recorded with the turn and the plot
    # so the event stream around it can be read.
    lost: list = []
    previous: set | None = None
    for state in states:
        held = {(c["x"], c["y"]) for c in (state.get("cities") or [])}
        if previous is not None:
            for plot in sorted(previous - held):
                lost.append({"turn": state.get("turn"), "plot": plot})
        previous = held

    # ⚠⚠ OVER THE WHOLE RUN, NOT THE LAST TURN — and reading the last turn alone
    # nearly bought me a wrong conclusion tonight. Sampled at the final state the four
    # attempts of 2026-07-31 read 13, 56, 4 and 2 tiles, which looks like a collapse
    # caused by whatever changed between them. It is not: the last turn of a run that
    # is losing units has three units left, all of them at home defending, and a run
    # that ended on a scout abroad reads 56. The number that means something is the
    # furthest ANY unit got at ANY point, beside the last-turn picture.
    def offset_distance(a, b):
        def axial(p):
            col, row = p
            return col - (row - (row & 1)) // 2, row
        (aq, ar), (bq, br) = axial(a), axial(b)
        return max(abs(aq - bq), abs(ar - br), abs((-aq - ar) - (-bq - br)))

    reach = {"furthest": None, "mean": None, "units": 0, "furthest_ever": None,
             "furthest_ever_turn": None}
    ever, ever_turn = None, None
    for state in states:
        centres = [(c["x"], c["y"]) for c in (state.get("cities") or [])]
        units = [
            unit for unit in (state.get("units") or [])
            if not is_bridge_managed_great_person(unit)
        ]
        if not centres or not units:
            continue
        spread = [
            min(offset_distance((u["x"], u["y"]), c) for c in centres) for u in units
        ]
        if ever is None or max(spread) > ever:
            ever, ever_turn = max(spread), state.get("turn")
        if state is states[-1]:
            reach.update({
                "furthest": max(spread),
                "mean": round(sum(spread) / len(spread), 1),
                "units": len(spread),
            })
    reach["furthest_ever"] = ever
    reach["furthest_ever_turn"] = ever_turn
    # How far into the game this judgement is being made. A scout has not had time to
    # get anywhere by turn 26, and a detector that cries wolf on every young run is one
    # people learn to scroll past.
    reach["first_turn"] = states[0].get("turn") if states else None
    reach["last_turn"] = states[-1].get("turn") if states else None
    if isinstance(reach["first_turn"], int) and isinstance(reach["last_turn"], int):
        reach["observed_turn_span"] = reach["last_turn"] - reach["first_turn"]
    else:
        reach["observed_turn_span"] = None

    # ★★★★ OSCILLATION, WHICH THE IDLE FRACTION CANNOT SEE AT ALL. A unit bouncing
    # between two tiles moves every single turn, so it is never "stuck" and never
    # "frozen" — and it is doing exactly as much for the empire as one that stands
    # still. Measured on run `civvis-20260731T092642Z` at turn 93: two warriors and an
    # archer, all embarked, alternating (18,11)/(19,11) and (16,7)/(16,8) for ten turns.
    #
    # The settler version of this cost forty turns of a one-city empire before it was
    # found by hand; the general form gets a number.
    tracks: dict[int, list] = defaultdict(list)
    for state in states:
        for unit in state.get("units") or []:
            if is_bridge_managed_great_person(unit):
                continue
            uid = unit.get("id")
            if uid is not None:
                tracks[uid].append((unit.get("x"), unit.get("y")))
    # ⚠ EVERY WINDOW, NOT JUST THE LAST ONE. A run-level report asks "did any unit
    # ever go in circles", and the tail alone answers "is one doing it right now" —
    # which read ZERO on a run whose warriors had been shuttling ten turns earlier and
    # then escaped. The first version of this checked only `trail[-12:]` and therefore
    # could not see the thing it was written after.
    oscillating = []
    for uid, trail in tracks.items():
        window = None
        for start in range(0, max(1, len(trail) - 11)):
            candidate = trail[start:start + 12]
            if len(candidate) < 8:
                continue
            distinct = set(candidate)
            moved = sum(1 for a, b in zip(candidate, candidate[1:]) if a != b)
            if len(distinct) <= 3 and moved >= len(candidate) - 3:
                window = candidate
                break
        if window is None:
            continue
        # ⚠ THREE TILES, NOT TWO, AND THE NUMBER IS NOT ARBITRARY: it is CIVVIS's own
        # `LIVELOCK_FOOTPRINT`. The first version asked for exactly two and missed the
        # very units it was written for — warriors shuttling (18,11)/(18,12)/(19,11)
        # on run civvis-20260731T092642Z. A loop is a small FOOTPRINT being re-entered,
        # not necessarily a pair.
        oscillating.append((uid, kinds.get(uid, "?"), sorted(set(window))))

    frozen = [
        (uid, kinds.get(uid, "?"), last_seen[uid] - first)
        for uid, first in first_seen.items()
        if uid not in ever_moved and (last_seen[uid] - first) >= frozen_turns
    ]
    return {
        "unit_turns": unit_turns,
        "stuck_unit_turns": stuck_turns,
        "stuck_fraction": round(stuck_turns / unit_turns, 3) if unit_turns else None,
        "surplus_on_centres_unit_turns": surplus_turns,
        "worst_stack": worst_stack,
        "worst_stack_at": worst_at,
        "reach": reach,
        "cities_lost": lost,
        "oscillating": oscillating[:6],
        "oscillating_count": len(oscillating),
        "frozen_units": len(frozen),
        "frozen_by_kind": dict(Counter(kind for _, kind, _ in frozen).most_common()),
        "frozen_worst": sorted(frozen, key=lambda row: -row[2])[:4],
        "units_seen": len(first_seen),
        "bridge_managed_great_person_observations": managed_great_people,
        "per_turn_tail": per_turn[-8:],
    }


# --------------------------------------------------------------------------- 2

def _modelled_improvements() -> set[str]:
    """The improvements CIVVIS models, READ FROM THE RULESET.

    ⚠ THIS WAS A HARDCODED SET AND IT DRIFTED, in both this file and `mirror.rs`.
    CIVVIS models 36 improvements; the two copies listed 16. The twenty dropped
    included `barbarian_camp`, `goody_hut` and `meteor_goody` — exactly the three
    `AdvancedAi` looks for in `invalidates_followers`, so the mirror stored None and
    the AI's own guard was blind to every barbarian camp on the map.

    A checker with its own copy of the thing it checks will eventually disagree with
    it for reasons that have nothing to do with the game. Read the same source.
    """
    try:
        data = json.loads((HERE.parent / "data" / "improvements.json").read_text())
    except (OSError, ValueError):
        return set()
    return set(data)


MODELLED_IMPROVEMENTS = _modelled_improvements()


def _model_names(filename: str) -> set[str]:
    try:
        return set(json.loads((HERE.parent / "data" / filename).read_text()))
    except (OSError, ValueError):
        return set()


MODELLED_DISTRICTS = _model_names("districts.json")
MODELLED_WONDERS = _model_names("wonders.json")
WONDER_ALIASES = {
    "halicarnassus_mausoleum": "mausoleum_at_halicarnassus",
    "statue_liberty": "statue_of_liberty",
    "university_sankore": "university_of_sankore",
}


def model_infrastructure_name(civ6: str, prefix: str, names: set[str]) -> str:
    """Independently resolve the same strict exact/article/unique-prefix contract."""
    base = civ6.removeprefix(prefix).lower()
    if prefix == "BUILDING_":
        alias = WONDER_ALIASES.get(base)
        if alias in names:
            return alias
    if base in names:
        return base
    without_article = base.removeprefix("the_")
    if without_article in names:
        return without_article
    matches = [name for name in names if name.startswith(base + "_")]
    if len(matches) == 1:
        return matches[0]
    # Deliberately cannot equal any actual ruleset name, so an unmodelled Firaxis
    # district is loud instead of being treated as correctly absent.
    return f"@unmapped:{civ6}"


def production_health(events: list[dict], builder_per_city: float = 0.8) -> dict:
    """Did production keep working as the empire and the eras grew.

    ⚠ WRITTEN BECAUSE THREE CONSTANTS FROZE PRODUCTION AND NOTHING COULD GO RED.
    Run `civvis-20260801T065721Z` lost all six cities and the game, and every
    existing report read healthy throughout: orders applied, mirror agreeing,
    `residual` clean. The three faults were each a number that is right for a small
    Ancient empire and silently wrong later:

      ArmyCap 10 UNITS          the army block never fired again past ~10 units, so
                                nothing re-armed through the war that ended the run
      the production floor      its fallbacks (campus project, warrior, slinger) all
                                need a Campus or go OBSOLETE; UNIT_BUILDER never
                                does, so the floor decayed into a builder factory
      MaxProductionPasses 4     the blocker fires once per CITY, so past four cities
                                the budget ran out and later blockers were answered
                                with nothing

    Fixed in #745 and #748. Each metric below is the one that WOULD have caught it,
    as a numerator over a denominator per this file's standing rule — a mechanism
    that works and one that has seized both read "producing".
    """
    at_war: set[int] = set()
    last_cities = 0
    peak_cities = 0
    # ⚠ ALIVE, not built. Civilization VI builders are CONSUMED — three charges and
    # they vanish — so a cumulative build count is not a stock and must not be judged
    # against a stock target. Measured: a healthy run built 7 builders over 90 turns
    # and never had more than TWO alive, which the old check called a 2.9x surplus.
    peak_builders = 0
    for event in events:
        if event.get("kind") != "state":
            continue
        turn = event.get("turn") or 0
        last_cities = len(event.get("cities") or [])
        peak_cities = max(peak_cities, last_cities)
        peak_builders = max(peak_builders, sum(
            1 for unit in (event.get("units") or [])
            if unit.get("kind") == "UNIT_BUILDER"))
        if any(r.get("at_war") for r in (event.get("rivals") or [])):
            at_war.add(turn)

    # The halfway point of the war, so "did it keep arming" is asked of the stretch
    # where losses actually mount rather than of the opening.
    war_midpoint = (min(at_war) + max(at_war)) // 2 if at_war else 0
    floor_builds = floor_builders = builders = 0
    late_war_builds = late_war_military = 0
    for event in events:
        if event.get("kind") != "build" or not event.get("applied"):
            continue
        item = str(event.get("item") or "")
        turn = event.get("turn") or 0
        if event.get("reason") == "floor":
            floor_builds += 1
            if item == "UNIT_BUILDER":
                floor_builders += 1
        if item == "UNIT_BUILDER":
            builders += 1
        # "Did it re-arm while losing" — the question the army cap answered wrongly.
        #
        # ⚠ MEASURED OVER THE SECOND HALF OF THE WAR, NOT THE WHOLE WAR, and the
        # distinction is the whole detector. Run `civvis-20260801T065721Z` built 9
        # military units across 79 war turns, so a "was it ever zero" test reads
        # 9/79 and stays SILENT — while the truth is that all nine came before turn
        # 100 and the last 40 turns of a war being lost produced 14 builders, 2
        # settlers, 1 trader and NOTHING that fights. A detector that cannot go red
        # for the run it was written for is the failure this file exists to stop.
        if turn in at_war and turn >= war_midpoint:
            late_war_builds += 1
            if item.startswith("UNIT_") and item not in (
                "UNIT_BUILDER", "UNIT_SETTLER", "UNIT_TRADER"
            ):
                late_war_military += 1

    # The production-pass budget only bites once the empire outgrows it, so the
    # early game must NOT be averaged in — it dilutes a total failure to a third.
    prod_late = prod_late_unanswered = 0
    for event in events:
        if (event.get("kind") != "blocked"
                or event.get("blocker") != "ENDTURN_BLOCKING_PRODUCTION"):
            continue
        if (event.get("turn") or 0) < 100:
            continue
        prod_late += 1
        if event.get("answered") is None:
            prod_late_unanswered += 1

    target = max(1.0, peak_cities * builder_per_city)
    return {
        "floor_builds": floor_builds,
        "floor_builder_builds": floor_builders,
        "floor_builder_fraction": (floor_builders / floor_builds) if floor_builds else None,
        # Kept for context — it is what production SPENT — but never the verdict.
        "builders_built": builders,
        "builder_target": round(target, 1),
        "peak_builders_alive": peak_builders,
        "builder_overshoot": round(peak_builders / target, 1) if target else None,
        "peak_cities": peak_cities,
        "war_turns": len(at_war),
        "war_midpoint": war_midpoint,
        "builds_late_war": late_war_builds,
        "military_builds_late_war": late_war_military,
        "prod_blocks_after_t100": prod_late,
        "prod_blocks_after_t100_unanswered": prod_late_unanswered,
    }


def load_vocab() -> dict:
    """The SAME table the mirror builds from, so the diff compares like with like.

    ⚠ Terrain, feature and resource names are written into the mirror THROUGH this
    table, so those three columns are only a check on the translation being applied
    at all — they cannot catch a wrong table. What they do catch, and what has
    actually bitten: a name absent from the table, which does not error and leaves
    the tile carrying whatever `Game::new` generated.

    The columns that are genuinely independent are `w` (Civ 6 answers `IsWater()`;
    CIVVIS derives water from its own ruleset) and the presence of the tile at all.
    """
    return json.loads((HERE / "civ6_control" / "vocab.json").read_text())


def expected(plot: dict, vocab: dict) -> dict:
    """What the mirror SHOULD hold for a plot Civilization VI exported."""
    terrain = vocab["terrains"].get(plot.get("t") or "")
    feature = vocab["features"].get(plot.get("f") or "") if plot.get("f") else None
    resource = vocab["resources"].get(plot.get("r") or "") if plot.get("r") else None
    improvement = None
    if plot.get("im"):
        short = plot["im"].removeprefix("IMPROVEMENT_").lower()
        improvement = short if short in MODELLED_IMPROVEMENTS else None
    return {
        "t": terrain["terrain"] if terrain else None,
        "h": terrain["hills"] if terrain else None,
        "w": plot.get("w"),
        "f": feature,
        "r": resource,
        "im": improvement,
        "own": plot.get("o") == 0,
    }


# Fields compared after translation. `t`/`h`/`f`/`r` go through the vocabulary; `w`
# and `own` are answered independently by each side.
COMPARED = ("t", "h", "w", "f", "r", "im", "own")


def expected_infrastructure(events: list[dict]) -> dict[tuple[int, int], dict]:
    """District/wonder occupancy from the newest authoritative city roster."""
    state = None
    best_turn = -1
    for event in events:
        if event.get("kind") != "state":
            continue
        turn = int(event.get("turn") or 0)
        if turn >= best_turn:
            state, best_turn = event, turn
    if state is None:
        return {}

    cities = list(state.get("cities") or [])
    for actor in list(state.get("rivals") or []) + list(state.get("minors") or []):
        cities.extend(actor.get("cities") or [])
    expected: dict[tuple[int, int], dict] = {}
    for city in cities:
        for district in city.get("districts") or []:
            kind = str(district.get("type") or "").upper()
            if kind in ("DISTRICT_CITY_CENTER", "DISTRICT_WONDER"):
                continue
            key = (district.get("x"), district.get("y"))
            complete = district.get("complete", True)
            name = model_infrastructure_name(kind, "DISTRICT_", MODELLED_DISTRICTS)
            expected[key] = {
                "district": name if complete else None,
                "foundation": name if not complete else None,
                "wonder": None,
                "pillaged": bool(district.get("pillaged")) if complete else False,
            }
        for wonder in city.get("wonders") or []:
            key = (wonder.get("x"), wonder.get("y"))
            kind = str(wonder.get("type") or "").upper()
            expected[key] = {
                "district": None,
                "foundation": None,
                "wonder": model_infrastructure_name(kind, "BUILDING_", MODELLED_WONDERS),
                "pillaged": False,
            }
    return expected


def latest_tiles(events: list[dict]) -> dict[tuple[int, int], dict]:
    """The most recent export of every plot, merged across chunks and re-exports.

    ⚠ `tiles` arrives in chunks and the map is re-exported every few turns, so a
    single event is a fragment. Later exports win: a plot's terrain can legitimately
    change (a chop, an improvement), and the mirror is rebuilt from the same stream.
    """
    plots: dict[tuple[int, int], dict] = {}
    for event in events:
        if event.get("kind") != "tiles":
            continue
        for plot in event.get("plots") or []:
            plots[(plot.get("x"), plot.get("y"))] = plot
    return plots


GREAT_WORK_OBJECTS = {
    "GREATWORKOBJECT_WRITING": "writing",
    "GREATWORKOBJECT_LANDSCAPE": "art",
    "GREATWORKOBJECT_PORTRAIT": "art",
    "GREATWORKOBJECT_SCULPTURE": "art",
    "GREATWORKOBJECT_RELIGIOUS": "religious_art",
    "GREATWORKOBJECT_ARTIFACT": "artifact",
    "GREATWORKOBJECT_MUSIC": "music",
    "GREATWORKOBJECT_RELIC": "relic",
}


def city_economy_agreement(events: list[dict], dump: dict) -> dict:
    """Diff exact private city facts independently of the tile comparison."""
    states = [event for event in events if event.get("kind") == "state"]
    if not states:
        return {
            "compared": 0, "agree": 0,
            "great_work_compared": 0, "great_work_agree": 0,
            "empire_compared": 0, "empire_agree": 0,
            "disagree_by_field": {}, "examples": {}, "max_model_yield_drift": 0.0,
        }
    state = states[-1]
    expected = {
        (city.get("x"), city.get("y")): city
        for city in state.get("cities") or []
        if isinstance(city.get("yields"), dict)
    }
    mirrored = {(city.get("x"), city.get("y")): city for city in dump.get("cities") or []}
    fields: Counter = Counter()
    examples: dict[str, list] = defaultdict(list)
    agree = 0
    yield_names = ("food", "production", "gold", "science", "culture", "faith")
    max_model_drift = 0.0
    for key, want in expected.items():
        got = mirrored.get(key)
        bad = []
        if got is None:
            bad.append("missing_city")
        else:
            for name in yield_names:
                a = (want.get("yields") or {}).get(name)
                b = (got.get("yields") or {}).get(name)
                if a is None or b is None or abs(float(a) - float(b)) > 1e-6:
                    bad.append(f"yield_{name}")
                model = (got.get("model_yields") or {}).get(name)
                if a is not None and model is not None:
                    max_model_drift = max(max_model_drift, abs(float(a) - float(model)))
            if want.get("worked") is not None:
                # GetWorkedPlots includes the city centre; CIVVIS accounts for
                # that tile intrinsically and dumps only citizen assignments.
                # It also includes every District plot a specialist staffs
                # (`IsPlotWorked` is true for a staffed Campus); those are the
                # `specialists` compared below, and the mirror deliberately
                # keeps them out of its tile list — importing them as tiles
                # paid each specialist twice (yield-fidelity work, 2026-08-16).
                district_plots = {
                    (district.get("x"), district.get("y"))
                    for district in want.get("districts") or []
                }
                a = {
                    (plot.get("x"), plot.get("y"))
                    for plot in want.get("worked") or []
                    if (plot.get("x"), plot.get("y")) != key
                    and (plot.get("x"), plot.get("y")) not in district_plots
                }
                b = {(plot.get("x"), plot.get("y")) for plot in got.get("worked") or []}
                if a != b:
                    bad.append("worked")
            if want.get("specialists") is not None:
                # Firaxis names district types while the dump uses CIVVIS family names.
                a = Counter(
                    model_infrastructure_name(value, "DISTRICT_", MODELLED_DISTRICTS)
                    for value in want.get("specialists") or []
                )
                b = Counter(got.get("specialists") or [])
                if a != b:
                    bad.append("specialists")
            progress = want.get("production_progress")
            if progress is not None and (
                got.get("production_progress") is None
                or abs(float(progress) - float(got["production_progress"])) > 1e-6
            ):
                bad.append("production_progress")
        if bad:
            for field in bad:
                fields[field] += 1
                if len(examples[field]) < 3:
                    examples[field].append({"xy": key, "civ6": want, "mirror": got})
        else:
            agree += 1

    great_work_compared = 0
    great_work_agree = 0
    cities = state.get("cities") or []
    if cities and all(city.get("great_works") is not None for city in cities):
        wanted_works: Counter = Counter()
        for city in cities:
            for work in city.get("great_works") or []:
                wanted_works[GREAT_WORK_OBJECTS.get(work.get("object"), work.get("object"))] += 1
        got_works = Counter({k: int(v) for k, v in (dump.get("great_works") or {}).items()})
        for kind in set(wanted_works) | set(got_works):
            great_work_compared += 1
            if wanted_works[kind] == got_works[kind]:
                great_work_agree += 1
            else:
                fields["great_works"] += 1
                if len(examples["great_works"]) < 3:
                    examples["great_works"].append({
                        "kind": kind, "civ6": wanted_works[kind], "mirror": got_works[kind],
                    })
    empire_compared = 0
    empire_agree = 0
    for name in ("science", "culture"):
        want = state.get(name)
        got = (dump.get("empire_yields") or {}).get(name)
        if want is None or float(want) < 0:
            continue
        empire_compared += 1
        if got is not None and abs(float(want) - float(got)) <= 1e-6:
            empire_agree += 1
        else:
            fields[f"empire_{name}"] += 1
            examples[f"empire_{name}"].append({"civ6": want, "mirror": got})
    return {
        "compared": len(expected),
        "agree": agree,
        "great_work_compared": great_work_compared,
        "great_work_agree": great_work_agree,
        "empire_compared": empire_compared,
        "empire_agree": empire_agree,
        "disagree_by_field": dict(fields.most_common()),
        "examples": dict(examples),
        "max_model_yield_drift": round(max_model_drift, 6),
    }


def governor_agreement(events: list[dict], dump: dict) -> dict:
    """Diff the exact Governor roster, assignments, promotions and title ledger."""
    states = [event for event in events if event.get("kind") == "state"]
    if not states or "governors" not in states[-1]:
        return {"compared": 0, "agree": 0, "disagree_by_field": {}, "examples": {}}
    state = states[-1]
    wanted = {
        governor.get("type"): governor
        for governor in state.get("governors") or []
        if governor.get("type")
    }
    mirrored = {
        governor.get("type"): governor
        for governor in dump.get("governors") or []
        if governor.get("type")
    }
    fields: Counter = Counter()
    examples: dict[str, list] = defaultdict(list)
    agree = 0
    compared = 0

    for governor_type in sorted(set(wanted) | set(mirrored)):
        compared += 1
        expected_governor = wanted.get(governor_type)
        actual_governor = mirrored.get(governor_type)
        bad = []
        if expected_governor is None:
            bad.append("extra_governor")
        elif actual_governor is None:
            bad.append("missing_governor")
        else:
            if (expected_governor.get("x", -1), expected_governor.get("y", -1)) != (
                actual_governor.get("x", -1), actual_governor.get("y", -1)
            ):
                bad.append("assignment")
            if bool(expected_governor.get("established")) != bool(
                actual_governor.get("established")
            ):
                bad.append("established")
            if (int(expected_governor.get("neutralized_turns") or 0) > 0) != bool(
                actual_governor.get("neutralized")
            ):
                bad.append("neutralized")
            expected_promotions = set(expected_governor.get("promotions") or [])
            # Firaxis includes the free appointment ability in GetPromotions;
            # CIVVIS represents it on the Governor specification, not as one of
            # the five separately purchased promotions.
            expected_promotions.discard(GOVERNOR_BASE_PROMOTIONS.get(governor_type))
            if expected_promotions != set(actual_governor.get("promotions") or []):
                bad.append("promotions")
        if bad:
            for field in bad:
                fields[field] += 1
                if len(examples[field]) < 3:
                    examples[field].append({
                        "type": governor_type,
                        "civ6": expected_governor,
                        "mirror": actual_governor,
                    })
        else:
            agree += 1

    point_fields = (
        ("governor_points", "titles"),
        ("governor_points_spent", "titles_spent"),
    )
    for source, field in point_fields:
        value = state.get(source)
        if value is None or int(value) < 0:
            continue
        compared += 1
        if dump.get(source) is not None and int(dump[source]) == int(value):
            agree += 1
        else:
            fields[field] += 1
            examples[field].append({"civ6": value, "mirror": dump.get(source)})
    if state.get("governor_points") is not None and state.get("governor_points_spent") is not None:
        expected_available = max(
            0, int(state["governor_points"]) - int(state["governor_points_spent"])
        )
        compared += 1
        if int(dump.get("governor_points_available", -1)) == expected_available:
            agree += 1
        else:
            fields["titles_available"] += 1
            examples["titles_available"].append({
                "civ6": expected_available,
                "mirror": dump.get("governor_points_available"),
            })
    return {
        "compared": compared,
        "agree": agree,
        "disagree_by_field": dict(fields.most_common()),
        "examples": dict(examples),
    }


def mirror_agreement(run: Path, events: list[dict], orders_bin: Path) -> dict:
    """Ask CIVVIS for its board in OFFSET coordinates and diff it against the export."""
    exported = latest_tiles(events)
    if not exported:
        return {"error": "run exported no tiles"}
    try:
        proc = subprocess.run(
            [str(orders_bin), "--mirror", str(run), "--dump-mirror"],
            capture_output=True, text=True, timeout=300,
        )
    except (subprocess.SubprocessError, OSError) as exc:
        return {"error": f"could not run {orders_bin}: {exc}"}
    if proc.returncode != 0:
        return {"error": f"dump-mirror exited {proc.returncode}: {proc.stderr[-400:]}"}
    try:
        dump = json.loads(proc.stdout)
    except ValueError:
        return {"error": f"dump-mirror printed no JSON: {proc.stdout[:200]!r}"}

    vocab = load_vocab()
    mirrored = {(p["x"], p["y"]): p for p in dump.get("plots", [])}
    missing = [k for k in exported if k not in mirrored]
    # Tiles the mirror holds that Civ 6 never exported are the deliberate land
    # frontier — the ring of invented land at the edge of what the seat has revealed,
    # which exists because unrevealed ground otherwise reads as ocean and the empire
    # looks like an island. Reported, never counted as a disagreement.
    extra = [k for k in mirrored if k not in exported]
    disagree: Counter = Counter()
    examples: dict[str, list] = defaultdict(list)
    unmodelled_improvements: Counter = Counter()
    compared = 0
    agree = 0
    for key, plot in exported.items():
        got = mirrored.get(key)
        if got is None:
            continue
        compared += 1
        want = expected(plot, vocab)
        if plot.get("im") and want["im"] is None:
            unmodelled_improvements[plot["im"]] += 1
        bad = []
        for field in COMPARED:
            a, b = want.get(field), got.get(field)
            if a != b:
                bad.append(field)
                disagree[field] += 1
                if len(examples[field]) < 3:
                    examples[field].append(
                        {"xy": key, "civ6": plot.get(field if field != "own" else "o"),
                         "expected": a, "mirror": b})
        if not bad:
            agree += 1

    expected_infra = expected_infrastructure(events)
    infrastructure_fields = Counter()
    infrastructure_examples: dict[str, list] = defaultdict(list)
    infrastructure_agree = 0
    # Public rival city records omit infrastructure. Compare every authoritative
    # current record, but do not call remembered rival districts "ghosts" merely
    # because the latest fog-limited roster cannot repeat them.
    infrastructure_keys = set(expected_infra)
    for key in infrastructure_keys:
        got = mirrored.get(key) or {}
        want = expected_infra.get(key) or {
            "district": None,
            "foundation": None,
            "wonder": None,
            "pillaged": False,
        }
        actual = {
            "district": got.get("d"),
            "foundation": got.get("df"),
            "wonder": got.get("wo"),
            "pillaged": bool(got.get("p")),
        }
        bad = []
        for field in ("district", "foundation", "wonder"):
            if want[field] != actual[field]:
                bad.append(field)
        if want["district"] is not None and want["pillaged"] != actual["pillaged"]:
            bad.append("district_pillaged")
        if bad:
            for field in bad:
                infrastructure_fields[field] += 1
                if len(infrastructure_examples[field]) < 3:
                    infrastructure_examples[field].append({
                        "xy": key, "expected": want, "mirror": actual,
                        "mirror_types": {
                            "district": got.get("d"),
                            "foundation": got.get("df"),
                            "wonder": got.get("wo"),
                        },
                    })
        else:
            infrastructure_agree += 1
    economy = city_economy_agreement(events, dump)
    governors = governor_agreement(events, dump)
    return {
        "exported_plots": len(exported),
        "mirrored_plots": len(mirrored),
        "compared": compared,
        "agree": agree,
        "agree_fraction": round(agree / compared, 4) if compared else None,
        "missing_in_mirror": len(missing),
        "missing_examples": missing[:6],
        "extra_in_mirror_frontier": len(extra),
        "unresolved_terrain": dump.get("unresolved_terrain") or {},
        "unmodelled_improvements": dict(unmodelled_improvements.most_common(8)),
        "disagree_by_field": dict(disagree.most_common()),
        "examples": {k: v for k, v in examples.items()},
        "infrastructure_compared": len(infrastructure_keys),
        "infrastructure_agree": infrastructure_agree,
        "infrastructure_disagree_by_field": dict(infrastructure_fields.most_common()),
        "infrastructure_examples": dict(infrastructure_examples),
        "city_economy_compared": economy["compared"],
        "city_economy_agree": economy["agree"],
        "great_work_compared": economy["great_work_compared"],
        "great_work_agree": economy["great_work_agree"],
        "empire_economy_compared": economy["empire_compared"],
        "empire_economy_agree": economy["empire_agree"],
        "city_economy_disagree_by_field": economy["disagree_by_field"],
        "city_economy_examples": economy["examples"],
        "max_model_yield_drift": economy["max_model_yield_drift"],
        "governor_compared": governors["compared"],
        "governor_agree": governors["agree"],
        "governor_disagree_by_field": governors["disagree_by_field"],
        "governor_examples": governors["examples"],
    }


# ---------------------------------------------------------------------------


def verdicts(report: dict, stuck_max: float, agree_min: float) -> list[str]:
    """The loud half. A detector nobody reads is the failure it was written for."""
    out = []
    idle = report.get("idle_stack") or {}
    frac = idle.get("stuck_fraction")
    if frac is not None and frac > stuck_max:
        out.append(
            f"IDLE STACK: {idle['stuck_unit_turns']}/{idle['unit_turns']} unit-turns "
            f"({frac:.0%}) motionless on a city centre, worst stack "
            f"{idle['worst_stack']} at {idle['worst_stack_at']}")
    drops = report.get("dropped_units") or {}
    if drops.get("checked") and drops.get("turns_with_drops"):
        out.append(
            f"UNITS MISSING FROM CIVVIS'S BOARD: on {drops['turns_with_drops']} turns, "
            f"worst {drops['worst_on_one_turn']} at once, by reason "
            f"{drops['by_reason']}. A unit the mirror drops gets no order and stands "
            f"where it was built.")
    if idle.get("cities_lost"):
        out.append(
            f"CITIES LOST: {len(idle['cities_lost'])} — {idle['cities_lost']}. "
            f"Founding a city and holding it are different problems and the peak "
            f"count hides the second one.")
    reach = idle.get("reach") or {}
    if (reach.get("furthest_ever") is not None
            and reach["furthest_ever"] <= 8
            and (reach.get("last_turn") or 0) >= 60
            and (reach.get("observed_turn_span") or 0) >= 40):
        out.append(
            f"THE EMPIRE NEVER REACHED: the furthest any unit ever got from one of our "
            f"cities was {reach['furthest_ever']} tiles, at turn "
            f"{reach['furthest_ever_turn']}. Nothing went looking for anybody — this is "
            f"what makes `met` stall and domination unreachable.")
    if idle.get("oscillating_count"):
        out.append(
            f"UNITS GOING IN CIRCLES: {idle['oscillating_count']} — "
            f"{idle['oscillating']}. They move every turn, so neither the idle "
            f"fraction nor the frozen count can see them, and they are doing as much "
            f"for the empire as a unit that stands still.")
    if idle.get("frozen_units"):
        out.append(
            f"FROZEN UNITS: {idle['frozen_units']} of {idle['units_seen']} units never "
            f"moved once in their whole life — {idle.get('frozen_by_kind')} "
            f"(longest: {idle.get('frozen_worst')})")
    prod = report.get("production_health") or {}
    ff = prod.get("floor_builder_fraction")
    if ff is not None and ff >= 0.8 and (prod.get("floor_builds") or 0) >= 5:
        out.append(
            f"THE PRODUCTION FLOOR HAS DECAYED: {prod['floor_builder_builds']}/"
            f"{prod['floor_builds']} ({ff:.0%}) of floor builds are UNIT_BUILDER. Its "
            f"other entries need a Campus or have gone obsolete, so the fallback that "
            f"exists to stop an empty queue is now the only thing choosing.")
    over = prod.get("builder_overshoot")
    if over is not None and over >= 2.0:
        out.append(
            f"BUILDER SURPLUS: {prod['peak_builders_alive']} builders ALIVE AT ONCE "
            f"against a target of {prod['builder_target']} for {prod['peak_cities']} "
            f"cities ({over}x); {prod['builders_built']} built in total. Builders are "
            f"what production falls back to when nothing else is playable, so a "
            f"standing surplus counts turns nothing better was chosen.")
    late = prod.get("prod_blocks_after_t100") or 0
    unans = prod.get("prod_blocks_after_t100_unanswered") or 0
    # ⚠ A MINIMUM DENOMINATOR, like every other check here has. Without it a single
    # blocked event reads "1/1 (100%) unanswered" and shouts — which is exactly what
    # a run that died early looks like, and exactly when this report is being read
    # most carefully. The other three verdicts guard on `floor_builds >= 5`,
    # `overshoot >= 2.0` and `>= 20` war turns with `>= 5` late-war builds; this one
    # was the odd case out. Eight is the smallest sample that can distinguish a real
    # budget exhaustion (which runs at ~50% on a six-city empire) from noise.
    if late >= 8 and unans:
        out.append(
            f"PRODUCTION BLOCKED AND UNANSWERED: {unans}/{late} of the "
            f"ENDTURN_BLOCKING_PRODUCTION blocks after turn 100 got no answer. It "
            f"fires once per city, so this is the pass budget running out before the "
            f"empire is served — it reads as 'nothing was buildable' and is not that.")
    if ((prod.get("war_turns") or 0) >= 20
            and (prod.get("builds_late_war") or 0) >= 5
            and prod.get("military_builds_late_war") == 0):
        out.append(
            f"NOTHING RE-ARMED: {prod['builds_late_war']} things built after turn "
            f"{prod['war_midpoint']} — the second half of a {prod['war_turns']}-turn "
            f"war — and NOT ONE was a military unit. An army that cannot replace its "
            f"losses loses the war it is in.")
    mirror = report.get("mirror") or {}
    if mirror.get("error"):
        out.append(f"MIRROR NOT CHECKED: {mirror['error']}")
    else:
        if mirror.get("missing_in_mirror"):
            out.append(
                f"MIRROR MISSING TILES: {mirror['missing_in_mirror']} plots Civ 6 "
                f"exported have no tile in CIVVIS's board "
                f"(e.g. {mirror.get('missing_examples')}) — the coordinate bug's signature")
        af = mirror.get("agree_fraction")
        if af is not None and af < agree_min:
            out.append(
                f"MIRROR DISAGREES: only {mirror['agree']}/{mirror['compared']} "
                f"({af:.1%}) of plots match tile for tile; "
                f"{mirror.get('disagree_by_field')}")
        if mirror.get("infrastructure_disagree_by_field"):
            out.append(
                "MIRROR INFRASTRUCTURE DISAGREES: "
                f"{mirror['infrastructure_agree']}/"
                f"{mirror['infrastructure_compared']} district/foundation/wonder "
                "plots agree; "
                f"{mirror['infrastructure_disagree_by_field']} "
                f"{mirror.get('infrastructure_examples')}")
        if mirror.get("city_economy_disagree_by_field"):
            out.append(
                "MIRROR CITY ECONOMY DISAGREES: "
                f"{mirror['city_economy_agree']}/{mirror['city_economy_compared']} "
                "cities and "
                f"{mirror['great_work_agree']}/{mirror['great_work_compared']} Great Work "
                "counts and "
                f"{mirror.get('empire_economy_agree', 0)}/"
                f"{mirror.get('empire_economy_compared', 0)} "
                "empire rates agree; "
                f"{mirror['city_economy_disagree_by_field']} "
                f"{mirror.get('city_economy_examples')}")
        if mirror.get("governor_disagree_by_field"):
            out.append(
                "MIRROR GOVERNORS DISAGREE: "
                f"{mirror['governor_agree']}/{mirror['governor_compared']} roster/title "
                "facts agree; "
                f"{mirror['governor_disagree_by_field']} "
                f"{mirror.get('governor_examples')}")
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", default=None)
    ap.add_argument("--all", action="store_true", help="every run that has events")
    ap.add_argument("--json", default=None, help="append one JSON object per run here")
    ap.add_argument("--no-mirror", action="store_true",
                    help="skip the tile diff (it rebuilds the board and costs seconds)")
    ap.add_argument("--frozen-turns", type=int, default=20)
    ap.add_argument("--stuck-max", type=float, default=0.35,
                    help="fraction of unit-turns motionless on a city centre before it is loud")
    ap.add_argument("--agree-min", type=float, default=0.98)
    ap.add_argument("--orders-bin",
                    default=str(HERE.parent / "target" / "release" / "civvis_orders"))
    args = ap.parse_args()

    if args.all:
        runs = sorted((p for p in RUN_ROOT.iterdir() if (p / "events.jsonl").exists()),
                      key=lambda p: p.name)
    elif args.run:
        runs = [RUN_ROOT / args.run]
    else:
        one = newest_run()
        if one is None:
            print("no run with events found")
            return 2
        runs = [one]

    handle = open(args.json, "a") if args.json else None
    bad = 0
    for run in runs:
        if not (run / "events.jsonl").exists():
            print(f"{run.name}: no events")
            continue
        events = read_events(run)
        report = {
            "run": run.name,
            "dropped_units": dropped_units(run),
            # How much stream this verdict was formed on, so a check taken while the
            # run was still playing can be told apart from the final one.
            "events_bytes": (run / "events.jsonl").stat().st_size,
            "idle_stack": idle_stack(events, args.frozen_turns),
            "production_health": production_health(events),
        }
        if not args.no_mirror:
            report["mirror"] = mirror_agreement(run, events, Path(args.orders_bin))
        found = verdicts(report, args.stuck_max, args.agree_min)
        report["verdicts"] = found
        if handle:
            handle.write(json.dumps(report, sort_keys=True, default=str) + "\n")
        idle = report["idle_stack"]
        print(f"{run.name}")
        reach = idle.get("reach") or {}
        print(f"  reach: furthest EVER {reach.get('furthest_ever')} tiles "
              f"(at t{reach.get('furthest_ever_turn')}); at the last turn "
              f"{reach.get('furthest')} furthest / {reach.get('mean')} mean "
              f"over {reach.get('units')} units; observed "
              f"{reach.get('observed_turn_span')} turns")
        print(f"  idle: stuck {idle['stuck_unit_turns']}/{idle['unit_turns']} unit-turns"
              f" ({idle['stuck_fraction']})  surplus-on-centres "
              f"{idle['surplus_on_centres_unit_turns']}  worst stack {idle['worst_stack']}"
              f" at {idle['worst_stack_at']}  frozen {idle['frozen_units']}/{idle['units_seen']}")
        prod = report["production_health"]
        print(f"  production: floor {prod['floor_builder_builds']}/{prod['floor_builds']}"
              f" builders  built {prod['builders_built']}/{prod['builder_target']} target"
              f"  late-war military {prod['military_builds_late_war']}/{prod['builds_late_war']}"
              f"  late blocks unanswered {prod['prod_blocks_after_t100_unanswered']}"
              f"/{prod['prod_blocks_after_t100']}")
        managed_people = max(
            report["dropped_units"].get(
                "bridge_managed_great_person_observations", 0
            ),
            idle.get("bridge_managed_great_person_observations", 0),
        )
        if managed_people:
            print(f"  bridge: {managed_people} physical Great Person observation(s) "
                  "handled outside the ordinary unit model")
        if "mirror" in report:
            mirror = report["mirror"]
            if mirror.get("error"):
                print(f"  mirror: {mirror['error']}")
            else:
                print(f"  mirror: agree {mirror['agree']}/{mirror['compared']} "
                      f"({mirror['agree_fraction']})  missing {mirror['missing_in_mirror']}"
                      f"  frontier {mirror['extra_in_mirror_frontier']}  "
                      f"{mirror['disagree_by_field']}")
                print(f"    infrastructure: {mirror['infrastructure_agree']}/"
                      f"{mirror['infrastructure_compared']}  "
                      f"{mirror['infrastructure_disagree_by_field']}")
                if (mirror.get("city_economy_compared") or mirror.get("great_work_compared")
                        or mirror.get("empire_economy_compared")):
                    print(f"    city economy: {mirror['city_economy_agree']}/"
                          f"{mirror['city_economy_compared']} cities, "
                          f"Great Works {mirror['great_work_agree']}/"
                          f"{mirror['great_work_compared']}, empire rates "
                          f"{mirror.get('empire_economy_agree', 0)}/"
                          f"{mirror.get('empire_economy_compared', 0)}, raw max yield drift "
                          f"{mirror['max_model_yield_drift']}  "
                          f"{mirror['city_economy_disagree_by_field']}")
                if mirror.get("governor_compared"):
                    print(f"    governors: {mirror['governor_agree']}/"
                          f"{mirror['governor_compared']} roster/title facts  "
                          f"{mirror['governor_disagree_by_field']}")
                if mirror.get("unresolved_terrain"):
                    print(f"    unresolved terrain names: {mirror['unresolved_terrain']}")
                if mirror.get("unmodelled_improvements"):
                    print(f"    improvements CIVVIS does not model (tile reads "
                          f"unimproved): {mirror['unmodelled_improvements']}")
                for field, cases in (mirror.get("examples") or {}).items():
                    print(f"    {field}: {cases}")
        for line in found:
            print(f"  ⚠ {line}")
            bad += 1
    if handle:
        handle.close()
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
