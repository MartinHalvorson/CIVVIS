#!/bin/zsh
# CAPTURE, the one number that has never moved in this project's history.
#
# ⚠ There is NO `capture` event in the agent — `emit()` never writes one — so a monitor
# keyed on an event kind would stay silent forever and read as "no capture yet" whether
# or not a city changed hands. The board is the evidence instead: OUR city count rising
# while at war, or a rival's visible city count falling.
#
# ⚠ Our count also rises from SETTLING, so a rise is only a capture if it coincides with
# a rival losing one. Both sides are printed so the two cannot be confused.
RUNS="$HOME/civvis-civ6-runs/control"
python3 -u - "$RUNS" <<'PY'
import json, os, sys, time, glob
runs = sys.argv[1]
cur, prev_ours, prev_theirs, said = None, None, None, set()
while True:
    newest = max(glob.glob(os.path.join(runs, "settler-*")), key=os.path.getmtime, default=None)
    if newest != cur:
        cur, prev_ours, prev_theirs, said = newest, None, None, set()
        print(f"watching {os.path.basename(cur)} for a CAPTURE (never yet achieved)")
    p = os.path.join(cur, "events.jsonl") if cur else None
    if p and os.path.exists(p):
        ours = theirs = None; atwar = False; turn = 0; siege = 0
        for line in open(p, errors="replace"):
            line = line.strip()
            if not line: continue
            try: e = json.loads(line)
            except Exception: continue
            if e.get("kind") == "state":
                ours = len(e.get("cities") or [])
                theirs = sum(len(r.get("cities") or []) for r in e.get("rivals") or [])
                atwar = any(r.get("at_war") for r in e.get("rivals") or [])
                turn = e.get("turn") or turn
            elif e.get("kind") == "turn":
                siege = e.get("siege") or 0
        if ours is not None:
            if (prev_ours is not None and ours > prev_ours and theirs is not None
                    and prev_theirs is not None and theirs < prev_theirs and atwar):
                print(f"*** t{turn}: CITY CAPTURED — ours {prev_ours}->{ours}, "
                      f"theirs {prev_theirs}->{theirs}")
            elif prev_ours is not None and ours > prev_ours:
                print(f"t{turn}: ours {prev_ours}->{ours} (settled, not captured; "
                      f"theirs {prev_theirs}->{theirs})")
            if siege > 0 and "siege" not in said:
                said.add("siege"); print(f"t{turn}: siege units ALIVE = {siege} (floor raised to 4)")
            prev_ours, prev_theirs = ours, theirs
    time.sleep(60)
PY
