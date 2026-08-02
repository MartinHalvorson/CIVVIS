#!/usr/bin/env python3
"""Does the CIVVIS board actually agree with the Civilization VI export?

One command. Every check here exists because the corresponding field was once
populated, plausible and WRONG -- so nothing below asks "is it non-empty", it
asks "does it AGREE".

    python3 tools/civ6_mirror_check.py [run-dir]        # newest run if omitted

Requires the mirror server on :8610 (see civvis-civ6-mirror/follow.py).

## What each line is guarding, and what it caught

- FOG     every exported plot must be ON the board. Remembered ground used to be
          dropped outright rather than dimmed: 2 of 6 charted plots survived.
- SETUP   Civ VI's seat settings must be the board's settings. A mirror that says
          Prince/Pangaea beside a Settler/Continents game is wrong before its first move.
- RIVERS  a Civilization VI river plot is fresh water BY DEFINITION, so CIVVIS's
          river tiles must be `fw` in the export far above the base rate. Read
          36.4% against a 25.7% base -- chance -- while the board showed the
          GENERATED map's rivers.
- LAND    every land plot the export names a continent for must carry one. Read
          200 of 776, with 336 WATER tiles carrying one they should not have.
- CLIFFS  Civilization VI exposes no cliff accessor, so any cliff on the board
          was invented by the map generator. Must be zero.
- CITIES  compare the SETS and name what is missing, never the counts.
- UNITS   likewise. "21 exported, 15 reconstructed" once read as healthy because
          nothing compared them.
- HOSTILES every hostile the seat can SEE must be on the board. FOG-GATED, and one
          direction only: the export's threat list is not fog-gated and the board is
          the seated view, so an unseen hostile is correctly absent.
- TREASURY gold and faith are BALANCES (`GetGoldBalance`, `GetFaithBalance`), not
          the per-turn rates `economy_drift` compares. Same turn, or the delta is
          just income.

## Six ways this checker itself cried wolf

Each of these is a real bug I nearly reported, caught only by looking again:

1. It compared the LATEST export against a board published up to 30s earlier and
   called the game's own progress "13 exported plots dropped". The export is now
   bounded to the board's turn -- see `load_export(upto=)`.
2. It judged rivers on an absolute share and called a 4.3x lift over the base
   rate "no better than chance". Judge the LIFT.
3. It asserted water carries no continent. Civilization VI really does put COAST
   tiles on a continent -- 17 read CONTINENT_AUSTRALIA on one run -- so the check
   is agreement with the export, not a rule of its own.
4. TREASURY, read against the NEWEST export rather than the board's own turn,
   showed `gold 176 vs 167` and `faith 23 vs 21` -- a confident 5% shortfall that
   was one turn of income at +9 gold and +2 faith. Bounded, the same instant read
   134/134 and 27/27. That is defect 1 again, in a new check, which is why the
   note below its `latest_state` says to bound every future reader.
5. HOSTILES, on its very first run, printed `export 0  board 1` as though those
   were a matched pair. They are not: the board's non-seat units include rivals
   and city-states, the export's `hostiles` is only the threat list. The count
   comparison was removed the same minute it was written -- a line that invites a
   false reading is worse than no line.
6. HOSTILES again, one run later: it reported `8 exported, 5 NOT on the board` on
   a healthy game. `hostiles` is the planner's threat list and is NOT fog-gated;
   the board is the SEATED view. The check was asking the board to hold units the
   seat cannot see. It is now gated on `visible`, and the decider's own
   `dropped_units` -- which recorded no hostile dropped -- was what disproved it.

⚠ The board served on :8610 is follow.py's FLIPPED staged copy:
`board_axial = offset_to_axial(x, TOP - y)`. The flip constant is discovered here
rather than assumed, because comparing two coordinate frames without first
proving they overlap has already produced one confident, wrong finding.
"""

import argparse
import glob
import json
import os
import subprocess
import sys
import time
import urllib.request
from collections import Counter
from pathlib import Path

from civ6_fidelity import ALIASES as IDENTIFIER_ALIASES

# The same root `civ6_civvis_climb.py` writes to, resolved from $HOME rather than
# hardcoded so this works on any machine that runs the ladder.
RUNS = str(Path.home() / "civvis-civ6-runs" / "control")
PORT = int(os.environ.get("CIVVIS_MIRROR_PORT", "8610"))
VOCABULARY = json.loads(
    (Path(__file__).resolve().parent / "civ6_control" / "vocab.json").read_text()
)
MIRRORED_IMPROVEMENTS = set(json.loads(
    (Path(__file__).resolve().parent.parent / "data" / "improvements.json").read_text()
))
MIRRORED_WONDERS = set(json.loads(
    (Path(__file__).resolve().parent.parent / "data" / "wonders.json").read_text()
))
UNIT_MODEL_FALLBACKS = {
    # Firaxis's barbarian Horse Archer shares the modeled Saka role; the host
    # implementation prefix is removed before this table is consulted.
    "horse_archer": "saka_horse_archer",
    # Exact stock roles from Firaxis's UnitReplaces table. CIVVIS does not yet
    # carry these unique specifications, but it must not erase the visible unit.
    "scottish_highlander": "ranger",
    "korean_hwacha": "field_cannon",
}
RESOURCE_RULES = json.loads(
    (Path(__file__).resolve().parent.parent / "data" / "resources.json").read_text()
)


def newest_run():
    dirs = [d for d in glob.glob(os.path.join(RUNS, "*")) if os.path.isdir(d)]
    live = [d for d in dirs if os.path.exists(os.path.join(d, "events.jsonl"))]
    return max(live, key=lambda d: os.path.getmtime(os.path.join(d, "events.jsonl")))


def live_runtime_problems(run, process_text=None, now=None, max_age=120.0):
    """Find a live Firaxis process that no longer has a state/control producer."""
    if process_text is None:
        process_text = subprocess.run(
            ["ps", "-axo", "command="], capture_output=True, text=True, check=False
        ).stdout
    now = time.time() if now is None else now
    lines = process_text.splitlines()
    game_running = any("Civ6_Exe_Child" in line for line in lines)
    tag = os.path.basename(os.path.abspath(run))
    controllers = [line for line in lines if "civ6_play.py" in line and tag in line]
    brains = [line for line in lines if "civ6_brain.py" in line and tag in line]
    events = os.path.join(run, "events.jsonl")
    try:
        age = max(0.0, now - os.path.getmtime(events))
    except OSError:
        age = float("inf")

    problems = []
    if game_running and not controllers:
        problems.append("Firaxis is running but this run's controller is absent")
    if controllers and any("--civvis-decides" in line for line in controllers) and not brains:
        problems.append("the CIVVIS decision worker is absent")
    if game_running and age > max_age:
        problems.append(f"the Firaxis export is {age:.0f}s stale")
    return problems


def axial(x, y):
    return (x - ((y - (y & 1)) // 2), y)


def civ6_id(value, prefix):
    """Normalize a Civ VI type identifier to a lower-case CIVVIS-style id."""
    value = str(value or "").strip()
    if value.upper().startswith(prefix):
        value = value[len(prefix):]
    return value.lower()


def civ6_map_script(value):
    """Normalize the Civ VI map-file spelling CIVVIS mirrors."""
    value = civ6_id(value, "")
    if value.endswith(".lua"):
        value = value[:-4]
    return {"smallcontinents": "small_continents"}.get(value, value)


def civ_id_matches(civ6, civvis):
    """Compare roster ids after the bridge's singular/plural normalization."""
    civ6 = str(civ6 or "").lower().removesuffix("_stk")
    civvis = str(civvis or "").lower()
    return civ6 == civvis or civ6.rstrip("s") == civvis.rstrip("s")


def leader_id_matches(civ6, civvis):
    """CIVVIS stores the shared leader identity for Firaxis alternate personas."""
    civ6 = str(civ6 or "").lower()
    civvis = str(civvis or "").lower()
    aliases = {
        "harald_alt": "harald_hardrada",
        "suleiman_alt": "suleiman",
    }
    return civ6 == civvis or civ6.removesuffix("_alt") == civvis or aliases.get(civ6) == civvis


def rival_identity_mismatches(state, board):
    """Compare each exported rival with the compact CIVVIS seat that owns it."""
    players = {player.get("id"): player for player in board.get("players") or []}
    mismatches = []
    for seat, rival in enumerate(state.get("rivals") or [], start=1):
        player = players.get(seat, {})
        expected_civ = civ6_id(rival.get("civ"), "CIVILIZATION_")
        expected_leader = civ6_id(rival.get("leader"), "LEADER_")
        actual_civ = str(player.get("civ") or "").replace(" ", "_").lower()
        actual_leader = civ6_id(player.get("leader_type"), "LEADER_") \
            if player.get("leader_type") else \
            str(player.get("leader") or "").replace(" ", "_").lower()
        wrong_civ = expected_civ and not civ_id_matches(expected_civ, actual_civ)
        wrong_leader = expected_leader and not leader_id_matches(expected_leader, actual_leader)
        if wrong_civ or wrong_leader:
            mismatches.append(
                f"seat {seat} Civ6={expected_civ or '?'} / {expected_leader or '?'} "
                f"CIVVIS={actual_civ or '?'} / {actual_leader or '?'}"
            )
    return mismatches


def public_fact_mismatches(state, board):
    """Compare diplomacy-ribbon facts and the viewed empire's live economy."""
    players = {player.get("id"): player for player in board.get("players") or []}
    expected = [(0, state)] + list(enumerate(state.get("rivals") or [], start=1))
    mismatches = []
    for seat, source in expected:
        player = players.get(seat, {})
        for key, board_key in (("score", "score"), ("military", "military")):
            want = source.get(key)
            got = player.get(board_key)
            if isinstance(want, (int, float)) and want >= 0 \
                    and (not isinstance(got, (int, float)) or abs(got - want) > 0.51):
                mismatches.append(f"seat {seat} {key} Civ6={want:g} CIVVIS={got!r}")

    ours = players.get(0, {})
    yields = ours.get("yields") or {}
    for key in ("science", "culture"):
        want = state.get(key)
        got = yields.get(key)
        if isinstance(want, (int, float)) and want > 0 \
                and (not isinstance(got, (int, float)) or abs(got - want) > 0.11):
            mismatches.append(f"seat 0 {key}/turn Civ6={want:g} CIVVIS={got!r}")
    for key in ("gold", "faith"):
        want = state.get(key)
        got = ours.get(key)
        if isinstance(want, (int, float)) and want >= 0 \
                and (not isinstance(got, (int, float)) or abs(got - want) > 0.11):
            mismatches.append(f"seat 0 {key} Civ6={want:g} CIVVIS={got!r}")
    capacity = state.get("trade_capacity")
    mirrored_capacity = (board.get("me") or {}).get("trade_capacity")
    if isinstance(capacity, (int, float)) and capacity >= 0 \
            and mirrored_capacity != capacity:
        mismatches.append(
            f"seat 0 trade_capacity Civ6={capacity:g} CIVVIS={mirrored_capacity!r}"
        )
    return mismatches


def mirrored_minor_sources(state):
    """Return real city-states and non-dormant Free Cities actors."""
    out = []
    for source in state.get("minors") or []:
        civ = source.get("civ")
        if civ == "CIVILIZATION_BARBARIAN":
            continue
        if civ == "CIVILIZATION_FREE_CITIES" \
                and not (source.get("cities") or source.get("units")):
            continue
        if civ:
            out.append(source)
    return out


def mirrored_minor_name(source):
    """Return the rendered actor name, not a potentially stale Firaxis type id."""
    cities = source.get("cities") or []
    capital = next((city for city in cities if city.get("capital")), None)
    city_name = (capital or (cities[0] if cities else {})).get("name")
    if city_name:
        return str(city_name).lower()
    return civ6_id(source.get("civ"), "CIVILIZATION_").replace("_", " ")


def minor_fact_mismatches(state, board, top):
    """Compare non-major identities, cities and public diplomacy facts."""
    sources = mirrored_minor_sources(state)
    players = list(board.get("players") or [])
    actual = [player for player in players
              if player.get("is_minor") and not player.get("is_barbarian")]
    free_cities = next((player for player in players if player.get("is_free_city")), None)
    cities = {tuple(city.get("pos") or []): city for city in board.get("cities") or []}
    host_to_board = {0: 0}
    host_to_board.update({rival.get("player"): seat
                          for seat, rival in enumerate(state.get("rivals") or [], start=1)})
    for source in sources:
        want = mirrored_minor_name(source)
        matched = free_cities if source.get("civ") == "CIVILIZATION_FREE_CITIES" else next(
            (candidate for candidate in actual
             if str(candidate.get("civ") or "").lower() == want), None
        )
        if matched is not None:
            host_to_board[source.get("player")] = matched.get("id")
    mismatches = []
    for source in sources:
        want = mirrored_minor_name(source)
        player = free_cities if source.get("civ") == "CIVILIZATION_FREE_CITIES" else next(
            (candidate for candidate in actual
             if str(candidate.get("civ") or "").lower() == want), None
        )
        if player is None:
            mismatches.append(f"missing minor actor {want or source.get('player')}")
            continue
        for key in ("score", "military"):
            expected, got = source.get(key), player.get(key)
            if isinstance(expected, (int, float)) and expected >= 0 \
                    and (not isinstance(got, (int, float)) or abs(got - expected) > 0.51):
                mismatches.append(f"{want} {key} Civ6={expected:g} CIVVIS={got!r}")
        if source.get("civ") != "CIVILIZATION_FREE_CITIES":
            expected_envoys = source.get("envoys")
            if isinstance(expected_envoys, (int, float)) \
                    and player.get("my_envoys") != expected_envoys:
                mismatches.append(
                    f"{want} envoys Civ6={expected_envoys:g} CIVVIS={player.get('my_envoys')!r}"
                )
            suzerain = source.get("suzerain")
            expected_suzerain = None if suzerain in (None, -1) else host_to_board.get(suzerain)
            if (suzerain in (None, -1) or expected_suzerain is not None) \
                    and player.get("suzerain") != expected_suzerain:
                mismatches.append(
                    f"{want} suzerain Civ6={suzerain!r} "
                    f"CIVVIS={player.get('suzerain')!r}"
                )
        for city in source.get("cities") or []:
            pos = axial(city.get("x", 0), top - city.get("y", 0))
            mirrored = cities.get(pos)
            if mirrored is None or mirrored.get("owner") != player.get("id"):
                mismatches.append(f"{want} city {city.get('name') or pos} missing at {pos}")
    return mismatches


def city_fact_mismatches(state, board, top):
    """Compare every host city field that has a CIVVIS representation."""
    by_pos = {tuple(city.get("pos") or []): city for city in board.get("cities") or []}
    mismatches = []
    for source in state.get("cities") or []:
        pos = axial(source.get("x", 0), top - source.get("y", 0))
        city = by_pos.get(pos)
        if city is None:
            continue
        name = source.get("name")
        if name and city.get("name") != name:
            mismatches.append(f"{name}@{pos} name={city.get('name')!r}")
        for key, tolerance in (("pop", 0), ("food", 0.11), ("loyalty", 0.11),
                               ("loyalty_per_turn", 0.11), ("defense", 0.11)):
            want, got = source.get(key), city.get(key)
            if isinstance(want, (int, float)) and want >= 0 \
                    and (not isinstance(got, (int, float)) or abs(got - want) > tolerance):
                mismatches.append(f"{name or pos} {key} Civ6={want:g} CIVVIS={got!r}")
        damage, max_damage = source.get("damage"), source.get("max_damage")
        if all(isinstance(value, (int, float)) and value >= 0
               for value in (damage, max_damage)) and max_damage > 0:
            want_hp = max(1, min(200, int(200 * (max_damage - damage) / max_damage + 0.5)))
            if city.get("hp") != want_hp:
                mismatches.append(
                    f"{name or pos} hp Civ6={want_hp} CIVVIS={city.get('hp')!r}"
                )
        wall_damage = source.get("wall_damage")
        max_wall_damage = source.get("max_wall_damage")
        if all(isinstance(value, (int, float)) and value >= 0
               for value in (wall_damage, max_wall_damage)):
            want_wall_max = int(max_wall_damage + 0.5)
            want_wall_hp = int(max(0, min(max_wall_damage,
                                          max_wall_damage - wall_damage)) + 0.5)
            if city.get("wall_max") != want_wall_max:
                mismatches.append(
                    f"{name or pos} wall_max Civ6={want_wall_max} "
                    f"CIVVIS={city.get('wall_max')!r}"
                )
            if city.get("wall_hp") != want_wall_hp:
                mismatches.append(
                    f"{name or pos} wall_hp Civ6={want_wall_hp} "
                    f"CIVVIS={city.get('wall_hp')!r}"
                )
        want_religion = civ6_id(source.get("religion"), "RELIGION_").replace("_", " ")
        got_religion = str(city.get("religion") or "").lower()
        if want_religion and want_religion != got_religion:
            mismatches.append(
                f"{name or pos} religion Civ6={want_religion!r} CIVVIS={got_religion!r}"
            )
        exported_buildings = {
            IDENTIFIER_ALIASES.get(name, name)
            for value in source.get("buildings") or []
            for name in [civ6_id(value, "BUILDING_")]
        }
        # CIVVIS models the Palace intrinsically on the current capital; keeping
        # it in the ordinary building collection would add its yields twice.
        want_buildings = exported_buildings - MIRRORED_WONDERS - {"palace"}
        got_buildings = {str(value).lower() for value in city.get("buildings") or []}
        if want_buildings != got_buildings:
            mismatches.append(
                f"{name or pos} buildings missing={sorted(want_buildings - got_buildings)} "
                f"extra={sorted(got_buildings - want_buildings)}"
            )
        if "wonders" in source:
            want_wonders = {
                IDENTIFIER_ALIASES.get(
                    civ6_id(wonder.get("type"), "BUILDING_"),
                    civ6_id(wonder.get("type"), "BUILDING_"),
                ): axial(wonder.get("x", 0), top - wonder.get("y", 0))
                for wonder in source.get("wonders") or []
            }
            got_wonders = {
                str(wonder).lower(): tuple(position)
                for wonder, position in (city.get("wonders") or {}).items()
            }
            if want_wonders != got_wonders:
                mismatches.append(
                    f"{name or pos} wonders Civ6={want_wonders!r} CIVVIS={got_wonders!r}"
                )
        want_districts = {
            IDENTIFIER_ALIASES.get(
                civ6_id(district.get("type"), "DISTRICT_"),
                civ6_id(district.get("type"), "DISTRICT_"),
            )
            for district in source.get("districts") or []
            # Firaxis exposes every wonder hex as the pseudo-type
            # DISTRICT_WONDER. CIVVIS stores the actual wonder and its position
            # in `wonders`, which is compared immediately above; counting the
            # pseudo-district too reports a duplicate representation as missing.
            if civ6_id(district.get("type"), "DISTRICT_")
            not in {"city_center", "wonder"}
            # An in-progress district is a foundation, not yet an entry in the
            # city's completed-district table. Its location and queue are
            # mirrored separately; comparing it here invents a missing district.
            and district.get("complete", True)
        }
        got_districts = {str(value).lower() for value in (city.get("districts") or {})}
        if want_districts != got_districts:
            mismatches.append(
                f"{name or pos} districts missing={sorted(want_districts - got_districts)} "
                f"extra={sorted(got_districts - want_districts)}"
            )
    return mismatches


def visible_exported_units(state, board):
    """Yield every currently visible unit with its compact CIVVIS owner seat."""
    yield from ((board.get("view_player", 0), unit)
                for unit in state.get("units") or [])
    for seat, rival in enumerate(state.get("rivals") or [], start=1):
        yield from ((seat, unit) for unit in rival.get("units") or [])
    for minor in mirrored_minor_sources(state):
        yield from ((None, unit) for unit in minor.get("units") or [])
    yield from ((None, unit) for unit in state.get("hostiles") or [])


def unmodelled_great_person(kind):
    """Great People are named individuals in CIVVIS rather than board units."""
    name = civ6_id(kind, "UNIT_")
    return name.startswith("great_") or name == "comandante_general"


def exported_unit_kind(unit):
    """Return the unit type across Firaxis's two export field names."""
    return unit.get("kind") or unit.get("type")


def unit_fact_mismatches(state, board, top):
    """Compare visible unit presence and facts across every exported actor."""
    by_pos = {}
    for unit in board.get("units") or []:
        by_pos.setdefault(tuple(unit.get("pos") or []), []).append(unit)
    source_groups = {}
    for owner, source in visible_exported_units(state, board):
        pos = axial(source.get("x", 0), top - source.get("y", 0))
        raw_kind = civ6_id(exported_unit_kind(source), "UNIT_")
        kind = IDENTIFIER_ALIASES.get(raw_kind, raw_kind)
        if kind.startswith("barbarian_"):
            kind = kind.removeprefix("barbarian_")
        # Apply aliases after removing Firaxis's barbarian implementation
        # prefix too: BARBARIAN_HORSE_ARCHER is the same modelled Saka horse
        # archer as SCYTHIAN_HORSE_ARCHER, not an absent `horse_archer` type.
        kind = IDENTIFIER_ALIASES.get(kind, kind)
        kind = UNIT_MODEL_FALLBACKS.get(kind, kind)
        source_groups.setdefault((owner, pos, kind), []).append(source)

    mismatches = []
    for (owner, pos, kind), sources in source_groups.items():
        candidates = [unit for unit in by_pos.get(pos, [])
                      if str(unit.get("type") or "").lower() == kind
                      and (owner is None or unit.get("owner") == owner)]
        if len(candidates) != len(sources):
            if not all(unmodelled_great_person(exported_unit_kind(source)) for source in sources):
                mismatches.append(
                    f"{exported_unit_kind(sources[0]) or '?'}@{pos} count "
                    f"Civ6={len(sources)} CIVVIS={len(candidates)}"
                )
            continue

        def source_key(source):
            hp = source.get("hp")
            turns = source.get("fortify_turns")
            return (
                int(hp + 0.5) if isinstance(hp, (int, float)) and hp > 0 else -1,
                bool(source.get("fortified")),
                max(0, min(2, int(turns)))
                if isinstance(turns, (int, float)) and turns >= 0 else -1,
            )

        def board_key(unit):
            return (
                int(unit.get("hp")) if isinstance(unit.get("hp"), (int, float)) else -1,
                bool(unit.get("fortified")),
                int(unit.get("fortify_turns"))
                if isinstance(unit.get("fortify_turns"), (int, float)) else -1,
            )

        wanted = sorted(source_key(source) for source in sources)
        actual = sorted(board_key(unit) for unit in candidates)
        if wanted != actual:
            for field, index in (("hp", 0), ("fortified", 1), ("fortify_turns", 2)):
                wanted_values = sorted(value[index] for value in wanted)
                actual_values = sorted(value[index] for value in actual)
                if wanted_values != actual_values:
                    mismatches.append(
                        f"{exported_unit_kind(sources[0]) or '?'}@{pos} {field} "
                        f"Civ6={wanted_values!r} CIVVIS={actual_values!r}"
                    )
    return mismatches


def production_item_name(value):
    """Normalize a live Civ VI production type to CIVVIS's queue vocabulary."""
    if not isinstance(value, str) or not value.strip():
        return None
    for prefix, kind in (
        ("UNIT_", "unit"),
        ("BUILDING_", "building"),
        ("DISTRICT_", "district"),
        ("WONDER_", "wonder"),
        ("PROJECT_", "project"),
        ("PRODUCT_", "product"),
    ):
        if value.upper().startswith(prefix):
            name = civ6_id(value, prefix)
            # Reuse the audited Firaxis-to-CIVVIS vocabulary for internal
            # implementation names such as BUILDING_GOV_CITYSTATES (the
            # player-facing Foreign Ministry), unique units, and era walls.
            name = IDENTIFIER_ALIASES.get(name, name)
            # Firaxis truncates these district type identifiers; the mirror
            # restores the full CIVVIS names when it resolves the rules table.
            if kind == "district":
                name = {
                    "government": "government_plaza",
                    "theater": "theater_square",
                }.get(name, name)
            elif kind == "project":
                name = {
                    "enhance_district_campus": "campus_research_grants",
                    "enhance_district_holy_site": "holy_site_prayers",
                    "enhance_district_commercial_hub": "commercial_hub_investment",
                    "enhance_district_harbor": "harbor_shipping",
                    "enhance_district_encampment": "encampment_training",
                    "enhance_district_industrial_zone": "industrial_zone_logistics",
                    "enhance_district_theater": "theater_square_festival",
                }.get(name, name)
            return kind, name
    return None


def queue_item_name(item):
    """Return the meaningful queue kind/name while ignoring placement metadata."""
    if not isinstance(item, dict):
        return None
    for kind in ("unit", "building", "district", "wonder", "project", "product"):
        value = item.get(kind)
        if isinstance(value, str) and value:
            return kind, value.lower()
    return None


def exported_route_pairs(state, top):
    """Active Civ VI route endpoints in the board's axial coordinate frame."""
    pairs = Counter()
    for route in state.get("trade_routes") or []:
        values = (route.get("origin_x"), route.get("origin_y"),
                  route.get("destination_x"), route.get("destination_y"))
        if not all(isinstance(value, int) and value >= 0 for value in values):
            continue
        origin_x, origin_y, destination_x, destination_y = values
        pairs[(axial(origin_x, top - origin_y),
               axial(destination_x, top - destination_y))] += 1
    return pairs


def board_route_pairs(board):
    """CIVVIS active route endpoints, resolved from route city ids to positions."""
    positions = {city.get("id"): tuple(city.get("pos") or [])
                 for city in board.get("cities") or []}
    pairs = Counter()
    for route in (board.get("me") or {}).get("routes") or []:
        origin = positions.get(route.get("origin"))
        destination = positions.get(route.get("dest"))
        if len(origin or ()) == 2 and len(destination or ()) == 2:
            pairs[(origin, destination)] += 1
    return pairs


def resource_visible_in_state(resource, state):
    if not resource or state is None:
        return True
    spec = RESOURCE_RULES.get(resource) or {}
    techs = {civ6_id(value, "TECH_") for value in state.get("techs") or []}
    civics = {civ6_id(value, "CIVIC_") for value in state.get("civics") or []}
    return (not spec.get("tech") or spec["tech"] in techs) and (
        not spec.get("civic") or spec["civic"] in civics
    )


def expected_tile_fields(plot, state=None):
    """Translate one exported plot through the same committed vocabulary as Rust."""
    terrain = VOCABULARY["terrains"].get(plot.get("t"))
    feature_name = plot.get("f")
    resource_name = plot.get("r")
    resource = VOCABULARY["resources"].get(resource_name) if resource_name else None
    if not resource_visible_in_state(resource, state):
        resource = None
    improvement_name = plot.get("im")
    improvement = None
    if isinstance(improvement_name, str):
        improvement = civ6_id(improvement_name, "IMPROVEMENT_")
        improvement = IDENTIFIER_ALIASES.get(improvement, improvement)
        if improvement not in MIRRORED_IMPROVEMENTS:
            improvement = f"<unmapped:{improvement_name}>"
    return {
        "terrain": terrain.get("terrain") if terrain else f"<unmapped:{plot.get('t')}>",
        "hills": bool(terrain.get("hills")) if terrain else None,
        "feature": (
            VOCABULARY["features"].get(feature_name)
            if feature_name else None
        ),
        "resource": resource,
        "improvement": improvement,
        "river": bool(plot.get("ri")),
        "coastal_lowland": max(0, min(3, int(plot.get("cl") or 0))),
    }


def exact_tile_mismatches(pairs, state=None, limit=12):
    """Count field-level disagreements and retain bounded coordinate examples."""
    counts, examples = Counter(), []
    for board_tile, plot in pairs:
        expected = expected_tile_fields(plot, state)
        for field, wanted in expected.items():
            actual = board_tile.get(field)
            if actual == wanted:
                continue
            counts[field] += 1
            if len(examples) < limit:
                examples.append(
                    f"{field}@{plot.get('x')},{plot.get('y')} "
                    f"Civ6={wanted!r} CIVVIS={actual!r}"
                )
    return counts, examples


def leaked_hidden_resources(pairs, state):
    leaks = []
    for _, plot in pairs:
        raw = plot.get("r")
        resource = VOCABULARY["resources"].get(raw) if raw else None
        if resource and not resource_visible_in_state(resource, state):
            leaks.append(f"{resource}@{plot.get('x')},{plot.get('y')}")
    return leaks


def latest_seat(run):
    """The latest startup seat event, which carries setup outside state patches."""
    latest = None
    with open(os.path.join(run, "events.jsonl")) as handle:
        for line in handle:
            if '"seat"' not in line:
                continue
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if (event.get("kind") or event.get("event")) == "seat":
                latest = event
    return latest


def load_export(run, upto=None):
    """Latest value per plot, exactly like Snapshot::from_chunks (later wins).

    ⚠ `upto` bounds the export to what had been sent BY THAT TURN. The mirror
    republishes on a cadence, so comparing a board from turn N against an export
    that has since reached turn N+7 reports the growth as loss -- this checker
    cried wolf exactly that way ("13 exported plots are NOT on the board", all of
    them present one publish later). The board's own turn is the cutoff.
    """
    plots, turn = {}, 0
    with open(os.path.join(run, "events.jsonl")) as handle:
        for line in handle:
            try:
                event = json.loads(line)
            except ValueError:
                continue
            kind = event.get("kind") or event.get("event")
            if kind == "turn":
                turn = max(turn, int(event.get("turn") or 0))
            if kind != "tiles":
                continue
            if upto is not None and (event.get("turn") or 0) > upto:
                continue
            for plot in event.get("plots") or []:
                plots[(plot["x"], plot["y"])] = plot
    return plots, turn


def latest_terminal_turn(run):
    """Return the last victory/own-defeat frame in a completed event stream."""
    turn = None
    with open(os.path.join(run, "events.jsonl")) as handle:
        for line in handle:
            try:
                event = json.loads(line)
            except ValueError:
                continue
            kind = event.get("kind") or event.get("event")
            if kind == "victory" or (kind == "defeat" and event.get("ours")):
                turn = int(event.get("turn") or 0)
    return turn


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("run", nargs="?", help="run directory (newest by default)")
    parser.add_argument(
        "--archive", action="store_true",
        help="compare a completed archive without requiring a live controller",
    )
    args = parser.parse_args(argv)
    run = args.run or newest_run()
    problems: list[str] = []
    if not args.archive:
        runtime = live_runtime_problems(run)
        if runtime:
            problems.append("control")
            print("CONTROL  ⚠ " + "; ".join(runtime))
        else:
            print("CONTROL  live game, export and CIVVIS worker are current   OK")
    # The viewer can publish a staged board a fraction of a second before follow.py
    # appends the corresponding host event. Never compare that future board with the
    # previous export: one ordinary unit move then looks exactly like a dropped unit.
    board = state = None
    game_turn = -1
    for _ in range(20):
        board = json.load(
            urllib.request.urlopen(f"http://127.0.0.1:{PORT}/state", timeout=30)
        )
        _, game_turn = load_export(run)
        state = latest_state(run, upto=board["turn"])
        state_turn = int((state or {}).get("turn") or -1)
        # A terminal frame has an exact state and victory/defeat event but no
        # following playable `turn` event. In archive mode that pair is the
        # authoritative host frame; requiring a turn-start marker would reject
        # every normally completed game one frame after its last playable turn.
        if args.archive and state_turn == board["turn"] \
                and latest_terminal_turn(run) == board["turn"]:
            game_turn = max(game_turn, state_turn)
        if game_turn >= board["turn"] and state_turn == board["turn"]:
            break
        time.sleep(0.1)
    assert board is not None
    state_turn = int((state or {}).get("turn") or -1)
    if game_turn < board["turn"] or state_turn != board["turn"]:
        print(f"run   {os.path.basename(run)}")
        print(f"turn  game {game_turn}   board {board['turn']}   state {state_turn}   ⚠ DRIFT")
        print("\nDISAGREEMENTS: no exact host frame exists for the published board")
        return 1
    plots, _ = load_export(run, upto=board["turn"])
    tiles = {tuple(t["pos"]): t for t in board["map"]["tiles"]}
    visible = {tuple(v) for v in board["visible"]}

    print(f"run   {os.path.basename(run)}")
    print(f"turn  game {game_turn}   board {board['turn']}   state {state_turn}   OK")

    # --- lobby setup -------------------------------------------------------
    # The seat event is emitted once rather than copied into each state patch.
    # Compare its actual Civ VI identifiers to the reconstructed board instead
    # of assuming the command-line defaults used to launch the viewer survived.
    seat = latest_seat(run)
    if seat is None:
        print("SETUP    no seat event yet")
    else:
        player = next((p for p in board.get("players", [])
                       if p.get("id") == board.get("view_player", 0)), {})
        expected = {
            "speed": civ6_id(seat.get("speed"), "GAMESPEED_"),
            "difficulty": civ6_id(seat.get("difficulty"), "DIFFICULTY_"),
            "map": civ6_map_script(seat.get("map")),
            "size": civ6_id(seat.get("size"), "MAPSIZE_"),
            "civ": civ6_id(seat.get("civ"), "CIVILIZATION_"),
            "leader": civ6_id(seat.get("leader"), "LEADER_"),
        }
        actual = {
            "speed": str(board.get("game_speed") or "").lower(),
            "difficulty": str(board.get("difficulty") or "").lower(),
            "map": str(board.get("map", {}).get("script") or "").lower(),
            "size": str(board.get("map", {}).get("size") or "").lower(),
            "civ": str(player.get("civ") or "").replace(" ", "_").lower(),
            "leader": str(player.get("leader") or "").replace(" ", "_").lower(),
        }
        mismatches = [
            key for key, want in expected.items()
            if want and not (
                civ_id_matches(want, actual.get(key)) if key == "civ"
                else leader_id_matches(want, actual.get(key)) if key == "leader"
                else actual.get(key) == want
            )
        ]
        print("SETUP    "
              f"speed {actual['speed'] or '?'}; "
              f"difficulty {actual['difficulty'] or '?'}; "
              f"map {actual['map'] or '?'}/{actual['size'] or '?'}; "
              f"{player.get('civ') or '?'} / {player.get('leader') or '?'}")
        if mismatches:
            problems.append("setup")
            detail = ", ".join(f"{key} Civ6={expected[key] or '?'} "
                               f"CIVVIS={actual[key] or '?'}" for key in mismatches)
            print(f"         ⚠ {detail}")
        else:
            print("         OK")

    # --- the flip constant, discovered not assumed -------------------------
    best, best_hits = None, -1
    for top in range(40, 50):
        hits = len({axial(x, top - y) for (x, y) in plots} & set(tiles))
        if hits > best_hits:
            best, best_hits = top, hits
    if best_hits < 0.9 * len(tiles):
        print(f"⚠ coordinate frames do not line up (best {best_hits}/{len(tiles)} "
              f"at TOP={best}); every comparison below would be meaningless")
        return 1
    print(f"frame TOP={best}  overlap {best_hits}/{len(tiles)}")
    pairs = [(tiles[axial(x, best - y)], p)
             for (x, y), p in plots.items() if axial(x, best - y) in tiles]

    print()
    tile_counts, tile_examples = exact_tile_mismatches(pairs, state)
    mismatched_tiles = sum(tile_counts.values())
    print(f"TILES    {len(pairs)} paired; {mismatched_tiles} field disagreement(s)"
          + (f"   ⚠ {dict(tile_counts)}" if mismatched_tiles else "   OK"))
    if tile_examples:
        problems.append("tiles")
        print("         " + "; ".join(tile_examples))
    leaks = leaked_hidden_resources(pairs, state)
    if leaks:
        print(f"KNOWLEDGE {len(leaks)} raw resource leak(s) hidden by CIVVIS: "
              + "; ".join(leaks[:8]))
        if not args.archive:
            problems.append("knowledge")

    # --- fog memory (#713) -------------------------------------------------
    # ⚠ The invariant is "the board carries every plot the mod exported", NOT
    # "some ground is fogged". Early on, a seat with two units and no cities can
    # SEE everything it has revealed, and a fogged count of zero is correct there.
    # Checking for fog directly cried wolf on turn 1 of a healthy run.
    fogged = len(tiles) - len(visible)
    missing = len(plots) - best_hits
    print(f"FOG      board {len(tiles)} tiles, {len(visible)} visible, "
          f"{fogged} remembered-but-fogged; export has {len(plots)}")
    if missing > 0:
        problems.append("fog")
        print(f"         ⚠ {missing} exported plots are NOT on the board — "
              f"remembered ground is being dropped")
    elif fogged == 0 and len(tiles) > 60:
        print("         ⚠ nothing is fogged on a board this large; suspect a collapse "
              "onto current visibility")
    else:
        print("         OK")

    # --- rivers (#714): a Civ 6 river plot is fresh water BY DEFINITION ----
    riv = [(b, p) for b, p in pairs if b.get("river")]
    fresh = [1 for _, p in pairs if p.get("fw")]
    base = len(fresh) / max(1, len(pairs))
    hit = sum(1 for _, p in riv if p.get("fw"))
    rate = hit / max(1, len(riv))
    exported_rv = sum(1 for _, p in pairs if p.get("rv"))
    print(f"RIVERS   {len(riv)} river tiles; {hit} of them fresh in the export "
          f"({rate:.1%}) vs {base:.1%} base rate")
    # ⚠ Judge by LIFT over the base rate, not by an absolute share. A fixed 0.8 bar
    # called 73.8%-against-a-17.5%-base "chance" — a 4.2x lift. The share falls as
    # the map opens up and ocean dilutes the denominator, so only the ratio is
    # comparable across turns. It cannot reach 100%: `set_river_edge` marks the tile
    # across the segment too, and where that neighbour is unrevealed the export has
    # no `fw` to agree with.
    lift = rate / base if base > 0 else 0.0
    if not exported_rv:
        print("         ⚠ export carries no `rv` at all — old mod, rivers cannot be mirrored")
    elif not riv:
        print("         (no river tiles on the board yet)")
    elif lift < 1.5:
        problems.append("rivers")
        print(f"         ⚠ lift {lift:.1f}x over base — no better than chance, "
              f"these are the GENERATED map's rivers")
    else:
        print(f"         OK  ({lift:.1f}x base rate)")

    # --- landmass (#716) ---------------------------------------------------
    land = [(b, p) for b, p in pairs if not p.get("w")]
    named = sum(1 for _, p in land if p.get("ct"))
    with_cont = sum(1 for b, _ in land if b.get("continent") is not None)
    water_cont = sum(1 for b, p in pairs if p.get("w") and b.get("continent") is not None)
    cliffs = sum(1 for t in tiles.values() if any(t.get("cliff_edges") or []))
    print(f"LAND     {len(land)} land plots; export names a continent on {named}; "
          f"board assigns one to {with_cont}")
    if not named:
        print("         ⚠ export carries no `ct` — old mod")
    elif with_cont < named:
        problems.append("land")
        print(f"         ⚠ {named - with_cont} land plots lost their continent")
    else:
        print("         OK")
    # ⚠ NOT "water must have no continent". Civilization VI really does put COAST
    # tiles on a continent — 17 of them read CONTINENT_AUSTRALIA on this very run —
    # and carrying that is correct, because "another continent" is a rule and
    # dropping it would lose information. CIVVIS's own field doc says water has none;
    # that is CIVVIS's convention, not Civilization VI's, and the mirror follows the
    # game. So the check is agreement with the export.
    water_named = sum(1 for _, p in pairs if p.get("w") and p.get("ct"))
    if water_cont != water_named:
        print(f"         ⚠ board gives {water_cont} water tiles a continent, "
              f"the export names {water_named}")
    print(f"CLIFFS   {cliffs} " + ("⚠ invented — Civ 6 exposes no cliff accessor"
                                   if cliffs else "OK (none, as intended)"))
    if cliffs:
        problems.append("cliffs")

    # --- cities and units: entity-level, not tile-level ---------------------
    # ⚠ Counts alone are the weak check this project keeps getting burned by --
    # 21 units exported and 15 reconstructed once read as healthy because nothing
    # compared them. So compare the SETS, and name what is missing.
    # Keep entities on the same temporal boundary as terrain. A game can export
    # the next state's units between the `/state` fetch and this read; comparing
    # them to an older board reports ordinary movement as a dropped mirror unit.
    if state is None:
        print("ENTITIES (no state event yet)")
        return 0

    rival_mismatches = rival_identity_mismatches(state, board)
    if rival_mismatches:
        problems.append("rivals")
        print(f"RIVALS   {len(state.get('rivals') or [])} met   ⚠ "
              + "; ".join(rival_mismatches))
    else:
        print(f"RIVALS   {len(state.get('rivals') or [])} met identities   OK")

    minor_sources = mirrored_minor_sources(state)
    minor_mismatches = minor_fact_mismatches(state, board, best)
    if minor_mismatches:
        problems.append("city-states")
        print(f"MINORS   {len(minor_sources)} present   ⚠ "
              + "; ".join(minor_mismatches))
    elif "minors" not in state:
        print("MINORS   export has no city-state records (old control mod)")
    else:
        print(f"MINORS   {len(minor_sources)} present minor actor(s)   OK")

    public_mismatches = public_fact_mismatches(state, board)
    if public_mismatches:
        problems.append("public facts")
        print("PUBLIC   ⚠ " + "; ".join(public_mismatches))
    else:
        print("PUBLIC   score, military, treasury, faith, science and culture   OK")

    civ6_cities = {(c["x"], c["y"]) for c in state.get("cities") or []}
    board_cities = {tuple(c["pos"]) for c in board.get("cities", [])
                    if c.get("owner") == board.get("view_player", 0)}
    mapped = {axial(x, best - y) for (x, y) in civ6_cities}
    missing_cities = mapped - board_cities
    if missing_cities:
        problems.append("cities")
    print(f"CITIES   export {len(civ6_cities)}  board {len(board_cities)}"
          + (f"   ⚠ MISSING {sorted(missing_cities)}" if missing_cities else "   OK"))
    city_mismatches = city_fact_mismatches(state, board, best)
    if city_mismatches:
        problems.append("city facts")
        print("CITYDATA ⚠ " + "; ".join(city_mismatches))
    else:
        print("CITYDATA population, health, loyalty, defense, religion and development   OK")

    # --- production: an in-progress city must not read as idle -------------
    # A completed item used to stay in the mirror queue, then a new real item
    # appeared as the old one. Compare production on the same state boundary as
    # cities and units so normal turn advancement cannot look like a phantom.
    board_city_by_pos = {
        tuple(city["pos"]): city for city in board.get("cities", [])
        if city.get("owner") == board.get("view_player", 0)
    }
    production_mismatches, unmapped_production, checked_production = [], [], 0
    for city in state.get("cities") or []:
        pos = axial(city.get("x", 0), best - city.get("y", 0))
        board_city = board_city_by_pos.get(pos)
        if board_city is None:
            # The city assertion above names this loss more clearly.
            continue
        raw = city.get("producing")
        expected = production_item_name(raw)
        if raw is not None and expected is None:
            unmapped_production.append(f"{city.get('name', '?')}={raw!r}")
            continue
        checked_production += 1
        queue = list(board_city.get("queue") or [])
        actual = queue_item_name(queue[0]) if queue else None
        valid = actual == expected and (not queue if expected is None else len(queue) == 1)
        if not valid:
            production_mismatches.append(
                f"{city.get('name', '?')} Civ6={expected or 'idle'} "
                f"CIVVIS={actual or 'idle'}"
            )
    if production_mismatches or unmapped_production:
        problems.append("production")
    detail = []
    if production_mismatches:
        detail.append("MISMATCH " + "; ".join(production_mismatches))
    if unmapped_production:
        detail.append("UNMAPPED " + "; ".join(unmapped_production))
    print(f"PRODUCTION export {checked_production} city queues"
          + (f"   ⚠ {'; '.join(detail)}" if detail else "   OK"))

    # --- active trade routes ------------------------------------------------
    # A Trader remains a physical unit while travelling in Civilization VI, so
    # comparing units cannot tell us whether it is available for a new route.
    # Compare the route graph itself: these routes occupy capacity and contribute
    # yields, and a missing one made CIVVIS repeatedly order the same Trader.
    if "trade_routes" not in state:
        print("TRADE    export has no route records (old control mod)")
    else:
        exported_routes = exported_route_pairs(state, best)
        mirrored_routes = board_route_pairs(board)
        if exported_routes != mirrored_routes:
            problems.append("trade")
            print(f"TRADE    Civ6 {sum(exported_routes.values())}  "
                  f"CIVVIS {sum(mirrored_routes.values())}   ⚠ "
                  f"MISSING {list((exported_routes - mirrored_routes).elements())}; "
                  f"EXTRA {list((mirrored_routes - exported_routes).elements())}")
        else:
            print(f"TRADE    {sum(exported_routes.values())} active route(s)   OK")

    # ⚠ Name what is missing, do not just count it. A bare "1 dropped" sends the
    # reader to the wrong place; the position and type say immediately whether it is
    # a known modelling gap (Great People are not units in CIVVIS) or something new.
    civ6_units = list(state.get("units") or [])
    ours = [u for u in board.get("units", []) if u.get("owner") == board.get("view_player", 0)]
    on_board = {tuple(u["pos"]) for u in ours if u.get("pos")}
    missing_units = [
        f'{exported_unit_kind(u) or "?"}@{u.get("x")},{u.get("y")}'
        for u in civ6_units
        if axial(u.get("x", 0), best - u.get("y", 0)) not in on_board
    ]
    unit_mismatches = unit_fact_mismatches(state, board, best)
    if unit_mismatches:
        problems.append("unit facts")
        print("UNITDATA ⚠ " + "; ".join(unit_mismatches))
    else:
        print("UNITDATA type, position, health and fortification   OK")
    # ⚠ COUNT AND POSITION, because neither alone is enough. Position-matching
    # cannot see a STACKED drop -- Civilization VI puts a civilian and a military
    # unit on one tile and CIVVIS does not, so two exported units collapse onto one
    # board tile and every position still looks covered. The count catches that.
    # Position-matching, in turn, names WHICH unit is gone when the count is equal
    # but the board holds a different one.
    # ⚠ CATEGORISE, DO NOT SUPPRESS. CIVVIS does not model Great People as units at
    # all, so they are absent from the board on EVERY run. Failing the gate on a
    # documented modelling gap means the gate always fails, which is the same as
    # having no gate -- and it buries the drop that is actually new.
    #
    # They are still counted and still printed. What changes is that a known gap does
    # not set the exit status, so a NEW disappearance stands out against it.
    # ⚠ Counted from the EXPORT, not from `missing_units`. Great People stack with
    # other units, so position-matching covers them and they never appear in the
    # missing list -- the count path is where they land, and that is where they have
    # to be discounted. Getting this wrong left the gate failing on them anyway.
    great_people = [u for u in civ6_units if "GREAT_" in (u.get("kind") or "")]
    unexplained = [u for u in missing_units if "GREAT_" not in u]
    short = len(civ6_units) - len(ours) - len(great_people)
    if short > 0 or unexplained:
        problems.append("units")
    detail = ""
    if unexplained:
        detail = f"   ⚠ NOT on the board: {unexplained}"
    elif great_people and short <= 0:
        detail = (f"   OK — {len(great_people)} Great People absent, which CIVVIS does "
                  f"not model as units")
    elif short > 0:
        # ⚠ Do NOT name the cause. From the board alone a collapsed stack and an
        # unmodelled type (Great People are not units in CIVVIS) look identical, and
        # this line used to assert "a STACK was collapsed" when the decider's own
        # `dropped_units` was saying `great_person`. Report the fact, point at the
        # field that knows why.
        detail = (f"   ⚠ {short} fewer on the board beyond the {len(great_people)} "
                  f"Great People, every position covered — a stack collapsed or a type "
                  f"CIVVIS does not model; the decider's `dropped_units` names which")
    print(f"UNITS    export {len(civ6_units)}  board {len(ours)}"
          + (detail or "   OK"))

    # --- HOSTILES ----------------------------------------------------------
    #
    # ★★★★★ THE ONE THING ON THE BOARD THE SEAT MOST NEEDS TO SEE, AND NOTHING
    # CHECKED IT. Every other line here verifies what the empire OWNS. The threat
    # list is what it must plan AROUND, and until now no instrument compared it.
    #
    # Measured 2026-08-02 on run civvis-20260802T041527Z: 14 settlers were built
    # and every one vanished at hp 100 having moved 0-4 tiles from the capital,
    # while the city count sat at 1 from turn 41 to 241. Civilization VI CAPTURES
    # civilians rather than killing them, so full health at disappearance is the
    # signature of capture — and on each settler's last turn a hostile stood 1-3
    # tiles away, 8 of 13 of them ADJACENT. One of those "hostiles" was itself a
    # UNIT_SETTLER: ours, already taken.
    #
    # The first question that asks is whether the seat could SEE them, and it took
    # a hand-written pass over events.jsonl to answer (it could — 11 to 15 hostiles
    # exported on every capture turn). This line makes that answer automatic.
    #
    # ⚠ Hostiles are planted under the BARBARIAN SEAT, not the viewer, so they are
    # board units whose owner is not `view_player` — see `rebuild_from_state`'s
    # `barbarian_seat` branch, which records `no_barbarian_seat` for every hostile
    # when that seat is missing. A roster with no barbarian seat cannot hold the
    # threat list at all, and that reads here as every hostile missing.
    civ6_hostiles = list(state.get("hostiles") or [])
    seat = board.get("view_player", 0)
    theirs = [u for u in board.get("units", []) if u.get("owner") != seat]
    their_pos = {tuple(u["pos"]) for u in theirs if u.get("pos")}
    # ⚠⚠ FOG-GATE IT, OR IT CRIES WOLF ON EVERY RUN.
    #
    # `hostiles` is documented in mirror.rs as "a threat list the planner needs,
    # NOT knowledge the seat has" -- it is not fog-gated. The board on :8610 is the
    # SEATED view and shows only what the seat can currently see. Comparing the two
    # directly asks the board to contain units the seat cannot see, which it must
    # not.
    #
    # The first version of this check did exactly that and reported
    # `8 exported, 5 NOT on the board` on a healthy run, while the decider's own
    # `dropped_units` recorded no hostile dropped at all -- only three Great
    # Writers. Same shape as the TREASURY wolf: two numbers that look comparable
    # and are measured over different populations.
    #
    # What IS assertable: a hostile standing on a tile the seat can SEE must be on
    # the board. Anything beyond the fog is the planner's private threat list and
    # is none of this check's business.
    seen_hostiles = [
        h for h in civ6_hostiles
        if axial(h.get("x", 0), best - h.get("y", 0)) in visible
    ]
    missing_hostiles = [
        f'{h.get("type") or h.get("kind") or "?"}@{h.get("x")},{h.get("y")}'
        for h in seen_hostiles
        if axial(h.get("x", 0), best - h.get("y", 0)) not in their_pos
    ]
    # ⚠ `type`, not `kind`. Our own units are exported as `kind`; the hostiles list
    # uses `type`, and reading the wrong one printed every name as "?".
    if missing_hostiles:
        problems.append("hostiles")
    print(f"HOSTILES export {len(civ6_hostiles)}, {len(seen_hostiles)} in sight"
          + (f"   ⚠ {len(missing_hostiles)} visible but NOT on the board: "
             f"{missing_hostiles[:6]}"
             if missing_hostiles
             else ("   all visible ones on the board   OK" if seen_hostiles
                   else "   none in sight   OK")))

    # --- TREASURY ----------------------------------------------------------
    #
    # ⚠⚠ THESE TWO ARE STOCKS, NOT RATES, AND THAT IS WHY THEY GET THEIR OWN
    # CHECK RATHER THAN JOINING `economy_drift`.
    #
    # The mod exports `gold` from `GetTreasury():GetGoldBalance()` and `faith`
    # from `GetReligion():GetFaithBalance()` — balances. `economy_drift` compares
    # science and culture, which the mod takes from `GetTechs():GetScienceYield()`
    # — a per-turn rate. Putting a balance beside a rate under one heading is the
    # apples-to-oranges the rest of this file exists to prevent.
    #
    # ⚠ AND IT MUST BE READ AT THE SAME TURN, which is the whole reason this was
    # worth adding rather than eyeballing. Measured 2026-08-02 on run
    # civvis-20260802T030910Z: read against the NEWEST export the treasury showed
    # `gold 176 vs 167` and `faith 23 vs 21` — a confident-looking 5% shortfall
    # that is nothing but one turn of income at +9 gold and +2 faith. Bounded to
    # the board's own turn the same instant read **134 vs 134 and 27 vs 27,
    # delta 0.0 on both**. An unbounded version of this check would have reported
    # a treasury defect on a perfectly faithful mirror, every single time.
    board_me = next((p for p in board.get("players") or [] if p.get("id") == 0), None)
    if board_me is None:
        print("TREASURY no seated player 0 on the board; cannot compare")
    else:
        rows = []
        for field in ("gold", "faith"):
            theirs, ours_value = state.get(field), board_me.get(field)
            # -1 is the mod's own "could not read it" sentinel, and a missing key
            # is an older export. Neither is a disagreement — say so rather than
            # inventing a delta, the same way `economy_drift` refuses to claim
            # anything from an export carrying no yields.
            if theirs is None or ours_value is None or theirs < 0:
                rows.append(f"{field} unknown")
                continue
            delta = ours_value - theirs
            rows.append(f"{field} {theirs:g}/{ours_value:g}"
                        + ("" if abs(delta) < 0.5 else f" ⚠{delta:+g}"))
            if abs(delta) >= 0.5:
                problems.append(f"treasury:{field}")
        print(f"TREASURY {'  '.join(rows)}"
              + ("   OK" if not any("⚠" in r for r in rows) else ""))

    # ⚠ Non-zero on a real disagreement, so this can gate a run rather than only
    # inform one. A frame mismatch already returned 1 above for the same reason:
    # a comparison whose coordinates do not line up is worse than no comparison.
    if problems:
        print()
        print(f"DISAGREEMENTS: {', '.join(problems)}")
        return 1
    return 0


def latest_state(run, upto=None):
    """The `state` event as of `upto`, or the most recent one.

    ⚠ BOUND IT, for exactly the reason `load_export` is bounded. The mirror
    republishes on a cadence; the export keeps going. Comparing the LATEST unit
    positions against a board published several turns earlier reports units that
    have simply MOVED as units that were dropped -- this checker did that and
    named four healthy units as missing, while a same-turn read showed 25 against
    25 with the rosters matching type for type.

    That is the third time publish lag has fooled this file. If a future check
    reads the run directory, bound it too.
    """
    latest = None
    with open(os.path.join(run, "events.jsonl")) as handle:
        for line in handle:
            if '"state"' not in line:
                continue
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if (event.get("kind") or event.get("event")) != "state":
                continue
            if upto is not None and int(event.get("turn") or 0) > upto:
                continue
            latest = event
    return latest


if __name__ == "__main__":
    sys.exit(main())
