"""Follow every settler from the turn it appears to the turn it stops existing.

    python3 tools/civ6_settler_trace.py --run TAG
    python3 tools/civ6_settler_trace.py --all --since civvis-20260730

⚠ PEAK CITY COUNT HAS BEEN 2 IN EVERY RUN OF THE LADDER while five to nine distinct
settlers existed in each. Both of the obvious explanations — "no settlers are built"
and "the sites are bad" — are refuted by that pair of numbers, and the ledger's
`cities` column cannot distinguish the ones that remain:

  a settler that DIED before it arrived        (barbarians, or a rival)
  a settler that ARRIVED and was REFUSED       (fog: a rival city too close)
  a settler that WALKED FOREVER                (a target it never reaches)
  a settler that STOOD STILL                   (an order that never lands)

They need completely different repairs, and the whole project has been guessing
between them. This prints, per settler: how long it lived, how far it got from where
it was built, how much of its life it spent motionless, and what became of it.

FATE is decided by evidence, never by assumption:

  founded    our city count rose on the turn after it was last seen, at a plot the
             settler could have been standing on. That is the only positive proof a
             settler became a city.
  refused    the game emitted `found_refused` for it, with Civilization VI's own
             reason.
  lost       it stopped existing and no city appeared. ⚠ This is inferred, and it
             includes any settler the export dropped for an unrelated reason.
  alive      still on the board when the run ended.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path

RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"


def trace(run: Path) -> dict:
    states, refused, choices = [], [], []
    for line in (run / "events.jsonl").read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        kind = event.get("kind")
        if kind == "state":
            states.append(event)
        elif kind == "found_refused":
            refused.append(event)
        elif kind == "settle_choice":
            choices.append(event)

    # A settler's life, keyed by the Civilization VI unit id.
    life: dict[int, dict] = {}
    cities_at: dict[int, set] = {}
    for state in states:
        turn = state.get("turn")
        cities_at[turn] = {(c["x"], c["y"]) for c in (state.get("cities") or [])}
        for unit in state.get("units") or []:
            if unit.get("kind") != "UNIT_SETTLER":
                continue
            uid = unit["id"]
            plot = (unit["x"], unit["y"])
            record = life.setdefault(uid, {
                "born": turn, "born_at": plot, "last": turn, "last_at": plot,
                "turns": 0, "moves": 0, "still": 0, "path": [],
            })
            if record["last_at"] == plot and record["last"] != turn:
                record["still"] += 1
            elif record["last"] != turn:
                record["moves"] += 1
            record["last"], record["last_at"] = turn, plot
            record["turns"] += 1
            if not record["path"] or record["path"][-1][1] != plot:
                record["path"].append((turn, plot))

    refusals_by_unit: dict[int, list] = defaultdict(list)
    for event in refused:
        refusals_by_unit[event.get("unit")].append(event)

    turns = sorted(cities_at)
    last_turn = turns[-1] if turns else 0

    def hexdist(a, b) -> int:
        # Offset (odd-r) distance, via axial. Both sides of this bridge speak offset,
        # and a plain Chebyshev distance would understate a diagonal walk.
        def axial(p):
            col, row = p
            return col - (row - (row & 1)) // 2, row
        (aq, ar), (bq, br) = axial(a), axial(b)
        return max(abs(aq - bq), abs(ar - br), abs((-aq - ar) - (-bq - br)))

    out = []
    for uid, record in sorted(life.items()):
        fate = "alive" if record["last"] >= last_turn else "lost"
        if fate == "lost":
            # Did a city appear where it last stood, on the next turn we saw?
            after = [t for t in turns if t > record["last"]]
            if after:
                gained = cities_at[after[0]] - cities_at[record["last"]]
                if any(hexdist(plot, record["last_at"]) <= 1 for plot in gained):
                    fate = "founded"
        mine = refusals_by_unit.get(uid, [])
        out.append({
            "unit": uid,
            "born": record["born"],
            "died": record["last"],
            "lived": record["last"] - record["born"] + 1,
            "from": record["born_at"],
            "to": record["last_at"],
            "distance": hexdist(record["born_at"], record["last_at"]),
            "moved_turns": record["moves"],
            "still_turns": record["still"],
            "refusals": len(mine),
            "why": Counter(e.get("why", "?")[:60] for e in mine).most_common(2),
            "fate": fate,
        })
    return {
        "run": run.name,
        "last_turn": last_turn,
        "peak_cities": max((len(v) for v in cities_at.values()), default=0),
        "final_cities": len(cities_at.get(last_turn, set())),
        "settlers": out,
        "settle_choices": len(choices),
        "found_refused_total": len(refused),
        "found_refused_why": Counter(e.get("why", "?")[:60] for e in refused).most_common(5),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", default=None)
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--since", default="", help="only runs whose name sorts at or after this")
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    if args.all:
        runs = [p for p in sorted(RUN_ROOT.iterdir())
                if (p / "events.jsonl").exists() and p.name >= args.since]
    elif args.run:
        runs = [RUN_ROOT / args.run]
    else:
        runs = [max((p for p in RUN_ROOT.iterdir() if (p / "events.jsonl").exists()),
                    key=lambda p: (p / "events.jsonl").stat().st_mtime)]

    totals: Counter = Counter()
    handle = open(args.json, "a") if args.json else None
    for run in runs:
        if not (run / "events.jsonl").exists():
            continue
        report = trace(run)
        if handle:
            handle.write(json.dumps(report, default=str) + "\n")
        if not report["settlers"]:
            continue
        print(f"{report['run']}  t{report['last_turn']}  cities peak "
              f"{report['peak_cities']} final {report['final_cities']}  "
              f"found_refused {report['found_refused_total']} {report['found_refused_why']}")
        for s in report["settlers"]:
            totals[s["fate"]] += 1
            print(f"    u{s['unit']:<9} t{s['born']}-{s['died']} ({s['lived']:>3}t)  "
                  f"{s['from']}->{s['to']} d={s['distance']:<3} "
                  f"moved {s['moved_turns']:>3} still {s['still_turns']:>3}  "
                  f"refused {s['refusals']:<3} {s['fate']}"
                  + (f"  {s['why']}" if s["why"] else ""))
    if handle:
        handle.close()
    if totals:
        total = sum(totals.values())
        print(f"\nfates over {total} settlers: {dict(totals.most_common())}")
        founded = totals.get("founded", 0)
        print(f"cities per settler: {founded}/{total} = {founded / total:.2f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
