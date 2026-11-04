"""Descriptive census, not a counterfactual native decision replay."""
import argparse
import collections
import gzip
import hashlib
import json
from pathlib import Path

STEMS = ("civvis-20261025T145013Z", "civvis-20261025T155305Z", "civvis-20261025T160321Z")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=Path, required=True)
    ap.add_argument("--repo", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    rules = json.loads((args.repo / "data/units.json").read_text())
    cards = json.loads((args.repo / "data/policies.json").read_text())
    report = {"schema": 1, "scope": "All first exported states of the three fixed development games; arithmetic opportunity only, not replay or attribution", "rules": {name: digest(args.repo / "data" / name) for name in ("units.json", "policies.json")}, "games": []}
    for stem in STEMS:
        run = args.runs / stem
        states = {}
        for line in (run / "events.jsonl").open():
            e = json.loads(line)
            if e.get("kind") == "state":
                states.setdefault(e["turn"], e)
        summary = json.loads((run / "summary.json").read_text())
        quotes = []
        missing = collections.Counter()
        for turn, state in states.items():
            discount = sum(cards.get(card.removeprefix("POLICY_").lower(), {}).get("effects", {}).get("unit_maintenance_discount", 0) for card in state.get("policies", []))
            for u in state["units"]:
                if not u.get("upgrade_to"):
                    continue
                old_name = u["kind"].removeprefix("UNIT_").lower()
                new_name = u["upgrade_to"].removeprefix("UNIT_").lower()
                # Exact alias already used by src/mirror.rs:7516.
                old_name = "llanero" if old_name == "colombian_llanero" else old_name
                new_name = "llanero" if new_name == "colombian_llanero" else new_name
                if old_name not in rules or new_name not in rules:
                    missing[(old_name, new_name)] += 1
                    continue
                old, new = rules[old_name], rules[new_name]
                if old.get("class") != "military" or old.get("promotion_class") == "recon" or old.get("domain") in ("sea", "air"):
                    continue
                scale = (1, 1.5, 2)[min(u.get("formation", 0), 2)]
                modeled_old = old.get("maintenance", 0) * scale
                observed_old = u.get("maintenance", modeled_old)
                modeled_new = new.get("maintenance", 0) * scale
                raw = modeled_new - modeled_old
                net = max(0, modeled_new - discount) - max(0, observed_old - discount)
                income, gold = state["gold_per_turn"], state["gold"]
                floor = max(30 + max(0, -income), 1.5 * state.get("unit_maintenance_total", 0))
                quote = u.get("upgrade_cost")
                funded_legal = (quote is not None and not u.get("upgrade_blocked_reason") and gold - quote >= floor)
                raw_holds = raw > 0 and income - raw < 0
                net_holds = net > 0 and income - net < 0
                quotes.append({"turn": turn, "unit": u, "discount": discount, "modeled_old_gross": modeled_old, "observed_old_gross": observed_old, "modeled_new_gross": modeled_new, "raw_delta": raw, "net_delta": net, "gold": gold, "income": income, "reserve": floor, "funded_native_legal_quote": funded_legal, "arithmetic_guard_differs": funded_legal and raw_holds != net_holds})
        sampled = {str(t): {k: states[t].get(k) for k in ("turn", "frame", "gold", "gold_per_turn", "science", "military", "unit_maintenance_total", "public_stats", "policies", "cities", "units")} for t in (50, 75, 100, 125, 150, 175, 200) if t in states}
        report["games"].append({"stem": stem, "source_sha256": {name: digest(run / name) for name in ("events.jsonl", "decisions.jsonl", "summary.json")}, "outcome": summary.get("outcome"), "combat": summary.get("combat"), "seat": summary.get("seat"), "revisions": summary.get("decider_revisions"), "first_states": len(states), "land_quotes": len(quotes), "changed_estimates": sum(q["raw_delta"] != q["net_delta"] for q in quotes), "funded_native_legal_guard_differences": sum(q["arithmetic_guard_differs"] for q in quotes), "unmapped": [{"pair": list(k), "count": v} for k, v in sorted(missing.items())], "milestones": sampled, "quotes": quotes})
    with gzip.open(args.out, "wt") as fp:
        json.dump(report, fp, sort_keys=True, separators=(",", ":"))
    for game in report["games"]:
        print({k: v for k, v in game.items() if k not in ("quotes", "milestones", "source_sha256", "seat", "combat", "outcome")})
    with gzip.open(args.out, "rt") as fp:
        assert json.load(fp) == report
    print("verified", args.out, digest(args.out))


if __name__ == "__main__":
    main()
