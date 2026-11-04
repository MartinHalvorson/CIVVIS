"""Compare selected diagnostic replays by actual gene tags and full seat rows."""
import argparse
import hashlib
import json
from pathlib import Path

TAG = "first-launch-keeps-research"
PROFILE = ("target_mix", "player_contract", "players", "width", "height", "turns",
           "city_states", "speed", "map", "randomize_civs", "victories", "all_seats",
           "native_competitions", "difficulty", "contested_field", "victory_mask")


def load(path):
    raw = path.read_bytes()
    records = [json.loads(line) for line in raw.splitlines()]
    header = next(r for r in records if r.get("kind") == "header")
    seats = {(r["seed"], r["seat"]): r for r in records if r.get("kind") == "game"}
    return header, seats, hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("reference", "candidate", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--candidate-off", action="store_true")
    parser.add_argument("--require-identical", action="store_true")
    args = parser.parse_args()
    ah, ar, ash = load(args.reference)
    bh, br, bsh = load(args.candidate)
    assert ar.keys() == br.keys() and ar, "incomplete or different seed/seat sets"
    assert all(ah.get(k) == bh.get(k) for k in PROFILE), "game profile differs"
    ap, bp = dict(zip(ah["genes"], ah["prior"])), dict(zip(bh["genes"], bh["prior"]))
    assert all(ap[k] == bp.get(k) for k in ap if not (args.candidate_off and k == TAG))
    added = {k: bp[k] for k in bp if k not in ap}
    assert all(p == 0 for p in added.values()), "new genes are not fixed OFF"
    result = []
    for key in sorted(ar):
        old, new = ar[key], br[key]
        assert len(old["genome"]) == len(ah["genes"])
        assert len(new["genome"]) == len(bh["genes"])
        ag, bg = (dict(zip(h["genes"], (b == "1" for b in r["genome"])))
                  for h, r in ((ah, old), (bh, new)))
        differences = [g for g in ag if ag[g] != bg.get(g)]
        assert all(g == TAG and args.candidate_off for g in differences), "another gene changed"
        assert all(not bg[g] for g in added)
        if args.candidate_off:
            assert bg[TAG] is False
        assert old["civ"] == new["civ"], "civilization assignment differs"
        fields = [k for k in old.keys() | new.keys()
                  if k not in ("secs", "genome") and old.get(k) != new.get(k)]
        trace = [{"turn": x.get("turn"),
                  "fields": [k for k in x.keys() | y.keys() if x.get(k) != y.get(k)]}
                 for x, y in zip(old.get("trace", []), new.get("trace", [])) if x != y]
        result.append({"seed": key[0], "seat": key[1], "assigned": old["player_target"],
                       "enabled_original": ag[TAG], "gene_differences": differences,
                       "changed_fields": sorted(fields), "trace_differences": trace,
                       "original_projects": old.get("project_turns", {}),
                       "candidate_projects": new.get("project_turns", {}),
                       "original_win": old["win"], "candidate_win": new["win"],
                       "original_turn": old["turn"], "candidate_turn": new["turn"]})
    report = {"reference_sha256": ash, "candidate_sha256": bsh,
              "candidate_off": args.candidate_off, "ignored_fields": ["secs", "genome"],
              "genomes_verified_by_tag": True, "new_fixed_off_tags": added,
              "all_other_row_fields_equal": all(not r["changed_fields"] for r in result),
              "results": result,
              "scope": "Selected diagnostic replays, not independent strength observations. Genome differences are checked separately from full row comparisons."}
    args.out.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: report[k] for k in ("genomes_verified_by_tag", "new_fixed_off_tags", "all_other_row_fields_equal")}))
    if args.require_identical:
        assert report["all_other_row_fields_equal"], "replay differs: retained in report"


if __name__ == "__main__":
    main()
