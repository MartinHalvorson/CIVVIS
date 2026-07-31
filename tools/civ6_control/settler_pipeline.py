#!/usr/bin/env python3
"""Where does the settler pipeline actually leak?

`applied: true` cannot answer this — 418 settler build requests came back applied while
38 cities were founded, and that is consistent with BOTH "the engine never built one"
(the PARAM_INSERT_MODE failure, controller bug #1) and "the agent re-asserts the queue
every turn". Same reading, opposite fixes.

`state.units` carries a per-unit `id` and `kind`, so distinct UNIT_SETTLER ids is the
count of settlers that actually EXISTED. That separates the two:

    few ids, many build requests  -> production never converts (a build-layer bug)
    many ids, few cities          -> settlers exist and never found (a movement/siting bug)
"""
import json, glob, os, sys

os.chdir(os.path.expanduser("~/civvis-civ6-runs/control"))
hdr = ("run", "end_t", "cities", "settler_ids", "build_reqs", "found_city", "move_to_site")
print("%8s %6s %7s %12s %11s %11s %13s" % hdr)
tot = dict(ids=0, cities=0, reqs=0, found=0, mts=0)
for d in sorted(glob.glob("settler-*"))[-12:]:
    p = os.path.join(d, "events.jsonl")
    if not os.path.exists(p):
        continue
    ids, last, mc, reqs, found, mts = set(), 0, 0, 0, 0, 0
    for line in open(p, errors="replace"):
        line = line.strip()
        if not line:
            continue
        try:
            e = json.loads(line)
        except Exception:
            continue
        k = e.get("kind")
        if k == "state":
            for u in e.get("units") or []:
                if u.get("kind") == "UNIT_SETTLER":
                    ids.add(u.get("id"))
        elif k == "turn":
            last = max(last, e.get("turn") or 0)
            mc = max(mc, e.get("cities") or 0)
            for a, n in (e.get("actions") or {}).items():
                if "found_city" in a:
                    found += n
                elif "move_to_site" in a:
                    mts += n
        elif k == "build" and "SETTLER" in str(e.get("item", "")).upper():
            reqs += 1
    if last:
        print("%8s %6d %7d %12d %11d %11d %13d"
              % (d[-7:], last, mc, len(ids), reqs, found, mts))
        tot["ids"] += len(ids); tot["cities"] += mc
        tot["reqs"] += reqs; tot["found"] += found; tot["mts"] += mts
print()
print("TOTALS  settlers that existed=%d  build requests=%d  found_city=%d  "
      "move_to_site=%d  peak cities summed=%d"
      % (tot["ids"], tot["reqs"], tot["found"], tot["mts"], tot["cities"]))
if tot["ids"] and tot["reqs"]:
    print("build requests per settler actually produced: %.1f"
          % (tot["reqs"] / tot["ids"]))
    print("cities per settler produced:                  %.2f"
          % (tot["cities"] / tot["ids"]))
