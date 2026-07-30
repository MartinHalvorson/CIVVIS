#!/usr/bin/env python3
"""Play a whole game of Civilization VI under program control, and record it.

One command configures a game to an exact specification, starts the game with
no window clicking, lets ``tools/civ6_control``'s mod take a seat and play it,
and writes down what happened. That is the unit the difficulty ladder is built
from: the milestone "beaten on Settler" is one of these runs ending in a
victory event, with the log that proves it kept.

Usage::

    python tools/civ6_play.py --difficulty DIFFICULTY_SETTLER --tag settler-1
    python tools/civ6_play.py --difficulty DIFFICULTY_PRINCE --max-turns 200
    python tools/civ6_play.py --status
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent / "civ6_control"))
import civ6_env as env  # noqa: E402
from civ6_control import install as modinstall  # noqa: E402
from civ6_control import gamelock, launcher, vision, watch  # noqa: E402

RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"

# The ladder, weakest first. These are the game's own handicap type names; the
# ladder is climbed in this order and each rung is only claimed by a win.
LADDER = [
    "DIFFICULTY_SETTLER",
    "DIFFICULTY_CHIEFTAIN",
    "DIFFICULTY_WARLORD",
    "DIFFICULTY_PRINCE",
    "DIFFICULTY_KING",
    "DIFFICULTY_EMPEROR",
    "DIFFICULTY_IMMORTAL",
    "DIFFICULTY_DEITY",
]


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def load_settle_plan(path: str | None) -> list[dict] | None:
    """CIVVIS's ranked sites, or None when the agent should use its own search."""
    if not path:
        return None
    try:
        doc = json.loads(Path(path).read_text())
    except Exception as error:
        print(f"cannot read settle plan {path}: {error}", file=sys.stderr)
        return None
    sites = [
        {"x": int(site["x"]), "y": int(site["y"])}
        for site in doc.get("sites", [])
        if "x" in site and "y" in site
    ]
    print(f"settle plan: {len(sites)} CIVVIS-ranked sites from {path}")
    return sites or None


def build_config(args: argparse.Namespace) -> dict:
    return {
        "RunTag": args.tag,
        "AutoStart": True,
        "Play": True,
        # The Create Game screen configures the game now, so the agent must
        # not host another one: reconfiguring from inside a running game loses
        # the mod or the turn limit, and sometimes takes the process down.
        "Rehost": False,
        "Survey": args.survey,
        # The enum dump is a one-off: it costs ten very long log lines and the
        # answer does not change between runs on the same install.
        "SurveyEnums": args.survey_enums,
        "RuleSet": args.ruleset,
        "MapScript": args.map,
        "MapSize": args.map_size,
        "Difficulty": args.difficulty,
        "GameSpeed": args.speed,
        "MapSeed": args.seed,
        "GameSeed": args.seed,
        "MaxTurns": args.max_turns,
        "HumanPlayers": 1,
        "CityTarget": args.city_target,
        # Domination on a Duel map is the only victory reachable inside a
        # hundred-odd turns — an unassisted science win lands past turn 900 —
        # so the war knobs are exposed rather than left at their Lua defaults.
        "WarFromTurn": args.war_from_turn,
        "WarArmy": args.war_army,
        "MilitaryPerCity": args.military_per_city,
        "ExploreUntilTurn": args.explore_until_turn,
        # Domination on a four-civ map needs ALL THREE enemy original capitals.
        # A score victory at the turn limit needs only to be ahead, and warring
        # actively costs cities — one run went 4 cities to 3 while besieging.
        # So peace is a real strategy here, not a concession, and it is one flag.
        "MakeWar": args.make_war,
        # How many units are aimed at the city plot each turn. Each MOVE_TO onto
        # an enemy city is an attack that bounces back unless it captures, so
        # this is attacks per turn — and a city heals between turns, so too few
        # attackers means a siege that never resolves.
        "AssaultWidth": args.assault_width,
        "SettlersInFlight": args.settlers_in_flight,
        # How many of CIVVIS's top-ranked settle sites the settler may choose the
        # NEAREST of. 1 reproduces the old behaviour exactly — always the highest
        # ranked unoccupied plot — which makes it the control arm.
        #
        # Measured over twelve runs at window 1: 484 `move_to_site` orders against
        # 48 `found_city`, about ten turns of walking per city. One settler in
        # flight at one city per ~15 turns caps the empire at 3-4 over the ~100
        # turns a run actually survives, and the observed median IS 3. That
        # arithmetic starves the army, which is why war is declared in only 19 of
        # 47 runs and no capital has ever been taken.
        "PlanNearWindow": args.plan_near_window,
        # How much a rival being STRONGER than us costs in the war-target score,
        # expressed in tiles of walking per unit of score ratio. `findWarTarget` used
        # to weigh only proximity, so it declared on the runaway leader: the deepest
        # run held a war from turn 88 to 198 against a civ that grew 1 -> 10 visible
        # cities, eliminated another civ mid-war, and finished 1066 to our 203.
        "StrengthWeight": args.strength_weight,
        # Ceiling on the army target. Without one it is nCities * MilitaryPerCity —
        # 25 at five cities, never reached — and every development entry below the army
        # block in the ladder is dead code. A 203-turn game built 7 monuments, 7
        # granaries and ZERO districts, and scored 203 against 1088.
        "ArmyCap": args.army_cap,
        # ★ Maximum tiles from our NEAREST existing city that a new city may be
        # founded. Loyalty support comes from our own nearby population, so beyond
        # this the city cannot be held at any price: run 071729Z founded (42,17)
        # THIRTEEN tiles out and it opened at -23 loyalty a turn and was gone in
        # four. A governor is only +8, so this is the lever, not governors.
        "MaxEmpireDistance": args.max_empire_distance,
        # Two defenders a city is enough at Settler. Beyond that, production
        # spent on units that stand still is production not spent on the
        # districts and buildings that score is actually made of.
        "GarrisonPerCity": args.garrison_per_city,
        # Mirror the board into the log once a turn so CIVVIS can be the engine
        # that decides. Off by default: it is the largest emit in the mod.
        "ExportState": args.export_state,
        # CIVVIS's own settle ranking, produced by `civvis-advise --plan` from a
        # previous run's exported map. Baked in because the mod has no runtime
        # inbound channel: no `io`, and FireTuner answered none of seven framings
        # against a live game. Valid only for the SEED it was planned on — the
        # world is a function of the seed, so the same seed is the same map.
        "SettlePlan": load_settle_plan(args.settle_plan),
        "AnnouncementSeconds": args.announcement_seconds,
        "EraAnnouncementSeconds": args.era_announcement_seconds,
        "Leader": args.leader,
        "StartDelayFrames": args.start_delay_frames,
        "TickFrames": args.tick_frames,
    }


# Main-menu items as fractions of the *game window*, not the screen. The menu
# is laid out relative to the render target, so fractions survive a resize
# where pixels do not; measuring against the window rather than the desktop is
# what makes them survive the window being moved as well.
#
# One caveat that cost a misclick: the menu re-centres when a submenu opens, so
# these only hold when starting from a menu with no submenu showing. Clicking
# "Single Player" at its no-submenu position while a submenu is already open
# lands on "Benchmark".
MENU = {
    "single_player": (0.474, 0.455),
}

# Where the Single Player submenu sits horizontally. Its *rows* are read off
# the screen by civ6_control.vision rather than assumed: which entries it has
# depends on whether there is a save and a game to resume, so the same measured
# fraction is right in one run and a row off in the next.
SUBMENU_X = 0.528

# How many times to try the click sequence before giving up. Most failures are
# simply "the menu is not up yet" -- the logos play for minutes after the game
# core writes its mod scan -- so this is generous.
BOOTSTRAP_ATTEMPTS = 16

# The Create Game screen. Unlike the main menu this layout is fixed: the same
# controls in the same order every time, so these are measured once and hold.
# It is also the only route that sets difficulty *before* the game exists.
# Configuring afterwards was tried and does not work -- every menu path calls
# GameConfiguration.SetToDefaults() on entry, and reconfiguring from inside a
# running game and hosting again either loses the mod, loses the turn limit, or
# takes the application down.
START_GAME = (0.500, 0.982)
BACK = (0.730, 0.144)
SETUP_X = 0.500

# Each dropdown's closed box, as a fraction of window height.
DROPDOWN = {
    "difficulty": 0.3203,
    "speed": 0.3753,
    "map_type": 0.4304,
    "map_size": 0.4841,
}
# An open dropdown lists its options directly below the box: the first option
# sits a fixed distance under it and the rest step evenly. Measured on the
# difficulty, speed and map-size lists, which all agree.
OPTION_FIRST = 0.02174
OPTION_STEP = 0.018637

# Option order within each list, as the game presents it.
OPTIONS = {
    "difficulty": ["DIFFICULTY_SETTLER", "DIFFICULTY_CHIEFTAIN", "DIFFICULTY_WARLORD",
                   "DIFFICULTY_PRINCE", "DIFFICULTY_KING", "DIFFICULTY_EMPEROR",
                   "DIFFICULTY_IMMORTAL", "DIFFICULTY_DEITY"],
    "speed": ["GAMESPEED_ONLINE", "GAMESPEED_QUICK", "GAMESPEED_STANDARD",
              "GAMESPEED_EPIC", "GAMESPEED_MARATHON"],
    "map_size": ["MAPSIZE_DUEL", "MAPSIZE_TINY", "MAPSIZE_SMALL",
                 "MAPSIZE_STANDARD", "MAPSIZE_LARGE", "MAPSIZE_HUGE"],
    # ⚠ THIS ORDER IS A HYPOTHESIS, AND IT IS VERIFIED RATHER THAN TRUSTED.
    #
    # Setting the map through config does NOT work: `CivvisControlSetup.lua` never
    # runs because the FrontEnd context does not load on this install, so
    # `MapScript` is ignored and every game so far has been Continents. That
    # matters more than it sounds: on Continents a seat can start ALONE. Run
    # settler-20260730T045551Z reached turn 118 with `met = 0` after 415 explore
    # orders, and first contact came at turn 130 — far too late for domination,
    # which needs three capitals.
    #
    # The dropdown is the only route that works, and it needs an index.
    # `vision.py` reads row POSITIONS, not text, so the name cannot be matched on
    # screen. This order is the scripted maps from the shipped `Maps` table sorted
    # by SortIndex (Continents 10, Fractal 20, InlandSea 25, Island_Plates 30,
    # Lakes 35, Pangaea 40, ...), on the assumption that fixed-size static maps are
    # filtered out at Tiny.
    #
    # The `seat` event reports the script the game ACTUALLY generated, so a wrong
    # guess is caught on the first run rather than silently played for hours —
    # which is exactly how "we have been asking for Pangaea and playing Continents"
    # survived this long.
    "map_type": ["Continents.lua", "Fractal.lua", "InlandSea.lua",
                 "Island_Plates.lua", "Lakes.lua", "Pangaea.lua",
                 "Seven_Seas.lua", "Shuffle.lua", "Small_Continents.lua",
                 "Terra.lua"],
}


def game_window() -> tuple[int, int, int, int] | None:
    """Position and size of the game window in points, or None."""
    script = ('tell application "System Events" to tell '
              '(first process whose name contains "Civ6") to '
              'get {position, size} of window 1')
    out = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
    parts = [p.strip() for p in out.stdout.split(",") if p.strip()]
    if len(parts) != 4 or not all(p.lstrip("-").isdigit() for p in parts):
        return None
    x, y, w, h = (int(p) for p in parts)
    return (x, y, w, h) if w > 400 and h > 300 else None


# Which half of the screen the game gets. Set from --window-side/--window-frac
# in main; module-level so the focus helpers do not need threading through every
# call site.
GAME_SIDE = "left"
GAME_FRACTION = 0.5
GAME_VFRACTION = 1.0


def desktop_size() -> tuple[int, int] | None:
    """Logical desktop size in points, or None if it cannot be read.

    There is no supported scripting route to the screen size, so this asks
    Finder for its desktop scroll area and treats a failure as "leave the
    window alone" — never as a guess. `system_profiler` reports the *physical*
    resolution (3456x2234 on this Mac) and window geometry is in points
    (1728x1117), so mixing the two would place the game off-screen.
    """
    out = subprocess.run(
        ["osascript", "-e",
         'tell application "System Events" to get size of scroll area 1 '
         'of process "Finder"'],
        capture_output=True, text=True)
    parts = [p.strip() for p in out.stdout.split(",") if p.strip()]
    if len(parts) != 2 or not all(p.isdigit() for p in parts):
        return None
    width, height = (int(p) for p in parts)
    return (width, height) if width > 800 and height > 600 else None


def place_game(side: str = "left", fraction: float = 0.5,
               vfraction: float = 1.0) -> None:
    """Park the game on part of the screen so other windows can own the rest.

    `fraction` is the share of screen WIDTH and `vfraction` the share of HEIGHT,
    measured from the top. The default is a full-height half. `vfraction` exists
    so the game can take a quadrant instead: CIVVIS itself now wants a half, and
    the operator asked for the real game in the upper right with a terminal
    beneath it.

    Re-applied on every focus pass rather than once at launch: each ladder
    attempt relaunches Civ 6, and a fresh process comes up wherever the game
    last remembered rather than where it was put.
    """
    if side == "none":
        return
    size = desktop_size()
    if size is None:
        return
    screen_w, screen_h = size
    menu = 33  # the menu bar; a window placed at y=0 hides behind it
    width = max(640, int(screen_w * fraction))
    height = max(480, int((screen_h - menu) * max(0.1, min(1.0, vfraction))))
    x = 0 if side == "left" else screen_w - width
    script = (
        'tell application "System Events" to tell '
        '(first process whose name contains "Civ6") to tell window 1\n'
        f'  set position to {{{x}, {menu}}}\n'
        f'  set size to {{{width}, {height}}}\n'
        'end tell')
    subprocess.run(["osascript", "-e", script], capture_output=True)


def focus_game(side: str = "left", fraction: float = 0.5) -> None:
    """Raise the game. Deliberately does NOT move it.

    ⚠ Placing the window here broke setup outright. `vision.py` reads the
    Create Game submenu rows off the screen, and re-placing on every focus pass
    resized the window between the read and the click — run
    settler-20260729T233831Z never started a game at all, failing with
    "no submenu (0 rows)" and "no game window yet" while events.jsonl stayed
    empty at zero bytes. Menu navigation needs stable geometry; only the
    in-game loop may move the window.
    """
    del side, fraction  # kept for call-site compatibility
    script = ('tell application "System Events" to set frontmost of '
              '(first process whose name contains "Civ6") to true')
    subprocess.run(["osascript", "-e", script], capture_output=True)


def click_at(px: int, py: int) -> None:
    # Move first, then click. Clicking without moving lands wherever the
    # pointer already was, and cliclick's own move is not always processed by
    # the game before the button event.
    subprocess.run(["cliclick", f"m:{px},{py}"], capture_output=True)
    time.sleep(0.5)
    subprocess.run(["cliclick", f"c:{px},{py}"], capture_output=True)


def click_menu(item: str, bounds: tuple[int, int, int, int]) -> None:
    x, y, w, h = bounds
    fx, fy = MENU[item]
    # Exactly once. A second click on the same entry closes the submenu the
    # first one opened, which reads as "the click did nothing" and then sends
    # the follow-up click, aimed at the submenu, into whatever main-menu row
    # sits at that height. That cost four failed bootstrap attempts.
    click_at(int(x + w * fx), int(y + h * fy))


def screenshot(path: Path) -> None:
    """Keep a picture of the screen. A misclick is a visual failure and the
    log cannot describe it; the shot is what says which row was hit."""
    subprocess.run(["screencapture", "-x", "-t", "png", str(path)],
                   capture_output=True)


def set_dropdown(bounds: tuple[int, int, int, int], name: str, value: str) -> bool:
    """Open one Create Game dropdown and pick a value by its position in the list."""
    if value not in OPTIONS[name]:
        return False
    x, y, w, h = bounds
    box = DROPDOWN[name]
    click_at(int(x + w * SETUP_X), int(y + h * box))
    time.sleep(1.2)
    index = OPTIONS[name].index(value)
    click_at(int(x + w * SETUP_X),
             int(y + h * (box + OPTION_FIRST + index * OPTION_STEP)))
    time.sleep(1.2)
    return True


def configure_and_start(bounds: tuple[int, int, int, int], args: argparse.Namespace,
                        run_dir: Path) -> None:
    """Set this run's game up on the Create Game screen and start it.

    Nothing here is verified by looking at the screen -- the agent's first
    report from inside the game is the check, and it names the difficulty, map
    size and speed the game actually has. A misclick therefore shows up as a
    run that says so, not as a Deity result quietly recorded as Settler.
    """
    set_dropdown(bounds, "difficulty", args.difficulty)
    set_dropdown(bounds, "map_size", args.map_size)
    set_dropdown(bounds, "speed", args.speed)
    # The map has to be chosen HERE. `MapScript` in the baked config is ignored,
    # because the FrontEnd context that would read it never loads, so every game so
    # far has been Continents whatever was asked for. On Continents a seat can start
    # alone: one run reached turn 118 with `met = 0`, and first contact at turn 130 is
    # too late for domination. The `seat` event reports the script the game actually
    # generated, so a wrong row shows up as a run that says so.
    # ⚠ REVERTED, AND LEFT REVERTED UNTIL IT CAN BE VERIFIED.
    #
    # Selecting the map here broke setup outright: four consecutive attempts logged
    # "no game started" and no `seat` event ever arrived, where the same path had
    # been reliable for hours. The dropdown row is an unverified guess (`vision.py`
    # reads row POSITIONS, not text, so "Pangaea" cannot be matched on screen), and
    # an unverified guess that breaks a working path is not worth keeping.
    #
    # The problem it was aimed at is REAL and still open: `MapScript` in the baked
    # config is ignored because the FrontEnd context never loads, so every game is
    # Continents, and on Continents a seat can start ALONE — one run reached turn 118
    # with `met = 0`. Fixing it properly needs OCR on the dropdown rows, or reading
    # the selected value back off the screen before committing to Start Game.
    if args.map in OPTIONS["map_type"]:
        print(f"map selection is disabled pending verification; the game will "
              f"generate its default rather than {args.map}", file=sys.stderr)
    screenshot(run_dir / "setup.png")
    x, y, w, h = bounds
    click_at(int(x + w * START_GAME[0]), int(y + h * START_GAME[1]))


def bootstrap_game(tail: watch.LogTail, on_event, run_dir: Path,
                   args: argparse.Namespace, verify_s: float = 120.0) -> bool:
    """Start *a* game from the main menu, so the agent has a context to work in.

    This is the one place a click is still needed. A mod's FrontEnd context
    does not load on this build, so nothing of ours runs at the main menu and
    the game cannot be configured before it starts. The agent handles the rest:
    the game started here carries no settings marker, so it reconfigures and
    hosts again, and the second game is the one that gets played.

    Clicking is verified rather than assumed. The menu re-lays-out when a
    submenu opens and the window can be a different size than last run, so a
    click that misses is normal; what is not acceptable is a run that proceeds
    to wait an hour for a game that was never started. The agent writing
    anything at all is the proof that a game exists.
    """
    def started(seconds: float) -> bool:
        deadline = time.time() + seconds
        while time.time() < deadline:
            saw = False
            for event in tail.poll():
                on_event(event)
                saw = True
            if saw:
                return True
            if not env.game_pids():
                return False
            time.sleep(2.0)
        return False

    # The mod scan lands in Modding.log minutes before the menu can be clicked
    # -- the 2K and Firaxis logos play over the top of it -- so "main menu
    # reached" is not the same as "main menu ready". Rather than guess a settle
    # time, each attempt clicks Single Player and looks for the submenu it
    # should have opened. No submenu means the menu was not up yet, which is a
    # reason to wait, not a reason to give up: an earlier run bailed with "no
    # game window found" while the game was still showing the 2K logo.
    for attempt in range(1, BOOTSTRAP_ATTEMPTS + 1):
        focus_game(GAME_SIDE, GAME_FRACTION)
        time.sleep(2.0)
        bounds = game_window()
        if bounds is None:
            print("no game window yet, waiting", file=sys.stderr)
            time.sleep(20.0)
            continue
        x, y, w, h = bounds
        # Make the window key with a click on empty artwork, well clear of the
        # menu: the first click on a background window is consumed activating
        # it, and spending that click on a menu entry loses the entry.
        click_at(int(x + w * 0.15), int(y + h * 0.85))
        time.sleep(1.5)

        click_menu("single_player", bounds)
        time.sleep(2.5)
        # Read the submenu rather than assume its shape. Which entries it has
        # depends on whether there is a save and a game to resume, so where
        # "Create Game" lands moves between runs.
        submenu = run_dir / f"submenu-attempt{attempt}.png"
        screenshot(submenu)
        rows = vision.submenu_rows(submenu, bounds) if vision.available() else []
        if len(rows) < 3:
            print(f"attempt {attempt}: no submenu ({len(rows)} rows) -- "
                  "the menu is not ready yet", file=sys.stderr)
            if not env.game_pids():
                print("the game exited while starting", file=sys.stderr)
                return False
            time.sleep(20.0)
            continue
        target = rows[-3]
        print(f"create game row: {target:.4f} (read from {len(rows)} submenu rows)")
        click_at(int(x + w * SUBMENU_X), int(y + h * target))
        time.sleep(2.5)
        screenshot(run_dir / f"create-attempt{attempt}.png")
        configure_and_start(bounds, args, run_dir)
        if started(verify_s):
            return True
        if not env.game_pids():
            print("the game exited while starting", file=sys.stderr)
            return False
        # Back out of whatever that opened and try again.
        click_at(int(x + w * BACK[0]), int(y + h * BACK[1]))
        time.sleep(1.0)
        press_escape(1)
        time.sleep(1.5)
        print(f"attempt {attempt}: no game started from row {target:.4f}",
              file=sys.stderr)
    return False


def dismiss_leader_dialogue(clicks: int = 4) -> bool:
    """Click through a leader conversation's dialogue options until it closes.

    ⚠ THIS IS THE FOURTH APPROACH TO THIS SCREEN AND THE FIRST ONE THAT WORKS.
    Verified by hand against a live stuck screen. The three that did not:

    * `ExitConversationMode` — only acts `if ms_currentViewMode ==
      CONVERSATION_MODE`, and a first-contact leader is CINEMA_MODE.
    * `CloseFocusedState` — its cinema branch is gated on a fade animation being
      stopped, and it never fired.
    * Escape — does nothing at all on this screen. Twice, with focus confirmed.

    A first-contact screen offers a stack of dialogue options in the lower-left of
    the game window, and it closes only when one is chosen. Each click consumes
    the option under the cursor and the stack shrinks, so clicking the same spot
    repeatedly walks down it and the last one ends the conversation. That is
    exactly what a person does.

    Measured on this display: the stack sits at x ~= 123 pt and the bottom option
    at y ~= 817 of 1117, so the position is taken as a fraction of the desktop
    rather than hardcoded. `clicks` defaults to 4 because three options plus one
    spare covers every first-contact screen seen so far.
    """
    # ⚠ MEASURE THE WINDOW, NOT THE DESKTOP.
    #
    # This first computed the position from the desktop size and GAME_FRACTION,
    # which was right only while the game owned the left half at full height. The
    # moment the operator asked for the game in the upper-right quadrant
    # (864,33,864,542) those clicks landed on the TERMINAL instead, and a run sat
    # stalled for ten minutes with the harness reporting "dialogue clicks sent".
    # A position derived from an assumption about layout is a position that breaks
    # when the layout changes; the window knows where it is.
    rect = game_window()
    if rect is None:
        print("[dialogue] cannot read the game window; not guessing a position",
              file=sys.stderr)
        return False
    wx, wy, ww, wh = rect
    focus_game(GAME_SIDE, GAME_FRACTION)
    time.sleep(1.0)
    # ⚠ THESE SCREENS ARE NOT ALL THE SAME SHAPE, and assuming they were cost a
    # run. A first-contact conversation offers a stack of dialogue options at the
    # LOWER-LEFT; a trade proposal (`DiplomacyDealView`) offers Accept/Refuse near
    # the TOP. Clicking the conversation position on a deal screen hits empty space,
    # which is exactly what happened: the harness logged "dialogue clicks sent"
    # while a peace offer from Wilhelmina sat unanswered for eleven minutes and the
    # run burned its stall timeout.
    #
    # ⚠ REFUSE, NEVER ACCEPT. The refuse button is clicked first and deliberately.
    # An accepted deal can cede cities, gold per turn or a peace treaty, and a peace
    # treaty ends the war that domination depends on — the only victory route still
    # open. Refusing an unseen offer costs nothing; accepting one can cost the game.
    # ⚠ THREE DIFFERENT SHAPES, and missing the third cost a run 457 seconds.
    #   * a trade proposal puts Accept/Refuse near the TOP
    #   * a single-button leader ("That's a shame." / Goodbye) sits at ~0.91 DOWN
    #   * a three-option conversation stack sits around 0.68-0.73
    # The stack positions miss the single-button variant completely. Verified by
    # hand: 0.172/0.913 recovered a run that had been stuck for 457s.
    targets = [
        ("refuse deal", 0.222, 0.174),
        ("goodbye only", 0.172, 0.913),
        ("dialogue stack", 0.14, 0.73),
        ("dialogue stack (2nd)", 0.14, 0.68),
    ]
    print(f"[dialogue] window {rect}")
    for name, fx, fy in targets:
        x, y = int(wx + ww * fx), int(wy + wh * fy)
        print(f"[dialogue]   {name} -> ({x},{y})")
        for _ in range(max(1, clicks // len(targets))):
            click_at(x, y)
            time.sleep(0.7)
    return True


def press_escape(times: int = 2) -> bool:
    """Dismiss the load screen, for when the mod's own dismissal does not land.

    The agent raises the two dismissals the shipped screen offers from Lua and
    they do work; this stays as the fallback because a run stuck here writes no
    log line at all and looks exactly like a slow map. It is deliberately not
    on the normal path: Escape during play opens the pause menu.
    """
    focus_game(GAME_SIDE, GAME_FRACTION)
    time.sleep(1.0)
    ok = True
    for _ in range(times):
        result = subprocess.run(["cliclick", "kp:esc"], capture_output=True)
        ok = ok and result.returncode == 0
        time.sleep(0.8)
    return ok


def play(args: argparse.Namespace) -> int:
    # One run at a time against this installation. Two harnesses share one mod
    # directory, one log and one process; the second one's install lands in the
    # middle of the first one's game and neither notices.
    if not gamelock.acquire(args.tag, wait_s=args.lock_wait):
        foreign = gamelock.foreign_run(args.tag)
        print(f"another run holds the game: {foreign or gamelock.describe()}",
              file=sys.stderr)
        return 6
    try:
        return _play(args)
    finally:
        gamelock.release()


def _play(args: argparse.Namespace) -> int:
    config = build_config(args)
    run_dir = RUN_ROOT / args.tag
    run_dir.mkdir(parents=True, exist_ok=True)
    events_path = run_dir / "events.jsonl"
    events = events_path.open("a")

    target = modinstall.install(config)
    print(f"installed {target}")
    print(f"  difficulty {config['Difficulty']}  map {config['MapSize']}"
          f"  speed {config['GameSpeed']}  max turns {config['MaxTurns']}")

    launcher.stop()
    launcher.clear_run_logs()
    launcher.launch(stdout=run_dir / "stdout.log")
    if not launcher.wait_for_main_menu(args.startup_timeout):
        print("the game did not reach the main menu", file=sys.stderr)
        return 3
    print("main menu reached; the setup context should host the game now")

    tail = watch.LogTail()
    state = {
        "hosted": False, "seat": None, "turn": -1, "score": -1,
        "outcome": None, "last_progress": time.time(), "configured": False,
    }

    def record(event: dict) -> None:
        events.write(json.dumps(event, sort_keys=True) + "\n")
        events.flush()
        kind = event.get("kind")
        if kind == "host":
            state["hosted"] = bool(event.get("started"))
            print(f"[setup] host started={event.get('started')}")
        elif kind == "rehost":
            state["hosted"] = True
            print(f"[agent] rehosting: difficulty={event.get('difficulty')} "
                  f"size={event.get('size')} speed={event.get('speed')} "
                  f"max_turns={event.get('max_turns')} humans={event.get('humans')} "
                  f"configured={event.get('configured')} {event.get('error') or ''}")
        elif kind == "turn_limit":
            print(f"[agent] turn limit asked={event.get('asked')} "
                  f"config={event.get('config')} game={event.get('game')}")
        elif kind == "war":
            print(f"[agent] war declared on player {event.get('target')} at "
                  f"({event.get('x')},{event.get('y')}) "
                  f"capital={event.get('capital')} army={event.get('army')}")
        elif kind == "actions" and event.get("missing"):
            print(f"[agent] unavailable actions: {event['missing']}")
        elif kind == "seat":
            state["seat"] = event
            # "Configured" means the game that is being played is the game this
            # run asked for -- read back from inside it, not from the command
            # line. Without this a misclick on the setup screen records a
            # Prince result under a Settler heading.
            state["configured"] = (
                event.get("difficulty") == args.difficulty
                and event.get("size") == args.map_size
                and event.get("speed") == args.speed)
            if not state["configured"]:
                print("[agent] the game does not match what was asked for",
                      file=sys.stderr)
            print(f"[agent] seat {event.get('local_player')} {event.get('civ')} "
                  f"difficulty={event.get('difficulty')} size={event.get('size')} "
                  f"speed={event.get('speed')} max_turns={event.get('max_turns')} "
                  f"players={event.get('players')} setup={event.get('setup')}")
        elif kind == "turn":
            state["turn"] = event.get("turn", -1)
            state["score"] = event.get("score", -1)
            state["last_progress"] = time.time()
            if state["turn"] % args.report_every == 0:
                actions = event.get("actions") or {}
                summary = " ".join(f"{k}={v}" for k, v in sorted(actions.items()))
                print(f"[turn {state['turn']:>4}] score={event.get('score')} "
                      f"cities={event.get('cities')} units={event.get('units')} "
                      f"blocker={event.get('blocker')} "
                      f"ticks={event.get('ticks_taken')}/{event.get('ticks_seen')} "
                      f"| {summary}")
        elif kind == "blocked":
            print(f"[turn {event.get('turn')}] blocked on {event.get('blocker')} "
                  f"({event.get('attempts')} attempts)")
        elif kind == "autoclose_stuck":
            # ⚠ THE SHIM HAS GIVEN UP, so press the key a person would.
            #
            # `autoclose_stuck` means twenty close attempts failed and the screen
            # called ClearUpdate, so it will never try again — the screen is up
            # for the rest of the game. Run settler-20260730T021107Z halted at
            # turn 121 on one, with four cities and a score of 126, the best of
            # the session.
            #
            # A leader conversation needs a dialogue option CHOSEN; everything
            # else on this list just needs dismissing. Escape was tried for the
            # conversation case and does nothing at all on it — verified by hand
            # against a live stuck screen — so the two get different treatment.
            screen = event.get("screen")
            print(f"[autoclose_stuck] {screen} gave up after "
                  f"{event.get('attempts')} attempts")
            if screen in ("DiplomacyActionView", "LeaderView", "DiplomacyDealView"):
                ok = dismiss_leader_dialogue()
                how = "dialogue clicks"
            else:
                ok = press_escape()
                how = "escape"
            print(f"[autoclose_stuck] {how} "
                  f"{'sent' if ok else 'FAILED'} for {screen}",
                  file=sys.stderr if not ok else sys.stdout)
        elif kind in ("victory", "defeat", "error"):
            print(f"[{kind}] {json.dumps(event, sort_keys=True)}")
            if kind in ("victory", "defeat"):
                state["outcome"] = event

    def finished(event: dict) -> bool:
        """Only OUR victory or OUR defeat ends the run.

        ⚠ This used to stop on any `defeat` event, and a Civilization VI game
        emits one every time ANY player is eliminated — including a rival or a
        city-state. Run settler-20260730T013057Z was stopped at turn 234 of 250
        with a score of 185 because player 7 died: sixteen turns short of the
        turn limit, which is where a score victory is awarded. The mod already
        gets this right — it sets `finished` only when the defeated player is the
        local one — and the harness was throwing that distinction away.

        `victory` carries `won`; `defeat` carries `ours`. Neither is a reason to
        stop unless it is about us.
        """
        kind = event.get("kind")
        if kind == "victory":
            return True
        if kind == "defeat":
            return bool(event.get("ours"))
        return False

    if not bootstrap_game(tail, record, run_dir, args):
        print("could not start a game from the main menu", file=sys.stderr)
        return 5
    print("in a configured game; the agent holds the seat from here")

    # Hold the foreground for the whole game. Anything else taking focus --
    # a browser, another agent's automation -- throttles the game to almost no
    # frames, and the turn loop runs off game-core events, so the run stops
    # without a single log line saying why.
    last_focus = [0.0]

    def keep_foreground() -> None:
        now = time.time()
        if now - last_focus[0] < args.focus_every:
            return
        last_focus[0] = now
        focus_game()
        # Safe here and only here: the game is in play, so there is no menu
        # being read off the screen for a resize to invalidate.
        place_game(GAME_SIDE, GAME_FRACTION, GAME_VFRACTION)

    reason = watch.follow(tail, args.timeout, record, stop_when=finished,
                          each_poll=keep_foreground)
    events.close()

    outcome = state["outcome"] or {}
    summary = {
        "tag": args.tag,
        "finished_utc": utc_stamp(),
        "difficulty": config["Difficulty"],
        "map_size": config["MapSize"],
        "speed": config["GameSpeed"],
        "seed": config["MapSeed"],
        "max_turns": config["MaxTurns"],
        "reason": reason,
        # Whether the game actually played was the one this run asked for.
        # A summary that reports the requested difficulty without this is a
        # claim about the command line, not about the game.
        "configured": state["configured"],
        "last_turn": state["turn"],
        "last_score": state["score"],
        "seat": state["seat"],
        "outcome": outcome or None,
    }
    (run_dir / "summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True))
    print(json.dumps(summary, indent=2, sort_keys=True))

    if outcome.get("kind") == "victory" and outcome.get("team") == outcome.get("local_team"):
        return 0
    return 1


def status() -> int:
    print(f"user dir : {env.user_dir()}")
    print(f"install  : {modinstall.install_dir()}"
          f"  ({'present' if modinstall.install_dir().is_dir() else 'absent'})")
    cfg = modinstall.installed_config()
    if cfg:
        for key in sorted(cfg):
            print(f"  {key:<20} {cfg[key]!r}")
    print(f"running  : {env.game_pids() or 'no'}")
    log = env.logs_dir() / "Automation.log"
    if log.is_file():
        hits = sum(1 for line in log.read_text(errors="replace").splitlines()
                   if "CIVVISJSON" in line)
        print(f"log      : {log} ({hits} CIVVISJSON lines)")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--tag", default=None, help="run tag; names the run directory")
    ap.add_argument("--difficulty", default="DIFFICULTY_SETTLER", choices=LADDER)
    ap.add_argument("--ruleset", default="RULESET_EXPANSION_2")
    ap.add_argument("--map", default="Continents.lua")
    ap.add_argument("--map-size", default="MAPSIZE_DUEL")
    ap.add_argument("--speed", default="GAMESPEED_ONLINE")
    ap.add_argument("--seed", type=int, default=424242)
    ap.add_argument("--max-turns", type=int, default=150)
    ap.add_argument("--city-target", type=int, default=6)
    # Standardise the civilization so two attempts are comparable. Rome's free
    # monument and road on every founding is a different game from a random
    # leader's, and a ladder climbed by a different civ each rung measures
    # nothing. Takes effect only when the setup context hosts the game; the
    # `seat` event reports the leader actually granted.
    ap.add_argument("--leader", default="LEADER_TRAJAN")
    # The game must stay frontmost to get frames, which makes it unwatchable if
    # it also owns the whole screen. Half is enough for the agent and leaves the
    # other half for a terminal.
    ap.add_argument("--window-side", choices=["left", "right", "none"],
                    default="left")
    ap.add_argument("--window-frac", type=float, default=0.5)
    # Taking a walled capital needs a real army, not a garrison. Four units is
    # the Lua default and is thin for a capture even at Settler.
    ap.add_argument("--war-from-turn", type=int, default=25)
    ap.add_argument("--war-army", type=int, default=4)
    ap.add_argument("--military-per-city", type=float, default=1.5)
    ap.add_argument("--explore-until-turn", type=int, default=12)
    ap.add_argument("--make-war", dest="make_war", action="store_true", default=True)
    ap.add_argument("--no-war", dest="make_war", action="store_false")
    ap.add_argument("--assault-width", type=int, default=2)
    ap.add_argument("--settlers-in-flight", type=int, default=1)
    # 1 = the shipped behaviour (always CIVVIS's top-ranked unoccupied site),
    # which is what makes it a usable control arm for the near-window A/B.
    ap.add_argument("--plan-near-window", type=int, default=6)
    ap.add_argument("--strength-weight", type=int, default=20)
    ap.add_argument("--army-cap", type=int, default=18)
    ap.add_argument("--max-empire-distance", type=int, default=6)
    ap.add_argument("--garrison-per-city", type=int, default=2)
    ap.add_argument("--export-state", action="store_true", default=False)
    ap.add_argument("--settle-plan", default=None,
                    help="JSON from `civvis-advise --plan`: CIVVIS decides where "
                         "cities go. Only valid for the seed it was planned on.")
    ap.add_argument("--window-vfrac", type=float, default=1.0,
                    help="share of screen height for the game window; 0.5 puts "
                         "it in a quadrant so CIVVIS can own the other half")
    ap.add_argument("--announcement-seconds", type=float, default=1.0)
    ap.add_argument("--era-announcement-seconds", type=float, default=0.5)
    ap.add_argument("--survey", action="store_true", default=True)
    ap.add_argument("--no-survey", dest="survey", action="store_false")
    ap.add_argument("--survey-enums", action="store_true",
                    help="dump every action enum this build defines (one-off)")
    ap.add_argument("--start-delay-frames", type=int, default=240)
    ap.add_argument("--tick-frames", type=int, default=12)
    ap.add_argument("--startup-timeout", type=float, default=420.0)
    ap.add_argument("--host-timeout", type=float, default=300.0)
    ap.add_argument("--load-wait", type=float, default=90.0)
    ap.add_argument("--timeout", type=float, default=7200.0)
    ap.add_argument("--report-every", type=int, default=5)
    ap.add_argument("--lock-wait", type=float, default=0.0,
                    help="seconds to wait for another run to finish")
    ap.add_argument("--focus-every", type=float, default=15.0,
                    help="seconds between raising the game window (0 disables)")
    ap.add_argument("--status", action="store_true")
    args = ap.parse_args(argv)
    global GAME_SIDE, GAME_FRACTION, GAME_VFRACTION
    GAME_SIDE, GAME_FRACTION = args.window_side, args.window_frac
    GAME_VFRACTION = args.window_vfrac

    if args.status:
        return status()
    if args.tag is None:
        args.tag = (args.difficulty.replace("DIFFICULTY_", "").lower()
                    + "-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ"))
    return play(args)


if __name__ == "__main__":
    sys.exit(main())
