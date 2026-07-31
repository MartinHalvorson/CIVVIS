#!/usr/bin/env python3
"""Report effect keys the engine reads that no ruleset row provides.

This is the mirror of ``civvis_inert.py``. That tool finds data with no
consumer; this one finds a consumer with no data. Both describe the same
failure -- a rule that is "implemented" and can never fire -- from opposite
ends of the string join, and neither the compiler nor `civvis validate` can see
it, because the join is made of string literals at runtime.

A read like ``policy_effect(pid, "production_flat")`` silently returns 0.0
forever when no policy carries that key. In engine code that means a clause
that never applies; in a test it means an assertion that cannot fail, which is
worse, because it reads as coverage.

Run from the repository root::

    python3 tools/civvis_dangling.py
"""

from __future__ import annotations

import json
import pathlib
import re
import sys

# Each engine accessor and the ruleset tables a key may legitimately come from.
# A table is read either as ``{name: {"effects": {...}}}`` or, for the tech and
# civic trees, as ``{group: {node: {key: value}}}``.
ACCESSORS: dict[str, tuple[tuple[str, str], ...]] = {
    "policy_effect": (("policies.json", "effects"),),
    "policy_effect_for_unit": (("policies.json", "effects"),),
    "promotion_effect": (("promotions.json", "effects"),),
    "tree_effect": (("tree_effects.json", "tree"),),
    "pantheon_effect": (("beliefs.json", "beliefs"),),
    "religion_belief_effect": (("beliefs.json", "beliefs"),),
    "city_religion_belief_effect": (("beliefs.json", "beliefs"),),
    "city_building_effect": (("buildings.json", "effects"),),
    "empire_wonder_effect": (("wonders.json", "effects"),),
    "city_wonder_effect": (("wonders.json", "effects"),),
    "city_district_effect": (("districts.json", "effects"),),
    "city_active_project_effect": (("projects.json", "effects"),),
    "governor_effect": (("governors.json", "governors"),),
}


def table_keys(path: pathlib.Path, shape: str) -> set[str]:
    table = json.loads(path.read_text())
    rows = table.get(path.stem, table)
    found: set[str] = set()
    if shape == "tree":
        for group in rows.values():
            if isinstance(group, dict):
                for node in group.values():
                    if isinstance(node, dict):
                        found.update(node.keys())
        return found
    if shape == "governors":
        for governor in rows.values():
            if not isinstance(governor, dict):
                continue
            found.update((governor.get("effects") or {}).keys())
            for promotion in (governor.get("promotions") or {}).values():
                if isinstance(promotion, dict):
                    found.update((promotion.get("effects") or {}).keys())
        return found
    if shape == "beliefs":
        for group in rows.values():
            if isinstance(group, dict):
                for belief in group.values():
                    if isinstance(belief, dict):
                        found.update((belief.get("effects") or {}).keys())
        return found
    for row in rows.values():
        if isinstance(row, dict):
            found.update((row.get("effects") or {}).keys())
    return found


def agenda_measures_without_a_scorer(root: pathlib.Path) -> list[tuple[str, str]]:
    """Agenda measures the engine never reads.

    Every agenda names a ``measure``; the scorer dispatches on it as a string
    literal. A measure with no matching branch falls through to 0.0 and the
    agenda silently never fires — which is how Sweden's ``great_works`` and,
    before it, several others were dead on arrival.
    """
    import json

    source = "\n".join(path.read_text() for path in sorted(root.glob("src/**/*.rs")))
    agendas = json.loads((root / "data" / "agendas.json").read_text())
    rows = agendas.get("agendas", agendas)
    missing = []
    for name, spec in rows.items():
        measure = spec.get("measure")
        if measure and f'"{measure}"' not in source:
            missing.append((name, measure))
    return missing


def main() -> int:
    root = pathlib.Path(__file__).resolve().parent.parent
    source = "\n".join(path.read_text() for path in sorted(root.glob("src/**/*.rs")))

    dangling: list[tuple[str, str]] = []
    for accessor, tables in ACCESSORS.items():
        provided: set[str] = set()
        for name, shape in tables:
            provided |= table_keys(root / "data" / name, shape)
        read = set(re.findall(accessor + r'\([^)]*?"([a-z_0-9]+)"', source))
        for key in sorted(read - provided):
            dangling.append((accessor, key))

    print(f"{len(dangling)} effect reads no ruleset row can answer:")
    for accessor, key in dangling:
        print(f"  {accessor}(.., {key!r})")

    agenda_gaps = agenda_measures_without_a_scorer(root)
    print(f"{len(agenda_gaps)} agenda measures with no scorer branch:")
    for name, measure in agenda_gaps:
        print(f"  agenda {name!r} -> measure {measure!r}")

    return 1 if dangling or agenda_gaps else 0


if __name__ == "__main__":
    sys.exit(main())
