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
* **Did the empire that led in science ever race for the science victory?**
  Over the 237 recorded runs that reached turn 200, this seat **led the field
  in science at the end of 177 of them (75%)** and completed all four launch
  projects in **none**. It ordered a Spaceport in 55, and the pad actually
  finished in 30. The `science race` section names which of the four steps
  stopped — the standing, the pad, the chain, or the decision — because the
  ending line cannot tell a pad that never finished from a race that was
  refused every turn, and those are different defects.

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

#: The four launch projects in the order the engine requires them, under the
#: host's own identifiers. `src/mirror.rs` maps these to the engine's names;
#: the Gathering Storm ruleset the ladder plays calls the third MARS_BASE, not
#: MARS_COLONY, and reading for the engine's spelling finds nothing.
#: The turn by which a run has had a real chance to launch. The chain needs
#: the industrial era for the pad's production and Rocketry for the pad; a game
#: abandoned before this has not failed to race, it has not got there.
RACE_ENDGAME_TURN = 200

SPACE_CHAIN = (
    ("PROJECT_LAUNCH_EARTH_SATELLITE", "earth satellite"),
    ("PROJECT_LAUNCH_MOON_LANDING", "moon landing"),
    ("PROJECT_LAUNCH_MARS_BASE", "mars colony"),
    ("PROJECT_LAUNCH_EXOPLANET_EXPEDITION", "exoplanet expedition"),
)

#: What the seat's own journal says about the race, by the phrase that only
#: that decision writes. The drive is `science-victory-drive`; the two
#: refusals are the horizons that can stop the race — the gene's own and the
#: stock one it replaces.
RACE_MARKS = {
    "drive": "Driving for a science victory",
    "stand_down": "The science drive stands down",
    "drive_refusal": "The science drive cannot land the race",
    "stock_refusal": "cannot finish before the turn limit",
}


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
    with why.open(errors="ignore") as handle:
        for line in handle:
            if "HELD short of" not in line:
                continue
            holds += 1
            found = re.search(r"\((\d+, \d+)\)", line)
            if found:
                sites[found.group(1)] = sites.get(found.group(1), 0) + 1
    ranked = sorted(sites.items(), key=lambda kv: -kv[1])[:3]
    return {"holds": holds, "sites": [{"site": s, "holds": n} for s, n in ranked]}


def rival_techs(state: dict) -> int:
    """The best rival's tech count. The seat exports its own `techs` as the
    LIST of what it knows and a rival's as an integer, so a reader that treats
    them alike reports either 0 rivals or a rival on one tech."""
    counts = []
    for rival in state.get("rivals") or []:
        known = rival.get("techs")
        counts.append(len(known) if isinstance(known, list) else (known or 0))
    return max(counts) if counts else 0


def rival_projects(state: dict) -> int:
    best = 0
    for rival in state.get("rivals") or []:
        best = max(best, len(rival.get("science_projects") or []))
    return best


def space_race(rows: Sequence[dict], run: Path) -> dict:
    """Whether the seat that led in science ever raced for the science victory.

    The operator's standing question about this lane (2026-08-24): *"we have
    regularly led science and not even attempted a science victory."* Four
    things have to be true in order for that lane to convert, and only the
    first was ever in doubt — so this reports all four and where the chain
    stopped:

    1. **the standing** — our science a turn and techs against the best rival's;
    2. **the pad** — a Spaceport ordered, and whether it ever COMPLETED (the
       export marks a district `complete: false` while it is still being
       built, and a pad under construction launches nothing);
    3. **the chain** — the turn each of the four launch projects completed,
       ours against the best rival's count;
    4. **the decision** — what the seat's own journal said: whether
       `science-victory-drive` engaged, and how often either horizon refused
       the race.

    ⚠ Counts only. A run where the pad never finished and one where the race
    was refused every turn look the same in the ending line and are different
    defects; naming which of the four stopped is the whole point.
    """
    if not rows:
        return {}
    last = rows[-1]
    ours_science = last.get("science") or 0
    ours_techs = len(last.get("techs") or [])
    completed: dict[str, int] = {}
    pad_ordered = pad_standing = None
    for row in rows:
        for name in row.get("science_projects") or []:
            completed.setdefault(name, row["turn"])
        for city in row.get("cities") or []:
            for district in city.get("districts") or []:
                # ⚠ The export has carried districts BOTH ways: older runs
                # write a bare type string, newer ones an object with the
                # completion flag. A reader that assumes the object shape dies
                # on the older corpus, which is most of it.
                if isinstance(district, str):
                    kind, complete = district, True
                else:
                    kind, complete = district.get("type"), district.get("complete")
                if kind != "DISTRICT_SPACEPORT":
                    continue
                if pad_ordered is None:
                    pad_ordered = row["turn"]
                if complete and pad_standing is None:
                    pad_standing = row["turn"]
    marks = {key: {"count": 0, "first_turn": None, "last": None}
             for key in RACE_MARKS}
    why = run / "why.log"
    if why.exists():
        with why.open(errors="ignore") as handle:
            for line in handle:
                for key, phrase in RACE_MARKS.items():
                    if phrase not in line:
                        continue
                    seen = marks[key]
                    seen["count"] += 1
                    turn = re.search(r"\[why\] t(\d+)", line)
                    if seen["first_turn"] is None and turn:
                        seen["first_turn"] = int(turn.group(1))
                    seen["last"] = line.strip()
    return {
        "science": round(ours_science, 1),
        "best_rival_science": round(max(
            [(r.get("science") or 0) for r in (last.get("rivals") or [])] or [0]), 1),
        "techs": ours_techs,
        "best_rival_techs": rival_techs(last),
        "pad_ordered_turn": pad_ordered,
        "pad_standing_turn": pad_standing,
        "projects": [{"project": name, "label": label,
                      "turn": completed.get(name)} for name, label in SPACE_CHAIN],
        "projects_done": sum(1 for name, _ in SPACE_CHAIN if name in completed),
        "best_rival_projects": rival_projects(last),
        "journal": marks,
        "journal_read": why.exists(),
    }


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
        "space_race": space_race(rows, run),
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
        # "for good", not "never behind": `crossover` deliberately reports the
        # last loss that stuck, so a seat that trailed at t100 and led again by
        # t150 lands here. Reading this as "never trailed" contradicts the
        # trajectory printed three lines below it, which is exactly the misread
        # it caused on run civvis-20260819T102134Z (-74 at t100, +189 at t225).
        lines.append("  never lost the lead for good (may have trailed; see the trajectory)")
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
    race = data.get("space_race") or {}
    if race:
        lead_sci = race["science"] - race["best_rival_science"]
        lead_tech = race["techs"] - race["best_rival_techs"]
        lines.append("")
        lines.append(f"  science race: {race['science']:.0f}/turn v best rival "
                     f"{race['best_rival_science']:.0f} ({lead_sci:+.0f}), "
                     f"{race['techs']} techs v {race['best_rival_techs']} "
                     f"({lead_tech:+d})")
        if race["pad_standing_turn"] is not None:
            pad = f"stood t{race['pad_standing_turn']}"
            if race["pad_ordered_turn"] != race["pad_standing_turn"]:
                pad += f" (ordered t{race['pad_ordered_turn']})"
        elif race["pad_ordered_turn"] is not None:
            # The distinction the ending line cannot make: a pad ordered and
            # never finished launches exactly as much as no pad at all.
            pad = f"ordered t{race['pad_ordered_turn']}, NEVER COMPLETED"
        else:
            pad = "none"
        lines.append(f"    spaceport: {pad}")
        done = ", ".join(
            f"{p['label']} t{p['turn']}" for p in race["projects"] if p["turn"])
        lines.append(f"    launches: {race['projects_done']}/4"
                     f"{' — ' + done if done else ''}"
                     f" · best rival {race['best_rival_projects']}/4")
        journal = race["journal"]
        if not race["journal_read"]:
            lines.append("    journal: no why.log beside this run")
        else:
            drive, stand = journal["drive"], journal["stand_down"]
            if drive["count"]:
                engaged = f"engaged t{drive['first_turn']}"
                if stand["count"]:
                    engaged += f", stood down {stand['count']}×"
                lines.append(f"    science-victory-drive: {engaged}")
            else:
                # Not the same as "the gene is off": a run recorded before the
                # gene merged writes no such line either. Say which is unknown.
                lines.append("    science-victory-drive: never engaged "
                             "(or the run predates it)")
            refused = (journal["drive_refusal"]["count"]
                       + journal["stock_refusal"]["count"])
            if refused:
                which = ("the drive's own horizon"
                         if journal["drive_refusal"]["count"]
                         else "the stock horizon")
                first = (journal["drive_refusal"]["first_turn"]
                         if journal["drive_refusal"]["count"]
                         else journal["stock_refusal"]["first_turn"])
                lines.append(f"    the race was refused on {refused} turns "
                             f"by {which}, from t{first}")
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


def aggregate(root: Path, every: int) -> dict:
    """The same questions, asked of every recorded run instead of one.

    ★★★ THIS EXISTS BECAUSE ONE GAME ANSWERED THEM WRONG. Reading three runs by
    hand produced "we win the opening and get out-developed from turn 100" —
    and the distribution over sixty-one completed losses says the median
    crossover is turn 77, the modal band is t25-49, and a third of losses never
    led at all. The three-game story came from one atypical run. A per-run
    report invites exactly that mistake; this is the counterweight, and it costs
    one command.

    Runs that never reached a terminal event, or never reached turn 60, are
    counted and excluded rather than silently dropped: a rate whose denominator
    is unstated is the other way to be wrong here.
    """
    runs = sorted(p for p in root.glob("civvis-*") if (p / "events.jsonl").exists())
    if not runs:
        raise ReportError(f"no run directories under {root}")
    by_cities: dict[int, list[bool]] = {}
    crossovers: list[int] = []
    never_led = wins = completed = skipped_unfinished = skipped_short = 0
    ballots_multi = ballots_registered = 0
    # ⚠ Tallied over runs that REACHED the endgame, not over completed runs:
    # the launch chain cannot start before the industrial era, so counting a
    # game abandoned at turn 40 as one that failed to launch would say the
    # lane is more broken than it is.
    race_seen = race_led = race_pad = race_pad_stood = race_launched = 0
    race_launches: dict[int, int] = {}
    race_rival_launched = race_refused_runs = race_drove = 0
    for run in runs:
        try:
            data = report(run, every)
        except ReportError:
            skipped_unfinished += 1
            continue
        ballots_multi += data["ballots"]["multi_vote_ballots"]
        ballots_registered += data["ballots"]["multi_vote_registered"]
        race = data.get("space_race") or {}
        if race and data["turns"] >= RACE_ENDGAME_TURN:
            race_seen += 1
            race_led += race["science"] > race["best_rival_science"]
            race_pad += race["pad_ordered_turn"] is not None
            race_pad_stood += race["pad_standing_turn"] is not None
            race_launched += race["projects_done"] > 0
            race_launches[race["projects_done"]] = (
                race_launches.get(race["projects_done"], 0) + 1)
            race_rival_launched += race["best_rival_projects"] > 0
            race_drove += race["journal"]["drive"]["count"] > 0
            race_refused_runs += (race["journal"]["drive_refusal"]["count"]
                                  + race["journal"]["stock_refusal"]["count"]) > 0
        if data["ending"].get("victory") is None:
            skipped_unfinished += 1
            continue
        completed += 1
        won = bool(data["ending"].get("won"))
        wins += won
        cities = data["cities_at_60"]
        if cities is None:
            skipped_short += 1
        else:
            by_cities.setdefault(cities, []).append(won)
        if won:
            continue
        cross = data["crossover"]
        if cross is None:
            continue
        if cross.get("last_led_turn") is None:
            never_led += 1
        else:
            crossovers.append(cross["last_led_turn"])
    crossovers.sort()
    bands: dict[int, int] = {}
    for turn in crossovers:
        bands[(turn // 25) * 25] = bands.get((turn // 25) * 25, 0) + 1
    return {
        "runs_seen": len(runs),
        "completed": completed,
        "wins": wins,
        "skipped_unfinished": skipped_unfinished,
        "skipped_before_turn_60": skipped_short,
        "by_cities_at_60": {c: {"games": len(v), "wins": sum(v)}
                            for c, v in sorted(by_cities.items())},
        "never_led": never_led,
        "crossovers": crossovers,
        "crossover_median": crossovers[len(crossovers) // 2] if crossovers else None,
        "crossover_bands": bands,
        "space_race": {
            "runs": race_seen, "led": race_led, "pad_ordered": race_pad,
            "pad_stood": race_pad_stood, "launched": race_launched,
            "launches": race_launches, "rival_launched": race_rival_launched,
            "refused": race_refused_runs, "drove": race_drove,
        },
        "multi_vote_ballots": ballots_multi,
        "multi_vote_registered": ballots_registered,
    }


def render_aggregate(data: dict) -> str:
    lines = [f"{data['completed']} completed runs of {data['runs_seen']} "
             f"({data['skipped_unfinished']} without a terminal event), "
             f"{data['wins']} won"]
    band = f"{WIN_BAND[0]}-{WIN_BAND[1]}"
    race = data.get("space_race") or {}
    if race.get("runs"):
        seen = race["runs"]
        lines.append("")
        lines.append(f"  the science race, over the {seen} runs that reached "
                     f"turn {RACE_ENDGAME_TURN}:")
        lines.append(f"    led the field in science at the end: {race['led']}")
        lines.append(f"    ordered a spaceport: {race['pad_ordered']} "
                     f"(it stood in {race['pad_stood']})")
        lines.append(f"    launched at least one project: {race['launched']}"
                     f" — a rival did in {race['rival_launched']}")
        spread = ", ".join(f"{n}/4 in {count}"
                           for n, count in sorted(race["launches"].items()))
        lines.append(f"    launches: {spread}")
        lines.append(f"    the race was refused at least once in "
                     f"{race['refused']}; the drive engaged in {race['drove']}")
    lines.append("")
    lines.append(f"  {'cities@60':>9} {'games':>6} {'wins':>5} {'rate':>6}")
    inside = outside = inside_won = outside_won = 0
    for cities, cell in data["by_cities_at_60"].items():
        rate = cell["wins"] / cell["games"]
        lines.append(f"  {cities:>9} {cell['games']:>6} {cell['wins']:>5} {rate:>5.0%}")
        if WIN_BAND[0] <= cities <= WIN_BAND[1]:
            inside += cell["games"]; inside_won += cell["wins"]
        else:
            outside += cell["games"]; outside_won += cell["wins"]
    if inside or outside:
        lines.append(f"  {'in ' + band:>9} {inside:>6} {inside_won:>5} "
                     f"{(inside_won / inside if inside else 0):>5.0%}")
        lines.append(f"  {'outside':>9} {outside:>6} {outside_won:>5} "
                     f"{(outside_won / outside if outside else 0):>5.0%}")
    if data["skipped_before_turn_60"]:
        lines.append(f"  ({data['skipped_before_turn_60']} completed before turn 60, "
                     f"excluded from the table above)")
    lines.append("")
    lines.append(f"  losses that never led once a rival was visible: {data['never_led']}")
    if data["crossover_median"] is not None:
        lines.append(f"  losses that led then lost it — median turn "
                     f"{data['crossover_median']}, n={len(data['crossovers'])}")
        for start in sorted(data["crossover_bands"]):
            lines.append(f"    t{start:>3}-{start + 24:<4} "
                         f"{data['crossover_bands'][start]:>4}")
    if data["multi_vote_ballots"]:
        lines.append("")
        lines.append(f"  purchased-vote ballots registered: "
                     f"{data['multi_vote_registered']}/{data['multi_vote_ballots']}")
    return "\n".join(lines)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("run", help="a recorded run directory, or the directory "
                                     "holding them with --aggregate")
    parser.add_argument("--aggregate", action="store_true",
                        help="treat the path as the parent of many runs and "
                             "report across all of them")
    parser.add_argument("--every", type=int, default=25,
                        help="trajectory stride in turns (default 25)")
    parser.add_argument("--json", help="also write the full report here")
    args = parser.parse_args(argv)

    target = Path(args.run).expanduser()
    if args.aggregate:
        data = aggregate(target, args.every)
        print(render_aggregate(data))
    else:
        data = report(target, args.every)
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
