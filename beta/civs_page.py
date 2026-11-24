#!/usr/bin/env python3
"""The Civ Analytics page's figures: how CIVVIS does as each civ, and the
early build orders that win against Deity.

    civs_page.py <source_tree>              print the record as JSON
    civs_page.py <source_tree> <page.html>  write it into the page

The Civ Analytics page (`beta/civs.html`, served at
`<lane>/charts/civ-analytics/`) is the third page under Charts/, after ELO
Rankings (operator 2026-11-23: "add another page: Civ Analytics. First can
be the civ records (pulled from win analytics), then early build order").
Both stood on Win Analytics until then, and both are derived exactly as
they were there:

- **civs**, the winningest civs: `home_rankings.civs`, off `results.jsonl`
  on the ledger branch.
- **build_orders**, the tree of early build orders against Deity: the
  record `research_page.research` builds (`research_page.build_orders`, the
  openings from the run summaries and `data/research/build_orders.json`).
  It is taken from that record rather than rebuilt here, so a change to how
  an opening is read reaches this page with nothing to keep in step.

Like the site's other pages this runs inside the script that publishes the
whole site: what it cannot read costs the page those figures, said on
stderr, and never the publish.
"""

from __future__ import annotations

import json
import pathlib
import re
import sys

import home_rankings as ranks
import research_page

SLOT = re.compile(r'(<script type="application/json" id="civ-data">)(.*?)(</script>)', re.S)


def record(source: pathlib.Path) -> dict | None:
    """The record, or `None` when neither figure can be read."""
    out: dict = {"civs": None, "build_orders": None}
    try:
        out["civs"] = ranks.civs(source)
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"civs_page: no civ records ({error})", file=sys.stderr)
    try:
        out["build_orders"] = (research_page.research(source) or {}).get("build_orders")
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"civs_page: no tree of build orders ({error})", file=sys.stderr)
    if not out["civs"] and not out["build_orders"]:
        return None
    return out


def fill(page: str, data: dict | None) -> str | None:
    """The page with its slot filled, or `None` when it has no slot."""
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c")
    filled, count = SLOT.subn(lambda m: m[1] + text + m[3], page, count=1)
    return filled if count else None


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        print(__doc__, file=sys.stderr)
        return 2
    data = record(pathlib.Path(argv[1]))
    if len(argv) == 2:
        json.dump(data, sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
    page = pathlib.Path(argv[2])
    filled = fill(page.read_text(encoding="utf-8"), data)
    if filled is None:
        print(f"civs_page: {page} has no civ slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    if data is None:
        print(f"civs_page: no figures to write into {page}", file=sys.stderr)
    else:
        said = []
        if data.get("civs"):
            said.append(f"{data['civs'].get('games')} games' civ records")
        if data.get("build_orders"):
            said.append(f"{data['build_orders'].get('games')} build orders")
        print(f"civs_page: wrote {' and '.join(said)} into {page}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
