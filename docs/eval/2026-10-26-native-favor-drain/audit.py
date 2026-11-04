"""Extract a descriptive ledger from a frozen native attempt, without game input."""
import argparse
import collections
import gzip
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--events", type=Path, required=True)
parser.add_argument("--registration", type=Path, required=True)
parser.add_argument("--out", type=Path, required=True)
args = parser.parse_args()
registration = json.loads(args.registration.read_text())
archive = args.events.read_bytes()
expected = registration["source_archive_files"]["events.jsonl"]
sha = lambda data: hashlib.sha256(data).hexdigest()
assert sha(archive) == expected["archive_sha256"]
raw = gzip.decompress(archive)
assert sha(raw) == expected["sha256"] and len(raw) == expected["bytes"]
first = {}
congress, policies, deals, occupations, outcomes = [], [], [], [], []
kinds = collections.Counter()
for line in raw.splitlines():
    event = json.loads(line)
    assert event["run"] == registration["run"]
    kind = event["kind"]
    kinds[kind] += 1
    if kind == "state":
        first.setdefault(event["turn"], event)
    if kind.startswith("wc_"):
        congress.append(event)
    if kind == "policy_deck_readback":
        policies.append(event)
    if kind.startswith("deal_"):
        deals.append(event)
    if kind == "city_occupation":
        occupations.append(event)
    if kind == "victory":
        outcomes.append(event)
assert sorted(first) == list(range(1, 240))
assert len(congress) == 72
assert outcomes == [registration["outcome"]]
timeline = []
for turn, state in sorted(first.items()):
    routes = state.get("trade_routes", [])
    timeline.append({
        "turn": turn, "frame": state["frame"], "utc": state["utc"],
        "favor": state.get("favor"), "gold": state["gold"],
        "gold_per_turn": state.get("gold_per_turn"), "dvp": state.get("dvp"),
        "favor_per_turn": state.get("favor_per_turn"),
        "favor_sources": state.get("favor_sources"),
        "unit_maintenance_total": state.get("unit_maintenance_total"),
        "building_maintenance_total": state.get("building_maintenance_total"),
        "district_maintenance_total": state.get("district_maintenance_total"),
        "city_count": len(state["cities"]), "unit_count": len(state["units"]),
        "trade_capacity": state.get("trade_capacity"),
        "active_routes": len(routes),
        "active_route_gold": sum(r.get("yields", {}).get("gold", 0) for r in routes),
        "climate": state.get("climate"), "policies": state.get("policies"),
        "held_original_capitals": [
            {k: c[k] for k in ("id", "name", "original_owner", "x", "y")}
            for c in state["cities"] if c.get("original_capital")
        ],
        "rivals": [{
            "player": r["player"], "civ": r.get("civ"), "dvp": r.get("dvp"),
            "at_war": r.get("at_war"), "known_city_count": len(r.get("cities", [])),
            "grievances_against_us": r.get("grievances_against_us"),
        } for r in state["rivals"]],
    })
packet = {
    "registration": registration, "archive_verification": expected,
    "selection": "First exported state per turn, in file order. All Congress events, "
                 "policy readbacks, deals, occupations and the terminal outcome retained.",
    "limits": "Retrospective descriptive audit of a known loss. Balance differences "
              "are not the engine's per-turn Favor rate. Original exports lack that "
              "rate and its breakdown; no causal attribution to emissions, capitals "
              "or grievances, no simulated changed ballot or claim of a saved game. "
              "Fog-visible cities are known counts, not complete rival empire counts. "
              "Occupation callbacks may repeat existing ownership. Reported captures "
              "are not deduplicated occupation callbacks. Other cohort attempts, "
              "continuations, stops and source/profile changes remain included.",
    "event_kinds": dict(kinds), "timeline": timeline, "congress": congress,
    "policy_readbacks": policies, "deals": deals, "occupations": occupations,
    "outcomes": outcomes,
}
args.out.parent.mkdir(parents=True, exist_ok=True)
with gzip.GzipFile(filename=str(args.out), mode="wb", mtime=0) as stream:
    stream.write((json.dumps(packet, sort_keys=True, separators=(",", ":")) + "\n").encode())
assert json.loads(gzip.decompress(args.out.read_bytes())) == packet
print("Verified source bytes; retained 239 turn states and all 72 Congress events.")
