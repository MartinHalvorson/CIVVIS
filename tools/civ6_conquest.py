#!/usr/bin/env python3
"""Measure native conquest from recorded ownership, never from score or kills.

Usage: python3 tools/civ6_conquest.py RUN_DIR [RUN_DIR ...]
Reads events.jsonl[.gz] and summary.json without changing a game or its records.
Counts are unique city plots observed held during this run segment, not capture
events. Continuations do not recover observations from before their first frame.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from civ6_race_audit import event_path, events


def conquest_totals(path: Path) -> dict | None:
    """Keep major cities, original capitals and minor cities separate.

    The native exporter walks major IDs into ``rivals`` and minor IDs into
    ``minors``. Remember those identities even after elimination. City IDs are
    owner-local and can change on capture; coordinates identify a city instead.
    Missing identity or original-capital metadata is reported, never guessed
    from player-number ranges or the current ``capital`` flag.
    ``war`` reports a successfully issued request, not an applied declaration;
    only the state's ``at_war`` reading proves observed war exposure.
    """
    local_player = None
    majors, minors = set(), set()
    first = last = None
    observed_turns = set()
    held, final = {}, {}
    own_capital_plots = set()
    declarations, war_exposure = [], []
    final_unknown = final_missing_capital = final_missing_plot = 0
    for event in events(path):
        kind, turn = event.get("kind"), event.get("turn")
        if kind == "seat" and type(event.get("local_player")) is int:
            local_player = event["local_player"]
        if type(turn) is not int:
            continue
        if last is not None and turn < last:
            continue
        if kind == "war" and event.get("source") == "civvis":
            if type(event.get("target")) is int:
                declarations.append((turn, event["target"]))
        if kind != "state" or not isinstance(event.get("cities"), list):
            continue
        if local_player is None:
            continue
        if first is None:
            first = turn
        last = turn
        observed_turns.add(turn)
        for field, identities in (("rivals", majors), ("minors", minors)):
            players = event.get(field)
            for player in players if isinstance(players, list) else []:
                if not isinstance(player, dict):
                    continue
                pid = player.get("player")
                if type(pid) is int and pid != local_player:
                    identities.add(pid)
                    if player.get("at_war") is True:
                        war_exposure.append((turn, pid))
        final = {}
        final_unknown = final_missing_capital = final_missing_plot = 0
        for city in event["cities"]:
            if not isinstance(city, dict):
                final_unknown += 1
                continue
            owner = city.get("original_owner")
            if type(owner) is not int:
                final_unknown += 1
            if type(city.get("original_capital")) is not bool:
                final_missing_capital += 1
            x, y = city.get("x"), city.get("y")
            if type(x) is not int or type(y) is not int:
                final_missing_plot += 1
                continue
            key = (x, y)
            row = {"original_owner": owner,
                   "original_capital": city.get("original_capital"),
                   "observed_turn": turn}
            final[key] = row
            history = held.setdefault(key, dict(row, present_at_first_frame=turn == first))
            # Metadata can become available after the first ownership frame.
            if type(owner) is int:
                if type(history["original_owner"]) is not int:
                    history["observed_turn"] = turn
                    history["present_at_first_frame"] = turn == first
                history["original_owner"] = owner
            if city.get("original_capital") is True and type(owner) is int:
                history["original_capital"] = True
                history.setdefault("capital_observed_turn", turn)
            if owner == local_player and city.get("original_capital") is True:
                own_capital_plots.add(key)
    if first is None:
        return None

    def classified(rows, identities, capitals=False):
        return {key: row for key, row in rows.items()
                if type(row["original_owner"]) is int
                and row["original_owner"] in identities
                and (not capitals or row["original_capital"] is True)}

    major_held = classified(held, majors)
    capital_held = {key: row for key, row in major_held.items()
                    if "capital_observed_turn" in row}
    major_final = classified(final, majors)
    capital_final = classified(final, majors, capitals=True)
    minor_held = classified(held, minors)
    minor_final = classified(final, minors)
    unknown_held = [row for row in held.values()
                    if row["original_owner"] != local_player
                    and (type(row["original_owner"]) is not int
                         or row["original_owner"] not in majors | minors)]
    final_unknown += sum(1 for row in final.values()
                         if type(row["original_owner"]) is int
                         and row["original_owner"] != local_player
                         and row["original_owner"] not in majors | minors)

    def milestone(rows, capitals=False):
        if not rows:
            return None
        field = "capital_observed_turn" if capitals else "observed_turn"
        turn = min(row[field] for row in rows.values())
        return {"observed_turn": turn, "present_at_first_frame": turn == first}

    major_declaration_requests = [turn for turn, target in declarations if target in majors]
    minor_declaration_requests = [turn for turn, target in declarations if target in minors]
    major_exposure = [turn for turn, target in war_exposure if target in majors]
    own_held = bool(own_capital_plots.intersection(final))
    return {
        "scope": "run_segment",
        "first_observed_turn": first,
        "last_observed_turn": last,
        "observed_turns": len(observed_turns),
        "missing_turns": last - first + 1 - len(observed_turns),
        "major_players_observed": sorted(majors),
        "minor_players_observed": sorted(minors),
        "major_declaration_requests": len(major_declaration_requests),
        "minor_declaration_requests": len(minor_declaration_requests),
        "unclassified_declaration_requests": len(declarations) - len(major_declaration_requests) - len(minor_declaration_requests),
        "first_major_declaration_request_turn": min(major_declaration_requests, default=None),
        "first_major_war_observed_turn": min(major_exposure, default=None),
        "foreign_major_cities_observed_held": len(major_held),
        "foreign_major_cities_held_final": len(major_final),
        "foreign_major_original_capitals_observed_held": len(capital_held),
        "foreign_major_original_capitals_held_final": len(capital_final),
        "foreign_minor_cities_observed_held": len(minor_held),
        "foreign_minor_cities_held_final": len(minor_final),
        "first_major_city_held": milestone(major_held),
        "first_major_original_capital_held": milestone(capital_held, capitals=True),
        "own_original_capital_held_final": own_held if own_capital_plots
            and (own_held or not final_missing_plot) else None,
        "unclassified_cities_observed_held": len(unknown_held),
        "unclassified_cities_held_final": final_unknown,
        "cities_missing_original_capital_final": final_missing_capital,
        "cities_missing_plot_final": final_missing_plot,
    }


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("runs", type=Path, nargs="+")
    args = parser.parse_args(argv)
    from civ6_ladder import is_win, victory_type
    for run in args.runs:
        path = event_path(run)
        summary_path = run / "summary.json"
        summary = json.loads(summary_path.read_text()) if summary_path.is_file() else {}
        ending = victory_type(summary)
        print(json.dumps({"run": run.name, "seat": summary.get("seat"),
                          "victory_target": summary.get("victory_target"),
                          "victory_type": ending,
                          "domination_win": is_win(summary) and ending == "VICTORY_CONQUEST"
                              if ending is not None else None,
                          "conquest": conquest_totals(path) if path else None}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
