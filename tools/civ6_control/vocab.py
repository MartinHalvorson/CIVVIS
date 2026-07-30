#!/usr/bin/env python3
"""Translate Civilization VI's own type names into CIVVIS's vocabulary.

The mod exports plots as the game's numeric terrain/feature/resource ids. A
CIVVIS `Game` needs names. This builds the mapping from two authorities rather
than from guesswork:

* Civilization VI's built database, `DebugGameplay.sqlite`, which carries the
  real tables with every expansion overlay already resolved
  (see the civvis-civ6-database-on-this-mac note), and
* CIVVIS's own `data/terrains.json`, `features.json` and `resources.json`.

CIVVIS was modelled on Civilization VI, so most names correspond directly once
the `TERRAIN_`/`FEATURE_`/`RESOURCE_` prefix is dropped and the rest is
lower-cased. The interesting part is where they do not:

* Civ 6 encodes elevation *in* the terrain — `TERRAIN_GRASS`,
  `TERRAIN_GRASS_HILLS`, `TERRAIN_GRASS_MOUNTAIN` are three types. CIVVIS has
  nine flat terrains plus a separate `mountain`, so a `_HILLS` type maps to its
  base terrain and sets a hills flag, and a `_MOUNTAIN` type maps to `mountain`.
* `TERRAIN_GRASS` is `grassland` in CIVVIS, not `grass`.

Run it directly to print a coverage report. Anything unmapped is printed rather
than silently dropped: a mirror that quietly loses terrain would have the
simulator planning on ground that does not exist.

    python3 tools/civ6_control/vocab.py
"""
from __future__ import annotations

import json
import pathlib
import sqlite3
import sys

CIV6_DB = pathlib.Path(
    "~/Library/Application Support/Sid Meier's Civilization VI/Cache"
    "/DebugGameplay.sqlite").expanduser()
DATA = pathlib.Path(__file__).resolve().parents[2] / "data"

# Where the two vocabularies disagree on spelling rather than on meaning.
TERRAIN_ALIASES = {"grass": "grassland"}

# Same idea for features, and it turned out to be the whole gap.
#
# ⚠ The first version of this file reported features at 74% and called the
# remainder "natural wonders CIVVIS does not model". That was wrong: CIVVIS has
# every one of them. The shortfall was in this table, not in CIVVIS — Civ 6 names
# a wonder by its type id and CIVVIS by its common name, and the two disagree
# whenever the wonder has more than one name.
#
# Four of these are confirmed by Civ 6's own en_US localization
# (DebugLocalization.sqlite): Cliffs of Dover, Mount Everest, Galapagos Islands,
# Tsingy de Bemaraha. The DLC-pack wonders carry no localized text on this
# install, so the two non-obvious pairings were verified STRUCTURALLY against the
# `Features` table instead, which is the stronger authority anyway:
#
# * FEATURE_WHITEDESERT -> sahara_el_beyda. Both are appeal 2, four tiles, no
#   river, no coast, desert/desert-hills/desert-mountain, culture 1 / gold 4 /
#   science 1. The White Desert *is* the Sahara el Beyda.
# * FEATURE_DEVILSTOWER -> mato_tipila. Both are appeal 2, one tile, impassable,
#   sight-through 2, no river, no coast, on grass/plains/desert/tundra. Civ 6
#   renamed the wonder to its Lakota name and left the type id alone; CIVVIS
#   followed the new name.
FEATURE_ALIASES = {
    # Spelling, not meaning.
    "floodplains_grassland": "grassland_floodplains",
    "floodplains_plains": "plains_floodplains",
    "barrier_reef": "great_barrier_reef",
    # One wonder, two names.
    "chocolatehills": "chocolate_hills",
    "cliffs_dover": "cliffs_of_dover",
    "devilstower": "mato_tipila",
    "everest": "mount_everest",
    "galapagos": "galapagos_islands",
    "ikkil": "ik_kil",
    "lysefjorden": "lysefjord",
    "roraima": "mount_roraima",
    "tsingy": "tsingy_de_bemaraha",
    "whitedesert": "sahara_el_beyda",
}

# Deliberately out of scope, recorded here so the coverage report can distinguish
# "we chose not to map this" from "we failed to map this". A silent drop in a
# state mirror is the dangerous kind: the simulator would plan on ground that
# does not exist.
EXCLUDED = {
    "RESOURCE_LEY_LINE":
        "Secret Societies game mode only. RESOURCECLASS_LEY_LINE with no "
        "yields — a mode marker, not a resource. Absent from a standard game, "
        "and inventing CIVVIS semantics for it would be fabrication.",
}


def civvis_names(filename: str) -> set[str]:
    payload = json.loads((DATA / filename).read_text())
    return set(payload.keys() if isinstance(payload, dict) else payload)


def civ6_rows(table: str, column: str) -> list[str]:
    if not CIV6_DB.is_file():
        raise SystemExit(f"Civilization VI database not found at {CIV6_DB}")
    with sqlite3.connect(f"file:{CIV6_DB}?mode=ro", uri=True) as db:
        return [row[0] for row in db.execute(f"select {column} from {table}")]


def strip(prefix: str, name: str) -> str:
    return name[len(prefix):].lower() if name.startswith(prefix) else name.lower()


def terrain_map() -> tuple[dict[str, tuple[str, bool]], list[str]]:
    """Civ 6 terrain type -> (CIVVIS terrain, is_hills). Plus the unmapped."""
    known = civvis_names("terrains.json")
    mapping: dict[str, tuple[str, bool]] = {}
    missing: list[str] = []
    for name in civ6_rows("Terrains", "TerrainType"):
        base = strip("TERRAIN_", name)
        hills = base.endswith("_hills")
        if hills:
            base = base[: -len("_hills")]
        elif base.endswith("_mountain"):
            base = "mountain"
        base = TERRAIN_ALIASES.get(base, base)
        if base in known:
            mapping[name] = (base, hills)
        else:
            missing.append(name)
    return mapping, missing


def simple_map(table: str, column: str, prefix: str, filename: str,
               aliases: dict[str, str] | None = None
               ) -> tuple[dict[str, str], list[str]]:
    known = civvis_names(filename)
    aliases = aliases or {}
    mapping: dict[str, str] = {}
    missing: list[str] = []
    for name in civ6_rows(table, column):
        if name in EXCLUDED:
            continue
        candidate = strip(prefix, name)
        candidate = aliases.get(candidate, candidate)
        if candidate in known:
            mapping[name] = candidate
        else:
            missing.append(name)
    return mapping, missing


def main() -> int:
    terrains, terrain_missing = terrain_map()
    features, feature_missing = simple_map(
        "Features", "FeatureType", "FEATURE_", "features.json", FEATURE_ALIASES)
    resources, resource_missing = simple_map(
        "Resources", "ResourceType", "RESOURCE_", "resources.json")

    for label, mapped, missing in (
            ("terrains", terrains, terrain_missing),
            ("features", features, feature_missing),
            ("resources", resources, resource_missing)):
        total = len(mapped) + len(missing)
        pct = 100.0 * len(mapped) / total if total else 0.0
        print(f"{label:<10} {len(mapped)}/{total} mapped ({pct:.0f}%)")
        for name in missing:
            print(f"    unmapped: {name}")

    for name, why in sorted(EXCLUDED.items()):
        print(f"excluded   {name}: {why}")

    hills = sorted(k for k, (_, h) in terrains.items() if h)
    print(f"\nhills encoded in terrain: {len(hills)}"
          f" (CIVVIS carries elevation separately)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
