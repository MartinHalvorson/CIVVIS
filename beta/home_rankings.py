#!/usr/bin/env python3
"""The home page's rankings column, read from the lane's own revision.

    home_rankings.py <source_tree>              print the rankings as JSON
    home_rankings.py <source_tree> <page.html>  write them into the page

The landing page (`beta/landing.html`) carries one empty slot,
`<script type="application/json" id="rankings-data">null</script>`, and
`beta/publish.sh` fills it with what this prints. Two rankings:

- **Player strategies**: the Civ AI Agents by published Elo, from
  `data/strategy_elo.json` (written by `strategy_elo`, see `docs/ELO.md`),
  with their handles and tilts from `strategies::ROSTER`, and Civilization
  VI's difficulty rungs in between as the scale's landmarks.
- **Heuristic genes**: the head of `GENE_HEURISTIC_RANKING.md`, which
  `tools/genes.py write` regenerates and CI holds current.

Nothing here is committed as a copy. The page once kept a hand-synced battle
catalog and it went the way such copies go; this is derived at publish time
from the same files the engine embeds, so it cannot drift from them.

A revision that predates either source (the stable lane is routinely pinned
to one) simply gets that ranking as `null`, and the page shows what it has.
"""

from __future__ import annotations

import json
import pathlib
import re
import sys

#: How many genes the column shows. The full table is one link away.
TOP_GENES = 15

#: Civilization VI's own ladder, lowest first: the order pooled rungs are named in.
CIV6_RUNGS = ["settler", "chieftain", "warlord", "prince", "king", "emperor", "immortal", "deity"]

SLOT = re.compile(
    r'(<script type="application/json" id="rankings-data">)(.*?)(</script>)', re.S
)


def roster(source: pathlib.Path) -> dict[str, dict[str, str]]:
    """`strategies::ROSTER` by id: the handle and tilt a person is shown."""
    path = source / "src" / "strategies.rs"
    if not path.is_file():
        return {}
    rows = re.finditer(
        r'id:\s*"([^"]+)",\s*handle:\s*"([^"]+)",\s*tilt:\s*"([^"]+)"',
        path.read_text(encoding="utf-8"),
    )
    return {m[1]: {"name": m[2], "tilt": m[3]} for m in rows}


def strategies(source: pathlib.Path) -> dict | None:
    path = source / "data" / "strategy_elo.json"
    if not path.is_file():
        return None
    published = json.loads(path.read_text(encoding="utf-8"))
    ratings = published.get("ratings") or {}
    names = roster(source)
    rows = []
    for key, rating in ratings.items():
        if key.startswith("civ6:"):
            continue
        shown = names.get(key, {"name": key, "tilt": ""})
        rows.append(
            {
                "kind": "agent",
                "id": key,
                "name": shown["name"],
                "tilt": shown["tilt"],
                "elo": rating["elo"],
                "sd": rating.get("sd", 0.0),
                "games": rating.get("games", 0),
                "wins": rating.get("wins", 0),
                "bound": rating.get("bound", ""),
            }
        )
    # The rungs as a person is shown them: `ladder` is the monotone pooling of
    # the raw fit, and rungs the tables cannot tell apart share one Elo, so
    # they share one row here.
    pooled: dict[float, list[str]] = {}
    for rung, elo in (published.get("ladder") or {}).items():
        pooled.setdefault(elo, []).append(rung)
    for elo, rungs in pooled.items():
        rungs.sort(key=lambda rung: CIV6_RUNGS.index(rung) if rung in CIV6_RUNGS else len(CIV6_RUNGS))
        bounds = {ratings.get(f"civ6:{rung}", {}).get("bound", "") for rung in rungs}
        rows.append(
            {
                "kind": "rung",
                "name": " · ".join(rung.title() for rung in rungs),
                "elo": elo,
                "bound": bounds.pop() if len(bounds) == 1 else "",
            }
        )
    # Strongest first; at a tie the agent stands above the rung it equals.
    rows.sort(key=lambda row: (-row["elo"], row["kind"] != "agent", row["name"]))
    return {
        "tables": published.get("tables"),
        "players": published.get("players"),
        "turns": published.get("turns"),
        "rows": rows,
    }


def cells(line: str) -> list[str]:
    """One Markdown table row. An escaped pipe (`1 \\| 2`) stays in its cell."""
    parts = line.replace("\\|", "\0").strip().strip("|").split("|")
    return [part.strip().replace("\0", "|") for part in parts]


def number(cell: str) -> float | None:
    match = re.search(r"[-+]?\d+(?:\.\d+)?", cell.replace(",", ""))
    return float(match[0]) if match else None


def genes(source: pathlib.Path) -> dict | None:
    path = source / "GENE_HEURISTIC_RANKING.md"
    if not path.is_file():
        return None
    lines = path.read_text(encoding="utf-8").splitlines()
    head = next((cells(line) for line in lines if line.startswith("| Rank |")), None)
    if head is None:
        return None
    # Columns by name, not position: the table has grown columns before.
    try:
        col = {name: head.index(name) for name in ("Rank", "Gene", "Description", "Default", "P(>0)", "Diff")}
    except ValueError:
        return None
    rows = []
    for line in lines:
        row = cells(line) if line.startswith("|") else []
        if len(row) != len(head) or not row[col["Rank"]].isdigit():
            continue
        rows.append(
            {
                "rank": int(row[col["Rank"]]),
                "tag": row[col["Gene"]].strip("`"),
                "description": row[col["Description"]],
                "on": "on" in row[col["Default"]],
                "p": number(row[col["P(>0)"]]),
                "diff": number(row[col["Diff"]]),
            }
        )
    if not rows:
        return None
    rows.sort(key=lambda row: row["rank"])
    last = next((name for name in head if "Last Batch" in name), "")
    batch = re.search(r"n=([\d,]+) total seats.*?end=(\d\d-\d\d)", last)
    return {
        "total": len(rows),
        "on": sum(row["on"] for row in rows),
        "last_batch_seats": batch[1] if batch else None,
        "last_batch_end": batch[2] if batch else None,
        "rows": rows[:TOP_GENES],
    }


def rankings(source: pathlib.Path) -> dict:
    return {"strategies": strategies(source), "genes": genes(source)}


def fill(page: str, data: dict) -> str | None:
    """The page with its slot filled, or `None` when it has no slot."""
    # `<` is escaped so no gene's sentence can close the script element.
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c")
    filled, count = SLOT.subn(lambda m: m[1] + text + m[3], page, count=1)
    return filled if count else None


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        print(__doc__, file=sys.stderr)
        return 2
    data = rankings(pathlib.Path(argv[1]))
    if len(argv) == 2:
        json.dump(data, sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
    page = pathlib.Path(argv[2])
    filled = fill(page.read_text(encoding="utf-8"), data)
    if filled is None:
        # A landing page from before the column: nothing to fill.
        print(f"home_rankings: {page} has no rankings slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    shown = [name for name, value in data.items() if value]
    print(f"home_rankings: wrote {', '.join(shown) or 'no rankings'} into {page}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
