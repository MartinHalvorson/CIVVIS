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
- SETUP   Civ VI's seat settings must be the board's settings. A mirror that says
          Prince/Pangaea beside a Settler/Continents game is wrong before its first move.
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

## Three ways this checker itself cried wolf

Each of these is a real bug I nearly reported, caught only by looking again:

1. It compared the LATEST export against a board published up to 30s earlier and
   called the game's own progress "13 exported plots dropped". The export is now
   bounded to the board's turn -- see `load_export(upto=)`.
2. It judged rivers on an absolute share and called a 4.3x lift over the base
   rate "no better than chance". Judge the LIFT.
3. It asserted water carries no continent. Civilization VI really does put COAST
   tiles on a continent -- 17 read CONTINENT_AUSTRALIA on one run -- so the check
   is agreement with the export, not a rule of its own.

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


def civ6_id(value, prefix):
    """Normalize a Civ VI type identifier to a lower-case CIVVIS-style id."""
    value = str(value or "").strip()
    if value.upper().startswith(prefix):
        value = value[len(prefix):]
    return value.lower()


def civ6_map_script(value):
    """Normalize the Civ VI map-file spelling CIVVIS mirrors."""
    value = civ6_id(value, "")
    if value.endswith(".lua"):
        value = value[:-4]
    return {"smallcontinents": "small_continents"}.get(value, value)


def latest_seat(run):
    """The latest startup seat event, which carries setup outside state patches."""
    latest = None
    with open(os.path.join(run, "events.jsonl")) as handle:
        for line in handle:
            if '"seat"' not in line:
                continue
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if (event.get("kind") or event.get("event")) == "seat":
                latest = event
    return latest


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

    # --- lobby setup -------------------------------------------------------
    # The seat event is emitted once rather than copied into each state patch.
    # Compare its actual Civ VI identifiers to the reconstructed board instead
    # of assuming the command-line defaults used to launch the viewer survived.
    seat = latest_seat(run)
    if seat is None:
        print("SETUP    no seat event yet")
    else:
        player = next((p for p in board.get("players", [])
                       if p.get("id") == board.get("view_player", 0)), {})
        expected = {
            "speed": civ6_id(seat.get("speed"), "GAMESPEED_"),
            "difficulty": civ6_id(seat.get("difficulty"), "DIFFICULTY_"),
            "map": civ6_map_script(seat.get("map")),
            "size": civ6_id(seat.get("size"), "MAPSIZE_"),
            "civ": civ6_id(seat.get("civ"), "CIVILIZATION_"),
            "leader": civ6_id(seat.get("leader"), "LEADER_"),
        }
        actual = {
            "speed": str(board.get("game_speed") or "").lower(),
            "difficulty": str(board.get("difficulty") or "").lower(),
            "map": str(board.get("map", {}).get("script") or "").lower(),
            "size": str(board.get("map", {}).get("size") or "").lower(),
            "civ": str(player.get("civ") or "").replace(" ", "_").lower(),
            "leader": str(player.get("leader") or "").replace(" ", "_").lower(),
        }
        mismatches = [key for key, want in expected.items()
                      if want and actual.get(key) != want]
        print("SETUP    "
              f"speed {actual['speed'] or '?'}; "
              f"difficulty {actual['difficulty'] or '?'}; "
              f"map {actual['map'] or '?'}/{actual['size'] or '?'}; "
              f"{player.get('civ') or '?'} / {player.get('leader') or '?'}")
        if mismatches:
            problems.append("setup")
            detail = ", ".join(f"{key} Civ6={expected[key] or '?'} "
                               f"CIVVIS={actual[key] or '?'}" for key in mismatches)
            print(f"         ⚠ {detail}")
        else:
            print("         OK")

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
    # Keep entities on the same temporal boundary as terrain. A game can export
    # the next state's units between the `/state` fetch and this read; comparing
    # them to an older board reports ordinary movement as a dropped mirror unit.
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
    short = len(civ6_units) - len(ours)
    if short > 0 or missing_units:
        problems.append("units")
    detail = ""
    if missing_units:
        detail = f"   ⚠ NOT on the board: {missing_units}"
    elif short > 0:
        detail = (f"   ⚠ {short} fewer on the board with every position covered — "
                  f"a STACK was collapsed; see `dropped_units` for the reason")
    print(f"UNITS    export {len(civ6_units)}  board {len(ours)}"
          + (detail or "   OK"))
    # ⚠ Non-zero on a real disagreement, so this can gate a run rather than only
    # inform one. A frame mismatch already returned 1 above for the same reason:
    # a comparison whose coordinates do not line up is worse than no comparison.
    if problems:
        print()
        print(f"DISAGREEMENTS: {', '.join(problems)}")
        return 1
    return 0


def latest_state(run, upto=None):
    """The newest state event no later than ``upto``, or None."""
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
            if upto is not None and int(event.get("turn") or 0) > upto:
                continue
            latest = event
    return latest


if __name__ == "__main__":
    sys.exit(main())
