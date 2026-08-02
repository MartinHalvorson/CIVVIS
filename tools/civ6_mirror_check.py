#!/usr/bin/env python3
"""Does the CIVVIS board actually agree with the Civilization VI export?

One command. Every check here exists because the corresponding field was once
populated, plausible and WRONG -- so nothing below asks "is it non-empty", it
asks "does it AGREE".

    python3 tools/civ6_mirror_check.py [run-dir]        # newest run if omitted

Requires the mirror server on :8610 (see civvis-civ6-mirror/follow.py).

## What each line is guarding, and what it caught

- FOG     every exported plot must be ON the board. Remembered ground used to be
          dropped outright rather than dimmed: 2 of 6 charted plots survived.
- RIVERS  a Civilization VI river plot is fresh water BY DEFINITION, so CIVVIS's
          river tiles must be `fw` in the export far above the base rate. Read
          36.4% against a 25.7% base -- chance -- while the board showed the
          GENERATED map's rivers.
- LAND    every land plot the export names a continent for must carry one. Read
          200 of 776, with 336 WATER tiles carrying one they should not have.
- CLIFFS  Civilization VI exposes no cliff accessor, so any cliff on the board
          was invented by the map generator. Must be zero.
- CITIES  compare the SETS and name what is missing, never the counts.
- UNITS   likewise. "21 exported, 15 reconstructed" once read as healthy because
          nothing compared them.
- HOSTILES every exported hostile must be somewhere on the board. ONE DIRECTION
          only: the board's non-seat units include rivals and city-states, so the
          two counts are not comparable and are deliberately not printed as a pair.
- TREASURY gold and faith are BALANCES (`GetGoldBalance`, `GetFaithBalance`), not
          the per-turn rates `economy_drift` compares. Same turn, or the delta is
          just income.

## Five ways this checker itself cried wolf

Each of these is a real bug I nearly reported, caught only by looking again:

1. It compared the LATEST export against a board published up to 30s earlier and
   called the game's own progress "13 exported plots dropped". The export is now
   bounded to the board's turn -- see `load_export(upto=)`.
2. It judged rivers on an absolute share and called a 4.3x lift over the base
   rate "no better than chance". Judge the LIFT.
3. It asserted water carries no continent. Civilization VI really does put COAST
   tiles on a continent -- 17 read CONTINENT_AUSTRALIA on one run -- so the check
   is agreement with the export, not a rule of its own.
4. TREASURY, read against the NEWEST export rather than the board's own turn,
   showed `gold 176 vs 167` and `faith 23 vs 21` -- a confident 5% shortfall that
   was one turn of income at +9 gold and +2 faith. Bounded, the same instant read
   134/134 and 27/27. That is defect 1 again, in a new check, which is why the
   note below its `latest_state` says to bound every future reader.
5. HOSTILES, on its very first run, printed `export 0  board 1` as though those
   were a matched pair. They are not: the board's non-seat units include rivals
   and city-states, the export's `hostiles` is only the threat list. The count
   comparison was removed the same minute it was written -- a line that invites a
   false reading is worse than no line.

⚠ The board served on :8610 is follow.py's FLIPPED staged copy:
`board_axial = offset_to_axial(x, TOP - y)`. The flip constant is discovered here
rather than assumed, because comparing two coordinate frames without first
proving they overlap has already produced one confident, wrong finding.
"""

import glob
import json
import os
import sys
import urllib.request
from pathlib import Path

# The same root `civ6_civvis_climb.py` writes to, resolved from $HOME rather than
# hardcoded so this works on any machine that runs the ladder.
RUNS = str(Path.home() / "civvis-civ6-runs" / "control")
PORT = int(os.environ.get("CIVVIS_MIRROR_PORT", "8610"))


def newest_run():
    dirs = [d for d in glob.glob(os.path.join(RUNS, "*")) if os.path.isdir(d)]
    live = [d for d in dirs if os.path.exists(os.path.join(d, "events.jsonl"))]
    return max(live, key=lambda d: os.path.getmtime(os.path.join(d, "events.jsonl")))


def axial(x, y):
    return (x - ((y - (y & 1)) // 2), y)


def load_export(run, upto=None):
    """Latest value per plot, exactly like Snapshot::from_chunks (later wins).

    ⚠ `upto` bounds the export to what had been sent BY THAT TURN. The mirror
    republishes on a cadence, so comparing a board from turn N against an export
    that has since reached turn N+7 reports the growth as loss -- this checker
    cried wolf exactly that way ("13 exported plots are NOT on the board", all of
    them present one publish later). The board's own turn is the cutoff.
    """
    plots, turn = {}, 0
    with open(os.path.join(run, "events.jsonl")) as handle:
        for line in handle:
            try:
                event = json.loads(line)
            except ValueError:
                continue
            kind = event.get("kind") or event.get("event")
            if kind == "turn":
                turn = max(turn, int(event.get("turn") or 0))
            if kind != "tiles":
                continue
            if upto is not None and (event.get("turn") or 0) > upto:
                continue
            for plot in event.get("plots") or []:
                plots[(plot["x"], plot["y"])] = plot
    return plots, turn


def main():
    run = sys.argv[1] if len(sys.argv) > 1 else newest_run()
    board = json.load(urllib.request.urlopen(f"http://127.0.0.1:{PORT}/state", timeout=30))
    # ⚠ Board first, then bound the export to the board's turn. The other order
    # measures the game's progress against a stale snapshot and calls it a defect.
    _, game_turn = load_export(run)
    plots, _ = load_export(run, upto=board["turn"])
    tiles = {tuple(t["pos"]): t for t in board["map"]["tiles"]}
    visible = {tuple(v) for v in board["visible"]}

    problems: list[str] = []
    print(f"run   {os.path.basename(run)}")
    print(f"turn  game {game_turn}   board {board['turn']}"
          f"   {'OK' if abs(game_turn - board['turn']) <= 1 else '⚠ DRIFT'}")

    # --- the flip constant, discovered not assumed -------------------------
    best, best_hits = None, -1
    for top in range(40, 50):
        hits = len({axial(x, top - y) for (x, y) in plots} & set(tiles))
        if hits > best_hits:
            best, best_hits = top, hits
    if best_hits < 0.9 * len(tiles):
        print(f"⚠ coordinate frames do not line up (best {best_hits}/{len(tiles)} "
              f"at TOP={best}); every comparison below would be meaningless")
        return 1
    print(f"frame TOP={best}  overlap {best_hits}/{len(tiles)}")
    pairs = [(tiles[axial(x, best - y)], p)
             for (x, y), p in plots.items() if axial(x, best - y) in tiles]

    print()
    # --- fog memory (#713) -------------------------------------------------
    # ⚠ The invariant is "the board carries every plot the mod exported", NOT
    # "some ground is fogged". Early on, a seat with two units and no cities can
    # SEE everything it has revealed, and a fogged count of zero is correct there.
    # Checking for fog directly cried wolf on turn 1 of a healthy run.
    fogged = len(tiles) - len(visible)
    missing = len(plots) - best_hits
    print(f"FOG      board {len(tiles)} tiles, {len(visible)} visible, "
          f"{fogged} remembered-but-fogged; export has {len(plots)}")
    if missing > 0:
        problems.append("fog")
        print(f"         ⚠ {missing} exported plots are NOT on the board — "
              f"remembered ground is being dropped")
    elif fogged == 0 and len(tiles) > 60:
        print("         ⚠ nothing is fogged on a board this large; suspect a collapse "
              "onto current visibility")
    else:
        print("         OK")

    # --- rivers (#714): a Civ 6 river plot is fresh water BY DEFINITION ----
    riv = [(b, p) for b, p in pairs if b.get("river")]
    fresh = [1 for _, p in pairs if p.get("fw")]
    base = len(fresh) / max(1, len(pairs))
    hit = sum(1 for _, p in riv if p.get("fw"))
    rate = hit / max(1, len(riv))
    exported_rv = sum(1 for _, p in pairs if p.get("rv"))
    print(f"RIVERS   {len(riv)} river tiles; {hit} of them fresh in the export "
          f"({rate:.1%}) vs {base:.1%} base rate")
    # ⚠ Judge by LIFT over the base rate, not by an absolute share. A fixed 0.8 bar
    # called 73.8%-against-a-17.5%-base "chance" — a 4.2x lift. The share falls as
    # the map opens up and ocean dilutes the denominator, so only the ratio is
    # comparable across turns. It cannot reach 100%: `set_river_edge` marks the tile
    # across the segment too, and where that neighbour is unrevealed the export has
    # no `fw` to agree with.
    lift = rate / base if base > 0 else 0.0
    if not exported_rv:
        print("         ⚠ export carries no `rv` at all — old mod, rivers cannot be mirrored")
    elif not riv:
        print("         (no river tiles on the board yet)")
    elif lift < 1.5:
        problems.append("rivers")
        print(f"         ⚠ lift {lift:.1f}x over base — no better than chance, "
              f"these are the GENERATED map's rivers")
    else:
        print(f"         OK  ({lift:.1f}x base rate)")

    # --- landmass (#716) ---------------------------------------------------
    land = [(b, p) for b, p in pairs if not p.get("w")]
    named = sum(1 for _, p in land if p.get("ct"))
    with_cont = sum(1 for b, _ in land if b.get("continent") is not None)
    water_cont = sum(1 for b, p in pairs if p.get("w") and b.get("continent") is not None)
    cliffs = sum(1 for t in tiles.values() if any(t.get("cliff_edges") or []))
    print(f"LAND     {len(land)} land plots; export names a continent on {named}; "
          f"board assigns one to {with_cont}")
    if not named:
        print("         ⚠ export carries no `ct` — old mod")
    elif with_cont < named:
        problems.append("land")
        print(f"         ⚠ {named - with_cont} land plots lost their continent")
    else:
        print("         OK")
    # ⚠ NOT "water must have no continent". Civilization VI really does put COAST
    # tiles on a continent — 17 of them read CONTINENT_AUSTRALIA on this very run —
    # and carrying that is correct, because "another continent" is a rule and
    # dropping it would lose information. CIVVIS's own field doc says water has none;
    # that is CIVVIS's convention, not Civilization VI's, and the mirror follows the
    # game. So the check is agreement with the export.
    water_named = sum(1 for _, p in pairs if p.get("w") and p.get("ct"))
    if water_cont != water_named:
        print(f"         ⚠ board gives {water_cont} water tiles a continent, "
              f"the export names {water_named}")
    print(f"CLIFFS   {cliffs} " + ("⚠ invented — Civ 6 exposes no cliff accessor"
                                   if cliffs else "OK (none, as intended)"))
    if cliffs:
        problems.append("cliffs")

    # --- cities and units: entity-level, not tile-level ---------------------
    # ⚠ Counts alone are the weak check this project keeps getting burned by --
    # 21 units exported and 15 reconstructed once read as healthy because nothing
    # compared them. So compare the SETS, and name what is missing.
    state = latest_state(run, upto=board["turn"])
    if state is None:
        print("ENTITIES (no state event yet)")
        return 0

    civ6_cities = {(c["x"], c["y"]) for c in state.get("cities") or []}
    board_cities = {tuple(c["pos"]) for c in board.get("cities", [])
                    if c.get("owner") == board.get("view_player", 0)}
    mapped = {axial(x, best - y) for (x, y) in civ6_cities}
    missing_cities = mapped - board_cities
    if missing_cities:
        problems.append("cities")
    print(f"CITIES   export {len(civ6_cities)}  board {len(board_cities)}"
          + (f"   ⚠ MISSING {sorted(missing_cities)}" if missing_cities else "   OK"))

    # ⚠ Name what is missing, do not just count it. A bare "1 dropped" sends the
    # reader to the wrong place; the position and type say immediately whether it is
    # a known modelling gap (Great People are not units in CIVVIS) or something new.
    civ6_units = list(state.get("units") or [])
    ours = [u for u in board.get("units", []) if u.get("owner") == board.get("view_player", 0)]
    on_board = {tuple(u["pos"]) for u in ours if u.get("pos")}
    missing_units = [
        f'{u.get("kind", "?")}@{u.get("x")},{u.get("y")}'
        for u in civ6_units
        if axial(u.get("x", 0), best - u.get("y", 0)) not in on_board
    ]
    # ⚠ COUNT AND POSITION, because neither alone is enough. Position-matching
    # cannot see a STACKED drop -- Civilization VI puts a civilian and a military
    # unit on one tile and CIVVIS does not, so two exported units collapse onto one
    # board tile and every position still looks covered. The count catches that.
    # Position-matching, in turn, names WHICH unit is gone when the count is equal
    # but the board holds a different one.
    # ⚠ CATEGORISE, DO NOT SUPPRESS. CIVVIS does not model Great People as units at
    # all, so they are absent from the board on EVERY run. Failing the gate on a
    # documented modelling gap means the gate always fails, which is the same as
    # having no gate -- and it buries the drop that is actually new.
    #
    # They are still counted and still printed. What changes is that a known gap does
    # not set the exit status, so a NEW disappearance stands out against it.
    # ⚠ Counted from the EXPORT, not from `missing_units`. Great People stack with
    # other units, so position-matching covers them and they never appear in the
    # missing list -- the count path is where they land, and that is where they have
    # to be discounted. Getting this wrong left the gate failing on them anyway.
    great_people = [u for u in civ6_units if "GREAT_" in (u.get("kind") or "")]
    unexplained = [u for u in missing_units if "GREAT_" not in u]
    short = len(civ6_units) - len(ours) - len(great_people)
    if short > 0 or unexplained:
        problems.append("units")
    detail = ""
    if unexplained:
        detail = f"   ⚠ NOT on the board: {unexplained}"
    elif great_people and short <= 0:
        detail = (f"   OK — {len(great_people)} Great People absent, which CIVVIS does "
                  f"not model as units")
    elif short > 0:
        # ⚠ Do NOT name the cause. From the board alone a collapsed stack and an
        # unmodelled type (Great People are not units in CIVVIS) look identical, and
        # this line used to assert "a STACK was collapsed" when the decider's own
        # `dropped_units` was saying `great_person`. Report the fact, point at the
        # field that knows why.
        detail = (f"   ⚠ {short} fewer on the board beyond the {len(great_people)} "
                  f"Great People, every position covered — a stack collapsed or a type "
                  f"CIVVIS does not model; the decider's `dropped_units` names which")
    print(f"UNITS    export {len(civ6_units)}  board {len(ours)}"
          + (detail or "   OK"))

    # --- HOSTILES ----------------------------------------------------------
    #
    # ★★★★★ THE ONE THING ON THE BOARD THE SEAT MOST NEEDS TO SEE, AND NOTHING
    # CHECKED IT. Every other line here verifies what the empire OWNS. The threat
    # list is what it must plan AROUND, and until now no instrument compared it.
    #
    # Measured 2026-08-02 on run civvis-20260802T041527Z: 14 settlers were built
    # and every one vanished at hp 100 having moved 0-4 tiles from the capital,
    # while the city count sat at 1 from turn 41 to 241. Civilization VI CAPTURES
    # civilians rather than killing them, so full health at disappearance is the
    # signature of capture — and on each settler's last turn a hostile stood 1-3
    # tiles away, 8 of 13 of them ADJACENT. One of those "hostiles" was itself a
    # UNIT_SETTLER: ours, already taken.
    #
    # The first question that asks is whether the seat could SEE them, and it took
    # a hand-written pass over events.jsonl to answer (it could — 11 to 15 hostiles
    # exported on every capture turn). This line makes that answer automatic.
    #
    # ⚠ Hostiles are planted under the BARBARIAN SEAT, not the viewer, so they are
    # board units whose owner is not `view_player` — see `rebuild_from_state`'s
    # `barbarian_seat` branch, which records `no_barbarian_seat` for every hostile
    # when that seat is missing. A roster with no barbarian seat cannot hold the
    # threat list at all, and that reads here as every hostile missing.
    civ6_hostiles = list(state.get("hostiles") or [])
    seat = board.get("view_player", 0)
    theirs = [u for u in board.get("units", []) if u.get("owner") != seat]
    their_pos = {tuple(u["pos"]) for u in theirs if u.get("pos")}
    missing_hostiles = [
        f'{h.get("kind", "?")}@{h.get("x")},{h.get("y")}'
        for h in civ6_hostiles
        if axial(h.get("x", 0), best - h.get("y", 0)) not in their_pos
    ]
    # ⚠⚠ ONE DIRECTION ONLY, AND THE COUNTS ARE NOT COMPARABLE. `theirs` is every
    # board unit the viewer does not own — rivals and city-states as well as
    # barbarians — while `hostiles` is only the threat list. Printing the two
    # side by side as though they should match invites exactly the false reading
    # the rest of this file exists to prevent; the first run of this check read
    # `export 0  board 1` and looked like a defect when nothing was wrong.
    #
    # The invariant that IS true: every exported hostile must be somewhere on the
    # board. The reverse is not.
    #
    # ⚠ An empty threat list is a REAL state, not a failure. Early turns and quiet
    # stretches genuinely have none, and failing on that would cry wolf across most
    # of a peaceful game — the same care the FOG line takes over a board with
    # nothing fogged.
    if missing_hostiles:
        problems.append("hostiles")
    print(f"HOSTILES export {len(civ6_hostiles)}"
          + (f"   ⚠ {len(missing_hostiles)} NOT on the board: {missing_hostiles[:6]}"
             if missing_hostiles
             else ("   all on the board   OK" if civ6_hostiles
                   else "   none exported   OK")))

    # --- TREASURY ----------------------------------------------------------
    #
    # ⚠⚠ THESE TWO ARE STOCKS, NOT RATES, AND THAT IS WHY THEY GET THEIR OWN
    # CHECK RATHER THAN JOINING `economy_drift`.
    #
    # The mod exports `gold` from `GetTreasury():GetGoldBalance()` and `faith`
    # from `GetReligion():GetFaithBalance()` — balances. `economy_drift` compares
    # science and culture, which the mod takes from `GetTechs():GetScienceYield()`
    # — a per-turn rate. Putting a balance beside a rate under one heading is the
    # apples-to-oranges the rest of this file exists to prevent.
    #
    # ⚠ AND IT MUST BE READ AT THE SAME TURN, which is the whole reason this was
    # worth adding rather than eyeballing. Measured 2026-08-02 on run
    # civvis-20260802T030910Z: read against the NEWEST export the treasury showed
    # `gold 176 vs 167` and `faith 23 vs 21` — a confident-looking 5% shortfall
    # that is nothing but one turn of income at +9 gold and +2 faith. Bounded to
    # the board's own turn the same instant read **134 vs 134 and 27 vs 27,
    # delta 0.0 on both**. An unbounded version of this check would have reported
    # a treasury defect on a perfectly faithful mirror, every single time.
    board_me = next((p for p in board.get("players") or [] if p.get("id") == 0), None)
    if board_me is None:
        print("TREASURY no seated player 0 on the board; cannot compare")
    else:
        rows = []
        for field in ("gold", "faith"):
            theirs, ours_value = state.get(field), board_me.get(field)
            # -1 is the mod's own "could not read it" sentinel, and a missing key
            # is an older export. Neither is a disagreement — say so rather than
            # inventing a delta, the same way `economy_drift` refuses to claim
            # anything from an export carrying no yields.
            if theirs is None or ours_value is None or theirs < 0:
                rows.append(f"{field} unknown")
                continue
            delta = ours_value - theirs
            rows.append(f"{field} {theirs:g}/{ours_value:g}"
                        + ("" if abs(delta) < 0.5 else f" ⚠{delta:+g}"))
            if abs(delta) >= 0.5:
                problems.append(f"treasury:{field}")
        print(f"TREASURY {'  '.join(rows)}"
              + ("   OK" if not any("⚠" in r for r in rows) else ""))

    # ⚠ Non-zero on a real disagreement, so this can gate a run rather than only
    # inform one. A frame mismatch already returned 1 above for the same reason:
    # a comparison whose coordinates do not line up is worse than no comparison.
    if problems:
        print()
        print(f"DISAGREEMENTS: {', '.join(problems)}")
        return 1
    return 0


def latest_state(run, upto=None):
    """The `state` event as of `upto`, or the most recent one.

    ⚠ BOUND IT, for exactly the reason `load_export` is bounded. The mirror
    republishes on a cadence; the export keeps going. Comparing the LATEST unit
    positions against a board published several turns earlier reports units that
    have simply MOVED as units that were dropped -- this checker did that and
    named four healthy units as missing, while a same-turn read showed 25 against
    25 with the rosters matching type for type.

    That is the third time publish lag has fooled this file. If a future check
    reads the run directory, bound it too.
    """
    latest = None
    with open(os.path.join(run, "events.jsonl")) as handle:
        for line in handle:
            if '"state"' not in line:
                continue
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if (event.get("kind") or event.get("event")) != "state":
                continue
            if upto is not None and (event.get("turn") or 0) > upto:
                continue
            latest = event
    return latest


if __name__ == "__main__":
    sys.exit(main())
