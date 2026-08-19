#!/usr/bin/env python3
"""What a recorded Civilization VI run did, in the numbers that decide it.

A finished attempt leaves `events.jsonl` — thousands of per-turn `state` records
— plus `why.log`. Reading a game out of that means grepping, and the same four
questions get grepped by hand every time:

* **How many cities at turn 60?** Across the ladder this is the sharpest early
  predictor there is: every recorded Settler win sat at 4-6, and the collapses
  at 1-3.
* **Where did the lead cross over?** These games are routinely won early and
  lost in the middle — one run led by 58 at turn 50 and trailed by 97 at turn
  200 — and the crossover turn is where the answer lives.
* **Did the game end early, and to whose victory?** A rival's Culture or
  Diplomatic win ends the game before the turn-250 score tally, and those are a
  different loss from being out-scored.
* **Did anything the seat asked for actually happen?** Congress ballots are the
  standing example: `wc_vote` reports Favor spent while the host records one
  vote and takes nothing.

    python3 tools/civ6_run_report.py ~/civvis-civ6-runs/control/civvis-...Z
    python3 tools/civ6_run_report.py <run> --json report.json

⚠ It reads and prints. It starts no game, changes no controller, and asks
nothing of the host — so it is safe to run against a game that is still being
played, which is the common case when an operator wants to know how the one on
screen is going.

⚠ Every number here is ONE game. A single run is never a result in this
repository; this is for reading the game you recorded, and for deciding which
runs are worth a closer look — not for concluding anything about a treatment.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path
from typing import Iterable, Sequence

#: Civilization VI's own victory identifiers, as `docs/CIV6_LADDER.md` keeps
#: them: the index the host reported, never a guessed name.
VICTORY_NAMES = {
    0: "SCORE", 1: "DEFAULT", 2: "CONQUEST", 3: "CULTURE",
    4: "RELIGIOUS", 5: "TECHNOLOGY", 6: "DIPLOMATIC",
}

#: The band every recorded Settler win has sat in at turn 60.
WIN_BAND = (4, 6)


class ReportError(RuntimeError):
    """A refusal that names its cause rather than printing an empty table."""


def states(events: Path) -> Iterable[dict]:
    with events.open(errors="ignore") as handle:
        for line in handle:
            if '"kind": "state"' not in line and '"kind":"state"' not in line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            if row.get("kind") == "state" and isinstance(row.get("turn"), int):
                yield row


def best_rival(state: dict) -> int:
    scores = [r.get("score") or 0 for r in (state.get("rivals") or [])]
    return max(scores) if scores else 0


def trajectory(rows: Sequence[dict], every: int) -> list[dict]:
    """Our score against the best rival's, at a fixed stride.

    The stride matters more than the resolution: a crossover is only visible if
    both series are sampled at the same turns, and `rivals` is only present on
    the seat's own export.
    """
    out = []
    for row in rows:
        turn = row["turn"]
        if turn % every:
            continue
        out.append({
            "turn": turn,
            "score": row.get("score") or 0,
            "best_rival": best_rival(row),
            # ⚠ Whether anyone had been MET. `best_rival` is 0 both when every
            # rival is on nothing and when none is visible, and rendering the
            # second as a gap of +117 shows a commanding lead over an empty
            # board — the same false signal this report exists to remove.
            "rival_seen": bool(row.get("rivals")),
            "cities": len(row.get("cities") or []),
            "techs": len(row.get("techs") or []),
            "science": round(row.get("science") or 0),
            "culture": round(row.get("culture") or 0),
        })
    return out


def crossover(rows: Sequence[dict]) -> dict | None:
    """The first turn the lead is lost and never regained.

    Not merely the first turn behind: an early wobble while nobody has met
    anybody is noise, and reporting it as the moment the game turned would send
    the reader to the wrong hundred turns.
    """
    ahead = [(r["turn"], (r.get("score") or 0) - best_rival(r)) for r in rows
             if r.get("rivals")]
    if not ahead:
        return None
    last_ahead = None
    for turn, gap in ahead:
        if gap > 0:
            last_ahead = turn
    if last_ahead is None:
        return {"turn": ahead[0][0], "note": "never led once a rival was visible"}
    after = [(t, g) for t, g in ahead if t > last_ahead]
    if not after:
        return None
    return {"turn": after[0][0], "last_led_turn": last_ahead,
            "gap_at_end": ahead[-1][1]}


def ballots(events: Path) -> dict:
    """Congress ballots the seat asked for, and what the host recorded.

    `wc_ballot_verdict` exists because `wc_vote` reported Favor it never spent;
    the two numbers side by side are the whole point of the row.
    """
    asked_multi = registered = total = 0
    worst = None
    with events.open(errors="ignore") as handle:
        for line in handle:
            if '"wc_ballot_verdict"' not in line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            total += 1
            if (row.get("asked") or 0) > 1:
                asked_multi += 1
                if row.get("registered"):
                    registered += 1
                elif worst is None:
                    worst = row
    return {"verdicts": total, "multi_vote_ballots": asked_multi,
            "multi_vote_registered": registered, "first_unregistered": worst}


def settler_holds(run: Path) -> dict:
    """Turns a settler stood still because every step was rejected.

    ⚠ Reported, not interpreted. A run with nine holds reached six cities by
    turn 60 while one with six holds reached two, so this does not predict the
    opening on its own — it is a pointer to the `why.log` lines, not a verdict.
    """
    why = run / "why.log"
    if not why.exists():
        return {"holds": 0, "sites": []}
    sites: dict[str, int] = {}
    holds = 0
    for line in why.open(errors="ignore"):
        if "HELD short of" not in line:
            continue
        holds += 1
        found = re.search(r"\((\d+, \d+)\)", line)
        if found:
            sites[found.group(1)] = sites.get(found.group(1), 0) + 1
    ranked = sorted(sites.items(), key=lambda kv: -kv[1])[:3]
    return {"holds": holds, "sites": [{"site": s, "holds": n} for s, n in ranked]}


def ending(rows: Sequence[dict], run: Path) -> dict:
    """How the game ended, from the host's own terminal event."""
    result = {"last_turn": rows[-1]["turn"] if rows else 0, "victory": None,
              "won": None, "ours": None}
    events = run / "events.jsonl"
    with events.open(errors="ignore") as handle:
        for line in handle:
            if '"victory"' not in line and '"defeat"' not in line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            kind = row.get("kind")
            if kind == "victory":
                index = row.get("victory")
                result.update(victory=index, won=bool(row.get("won")),
                              victory_name=VICTORY_NAMES.get(index, "?"))
            elif kind == "defeat" and row.get("ours"):
                result.update(defeat=True)
    return result


def report(run: Path, every: int) -> dict:
    events = run / "events.jsonl"
    if not events.exists():
        raise ReportError(f"{events} does not exist; pass a run directory")
    rows = list(states(events))
    if not rows:
        raise ReportError(f"{events} holds no state records yet")
    # ⚠ Only when the run actually REACHED turn 60. A game stopped at turn 20
    # has a last-at-or-below-60 record, and reporting its two cities as the
    # turn-60 count reads as a collapse when the opening has simply not
    # happened yet — the same false signal this report exists to remove.
    at60 = [r for r in rows if r["turn"] <= 60]
    reached_60 = rows[-1]["turn"] >= 60
    cities_at_60 = (len(at60[-1].get("cities") or [])
                    if at60 and reached_60 else None)
    return {
        "run": run.name,
        "turns": rows[-1]["turn"],
        "cities_at_60": cities_at_60,
        "in_win_band": (cities_at_60 is not None
                        and WIN_BAND[0] <= cities_at_60 <= WIN_BAND[1]),
        "trajectory": trajectory(rows, every),
        "crossover": crossover(rows),
        "ending": ending(rows, run),
        "ballots": ballots(events),
        "settler": settler_holds(run),
    }


def render(data: dict) -> str:
    lines = [f"{data['run']} — {data['turns']} turns"]
    band = f"{WIN_BAND[0]}-{WIN_BAND[1]}"
    cities = data["cities_at_60"]
    if cities is None:
        lines.append("  turn 60 not reached yet")
    else:
        verdict = "inside" if data["in_win_band"] else "OUTSIDE"
        lines.append(f"  cities at turn 60: {cities} ({verdict} the {band} win band)")
    end = data["ending"]
    if end.get("victory") is not None:
        who = "OURS" if end.get("won") else "a rival's"
        lines.append(f"  ended turn {end['last_turn']} on {who} "
                     f"{end.get('victory_name', '?')} victory")
    else:
        lines.append(f"  no terminal event yet (turn {end['last_turn']})")
    cross = data["crossover"]
    if cross and cross.get("last_led_turn") is not None:
        lines.append(f"  lead lost at turn {cross['turn']} "
                     f"(last led t{cross['last_led_turn']}, "
                     f"ended {cross['gap_at_end']:+d})")
    elif cross:
        lines.append(f"  {cross['note']}")
    else:
        lines.append("  never lost the lead")
    lines.append("")
    lines.append(f"  {'turn':>5} {'us':>6} {'best':>6} {'gap':>6} "
                 f"{'cities':>7} {'techs':>6} {'sci':>5} {'cul':>5}")
    for row in data["trajectory"]:
        if row.get("rival_seen"):
            best = f"{row['best_rival']:>6}"
            gap = f"{row['score'] - row['best_rival']:>+6}"
        else:
            best, gap = f"{'—':>6}", f"{'—':>6}"
        lines.append(f"  {row['turn']:>5} {row['score']:>6} {best} "
                     f"{gap} {row['cities']:>7} {row['techs']:>6} "
                     f"{row['science']:>5} {row['culture']:>5}")
    ball = data["ballots"]
    if ball["verdicts"]:
        lines.append("")
        lines.append(f"  congress: {ball['multi_vote_registered']}/"
                     f"{ball['multi_vote_ballots']} purchased-vote ballots registered "
                     f"({ball['verdicts']} verdicts)")
        worst = ball["first_unregistered"]
        if worst:
            lines.append(f"    first refused: t{worst.get('turn')} asked "
                         f"{worst.get('asked')} sent {worst.get('votes_sent')} "
                         f"recorded {worst.get('recorded')} "
                         f"favor {worst.get('favor_at_ballot')}")
    settler = data["settler"]
    if settler["holds"]:
        # Parenthesised: a site is "14, 28" and joining bare pairs with a comma
        # renders three sites as six numbers.
        sites = ", ".join(f"({s['site']})×{s['holds']}" for s in settler["sites"])
        lines.append("")
        lines.append(f"  settler held short {settler['holds']} times ({sites})")
        lines.append("    ⚠ a pointer to why.log, not a verdict: holds do not "
                     "predict the opening on their own")
    return "\n".join(lines)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("run", help="a recorded run directory")
    parser.add_argument("--every", type=int, default=25,
                        help="trajectory stride in turns (default 25)")
    parser.add_argument("--json", help="also write the full report here")
    args = parser.parse_args(argv)

    data = report(Path(args.run).expanduser(), args.every)
    print(render(data))
    if args.json:
        Path(args.json).write_text(json.dumps(data, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ReportError as exc:
        print(f"civ6_run_report: {exc}", file=sys.stderr)
        raise SystemExit(2)
