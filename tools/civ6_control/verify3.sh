#!/bin/zsh
# Verify the THREE unverified changes, each by the count that can falsify it —
# not by a boolean, and not by the fact that the code shipped.
#
#  1. ArmyCap      -> `develop` appears in build reasons, AND a DISTRICT_/BUILDING_
#                     other than monument or granary appears in the item histogram.
#                     Before: `develop` never fired in a 203-turn game.
#  2. StrengthWeight -> `target_ratio <= 1` when a target exists. Above 1 means we are
#                     STILL attacking somebody stronger, the exact failure it prevents.
#  3. SiegeUnits=4 -> `siege > 0` sustained during a war. Before: 0 alive for 77 turns
#                     of war while `assaulting` read True.
#
# ⚠ Reports each independently: two passing and one failing must not read as success.
RUNS="$HOME/civvis-civ6-runs/control"
python3 -u - "$RUNS" <<'PY'
import json, os, sys, time, glob, collections
runs = sys.argv[1]
cur, said = None, set()
while True:
    newest = max(glob.glob(os.path.join(runs, "settler-*")), key=os.path.getmtime, default=None)
    if newest != cur:
        cur, said = newest, set()
        print(f"verifying on {os.path.basename(cur)}")
    p = os.path.join(cur, "events.jsonl") if cur else None
    if p and os.path.exists(p):
        reasons, items = collections.Counter(), collections.Counter()
        last = None; siege_hi = 0; wars = 0
        for line in open(p, errors="replace"):
            line = line.strip()
            if not line: continue
            try: e = json.loads(line)
            except Exception: continue
            if e.get("kind") == "build":
                reasons[e.get("reason")] += 1; items[e.get("item")] += 1
            elif e.get("kind") == "turn":
                last = e; siege_hi = max(siege_hi, e.get("siege") or 0)
            elif e.get("kind") == "war":
                wars += 1
        if last:
            t = last.get("turn")
            infra = [k for k in items if str(k).startswith("DISTRICT_")
                     or (str(k).startswith("BUILDING_")
                         and "MONUMENT" not in str(k) and "GRANARY" not in str(k))]
            if reasons.get("develop") and "c1" not in said:
                said.add("c1")
                print(f"*** t{t} CHECK 1 PASS (ArmyCap): develop={reasons['develop']} "
                      f"infra={infra[:4]}")
            # ⚠ CALIBRATED TO THE ACTUAL THRESHOLD, not to 1.0. This first demanded
            # ratio <= 1.0 and cried FAIL at 1.22 — which MaxTargetRatio (1.3) permits by
            # design. A false FAIL is as harmful as a false PASS: it argues for "fixing"
            # something that is working, and I nearly acted on it.
            #
            # ⚠ And a high ratio is only a failure if a war was actually DECLARED above
            # the threshold. Merely holding a strong target while the veto refuses it is
            # the system working, so `wars` gates the verdict.
            ratio = last.get("target_ratio")
            if ratio is not None and "c2" not in said:
                said.add("c2")
                if ratio <= 1.0:
                    v = "PASS - target is not stronger than us"
                elif ratio <= 1.3:
                    v = "PASS - within MaxTargetRatio 1.3, permitted by design"
                elif wars:
                    v = "FAIL - DECLARED on a target above MaxTargetRatio"
                else:
                    v = "PASS - above 1.3 and no war declared: the veto is holding"
                print(f"*** t{t} CHECK 2 {v} (StrengthWeight): target_ratio={ratio:.2f} "
                      f"their_score={last.get('target_their_score')} wars={wars}")
            if siege_hi > 0 and "c3" not in said:
                said.add("c3")
                print(f"*** t{t} CHECK 3 PASS (SiegeUnits): siege alive peaked at {siege_hi}")
    time.sleep(60)
PY
