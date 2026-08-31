#!/usr/bin/env python3
"""Generate the divergence queue: what live play and the rules audit disagree on.

`docs/fidelity/QUEUE.md` is generated, never edited. Its rows come from four
sources, each tolerated when absent:

- **live refusals** — every request-side refusal reason above
  `--refusal-share` (default 5%) of its order kind over the last `--last` runs
  (a local runs directory or the pulled ledger, `tools/live_ledger.py pull`),
  via `civ6_ladder.orders_by_kind`;
- **live postcondition failures** — every receiving-side failure above that
  same share of its kind's explicitly checkable outcomes, via
  `civ6_ladder.postconditions_by_kind`;
- **the divergence scoreboard** — rows of `docs/fidelity/SCOREBOARD.md`
  (`tools/live_divergence.py`) whose MAE is above their subsystem's threshold;
- **the rules-data audit** — unwaived divergences from a `civ6_fidelity.py
  --json` report at `--fidelity` (default `docs/fidelity/fidelity.json`).

Oldest first, so the row that has waited longest is the first one read. Every
line carries the run or tag it was measured on and the number behind it.

    python tools/fidelity_queue.py --runs ~/civvis-civ6-runs/control --last 5
    python tools/fidelity_queue.py --ledger --write        # regenerate docs/fidelity/QUEUE.md
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_ladder  # noqa: E402
import live_ledger  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
QUEUE_DEFAULT = REPO / "docs" / "fidelity" / "QUEUE.md"
SCOREBOARD_DEFAULT = REPO / "docs" / "fidelity" / "SCOREBOARD.md"
FIDELITY_DEFAULT = REPO / "docs" / "fidelity" / "fidelity.json"

NUMBER = re.compile(r"-?\d+(?:\.\d+)?")


def live_rows(root: Path, last: int, share: float) -> list[dict]:
    """Refusal reasons above `share` of their kind, over the newest `last` runs.

    One row per (kind, reason), dated by the OLDEST run in the window that
    carries it, so a refusal that has persisted sorts ahead of a new one."""
    first_seen: dict[tuple[str, str], str] = {}
    seen: dict[str, int] = {}
    refused: dict[tuple[str, str], int] = {}
    for body in live_ledger.summaries(root, last):
        block = body.get("orders")
        if not isinstance(block, dict) or not block:
            events = live_ledger.events_path(body["_dir"])
            block = civ6_ladder.orders_by_kind(events) if events else None
        if not block:
            continue
        stamp = body.get("finished_utc") or ""
        tag = str(body.get("tag") or body["_dir"].name)
        for kind, row in block.items():
            if kind == "*" or not isinstance(row, dict):
                continue
            seen[kind] = seen.get(kind, 0) + int(row.get("seen") or 0)
            for reason, n in (row.get("refused") or {}).items():
                key = (kind, str(reason))
                refused[key] = refused.get(key, 0) + int(n or 0)
                first_seen.setdefault(key, f"{stamp} {tag}")
    rows = []
    for (kind, reason), n in refused.items():
        total = seen.get(kind, 0)
        if total <= 0 or n / total <= share:
            continue
        when, _, tag = first_seen[(kind, reason)].partition(" ")
        rows.append({"when": when, "source": "live", "ref": tag,
                     "what": f"`{kind}` refused `{reason}`",
                     "number": f"{n}/{total} = {100.0 * n / total:.1f}% of the kind"})
    return rows


def postcondition_rows(root: Path, last: int, share: float) -> list[dict]:
    """Failed receiving-side outcomes above ``share`` of their checkable kind.

    A host call that returns true is only the issuing side of an order.  The
    verifier writes an ``order_failed`` event only after the next state frame
    shows that the requested effect did not land.  Keep these rows separate
    from ``live_rows``: their denominator is explicit checkable verdicts, not
    all requests, and legacy verdicts without an order kind cannot safely
    implicate a named kind.
    """
    first_seen: dict[tuple[str, str], str] = {}
    checked: dict[str, int] = {}
    failed: dict[tuple[str, str], int] = {}
    for body in live_ledger.summaries(root, last):
        events = live_ledger.events_path(body["_dir"])
        block = civ6_ladder.postconditions_by_kind(events) if events else None
        if not block:
            continue
        stamp = body.get("finished_utc") or ""
        tag = str(body.get("tag") or body["_dir"].name)
        for kind, row in block.items():
            if kind in ("*", civ6_ladder.POSTCONDITION_UNATTRIBUTED):
                continue
            if not isinstance(row, dict):
                continue
            checked[kind] = checked.get(kind, 0) + int(row.get("verified") or 0) + int(
                row.get("failed") or 0)
            for reason, n in (row.get("reasons") or {}).items():
                key = (kind, str(reason))
                failed[key] = failed.get(key, 0) + int(n or 0)
                first_seen.setdefault(key, f"{stamp} {tag}")
    rows = []
    for (kind, reason), n in failed.items():
        total = checked.get(kind, 0)
        if total <= 0 or n / total <= share:
            continue
        when, _, tag = first_seen[(kind, reason)].partition(" ")
        rows.append({"when": when, "source": "postcondition", "ref": tag,
                     "what": f"`{kind}` failed postcondition `{reason}`",
                     "number": (f"{n}/{total} = {100.0 * n / total:.1f}% "
                                "of checkable outcomes")})
    return rows


LINK = re.compile(r"\[([^\]]+)\]\([^)]*\)")


def scoreboard_rows(path: Path, threshold: float) -> list[dict]:
    """Rows of the divergence scoreboard whose error is above threshold.

    `docs/fidelity/SCOREBOARD.md` (`tools/live_divergence.py`) has one row per
    run per subsystem with `MAE` and its per-subsystem `Threshold`; a row is
    queued when MAE > Threshold. Read as a pipe table by header name so a
    column added later does not break it: the date is a `processed`/`when`/
    `date` column, the run a `run`/`tag` column (a markdown link is unwrapped),
    and the number the first `mae`/`diverg`/`score`/`delta`/`gap`/`rate`
    column. Without a `threshold` column the flag value is the threshold."""
    if not Path(path).is_file():
        return []
    rows = []
    header: list[str] | None = None
    for line in Path(path).read_text().splitlines():
        if not line.strip().startswith("|"):
            header = None
            continue
        cells = [LINK.sub(r"\1", c.strip()) for c in line.strip().strip("|").split("|")]
        if header is None:
            header = [c.lower() for c in cells]
            continue
        if all(set(c) <= set(":- ") for c in cells) or len(cells) != len(header):
            continue
        row = dict(zip(header, cells))

        def pick(*names: str) -> str | None:
            for name in header:
                if any(n in name for n in names):
                    return row[name]
            return None
        number_cell = pick("mae", "diverg", "score", "delta", "gap", "rate")
        found = NUMBER.search(number_cell or "")
        if not found:
            continue
        limit_cell = pick("threshold")
        limit_found = NUMBER.search(limit_cell or "") if limit_cell is not None else None
        if limit_cell is not None and not limit_found:
            continue
        limit = float(limit_found.group()) if limit_found else threshold
        if float(found.group()) <= limit:
            continue
        what = pick("subsystem", "axis", "what") or cells[0]
        rows.append({"when": pick("processed", "when", "date", "finished") or "",
                     "source": "scoreboard",
                     "ref": pick("run", "tag") or Path(path).name,
                     "what": f"`{what}`",
                     "number": f"{number_cell} > threshold {limit:g}"})
    return rows


def fidelity_rows(path: Path) -> list[dict]:
    """Unwaived divergences from a `civ6_fidelity.py --json` report."""
    if not Path(path).is_file():
        return []
    try:
        results = json.loads(Path(path).read_text())
    except json.JSONDecodeError:
        return []
    if isinstance(results, dict):
        results = results.get("results") or results.get("tables") or []
    rows = []
    for table in results if isinstance(results, list) else []:
        for d in (table.get("divergences") or []) if isinstance(table, dict) else []:
            rows.append({"when": "", "source": "fidelity", "ref": Path(path).name,
                         "what": f"`{d.get('table')}` {d.get('entry')}.{d.get('field')}",
                         "number": f"ours {d.get('ours')!r} theirs {d.get('theirs')!r}"})
    return rows


def render(rows: list[dict], *, window: int, sources: dict[str, str]) -> str:
    ordered = sorted(rows, key=lambda r: (r["when"] == "", r["when"], r["source"], r["what"]))
    lines = ["# Divergence queue", "",
             "Generated by `tools/fidelity_queue.py`; do not edit. Oldest first. "
             f"Live rows are over the last {window} run(s).", ""]
    for name, state in sources.items():
        lines.append(f"- {name}: {state}")
    lines.append("")
    if not ordered:
        lines.append("Nothing queued.")
    for row in ordered:
        when = row["when"] or "undated"
        lines.append(f"- {when} · {row['source']} · {row['ref']} — {row['what']} — {row['number']}")
    return "\n".join(lines) + "\n"


def build(root: Path | None, *, last: int, share: float, scoreboard: Path,
          threshold: float, fidelity: Path) -> str:
    rows: list[dict] = []
    sources = {}
    if root is not None and Path(root).is_dir():
        live = live_rows(root, last, share)
        postconditions = postcondition_rows(root, last, share)
        rows += live
        rows += postconditions
        sources["live refusals"] = (f"{len(live)} above {100 * share:.0f}% of their kind "
                                    f"({Path(root).name}, last {last})")
        sources["live postcondition failures"] = (
            f"{len(postconditions)} above {100 * share:.0f}% of checkable outcomes "
            f"({Path(root).name}, last {last})")
    else:
        sources["live refusals"] = "no runs directory"
        sources["live postcondition failures"] = "no runs directory"
    board = scoreboard_rows(scoreboard, threshold)
    rows += board
    sources["scoreboard"] = (f"{len(board)} above threshold ({scoreboard.name})"
                             if Path(scoreboard).is_file() else "absent")
    audit = fidelity_rows(fidelity)
    rows += audit
    sources["rules audit"] = (f"{len(audit)} unwaived ({fidelity.name})"
                              if Path(fidelity).is_file() else "absent")
    return render(rows, window=last, sources=sources)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", type=Path, default=None, help="a live runs directory")
    ap.add_argument("--ledger", action="store_true",
                    help="read the pulled ledger cache instead")
    ap.add_argument("--cache", type=Path, default=live_ledger.CACHE_DEFAULT)
    ap.add_argument("--last", type=int, default=5)
    ap.add_argument("--refusal-share", type=float, default=0.05)
    ap.add_argument("--scoreboard", type=Path, default=SCOREBOARD_DEFAULT)
    ap.add_argument("--scoreboard-threshold", type=float, default=0.0)
    ap.add_argument("--fidelity", type=Path, default=FIDELITY_DEFAULT)
    ap.add_argument("--write", type=Path, nargs="?", const=QUEUE_DEFAULT, default=None,
                    help="write the queue here (default docs/fidelity/QUEUE.md)")
    args = ap.parse_args(argv)
    root = args.runs if args.runs else (args.cache if args.ledger else None)
    text = build(root, last=args.last, share=args.refusal_share,
                 scoreboard=args.scoreboard, threshold=args.scoreboard_threshold,
                 fidelity=args.fidelity)
    if args.write:
        Path(args.write).parent.mkdir(parents=True, exist_ok=True)
        Path(args.write).write_text(text)
        print(f"wrote {args.write}")
    else:
        print(text, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
