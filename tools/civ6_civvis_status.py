"""Report whether CIVVIS is actually driving a Civilization VI run, and how it is doing.

    python3 tools/civ6_civvis_status.py                 # newest run
    python3 tools/civ6_civvis_status.py --run <tag>

⚠ THE POINT IS THE DENOMINATOR. "CIVVIS is deciding" reads identical in a log
whether every order landed or every one was refused, so this prints the fractions
that can distinguish them:

  source        civvis / civvis_stale / fallback per turn. A run that is mostly
                `fallback` is a measurement of the built-in heuristics, not CIVVIS.
  applied       orders the engine accepted, over orders CIVVIS issued. `pcall`
                succeeding is not acceptance; the mod asks `CanStartOperation` first.
  residual      built-in passes that still ran on a turn credited to CIVVIS. Any
                non-zero entry is a decision CIVVIS did not make.
  refusals      why orders were rejected, by reason. One dominant reason is a bug,
                not noise — `no_params` on every produce order was a wrong lookup.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path

RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"


#: What each residual bucket means, in the order a reader should take them.
#: The agent writes them as `!<bucket>` totals and `<prompt>!<bucket>` rows.
RESIDUAL_BUCKETS = (
    ("unasked", "LEAKED    ", "a prompt CIVVIS owns, decided by a heuristic first"),
    ("after_civvis", "escape    ", "CIVVIS answered, prompt stood, bounded ladder retry"),
    ("declined", "declined  ", "the ladder had no answer; nothing decided"),
)


def print_residual(residual: Counter) -> None:
    """Report the residual census as three numbers, never as one.

    ⚠⚠⚠ ONE FLAT TOTAL READS AS THE LEAK, AND IT IS MOSTLY NOT THE LEAK.
    On 2026-08-17 a review of fourteen runs read this line's 1,577 as "1,577
    decisions taken by the Lua fallback instead of CIVVIS" and had to withdraw
    it: 937 were the bounded escape *after* CIVVIS had already answered and the
    prompt came back anyway — a mechanism whose absence cost several 900-second
    wedges — about 350 were declines where nothing decided anything at all, and
    the genuine leak was **three**. The reader had the source open.

    A number that makes a careful reader reach the wrong conclusion is a broken
    instrument. `unasked` is the one that means what the whole counter was built
    to mean, so it is printed first, in capitals, with its own per-prompt
    breakdown; the other two are context.

    Older runs carry no `!bucket` keys at all. They are reported as
    unclassified rather than folded into any bucket — a pre-#1839 total cannot
    be split after the fact, and guessing which way it went is exactly the
    error this function exists to prevent.
    """
    if not residual:
        print("  residual (built-ins on a CIVVIS turn): none")
        return
    buckets = {name: residual.get(f"!{name}", 0) for name, _, _ in RESIDUAL_BUCKETS}
    classified = sum(buckets.values())
    # Plain `<prompt>` keys are the flat per-name totals the agent has always
    # written; `@source` and `!bucket` are the two breakdowns beside them.
    flat = sum(v for k, v in residual.items() if "@" not in k and "!" not in k)
    print("  residual (built-ins on a CIVVIS turn):")
    if not classified:
        print(f"      unclassified: {flat}  (run predates the bucket census; "
              "the total cannot be split after the fact)")
        print(f"      by prompt: {dict(Counter({k: v for k, v in residual.items() if '@' not in k and '!' not in k}).most_common(8))}")
        return
    for name, label, meaning in RESIDUAL_BUCKETS:
        print(f"      {label} {buckets[name]:5d}  {meaning}")
    if flat > classified:
        print(f"      unclassified {flat - classified:5d}  (turns before the "
              "bucket census)")
    leaks = Counter({
        key.split("!", 1)[0]: value
        for key, value in residual.items()
        if key.endswith("!unasked") and not key.startswith("!")
    })
    if leaks:
        print(f"      leaked prompts: {dict(leaks.most_common(8))}")
        print("      ^ each is a decision CIVVIS issues orders for, answered by "
              "the hand-written ladder first. Add the prompt to "
              "CIVVIS_OWNED_BLOCKERS.")


def newest_run() -> Path | None:
    runs = [p for p in RUN_ROOT.iterdir() if (p / "events.jsonl").exists()]
    if not runs:
        return None
    return max(runs, key=lambda p: (p / "events.jsonl").stat().st_mtime)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", default=None)
    ap.add_argument("--tail", type=int, default=6, help="turns to print in detail")
    args = ap.parse_args()

    run = (RUN_ROOT / args.run) if args.run else newest_run()
    if run is None or not (run / "events.jsonl").exists():
        print("no run with events found")
        return 2

    turns, orders, wars, stale, errors = [], [], [], [], []
    for line in (run / "events.jsonl").read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        kind = event.get("kind")
        if kind == "turn":
            turns.append(event)
        elif kind == "orders":
            orders.append(event)
        elif kind == "war":
            wars.append(event)
        elif kind == "orders_stale":
            stale.append(event)
        elif kind == "error":
            errors.append(event)

    print(f"run {run.name}")
    if not turns:
        print("  no turn records yet")
        return 0

    last = turns[-1]
    print(f"  turns {turns[0].get('turn')}..{last.get('turn')}  "
          f"score={last.get('score')} rival_best={last.get('rival_best')} "
          f"lead={last.get('lead')}")
    print(f"  cities={last.get('cities')} units={last.get('units')} "
          f"army={last.get('army')} met={last.get('met')} gold={last.get('gold')}")

    sources = Counter(t.get("orders_source") for t in turns)
    total = sum(sources.values()) or 1
    parts = [f"{k}={v} ({100 * v // total}%)" for k, v in sources.most_common()]
    print(f"  source: {'  '.join(parts)}")

    seen = sum(o.get("seen", 0) for o in orders)
    applied = sum(o.get("applied", 0) for o in orders)
    refused = sum(o.get("refused", 0) for o in orders)
    pct = (100 * applied // seen) if seen else 0
    explored = sum(o.get("explored", 0) or 0 for o in orders)
    # ★★★★★ ACCEPTED IS NOT DONE. `missed` counts orders Civilization VI took and
    # then did not carry out — the unit ended FARTHER from where it was sent. It is
    # kept out of `refused` because a refusal is the bridge working; this is the
    # bridge reporting success for something that did not happen, which is the failure
    # mode that has cost this project the most days.
    missed = sum(o.get("missed", 0) or 0 for o in orders)
    print(f"  orders: applied {applied}/{seen} ({pct}%)  refused {refused}  "
          f"MISSED (accepted, unit went the other way) {missed}")
    # Kept apart from `applied` on purpose: these are units CIVVIS said nothing about,
    # handed to Civ 6's own explore automation. Reporting them inside `applied` would
    # credit CIVVIS with orders it never gave.
    print(f"  explored by game automation (not CIVVIS's orders): {explored}")

    by = Counter()
    for order in orders:
        for key, value in (order.get("by") or {}).items():
            by[key] += value
    if by:
        print(f"  by kind: {dict(by.most_common())}")

    refusals = Counter()
    for order in orders:
        reasons = order.get("refusals")
        if isinstance(reasons, dict):
            for key, value in reasons.items():
                refusals[key] += value
    if refusals:
        print(f"  refusals: {dict(refusals.most_common(6))}")

    # ⚠ A LUA TABLE WITH NO KEYS ENCODES AS `[]`, NOT `{}`, and this used to count
    # only dicts — so an EMPTY residual read as "none" correctly and a residual the mod
    # happened to emit as a list would have read as "none" too. The counter that is
    # supposed to prove the heuristics are quiet must not have a shape it silently
    # ignores; this one has already been wrong in the reassuring direction once.
    residual = Counter()
    # ★★★★★ THE `residual` EVENT, WHICH IS THE ONLY HONEST SOURCE. The field on the
    # turn record is copied when CIVVIS's orders arrive, which is BEFORE most end-turn
    # blockers fire — so it was almost always empty and the entries that came after it
    # were wiped by the next turn's reset. Every run reported `residual: none` while
    # `driveProduction` was choosing what the empire built.
    for line in (run / "events.jsonl").read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if event.get("kind") == "residual":
            for key, value in (event.get("counts") or {}).items():
                residual[key] += value
    for turn in turns:
        entries = turn.get("residual")
        if isinstance(entries, dict):
            for key, value in entries.items():
                residual[key] += value
        elif isinstance(entries, list):
            for entry in entries:
                residual[str(entry)] += 1
    print_residual(residual)
    # ⚠ AND THE ONE THAT IS NOT A PASS. `ENDTURN_BLOCKING_PRODUCTION` routes into
    # `driveProduction`, which picks the item ITSELF from the hand-written ladder — so
    # unlike `units`, this blocker's answer is a DECISION, and it competes with
    # CIVVIS's own produce orders. Printed as a fraction because that is the only form
    # that distinguishes "the heuristic filled a gap" from "the heuristic ran the
    # economy": run civvis-20260731T075743Z was 11 CIVVIS produce orders against 33
    # heuristic builds, with ten battering rams among them.
    heuristic_builds = 0
    civvis_answered = 0
    for line in (run / "events.jsonl").read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if event.get("kind") == "build":
            # ⚠ A build whose reason is `civvis` was the prompt being answered with
            # CIVVIS's own choice for that city, not the ladder inventing one. Counting
            # it as the ladder's would understate CIVVIS exactly as badly as the old
            # `residual: none` overstated it.
            if event.get("reason") == "civvis":
                civvis_answered += 1
            else:
                heuristic_builds += 1
    civvis_produce = sum((o.get("by") or {}).get("produce", 0) for o in orders)
    total_builds = heuristic_builds + civvis_produce + civvis_answered
    if total_builds:
        print(f"  ⚠ PRODUCTION: CIVVIS chose {civvis_produce} directly and "
              f"{civvis_answered} through the blocker, the built-in ladder chose "
              f"{heuristic_builds} "
              f"({100 * heuristic_builds // total_builds}% of build decisions)")

    # ★★★★★ CROSS-CHECK THE COUNTER AGAINST THE EVENT LOG, because the counter has
    # already been wrong once and a zero read as proof.
    #
    # `residual` only ever incremented when `awaiting.source == "civvis"`, but
    # blockers are answered from the game-core event loop BEFORE CIVVIS's reply
    # arrives — so it read NONE for the whole project while the mod's own heuristics
    # were picking policy cards and pantheons. A `blocked` event carrying an
    # `answered` value IS a decision something other than CIVVIS made, whatever the
    # counter says, so it is counted here independently.
    # Two classes, kept apart on purpose. Merging them overstates the problem, and
    # this counter has already been wrong in the reassuring direction once.
    #
    #   named   the answer IS the choice — `TECH_MATHEMATICS`, `CIVIC_FEUDALISM`,
    #           `BELIEF_DANCE_OF_THE_AURORA`. Unambiguously a hand-written decision.
    #   passes  the answer is a pass name — `units`, `production`. The pass may well
    #           be applying CIVVIS's own order, so this is NOT evidence of a
    #           heuristic choice and must not be reported as one.
    named, passes = Counter(), Counter()
    for line in (run / "events.jsonl").read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if event.get("kind") != "blocked":
            continue
        answer = str(event.get("answered") or "")
        if not answer or answer in ("False", "None"):
            continue
        blocker = str(event.get("blocker"))
        if answer in ("units", "production"):
            passes[blocker] += 1
        else:
            named[f"{blocker}={answer[:28]}"] += 1
    if named:
        # ⚠ THIS NUMBER LOOKS WORSE THAN IT IS, AND THE FIRST READING OF IT WAS WRONG.
        # Measured on run civvis-20260731T040858Z: 25 research answers and 22 civic
        # answers, and EVERY ONE of them lands within a turn of a tech or civic
        # completing (25/25, 22/22). Civilization VI asks for the next item the instant
        # one finishes, and CIVVIS has already spoken for that turn — so the heuristic
        # is filling the completion frame, not choosing the plan.
        #
        # What actually gets researched is CIVVIS's: of the 26 techs that completed in
        # that run, only 2 were the tech the harness had last asked for. CIVVIS's own
        # research order lands every turn and replaces the stopgap. The real cost is
        # about one turn of the wrong research per completion, plus progress spread
        # across two techs — worth closing, not worth calling the plan heuristic.
        print(f"  harness answers on a CIVVIS turn: {sum(named.values())} "
              f"(expect ~1 per tech/civic COMPLETION — the game asks for the next "
              f"item mid-turn, after CIVVIS has spoken)")
        for label, count in named.most_common(6):
            print(f"      {count}x {label}")
    else:
        print("  decisions made by the harness: none")
    if passes:
        print(f"  (order/production passes re-run for a blocker, may be CIVVIS's own: "
              f"{dict(passes.most_common(4))})")

    if wars:
        print(f"  WARS DECLARED: {[(w.get('turn'), w.get('target')) for w in wars]}")
    else:
        print("  wars declared: none")
    if stale:
        print(f"  stale answers used: {len(stale)} "
              f"(worst {max(s.get('behind', 0) for s in stale)} turns behind)")
    if errors:
        kinds = Counter(str(e.get("error", ""))[:60] for e in errors)
        print(f"  ERRORS: {len(errors)}")
        for text, count in kinds.most_common(3):
            print(f"    {count}x {text}")

    # ★ CAPTURE IS THE THING THIS PROJECT HAS NEVER PROVEN. Everything up to the wall
    # has worked before — rams built, walls hit, 118 melee strikes — and no city has
    # ever changed hands, so "did a city change owner" gets its own detector rather
    # than being inferred from a rising city count. A city count can rise because a
    # settler founded one, which is a completely different event.
    states = []
    for line in (run / "events.jsonl").read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if event.get("kind") == "state":
            states.append(event)
    captures = []
    prev_ours = None
    prev_theirs = None
    for state in states:
        ours = {(c["x"], c["y"]) for c in state.get("cities", [])}
        theirs = set()
        for rival in state.get("rivals", []) or []:
            for city in rival.get("cities", []) or []:
                theirs.add((city["x"], city["y"]))
        if prev_ours is not None:
            # A plot that was THEIRS last turn and is OURS now changed hands. A
            # newly founded city was on nobody's list, so this cannot confuse them.
            gained = (ours - prev_ours) & prev_theirs
            for plot in gained:
                captures.append((state.get("turn"), plot))
        prev_ours, prev_theirs = ours, theirs
    if captures:
        print(f"  *** CITIES CAPTURED: {captures} ***")
    else:
        print("  cities captured: none")
    if states:
        seen = [(s.get("turn"), len(s.get("cities", [])),
                 sum(len(r.get("cities", []) or []) for r in (s.get("rivals") or [])))
                for s in states[-1:]]
        print(f"  last state: turn/our-cities/rival-cities-seen {seen}")

    print("  recent turns:")
    for turn in turns[-args.tail:]:
        print(f"    t{turn.get('turn'):>3} score={turn.get('score')} "
              f"cities={turn.get('cities')} army={turn.get('army')} "
              f"src={turn.get('orders_source')} "
              f"applied={turn.get('orders_applied')}/{turn.get('orders_seen')} "
              f"polls={turn.get('orders_polls')}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
