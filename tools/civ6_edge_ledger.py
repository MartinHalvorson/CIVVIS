#!/usr/bin/env python3
"""The edge ledger of live Civilization VI runs: how far apart the two sides stood in every fight.

The tactics ledger (``civ6_tactics_ledger.py``) says how the army moved and
what it killed. It cannot say whether a fight was winnable: a unit lost to a
rival two eras ahead and a unit lost to a careless stand both read as one
loss. This ledger reads each blow's **strength gap**, with the same
definitions the simulator's ledger uses (``src/game/combat_ledger.rs``,
``EdgeLedger``), so a live run and a ``gene_screen`` row can be set side by
side (``docs/EDGE_LEDGER.md``).

Per run, and pooled over a control directory (split by lane: the seat's civ,
players, map and city-states):

* **(a) blows taken** -- unit-on-unit blows on our units, by the striker's side
  (rival major, city-state, barbarian, other) and **base gap**: the striker's
  attack strength minus our unit's combat strength, both from
  ``data/units.json`` (attack = Bombard, else Ranged, else Combat), with our
  units killed. Plus the rival majors' mean base gap by turn bucket (0-49,
  50-99, 100-149, 150+), raw and normalised to Online speed.
* **(b) blows struck** -- our unit-on-unit blows, by the target's side and
  **total gap**: ``preview.attacker_total_strength -
  defender_total_strength``, the host's own combat preview with every
  modifier. Also by base gap, for the blows (a) is read on.
* **(c) upgrades** at Online-equivalent turns 50 / 100 / 150: the share of our
  land army with an upgrade unlocked (the host's ``upgrade_to``), also with the
  successor's strategic resource in stock, and one the host would take this
  turn (no ``upgrade_blocked_reason``); the mean Combat gained; treasury and
  net gold per turn.
* **(d) strength** at the same marks: the mean Combat strength of our land
  army and of the rival majors' land military units we can see.

Only unit-against-unit blows count, both units military and neither an
aircraft: a city, district or encampment strike, an air strike and a blow on
a civilian are not in it, here or in the simulator. A unit type the ruleset
does not know (`unmapped`) is counted and listed, never guessed.

The unit names cross through ``civvis_orders::civ6_unit_type`` (read by
``civ6_unit_glyphs.civ6_unit_types``), so a unique unit such as
``UNIT_ROMAN_LEGION`` or ``UNIT_COLOMBIAN_LLANERO`` reads as its own row.

Usage::

    python3 tools/civ6_edge_ledger.py                       # pooled over the control dir
    python3 tools/civ6_edge_ledger.py --since 20261112 --per-run --metrics
    python3 tools/civ6_edge_ledger.py <run-dir> [<run-dir> ...] [--json out.json]

Read only. A state row is large, so rows are filtered by substring before
``json.loads``; run it niced, one process, beside a live game.
"""

from __future__ import annotations

import argparse
import json
import math
import statistics
import sys
from pathlib import Path
from typing import Any, Iterable

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_unit_glyphs  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
UNITS = ROOT / "data" / "units.json"
DEFAULT_CONTROL_DIR = Path.home() / "civvis-civ6-runs" / "control"

#: The five gap bins, low to high; the simulator's `EDGE_BINS` in the same order.
BINS = ("<=-10", "-9..-4", "-3..+3", "+4..+9", ">=+10")
#: Turn buckets of the mean-gap line: 0-49, 50-99, 100-149, 150+.
TURN_BUCKETS = ("0-49", "50-99", "100-149", "150+")
#: Snapshot marks for (c) and (d), in Online-equivalent turns.
MARKS = (50, 100, 150)
#: The sides a blow is split by. `other` is the Free Cities (player 62) and
#: anything unclassified; the simulator counts the Free Cities nowhere.
SIDES = ("major", "city_state", "barbarian", "other")
BARBARIAN_PLAYER = 63
FREE_CITIES_PLAYER = 62
#: GameSpeeds.CostMultiplier, the shipped speed rows (`GameSpeed::cost_percent`).
SPEED_COST_PERCENT = {
    "GAMESPEED_ONLINE": 50,
    "GAMESPEED_QUICK": 67,
    "GAMESPEED_STANDARD": 100,
    "GAMESPEED_EPIC": 150,
    "GAMESPEED_MARATHON": 300,
}
#: A state row whose mark is this many turns late is still read at that mark.
MARK_SLACK = 8


def gap_bin(gap: float) -> int:
    """The bin index of one gap, rounded to a whole strength point first."""
    gap = int(round(gap))
    if gap <= -10:
        return 0
    if gap <= -4:
        return 1
    if gap <= 3:
        return 2
    if gap <= 9:
        return 3
    return 4


def turn_bucket(turn: float) -> int:
    return min(int(turn) // 50, 3)


def online_turn(turn: int, cost_percent: int) -> float:
    """The Online-speed turn the same progress lands on: a Standard turn 100
    reads as Online turn 50, because Standard pays twice Online's costs."""
    return turn * 50.0 / cost_percent


# ------------------------------------------------------------------ the units
class UnitBook:
    """`data/units.json` read through the host's own type names."""

    def __init__(self, units: dict[str, dict[str, Any]], translate) -> None:
        self.units = units
        self.by_type = {translate(key): key for key in units}

    @classmethod
    def load(cls) -> "UnitBook":
        return cls(json.loads(UNITS.read_text()), civ6_unit_glyphs.civ6_unit_types())

    def key(self, civ6_type: Any) -> str | None:
        if not isinstance(civ6_type, str):
            return None
        return self.by_type.get(civ6_type)

    def spec(self, key: str | None) -> dict[str, Any]:
        return self.units.get(key or "", {})

    def attack(self, key: str) -> float:
        """Bombard, else Ranged, else Combat: the strength a blow is struck with."""
        spec = self.spec(key)
        for field in ("bombard_strength", "ranged_strength", "strength"):
            value = spec.get(field) or 0
            if value > 0:
                return float(value)
        return 0.0

    def combat(self, key: str) -> float:
        return float(self.spec(key).get("strength") or 0)

    def is_fighter(self, key: str | None) -> bool:
        """A military unit that is not an aircraft: the units a blow counts between."""
        spec = self.spec(key)
        return spec.get("class") == "military" and spec.get("domain") != "air"

    def is_land_army(self, key: str | None) -> bool:
        """A military land unit with a Combat strength: the army of (c) and (d)."""
        spec = self.spec(key)
        return (
            spec.get("class") == "military"
            and spec.get("domain") in (None, "land")
            and (spec.get("strength") or 0) > 0
        )

    def chain_upgrade(self, key: str, techs: set[str]) -> str | None:
        """The furthest successor whose technology we hold, for an export
        that predates the host's own `upgrade_to`."""
        best = None
        current = self.spec(key).get("upgrade_to")
        for _ in range(8):
            spec = self.units.get(current or "")
            if spec is None:
                break
            tech = spec.get("tech")
            if tech is None or "TECH_" + tech.upper() in techs:
                best = current
            current = spec.get("upgrade_to")
        return best


# -------------------------------------------------------------------- tallies
def _cell() -> list[float]:
    # blows, damage to the defender, damage to the attacker, defender killed,
    # attacker killed: the simulator's `EdgeCell` in the same order.
    return [0, 0, 0, 0, 0]


def _table() -> dict[str, list[list[float]]]:
    return {side: [_cell() for _ in BINS] for side in SIDES}


def _add_cell(cell: list[float], blow: dict[str, Any]) -> None:
    cell[0] += 1
    cell[1] += blow["damage_to_defender"]
    cell[2] += blow["damage_to_attacker"]
    cell[3] += 1 if blow["defender_killed"] else 0
    cell[4] += 1 if blow["attacker_killed"] else 0


def empty_ledger() -> dict[str, Any]:
    return {
        "runs": 0,
        "taken_base": _table(),
        "struck_total": _table(),
        "struck_base": _table(),
        # [blows, sum of base gaps] per bucket, rival majors' blows on us.
        "major_gap_by_turn": [[0, 0.0] for _ in TURN_BUCKETS],
        "major_gap_by_online_turn": [[0, 0.0] for _ in TURN_BUCKETS],
        "struck_without_preview": 0,
        "unmapped": {},
        "speed_unknown_runs": 0,
        "marks": {str(mark): [] for mark in MARKS},
        "kills": 0,
        "losses": 0,
        "cities_lost": 0,
        "summaries": 0,
    }


def merge(into: dict[str, Any], other: dict[str, Any]) -> None:
    into["runs"] += other["runs"]
    for table in ("taken_base", "struck_total", "struck_base"):
        for side in SIDES:
            for a, b in zip(into[table][side], other[table][side]):
                for i in range(5):
                    a[i] += b[i]
    for table in ("major_gap_by_turn", "major_gap_by_online_turn"):
        for a, b in zip(into[table], other[table]):
            a[0] += b[0]
            a[1] += b[1]
    for key in ("struck_without_preview", "speed_unknown_runs", "kills", "losses",
                "cities_lost", "summaries"):
        into[key] += other[key]
    for kind, n in other["unmapped"].items():
        into["unmapped"][kind] = into["unmapped"].get(kind, 0) + n
    for mark in into["marks"]:
        into["marks"][mark].extend(other["marks"][mark])


# -------------------------------------------------------------------- reading
def _state_turn(line: bytes) -> int | None:
    """The top-level turn of a `state` row without decoding it: keys are
    written sorted, so `"turn"` sits just before `"unit_maintenance_discount"`."""
    j = line.rfind(b'"unit_maintenance_discount"')
    if j < 0:
        return None
    i = line.rfind(b'"turn": ', 0, j)
    if i < 0:
        return None
    end = line.find(b",", i)
    try:
        return int(line[i + 8:end])
    except ValueError:
        return None


def _resource_stock(state: dict[str, Any]) -> dict[str, float]:
    stock: dict[str, float] = {}
    resources = state.get("strategic_resources")
    if isinstance(resources, dict):
        for name, value in resources.items():
            if isinstance(value, (int, float)):
                stock[str(name)] = float(value)
    return stock


def snapshot(state: dict[str, Any], book: UnitBook, majors: set[int],
             host_verdicts: bool) -> dict[str, Any]:
    """(c) and (d) on one frame: our land army and what it could become, and
    the rival majors' land military units we can see.

    `host_verdicts` says the run's export carries the host's own upgrade
    reading per unit (`upgrade_to`, `upgrade_blocked_reason`, both written
    only when a successor is unlocked). Without it the chain is walked on our
    techs, and whether the host would take the upgrade this turn is unknown."""
    techs = set(state.get("techs") or [])
    stock = _resource_stock(state)
    army = unlocked = with_material = offer = 0
    gains: list[float] = []
    ours: list[float] = []
    for unit in state.get("units") or []:
        key = book.key(unit.get("kind"))
        if not book.is_land_army(key):
            continue
        army += 1
        ours.append(book.combat(key))
        if host_verdicts:
            target_type = unit.get("upgrade_to")
            if not target_type:
                continue
            target = book.key(target_type)
            resource = unit.get("upgrade_resource")
            cost = unit.get("upgrade_resource_cost") or 0
            if not unit.get("upgrade_blocked_reason"):
                offer += 1
        else:
            target = book.chain_upgrade(key, techs)
            if target is None:
                continue
            spec = book.spec(target)
            resource = spec.get("requires_resource")
            resource = "RESOURCE_" + resource.upper() if resource else None
            cost = spec.get("resource_cost") or 0
        unlocked += 1
        if target is not None:
            gains.append(book.combat(target) - book.combat(key))
        if not resource or stock.get(resource, 0.0) >= cost:
            with_material += 1
    rivals: list[float] = []
    for rival in state.get("rivals") or []:
        if rival.get("player") not in majors:
            continue
        for unit in rival.get("units") or []:
            key = book.key(unit.get("kind") or unit.get("type"))
            if book.is_land_army(key):
                rivals.append(book.combat(key))
    gold_per_turn = state.get("gold_per_turn")
    return {
        "turn": state.get("turn"),
        "army": army,
        "unlocked": unlocked,
        "with_material": with_material,
        "offer": offer if host_verdicts else None,
        "gain_sum": sum(gains),
        "gained": len(gains),
        "gold": float(state.get("gold") or 0),
        "gold_per_turn": float(gold_per_turn) if isinstance(gold_per_turn, (int, float)) else None,
        "our_strength_sum": sum(ours),
        "rival_units": len(rivals),
        "rival_strength_sum": sum(rivals),
    }


def read_run(run: Path, book: UnitBook) -> dict[str, Any]:
    """One run's ledger. Blows are kept until the seat row has said who the
    majors are, then classified."""
    seat: dict[str, Any] | None = None
    blows: list[dict[str, Any]] = []
    states: dict[int, dict[str, Any]] = {}
    pending_marks: list[int] | None = None
    major_ids: set[int] = set()
    host_verdicts = False
    events = run / "events.jsonl"
    with events.open("rb") as handle:
        for line in handle:
            if b'"kind": "combat"' in line and len(line) < 20000:
                try:
                    event = json.loads(line)
                except ValueError:
                    continue
                if event.get("kind") == "combat":
                    blow = _blow(event)
                    if blow is not None:
                        blows.append(blow)
                continue
            if seat is None and b'"kind": "seat"' in line and len(line) < 20000:
                try:
                    event = json.loads(line)
                except ValueError:
                    continue
                if event.get("kind") == "seat":
                    seat = event
                continue
            if b'"kind": "state"' not in line:
                continue
            if not host_verdicts and b'"upgrade_to"' in line:
                host_verdicts = True
            if pending_marks is None:
                cost = SPEED_COST_PERCENT.get(str((seat or {}).get("speed") or ""))
                if cost is None:
                    cost = SPEED_COST_PERCENT.get(_summary_speed(run) or "", 0)
                # Raw turns of the Online-equivalent marks; none when the speed is unknown.
                pending_marks = (
                    [math.ceil(mark * cost / 50) for mark in MARKS] if cost else []
                )
                mark_of_raw = dict(zip(pending_marks, MARKS))
            if not pending_marks:
                continue
            turn = _state_turn(line)
            if turn is None or turn < pending_marks[0]:
                continue
            while pending_marks and turn > pending_marks[0] + MARK_SLACK:
                pending_marks.pop(0)
            if not pending_marks or turn < pending_marks[0]:
                continue
            try:
                state = json.loads(line)
            except ValueError:
                continue
            if state.get("kind") != "state":
                continue
            states[mark_of_raw[pending_marks.pop(0)]] = state
            for rival in state.get("rivals") or []:
                if isinstance(rival.get("player"), int):
                    major_ids.add(rival["player"])
    speed = str((seat or {}).get("speed") or _summary_speed(run) or "")
    cost = SPEED_COST_PERCENT.get(speed)
    local = (seat or {}).get("local_player")
    players = (seat or {}).get("players")
    if isinstance(players, int) and players > 0:
        major_ids |= set(range(players))
    if isinstance(local, int):
        major_ids.add(local)
    ledger = empty_ledger()
    ledger["runs"] = 1
    if cost is None:
        ledger["speed_unknown_runs"] = 1
    for blow in blows:
        _tally(ledger, blow, book, major_ids, cost)
    for mark, state in sorted(states.items()):
        ledger["marks"][str(mark)].append(snapshot(state, book, major_ids - {local}, host_verdicts))
    summary = _summary(run)
    combat = (summary or {}).get("combat") or {}
    if isinstance(combat.get("kills"), int) and isinstance(combat.get("losses"), int):
        ledger["kills"] = combat["kills"]
        ledger["losses"] = combat["losses"]
        ledger["cities_lost"] = int(combat.get("cities_lost") or 0)
        ledger["summaries"] = 1
    ledger["lane"] = lane_of(seat, summary)
    ledger["speed"] = speed or None
    ledger["run"] = run.name
    ledger["host_verdicts"] = host_verdicts
    return ledger


def _blow(event: dict[str, Any]) -> dict[str, Any] | None:
    attacker = event.get("attacker") or {}
    defender = event.get("defender") or {}
    if attacker.get("type") != "unit" or defender.get("type") != "unit":
        return None
    preview = event.get("preview") or {}
    total = None
    if isinstance(preview.get("attacker_total_strength"), (int, float)) and isinstance(
        preview.get("defender_total_strength"), (int, float)
    ):
        total = preview["attacker_total_strength"] - preview["defender_total_strength"]
    return {
        "turn": event.get("turn") if isinstance(event.get("turn"), int) else 0,
        "ours": bool(event.get("ours")),
        "against_us": bool(event.get("against_us")),
        "attacker_player": attacker.get("player"),
        "attacker_kind": attacker.get("kind"),
        "defender_player": defender.get("player"),
        "defender_kind": defender.get("kind"),
        "total_gap": total,
        "damage_to_defender": float(event.get("damage_to_defender") or 0),
        "damage_to_attacker": float(event.get("damage_to_attacker") or 0),
        "defender_killed": bool(event.get("defender_killed")),
        "attacker_killed": bool(event.get("attacker_killed")),
    }


def side_of(player: Any, majors: set[int]) -> str:
    if player == BARBARIAN_PLAYER:
        return "barbarian"
    if player == FREE_CITIES_PLAYER or not isinstance(player, int) or player < 0:
        return "other"
    if player in majors:
        return "major"
    return "city_state"


def _tally(ledger: dict[str, Any], blow: dict[str, Any], book: UnitBook,
           majors: set[int], cost: int | None) -> None:
    if not (blow["ours"] or blow["against_us"]):
        return
    attacker = book.key(blow["attacker_kind"])
    defender = book.key(blow["defender_kind"])
    for kind, key in ((blow["attacker_kind"], attacker), (blow["defender_kind"], defender)):
        if key is None:
            ledger["unmapped"][str(kind)] = ledger["unmapped"].get(str(kind), 0) + 1
    if attacker is None or defender is None:
        return
    if not (book.is_fighter(attacker) and book.is_fighter(defender)):
        return
    base = book.attack(attacker) - book.combat(defender)
    if blow["against_us"]:
        side = side_of(blow["attacker_player"], majors)
        _add_cell(ledger["taken_base"][side][gap_bin(base)], blow)
        if side == "major":
            bucket = ledger["major_gap_by_turn"][turn_bucket(blow["turn"])]
            bucket[0] += 1
            bucket[1] += base
            if cost:
                bucket = ledger["major_gap_by_online_turn"][
                    turn_bucket(online_turn(blow["turn"], cost))
                ]
                bucket[0] += 1
                bucket[1] += base
    else:
        side = side_of(blow["defender_player"], majors)
        _add_cell(ledger["struck_base"][side][gap_bin(base)], blow)
        if blow["total_gap"] is None:
            ledger["struck_without_preview"] += 1
        else:
            _add_cell(ledger["struck_total"][side][gap_bin(blow["total_gap"])], blow)


def _summary(run: Path) -> dict[str, Any] | None:
    try:
        return json.loads((run / "summary.json").read_text())
    except (OSError, ValueError):
        return None


def _summary_speed(run: Path) -> str | None:
    speed = (_summary(run) or {}).get("speed")
    return speed if isinstance(speed, str) else None


def lane_of(seat: dict[str, Any] | None, summary: dict[str, Any] | None) -> str:
    """The seat's civ, players, map and city-states: what a lane recipe fixes.
    Read from the seat row, so a run still in play joins its lane."""
    seat = seat or (summary or {}).get("seat") or {}
    civ = str(seat.get("civ") or "?").replace("CIVILIZATION_", "")
    world = str(seat.get("map") or "?").replace(".lua", "")
    return f"{civ} {seat.get('players', '?')}p {world} {seat.get('city_states', '?')}cs"


def control_runs(control_dir: Path, since: str | None, min_mb: float,
                 until: str | None = None) -> list[Path]:
    """Runs under the control dir named in [since, until) by their UTC stamp,
    whose event log is at least `min_mb` long."""
    runs = []
    for run in sorted(control_dir.glob("civvis-*")):
        events = run / "events.jsonl"
        if not run.is_dir() or not events.is_file():
            continue
        stamp = run.name[len("civvis-"):]
        if since and stamp[:len(since)] < since:
            continue
        if until and stamp[:len(until)] >= until:
            continue
        if events.stat().st_size < min_mb * 1e6:
            continue
        runs.append(run)
    return runs


# ------------------------------------------------------------------ reporting
def _mean(values: Iterable[float]) -> float | None:
    values = list(values)
    return sum(values) / len(values) if values else None


def _median(values: Iterable[float]) -> float | None:
    values = list(values)
    return statistics.median(values) if values else None


def mark_summary(rows: list[dict[str, Any]]) -> dict[str, Any]:
    """(c) and (d) over the games that reached one mark: per-game shares and
    means, averaged over games; treasury and income as medians."""
    with_army = [row for row in rows if row["army"] > 0]
    with_gain = [row for row in rows if row.get("gained", row["unlocked"]) > 0]
    with_offer = [row for row in with_army if row["offer"] is not None]
    seen = [row for row in rows if row["rival_units"] > 0]
    both = [row for row in seen if row["army"] > 0]
    gpt = [row["gold_per_turn"] for row in rows if row["gold_per_turn"] is not None]
    return {
        "games": len(rows),
        "army": _mean(row["army"] for row in rows),
        "unlocked_share": _mean(row["unlocked"] / row["army"] for row in with_army),
        "with_material_share": _mean(row["with_material"] / row["army"] for row in with_army),
        "offer_share": _mean(row["offer"] / row["army"] for row in with_offer),
        "gain": _mean(row["gain_sum"] / row.get("gained", row["unlocked"]) for row in with_gain),
        "gold_median": _median(row["gold"] for row in rows),
        "gold_per_turn_median": _median(gpt),
        "our_strength": _mean(row["our_strength_sum"] / row["army"] for row in with_army),
        "rival_strength": _mean(row["rival_strength_sum"] / row["rival_units"] for row in seen),
        "rival_games": len(seen),
        "strength_gap": _mean(
            row["rival_strength_sum"] / row["rival_units"] - row["our_strength_sum"] / row["army"]
            for row in both
        ),
    }


def _num(value: float | None, fmt: str) -> str:
    return "-" if value is None else format(value, fmt)


def render_ledger(ledger: dict[str, Any], title: str) -> str:
    out = [f"== {title}: {ledger['runs']} run(s)"]
    taken = ledger["taken_base"]
    out.append("(a) blows on our units by BASE gap (their attack - our unit's combat):")
    out.append(f"    {'side':<10} {'gap':>7} {'blows':>6} {'share':>6} {'dmg/blow':>8} "
               f"{'deaths':>6} {'death/blow':>10}")
    for side in SIDES:
        total = sum(cell[0] for cell in taken[side])
        if not total:
            continue
        for label, cell in zip(BINS, taken[side]):
            if cell[0]:
                out.append(f"    {side:<10} {label:>7} {cell[0]:>6.0f} {cell[0] / total:>6.2f} "
                           f"{cell[1] / cell[0]:>8.1f} {cell[3]:>6.0f} {cell[3] / cell[0]:>10.2f}")
    for name, table in (("raw turn", "major_gap_by_turn"), ("Online-equivalent turn", "major_gap_by_online_turn")):
        cells = [f"{label} {_num(gap / n if n else None, '+.1f')} (n {n:.0f})"
                 for label, (n, gap) in zip(TURN_BUCKETS, ledger[table])]
        out.append(f"    rival-major mean base gap by {name}: " + " | ".join(cells))
    for name, table in (("TOTAL gap (host preview)", "struck_total"), ("BASE gap", "struck_base")):
        out.append(f"(b) our blows on units by {name}:")
        out.append(f"    {'side':<10} {'gap':>7} {'blows':>6} {'share':>6} {'dealt/blow':>10} "
                   f"{'taken/blow':>10} {'kills':>5} {'kill/blow':>9} {'our deaths':>10}")
        for side in SIDES:
            rows = ledger[table][side]
            total = sum(cell[0] for cell in rows)
            if not total:
                continue
            for label, cell in zip(BINS, rows):
                if cell[0]:
                    out.append(f"    {side:<10} {label:>7} {cell[0]:>6.0f} {cell[0] / total:>6.2f} "
                               f"{cell[1] / cell[0]:>10.1f} {cell[2] / cell[0]:>10.1f} "
                               f"{cell[3]:>5.0f} {cell[3] / cell[0]:>9.2f} {cell[4]:>10.0f}")
    if ledger["struck_without_preview"]:
        out.append(f"    ({ledger['struck_without_preview']} of our blows carried no preview "
                   "and are in the base table only)")
    out.append("(c) upgrades and (d) strength at Online-equivalent marks (per-game means; gold medians):")
    out.append(f"    {'mark':>4} {'games':>5} {'army':>5} {'unlocked':>8} {'+material':>9} {'offer':>6} "
               f"{'gain':>6} {'gold':>6} {'gpt':>6} {'ours':>6} {'rivals':>6} {'gap':>6}")
    for mark in MARKS:
        rows = ledger["marks"][str(mark)]
        if not rows:
            continue
        s = mark_summary(rows)
        out.append(
            f"    t{mark:<3} {s['games']:>5} {_num(s['army'], '.1f'):>5} "
            f"{_num(s['unlocked_share'], '.2f'):>8} {_num(s['with_material_share'], '.2f'):>9} "
            f"{_num(s['offer_share'], '.2f'):>6} {_num(s['gain'], '+.1f'):>6} "
            f"{_num(s['gold_median'], '.0f'):>6} {_num(s['gold_per_turn_median'], '.1f'):>6} "
            f"{_num(s['our_strength'], '.1f'):>6} {_num(s['rival_strength'], '.1f'):>6} "
            f"{_num(s['strength_gap'], '+.1f'):>6}"
        )
    if ledger["summaries"]:
        out.append(f"    summary.json combat: kills {ledger['kills']} losses {ledger['losses']} "
                   f"(kill:loss {ledger['kills'] / max(ledger['losses'], 1):.2f}), cities lost "
                   f"{ledger['cities_lost'] / ledger['summaries']:.2f} a game, over {ledger['summaries']} run(s)")
    if ledger["unmapped"]:
        top = sorted(ledger["unmapped"].items(), key=lambda item: -item[1])[:8]
        out.append("    unmapped unit types (blow skipped): "
                   + ", ".join(f"{kind} {n}" for kind, n in top))
    if ledger["speed_unknown_runs"]:
        out.append(f"    {ledger['speed_unknown_runs']} run(s) of unknown speed: raw buckets only, no marks")
    return "\n".join(out)


def metrics(ledger: dict[str, Any]) -> list[tuple[str, float | None]]:
    """The short list a live lane and a simulated batch are set side by side
    on. `gene_screen`'s `edge_ledger::metrics` prints the same names in the
    same order; keep the two in step."""
    out: list[tuple[str, float | None]] = []

    def ratio(a: float, b: float) -> float | None:
        return a / b if b > 0 else None

    major = ledger["taken_base"]["major"]
    major_blows = sum(cell[0] for cell in major)
    out.append(("major_blows_per_game", ratio(major_blows, ledger["runs"])))
    out.append(("major_share_ge10", ratio(major[4][0], major_blows)))
    out.append(("major_death_per_blow_ge10", ratio(major[4][3], major[4][0])))
    out.append(("major_death_per_blow_lt10",
                ratio(sum(cell[3] for cell in major[:4]), sum(cell[0] for cell in major[:4]))))
    for label, (n, gap) in zip(("0_49", "50_99", "100_149", "150"), ledger["major_gap_by_online_turn"]):
        out.append((f"major_gap_{label}", ratio(gap, n)))
    struck = ledger["struck_total"]["major"]
    struck_blows = sum(cell[0] for cell in struck)
    out.append(("struck_major_share_le_m10", ratio(struck[0][0], struck_blows)))
    out.append(("struck_major_share_ge10", ratio(struck[4][0], struck_blows)))
    out.append(("struck_major_dmg_le_m10", ratio(struck[0][1], struck[0][0])))
    out.append(("struck_major_dmg_ge10", ratio(struck[4][1], struck[4][0])))
    out.append(("struck_major_kill_per_blow_ge10", ratio(struck[4][3], struck[4][0])))
    for mark in MARKS:
        s = mark_summary(ledger["marks"][str(mark)])
        for name, key in (("unlocked", "unlocked_share"), ("material", "with_material_share"),
                          ("offer", "offer_share"), ("gain", "gain"), ("gold", "gold_median"),
                          ("gpt", "gold_per_turn_median"), ("ours", "our_strength"),
                          ("rivals", "rival_strength"), ("strength_gap", "strength_gap")):
            out.append((f"{name}_{mark}", s[key]))
    out.append(("kill_loss", ratio(ledger["kills"], ledger["losses"])))
    out.append(("cities_lost_per_game", ratio(ledger["cities_lost"], ledger["summaries"])))
    return out


def render_metrics(ledger: dict[str, Any], title: str) -> str:
    """`metrics` as Rust tuples, ready to paste beside a simulated batch."""
    lines = [f"// {title}: {ledger['runs']} run(s)"]
    for name, value in metrics(ledger):
        lines.append(f'    ("{name}", {"f64::NAN" if value is None else format(value, ".3f")}),')
    return "\n".join(lines)


def run_line(ledger: dict[str, Any]) -> str:
    major = ledger["taken_base"]["major"]
    blows = sum(cell[0] for cell in major)
    deaths = sum(cell[3] for cell in major)
    high = major[4][0]
    gap_n = sum(n for n, _ in ledger["major_gap_by_turn"])
    gap = sum(g for _, g in ledger["major_gap_by_turn"]) / gap_n if gap_n else None
    struck = [cell for side in SIDES for cell in [ledger["struck_total"][side]]]
    ours = sum(cell[0] for rows in struck for cell in rows)
    low = sum(rows[0][0] for rows in struck)
    up = ledger["marks"]["100"]
    share = (up[0]["unlocked"] / up[0]["army"]) if up and up[0]["army"] else None
    return (f"{ledger['run']:<28} {ledger.get('lane', '?'):<44} major blows {blows:>4} "
            f">=+10 {high / blows if blows else 0:.2f} deaths {deaths:>3} mean gap {_num(gap, '+.1f'):>6} | "
            f"our blows {ours:>4} <=-10 {low / ours if ours else 0:.2f} | t100 unlocked {_num(share, '.2f')}")


def ledger_report(runs: list[Path], book: UnitBook, per_run: bool,
                  with_metrics: bool = False) -> tuple[str, dict[str, Any]]:
    pooled = empty_ledger()
    lanes: dict[str, dict[str, Any]] = {}
    per: list[dict[str, Any]] = []
    lines = []
    for run in runs:
        ledger = read_run(run, book)
        per.append(ledger)
        if per_run:
            lines.append(run_line(ledger))
        merge(pooled, ledger)
        lane = lanes.setdefault(ledger["lane"], empty_ledger())
        merge(lane, ledger)
    out = []
    if per_run and lines:
        out.append("per run:")
        out.extend("  " + line for line in lines)
    if len(runs) == 1:
        out.append(render_ledger(per[0], per[0]["run"] + " (" + per[0]["lane"] + ")"))
    else:
        out.append(render_ledger(pooled, "pooled"))
        for lane, ledger in sorted(lanes.items(), key=lambda item: -item[1]["runs"]):
            out.append(render_ledger(ledger, lane))
    if with_metrics:
        out.append(render_metrics(pooled, "pooled"))
        for lane, ledger in sorted(lanes.items(), key=lambda item: -item[1]["runs"]):
            out.append(render_metrics(ledger, lane))
    return "\n\n".join(out), {"pooled": pooled, "lanes": lanes, "runs": per}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("runs", nargs="*", type=Path, help="run directories; default: the control dir")
    ap.add_argument("--control-dir", type=Path, default=DEFAULT_CONTROL_DIR)
    ap.add_argument("--since", help="only control-dir runs named at or after this UTC prefix, e.g. 20261112")
    ap.add_argument("--until", help="only control-dir runs named before this UTC prefix, e.g. 20261113T0530")
    ap.add_argument("--min-mb", type=float, default=8.0,
                    help="skip control-dir runs whose events.jsonl is smaller (default 8, a game that played on)")
    ap.add_argument("--lane", help="only runs whose lane contains this text, e.g. ROME")
    ap.add_argument("--per-run", action="store_true", help="one line per run before the pooled tables")
    ap.add_argument("--metrics", action="store_true",
                    help="also print the side-by-side metric list (gene_screen's edge_ledger::metrics) as Rust tuples")
    ap.add_argument("--json", type=Path, help="write the ledgers as JSON")
    args = ap.parse_args(argv)
    runs = args.runs or control_runs(args.control_dir, args.since, args.min_mb, args.until)
    runs = [run for run in runs if (run / "events.jsonl").is_file()]
    if not runs:
        print("no run with an events.jsonl", file=sys.stderr)
        return 1
    book = UnitBook.load()
    if args.lane:
        kept = []
        for run in runs:
            seat = None
            with (run / "events.jsonl").open("rb") as handle:
                for number, line in enumerate(handle):
                    if b'"kind": "seat"' in line and len(line) < 20000:
                        seat = json.loads(line)
                        break
                    if number > 2000:
                        break
            if args.lane in lane_of(seat, _summary(run)):
                kept.append(run)
        runs = kept
    text, data = ledger_report(runs, book, args.per_run, args.metrics)
    print(text)
    if args.json:
        args.json.write_text(json.dumps(data, indent=1, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
