#!/usr/bin/env python3
"""Actuation, counted at both sides of the host boundary and ratcheted.

`orders_seen`/`orders_applied` say how much of what CIVVIS said the engine
did. This says WHICH kind of order is refused and WHY, and independently
whether the next exported host frame verified the requested effect. It keeps
separate floors under the host-accepted and postcondition-verified rates, each
of which may only rise:

    python tools/live_actuation.py table --runs ~/civvis-civ6-runs/control --last 5
    python tools/live_actuation.py table --ledger --last 5      # the pulled ledger branch
    python tools/live_actuation.py check --floors tools/actuation_floors.json --last 5
    python tools/live_actuation.py floors --write --last 5      # ratchet: floors only rise

Host-acceptance numbers come from each run's `summary["orders"]` (written by
`civ6_play.py` from `civ6_ladder.orders_by_kind`) or, for a run recorded
before that key existed, from its `events.jsonl` directly. Receiving-side
outcomes always come from the run's `events.jsonl`: the summary cannot prove
an effect. `check` is a function first (`check_floors` and
`check_verified_floors`) so it is unit-tested without live data; the floors
file is the ratchet's memory and is edited only by `floors --write`.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_ladder  # noqa: E402
import live_ledger  # noqa: E402

FLOORS_DEFAULT = Path(__file__).resolve().parent / "actuation_floors.json"
#: Below this many orders of a kind over the window, a rate is noise and gets
#: neither a floor nor a failure.
MIN_SEEN_DEFAULT = 50
#: All orders of the run, whatever the kind.
TOTAL = "*"


def run_orders(body: dict) -> dict | None:
    """The run's per-kind block, from the summary or from its events."""
    block = body.get("orders")
    if isinstance(block, dict) and block:
        return block
    events = live_ledger.events_path(body["_dir"])
    return civ6_ladder.orders_by_kind(events) if events else None


def run_postconditions(body: dict) -> dict | None:
    """Receiving-side order outcomes, when this run has its event stream."""
    events = live_ledger.events_path(body["_dir"])
    return civ6_ladder.postconditions_by_kind(events) if events else None


def aggregate(bodies: list[dict]) -> dict:
    """Sum host returns and receiving-side outcomes over the supplied runs.

    Each ordinary row remains ``{seen, applied, refused}`` for compatibility
    with older ledgers and consumers. Runs that carry postcondition evidence
    add ``postconditions: {verified, failed, reasons}`` beside it. The two
    measurements intentionally remain independent: a Lua call can return true
    without changing the host state, and a legacy run can have request counts
    without next-frame verdicts.
    """
    total: dict[str, dict] = {}

    def row(kind: str) -> dict:
        return total.setdefault(kind, {"seen": 0, "applied": 0, "refused": {}})

    for body in bodies:
        block = run_orders(body)
        if block:
            for kind, values in block.items():
                if not isinstance(values, dict):
                    continue
                slot = row(kind)
                slot["seen"] += int(values.get("seen") or 0)
                slot["applied"] += int(values.get("applied") or 0)
                for reason, n in (values.get("refused") or {}).items():
                    slot["refused"][reason] = slot["refused"].get(reason, 0) + int(n or 0)
        outcomes = run_postconditions(body)
        if not outcomes:
            continue
        for kind, values in outcomes.items():
            if not isinstance(values, dict):
                continue
            slot = row(kind)
            postconditions = slot.setdefault(
                "postconditions", {"verified": 0, "failed": 0, "reasons": {}})
            postconditions["verified"] += int(values.get("verified") or 0)
            postconditions["failed"] += int(values.get("failed") or 0)
            for reason, n in (values.get("reasons") or {}).items():
                reasons = postconditions["reasons"]
                reasons[reason] = reasons.get(reason, 0) + int(n or 0)
    return total


def rate(row: dict) -> float | None:
    seen = int(row.get("seen") or 0)
    if seen <= 0:
        return None
    return round(100.0 * int(row.get("applied") or 0) / seen, 1)


def top_reasons(row: dict, limit: int = 3) -> str:
    reasons = sorted((row.get("refused") or {}).items(), key=lambda kv: (-kv[1], kv[0]))
    return ", ".join(f"{reason} {n}" for reason, n in reasons[:limit]) or "-"


def postcondition_rate(row: dict) -> float | None:
    """Verified percentage among this kind's explicitly checkable outcomes."""
    postconditions = row.get("postconditions") or {}
    checked = int(postconditions.get("verified") or 0) + int(postconditions.get("failed") or 0)
    if checked <= 0:
        return None
    return round(100.0 * int(postconditions.get("verified") or 0) / checked, 1)


def postcondition_summary(row: dict) -> str:
    postconditions = row.get("postconditions") or {}
    checked = int(postconditions.get("verified") or 0) + int(postconditions.get("failed") or 0)
    pct = postcondition_rate(row)
    if not checked or pct is None:
        return "-"
    return f"{postconditions.get('verified', 0)}/{checked} ({pct:.1f}%)"


def top_postcondition_failures(row: dict, limit: int = 3) -> str:
    postconditions = row.get("postconditions") or {}
    reasons = sorted((postconditions.get("reasons") or {}).items(),
                     key=lambda kv: (-kv[1], kv[0]))
    return ", ".join(f"{reason} {n}" for reason, n in reasons[:limit]) or "-"


def table(agg: dict) -> str:
    header = ["kind", "seen", "host accepted", "host accepted%", "postcondition",
              "top refusal reasons", "top postcondition failures"]
    rows = []
    ordered = sorted(agg.items(), key=lambda kv: (kv[0] != TOTAL, -int(kv[1]["seen"]), kv[0]))
    for kind, row in ordered:
        pct = rate(row)
        rows.append([kind, str(row["seen"]), str(row["applied"]),
                     f"{pct:.1f}%" if pct is not None else "-", postcondition_summary(row),
                     top_reasons(row), top_postcondition_failures(row)])
    if civ6_ladder.UNATTRIBUTED in agg:
        rows.append(["", "", "", "", "", "(`unattributed`: runs whose mod predates "
                     "per-kind refusal counts; named kinds there read seen == applied)", ""])
    if civ6_ladder.POSTCONDITION_UNATTRIBUTED in agg:
        rows.append(["", "", "", "", "", "", "(`unattributed_postcondition`: a "
                     "legacy verdict omitted its order kind; no named verification floor is inferred)"])
    return live_ledger.table(rows, header)


def load_floor_block(path: Path, name: str) -> dict:
    if not Path(path).is_file():
        return {}
    body = json.loads(Path(path).read_text())
    return {k: float(v) for k, v in (body.get(name) or {}).items()}


def load_floors(path: Path) -> dict:
    """Host-acceptance floors (the legacy ``floors`` field)."""
    return load_floor_block(path, "floors")


def load_verified_floors(path: Path) -> dict:
    """Receiving-side postcondition floors, absent in legacy floor files."""
    return load_floor_block(path, "verified_floors")


def check_floors(agg: dict, floors: dict, *, min_seen: int = MIN_SEEN_DEFAULT
                 ) -> list[str]:
    """Every kind whose applied rate over the window sits under its floor."""
    problems = []
    for kind, floor in sorted(floors.items()):
        row = agg.get(kind)
        if not row or int(row["seen"]) < min_seen:
            continue
        pct = rate(row)
        if pct is not None and pct < floor:
            problems.append(f"{kind}: applied {pct:.1f}% of {row['seen']} "
                            f"< floor {floor:.1f}% ({top_reasons(row)})")
    return problems


def check_verified_floors(agg: dict, floors: dict, *, min_seen: int = MIN_SEEN_DEFAULT
                          ) -> list[str]:
    """Every sufficiently sampled verified-effect rate below its own floor."""
    problems = []
    for kind, floor in sorted(floors.items()):
        row = agg.get(kind)
        postconditions = row.get("postconditions") if row else None
        if not postconditions:
            continue
        checked = int(postconditions.get("verified") or 0) + int(postconditions.get("failed") or 0)
        if checked < min_seen:
            continue
        pct = postcondition_rate(row)
        if pct is not None and pct < floor:
            problems.append(f"{kind}: verified {pct:.1f}% of {checked} checkable "
                            f"< floor {floor:.1f}% ({top_postcondition_failures(row)})")
    return problems


def ratchet(floors: dict, agg: dict, *, min_seen: int = MIN_SEEN_DEFAULT) -> dict:
    """Floors after this window: a floor may rise to the measured rate, never fall.

    Named kinds get a floor only when their seen count is a measurement —
    runs from a mod without `seen_by` put every refusal under `unattributed`
    and would floor every named kind at 100%.
    """
    out = dict(floors)
    per_kind_measured = civ6_ladder.UNATTRIBUTED not in agg
    for kind, row in agg.items():
        if kind == civ6_ladder.UNATTRIBUTED:
            continue
        if kind != TOTAL and not per_kind_measured:
            continue
        if int(row["seen"]) < min_seen:
            continue
        pct = rate(row)
        if pct is None:
            continue
        out[kind] = max(float(out.get(kind, 0.0)), pct)
    return out


def ratchet_verified(floors: dict, agg: dict, *, min_seen: int = MIN_SEEN_DEFAULT) -> dict:
    """Postcondition floors after this window, never inferred from unknown kinds."""
    out = dict(floors)
    per_kind_measured = civ6_ladder.POSTCONDITION_UNATTRIBUTED not in agg
    for kind, row in agg.items():
        if kind == civ6_ladder.POSTCONDITION_UNATTRIBUTED:
            continue
        if kind != TOTAL and not per_kind_measured:
            continue
        postconditions = row.get("postconditions") or {}
        checked = int(postconditions.get("verified") or 0) + int(postconditions.get("failed") or 0)
        if checked < min_seen:
            continue
        pct = postcondition_rate(row)
        if pct is None:
            continue
        out[kind] = max(float(out.get(kind, 0.0)), pct)
    return out


def write_floors(path: Path, floors: dict, verified_floors: dict, *, window: int,
                 runs: list[str]) -> None:
    body = {
        "note": "Host-accepted (`floors`) and next-frame verified-effect "
                "(`verified_floors`) rates per order kind, in percent, over the last "
                "`window` live runs. Written only by `live_actuation.py floors "
                "--write`; a floor may rise and never falls. `*` is every order.",
        "window": window,
        "runs": runs,
        "floors": {k: floors[k] for k in sorted(floors)},
        "verified_floors": {k: verified_floors[k] for k in sorted(verified_floors)},
    }
    Path(path).write_text(json.dumps(body, indent=2) + "\n")


def source_dir(args: argparse.Namespace) -> Path:
    if args.runs:
        return args.runs
    return args.cache


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", type=Path, default=None,
                    help="a live runs directory (default: the pulled ledger cache)")
    ap.add_argument("--ledger", action="store_true",
                    help="read the pulled ledger cache (the default source)")
    ap.add_argument("--cache", type=Path, default=live_ledger.CACHE_DEFAULT)
    ap.add_argument("--last", type=int, default=5)
    ap.add_argument("--min-seen", type=int, default=MIN_SEEN_DEFAULT)
    sub = ap.add_subparsers(dest="command", required=True)
    sub.add_parser("table", help="kind x host acceptance and verified postconditions")
    chk = sub.add_parser("check", help="fail when a host or verified-effect rate is under its floor")
    chk.add_argument("--floors", type=Path, default=FLOORS_DEFAULT)
    flo = sub.add_parser("floors", help="show, or with --write ratchet, the floors")
    flo.add_argument("--floors", type=Path, default=FLOORS_DEFAULT)
    flo.add_argument("--write", action="store_true")
    args = ap.parse_args(argv)

    bodies = live_ledger.summaries(source_dir(args), args.last)
    agg = aggregate(bodies)
    if args.command == "table":
        print(f"{len(bodies)} run(s): {', '.join(str(b.get('tag') or b['_dir'].name) for b in bodies)}")
        print(table(agg))
        return 0
    floors = load_floors(args.floors)
    verified_floors = load_verified_floors(args.floors)
    if args.command == "check":
        problems = check_floors(agg, floors, min_seen=args.min_seen)
        verified_problems = check_verified_floors(agg, verified_floors, min_seen=args.min_seen)
        for problem in problems:
            print(f"ACTUATION: {problem}")
        for problem in verified_problems:
            print(f"POSTCONDITION: {problem}")
        if not problems and not verified_problems:
            print(f"actuation: {len(floors)} host floor(s), {len(verified_floors)} "
                  f"postcondition floor(s) held over {len(bodies)} run(s)")
        return 1 if problems or verified_problems else 0
    new = ratchet(floors, agg, min_seen=args.min_seen)
    new_verified = ratchet_verified(verified_floors, agg, min_seen=args.min_seen)
    for kind in sorted(new):
        mark = "" if new[kind] == floors.get(kind) else "  (raised)" if kind in floors else "  (new)"
        print(f"host {kind:<15} {new[kind]:6.1f}%{mark}")
    for kind in sorted(new_verified):
        mark = ("" if new_verified[kind] == verified_floors.get(kind) else
                "  (raised)" if kind in verified_floors else "  (new)")
        print(f"postcondition {kind:<5} {new_verified[kind]:6.1f}%{mark}")
    if args.write:
        write_floors(args.floors, new, new_verified, window=args.last,
                     runs=[str(b.get("tag") or b["_dir"].name) for b in bodies])
        print(f"wrote {args.floors}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
