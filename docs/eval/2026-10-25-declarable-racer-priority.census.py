"""Post-hoc eligibility census; this does not establish a safe air mission.

The fixed registration names the last eighty terminal Emperor Domination
games. Never interpret these selected observations as a policy effect.
"""
import argparse
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--registration", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    registration = json.loads(args.registration.read_text())
    rules = json.loads(Path("data/units.json").read_text())
    kinds = {"UNIT_" + name.upper() for name, spec in rules.items()
             if spec.get("domain") == "air" and spec.get("siege")}
    assert kinds, "no modeled siege aircraft discovered"
    root = Path.home() / "civvis-civ6-runs/control"
    paths = [root / tag / "events.jsonl" for game in registration["games"]
             for tag in game["segments"]]
    existing = [path for path in paths if path.exists()]
    pattern = r'"kind"\s*:\s*"(' + "|".join(re.escape(k) for k in sorted(kinds)) + ')"'
    probe = subprocess.run(["rg", "-l", "--", pattern, *map(str, existing)],
                           capture_output=True, text=True)
    if probe.returncode not in (0, 1):
        raise RuntimeError(probe.stderr)
    matched = set(probe.stdout.splitlines())
    args.out.mkdir(exist_ok=False)
    result = {"registration": registration, "aircraft_kinds": sorted(kinds),
              "source_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "files_existing": len(existing), "files_with_aircraft_token": len(matched),
              "games": [], "totals": Counter(), "candidate_frames": [],
              "scope": "ready aircraft plus a declarable short Culture clock; mission safety unproven"}
    with gzip.open(args.out / "first-candidate-states.jsonl.gz", "wt") as states:
        for game in registration["games"]:
            entry = {"stem": game["stem"], "segments": [], "candidate_turns": 0}
            retained_state = False
            for tag in game["segments"]:
                path = root / tag / "events.jsonl"
                segment = {"tag": tag, "status": "missing_events", "first_states": 0,
                           "aircraft_turns": 0, "ready_aircraft_turns": 0,
                           "candidate_turns": 0, "unreadable_rows": 0}
                entry["segments"].append(segment)
                if not path.exists():
                    result["totals"]["missing_events"] += 1
                    continue
                if str(path) not in matched:
                    segment["status"] = "no_modeled_aircraft_token"
                    continue
                segment["status"] = "scanned"
                seen = set()
                with path.open() as stream:
                    for line in stream:
                        try:
                            state = json.loads(line)
                        except ValueError:
                            segment["unreadable_rows"] += 1
                            continue
                        if state.get("kind") != "state" or state.get("turn") in seen:
                            continue
                        seen.add(state["turn"])
                        segment["first_states"] += 1
                        aircraft = [u for u in state.get("units", []) if u.get("kind") in kinds]
                        if not aircraft:
                            continue
                        segment["aircraft_turns"] += 1
                        ready = [u for u in aircraft if u.get("hp", 0) >= 80
                                 and u.get("moves", 0) > 0
                                 and u.get("attacks_remaining", 0) > 0]
                        if not ready:
                            continue
                        segment["ready_aircraft_turns"] += 1
                        # Exported rivals are met, alive majors:
                        # CivvisControlAgent.lua:8205 GetAliveMajorIDs.
                        racers = [r for r in state.get("rivals", [])
                                  if not r.get("at_war", True) and r.get("can_declare") is True
                                  and isinstance(r.get("culture_turns_to_victory"), (int, float))
                                  and 0 <= r["culture_turns_to_victory"] <= 20]
                        if not racers:
                            continue
                        segment["candidate_turns"] += 1
                        entry["candidate_turns"] += 1
                        result["candidate_frames"].append({"tag": tag, "turn": state["turn"],
                            "aircraft": [{k: u.get(k) for k in
                                ("id", "kind", "x", "y", "hp", "moves", "attacks_remaining")}
                                         for u in ready],
                            "racers": [{k: r.get(k) for k in
                                ("player", "civ", "military", "culture_turns_to_victory",
                                 "foreign_tourists", "at_war", "can_declare")}
                                       for r in racers]})
                        if not retained_state:
                            states.write(json.dumps({"tag": tag, "state": state}) + "\n")
                            retained_state = True
                for key in ("first_states", "aircraft_turns", "ready_aircraft_turns", "candidate_turns"):
                    result["totals"][key] += segment[key]
            result["games"].append(entry)
            print(game["stem"], "candidate turns", entry["candidate_turns"], flush=True)
    result["totals"]["games"] = len(result["games"])
    result["totals"]["candidate_games"] = sum(g["candidate_turns"] > 0 for g in result["games"])
    result["totals"] = dict(result["totals"])
    (args.out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result["totals"], indent=2), flush=True)


if __name__ == "__main__":
    main()
