"""Recompute paired checkpoints and retain every game's final outcome."""
import csv
import json
from pathlib import Path
import re
import statistics
import sys

ROOT = Path(__file__).resolve().parent
PREFIX = "2026-10-04-researched-builder-yields"
PHASE = sys.argv[1] if len(sys.argv) > 1 else "pilot"
PROTOCOL = {
    "pilot": {"emperor": (61005400, 4), "deity": (61005500, 4)},
    "confirmation": {"emperor": (61005600, 12), "deity": (61005700, 8)},
}
result = {}
for difficulty, (first, count) in PROTOCOL[PHASE].items():
    expected = set(range(first, first + count))
    data, outcomes = {}, {}
    for arm in ["control", "candidate"]:
        stem = ROOT / f"{PREFIX}-{PHASE}-{difficulty}-{arm}"
        with stem.with_suffix(".csv").open() as source:
            raw = list(csv.DictReader(source))
        data[arm] = {(int(r["seed"]), int(r["turn"])): r for r in raw}
        assert len(data[arm]) == len(raw), "duplicate checkpoint"
        matches = re.findall(
            r"(\d+): turn (\d+), winner (Some\(\d+\)|None), alive (true|false)",
            stem.with_suffix(".txt").read_text(),
        )
        assert len(matches) == count and {int(s) for s, *_ in matches} == expected
        assert {s for s, _ in data[arm]} <= expected
        outcomes[arm] = {
            "games": count,
            "wins": sum(w == "Some(0)" for _, _, w, _ in matches),
            "eliminated": sum(a == "false" for _, _, _, a in matches),
            "raw": [
                {"seed": int(s), "turn": int(t), "winner": w, "alive": a == "true"}
                for s, t, w, a in matches
            ],
        }
    rows = []
    for turn in [25, 50, 75, 100, 125, 150]:
        common = sorted(k for k in data["control"].keys() & data["candidate"].keys() if k[1] == turn)
        row = {"turn": turn, "matched_games": len(common), "matched_seeds": [s for s, _ in common]}
        row["missing_checkpoint"] = {
            arm: sorted(expected - {s for s, t in data[arm] if t == turn})
            for arm in data
        }
        if not common:
            rows.append(row)
            continue
        for key in ["production", "cumulative_production", "science", "culture", "cities"]:
            a, b = [statistics.mean(float(data[arm][k][key]) for k in common) for arm in data]
            row[key] = {"control": a, "candidate": b, "change_pct": 100 * (b / a - 1) if a else None}
        deltas = [float(data["candidate"][k]["production"]) - float(data["control"][k]["production"]) for k in common]
        row["production_pairs"] = {
            "higher": sum(d > 1e-6 for d in deltas),
            "lower": sum(d < -1e-6 for d in deltas),
            "equal": sum(abs(d) <= 1e-6 for d in deltas),
        }
        row["production_to_strongest_rival"] = {}
        for arm in data:
            ratios = [float(data[arm][k]["production"]) / float(data[arm][k]["rival_production"]) for k in common if float(data[arm][k]["rival_production"]) > 0]
            row["production_to_strongest_rival"][arm] = {"mean": statistics.mean(ratios) if ratios else None, "games": len(ratios)}
        rows.append(row)
    result[difficulty] = {"outcomes": outcomes, "checkpoints": rows}
(ROOT / f"{PREFIX}-{PHASE}-summary.json").write_text(json.dumps(result, indent=2) + "\n")
for difficulty, record in result.items():
    print(difficulty, {arm: {k: v for k, v in out.items() if k != "raw"} for arm, out in record["outcomes"].items()})
    for row in record["checkpoints"]:
        if row["matched_games"]:
            print(row["turn"], row["matched_games"], *[f"{k}={row[k]['control']:.3f}->{row[k]['candidate']:.3f} ({row[k]['change_pct']:+.2f}%)" for k in ["production", "cumulative_production", "science", "culture"]], row["production_pairs"])
