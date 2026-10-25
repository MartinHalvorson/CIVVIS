#!/usr/bin/env python3
"""Archive every unit promotion in the four pre-existing native pilot runs."""

import argparse
from collections import Counter, defaultdict
import gzip
import hashlib
import json
from pathlib import Path
import sqlite3


RUNS = (
    "civvis-20261025T032555Z",
    "civvis-20261025T034102Z",
    "civvis-20261025T035310Z",
    "civvis-20261025T040737Z",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs-root", type=Path, required=True)
    parser.add_argument("--assets", type=Path, required=True)
    parser.add_argument("--gameplay-db", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    bundle = {"schema": 1, "selection": list(RUNS), "sources": [], "blobs": {}, "promotions": []}

    def blob(raw):
        digest = hashlib.sha256(raw.encode()).hexdigest()
        bundle["blobs"][digest] = raw
        return digest

    def source(path):
        raw = path.read_bytes()
        record = {"path": str(path), "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}
        bundle["sources"].append(record)
        return raw.decode().splitlines(keepends=True)

    xml = args.assets / "DLC/GranColombia_Maya/Data/GranColombia_Maya_Civilizations.xml"
    xml_lines = source(xml)
    bundle["shipped_xml"] = [
        {"start_line": start, "end_line": end, "blob": blob("".join(xml_lines[start - 1:end]))}
        for start, end in ((295, 296), (327, 328), (361, 363))
    ]
    db = sqlite3.connect(args.gameplay_db.resolve().as_uri() + "?mode=ro", uri=True)
    db.row_factory = sqlite3.Row
    bundle["gameplay_rows"] = {
        "TraitModifiers": [dict(r) for r in db.execute(
            "SELECT * FROM TraitModifiers WHERE ModifierId = ?", ("TRAIT_PROMOTE_NO_FINISH_MOVES",))],
        "Modifiers": [dict(r) for r in db.execute(
            "SELECT * FROM Modifiers WHERE ModifierId = ?", ("TRAIT_PROMOTE_NO_FINISH_MOVES",))],
        "ModifierArguments": [dict(r) for r in db.execute(
            "SELECT * FROM ModifierArguments WHERE ModifierId = ?", ("TRAIT_PROMOTE_NO_FINISH_MOVES",))],
    }
    db.close()
    for table in bundle["gameplay_rows"].values():
        if not table:
            raise ValueError("active gameplay database lacks the registered modifier")

    for run in RUNS:
        folder = args.runs_root / "control" / run
        events = source(folder / "events.jsonl")
        decisions = source(folder / "decisions.jsonl")
        states = []
        identities = defaultdict(list)
        for line, raw in enumerate(events):
            event = json.loads(raw)
            if event.get("kind") == "state":
                identities[(event["turn"], event.get("frame", 0))].append(len(states))
                states.append((line, event))
        for name in ("summary.json",):
            path = folder / name
            if path.exists():
                bundle.setdefault("context", {})[str(path)] = blob("".join(source(path)))
        observation = args.runs_root / "experiments/2026-10-24-army-upgrade-native-pilot/observations" / run
        for name in ("seat-readback.json", "genome-header.json", "binary-provenance.json"):
            path = observation / name
            if path.exists():
                bundle.setdefault("context", {})[str(path)] = blob("".join(source(path)))
        for decision_line, raw in enumerate(decisions):
            receipt = json.loads(raw)
            decision = receipt.get("decision", {})
            for action in decision.get("native_actions", []):
                if action.get("type") != "promote":
                    continue
                identity = (decision["turn"], decision.get("frame", 0))
                native = decision.get("unit_ids", {}).get(str(action["unit"]))
                entry = {
                    "run": run, "identity": identity, "decision_line": decision_line + 1,
                    "decision": blob(raw), "action": action, "native_unit": native, "flags": [],
                }
                bundle["promotions"].append(entry)
                flags = entry["flags"]
                matches = identities.get(identity, [])
                if native is None or len(matches) != 1:
                    flags.append("missing_native_id_or_ambiguous_before_state")
                    continue
                index = matches[0]
                before_line, before = states[index]
                entry["before_state"] = {"line": before_line + 1, "blob": blob(events[before_line])}
                if index + 1 == len(states):
                    flags.append("missing_immediate_next_state")
                    continue
                after_line, after = states[index + 1]
                entry["after_state"] = {"line": after_line + 1, "blob": blob(events[after_line])}
                entry["intervening_events"] = [blob(row) for row in events[before_line + 1:after_line]]
                if after["turn"] != before["turn"]:
                    flags.append("cross_turn")
                if (after["turn"], after.get("frame", 0)) <= identity:
                    flags.append("non_increasing_state_identity")
                old = [u for u in before["units"] if u["id"] == native]
                new = [u for u in after["units"] if u["id"] == native]
                if len(old) != 1 or len(new) != 1:
                    flags.append("missing_or_ambiguous_unit")
                    continue
                old, new = old[0], new[0]
                entry["before_unit"], entry["after_unit"] = old, new
                orders = [o for o in receipt.get("orders", []) if o.get("kind") == "unit" and o.get("subject") == native]
                promotions = [o for o in orders if str(o.get("verb", "")).startswith("PROMOTE:")]
                if len(promotions) != 1:
                    flags.append("missing_or_ambiguous_promotion_order")
                elif promotions[0]["verb"].split(":", 1)[1] not in set(new.get("promotions", [])) - set(old.get("promotions", [])):
                    flags.append("promotion_not_newly_observed")
                if len(orders) != len(promotions):
                    flags.append("other_order_for_same_unit")
                for row in events[before_line + 1:after_line]:
                    event = json.loads(row)
                    if event.get("kind") == "moves_restored":
                        flags.append("movement_restoration_in_interval")
                    if event.get("kind") == "combat":
                        flags.append("combat_in_interval_conservative_exclusion")
                    if event.get("unit") == native and event.get("kind") == "promotion_refused":
                        flags.append("promotion_refused_in_interval")
                if (old.get("x"), old.get("y")) != (new.get("x"), new.get("y")):
                    flags.append("position_changed")
                if new.get("hp") != min(100, old.get("hp", 0) + 50):
                    flags.append("heal_not_isolated")
                if new.get("level") != old.get("level", 0) + 1 or new.get("xp") != old.get("xp"):
                    flags.append("experience_or_level_not_isolated")
                entry["budgets_preserved"] = all(old.get(k) == new.get(k) for k in ("moves", "max_moves", "attacks_remaining"))
                entry["flags"] = sorted(set(flags))
                entry["isolated_same_turn_promotion"] = not flags
    bundle["summary"] = {
        "candidates": len(bundle["promotions"]),
        "isolated": sum(r.get("isolated_same_turn_promotion", False) for r in bundle["promotions"]),
        "isolated_budgets_preserved": sum(r.get("isolated_same_turn_promotion", False) and r.get("budgets_preserved", False) for r in bundle["promotions"]),
        "flags": dict(Counter(f for r in bundle["promotions"] for f in r["flags"])),
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("xb") as target:
        with gzip.GzipFile(fileobj=target, mode="wb", mtime=0) as compressed:
            compressed.write((json.dumps(bundle, sort_keys=True, separators=(",", ":")) + "\n").encode())
    print(json.dumps({"out": str(args.out), "sha256": hashlib.sha256(args.out.read_bytes()).hexdigest(), "bytes": args.out.stat().st_size, **bundle["summary"]}, sort_keys=True))


if __name__ == "__main__":
    main()
