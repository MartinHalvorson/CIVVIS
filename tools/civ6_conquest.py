#!/usr/bin/env python3
"""Measure native conquest from recorded ownership, never from score or kills.

Usage: python3 tools/civ6_conquest.py RUN_DIR [RUN_DIR ...]
Reads events.jsonl[.gz] and summary.json without changing a game or its records.
Counts are unique city plots observed held, not capture events. Segment results
remain local to that run. Explicit recovery ancestry additionally reconstructs
the surviving observed game path and excludes observations rolled back by a save.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import tempfile

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
    return _conquest_totals(events(path))


def _conquest_totals(rows) -> dict | None:
    local_player = None
    majors, minors = set(), set()
    first = last = None
    observed_turns = set()
    held, final = {}, {}
    own_capital_plots = set()
    declarations, war_exposure = [], []
    final_unknown = final_missing_capital = final_missing_plot = 0
    for event in rows:
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


RECOVERY_CHAIN = "recovery-chain.json"
SEAT_FIELDS = ("local_player", "civ", "leader", "difficulty", "players",
               "map", "size", "speed", "ruleset", "max_turns",
               "city_states", "modes", "victories")


def _tag(value) -> str:
    if not isinstance(value, str) or not value or value in (".", "..") \
            or Path(value).name != value or "\\" in value:
        raise ValueError("invalid recovery run tag")
    return value


def _chain_tags(chain: dict, current: str) -> list[str]:
    if not isinstance(chain, dict) or type(chain.get("schema")) is not int \
            or chain["schema"] != 1 or chain.get("source") != "civ6_civvis_climb":
        raise ValueError("unsupported recovery chain")
    segments = chain.get("segments")
    if not isinstance(segments, list) or len(segments) < 2:
        raise ValueError("recovery chain needs its original run and continuations")
    tags = [_tag(tag) for tag in segments]
    if len(set(tags)) != len(tags) or chain.get("root") != tags[0] \
            or chain.get("current") != current or tags[-1] != current:
        raise ValueError("recovery chain does not identify this run uniquely")
    reloads = chain.get("reloads")
    if not isinstance(reloads, list) or len(reloads) != len(tags) - 1:
        raise ValueError("recovery chain is missing reload records")
    for tag, reload in zip(tags[1:], reloads):
        if not isinstance(reload, dict) or reload.get("tag") != tag \
                or type(reload.get("from_turn")) is not int \
                or reload["from_turn"] < 0:
            raise ValueError("recovery reload record does not match its segment")
        _tag(reload.get("save"))
    return tags


def write_recovery_chain(runs_dir: Path, root: str, resumes: list[dict]) -> Path:
    """Publish explicit ancestry before the continuation's player is started.

    Save names are provenance, never turn numbers. Actual state exports supply
    the rollback boundaries when the completed summary is recorded.
    """
    chain = {"schema": 1, "source": "civ6_civvis_climb", "root": root,
             "current": resumes[-1]["tag"],
             "segments": [root, *(resume["tag"] for resume in resumes)],
             "reloads": resumes}
    _chain_tags(chain, chain["current"])
    directory = Path(runs_dir) / chain["current"]
    directory.mkdir(parents=True, exist_ok=True)
    destination = directory / RECOVERY_CHAIN
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", dir=directory,
                                         prefix=".recovery-chain-", delete=False) as handle:
            temporary = Path(handle.name)
            json.dump(chain, handle, indent=2)
            handle.write("\n")
        os.replace(temporary, destination)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    return destination


def _recovery_segment(directory: Path, tag: str) -> dict:
    evidence = event_path(directory)
    if evidence is None:
        raise ValueError(f"missing events for {tag}")
    rows, seats, homes = [], [], set()
    first = None
    for row in events(evidence):
        if row.get("kind") not in ("seat", "state", "war"):
            continue
        if row.get("run") not in (None, tag):
            raise ValueError(f"event belongs to a different run in {tag}")
        if row.get("kind") == "seat":
            if row.get("run") != tag or any(row.get(field) is None for field in SEAT_FIELDS):
                raise ValueError(f"missing native seat identity in {tag}")
            seats.append({field: row[field] for field in SEAT_FIELDS})
            rows.append(row)
            continue
        turn = row.get("turn")
        if type(turn) is not int:
            continue
        if row.get("kind") == "state" and isinstance(row.get("cities"), list):
            if not seats:
                raise ValueError(f"ownership precedes native seat identity in {tag}")
            if first is None:
                first = turn
            local = seats[0]["local_player"]
            cities = list(row["cities"])
            for field in ("rivals", "minors"):
                players = row.get(field)
                for player in players if isinstance(players, list) else []:
                    if isinstance(player, dict) and isinstance(player.get("cities"), list):
                        cities.extend(player["cities"])
            for city in cities:
                if isinstance(city, dict) and city.get("original_owner") == local \
                        and city.get("original_capital") is True \
                        and type(city.get("x")) is int and type(city.get("y")) is int:
                    homes.add((city["x"], city["y"]))
        # Keep only the fields ownership accounting consumes. Hundreds of
        # research menus and unit inventories need not stay in memory.
        reduced = {field: row[field] for field in ("kind", "turn", "source", "target")
                   if field in row}
        if "cities" in row:
            reduced["cities"] = [
                {field: city[field] for field in ("x", "y", "original_owner", "original_capital")
                 if field in city} if isinstance(city, dict) else city
                for city in row["cities"]] if isinstance(row["cities"], list) else row["cities"]
        for field in ("rivals", "minors"):
            if isinstance(row.get(field), list):
                reduced[field] = [{key: player[key] for key in ("player", "at_war") if key in player}
                                  for player in row[field] if isinstance(player, dict)]
        rows.append(reduced)
    if first is None or not seats or any(seat != seats[0] for seat in seats):
        raise ValueError(f"missing ownership or changing native seat in {tag}")
    if len(homes) != 1:
        raise ValueError(f"missing or ambiguous original home-capital plot in {tag}")
    return {"tag": tag, "rows": rows, "first": first,
            "seat": seats[0], "home": homes}


def recovered_conquest(run: Path, chain_path: Path | None = None) -> dict | None:
    """Observe the surviving recovery path, never sum continuation totals.

    Explicit climb ancestry, matching native seats and an original home-capital
    plot bind the inputs. Each reload replaces every earlier observation from
    its actual first state turn onward, even when that removes a whole segment.
    Missing evidence yields an unavailable result, preserving segment metrics.
    """
    run = Path(run)
    manifest = Path(chain_path) if chain_path is not None else run / RECOVERY_CHAIN
    if chain_path is None and not manifest.is_file():
        return None
    try:
        chain = json.loads(manifest.read_text())
        tags = _chain_tags(chain, run.name)
        segments = [_recovery_segment(run.parent / tag, tag) for tag in tags]
        if any(segment["seat"] != segments[0]["seat"]
               or segment["home"] != segments[0]["home"] for segment in segments):
            raise ValueError("native seat or original home-capital plot differs across recoveries")
        retained = []
        for segment in segments:
            for previous in retained:
                previous["rows"] = [row for row in previous["rows"]
                                    if row.get("kind") == "seat"
                                    or row.get("turn", segment["first"]) < segment["first"]]
            retained = [previous for previous in retained
                        if any(row.get("kind") == "state"
                               and isinstance(row.get("cities"), list)
                               for row in previous["rows"])]
            retained.append(dict(segment))
        totals = _conquest_totals(row for segment in retained for row in segment["rows"])
        if totals is None:
            raise ValueError("recovery path has no ownership observations")
        used = []
        for segment in retained:
            turns = [row["turn"] for row in segment["rows"]
                     if row.get("kind") == "state" and isinstance(row.get("cities"), list)]
            used.append({"tag": segment["tag"], "first_observed_turn": min(turns),
                         "last_observed_turn": max(turns)})
        return dict(totals, scope="recovered_game_path", available=True,
                    root=chain["root"], segments_used=used,
                    discarded_segments=[tag for tag in tags if tag not in {x["tag"] for x in used}],
                    validation="explicit climb ancestry, native seats, original home-capital plot")
    except (OSError, ValueError, TypeError, KeyError) as error:
        return {"scope": "recovered_game_path", "available": False, "reason": str(error)}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("runs", type=Path, nargs="+")
    parser.add_argument("--chain", type=Path,
                        help="explicit recovery manifest for one historical run")
    args = parser.parse_args(argv)
    if args.chain is not None and len(args.runs) != 1:
        parser.error("--chain applies to exactly one run")
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
                          "conquest": conquest_totals(path) if path else None,
                          "game_conquest": recovered_conquest(run, args.chain)
                              or summary.get("game_conquest")}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
