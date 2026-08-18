#!/usr/bin/env python3
"""Harvest the type names Civilization VI actually ships, into a committed snapshot.

CIVVIS names its own rules. The outbound order channel has to turn those names
into Civilization VI's, and where the two disagree the host does not complain --
it silently discards the order. Live run `civvis-20260803T014330Z` spent turns
118-250 ordering `BUILDING_ARCHAEOLOGICAL_MUSEUM` in all three of its main
cities, 248 orders, and finished with that building in none of them. That string
occurs in exactly one file on a machine with the game installed: CIVVIS's own
control mod. Civilization VI calls it `BUILDING_MUSEUM_ARTIFACT`.

So the mapping cannot be checked by reading it. It has to be checked against the
game. This writes `data/civ6_type_names.json`; `civvis_orders`'s own test reads
that snapshot and fails when a name CIVVIS can emit is not in it.

    python3 tools/civ6_type_names.py --civ6 "$CIV6_DIR"

⚠ It excludes `DLC/CivvisControl`. Our control mod is installed *into* the game's
Assets tree, so a naive scan reads our own invented names back as evidence that
the game has them -- which is exactly how this defect survived. That exclusion is
the whole point of the script; do not remove it.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys

# Every row in the shipped Data XML declares its identity in a `*Type="..."`
# attribute. Matching that, rather than any uppercase token, keeps localization
# keys, art definitions and requirement-set names out of the snapshot.
#
# ★★★ `UnitPromotionType` WAS MISSING FROM THIS LIST, AND THAT IS WHY THE CLASS
# REOPENED. The docstring above describes a building name discarded 248 times;
# the same defect then shipped in promotions, because a family the harvest does
# not collect is a family the audit cannot check. Civilization VI spells a spy
# promotion `PROMOTION_SPY_SMEAR_CAMPAIGN` and the bridge emitted
# `PROMOTION_SMEAR_CAMPAIGN` for all seventeen; the refusals ran 259-341 per
# live game and took the ledger's orders-applied rate from 96% to 87%.
#
# When adding a family here, add the matching loop to `civ6_name_audit` in
# `src/bin/civvis_orders.rs`. Harvesting a name nothing checks buys nothing.
TYPE_ATTR = re.compile(
    rb'(?:BuildingType|DistrictType|UnitType|ImprovementType|ProjectType'
    rb'|UnitPromotionType)'
    rb'\s*=\s*"((?:BUILDING|DISTRICT|UNIT|IMPROVEMENT|PROJECT|PROMOTION)_[A-Z0-9_]+)"'
)

MAC_DEFAULT = os.path.expanduser(
    "~/Library/Application Support/Steam/steamapps/common/"
    "Sid Meier's Civilization VI/Civ6.app/Contents/Assets"
)

OUR_OWN_MOD = "CivvisControl"


def harvest(assets: str) -> list[str]:
    found: set[str] = set()
    scanned = 0
    for dirpath, dirs, files in os.walk(assets):
        dirs[:] = [d for d in dirs if d != OUR_OWN_MOD]
        if OUR_OWN_MOD in dirpath:
            continue
        for name in files:
            if not name.lower().endswith((".xml", ".sql")):
                continue
            scanned += 1
            try:
                blob = open(os.path.join(dirpath, name), "rb").read()
            except OSError:
                continue
            for match in TYPE_ATTR.finditer(blob):
                found.add(match.group(1).decode())
    print(f"scanned {scanned} rule files, found {len(found)} shipped type names",
          file=sys.stderr)
    return sorted(found)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--civ6", default=os.environ.get("CIV6_DIR", MAC_DEFAULT),
                        help="Civilization VI Assets directory")
    parser.add_argument("--out", default="data/civ6_type_names.json")
    args = parser.parse_args()

    if not os.path.isdir(args.civ6):
        print(f"Civilization VI assets not found at {args.civ6!r}; "
              f"pass --civ6 or set CIV6_DIR", file=sys.stderr)
        return 2

    names = harvest(args.civ6)
    if len(names) < 500:
        print(f"only {len(names)} names harvested -- that is too few to be the "
              f"shipped ruleset; refusing to overwrite the snapshot",
              file=sys.stderr)
        return 3
    with open(args.out, "w") as handle:
        json.dump(names, handle, indent=0)
        handle.write("\n")
    print(f"wrote {len(names)} names to {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
