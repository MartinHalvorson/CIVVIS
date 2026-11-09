#!/usr/bin/env python3
"""Read native religious progress without treating retirements as losses.

Usage: python3 tools/civ6_religion_report.py <run-directory-or-runs-root>
Reads local or pulled ledger runs; writes JSON to stdout. Starts no game and
changes no controller. Continuations count as one game. Counts are descriptive,
not an estimate of treatment effect.

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


def report(root: Path, *, since: str | None = None, civ: str | None = None) -> dict:
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
    for segments in groups.values():
        last = max(segments, key=lambda s: live_ledger.segment_index(s["tag"]))
        if last.get("victory_target") != "religious":
            continue
        if since and (last.get("finished_utc") or "") < since:
            continue
        if civ and (last.get("seat") or {}).get("civ") != civ:
            continue
        games.append(report_game(segments))
    games.sort(key=lambda g: (g.get("finished_utc") or "", g["tag"]))
    counts = Counter(g["outcome"] for g in games)
    return {"games": games, "counts": dict(sorted(counts.items())),
            "scope": "Descriptive native observations; no causal treatment comparison. "
                     "Null rival majorities may mean no majority or unreadable data. "
                     "Reported rivals need not include every living major."}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("root", type=Path)
    parser.add_argument("--since", help="finished UTC timestamp lower bound")
    parser.add_argument("--civ", help="exact native civilization type")
    args = parser.parse_args()
    print(json.dumps(report(args.root, since=args.since, civ=args.civ), indent=2))


if __name__ == "__main__":
    main()
