#!/usr/bin/env python3
"""Every settler the live seat lost to a barbarian, detected, attributed, written up.

The operator's standing rule: *every single time a barbarian captures a settler,
deeply analyze what happened and fix it. It is a game-changing mistake.* On
2026-08-28 the live seat lost 24 settlers across ten runs and no ledger column
showed it, because the only witness was a `unit_lost` line that looks exactly
like a settler founding a city. This module is the instrument side of that
rule: it finds each capture, reconstructs the two turns before it from the
event stream and the brain's journal, names the mechanism, and counts it in the
ladder row.

Detection, in order of trust:

1. `unit_captured` — the mod's own `Events.UnitCaptured` handler (modelled on
   `Base/Assets/UI/Popups/UnitCaptured.lua:8`, `OnUnitCaptured(currentUnitOwner,
   unit, owningPlayer, capturingPlayer)`). Exact: it names the captor.
2. `unit_lost` of `UNIT_SETTLER` with no `found` for that unit — the
   heuristic that found all of the 2026-08-28 losses on runs recorded before
   the precise event existed. A settler that founds a city is ALSO removed from
   the map (`docs/ELO_REPINS.md`: "`unit_lost` when a Settler founds a city"),
   which is why the `found` event has to be subtracted. A loss on or after the
   game's terminal event is the empire being dissolved, not a capture.

Each capture reports which method fired, so a dossier from the heuristic is
never mistaken for one from the game's own word.

Usage:

    python3 tools/civ6_settler_captures.py <run-dir> [--json] [--markdown]
        [--ledger <jsonl>]
    python3 tools/civ6_settler_captures.py --all <control-dir> [--match GLOB]

`--ledger` appends one JSON line per capture, idempotently (a row already
present for that run and unit is skipped). `--all` prints a per-run census.
`civ6_civvis_climb.py` imports `detect_captures` for the ladder row's
`settlers_captured` column and `render_markdown` for the per-run dossier file.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

# Recon-class hostiles. A barbarian scout has 10 strength and three moves, and
# the brain treated it as harmless for the whole of 2026-08-28; the class is
# what the classifier names, so a Skirmisher counts the same way.
RECON_TYPES = {
    "UNIT_SCOUT", "UNIT_SKIRMISHER", "UNIT_RANGER", "UNIT_SPEC_OPS",
    "UNIT_BARBARIAN_HORSEMAN", "UNIT_BARBARIAN_HORSE_ARCHER",
}
BARBARIAN_PLAYER = 63
WHY_LINE = re.compile(r"^\[why\] t(?P<turn>\d+) (?P<rest>.*)$")
WHY_KEYWORDS = re.compile(r"settler|guard|raider|escort|flee|capture", re.IGNORECASE)
WHY_CIV6_TARGET = re.compile(r"\[civ6 \((?P<x>-?\d+),(?P<y>-?\d+)\) = axial")
WHY_MARCH = re.compile(r"Settler (?:marching to|HELD short of|takes the nearest legal site at|"
                       r"takes a site the preferred search refused)", re.IGNORECASE)
FLED_WORDS = re.compile(r"flees|fled|out of reach|sidesteps", re.IGNORECASE)
ESCORT_KINDS = (
    "escort_cap_synced", "escort_cap_unresolved", "escort_shadow_injected",
    "settler_barbarian_combat_guard_rescue", "settler_barbarian_combat_capture_hold",
)
ORDER_KINDS = ("order_verified", "order_failed")
TERMINAL_KINDS = ("defeat", "victory", "gameover")
MECHANISMS = (
    "site-in-barbarian-nest", "barbarian-scout", "alone-in-fog", "weak-guard",
    "held-beside-raider", "fled-into-reach", "unclassified",
)


# ------------------------------------------------------------------ geometry
def cube(x: int, y: int) -> tuple[int, int, int]:
    """Civ 6 odd-r offset -> cube coordinates."""
    q = x - (y - (y & 1)) // 2
    r = y
    return q, r, -q - r


def hex_distance(a: tuple[int, int], b: tuple[int, int]) -> int:
    q1, r1, s1 = cube(*a)
    q2, r2, s2 = cube(*b)
    return max(abs(q1 - q2), abs(r1 - r2), abs(s1 - s2))


# --------------------------------------------------------------- run reading
@dataclass
class RunData:
    """What one run directory says, read once."""
    name: str
    states: dict[int, dict] = field(default_factory=dict)   # last frame per turn
    last_turn: int = 0
    terminal_turn: int | None = None
    settler_lost: list[dict] = field(default_factory=list)
    founded_units: set = field(default_factory=set)
    found_turns: list[int] = field(default_factory=list)
    captured: list[dict] = field(default_factory=list)      # unit_captured events
    escort: list[dict] = field(default_factory=list)
    orders: list[dict] = field(default_factory=list)
    refusals: list[dict] = field(default_factory=list)
    camps: set = field(default_factory=set)
    why: dict[int, list[str]] = field(default_factory=dict)
    has_events: bool = False

    def state_at(self, turn: int) -> dict | None:
        return self.states.get(turn)


def _int(value, default=None):
    try:
        return int(value)
    except (TypeError, ValueError):
        return default


def load_run(run_dir: Path) -> RunData:
    run_dir = Path(run_dir)
    data = RunData(name=run_dir.name)
    events = run_dir / "events.jsonl"
    if not events.is_file():
        return data
    data.has_events = True
    try:
        lines = events.read_text(errors="replace").splitlines()
    except OSError:
        return data
    for line in lines:
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if not isinstance(event, dict):
            continue
        kind = event.get("kind")
        turn = _int(event.get("turn"))
        if turn is not None and turn > data.last_turn:
            data.last_turn = turn
        if kind == "state":
            if turn is not None:
                data.states[turn] = event
        elif kind == "unit_lost":
            if event.get("unit_kind") == "UNIT_SETTLER":
                data.settler_lost.append(event)
        elif kind == "unit_captured":
            data.captured.append(event)
        elif kind == "found":
            if event.get("unit") is not None:
                data.founded_units.add(event.get("unit"))
            if turn is not None:
                data.found_turns.append(turn)
        elif kind in ESCORT_KINDS:
            data.escort.append(event)
        elif kind in ORDER_KINDS:
            data.orders.append(event)
        elif kind == "move_refused":
            data.refusals.append(event)
        elif kind in TERMINAL_KINDS:
            # ⚠ `defeat` is emitted for EVERY player's elimination: run
            # `civvis-20260829T000643Z` carries `defeat` for player 12 at t164
            # with `ours: false` and plays on to t172. Only our own end
            # dissolves our settlers.
            ours = event.get("ours")
            if kind == "defeat" and ours is not True and (
                    ours is False or event.get("player") != event.get("local_player")):
                continue
            if turn is not None and (data.terminal_turn is None or turn < data.terminal_turn):
                data.terminal_turn = turn
        elif kind in ("tiles", "tiles_delta"):
            plots = event.get("plots")
            for plot in plots if isinstance(plots, list) else []:
                if isinstance(plot, dict) and plot.get("im") == "IMPROVEMENT_BARBARIAN_CAMP":
                    data.camps.add((plot.get("x"), plot.get("y")))
    why = run_dir / "why.log"
    if why.is_file():
        try:
            for raw in why.read_text(errors="replace").splitlines():
                match = WHY_LINE.match(raw)
                if match:
                    data.why.setdefault(int(match.group("turn")), []).append(
                        match.group("rest").strip())
        except OSError:
            pass
    return data


# ------------------------------------------------------------------ analysis
def _unit_kind(unit: dict) -> str:
    return str(unit.get("kind") or unit.get("type") or "?")


def _find_unit(state: dict | None, uid) -> dict | None:
    if not state:
        return None
    for unit in state.get("units") or []:
        if unit.get("id") == uid:
            return unit
    return None


def _hostiles_near(state: dict | None, pos, radius: int) -> list[dict]:
    out = []
    for hostile in (state or {}).get("hostiles") or []:
        if hostile.get("x") is None or hostile.get("y") is None:
            continue
        distance = hex_distance(pos, (hostile["x"], hostile["y"]))
        if distance <= radius:
            out.append({
                "id": hostile.get("id"), "type": hostile.get("type"),
                "player": hostile.get("player"), "combat": hostile.get("combat"),
                "moves": hostile.get("moves"), "hp": hostile.get("hp"),
                "pos": [hostile["x"], hostile["y"]], "distance": distance,
            })
    out.sort(key=lambda h: (h["distance"], -(h["combat"] or 0)))
    return out


def _friendly_military_near(state: dict | None, pos, radius: int, exclude) -> list[dict]:
    out = []
    for unit in (state or {}).get("units") or []:
        if unit.get("id") == exclude or (unit.get("combat") or 0) <= 0:
            continue
        if unit.get("x") is None or unit.get("y") is None:
            continue
        distance = hex_distance(pos, (unit["x"], unit["y"]))
        if distance <= radius:
            out.append({
                "id": unit.get("id"), "type": _unit_kind(unit),
                "combat": unit.get("combat"), "hp": unit.get("hp"),
                "moves": unit.get("moves"), "pos": [unit["x"], unit["y"]],
                "distance": distance,
            })
    out.sort(key=lambda u: (u["distance"], -(u["combat"] or 0)))
    return out


def _is_recon(hostile: dict | None) -> bool:
    if not hostile:
        return False
    kind = str(hostile.get("type") or "")
    return kind in RECON_TYPES or "SCOUT" in kind


def _site_from_why(data: RunData, turns: list[int], near, max_distance: int = 12):
    """The marching target the journal named closest to this settler, if any.

    The journal does not name unit ids, so with two settlers afoot the line is
    matched on distance: the nearest target within `max_distance` of the
    settler's last position wins, and a target further than that is somebody
    else's.
    """
    best = None
    for turn in turns:
        for line in data.why.get(turn, []):
            if not WHY_MARCH.search(line):
                continue
            match = WHY_CIV6_TARGET.search(line)
            if not match:
                continue
            site = (int(match.group("x")), int(match.group("y")))
            distance = hex_distance(near, site)
            if distance <= max_distance and (best is None or distance < best[0]):
                best = (distance, site)
    return best[1] if best else None


def _guard_of(data: RunData, uid, turn: int, pos, state_before: dict | None) -> dict | None:
    """The settler's guard: the escort the host named, else who stood with it."""
    named = None
    for event in data.escort:
        if event.get("settler") != uid or event.get("guard") is None:
            continue
        event_turn = _int(event.get("turn"), -1)
        if event_turn <= turn and (named is None or event_turn >= named[0]):
            named = (event_turn, event.get("guard"), event.get("kind"))
    if named is not None:
        guard_unit = _find_unit(state_before, named[1])
        record = {"id": named[1], "named_by": named[2], "named_turn": named[0]}
        if guard_unit is not None and pos is not None:
            record.update({
                "type": _unit_kind(guard_unit), "combat": guard_unit.get("combat"),
                "hp": guard_unit.get("hp"), "moves": guard_unit.get("moves"),
                "pos": [guard_unit.get("x"), guard_unit.get("y")],
                "distance": hex_distance(pos, (guard_unit["x"], guard_unit["y"])),
            })
        else:
            record["distance"] = None
        return record
    if pos is None:
        return None
    nearby = _friendly_military_near(state_before, pos, 2, uid)
    if nearby:
        record = dict(nearby[0])
        record["named_by"] = "proximity"
        return record
    return None


def classify(frames: dict, why_before: list[str], guard: dict | None,
             site_hostile_seen: bool, camp_near_site: bool) -> tuple[str, list[str]]:
    """Name the mechanism; the first match in the operator's order is the verdict.

    `frames` maps relative turn offset (-2, -1, 0) to a dict with `pos`,
    `moves`, `hostiles` (within 3, sorted by distance) and `friendly`
    (military within 2). Every mechanism that matches is returned too, so a
    capture that is both a scout's work and a weak guard shows both.
    """
    matched: list[str] = []
    now = frames.get(0) or {}
    before = frames.get(-1) or {}
    if site_hostile_seen or camp_near_site:
        matched.append("site-in-barbarian-nest")
    nearest_now = (now.get("hostiles") or [None])[0]
    nearest_before = (before.get("hostiles") or [None])[0]
    for nearest in (nearest_now, nearest_before):
        if nearest and nearest["distance"] <= 2 and _is_recon(nearest):
            matched.append("barbarian-scout")
            break
    if before.get("pos") is not None and not before.get("hostiles") and not before.get("friendly"):
        matched.append("alone-in-fog")
    stacked = [u for u in before.get("friendly") or [] if u["distance"] == 0]
    if stacked:
        threat = nearest_before or nearest_now
        for unit in stacked:
            weak_hp = (unit.get("hp") or 0) < 50
            outgunned = threat is not None and (unit.get("combat") or 0) < (threat.get("combat") or 0)
            if weak_hp or outgunned:
                matched.append("weak-guard")
                break
    if (nearest_before and nearest_before["distance"] <= 1
            and before.get("pos") is not None and now.get("pos") == before.get("pos")
            and (before.get("moves") or 0) > 0):
        matched.append("held-beside-raider")
    if any(FLED_WORDS.search(line) for line in why_before):
        matched.append("fled-into-reach")
    mechanism = matched[0] if matched else "unclassified"
    return mechanism, matched


def _dossier(data: RunData, uid, turn: int, method: str, extra: dict) -> dict:
    # The settler is in the frame of its `unit_lost` turn (it is taken during the
    # barbarians' move, after ours) — measured on every 2026-08-28 loss. Should a
    # run ever record the loss one turn later, fall back to its last sighting.
    seen_turn = turn
    if _find_unit(data.state_at(turn), uid) is None:
        for candidate in range(turn - 1, max(turn - 4, -1), -1):
            if _find_unit(data.state_at(candidate), uid) is not None:
                seen_turn = candidate
                break
    frames: dict[int, dict] = {}
    last_pos = None
    for offset in (-2, -1, 0):
        t = seen_turn + offset
        state = data.state_at(t)
        me = _find_unit(state, uid)
        frame = {"turn": t, "present": me is not None, "pos": None, "moves": None,
                 "activity": None, "embarked": None, "hostiles": [], "friendly": []}
        if me is not None and me.get("x") is not None:
            pos = (me["x"], me["y"])
            last_pos = pos
            frame.update({
                "pos": [me["x"], me["y"]], "moves": me.get("moves"),
                "activity": me.get("activity"), "embarked": me.get("embarked"),
                "hostiles": _hostiles_near(state, pos, 3),
                "friendly": _friendly_military_near(state, pos, 2, uid),
            })
        elif state is None:
            frame["present"] = None   # no frame for that turn at all
        frames[offset] = frame
    pos = tuple(frames[0]["pos"]) if frames[0]["pos"] else last_pos
    state_before = data.state_at(seen_turn - 1)
    guard = _guard_of(data, uid, seen_turn, pos, state_before)
    guard_id = guard.get("id") if guard else None
    window = list(range(seen_turn - 2, seen_turn + 1))

    site = _site_from_why(data, window, pos) if pos is not None else None
    site_hostile_seen = False
    if site is not None:
        for t in range(seen_turn - 5, seen_turn + 1):
            if _hostiles_near(data.state_at(t), site, 3):
                site_hostile_seen = True
                break
    camp_near_site = site is not None and any(
        camp[0] is not None and hex_distance(site, camp) <= 3 for camp in data.camps)

    orders = [
        {k: v for k, v in event.items() if k not in ("run", "ctx")}
        for event in data.orders + data.refusals + data.escort
        if _int(event.get("turn"), -99) in window
        and (event.get("subject") in (uid, guard_id) or event.get("unit") in (uid, guard_id)
             or event.get("settler") == uid or event.get("guard") == guard_id)
    ]
    orders.sort(key=lambda e: (_int(e.get("turn"), 0), str(e.get("kind"))))
    why_lines = [
        f"t{t} {line}" for t in window for line in data.why.get(t, [])
        if WHY_KEYWORDS.search(line)
    ]
    why_before = data.why.get(seen_turn - 1, [])
    mechanism, matched = classify(frames, why_before, guard, site_hostile_seen, camp_near_site)
    nearest = (frames[0]["hostiles"] or frames[-1]["hostiles"] or [None])[0]
    return {
        "run": data.name, "turn": turn, "seen_turn": seen_turn, "unit": uid,
        "method": method, "pos": list(pos) if pos else None,
        "mechanism": mechanism, "mechanisms": matched,
        "nearest_hostile": nearest, "guard": guard,
        "site": list(site) if site else None,
        "site_hostile_seen": site_hostile_seen, "camp_near_site": camp_near_site,
        "frames": [frames[o] for o in (-2, -1, 0)],
        "orders": orders, "why": why_lines,
        **extra,
    }


def detect_captures(run_dir: Path) -> list[dict]:
    """Every settler capture in the run, precise events first, each with a dossier."""
    data = load_run(Path(run_dir))
    if not data.has_events:
        return []
    captures: list[dict] = []
    precise_units = set()
    for event in data.captured:
        uid = event.get("unit")
        kind = event.get("unit_kind")
        if kind != "UNIT_SETTLER":
            continue
        turn = _int(event.get("turn"), -1)
        precise_units.add(uid)
        captures.append(_dossier(data, uid, turn, "unit_captured", {
            "captor": event.get("captor"),
            "captor_is_barbarian": event.get("captor_is_barbarian"),
        }))
    for event in data.settler_lost:
        uid = event.get("unit")
        if uid in precise_units or uid in data.founded_units:
            continue
        turn = _int(event.get("turn"), -1)
        if data.terminal_turn is not None and turn >= data.terminal_turn:
            continue   # the empire dissolving at the end screen, not a raid
        captures.append(_dossier(data, uid, turn, "unit_lost_without_found", {
            "captor": None, "captor_is_barbarian": None,
        }))
    captures.sort(key=lambda c: (c["turn"], str(c["unit"])))
    return captures


def census_row(run_dir: Path) -> dict | None:
    data = load_run(Path(run_dir))
    if not data.has_events:
        return None
    captures = detect_captures(run_dir)
    return {
        "run": data.name, "last_turn": data.last_turn,
        "settlers_lost": len(data.settler_lost), "founds": len(data.found_turns),
        "captures": len(captures),
        "precise": sum(1 for c in captures if c["method"] == "unit_captured"),
        "mechanisms": [c["mechanism"] for c in captures],
        "turns": [c["turn"] for c in captures],
    }


def census(control_dir: Path, match: str = "civvis-*") -> list[dict]:
    rows = []
    for run_dir in sorted(Path(control_dir).glob(match)):
        if not run_dir.is_dir():
            continue
        row = census_row(run_dir)
        if row is not None:
            rows.append(row)
    return rows


def format_census(rows: list[dict]) -> str:
    head = ["run", "last_turn", "settlers_lost", "founds", "captures", "mechanisms"]
    lines = ["| " + " | ".join(head) + " |", "|" + "|".join("---" for _ in head) + "|"]
    total = 0
    for row in rows:
        total += row["captures"]
        mechanisms = ", ".join(
            f"t{t}:{m}" for t, m in zip(row["turns"], row["mechanisms"])) or "-"
        lines.append(f"| {row['run']} | {row['last_turn']} | {row['settlers_lost']} | "
                     f"{row['founds']} | {row['captures']} | {mechanisms} |")
    lines.append(f"| **total** | | | | **{total}** | {len(rows)} runs |")
    return "\n".join(lines)


# ----------------------------------------------------------------- rendering
def _fmt_unit(unit: dict | None) -> str:
    if not unit:
        return "none"
    parts = [str(unit.get("type") or "?")]
    if unit.get("combat") is not None:
        parts.append(f"str {unit['combat']}")
    if unit.get("hp") is not None:
        parts.append(f"hp {unit['hp']}")
    if unit.get("moves") is not None:
        parts.append(f"mv {unit['moves']}")
    if unit.get("pos"):
        parts.append(f"at ({unit['pos'][0]},{unit['pos'][1]})")
    if unit.get("distance") is not None:
        parts.append(f"d={unit['distance']}")
    if unit.get("id") is not None:
        parts.append(f"id {unit['id']}")
    if unit.get("named_by"):
        parts.append(f"[{unit['named_by']}]")
    return " ".join(parts)


def render_markdown(run_name: str, captures: list[dict]) -> str:
    out = [f"# Settler captures — {run_name}", ""]
    if not captures:
        out.append("No settler capture detected.")
        return "\n".join(out) + "\n"
    out.append(f"{len(captures)} capture(s). Mechanism is the first match in the "
               f"operator's order; every match is listed.")
    out.append("")
    for capture in captures:
        pos = capture.get("pos")
        where = f"({pos[0]},{pos[1]})" if pos else "unknown"
        out.append(f"## t{capture['turn']} settler {capture['unit']} at {where} — "
                   f"`{capture['mechanism']}`")
        out.append("")
        out.append(f"- detected by: `{capture['method']}`"
                   + (f", captor player {capture['captor']}"
                      f"{' (barbarian)' if capture.get('captor_is_barbarian') else ''}"
                      if capture.get("captor") is not None else ""))
        out.append(f"- mechanisms matched: {', '.join(capture['mechanisms']) or 'none'}")
        out.append(f"- nearest hostile: {_fmt_unit(capture.get('nearest_hostile'))}")
        out.append(f"- guard: {_fmt_unit(capture.get('guard'))}")
        site = capture.get("site")
        if site:
            out.append(f"- site: ({site[0]},{site[1]}) — hostile seen within 3 in the last "
                       f"5 turns: {capture['site_hostile_seen']}; camp within 3: "
                       f"{capture['camp_near_site']}")
        out.append("")
        out.append("### Settler t-2..t")
        out.append("")
        out.append("| turn | pos | moves | activity | hostiles within 3 | ours within 2 |")
        out.append("|---|---|---|---|---|---|")
        for frame in capture["frames"]:
            if frame["present"] is None:
                out.append(f"| t{frame['turn']} | (no frame) | | | | |")
                continue
            if not frame["present"]:
                out.append(f"| t{frame['turn']} | (not in frame) | | | | |")
                continue
            hostiles = "<br>".join(_fmt_unit(h) for h in frame["hostiles"]) or "none"
            friendly = "<br>".join(_fmt_unit(u) for u in frame["friendly"]) or "none"
            out.append(f"| t{frame['turn']} | ({frame['pos'][0]},{frame['pos'][1]}) | "
                       f"{frame['moves']} | {frame['activity']} | {hostiles} | {friendly} |")
        out.append("")
        out.append("### Orders (settler and guard)")
        out.append("")
        if capture["orders"]:
            for order in capture["orders"]:
                out.append(f"- `{json.dumps(order, sort_keys=True)}`")
        else:
            out.append("- none in the window")
        out.append("")
        out.append("### why.log t-2..t")
        out.append("")
        if capture["why"]:
            for line in capture["why"]:
                out.append(f"- {line}")
        else:
            out.append("- nothing mentioning settler/guard/raider/escort/flee/capture")
        out.append("")
    return "\n".join(out) + "\n"


# -------------------------------------------------------------------- ledger
def ledger_row(capture: dict) -> dict:
    return {
        "run": capture["run"], "turn": capture["turn"], "unit": capture["unit"],
        "pos": capture.get("pos"), "mechanism": capture["mechanism"],
        "mechanisms": capture.get("mechanisms"), "method": capture["method"],
        "captor": capture.get("captor"),
        "captor_is_barbarian": capture.get("captor_is_barbarian"),
        "nearest_hostile": capture.get("nearest_hostile"), "guard": capture.get("guard"),
    }


def append_ledger(path: Path, captures: list[dict]) -> int:
    """Append one row per capture; rows already present for (run, unit) are skipped."""
    path = Path(path)
    present = set()
    if path.is_file():
        for line in path.read_text(errors="replace").splitlines():
            try:
                row = json.loads(line)
            except ValueError:
                continue
            if isinstance(row, dict):
                present.add((row.get("run"), row.get("unit")))
    written = 0
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a") as handle:
        for capture in captures:
            key = (capture["run"], capture["unit"])
            if key in present:
                continue
            handle.write(json.dumps(ledger_row(capture), sort_keys=True) + "\n")
            present.add(key)
            written += 1
    return written


# ----------------------------------------------------------------------- CLI
def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("run_dir", nargs="?", help="a run directory holding events.jsonl")
    parser.add_argument("--json", action="store_true", help="print the dossiers as JSON")
    parser.add_argument("--markdown", action="store_true", help="print the dossiers as Markdown")
    parser.add_argument("--ledger", type=Path, help="append one JSON line per capture here")
    parser.add_argument("--all", type=Path, metavar="CONTROL_DIR",
                        help="census every run directory under this control directory")
    parser.add_argument("--match", default="civvis-*",
                        help="glob for run directories under --all (default: civvis-*)")
    args = parser.parse_args(argv)

    if args.all is not None:
        rows = census(args.all, args.match)
        if args.json:
            print(json.dumps(rows, indent=1, sort_keys=True))
        else:
            print(format_census(rows))
        return 0
    if not args.run_dir:
        parser.error("a run directory or --all <control-dir> is required")
    run_dir = Path(args.run_dir)
    if not (run_dir / "events.jsonl").is_file():
        print(f"no events.jsonl under {run_dir}", file=sys.stderr)
        return 2
    captures = detect_captures(run_dir)
    if args.ledger is not None:
        written = append_ledger(args.ledger, captures)
        print(f"ledger: {written} new row(s) in {args.ledger}", file=sys.stderr)
    if args.json:
        print(json.dumps(captures, indent=1, sort_keys=True))
    elif args.markdown or not args.ledger:
        print(render_markdown(run_dir.name, captures), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
