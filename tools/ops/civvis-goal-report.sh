#!/bin/zsh
# Read the exhibition's own ledger and say which strategies are actually
# winning. Placement rating and win rate disagree in this league by design --
# the league breeds on mean placement while the promotion gate counts wins --
# so both are printed side by side and neither is called "the" answer.
#
# Usage: civvis-goal-report.sh [first-round]   (default: all rounds on file)
exec /usr/bin/python3 - "$@" <<'PY'
import csv
import json
import sys
from collections import defaultdict

DIR = "$HOME/civvis-spectator-src/league"
since = int(sys.argv[1]) if len(sys.argv) > 1 else 0

league = json.load(open(f"{DIR}/league.json"))
info = {s["name"]: s for s in league["strategies"]}

games = wins = 0
seen = defaultdict(lambda: {"g": 0, "w": 0, "place": 0.0, "seats": 0, "par": 0.0})
victories = defaultdict(int)
turns = []

with open(f"{DIR}/matches.csv") as fh:
    for row in csv.DictReader(fh):
        if int(row["round"]) < since:
            continue
        games += 1
        victories[row["victory"]] += 1
        turns.append(int(row["turns"]))
        seats = row["placements"].split("|")
        for place, seat in enumerate(seats):
            name = seat.split("@")[0]
            rec = seen[name]
            rec["g"] += 1
            # A win is worth less at a crowded table. Carrying each game's own
            # parity share makes 4-, 6- and 10-seat games comparable; a raw
            # win% across mixed seat counts ranks the strategy that happened
            # to draw the small tables.
            rec["par"] += 1.0 / len(seats)
            rec["seats"] += len(seats)
            # Normalised placement: 1.0 = won, 0.0 = last. Comparable across
            # the 4-to-10 seat counts the exhibition mixes.
            rec["place"] += 1.0 - place / max(len(seats) - 1, 1)
            if place == 0:
                rec["w"] += 1
                wins += 1

if not games:
    print(f"no rated games at or after round {since} yet")
    raise SystemExit(0)

print(f"{games} rated games from round {since} "
      f"(league is at round {league['round']}), median end turn "
      f"{sorted(turns)[len(turns) // 2]}")
print("victories: " + ", ".join(f"{k} {v}" for k, v in
                                sorted(victories.items(), key=lambda kv: -kv[1])))
capped = sum(1 for t in turns if t >= 250)
print(f"{100 * capped / games:.0f}% ended ON THE 250 CAP -- at this budget most games are decided by")
print("score when the clock runs out, not by winning. `win%`/`vs par`/`place` below are")
print("computed only from games in this round range, so they describe THIS format.")
print("The `rating` column does NOT: it is a Glicko scale carried over from longer")
print("stock-budget games (median end t233, 45% diplomatic at 6 seats) blended with")
print("these. Rank on the ledger columns; treat rating as seeding only.")
print()
print(f"{'strategy':16s} {'handle':14s} {'games':>5s} {'win%':>6s} {'vs par':>6s} "
      f"{'place':>6s} {'rating':>7s} {'rd':>5s}  note")
for name, rec in sorted(seen.items(), key=lambda kv: -kv[1]["w"] / kv[1]["par"]):
    s = info.get(name, {})
    kind = s.get("kind", {})
    note = ""
    if "Advanced" in kind:
        adv = kind["Advanced"]
        bits = []
        if adv.get("target"):
            bits.append(f"lane={adv['target']}")
        deck = adv.get("weights", {}).get("policy_deck")
        ded = adv.get("weights", {}).get("dedication_choice")
        if deck:
            bits.append(f"deck={deck}")
        if ded:
            bits.append(f"dedication={ded}")
        note = " ".join(bits)
    elif "Builtin" in kind:
        note = kind["Builtin"]["ai"]
    if s.get("born_round", 0) >= since:
        note = ("challenger " + note).strip()
    print(f"{name:16s} {s.get('username', '?'):14s} {rec['g']:5d} "
          f"{100 * rec['w'] / rec['g']:5.1f}% {rec['w'] / rec['par']:5.2f}x "
          f"{rec['place'] / rec['g']:6.3f} "
          f"{s.get('rating', float('nan')):7.1f} {s.get('rd', float('nan')):5.1f}  {note}")
PY
