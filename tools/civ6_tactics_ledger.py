#!/usr/bin/env python3
"""The tactical ledger of one live Civilization VI run: how the army moved and fought.

Every earlier reading of the live army was a reconstruction — units that
vanished from one ``state`` to the next, attack orders whose target looked
undamaged a turn later, a Hall of Fame opened by hand. This tool is the one
place those numbers come from, so the next report on unit tactics cites a
ledger rather than a tally script.

It reads a run directory (``events.jsonl`` and ``orders.sqlite``) and prints:

* **orders** — unit orders per turn, and how many rode the per-unit queue
  (``orders.queued`` / ``orders_queue``, PR #2107); strikes planned versus
  strikes that landed on the turn they were planned (``strikes_landed``);
* **arrival** — every ``MOVE_TO`` judged against the unit's position on the
  next exported frame: arrived, short, did not move (with the movement the
  export recorded at issue), gone; by unit kind;
* **combat** — from the mod's ``combat`` events (attacker, defender, damage
  both ways, kill), when the mod that recorded the run emitted them: our
  attacks and the attacks we received, damage dealt and taken, kills and
  losses, city strikes; and the host's own strike preview
  (``CombatManager.SimulateAttackInto``) against what the strike did;
* **roster** — our military units that left the board and what stood beside
  them when last seen (a hostile within two tiles, an enemy city, nothing);
* **hover** — military unit-turns 2–4 tiles from a visible hostile that neither
  moved nor struck (the ``strike_opening`` measurement, per run);
* **hall of fame** — with ``--hof``, the host's own kills/losses/captures for
  the local player of that game.

A section whose events the recording mod did not emit says so
(``(mod predates the ledger)``) instead of printing zeros: a zero that means
"unmeasured" is the mistake this repository keeps paying for.

Usage::

    python3 tools/civ6_tactics_ledger.py <run-dir> [--hof HallofFame.sqlite] [--json out.json]
    python3 tools/civ6_tactics_ledger.py --latest    # newest run under the control dir
"""

from __future__ import annotations

import argparse
import collections
import json
import sqlite3
import sys
from pathlib import Path
from typing import Any, Iterable

sys.path.insert(0, str(Path(__file__).resolve().parent))

DEFAULT_CONTROL_DIR = Path.home() / "civvis-civ6-runs" / "control"


# ------------------------------------------------------------------ geometry
def _oddr_to_cube(x: int, y: int) -> tuple[int, int, int]:
    q = x - (y - (y & 1)) // 2
    return q, y, -q - y


def hex_distance(a: tuple[int, int], b: tuple[int, int]) -> int:
    """Distance between two host OFFSET (odd-r) plots, in hexes."""
    ax, ay, az = _oddr_to_cube(*a)
    bx, by, bz = _oddr_to_cube(*b)
    return max(abs(ax - bx), abs(ay - by), abs(az - bz))


# -------------------------------------------------------------------- reading
def read_events(path: Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    if not path.is_file():
        return events
    with path.open("r", errors="replace") as handle:
        for raw in handle:
            try:
                event = json.loads(raw)
            except ValueError:
                continue
            if isinstance(event, dict):
                events.append(event)
    return events


def read_unit_orders(path: Path) -> list[tuple[int, int, int, str, Any, Any]]:
    """``(turn, seq, subject, verb, x, y)`` for every unit order the brain wrote."""
    if not path.is_file():
        return []
    try:
        con = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
        rows = con.execute(
            "SELECT turn, seq, subject, verb, x, y FROM orders "
            "WHERE kind = 'unit' ORDER BY turn, seq"
        ).fetchall()
        con.close()
    except sqlite3.Error:
        return []
    out = []
    for turn, seq, subject, verb, x, y in rows:
        try:
            out.append((int(turn), int(seq), int(subject), str(verb or ""), x, y))
        except (TypeError, ValueError):
            continue
    return out


def _states(events: Iterable[dict[str, Any]]) -> dict[int, dict[str, Any]]:
    """The board at the START of each turn, keyed by turn.

    ⚠⚠⚠ THE FIRST FRAME, NOT THE LAST. This kept the LAST `state` of each turn,
    and 137 of 150 turns in an ordinary run carry more than one — the mid-turn
    replan and combat frames each export another. Every caller here uses the
    entry as the board an order was decided FROM, so the last frame is the board
    AFTER that order already moved the unit.

    The cost was not subtle. `self_tile` counts a MOVE_TO whose destination
    equals the unit's position, and judged against the last frame that is every
    order that ARRIVED. Run `civvis-20260830T121826Z`, first move per unit-turn,
    698 orders with both frames known:

        judged against the turn's LAST state    574 self_tile  (82%)
        judged against the turn's FIRST state     0 self_tile  ( 0%)

    Not one order in that game actually named the tile its unit already stood
    on. Worse, `self_tile` orders are skipped before the arrival verdict, so the
    reported "arrived 16.8%" was computed after discarding 82% of the orders
    that had arrived — the metric said the seat could not move when it was
    moving normally.
    """
    states: dict[int, dict[str, Any]] = {}
    for event in events:
        if event.get("kind") == "state" and isinstance(event.get("turn"), int):
            states.setdefault(event["turn"], event)
    return states


def _own_units(state: dict[str, Any]) -> dict[int, dict[str, Any]]:
    out = {}
    for unit in state.get("units") or []:
        try:
            out[int(unit["id"])] = unit
        except (KeyError, TypeError, ValueError):
            continue
    return out


def _hostile_plots(state: dict[str, Any]) -> list[tuple[int, int]]:
    """Visible hostile combat units and at-war actor cities, as offset plots.

    The host exports major civilizations in ``rivals`` and city-states in
    ``minors``.  Both lists carry the local player's ``at_war`` relation, while
    ``hostiles`` is reserved for barbarians and Free Cities.  Omitting minors
    made a city-state army disappear from the loss/hover context even though
    the same units were visible to the controller's contact guard.
    """
    plots: list[tuple[int, int]] = []
    for hostile in state.get("hostiles") or []:
        combat = hostile.get("combat") or 0
        ranged = hostile.get("ranged") or 0
        if (combat or ranged) and isinstance(hostile.get("x"), int):
            plots.append((hostile["x"], hostile["y"]))

    for actor_group in ("rivals", "minors"):
        for actor in state.get(actor_group) or []:
            if not actor.get("at_war"):
                continue
            for unit in actor.get("units") or []:
                if ((unit.get("combat") or 0) or (unit.get("ranged") or 0)) and isinstance(
                    unit.get("x"), int
                ):
                    plots.append((unit["x"], unit["y"]))
            for city in actor.get("cities") or []:
                if isinstance(city.get("x"), int):
                    plots.append((city["x"], city["y"]))
    return plots


def _is_military(unit: dict[str, Any]) -> bool:
    return bool((unit.get("combat") or 0) > 0 or (unit.get("ranged") or 0) > 0)


# ------------------------------------------------------------------- sections
def orders_section(events: list[dict[str, Any]], unit_orders: list) -> dict[str, Any]:
    turns = sorted({turn for turn, *_ in unit_orders})
    per_turn = collections.Counter(turn for turn, *_ in unit_orders)
    per_unit_turn = collections.Counter((turn, subject) for turn, _, subject, *_ in unit_orders)
    verbs = collections.Counter(verb for *_, verb, _x, _y in unit_orders)
    queued = 0
    queue_events = 0
    strikes_planned = strikes_landed = queue_applied = queue_refused = 0
    queue_refusals: collections.Counter = collections.Counter()
    saw_queue_field = False
    queue_drained = 0
    for event in events:
        kind = event.get("kind")
        if kind == "orders" and "queued" in event:
            saw_queue_field = True
            queued += int(event.get("queued") or 0)
        elif kind == "orders_queue":
            queue_events += 1
            strikes_planned += int(event.get("strikes_planned") or 0)
            strikes_landed += int(event.get("strikes_landed") or 0)
            queue_applied += int(event.get("applied") or 0)
            queue_refused += int(event.get("refused") or 0)
            # ⚠ The drain event carries its OWN `queued`, and that is the only
            # number `applied` and `refused` can honestly be read against.
            queue_drained += int(event.get("queued") or 0)
            for why, count in (event.get("refusals") or {}).items():
                queue_refusals[str(why)] += int(count or 0)
    return {
        "turns_with_unit_orders": len(turns),
        "unit_orders": len(unit_orders),
        "unit_orders_per_turn": round(len(unit_orders) / len(turns), 2) if turns else 0.0,
        "unit_turns_with_more_than_one_order": sum(1 for n in per_unit_turn.values() if n > 1),
        "verbs": dict(verbs.most_common()),
        "queue": None
        if not saw_queue_field
        else {
            "queued_followups": queued,
            "drained": queue_drained,
            "orders_queue_events": queue_events,
            "applied": queue_applied,
            "refused": queue_refused,
            "refusals": dict(queue_refusals.most_common()),
            "strikes_planned": strikes_planned,
            "strikes_landed": strikes_landed,
        },
        "_max_per_turn": max(per_turn.values()) if per_turn else 0,
    }


def arrival_section(events: list[dict[str, Any]], unit_orders: list) -> dict[str, Any]:
    states = _states(events)
    outcome: collections.Counter = collections.Counter()
    by_kind: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    no_move_with_moves = 0
    no_move_without_moves = 0
    first_move_seen: set[tuple[int, int]] = set()
    for turn, _seq, subject, verb, x, y in unit_orders:
        if verb != "MOVE_TO" or not isinstance(x, int) or not isinstance(y, int):
            continue
        # Judge only a unit's FIRST move of the turn: a sequenced follow-up move
        # was planned from where an act leaves the unit, not from the export.
        if (turn, subject) in first_move_seen:
            continue
        first_move_seen.add((turn, subject))
        before = _own_units(states.get(turn, {})).get(subject)
        after_state = states.get(turn + 1)
        if before is None or after_state is None:
            outcome["unjudged"] += 1
            continue
        kind = str(before.get("kind") or "?")
        start = (int(before["x"]), int(before["y"]))
        want = (x, y)
        if start == want:
            outcome["self_tile"] += 1
            by_kind[kind]["self_tile"] += 1
            continue
        after = _own_units(after_state).get(subject)
        if after is None:
            outcome["gone"] += 1
            by_kind[kind]["gone"] += 1
            continue
        end = (int(after["x"]), int(after["y"]))
        if end == want:
            outcome["arrived"] += 1
            by_kind[kind]["arrived"] += 1
        elif end == start:
            outcome["did_not_move"] += 1
            by_kind[kind]["did_not_move"] += 1
            if (before.get("moves") or 0) > 0:
                no_move_with_moves += 1
            else:
                no_move_without_moves += 1
        elif hex_distance(end, want) < hex_distance(start, want):
            outcome["short"] += 1
            by_kind[kind]["short"] += 1
        else:
            outcome["elsewhere"] += 1
            by_kind[kind]["elsewhere"] += 1
    judged = sum(outcome[k] for k in ("arrived", "short", "did_not_move", "elsewhere", "gone"))
    return {
        "moves_judged": judged,
        "outcomes": dict(outcome),
        "arrived_share": round(outcome["arrived"] / judged, 3) if judged else None,
        "did_not_move_share": round(outcome["did_not_move"] / judged, 3) if judged else None,
        "did_not_move_with_moves_at_export": no_move_with_moves,
        "did_not_move_without_moves_at_export": no_move_without_moves,
        "by_kind": {kind: dict(c) for kind, c in sorted(by_kind.items())},
    }


def city_occupations(
    events: list[dict[str, Any]], local_player: int | None
) -> tuple[int, int]:
    """`(taken, lost)` cities, from the mod's `city_occupation` events.

    The mod has emitted these since the tactical ledger landed and nothing
    has ever read them, so no report has been able to say whether a war
    ended in a capture — the question eleven declared wars and four sieges
    to 180-190/200 were waiting on.
    """
    taken = lost = 0
    for event in events:
        if event.get("kind") != "city_occupation" or local_player is None:
            continue
        ours_now = event.get("ours_now")
        was_ours = event.get("original_owner") == local_player
        if ours_now is True and not was_ours:
            taken += 1
        elif ours_now is False and was_ours:
            lost += 1
    return taken, lost


def combat_section(events: list[dict[str, Any]], local_player: int | None) -> dict[str, Any] | None:
    combats = [event for event in events if event.get("kind") == "combat"]
    if not combats:
        return None
    ours: collections.Counter = collections.Counter()
    theirs: collections.Counter = collections.Counter()
    preview_error: list[float] = []
    kills_by_kind: collections.Counter = collections.Counter()
    losses_by_kind: collections.Counter = collections.Counter()
    for event in combats:
        attacker = event.get("attacker") or {}
        defender = event.get("defender") or {}
        we_attack = local_player is not None and attacker.get("player") == local_player
        we_defend = local_player is not None and defender.get("player") == local_player
        side = ours if we_attack else theirs if we_defend else None
        if side is None:
            continue
        side["combats"] += 1
        dealt = event.get("damage_to_defender")
        taken = event.get("damage_to_attacker")
        if isinstance(dealt, (int, float)):
            side["damage_dealt" if we_attack else "damage_taken"] += int(dealt)
        if isinstance(taken, (int, float)):
            side["damage_taken" if we_attack else "damage_dealt"] += int(taken)
        if defender.get("type") == "city" or defender.get("type") == "district":
            side["city_strikes"] += 1
        if event.get("defender_killed"):
            if we_attack:
                ours["kills"] += 1
                kills_by_kind[str(defender.get("kind") or "?")] += 1
            else:
                theirs["kills"] += 1
                losses_by_kind[str(defender.get("kind") or "?")] += 1
        if event.get("attacker_killed"):
            if we_attack:
                ours["losses_attacking"] += 1
                losses_by_kind[str(attacker.get("kind") or "?")] += 1
            else:
                theirs["losses_attacking"] += 1
                kills_by_kind[str(attacker.get("kind") or "?")] += 1
        preview = event.get("preview") or {}
        predicted = preview.get("damage_to_defender")
        if we_attack and isinstance(predicted, (int, float)) and isinstance(dealt, (int, float)):
            comparable = float(predicted)
            # `SimulateAttackInto` reports the strike's potential damage, while
            # the combat callback reports the HP delta. Civ VI caps that delta
            # at the defender's remaining HP, so a lethal preview of 64 against
            # a 26-HP unit is an expected 26, not a -38 combat-model mismatch.
            # Cities and districts stay uncapped here: their preview also
            # carries wall damage while `damage_to_defender` is the garrison
            # readback, so the two fields are not the same quantity.
            defender_hp = defender.get("hp")
            if defender.get("type") == "unit" and isinstance(defender_hp, (int, float)):
                comparable = min(comparable, max(0.0, float(defender_hp)))
            preview_error.append(float(dealt) - comparable)
    kills = ours["kills"] + theirs["losses_attacking"]
    losses = theirs["kills"] + ours["losses_attacking"]
    mean_err = (sum(preview_error) / len(preview_error)) if preview_error else None
    cities_taken, cities_lost = city_occupations(events, local_player)
    return {
        "combats": len(combats),
        "our_attacks": ours["combats"],
        "attacks_received": theirs["combats"],
        "damage_dealt": ours["damage_dealt"] + theirs["damage_dealt"],
        "damage_taken": ours["damage_taken"] + theirs["damage_taken"],
        "kills": kills,
        "losses": losses,
        "kills_per_loss": round(kills / losses, 2) if losses else None,
        "city_strikes": ours["city_strikes"],
        "cities_taken": cities_taken,
        "cities_lost": cities_lost,
        "kills_by_kind": dict(kills_by_kind.most_common()),
        "losses_by_kind": dict(losses_by_kind.most_common()),
        "host_preview": None
        if not preview_error
        else {
            "strikes_previewed": len(preview_error),
            "mean_actual_minus_predicted": round(mean_err, 2),
            "within_20pct_of_30": sum(1 for e in preview_error if abs(e) <= 6),
        },
    }


#: A unit at or below this the last time we saw it was one the controller had
#: a turn's warning about — below `withdraw_hp` (45), the line its own
#: recovery uses. The arena's twin is `doctrine::SALVAGEABLE_HP`.
SALVAGEABLE_HP = 30


def roster_section(events: list[dict[str, Any]]) -> dict[str, Any]:
    states = _states(events)
    turns = sorted(states)
    gone: collections.Counter = collections.Counter()
    gone_by_kind: collections.Counter = collections.Counter()
    salvageable = 0
    for i, turn in enumerate(turns[:-1]):
        now = _own_units(states[turn])
        nxt = _own_units(states[turns[i + 1]])
        hostiles = _hostile_plots(states[turn])
        gold = states[turn].get("gold")
        for uid, unit in now.items():
            if uid in nxt or not _is_military(unit):
                continue
            kind = str(unit.get("kind") or "?")
            gone_by_kind[kind] += 1
            pos = (int(unit["x"]), int(unit["y"]))
            near = any(hex_distance(pos, plot) <= 2 for plot in hostiles)
            if (unit.get("hp") or 0) <= SALVAGEABLE_HP:
                salvageable += 1
            if (unit.get("hp") or 0) >= 100 and isinstance(gold, (int, float)) and gold <= 0:
                gone["full_hp_treasury_empty"] += 1
            elif near:
                gone["hostile_within_2"] += 1
            else:
                gone["no_visible_threat"] += 1
    lost = sum(gone.values())
    return {
        "military_units_gone": lost,
        # ⭐ HOW MANY OF THEM THE SEAT SAW COMING. A unit last seen at or
        # below `SALVAGEABLE_HP` is one the controller had a turn's warning
        # about and could have rotated, withdrawn or healed out of; the rest
        # were killed from a health it had no reason to act on. The two are
        # different failures and only one of them is worth a preservation
        # change. The arena reports the same share as `salvag.`.
        "lost_when_salvageable": salvageable,
        "salvageable_share": round(salvageable / lost, 2) if lost else None,
        "context_at_last_sight": dict(gone),
        "by_kind": dict(gone_by_kind.most_common()),
    }


def hover_section(events: list[dict[str, Any]], unit_orders: list) -> dict[str, Any]:
    states = _states(events)
    turns = sorted(states)
    strikes_by_turn_unit = {
        (turn, subject)
        for turn, _seq, subject, verb, *_ in unit_orders
        if verb in ("ATTACK", "RANGE_ATTACK")
    }
    military_turns = 0
    near_turns = 0
    hover = 0
    fortified_hover = 0
    healing_hover = 0
    for i, turn in enumerate(turns[:-1]):
        now = _own_units(states[turn])
        nxt = _own_units(states[turns[i + 1]])
        hostiles = _hostile_plots(states[turn])
        for uid, unit in now.items():
            if not _is_military(unit):
                continue
            military_turns += 1
            pos = (int(unit["x"]), int(unit["y"]))
            dist = min((hex_distance(pos, plot) for plot in hostiles), default=None)
            if dist is None or not (2 <= dist <= 4):
                continue
            near_turns += 1
            after = nxt.get(uid)
            moved = after is not None and (int(after["x"]), int(after["y"])) != pos
            if not moved and (turn, uid) not in strikes_by_turn_unit:
                hover += 1
                # ⚠ A FORTIFIED UNIT HOLDING GROUND IS NOT HOVERING. "Neither
                # moved nor struck" is exactly what a defender is ordered to do,
                # and the run carries 333 FORTIFY orders, so the raw count mixes
                # deliberate defence with idleness. Measured on run
                # civvis-20260830T121826Z: of 105 hovering unit-turns only 17
                # were fortified — the other 88 are a military unit standing
                # two to four tiles from a hostile, unfortified, doing nothing.
                # That 88 is the number worth acting on.
                if unit.get("fortified"):
                    fortified_hover += 1
                elif (unit.get("hp") or 100) < 100:
                    # ⚠⚠ A DAMAGED UNIT RESTING IS HEALING, NOT LOITERING. Civ 6
                    # heals a unit that neither moves nor attacks, so "did
                    # nothing beside an enemy" is also the description of a
                    # wounded unit doing the right thing. On run
                    # civvis-20260830T121826Z these are 37 of 105 hovering
                    # unit-turns — more than the fortified ones — and #2816's
                    # split reported all 88 non-fortified as "idle", which
                    # overstated the defect by better than 2x.
                    healing_hover += 1
    return {
        "military_unit_turns": military_turns,
        "unit_turns_2_to_4_from_a_hostile": near_turns,
        "hovering_unit_turns": hover,
        "hovering_fortified": fortified_hover,
        "hovering_healing": healing_hover,
        # ⚠ NOT "idle" in any stronger sense than "we cannot name a reason".
        # `moves` is NOT usable to narrow this further: at the first frame of a
        # turn it reads 0 for 1156 of ~1350 military units, so it is the
        # export's default rather than evidence about that unit.
        "hovering_unexplained": hover - fortified_hover - healing_hover,
        "hover_share_of_near": round(hover / near_turns, 3) if near_turns else None,
    }


def hall_of_fame_section(path: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    wanted = (
        "UnitsLost",
        "UnitsKilled",
        "Combats",
        "CitiesConquered",
        "CitiesLost",
        "WarsDeclared",
        "BarbarianCampsCleared",
        "BarbariansKilled",
    )
    try:
        con = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
        rows = con.execute(
            "SELECT p.GameId, v.DataPoint, v.ValueNumeric "
            "FROM ObjectDataPointValues v JOIN GamePlayers p ON p.PlayerObjectId = v.ObjectId "
            "WHERE p.IsLocal = 1 AND v.DataPoint IN (%s)" % ",".join("?" for _ in wanted),
            wanted,
        ).fetchall()
        con.close()
    except sqlite3.Error:
        try:
            # Older schema without GameId on GamePlayers.
            con = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
            rows = con.execute(
                "SELECT p.PlayerObjectId, v.DataPoint, v.ValueNumeric "
                "FROM ObjectDataPointValues v JOIN GamePlayers p ON p.PlayerObjectId = v.ObjectId "
                "WHERE p.IsLocal = 1 AND v.DataPoint IN (%s)" % ",".join("?" for _ in wanted),
                wanted,
            ).fetchall()
            con.close()
        except sqlite3.Error:
            return None
    games: dict[Any, dict[str, float]] = collections.defaultdict(dict)
    for game, point, value in rows:
        try:
            games[game][str(point)] = float(value or 0)
        except (TypeError, ValueError):
            continue
    if not games:
        return None
    latest = games[sorted(games, key=lambda g: str(g))[-1]]
    totals: collections.Counter = collections.Counter()
    for game in games.values():
        for point, value in game.items():
            totals[point] += value
    return {
        "games": len(games),
        "latest": {k: int(v) for k, v in latest.items()},
        "totals": {k: int(v) for k, v in totals.items()},
        "kills_per_loss_all_games": round(totals["UnitsKilled"] / totals["UnitsLost"], 2)
        if totals["UnitsLost"]
        else None,
    }


# --------------------------------------------------------------------- report
def ledger(run_dir: Path, hof: Path | None = None) -> dict[str, Any]:
    events = read_events(run_dir / "events.jsonl")
    unit_orders = read_unit_orders(run_dir / "orders.sqlite")
    local_player = None
    for event in events:
        if event.get("kind") == "seat" and isinstance(event.get("local_player"), int):
            local_player = event["local_player"]
            break
    states = _states(events)
    report: dict[str, Any] = {
        "run": run_dir.name,
        "turns_recorded": len(states),
        "orders": orders_section(events, unit_orders),
        "arrival": arrival_section(events, unit_orders),
        "combat": combat_section(events, local_player),
        "roster": roster_section(events),
        "hover": hover_section(events, unit_orders),
    }
    if hof is not None:
        report["hall_of_fame"] = hall_of_fame_section(hof)
    return report


def _fmt_share(value: float | None) -> str:
    return "n/a" if value is None else f"{100 * value:.1f}%"


def render(report: dict[str, Any]) -> str:
    lines = [f"{report['run']} — {report['turns_recorded']} exported turns"]
    orders = report["orders"]
    lines.append(
        f"  orders   {orders['unit_orders']} unit orders on {orders['turns_with_unit_orders']} turns "
        f"({orders['unit_orders_per_turn']}/turn); "
        f"{orders['unit_turns_with_more_than_one_order']} unit-turns with >1 order"
    )
    top_verbs = ", ".join(f"{v} {n}" for v, n in list(orders["verbs"].items())[:6])
    lines.append(f"           verbs: {top_verbs}")
    queue = orders["queue"]
    if queue is None:
        lines.append("           queue: (mod predates the per-unit order queue)")
    else:
        # ⚠⚠⚠ TWO DIFFERENT STREAMS, AND PRINTING THEM AS A RATIO INVITED THE
        # WRONG READING. `queued_followups` sums the `queued` field of `orders`
        # events (410 of them in run civvis-20260830T121826Z, total 865);
        # `applied` and `refused` come from the far rarer `orders_queue` drain
        # events (82 of them). Side by side that read as "865 queued, 148
        # applied" — an 82% loss that does not exist. Within the drain stream
        # the same run is queued 159, applied 148, refused 11: **93% applied**.
        #
        # So the drain is reported against its own queued count, and the
        # decider-side total is named separately as what it is.
        lines.append(
            f"           queue: {queue['drained']} follow-ups drained, "
            f"{queue['applied']} applied, {queue['refused']} refused; "
            f"strikes planned {queue['strikes_planned']}, landed same turn {queue['strikes_landed']}"
        )
        lines.append(
            f"           (the decider reported queuing {queue['queued_followups']} "
            f"across its own order events — a different stream, not this one's denominator)"
        )
        if queue["refusals"]:
            lines.append(
                "           queue refusals: "
                + ", ".join(f"{k} {n}" for k, n in list(queue["refusals"].items())[:6])
            )
    arrival = report["arrival"]
    lines.append(
        f"  arrival  {arrival['moves_judged']} first moves judged: arrived {_fmt_share(arrival['arrived_share'])}, "
        f"did not move {_fmt_share(arrival['did_not_move_share'])} "
        f"({arrival['did_not_move_with_moves_at_export']} with movement at export, "
        f"{arrival['did_not_move_without_moves_at_export']} without); "
        + ", ".join(f"{k} {n}" for k, n in arrival["outcomes"].items())
    )
    combat = report["combat"]
    if combat is None:
        lines.append("  combat   (mod predates the ledger: no combat events)")
    else:
        lines.append(
            f"  combat   {combat['combats']} combats: ours {combat['our_attacks']}, received {combat['attacks_received']}; "
            f"kills {combat['kills']}, losses {combat['losses']} "
            f"(kills/loss {combat['kills_per_loss'] if combat['kills_per_loss'] is not None else 'n/a'}); "
            f"damage dealt {combat['damage_dealt']}, taken {combat['damage_taken']}; city strikes {combat['city_strikes']}; "
            f"cities taken {combat['cities_taken']}, lost {combat['cities_lost']}"
        )
        preview = combat["host_preview"]
        if preview:
            lines.append(
                f"           host preview on {preview['strikes_previewed']} strikes: "
                f"actual−predicted mean {preview['mean_actual_minus_predicted']:+.2f}, "
                f"{preview['within_20pct_of_30']} within ±6"
            )
    roster = report["roster"]
    lines.append(
        f"  roster   {roster['military_units_gone']} military units left the board: "
        + ", ".join(f"{k} {n}" for k, n in roster["context_at_last_sight"].items())
    )
    if roster["military_units_gone"]:
        lines.append(
            f"           {roster['lost_when_salvageable']} were last seen at or below "
            f"{SALVAGEABLE_HP} hp ({_fmt_share(roster['salvageable_share'])}) — the losses the "
            f"seat had a turn's warning of"
        )
    hover = report["hover"]
    lines.append(
        f"  hover    {hover['hovering_unit_turns']} of {hover['unit_turns_2_to_4_from_a_hostile']} "
        f"near-hostile unit-turns neither moved nor struck ({_fmt_share(hover['hover_share_of_near'])}"
        f" — {hover['hovering_fortified']} fortified, {hover['hovering_healing']} healing, "
        f"{hover['hovering_unexplained']} unexplained); "
        f"{hover['military_unit_turns']} military unit-turns"
    )
    hof = report.get("hall_of_fame")
    if "hall_of_fame" in report:
        if hof is None:
            lines.append("  hall of fame: unreadable or empty")
        else:
            latest = hof["latest"]
            lines.append(
                f"  hall of fame ({hof['games']} local games): latest lost {latest.get('UnitsLost', 0)} / "
                f"killed {latest.get('UnitsKilled', 0)}, conquered {latest.get('CitiesConquered', 0)}, "
                f"camps {latest.get('BarbarianCampsCleared', 0)}; all games kills/loss "
                f"{hof['kills_per_loss_all_games'] if hof['kills_per_loss_all_games'] is not None else 'n/a'}"
            )
    return "\n".join(lines)


def latest_run(control_dir: Path) -> Path | None:
    candidates = [
        path
        for path in control_dir.iterdir()
        if path.is_dir() and (path / "events.jsonl").is_file()
    ] if control_dir.is_dir() else []
    if not candidates:
        return None
    return max(candidates, key=lambda path: (path / "events.jsonl").stat().st_mtime)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("run", nargs="?", type=Path, help="run directory (events.jsonl + orders.sqlite)")
    ap.add_argument("--latest", action="store_true", help="the newest run under the control dir")
    ap.add_argument("--control-dir", type=Path, default=DEFAULT_CONTROL_DIR)
    ap.add_argument("--hof", type=Path, help="the host's HallofFame.sqlite, for kills/losses/captures")
    ap.add_argument("--json", type=Path, help="write the full ledger as JSON")
    args = ap.parse_args(argv)

    run_dir = args.run
    if args.latest or run_dir is None:
        run_dir = latest_run(args.control_dir)
        if run_dir is None:
            print(f"no run with events.jsonl under {args.control_dir}", file=sys.stderr)
            return 2
    if not run_dir.is_dir():
        print(f"not a run directory: {run_dir}", file=sys.stderr)
        return 2
    report = ledger(run_dir, args.hof)
    print(render(report))
    if args.json:
        args.json.write_text(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
