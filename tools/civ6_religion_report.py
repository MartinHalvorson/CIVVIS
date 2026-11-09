#!/usr/bin/env python3
"""Read native religious progress without treating retirements as losses.

Usage: python3 tools/civ6_religion_report.py <run-directory-or-runs-root>
Reads local or pulled ledger runs, or the live root (`~/civvis-civ6-runs`, whose
games sit in `control/`); writes JSON to stdout. Starts no game and changes no
controller. Continuations count as one game. Counts are descriptive, not an
estimate of treatment effect.

`--spread-yield` adds what the Missionary and Apostle charges bought: per game
and summed, spreads by target, flips of rival cities, and how many held ten
turns later. Nothing in it can be read off the ladder row; it streams every
game's events.

The native exporter reads rival.religion from GetReligionInMajorityOfCities
(CivvisControlAgent.lua, citing Firaxis WorldRankings.lua:2049). A null means
no majority OR an unreadable answer. cities_following_religion counts world
cities, not converted civilizations. Neither observation proves a victory.
"""

from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path

import civ6_ladder
import live_ledger


REFORMED_GENE = "religion-seat-reaches-reformed-church"
RELIGIOUS_UNITS = ("UNIT_APOSTLE", "UNIT_MISSIONARY", "UNIT_GURU", "UNIT_INQUISITOR")

#: The units whose `spread_charges` are religion spreads.
SPREADERS = ("UNIT_APOSTLE", "UNIT_MISSIONARY")
#: A flip is called held when a reading at this many turns later (or within
#: `HOLD_WINDOW` more) still shows our majority.
HOLD_AFTER = 10
HOLD_WINDOW = 2
#: A reading of a rival city is only ever taken at the start of a turn, the
#: board the decider saw, and the gap a spread can look across is this many
#: turns either side. Longer than that the reading is treated as unknown.
READING_REACH = 3
#: Turn marks for the per-rival timeline; the last frame is always added.
TIMELINE_TURNS = (50, 75, 100, 125, 150, 175, 200)


def outcome_of(summary: dict) -> tuple[str, str | None]:
    outcome = summary.get("outcome") or {}
    victory = civ6_ladder.victory_type(summary) or outcome.get("victory_type")
    if outcome.get("kind") == "victory":
        if outcome.get("won") is True:
            if victory is None:
                return "win_type_unknown", None
            return ("religious_win" if victory == "VICTORY_RELIGIOUS"
                    else "other_victory"), victory
        if outcome.get("won") is False:
            return "loss", victory
        return "terminal_unresolved", victory
    if civ6_ladder.is_defeat(summary):
        return "loss", None
    return "censored", None


def final_progress(state: dict | None) -> dict | None:
    if state is None:
        return None
    faith = state.get("founded_religion")
    rival_data = state.get("rivals")
    rivals = [{"player": r.get("player"), "civ": r.get("civ"),
               "majority": r.get("religion")}
              for r in rival_data or []
              if r.get("alive") is not False and not r.get("is_minor")]
    unit_data = state.get("units")
    units = Counter(u.get("kind") for u in unit_data or [])
    return {
        "turn": state.get("turn"), "frame": state.get("frame", 0),
        "founded_religion": faith,
        "government": state.get("government"), "policies": state.get("policies"),
        "faith": state.get("faith"), "faith_per_turn": state.get("faith_per_turn"),
        "world_cities_following_religion": state.get("cities_following_religion"),
        "reported_rivals": rivals if isinstance(rival_data, list) else None,
        "reported_rival_majorities_matching_ours": (
            sum(r["majority"] == faith for r in rivals)
            if faith and isinstance(rival_data, list) else None),
        "reported_rivals_without_resolved_majority": (
            sum(r["majority"] is None for r in rivals)
            if isinstance(rival_data, list) else None),
        "religious_units": ({kind: units[kind] for kind in RELIGIOUS_UNITS}
                            if isinstance(unit_data, list) else None),
    }


def report_game(segments: list[dict]) -> dict:
    segments = sorted(segments, key=lambda s: live_ledger.segment_index(s["tag"]))
    last = segments[-1]
    first = {key: None for key in (
        "founded_religion", "theology", "reformed_church", "temple", "apostle",
        "verified_apostle_purchase", "theocracy", "religious_orders")}
    final, invalid_lines, identity = None, 0, []

    def mark(key, turn):
        if first[key] is None or turn < first[key]:
            first[key] = turn

    for segment in segments:
        genome = segment.get("genome_treatments") or {}
        treatments = genome.get("treatments")
        identity.append({
            "tag": segment["tag"], "screen_gene": segment.get("screen_gene"),
            "screen_arm": segment.get("screen_arm"),
            "decider_revisions": segment.get("decider_revisions"),
            "reformed_church_reported_active": (
                REFORMED_GENE in treatments if isinstance(treatments, list) else None),
        })
        path = live_ledger.events_path(segment["_dir"])
        # The final segment is authoritative, including missing state evidence.
        final = None
        if path is None:
            continue
        with live_ledger.open_events(path) as stream:
            for line in stream:
                try:
                    event = json.loads(line)
                except json.JSONDecodeError:
                    invalid_lines += 1
                    continue
                if not isinstance(event, dict) or type(event.get("turn")) is not int:
                    continue
                turn = event["turn"]
                if (event.get("kind") == "order_verified"
                        and event.get("order_kind") == "purchase_faith"
                        and event.get("verb") == "UNIT_APOSTLE"):
                    mark("verified_apostle_purchase", turn)
                if event.get("kind") != "state":
                    continue
                if final is None or (turn, event.get("frame", 0)) >= (
                        final["turn"], final.get("frame", 0)):
                    final = event
                if event.get("founded_religion"):
                    mark("founded_religion", turn)
                civics = event.get("civics") or []
                for civic in ("theology", "reformed_church"):
                    if "CIVIC_" + civic.upper() in civics:
                        mark(civic, turn)
                if any("BUILDING_TEMPLE" in (c.get("buildings") or [])
                       for c in event.get("cities") or []):
                    mark("temple", turn)
                if any(u.get("kind") == "UNIT_APOSTLE"
                       for u in event.get("units") or []):
                    mark("apostle", turn)
                if event.get("government") == "GOVERNMENT_THEOCRACY":
                    mark("theocracy", turn)
                if "POLICY_RELIGIOUS_ORDERS" in (event.get("policies") or []):
                    mark("religious_orders", turn)
    outcome, victory = outcome_of(last)
    return {
        "tag": live_ledger.game_stem(last["tag"]), "segments": identity,
        "stem_present": live_ledger.segment_index(segments[0]["tag"]) == 0,
        "finished_utc": last.get("finished_utc"),
        "seat": last.get("seat"), "difficulty": last.get("difficulty"),
        "outcome": outcome, "victory_type": victory, "reason": last.get("reason"),
        "retirement": last.get("abandoned") or last.get("operator_retire"),
        "first_observed_turn": first, "final": final_progress(final),
        "invalid_event_lines": invalid_lines,
    }


def hex_distance(a: tuple[int, int], b: tuple[int, int]) -> int:
    """Grid distance between two odd-r offset plots. The world wrap is ignored:
    a cast across the seam counts as unplaced, not as a wrong city."""
    def cube(x, y):
        q = x - (y - (y & 1)) // 2
        return q, y, -q - y
    A, B = cube(*a), cube(*b)
    return max(abs(A[0] - B[0]), abs(A[1] - B[1]), abs(A[2] - B[2]))


def yield_log(segments: list[dict]) -> tuple[list[dict], set, dict, Counter]:
    """The facts `--spread-yield` needs from a game's events, oldest first.

    Frames are held in the order the log wrote them and keep only spreaders and
    city religion, so a 100 MB log costs a few megabytes. Verified `SPREAD_RELIGION`
    orders are recorded by (unit, turn) because one cast often logs two lines.
    """
    frames, verified, moves, unspent = [], set(), {}, Counter()
    for segment in sorted(segments, key=lambda s: live_ledger.segment_index(s["tag"])):
        path = live_ledger.events_path(segment["_dir"])
        if path is None:
            continue
        with live_ledger.open_events(path) as stream:
            for line in stream:
                if '"kind": "state"' in line:
                    try:
                        event = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if event.get("kind") == "state":
                        frames.append(yield_frame(event, len(frames)))
                elif '"kind": "host_move"' in line:
                    try:
                        event = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    moves[(event.get("unit"), len(frames))] = (event.get("x"), event.get("y"))
                elif "SPREAD_RELIGION" in line and '"order_' in line:
                    try:
                        event = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if event.get("verb") != "SPREAD_RELIGION":
                        continue
                    if event.get("kind") == "order_verified":
                        verified.add((event.get("subject"), event.get("turn")))
                    elif event.get("kind") == "order_failed":
                        unspent[event.get("reason")] += 1
    return frames, verified, moves, unspent


def yield_frame(state: dict, index: int) -> dict:
    """One state frame reduced to spreaders and each city's religion reading."""
    def foreign(cities):
        return {(c["x"], c["y"]): {
            "observed": c.get("religion_observed") is True,
            "religion": c.get("religion")} for c in cities or []}
    units = {u["id"]: {"x": u["x"], "y": u["y"], "kind": u["kind"],
                       "charges": u.get("spread_charges")}
             for u in state.get("units") or [] if u.get("kind") in SPREADERS}
    rivals = {r.get("player"): {
        "civ": r.get("civ"), "majority": r.get("religion"),
        "alive": r.get("alive") is not False,
        "owned": (r.get("public_stats") or {}).get("city_count"),
        "cities": foreign(r.get("cities"))}
        for r in state.get("rivals") or []}
    return {
        "turn": state["turn"], "index": index, "founded": state.get("founded_religion"),
        "following": state.get("cities_following_religion"), "units": units,
        "own": {(c["x"], c["y"]): c.get("religion") for c in state.get("cities") or []},
        "rivals": rivals,
        "minors": {m.get("player"): foreign(m.get("cities")) for m in state.get("minors") or []},
    }


def landed_spreads(frames: list[dict], verified: set, moves: dict) -> list[dict]:
    """Each charge that really fell, taken from the frames rather than the
    order log: one cast writes up to four `order_verified` lines, and a verdict
    of "the unit is gone" fits a killed unit as well as a spent one."""
    spreads = []
    for before, after in zip(frames, frames[1:]):
        for uid, unit in before["units"].items():
            charges = unit["charges"]
            if charges is None:
                continue
            now = after["units"].get(uid)
            if now is not None:
                if now["charges"] is not None and now["charges"] < charges:
                    spreads.append(dict(turn=before["turn"], unit=uid, kind=unit["kind"],
                                        pos=(now["x"], now["y"]), frame=after["index"]))
            elif charges == 1 and (uid, before["turn"]) in verified:
                # A spent Missionary disappears, so its last position is the frame
                # before or the last move the host took from it during this one.
                pos = moves.get((uid, after["index"])) or (unit["x"], unit["y"])
                spreads.append(dict(turn=before["turn"], unit=uid, kind=unit["kind"],
                                    pos=pos, frame=after["index"]))
    return spreads


def spread_target(frame: dict, pos: tuple[int, int]) -> dict | None:
    """The city a cast on `pos` reached: a spread lands on or beside a city
    centre, and the report's `unplaced` count shows any that does not."""
    cities = [(p, "own", None, None) for p in frame["own"]]
    for pid, rival in frame["rivals"].items():
        cities += [(p, "rival", pid, rival["civ"]) for p in rival["cities"]]
    for pid, minor in frame["minors"].items():
        cities += [(p, "minor", pid, None) for p in minor]
    if not cities:
        return None
    where = min(cities, key=lambda c: hex_distance(c[0], pos))
    if hex_distance(where[0], pos) > 1:
        return None
    return {"pos": where[0], "kind": where[1], "player": where[2], "civ": where[3]}


def city_readings(frames: list[dict]) -> dict:
    """{plot: {turn: majority}} at the start of each turn, read cities only: a
    city the seat could not read has no entry, and an observed lack of majority
    is an entry of None. Never confuse the two."""
    readings: dict = {}
    seen_turns = set()
    for frame in frames:
        if frame["turn"] in seen_turns:
            continue
        seen_turns.add(frame["turn"])
        for rival in frame["rivals"].values():
            for pos, city in rival["cities"].items():
                if city["observed"]:
                    readings.setdefault(pos, {})[frame["turn"]] = city["religion"]
    return readings


def read_near(history: dict, turn: int, steps) -> tuple[bool, str | None]:
    """The first reading at `turn + step` for `step` in `steps`, and whether any exists."""
    for step in steps:
        if turn + step in history:
            return True, history[turn + step]
    return False, None


def outcome_of_spread(history: dict, turn: int, ours: str) -> str:
    """already_ours / flipped / no_flip / unread for a cast on `turn`, from the
    readings at the start of that turn and of the next few."""
    known, before = read_near(history, turn, range(0, -READING_REACH - 1, -1))
    if not known:
        return "unread"
    if before == ours:
        return "already_ours"
    known, after = read_near(history, turn, range(1, READING_REACH + 1))
    if not known:
        return "unread"
    return "flipped" if after == ours else "no_flip"


def rival_flips(readings: dict, casts: dict, ours: str) -> list[dict]:
    """Every observed change of a rival city's turn-start majority to ours,
    whether our charges caused it or pressure did, and whether it held."""
    flips = []
    for pos, history in readings.items():
        turns = sorted(history)
        for earlier, turn in zip(turns, turns[1:]):
            if history[earlier] == ours or history[turn] != ours:
                continue
            cast = sum(1 for t in casts.get(pos, ()) if earlier <= t < turn)
            read, later = read_near(history, turn, range(HOLD_AFTER, HOLD_AFTER + HOLD_WINDOW + 1))
            flips.append({"pos": pos, "turn": turn, "casts": cast,
                          "held": (later == ours) if read else None})
    return flips


def rival_timeline(frames: list[dict], ours: str) -> dict:
    """Per rival: cities reading ours against cities listed, read and owned."""
    first_of_turn: dict = {}
    for frame in frames:
        first_of_turn.setdefault(frame["turn"], frame)
    last = max(first_of_turn)
    timeline: dict = {}
    for turn in (*[t for t in TIMELINE_TURNS if t < last], last):
        frame = first_of_turn.get(turn)
        if frame is None:
            continue
        for pid, rival in frame["rivals"].items():
            cities = rival["cities"].values()
            timeline.setdefault(str(pid), {"civ": rival["civ"], "marks": []})["marks"].append({
                "turn": turn, "owned": rival["owned"], "listed": len(rival["cities"]),
                "read": sum(c["observed"] for c in cities),
                "ours": sum(c["observed"] and c["religion"] == ours for c in cities),
                "country_ours": rival["majority"] == ours})
    return timeline


def spread_yield_of_game(segments: list[dict]) -> dict | None:
    """What the charges of one game bought, or None when it never held a faith."""
    frames, verified, moves, unspent = yield_log(segments)
    ours = next((f["founded"] for f in reversed(frames) if f["founded"]), None)
    if ours is None or not frames:
        return None
    spreads = landed_spreads(frames, verified, moves)
    targets = Counter({"rival": 0, "own": 0, "minor": 0, "unplaced": 0})
    rival_casts, casts_on = Counter(), {}
    for spread in spreads:
        target = spread_target(frames[spread["frame"]], spread["pos"])
        spread["target"] = target
        targets[target["kind"] if target else "unplaced"] += 1
        if target and target["kind"] == "rival":
            rival_casts[str(target["player"])] += 1
            casts_on.setdefault(target["pos"], []).append(spread["turn"])
    readings = city_readings(frames)
    outcomes = Counter({"already_ours": 0, "flipped": 0, "no_flip": 0, "unread": 0})
    for spread in spreads:
        if spread["target"] and spread["target"]["kind"] == "rival":
            outcomes[outcome_of_spread(readings.get(spread["target"]["pos"], {}),
                                       spread["turn"], ours)] += 1
    flips = rival_flips(readings, casts_on, ours)
    world_series = []
    for frame in frames:
        if not world_series or world_series[-1][0] != frame["turn"]:
            if frame["following"] is not None:
                own = sum(1 for religion in frame["own"].values() if religion == ours)
                world_series.append((frame["turn"], frame["following"] - own))
    deltas = [b[1] - a[1] for a, b in zip(world_series, world_series[1:])]
    return {
        "religion": ours, "last_turn": frames[-1]["turn"],
        "spreads": dict(targets), "rival_spreads_by_player": dict(rival_casts),
        "verified_unit_turns": len(verified),
        "spread_outcomes": dict(outcomes),
        "flips_credited": sum(f["casts"] > 0 for f in flips),
        "flips_uncredited": sum(f["casts"] == 0 for f in flips),
        "flips_held": sum(f["casts"] > 0 and f["held"] is True for f in flips),
        "flips_reverted": sum(f["casts"] > 0 and f["held"] is False for f in flips),
        "flips_unread": sum(f["casts"] > 0 and f["held"] is None for f in flips),
        "world_followers": {"gained": sum(d for d in deltas if d > 0),
                            "lost": sum(-d for d in deltas if d < 0),
                            "last": world_series[-1][1] if world_series else None},
        "unspent_reasons": dict(unspent),
        "rival_timeline": rival_timeline(frames, ours),
    }


def yield_summary(per_game: list[dict]) -> dict:
    """The pooled counts and the per-100-spread answer, with the game count."""
    def total(fn):
        return sum(fn(g) for g in per_game)
    rival = total(lambda g: g["spreads"]["rival"])
    credited = total(lambda g: g["flips_credited"])
    held, reverted = total(lambda g: g["flips_held"]), total(lambda g: g["flips_reverted"])
    per_100 = (lambda n: round(100 * n / rival, 1)) if rival else (lambda n: None)
    rank_shares = [0.0] * 5
    ranked_games = 0
    for game in per_game:
        rank = sorted(game["rival_spreads_by_player"].values(), reverse=True)
        total_rival = sum(rank)
        if total_rival < 20:
            continue
        ranked_games += 1
        for i, value in enumerate(rank[:5]):
            rank_shares[i] += value / total_rival
    return {
        "games": len(per_game),
        "spreads": dict(sum((Counter(g["spreads"]) for g in per_game), Counter())),
        "spread_outcomes": dict(sum((Counter(g["spread_outcomes"]) for g in per_game), Counter())),
        "flips_credited": credited, "flips_uncredited": total(lambda g: g["flips_uncredited"]),
        "flips_held": held, "flips_reverted": reverted,
        "flips_unread": total(lambda g: g["flips_unread"]),
        "spreads_per_flip": round(rival / credited, 1) if credited else None,
        "flips_per_100_rival_spreads": per_100(credited),
        "held_per_100_rival_spreads_confirmed": per_100(held),
        "held_share_of_read_flips": round(held / (held + reverted), 3) if held + reverted else None,
        "world_followers_gained": total(lambda g: g["world_followers"]["gained"]),
        "world_followers_lost": total(lambda g: g["world_followers"]["lost"]),
        "world_followers_net_at_end": total(lambda g: g["world_followers"]["last"] or 0),
        "world_gained_per_100_rival_spreads": per_100(total(lambda g: g["world_followers"]["gained"])),
        "mean_rival_share_of_spreads_by_rank": (
            [round(v / ranked_games, 3) for v in rank_shares] if ranked_games else None),
        "games_with_20_plus_rival_spreads": ranked_games,
    }


def report(root: Path, *, since: str | None = None, civ: str | None = None,
           with_spread_yield: bool = False) -> dict:
    if (root / "summary.json").is_file():
        body = json.loads((root / "summary.json").read_text())
        body["_dir"] = root
        summaries = [body]
    else:
        summaries = live_ledger.summaries(root)
    groups = {}
    for body in summaries:
        body["tag"] = body.get("tag") or body["_dir"].name
        groups.setdefault(live_ledger.game_stem(body["tag"]), []).append(body)
    games = []
    yields = []
    for segments in groups.values():
        last = max(segments, key=lambda s: live_ledger.segment_index(s["tag"]))
        if last.get("victory_target") != "religious":
            continue
        if since and (last.get("finished_utc") or "") < since:
            continue
        if civ and (last.get("seat") or {}).get("civ") != civ:
            continue
        game = report_game(segments)
        if with_spread_yield:
            game["spread_yield"] = spread_yield_of_game(segments)
            if game["spread_yield"]:
                yields.append(game["spread_yield"])
        games.append(game)
    games.sort(key=lambda g: (g.get("finished_utc") or "", g["tag"]))
    counts = Counter(g["outcome"] for g in games)
    out = {"games": games, "counts": dict(sorted(counts.items())),
           "summaries_read": len(summaries),
           "scope": "Descriptive native observations; no causal treatment comparison. "
                    "Null rival majorities may mean no majority or unreadable data. "
                    "Reported rivals need not include every living major."}
    if with_spread_yield:
        out["spread_yield"] = yield_summary(yields)
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("root", type=Path)
    parser.add_argument("--since", help="finished UTC timestamp lower bound")
    parser.add_argument("--civ", help="exact native civilization type")
    parser.add_argument("--spread-yield", action="store_true",
                        help="also stream every game's events for what its spreads bought")
    args = parser.parse_args()
    print(json.dumps(report(args.root, since=args.since, civ=args.civ,
                            with_spread_yield=args.spread_yield), indent=2))


if __name__ == "__main__":
    main()
