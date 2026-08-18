#!/usr/bin/env python3
"""Start a new evaluation round in a file of its own.

## The collision this removes

`docs/EVAL.md` is 10,386 lines and every round is appended to the end of it.
Thirty-two commits touched it in the seven days to 2026-08-18 and **every one of
them edited the last few lines** — the diffs land at 10242, 10335, 10362. At a
hundred merged pull requests a day, a document with one write point is a
document every author queues behind, and the conflict has nothing to do with
what any of them measured.

This is the shape `src/main.rs` had before its changelog moved out, when it was
serializing roughly half of all merges.

## What changed, and what deliberately did not

Rounds from 2026-08-18 onward get one file each under `docs/eval/`, so two
agents finishing two evaluations on the same afternoon write two different
files and never meet. There is no shared tail left to append to.

`docs/EVAL.md` keeps all 168 historical rounds exactly where they are. It is
cited by `src/elo.rs`, `docs/AI_GUIDE.md`, `tools/eval_manifest.py` and others,
and rewriting it into 168 files would break those citations to solve a problem
that only affects NEW rounds. The archive is not the thing that collides.

    tools/eval_round.py "the maritime splice screens null-positive"
    tools/eval_round.py --list
"""

from __future__ import annotations

import argparse
import datetime
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ROUNDS = REPO / "docs" / "eval"

TEMPLATE = """# {title}

_{date} · `{commit}`_

## What was asked

<!-- The question this round answers, in one or two sentences. An evaluation
     that cannot state its question measured something, but nobody can say
     what. -->

## How it was measured

<!-- Arms, games, seeds, and the shape they ran at. `docs/EVAL.md` records the
     doctrine this has to satisfy: gate on the deployment shape, one seed is
     never a result, and a composite gate licenses the composite, never its
     parts. -->

## What it measured

<!-- The numbers, with intervals. A point estimate with no interval cannot be
     compared against the next one, which is how a 40-game figure came to be
     read against a 480-game figure on 2026-08-17. -->

## What was decided

<!-- Shipped, withheld, or unresolved — and the reason. A null result is a
     result and belongs here in the same detail as a win. -->
"""


def slug(title: str) -> str:
    text = re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-")
    return text[:60] or "round"


def new_round(title: str, date: str, commit: str) -> Path:
    ROUNDS.mkdir(parents=True, exist_ok=True)
    path = ROUNDS / f"{date}-{slug(title)}.md"
    if path.exists():
        raise SystemExit(f"{path} already exists; pick a different title")
    path.write_text(TEMPLATE.format(title=title, date=date, commit=commit))
    return path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("title", nargs="?", help="what this round is about")
    parser.add_argument("--list", action="store_true", help="list the rounds recorded so far")
    parser.add_argument("--date", help="YYYY-MM-DD (default: today, UTC)")
    parser.add_argument("--commit", default="unknown", help="the revision measured")
    args = parser.parse_args(argv)

    if args.list:
        for path in sorted(ROUNDS.glob("*.md")):
            if path.name == "README.md":
                continue
            print(path.relative_to(REPO))
        return 0

    if not args.title:
        parser.error("a title is required (or pass --list)")
    date = args.date or datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
    path = new_round(args.title, date, args.commit)
    print(f"wrote {path.relative_to(REPO)}")
    print("Fill it in, then commit it. No other file needs editing — that is the "
          "point: two rounds on the same day are two files and never conflict.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
