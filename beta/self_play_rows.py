#!/usr/bin/env python3
"""The seats of a finished self-play batch, a row at a time.

Two pages of Charts/ are read off what each seat of a self-play batch wrote
down as it played: AI Forecasting (`beta/forecasting_page.py`) and AI
Interpretability (`beta/ai_interpretability_page.py`). Their snapshots are
rebuilt from the same directory the Civ AI Interpretability snapshot is
(`beta/interpretability_page.py --self-play`), and from the same batch: the
newest finished one that plays the table in `SETUP`. This module is the part
the two share, so both read a batch the same way.

A batch directory holds `rows*.jsonl`, one JSON object to a line: `header`
rows that say what table the games were played at and from which build, and a
`game` row per seat of each game. A batch's games can be written to more than
one file, so a seat is given once, by its game's seed and its seat.

The rows are on the machine that plays the games, not in the repository, and
a batch is a few hundred megabytes: nothing here keeps a row after it is
read.
"""

from __future__ import annotations

import json
import pathlib
from collections.abc import Iterator

import interpretability_page as civ

#: The table a batch must play to be read, and how a batch directory is
#: named: the Civ AI Interpretability snapshot's own, so the three snapshots
#: are of one kind of game.
SETUP = civ.SETUP
BATCH = civ.BATCH
TURNS = civ.TURNS


class OtherTable(Exception):
    """A batch whose header is not the table in `SETUP`."""


def seats(directory: pathlib.Path, into: dict | None = None) -> Iterator[dict]:
    """Each seat's row of one batch, once. Raises `OtherTable` at a header
    that is not `SETUP`. `into`, when given, is told the commit the batch's
    build was made from (`commit`) as soon as a header says it."""
    seen = set()
    for rows in sorted(directory.glob("rows*.jsonl")):
        with rows.open(encoding="utf-8") as lines:
            for line in lines:
                try:
                    row = json.loads(line)
                except ValueError:
                    continue
                if not isinstance(row, dict):
                    continue
                if row.get("kind") == "header":
                    if any(row.get(key) != value for key, value in SETUP.items()):
                        raise OtherTable(directory.name)
                    build = row.get("build")
                    if isinstance(build, str):
                        try:
                            build = json.loads(build)
                        except ValueError:
                            build = {}
                    if into is not None and not into.get("commit"):
                        into["commit"] = str((build if isinstance(build, dict) else {}).get("commit") or "")[:7]
                    continue
                seat = (row.get("seed"), row.get("seat"))
                if row.get("kind") != "game" or seat in seen:
                    continue
                seen.add(seat)
                yield row


def finished(batches: pathlib.Path) -> list[pathlib.Path]:
    """The finished batches under a gene-screen state directory's `batches/`,
    the newest first. A batch is finished once its `analysis.json` is
    written."""
    named = (p for p in batches.iterdir() if p.is_dir() and BATCH.fullmatch(p.name))
    return [p for p in sorted(named, reverse=True) if (p / "analysis.json").is_file()]


def began(directory: pathlib.Path) -> str:
    """The day a batch began, off its directory's name."""
    year, month, day = BATCH.fullmatch(directory.name).groups()
    return f"{year}-{month}-{day}"


def quantile(ordered: list[float], share: float) -> float:
    """The value a share of the way through an ordered list, between two
    neighbours where it falls between them."""
    if not ordered:
        raise ValueError("no values")
    at = share * (len(ordered) - 1)
    low = int(at)
    high = min(low + 1, len(ordered) - 1)
    return ordered[low] + (ordered[high] - ordered[low]) * (at - low)
