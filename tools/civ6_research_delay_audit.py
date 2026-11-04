#!/usr/bin/env python3
"""Measure research *selection* intervals in completed native ladder attempts.

Read-only: never touches the live game. Intervals include research time and
required prerequisites; they are NOT estimates of turns a treatment saves.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import statistics

CHOICE = re.compile(r"^\[why\] t(\d+) Research/Decision Researching ([a-z ]+) \| (.*)$")
MILESTONES = ("writing", "education", "chemistry", "rocketry", "satellites")
MILITARY_REASONS = ("barbarian army", "standing army at war", "decisive-window")


def prerequisite_closure(techs, goal):
    pending, seen = [goal], set()
    while pending:
        node = pending.pop()
        if node not in seen:
            seen.add(node)
            pending.extend(techs[node].get("requires", []))
    return seen


def native_milestones(path):
    """First observed owned technologies, from authoritative native states."""
    first, digest = {}, hashlib.sha256()
    with path.open("rb") as stream:
        for line in stream:
            digest.update(line)
            row = json.loads(line)
            if row.get("kind") != "state":
                continue
            for tech in MILESTONES:
                if "TECH_" + tech.upper() in row.get("techs", []):
                    first[tech] = min(first.get(tech, row["turn"]), row["turn"])
    return {"first_observed_owned_turns": first, "events_sha256": digest.hexdigest()}


def parse_choices(content):
    """Keep distinct decisions, collapsing repeated same-turn frame output."""
    choices, seen = [], set()
    for line in content.splitlines():
        match = CHOICE.match(line)
        if not match:
            continue
        turn, tech, reason = int(match[1]), match[2].replace(" ", "_"), match[3]
        key = (turn, tech)
        if key not in seen:
            choices.append({"turn": turn, "tech": tech, "reason": reason})
            seen.add(key)
    return sorted(choices, key=lambda row: row["turn"])


def summarize_attempt(attempt, choices):
    first = {}
    for row in choices:
        first.setdefault(row["tech"], row["turn"])
    rocket, satellite = first.get("rocketry"), first.get("satellites")
    interval = [row for row in choices if rocket is not None
                and row["turn"] > rocket
                and (satellite is None or row["turn"] < satellite)]
    result = {key: attempt.get(key) for key in (
        "tag", "utc", "difficulty", "victory_target", "won", "reason", "turns",
        "victory_type", "revisions", "spaceport_turn")}
    result.update(
        selection_turns={key: first.get(key) for key in MILESTONES},
        rocket_to_satellite_selection_turns=(satellite - rocket
            if rocket is not None and satellite is not None else None),
        satellite_unobserved_after_rocketry=rocket is not None and satellite is None,
        choices_between_rocket_and_satellite=interval,
        military_choices_between_rocket_and_satellite=[row for row in interval
            if any(reason in row["reason"] for reason in MILITARY_REASONS)],
    )
    return result


def audit(root, since):
    ladder_bytes = (root / "ladder.json").read_bytes()
    attempts = json.loads(ladder_bytes)["attempts"]
    rows, excluded, seen = [], [], set()
    tech_bytes = (Path(__file__).resolve().parents[1] / "data/techs.json").read_bytes()
    satellite_path = prerequisite_closure(json.loads(tech_bytes), "satellites")
    for attempt in attempts:
        if attempt.get("utc", "") < since or not attempt.get("configured"):
            continue
        tag = attempt["tag"]
        if tag in seen:
            raise ValueError(f"duplicate ladder attempt: {tag}")
        seen.add(tag)
        if Path(tag).name != tag:
            raise ValueError(f"unsafe run tag: {tag}")
        path = root / tag / "why.log"
        if not path.exists():
            excluded.append({"tag": tag, "reason": "missing why.log"})
            continue
        content = path.read_bytes()
        choices = parse_choices(content.decode("utf-8", errors="replace"))
        if not choices:
            excluded.append({"tag": tag, "reason": "no research selections"})
            continue
        row = summarize_attempt(attempt, choices)
        for choice in row["military_choices_between_rocket_and_satellite"]:
            choice["on_model_satellites_prerequisite_path"] = choice["tech"] in satellite_path
        events = root / tag / "events.jsonl"
        row["native_witness"] = native_milestones(events) if events.exists() else None
        row["why_sha256"] = hashlib.sha256(content).hexdigest()
        rows.append(row)
    groups = {}
    for row in rows:
        # Abandonments are retained, but must not dilute the terminal win rate.
        status = "abandoned" if row["reason"] == "abandoned" else "other"
        key = f'{row["difficulty"]}/{row["victory_target"]}/{status}'
        groups.setdefault(key, []).append(row)
    summary = {}
    for key, group in sorted(groups.items()):
        intervals = [row["rocket_to_satellite_selection_turns"] for row in group
                     if row["rocket_to_satellite_selection_turns"] is not None]
        summary[key] = {
            "attempts": len(group), "wins": sum(row["won"] is True for row in group),
            "rocketry_selected": sum(row["selection_turns"]["rocketry"] is not None for row in group),
            "satellite_unobserved_after_rocketry": sum(row["satellite_unobserved_after_rocketry"] for row in group),
            "military_in_interval": sum(bool(row["military_choices_between_rocket_and_satellite"]) for row in group),
            "military_off_model_satellite_path": sum(any(not choice["on_model_satellites_prerequisite_path"] for choice in row["military_choices_between_rocket_and_satellite"]) for row in group),
            "observed_interval_count": len(intervals),
            "median_selection_interval": statistics.median(intervals) if intervals else None,
        }
    return {"since": since, "source": str(root),
            "ladder_sha256": hashlib.sha256(ladder_bytes).hexdigest(),
            "model_tech_graph_sha256": hashlib.sha256(tech_bytes).hexdigest(),
            "interpretation": "Selection times, not completion times or causal savings. Missing Satellites are censored. Other outcomes can include interruptions; inspect reason and victory_type.",
            "summary": summary, "excluded": excluded, "attempts": rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path, help="native control directory containing ladder.json")
    parser.add_argument("--since", required=True, help="inclusive ISO UTC lower bound")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    result = audit(args.root, args.since)
    args.out.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result["summary"], indent=2))


if __name__ == "__main__":
    main()
