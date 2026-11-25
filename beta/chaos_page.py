#!/usr/bin/env python3
"""The Chaos Monkey page's figures: worlds played under a very high frequency
of natural disasters, and what they teach.

    chaos_page.py <source_tree>              print the record as JSON
    chaos_page.py <source_tree> <page.html>  write it into the page

The Chaos Monkey page (`beta/chaos.html`, served at `<lane>/chaos-monkey/`)
stands in the site list between Experiments and CIVVIS (operator 2026-11-25:
"A great thought today is chaos engineering society in simulation to try to
learn any lessons we can, without having to go othrough the event ourselves
to learn. above civvis add "Chaos Monkey". Here we can run civvis sims with a
very high frequency of natural disasteres. what can we learn about avoiding
horrible outcomes?").

Its record is the lane's own `data/chaos_monkey.json`, which
`tools/chaos_monkey.py` writes from the games `civvis chaos-monkey` played:
every seed once at each disaster frequency, by the same agents, so a world
under chaos has its own calm self to be read against. The record holds, per
frequency, the disasters a game, the world as a share of the same world
played calm, wars, cities that changed hands and games won outright, each
figure with its 95% range and whether an exact test settles it, and the
lessons, written from the settled figures alone.

Unlike the pages that count the run ledger at every publish, this record is
a snapshot in the tree: the games are a batch somebody played, not a stream,
and the page, this script and the record are one revision's. A page with no
record keeps its slot `null` and says it has no figures.

Like the site's other pages this runs inside the script that publishes the
whole site: what it cannot read costs the page its figures, said on stderr,
and never the publish.
"""

from __future__ import annotations

import json
import pathlib
import re
import sys

SLOT = re.compile(r'(<script type="application/json" id="chaos-data">)(.*?)(</script>)', re.S)

#: Where the record is kept in a tree.
RECORD = pathlib.Path("data") / "chaos_monkey.json"


def record(source: pathlib.Path) -> dict | None:
    """The page's record, or `None` when the tree has none to draw: no file,
    one that does not parse, or one without two frequencies to compare."""
    path = source / RECORD
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        print(f"chaos_page: {path} is not there", file=sys.stderr)
        return None
    except (OSError, ValueError) as error:
        print(f"chaos_page: {path} cannot be read ({error})", file=sys.stderr)
        return None
    doses = data.get("doses") if isinstance(data, dict) else None
    if not isinstance(doses, list) or len(doses) < 2:
        print(f"chaos_page: {path} has fewer than two frequencies; nothing to compare", file=sys.stderr)
        return None
    return data


def fill(page: str, data: dict | None) -> str | None:
    """The page with its slot filled, or `None` when it has no slot."""
    # `<` is escaped so no name or sentence can close the script element.
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
        print(f"chaos_page: {page} has no chaos slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    if data is None:
        print(f"chaos_page: no figures to write into {page}", file=sys.stderr)
    else:
        said = ", ".join(f"{dose['frequency']}x {dose['games']}" for dose in data["doses"])
        print(
            f"chaos_page: wrote the Chaos Monkey games ({said}; {len(data.get('lessons') or [])} lessons) into {page}",
            file=sys.stderr,
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
