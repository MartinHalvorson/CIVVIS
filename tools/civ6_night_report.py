"""One screen for "what happened while I was asleep".

    python3 tools/civ6_night_report.py                # everything since midnight UTC
    python3 tools/civ6_night_report.py --since 20260731

Pulls the three records that were written independently — the climb ledger, the
watchdog verdicts, and each run's settler trace — and puts them beside each other,
because each one is unreadable alone:

  the ledger      says how far a run got and what code it ran (`code_rev`)
  the watchdogs   say whether the mirror and the army were sane on the way
  the settlers    say WHY the city count is what it is

⚠ THE ROWS ARE NOT A SERIES. The code changes between attempts — that is the point
of an overnight grind — so `code_rev` is printed on every line and a batch must be
compared within a revision, never averaged across them. This project has already
published a "regression" that was two different maps at n=1 each.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path

RUNS = Path.home() / "civvis-civ6-runs"
LEDGER = RUNS / "civvis_ladder.jsonl"
WATCHDOGS = RUNS / "watchdogs.jsonl"
RUN_ROOT = RUNS / "control"


def read_jsonl(path: Path) -> list[dict]:
    if not path.exists():
        return []
    out = []
    for line in path.read_text(errors="replace").splitlines():
        try:
            out.append(json.loads(line))
        except ValueError:
            continue
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--since", default="", help="run-tag prefix filter, e.g. 20260731")
    ap.add_argument("--after", default="",
                    help="only ledger rows finished at or after this UTC stamp, "
                         "e.g. 2026-07-31T04:00:00Z. ⚠ USE THIS, NOT --since, TO "
                         "SEPARATE BATCHES: run tags are UTC so a batch that starts "
                         "before midnight local shares its date prefix with the "
                         "previous night's, and `attempt` restarts at 1 per batch.")
    args = ap.parse_args()

    rows = [r for r in read_jsonl(LEDGER)
            if args.since in str(r.get("tag", ""))
            and str(r.get("utc", "")) >= args.after]
    # The watchdogs re-check a run whenever its stream grows, so the LAST verdict for a
    # run is the one formed on the most complete evidence.
    watch: dict[str, dict] = {}
    for entry in read_jsonl(WATCHDOGS):
        watch[entry.get("run", "")] = entry

    # ⚠ PEAK CITIES, NOT THE LAST TURN'S. The ledger records the final `cities`, and a
    # city lost at turn 230 hides an empire that reached four — which is exactly the
    # number this ladder is trying to move. Both are printed; they differ for a reason
    # worth seeing.
    peaks: dict[str, int] = {}
    for row in rows:
        run = RUN_ROOT / str(row.get("tag"))
        if (run / "events.jsonl").exists():
            best = 0
            for line in (run / "events.jsonl").read_text(errors="replace").splitlines():
                try:
                    event = json.loads(line)
                except ValueError:
                    continue
                if event.get("kind") == "turn":
                    best = max(best, event.get("cities") or 0)
            peaks[str(row.get("tag"))] = best
    print(f"{'#':>3}  {'run':<26} {'code':<14} {'turn':>4} {'score':>5} {'rival':>5} "
          f"{'peak':>4} {'end':>3} {'army':>4} {'met':>3}  reason")
    for row in rows:
        print(f"{row.get('attempt'):>3}  {str(row.get('tag')):<26} "
              f"{str(row.get('code_rev'))[:14]:<14} "
              f"{str(row.get('last_turn')):>4} {str(row.get('last_score')):>5} "
              f"{str(row.get('rival_best')):>5} "
              f"{str(peaks.get(str(row.get('tag')), '?')):>4} "
              f"{str(row.get('cities')):>3} "
              f"{str(row.get('army')):>4} {str(row.get('met')):>3}  "
              f"{str(row.get('reason'))[:44]}")

    print("\nwatchdogs (loud lines only):")
    # ⚠ SCOPED TO THE ROWS ABOVE, not to every run on disk. The watchdog report
    # accumulates across nights, and printing all of it under a batch heading invites
    # exactly the mistake this tool exists to prevent: reading an old run's verdict as
    # evidence about the code that ran tonight.
    batch = {str(row.get("tag")) for row in rows}
    quiet = True
    for tag, entry in sorted(watch.items()):
        if tag not in batch:
            continue
        for line in entry.get("verdicts") or []:
            print(f"  {tag}: {line}")
            quiet = False
    if quiet:
        print("  none — no idle stacks, no frozen units, mirror agrees tile for tile")

    # ★★★★ WHO ACTUALLY CHOSE WHAT THE EMPIRE BUILT. `orders_source: civvis` on every
    # turn says nothing about this: the end-turn production prompt fires after CIVVIS
    # has answered and the built-in ladder used to pick the item itself, which
    # `residual: none` could not show because it was emitted before the prompt. This
    # column is the honest version and belongs in the overnight readout, not in a
    # separate tool nobody runs.
    print("\nwho chose production (CIVVIS direct + via the prompt vs the built-in ladder):")
    for row in rows:
        run = RUN_ROOT / str(row.get("tag"))
        if not (run / "events.jsonl").exists():
            continue
        direct, answered, ladder = 0, 0, 0
        for line in (run / "events.jsonl").read_text(errors="replace").splitlines():
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if event.get("kind") == "orders":
                direct += (event.get("by") or {}).get("produce", 0)
            elif event.get("kind") == "build":
                if event.get("reason") == "civvis":
                    answered += 1
                else:
                    ladder += 1
        total = direct + answered + ladder
        if total:
            print(f"  {row.get('tag')}  CIVVIS {direct}+{answered}  ladder {ladder}  "
                  f"({100 * ladder // total}% of build decisions were the ladder's)")

    print("\nsettler fates (the city count, explained):")
    try:
        import civ6_settler_trace as trace  # type: ignore
    except ImportError:
        import importlib.util
        spec = importlib.util.spec_from_file_location(
            "civ6_settler_trace", Path(__file__).with_name("civ6_settler_trace.py"))
        trace = importlib.util.module_from_spec(spec)  # type: ignore
        spec.loader.exec_module(trace)  # type: ignore
    totals: Counter = Counter()
    for row in rows:
        tag = row.get("tag")
        run = RUN_ROOT / str(tag)
        if not (run / "events.jsonl").exists():
            continue
        report = trace.trace(run)
        fates = Counter(s["fate"] for s in report["settlers"])
        totals.update(fates)
        walked = [s["lived"] for s in report["settlers"] if s["fate"] == "lost"]
        print(f"  {tag}  peak {report['peak_cities']} cities from "
              f"{len(report['settlers'])} settlers  {dict(fates)}"
              + (f"  lost after {walked} turns" if walked else ""))
    if totals:
        total = sum(totals.values())
        print(f"  TOTAL {dict(totals)}  ->  "
              f"{totals.get('founded', 0)}/{total} settlers became cities")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
