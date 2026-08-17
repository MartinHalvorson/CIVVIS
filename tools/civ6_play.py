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
import atexit
import json
import os
import subprocess
import textwrap
import sys
import tempfile
import time
import unicodedata
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent / "civ6_control"))
import civ6_env as env  # noqa: E402
from civ6_control import install as modinstall  # noqa: E402
from civ6_control import (gamelock, launcher, macos_input, macos_ocr,
                          popup_clear, vision, watch)  # noqa: E402
from civ6_control.orders import orders_db_path, reset_orders_db  # noqa: E402

RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"
REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_CIVVIS_VICTORY = "science"
DEFAULT_CIVVIS_STRATEGY = ""

# The ladder, weakest first. These are the game's own handicap type names; the
# ladder is climbed in this order and each rung is only claimed by a win.
# Civilization VI's optional game modes, from the `ConfigurationId`s its
# content packs register. They are all OFF unless a run asks for one.
#
# ⚠ These PERSIST. `GameConfiguration.SetToDefaults()` does not clear them, so a
# mode enabled once -- by a person hosting a game, or by GAMEMODE_RANDOM
# choosing some -- stays on for every run afterwards. That is not hypothetical:
# GAMEMODE_HEROES was found true on a live run, which had been playing with
# twelve hero units and the Heroes & Legends rules while every log said plain
# Gathering Storm. Nothing in this harness set them and nothing reported them,
# so there was no reading that could have been wrong -- the axis did not exist.
#
# CIVVIS's ruleset is Gathering Storm and nothing else; `src/elo.rs` writes the
# same thing into its setup contract as `modes=none`. A mode adds units,
# districts and rules CIVVIS has no model for, so a run with one on is not
# measuring the game CIVVIS is being compared against.
GAME_MODES = [
    "GAMEMODE_APOCALYPSE",
    "GAMEMODE_BARBARIAN_CLANS",
    "GAMEMODE_DRAMATICAGES",
    "GAMEMODE_HEROES",
    "GAMEMODE_MONOPOLIES",
    # Not a mode but a chooser: left on it enables the others for you, so a
    # harness that cleared the other eight and not this one would still be
    # playing a game nobody picked.
    "GAMEMODE_RANDOM",
    "GAMEMODE_SECRETSOCIETIES",
    "GAMEMODE_TOWERDEFENSE",
    "GAMEMODE_TREE_RANDOMIZER",
]

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


def startup_event_proves_game_started(event: dict) -> bool:
    """Return whether an event proves the in-game agent has loaded.

    Auto-close contexts are installed before the map is playable and emit
    ``autoclose_armed`` during the setup/intro screens.  They prove only that
    the mod package was discovered, not that the requested game is ready for
    the agent.  The agent lifecycle events are the first reliable boundary.
    """
    return (
        event.get("ctx") == "agent"
        and event.get("kind") in {"loaded", "seat", "turn"}
    )


def board_event_proves_intro_is_gone(event: dict) -> bool:
    """Return whether the live board, not merely the agent, is available.

    The agent UI context loads behind Civilization VI's leader introduction, so
    its ``loaded`` event must *not* make the launcher skip a verified Begin
    Game click.  A turn or an exported state, on the other hand, requires the
    playable board and lets us stop repeatedly OCRing an intro that is already
    gone.  This keeps the modal safety gate while avoiding a full startup
    budget of redundant screenshots after a direct host transition.
    """
    return (
        event.get("ctx") == "agent"
        and event.get("kind") in {"state", "turn"}
    )


def wait_for_agent_start(tail, on_event, seconds: float,
                         still_loading=None) -> bool:
    """Wait for the in-game agent, staying patient while the game LOADS.

    ⚠⚠⚠ THE FIXED DEADLINE KILLED FOUR CONSECUTIVE STARTS ON 2026-08-10.
    #1481 made this gate require `agent` lifecycle telemetry rather than any
    event at all, which is right -- an `autoclose_armed` from a setup screen is
    not a live game. But the gate kept a FIXED budget, and on this machine map
    generation after Begin Game can take longer than the 120 s default. Runs
    civvis-20260810T171138Z, ...T171753Z, ...T172435Z and ...T173214Z each ended
    with exactly 22 `autoclose_armed` events and ZERO `agent` events; the caller
    then ran `return_to_main_menu`, which pressed ESC on a game that was still
    loading and walked into "are you sure you wish to quit". The revision
    immediately before #1481 played a 232-turn game on the same machine and
    settings, because its gate accepted the first autoclose event and waited.

    #1505 answered that with "any event extends the budget", which is right for
    a mod that keeps talking -- and cannot reach the case it was written for. A
    game GENERATING A MAP emits NOTHING: its in-game context has not loaded, so
    there is no one to talk. The only events available to extend the budget are
    the setup-screen batch already buffered when the gate opens, and those all
    arrive on the first poll at t=0, where `now + seconds` is the deadline the
    gate already had. So the extension is a no-op on a dead click and
    unreachable on a live load, and the gate still expires at exactly `seconds`
    -- which is why runs civvis-20260810T194817Z and ...T195339Z died the same
    way AFTER #1505 landed, with the same 22 `autoclose_armed` and zero `agent`.

    So stop inferring "is it coming up?" from a stream that is silent exactly
    when the answer matters, and ASK THE SCREEN. ``still_loading`` is consulted
    only when the quiet budget expires, and answers one question: is the game
    somewhere other than the main menu? If it is, the click worked and the
    machine is merely slow -- extend. If the main menu is back, the click did
    nothing and no amount of waiting will change that -- give up NOW and let the
    caller retry, instead of spending the budget staring at a menu.

    That makes the wait scale with the host rather than with a constant: on a
    loaded machine map generation takes longer in wall-clock and the gate waits
    longer, with no threshold to calibrate and nothing to re-tune when the fleet
    changes. Silence still ends the wait when the screen cannot vouch for the
    game, the process dying still ends it immediately, and a hard bound of six
    times the budget still stops an endlessly chattering game from hanging the
    harness.
    """
    start = time.monotonic()
    quiet_deadline = start + seconds
    hard_deadline = start + max(seconds, 0.0) * 6.0
    while time.monotonic() < hard_deadline:
        progressed = False
        started = False
        for event in tail.poll():
            on_event(event)
            progressed = True
            if startup_event_proves_game_started(event):
                # Drain the batch before returning.  The first poll can contain
                # loaded, seat, and turn-1 state together; returning immediately
                # after loaded would advance the tail past the latter two and
                # leave the decision worker waiting on a state that was emitted.
                started = True
        if started:
            return True
        if not env.game_pids():
            return False
        if progressed:
            quiet_deadline = time.monotonic() + seconds
        elif time.monotonic() >= quiet_deadline:
            # Budget spent in silence. One look at the screen decides whether
            # that silence is a loading map or a click that missed.
            if still_loading is None or not still_loading():
                return False
            quiet_deadline = time.monotonic() + seconds
        time.sleep(2.0)
    return False


def hold_macos_awake() -> bool:
    """Keep an active live run awake even when the console is locked.

    The assertion is tied to this process, so macOS releases it automatically
    on normal exit, interruption, or a crash. It prevents idle display/system
    sleep; a MacBook's hardware lid-close sleep still pauses execution unless
    macOS is already in a supported clamshell configuration, and the run then
    resumes from its persisted files after wake.
    """
    if sys.platform != "darwin":
        return False
    try:
        subprocess.Popen(
            ["/usr/bin/caffeinate", "-dims", "-w", str(os.getpid())],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            close_fds=True,
        )
    except OSError as exc:
        print(f"[session] could not hold macOS awake: {exc}", file=sys.stderr)
        return False
    return True


def state_export_enabled(args: argparse.Namespace) -> bool:
    """Whether this run must publish an authoritative board each turn."""
    # A CIVVIS decision worker cannot make an informed decision without a state
    # event.  Keeping this derived here, where the baked mod config is made,
    # makes `--civvis-decides` self-contained instead of relying on callers to
    # remember a second, otherwise optional diagnostic flag.
    return bool(args.export_state or args.civvis_decides)


def supervised_brain_command(args: argparse.Namespace, run_dir: Path,
                             orders_db: Path, binary: Path) -> list[str]:
    """The decision worker's full command line — one builder, echoed whole by
    the launch banner.

    `--civvis-refresh-seconds` forwards as the brain's
    `--github-refresh-seconds`: the brain re-execs itself onto every
    origin/main advance at a turn boundary, so without this route a "pinned"
    batch still measures a moving program (run `civvis-20260817T160515Z`
    carried four decider revisions while its ledger row named one).
    """
    command = [
        sys.executable,
        str(REPO_ROOT / "tools" / "civ6_brain.py"),
        "--run-dir", str(run_dir),
        "--orders-db", str(orders_db),
        "--mode", "civvis",
        "--bin", str(binary),
        "--victory", args.civvis_victory,
        "--strategy", args.civvis_strategy,
        "--seconds", str(max(21600.0, args.timeout + 3600.0)),
    ]
    if args.civvis_war_from_plan:
        command.append("--war-from-plan")
    if args.civvis_refresh_seconds is not None:
        command += ["--github-refresh-seconds", str(args.civvis_refresh_seconds)]
    return command


def build_config(args: argparse.Namespace) -> dict:
    config = {
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
        # The seed rows are a diagnostic probe only. They are omitted from normal
        # runs because this build demonstrably ignores them at world generation.
        "MapSeed": args.seed if args.seed_probe else None,
        "GameSeed": args.seed if args.seed_probe else None,
        "MaxTurns": args.max_turns,
        "HumanPlayers": 1,
        # Explicit, every run, so a mode can never be inherited from whatever
        # was last configured on this installation. See GAME_MODES.
        "GameModes": {mode: mode in set(args.game_mode) for mode in GAME_MODES},
        "CityTarget": args.city_target,
        # Domination on a Duel map is the only victory reachable inside a
        # hundred-odd turns — an unassisted science win lands past turn 900 —
        # so the war knobs are exposed rather than left at their Lua defaults.
        "WarFromTurn": args.war_from_turn,
        "WarArmy": args.war_army,
        "MilitaryPerCity": args.military_per_city,
        # The fallback ladder's army row weighs the strongest MET major in
        # peacetime (below half its strength, grow by two, still under
        # ArmyCap). `losingWar` arms only after a declaration, and the wars
        # that end runs are declared on us.
        "PeaceDeterrence": args.peace_deterrence,
        "ExploreUntilTurn": args.explore_until_turn,
        # Domination on a four-civ map needs ALL THREE enemy original capitals.
        # A score victory at the turn limit needs only to be ahead, and warring
        # actively costs cities — one run went 4 cities to 3 while besieging.
        # So peace is a real strategy here, not a concession, and it is one flag.
        "MakeWar": args.make_war,
        # ★ The largest measured headroom in the project and the one lane that
        # is switched off. An oracle granted suzerainty wins 56.7% against
        # 22.7%; headless the same agent places 18.1 envoys and holds 0.71
        # suzerainties per seat against a live median of 1 and 0 over 36 runs.
        # Live turn 231 of civvis-20260803T191900Z: 56 envoys held, 0
        # suzerainties, and all four met city-states flying a rival's flag.
        "EnvoyEnabled": args.envoys,
        "EnvoyPlace": args.envoy_place,
        "EnvoyLevy": args.envoy_levy,
        "EnvoyConsider": args.envoy_consider,
        # How many units are aimed at the city plot each turn. Each MOVE_TO onto
        # an enemy city is an attack that bounces back unless it captures, so
        # this is attacks per turn — and a city heals between turns, so too few
        # attackers means a siege that never resolves.
        "AssaultWidth": args.assault_width,
        "SettlersInFlight": args.settlers_in_flight,
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
        # How many battering rams to keep alive. They are the BREACH — walls halve melee
        # damage and the city heals between turns — and they die: the deepest run built 8
        # and issued 26 positioning orders with `siege = 0` alive through a 77-turn war.
        # A support unit has no combat strength and the city bombards it, so this buys
        # concurrency rather than survival, and it needs to be tunable to find out how
        # much concurrency is enough.
        "SiegeUnits": args.siege_units,
        # One turn in N where development outranks the army outright. A ladder POSITION
        # was not enough: combat losses hold the army below its cap for the whole war, so
        # anything below the army block never gets built. 3 gives the economy a third of
        # the decisions while the army keeps the rest.
        "DevelopEvery": args.develop_every,
        # Refuse to DECLARE on a rival more than this many times our score. The strength
        # term only biases selection, which does nothing when `met` is 1 — measured: the
        # sole met rival at 2.17x our score, declared anyway, and the empire went from 3
        # cities to 1 city and 1 unit by turn 150.
        "MaxTargetRatio": args.max_target_ratio,
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
        "ExportState": state_export_enabled(args),
        # Ask every candidate inbound API what it holds, once a turn, and emit the
        # answer. Paired with `probe_channel.py`, which writes a changing nonce into
        # each sink from outside: the channel is whichever field reports the nonce
        # back. Diagnostic only, and off by default.
        "ProbeChannels": args.probe_channels,
        # ⚠⚠ WITHOUT THIS LINE THE PROBE COULD NEVER BE TURNED ON. #1098 added
        # `probeCitizenSlots` to the mod behind `cfg.ProbeCitizens`, and nothing
        # ever put that key in the config -- so `cfg.ProbeCitizens` was nil in
        # every game and the whole function was unreachable. That is the same
        # class as a ladder entry the engine has no type for: shipped, invisible,
        # and silently never running.
        #
        # It answers the one question blocking the last untouched science lane.
        # Only 8 of 50 live campus cities carry a specialist on the Campus, and
        # 45% of all specialists sit on Commercial Hubs, because CIVVIS has never
        # issued a citizen order. The probe asks `CityManager.CanStartCommand`
        # with four candidate `PARAM_MANAGE_CITIZEN` values and emits the verdict
        # WITHOUT acting, so one game settles whether the lane is actuable.
        "ProbeCitizens": args.probe_citizens,
        # ⭐ The probe answered yes, so this is the lane it opened. A citizen is
        # moved into a Campus specialist slot only where a Library already
        # stands, at most one per city, `CanStartCommand` first and the outcome
        # emitted either way. See `fillCampusSpecialists`.
        "CampusSpecialist": args.campus_specialist,
        # A SQLite file THIS process owns, offered to the mod's `DB.Query` via
        # ATTACH. If that works it is the live inbound channel the architecture
        # needs, and it is safer than the game's own `DebugGameplay.sqlite`, which
        # the game rebuilds and holds locks on.
        "OrdersDb": args.orders_db,
        # ★ CIVVIS decides; this mod actuates. With this on, the turn publishes the
        # board and then WAITS for orders on the SQLite channel instead of running
        # the hand-written heuristics. `OrdersWaitTicks` is the floor: past it the
        # built-ins run and the turn is recorded `fallback`, so a brain that is slow
        # or absent costs decision quality rather than progress.
        "CivvisDecides": args.civvis_decides,
        # Hand units CIVVIS gave no order to over to the game's own explore
        # automation. A policy, and counted separately as `explored` so it is never
        # mistaken for CIVVIS's work — see the note in the mod.
        "ExploreUnassigned": args.explore_unassigned,
        # Hand a builder to Civ 6's own automation when CIVVIS's improvement is
        # refused outright. A policy, counted as IMPROVE_AUTOMATED.
        "AutomateStuckBuilders": args.automate_stuck_builders,
        # How close a visible enemy must be before the emergency wall override
        # takes a city's queue away from CIVVIS. Damage still overrides at any
        # distance; this bounds only the "an enemy is around" half, which was
        # unbounded and fired on enemies up to fourteen tiles away.
        "EmergencyWallRadius": args.emergency_wall_radius,
        # Walk earned Great People to a legal activation plot and press Activate.
        # An actuation formality (CIVVIS banks the effect at recruit and its
        # mirror drops the walking unit); counted separately as gp_* fields.
        "GreatPeopleUse": args.great_people,
        # ⚠ Polling, not spinning. A `DB.Query` per tick pinned the game at 139% CPU
        # and starved the log flush that carries the board out, deadlocking the loop
        # on turn 2 of run civvis-20260730T110209Z. These are all counted in POLLS.
        # ⚠⚠ BOTH DEFAULT OFF: answering ENDTURN_BLOCKING_GOVERNOR_APPOINTMENT
        # segfaults the Game Core. Three runs, three maps, byte-identical faulting
        # frames at null+0x18 on the Game Core thread. Separate flags so whichever
        # mutation faults can be identified without re-running both — turning both
        # on at once is what made the envoy crash un-attributable.
        "GovernorAppoint": args.governor_appoint,
        "GovernorAssign": args.governor_assign,
        "OrdersPollTicks": args.orders_poll_ticks,
        "OrdersWaitPolls": args.orders_wait_polls,
        "OrdersFallbackPolls": args.orders_fallback_polls,
        "OrdersMaxStale": args.orders_max_stale,
        # How often the map crosses. 25 turns is fine for an after-the-fact mirror
        # and far too slow for a decision loop: newly explored ground is exactly
        # what changes where the army and the next city should go.
        "TileExportEvery": args.tile_export_every,
        "AnnouncementSeconds": args.announcement_seconds,
        "EraAnnouncementSeconds": args.era_announcement_seconds,
        # ⚠ The victory/defeat screen is the only one that states the OUTCOME, and
        # it had no clock of its own — so it took the general announcement one,
        # which the climb sets to 0.05s so popups never sit on the map the operator
        # is comparing against CIVVIS. The result a whole run exists to produce was
        # on screen for a twentieth of a second.
        "EndGameSeconds": args.end_game_seconds,
        "StartDelayFrames": args.start_delay_frames,
        "TickFrames": args.tick_frames,
    }
    # The Create Game automation selects this from the rendered, DLC-dependent
    # list. The installed config also carries it so the requested setup remains
    # explicit in the run artefacts and any usable FrontEnd context.
    if args.leader:
        config["Leader"] = args.leader
    return config


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
# How many extra startup budgets the whole bootstrap may spend on a screen that
# shows no main menu, i.e. a map that is genuinely still generating. Five at the
# 120 s default is ten minutes of patience for the slowest observed load, shared
# across every attempt rather than granted to each one.
LOADING_PATIENCE = 5

# The Create Game screen. Unlike the main menu this layout is fixed: the same
# controls in the same order every time, so these are measured once and hold.
# It is also the only route that sets difficulty *before* the game exists.
# Configuring afterwards was tried and does not work -- every menu path calls
# GameConfiguration.SetToDefaults() on entry, and reconfiguring from inside a
# running game and hosting again either loses the mod, loses the turn limit, or
# takes the application down.
START_GAME = (0.500, 0.978)
# The post-host leader introduction is a separate, verified screen.  Its
# button is stable in the half-height game window used by the live harness,
# but it must only be aimed at after the requested leader is read back from
# the screenshot; otherwise this becomes the same blind click the setup gate
# was written to prevent.
LEADER_INTRO_BEGIN = (0.394, 0.801)
# Measured in the required half-height window. 0.144 lands above the rendered
# button there, so a failed setup stayed open and the next main-menu click hit
# Choose Map Type instead.
BACK = (0.730, 0.174)
SETUP_X = 0.500
# The Start Game button sits at the very bottom of the Create Game panel,
# around 0.99 of window height — BELOW the general menu crop band (0.22..0.72),
# so the enlarged-crop recovery pass never covered it. Its label is also too
# small for a full-desktop read to hold: on 2026-08-13 Vision returned it on
# one attempt in the morning and then missed it on ~50 consecutive attempts
# all afternoon, on screenshots a human cannot tell apart, and every game
# refused to launch. The same screenshots read "Start Game" at confidence 1
# once this strip is cropped and enlarged. Left/top/right/bottom, as fractions
# of the game window.
START_GAME_STRIP = (0.25, 0.86, 0.80, 1.0)

# The civilization control is one fixed Firaxis setup row above difficulty.
# Expressing that relationship keeps it aligned when a different window height
# changes the normalized coordinate (measured at 0.277 on 1474x949 and 0.294 on
# the taller full-screen setup).
LEADER_PICKER_OFFSET = 0.056
# ⚠ RETUNED 2026-08-03 when scrolling started working at all — see
# `macos_input.scroll`, which was emitting line-unit events Civilization VI
# ignores. These are wheel NOTCHES now, and they are measured, not guessed:
#
#   20 notches up      returns the list to `Random Leader` from anywhere in it
#    2 notches down    advances ~10 rows against ~10 visible: page by page
#    3 notches down    advances ~11 against 10 visible: it SKIPS rows
#
# At 2 the sweep saw 73 distinct rows in 6 steps — the whole installed roster —
# and found Trajan. `STEPS` stays generous because overshooting the bottom costs
# one wasted screenshot and undershooting costs the run.
LEADER_SCROLL_STEPS = 40
LEADER_SCROLL_RESET = 20
LEADER_SCROLL_AMOUNT = -2

# Each dropdown's closed box, as a fraction of window height.
DROPDOWN = {
    # The Create Game panel reflows in the required 756x480 quadrant. These
    # are the observed value-row centers in that window; the old tall-window
    # ratios landed 5-10 pixels above each closed control.
    "difficulty": 0.3520,
    "speed": 0.4040,
    "map_type": 0.4600,
    "map_size": 0.5150,
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


def screen_locked() -> bool:
    """Return whether the active macOS console session is locked."""
    try:
        result = subprocess.run(
            ["ioreg", "-n", "Root", "-d1"],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        return False
    return 'CGSSessionScreenIsLocked"=Yes' in result.stdout


def wait_for_unlocked_session(poll_s: float = 2.0) -> None:
    """Wait at the macOS authentication boundary instead of aborting the run.

    GUI scripting cannot operate the protected lock screen.  Waiting here keeps
    the requested run alive without trying to bypass that boundary, and lets a
    launch requested while locked continue as soon as the operator unlocks.
    """
    if not screen_locked():
        return
    print("[session] macOS is locked; waiting to continue after unlock", flush=True)
    while screen_locked():
        time.sleep(poll_s)
    print("[session] macOS unlocked; continuing", flush=True)


# Which half of the screen the game gets. Set from --window-side/--window-frac
# in main; module-level so the focus helpers do not need threading through every
# call site.
GAME_SIDE = "left"
GAME_FRACTION = 0.5
GAME_VFRACTION = 1.0


def desktop_size() -> tuple[int, int] | None:
    """Logical size of the MAIN display in points, or None if unreadable.

    ⚠⚠ NOT the desktop's total area. This asked Finder for its desktop scroll
    area, which spans EVERY attached display — and on 2026-08-04 an external
    2560x1440 monitor was plugged in beside the built-in Retina. Finder then
    reported **3225x2557**, the union. `place_game` halved that, placed Civ 6
    1612 points wide at y=1333 (below a 1117-point screen), and the setup vision
    could no longer read the difficulty dropdown:

        [setup] difficulty: current value was not readable (attempt 1)
        [setup] difficulty: refusing to click an unverified coordinate
        NO GAME -- could not start a game from the main menu

    Every attempt in the batch failed that way. The old docstring warned that
    mixing coordinate spaces "would place the game off-screen"; the same hazard
    arrived through a second display rather than through DPI.

    `NSScreen` answers for one screen, which is the quantity `place_game` needs.
    The screen at origin (0,0) is the one holding the menu bar; `NSScreen.main`
    is the fallback and follows the key window, so it is second choice.
    """
    swift = textwrap.dedent("""
        import AppKit
        if let s = NSScreen.screens.first(where: { $0.frame.origin == .zero })
                    ?? NSScreen.main {
            print("\\(Int(s.frame.width)),\\(Int(s.frame.height))")
        }
    """)
    try:
        out = subprocess.run(["swift", "-"], input=swift,
                             capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.SubprocessError):
        return None
    parts = [p.strip() for p in out.stdout.split(",") if p.strip()]
    if len(parts) != 2 or not all(p.isdigit() for p in parts):
        return None
    width, height = (int(p) for p in parts)
    # ⚠ Keep the sanity floor AND add a ceiling, as the last line of defence if
    # the source ever reports a display UNION again. The bound is principled
    # rather than arbitrary: the largest Apple display is a Pro Display XDR at
    # 6016x3384 pixels, which is **3008x1692 POINTS**, and window geometry is in
    # points. Anything taller than ~1700 points is therefore two screens stacked,
    # not one screen — the observed union was 3225x2557.
    if not (800 < width <= 4000 and 600 < height <= 2000):
        return None
    return (width, height)


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
    # "bottomright" anchors the window to the screen's bottom-right corner —
    # the operator's 2026-08-01 layout: CIVVIS holds the upper left at 2/3 of
    # the diagonal, the real game the LOWER right at the same, overlapping in
    # the middle with the game in front where they cross. The top-anchored
    # sides keep their old meaning exactly.
    if side == "bottomright":
        x, y = screen_w - width, screen_h - height
    else:
        x, y = (0 if side == "left" else screen_w - width), menu
    script = (
        'tell application "System Events" to tell '
        '(first process whose name contains "Civ6") to tell window 1\n'
        f'  set size to {{{width}, {height}}}\n'
        # Aspyr constrains the existing origin while applying the smaller size.
        # Position last or a requested upper quadrant lands at the bottom.
        f'  set position to {{{x}, {y}}}\n'
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
    #
    # ★★★★★ AND THE PRESS MUST BE HELD. `cliclick c:` sends down and up in the same
    # instant and Civilization VI's leader-dialogue buttons do not act on it.
    #
    # Measured live at 04:29 on 2026-07-31, attempt 6 stalled at turn 66 on a
    # three-option `DiplomacyActionView` from Pericles. Everything checkable checked
    # out and the screen would not close: the window rect the harness read (864, 33,
    # 864, 542) matches the screenshot exactly, the three buttons sit at fy 0.827,
    # 0.869 and 0.913 which the 0.02-step sweep straddles within ~5 points, and
    # `cliclick p` afterwards reported the pointer at the last swept position — so the
    # move was delivered and Accessibility permission was granted. Three full passes,
    # sixty clicks, nothing.
    #
    # One `dd:` / wait / `du:` on the same pixel, by hand, and the turn advanced
    # immediately. Stalls have been the dominant way runs end in this project
    # (t87, t95, t106, t142, t179, t199), and a zero-length click is why the rescue
    # that "already works" so often did not.
    #
    # ⚠ n = 1, and the harness's own stall watchdog had not fired yet when the held
    # press landed, so nothing else can account for the recovery. Worth re-checking on
    # the next stuck screen before treating it as settled.
    macos_input.move(px, py)
    time.sleep(0.5)
    macos_input.click(px, py, hold_s=0.12)


def click_menu(item: str, bounds: tuple[int, int, int, int]) -> None:
    x, y, w, h = bounds
    fx, fy = MENU[item]
    # Exactly once. A second click on the same entry closes the submenu the
    # first one opened, which reads as "the click did nothing" and then sends
    # the follow-up click, aimed at the submenu, into whatever main-menu row
    # sits at that height. That cost four failed bootstrap attempts.
    click_at(int(x + w * fx), int(y + h * fy))


def screenshot(path: Path) -> bool:
    """Keep a picture of the screen. A misclick is a visual failure and the
    log cannot describe it; the shot is what says which row was hit.

    ⚠ `screencapture` can fail SILENTLY under machine load and write nothing —
    three consecutive ladder attempts died on 2026-08-17 (17:46–18:29Z, a
    window of heavy concurrent `cargo test` runs) because a missing shot
    cascaded through a PIL `FileNotFoundError` into the native-OCR fallback,
    which raised `OCRUnavailable` ("zero-dimensioned image") that no setup
    caller catches. The capture is verified and retried once here, at the
    source, so every caller inherits the cover; a shot that still fails is
    reported loudly and the caller's own "not readable this attempt" retry
    handles it as an ordinary unreadable poll.
    """
    for attempt in (1, 2):
        subprocess.run(["screencapture", "-x", "-t", "png", str(path)],
                       capture_output=True)
        try:
            if path.stat().st_size > 0:
                return True
        except OSError:
            pass
        if attempt == 1:
            time.sleep(1.0)
    print(f"[shot] screencapture wrote nothing for {path.name} after a retry; "
          "treating this poll as unreadable", flush=True)
    return False


def option_strip(bounds: tuple[int, int, int, int], name: str) -> tuple[int, int, int, int]:
    """The screen rectangle an open list covers, in physical pixels.

    Physical, not logical: `screencapture` works in device pixels and this Mac is
    2x, so a rectangle in points addresses a quarter of the intended area.
    """
    x, y, w, h = bounds
    box = DROPDOWN[name]
    # ⚠ Narrow, and only as tall as THIS list. A strip the full width of the window
    # is 90% background: the list is about a tenth of it across, so an open list
    # moved only ~5% of the rectangle and read as closed. Measured against a real
    # open map-size list before this was tightened.
    rows = len(OPTIONS[name])
    top = y + h * (box + OPTION_FIRST * 0.5)
    bottom = y + h * (box + OPTION_FIRST + (rows - 1) * OPTION_STEP)
    left = x + w * (SETUP_X - 0.08)
    right = x + w * (SETUP_X + 0.08)
    return (int(left * 2), int(top * 2), int(right * 2), int(bottom * 2))


def list_opened(before: Path, after: Path, rect: tuple[int, int, int, int]) -> bool:
    """Did clicking the box actually drop a list over the rows beneath it?

    An open list repaints that whole strip, so the two shots differ across a
    large share of it. A closed one leaves the Create Game rows exactly as they
    were — the menu's animated background is *behind* the panel and does not
    reach here, which is why a plain difference is enough.
    """
    try:
        from PIL import Image, ImageChops
    except ImportError:
        return True  # cannot look; fall back to the old blind behaviour
    try:
        a = Image.open(before).convert("L").crop(rect)
        b = Image.open(after).convert("L").crop(rect)
    except Exception:
        return True
    diff = ImageChops.difference(a, b)
    changed = sum(1 for v in diff.getdata() if v > 24)
    return changed > 0.10 * (diff.size[0] * diff.size[1])


def _setup_option_label(value: str) -> str:
    """Return the English value Firaxis renders in a setup dropdown."""
    label = value.removesuffix(".lua")
    for prefix in ("DIFFICULTY_", "GAMESPEED_", "MAPSIZE_"):
        label = label.removeprefix(prefix)
    return label.replace("_", " ").title()


# The heading Civilization VI renders directly above each setup dropdown.  These
# are what a row is identified BY; see `_setup_current_value`.
SETUP_HEADINGS = {
    "difficulty": "Choose Game Difficulty",
    "speed": "Choose Game Speed",
    "map_type": "Choose Map Type",
    "map_size": "Choose Map Size",
}

# Top-to-bottom order of the rows in the Create Game panel.  Used to place a heading
# an open dropdown is covering; see `_setup_rows`.
SETUP_ROW_ORDER = ["difficulty", "speed", "map_type", "map_size"]

# Fallback only, for a panel whose headings did not survive OCR.  ⚠ These bands
# OVERLAP each other and cannot separate the rows on their own -- that is the whole
# reason they are no longer the primary mechanism.
SETUP_BANDS = {
    "difficulty": (0.20, 0.38),
    "speed": (0.24, 0.44),
    "map_type": (0.28, 0.50),
    "map_size": (0.34, 0.56),
}

# The values occupy the narrow central setup column.  Rejects an identical word from
# the news artwork or another desktop window.
SETUP_COLUMN = (0.38, 0.62)


def _setup_rows(observations: list[dict], bounds: tuple[int, int, int, int],
                screen: tuple[int, int]) -> dict[str, int]:
    """Screen y of each setup heading that is legible in this shot."""
    screen_w, screen_h = screen
    x, y, w, h = bounds
    rows: dict[str, int] = {}
    for observation in observations:
        text = str(observation.get("text", ""))
        for name, heading in SETUP_HEADINGS.items():
            if name in rows or not _menu_label_matches(text, heading):
                continue
            point = _observation_point(observation)
            if point is None:
                continue
            px, py = int(point[0] * screen_w), int(point[1] * screen_h)
            if x <= px <= x + w and y <= py <= y + h:
                rows[name] = py
    return _with_covered_headings(rows)


def _with_covered_headings(rows: dict[str, int]) -> dict[str, int]:
    """Place the headings an open dropdown is sitting on top of.

    An open list covers the rows beneath it, so a shot taken while one is down reads
    only the headings above it -- on a live map-size list, `Choose Map Type` and
    `Choose Map Size` both vanished and only difficulty and speed came back.  Without
    this the reader would fall through to the overlapping bands, which is exactly the
    mechanism that read the speed row as the map size in the first place.

    The panel is a fixed, evenly spaced column, so a covered heading is not a guess:
    fit a line through the ones that ARE legible against their row index and read off
    the missing one.  Measured on `civvis-20260802T131519Z`, difficulty=205 and
    speed=234 give a pitch of 29, predicting map_size (two rows below speed) at 292 --
    which is where it actually is when nothing covers it.

    Needs two legible headings to establish a pitch; with fewer, the caller keeps its
    band fallback rather than inventing a scale from one point.
    """
    known = [(SETUP_ROW_ORDER.index(name), py)
             for name, py in rows.items() if name in SETUP_ROW_ORDER]
    if len(known) < 2 or len(known) == len(SETUP_ROW_ORDER):
        return rows
    known.sort()
    first, last = known[0], known[-1]
    span = last[0] - first[0]
    if span <= 0:
        return rows
    pitch = (last[1] - first[1]) / span
    if pitch <= 0:
        # Rows must descend down the screen.  Anything else means these are not the
        # panel's headings, and extrapolating from them would be worse than nothing.
        return rows
    filled = dict(rows)
    for index, name in enumerate(SETUP_ROW_ORDER):
        if name not in filled:
            filled[name] = int(round(first[1] + (index - first[0]) * pitch))
    return filled


def _setup_current_value(path: Path, bounds: tuple[int, int, int, int],
                         name: str) -> tuple[str, tuple[int, int]] | None:
    """Read a closed setup dropdown's value and exact screen position.

    ★★★★★ THE ROW IS IDENTIFIED BY ITS HEADING, NOT BY WHERE IT SITS.

    This used to pick the first option-shaped word inside a fixed vertical band, and
    the bands OVERLAP -- `map_size` spanned 0.34-0.56 while `speed` spanned
    0.24-0.44.  That is only survivable while no value is legal for two rows, and
    "Standard" is legal for both.  Measured on a live Create Game screen
    (`civvis-20260802T131519Z`, game window 864x528 logical points), three matches
    fell inside the map-size band at once:

        Standard    ry=0.377   <- the GAME SPEED row, and the one that won
        Continents  ry=0.432   <- the MAP TYPE row, skipped: not a map size
        Small       ry=0.487   <- the actual MAP SIZE row, never reached

    So `map_size` read `MAPSIZE_STANDARD` off a screen that plainly said Small.
    `set_dropdown` then clicked the speed row to "fix" it, never read Small back, and
    gave up with "refusing to start an unverified game" -- on a game that was already
    configured exactly as asked.  Every attempt in every batch on 2026-08-02 died
    this way: 0 games from 4 ledger rows, while the ledger's own screenshots show a
    correct panel.  ⚠ The instrument was wrong; the game was right.  A wider band or
    a nudged constant would have moved the collision, not removed it.

    Firaxis renders a heading directly above each dropdown, so a row can be named
    instead of located: find "Choose Map Size", then take the option between it and
    whichever heading comes next.  That survives the panel reflowing, the window
    being resized, and two rows sharing a value -- none of which a constant can.
    """
    screen = desktop_size()
    if screen is None:
        return None
    screen_w, screen_h = screen
    x, y, w, h = bounds
    allowed = {
        _normalized_label(_setup_option_label(option)): option
        for option in OPTIONS[name]
    }
    observations = macos_ocr.recognize(path)
    observations.extend(_menu_crop_ocr(path, bounds))

    left, right = SETUP_COLUMN
    candidates: list[tuple[int, str, tuple[int, int]]] = []
    for observation in observations:
        option = allowed.get(_normalized_label(str(observation.get("text", ""))))
        if option is None:
            continue
        point = _observation_point(observation)
        if point is None:
            continue
        px, py = int(point[0] * screen_w), int(point[1] * screen_h)
        if not left <= (px - x) / w <= right:
            continue
        candidates.append((py, option, (px, py)))
    if not candidates:
        return None
    candidates.sort()

    rows = _setup_rows(observations, bounds, screen)
    heading_y = rows.get(name)
    if heading_y is not None:
        # The row owns the strip between its own heading and the next one down.
        below = [row_y for row_y in rows.values() if row_y > heading_y]
        limit = min(below) if below else None
        for py, option, point in candidates:
            if py <= heading_y:
                continue
            if limit is not None and py >= limit:
                break
            return option, point
        # The heading was legible and nothing sits under it. Say nothing rather than
        # fall through to the bands: the caller retries and then refuses, which is
        # the correct outcome, and a band guess here is exactly the wrong answer that
        # cost every game above.
        return None

    # No heading survived OCR. Fall back to the historical band, which is better than
    # nothing on a panel this reader cannot otherwise locate at all.
    top, bottom = SETUP_BANDS[name]
    for py, option, point in candidates:
        if top <= (py - y) / h <= bottom:
            return option, point
    return None


def set_dropdown(bounds: tuple[int, int, int, int], name: str, value: str,
                 run_dir: Path | None = None) -> bool:
    """Select and read back one visible Create Game dropdown value.

    The setup panel reflowed in a live 756x480 window: the rendered Prince box
    was at y=167 while the old measured ratio clicked y=202.  A pixel-difference
    test correctly said the list had not opened, but retrying the same stale
    coordinate could never recover.  Read the current value, click that exact
    rendered row, then read and click the requested option from the opened list.
    No option position is assumed, and success requires a final OCR readback.
    """
    if value not in OPTIONS[name]:
        return False
    shots = run_dir if run_dir is not None else Path(tempfile.gettempdir())
    before = shots / f"dropdown-{name}-closed.png"
    after = shots / f"dropdown-{name}-open.png"
    verified = shots / f"dropdown-{name}-selected.png"

    for attempt in (1, 2):
        screenshot(before)
        current = _setup_current_value(before, bounds, name)
        if current is None:
            print(f"[setup] {name}: current value was not readable (attempt {attempt})",
                  flush=True)
            continue
        current_value, current_point = current
        if current_value == value:
            print(f"[setup] {name}: already verified {_setup_option_label(value)}",
                  flush=True)
            return True

        click_at(*current_point)
        time.sleep(1.2)
        screenshot(after)
        target = _observed_label_point(after, _setup_option_label(value), bounds)
        if target is None:
            print(f"[setup] {name}: requested option was not visible (attempt {attempt})",
                  flush=True)
            continue

        click_at(*target)
        time.sleep(1.2)
        screenshot(verified)
        selected = _setup_current_value(verified, bounds, name)
        if selected is not None and selected[0] == value:
            print(f"[setup] {name}: selected and verified {_setup_option_label(value)}",
                  flush=True)
            return True
        print(f"[setup] {name}: selection did not read back (attempt {attempt})",
              flush=True)

    print(f"[setup] {name}: refusing to click an unverified coordinate", flush=True)
    return False


def _normalized_label(value: str) -> str:
    ascii_value = unicodedata.normalize("NFKD", value).encode("ascii", "ignore").decode()
    return "".join(character for character in ascii_value.casefold() if character.isalnum())


def _menu_label_matches(observed: str, wanted: str) -> bool:
    """Tolerate a small OCR edit distance in a long menu label.

    In the required 756-point-wide game quadrant Vision has returned both
    ``Single Plaver`` (one substitution) and ``Single Plave`` (a substitution
    plus a missing final glyph) for ``Single Player``.  Menu labels remain
    constrained to the game window, so two edits in a ten-character label are
    conservative enough to recover the visible row without a coordinate guess.
    """
    observed = _normalized_label(observed)
    wanted = _normalized_label(wanted)
    if observed == wanted:
        return True
    if len(wanted) < 10 or abs(len(observed) - len(wanted)) > 2:
        return False
    previous = list(range(len(wanted) + 1))
    for row, left in enumerate(observed, 1):
        current = [row]
        for column, right in enumerate(wanted, 1):
            current.append(min(
                current[-1] + 1,
                previous[column] + 1,
                previous[column - 1] + (left != right),
            ))
        previous = current
    return previous[-1] <= 2


def leader_display_name(leader: str) -> str:
    """Resolve a Firaxis leader type to the label CIVVIS expects on the picker."""
    requested = _normalized_label(leader.removeprefix("LEADER_"))
    try:
        roster = json.loads((REPO_ROOT / "data" / "civs.json").read_text())
    except (OSError, json.JSONDecodeError):
        roster = {}
    for civilization in roster.values() if isinstance(roster, dict) else ():
        name = civilization.get("leader") if isinstance(civilization, dict) else None
        if isinstance(name, str) and _normalized_label(name) == requested:
            return name
    return leader.removeprefix("LEADER_").replace("_", " ").title()


def _setup_current_leader(path: Path, bounds: tuple[int, int, int, int]
                          ) -> tuple[str, tuple[int, int]] | None:
    """Read the current civilization-picker value and its screen position."""
    screen = desktop_size()
    if screen is None:
        return None
    labels = {"randomleader": "Random Leader"}
    try:
        roster = json.loads((REPO_ROOT / "data" / "civs.json").read_text())
    except (OSError, json.JSONDecodeError):
        roster = {}
    for civilization in roster.values() if isinstance(roster, dict) else ():
        label = civilization.get("leader") if isinstance(civilization, dict) else None
        if isinstance(label, str):
            labels[_normalized_label(label)] = label

    screen_w, screen_h = screen
    x, y, w, h = bounds
    observations = macos_ocr.recognize(path)
    observations.extend(_menu_crop_ocr(path, bounds))
    headings: dict[str, int] = {}
    for observation in observations:
        point = _observation_point(observation)
        if point is None:
            continue
        px, py = int(point[0] * screen_w), int(point[1] * screen_h)
        if not (x <= px <= x + w and y <= py <= y + h):
            continue
        text = str(observation.get("text", ""))
        if _menu_label_matches(text, "Choose Civilization"):
            headings["civilization"] = py
        elif _menu_label_matches(text, SETUP_HEADINGS["difficulty"]):
            headings["difficulty"] = py

    for observation in observations:
        label = labels.get(_normalized_label(str(observation.get("text", ""))))
        if label is None:
            continue
        point = _observation_point(observation)
        if point is None:
            continue
        px, py = int(point[0] * screen_w), int(point[1] * screen_h)
        rx, ry = (px - x) / w, (py - y) / h
        civilization_heading = headings.get("civilization")
        difficulty_heading = headings.get("difficulty")
        in_civilization_row = (
            civilization_heading is not None
            and difficulty_heading is not None
            and civilization_heading < py < difficulty_heading
        )
        # A full-height window vertically centres the Create Game panel lower
        # than the half-height layout.  Its rendered leader row was at 0.361
        # of the window in the 2026-08-16 launch, just below the historical
        # fallback band.  When both surrounding headings are visible, their
        # interval names this exact row without relaxing the safety guard.
        if 0.38 <= rx <= 0.62 and (
                in_civilization_row or 0.12 <= ry <= 0.34):
            return label, (px, py)
    return None


def _observation_point(observation: dict) -> tuple[float, float] | None:
    try:
        return (
            float(observation["x"]) + float(observation["width"]) / 2.0,
            float(observation["y"]) + float(observation["height"]) / 2.0,
        )
    except (KeyError, TypeError, ValueError):
        return None


def _menu_crop_ocr(path: Path, bounds: tuple[int, int, int, int],
                   strip: tuple[float, float, float, float] = (0.18, 0.22, 0.82, 0.72),
                   tag: str = "menu") -> list[dict]:
    """Read the small main-menu columns from an enlarged crop.

    At 756x480 Vision can read the main row inconsistently and omit every
    Single Player submenu row from a full desktop capture.  The pixels are
    present: cropping the two menu columns and enlarging them is the same
    recovery already required by the DLC-dependent leader picker below.
    Returned boxes are mapped back to full-desktop normalized coordinates.

    ``strip`` is the window region to crop (left/top/right/bottom fractions);
    the default is the band the main-menu columns occupy. ``tag`` keeps each
    caller's debug crop distinguishable on disk.
    """
    try:
        from PIL import Image

        screen = desktop_size()
        if screen is None:
            return []
        image = Image.open(path)
        screen_w, screen_h = screen
        x, y, w, h = bounds
        scale_x, scale_y = image.width / screen_w, image.height / screen_h
        left_f, top_f, right_f, bottom_f = strip
        rect = (
            max(0, int((x + w * left_f) * scale_x)),
            max(0, int((y + h * top_f) * scale_y)),
            min(image.width, int((x + w * right_f) * scale_x)),
            min(image.height, int((y + h * bottom_f) * scale_y)),
        )
        if rect[2] <= rect[0] or rect[3] <= rect[1]:
            return []
        crop = image.crop(rect)
        crop = crop.resize((crop.width * 4, crop.height * 4))
        crop_path = path.with_name(f"{path.stem}-{tag}-crop.png")
        crop.save(crop_path)
        observations = macos_ocr.recognize(crop_path)
    except (OSError, ValueError):
        return []

    left, crop_top, right, crop_bottom = rect
    crop_w, crop_h = right - left, crop_bottom - crop_top
    mapped = []
    for observation in observations:
        item = dict(observation)
        try:
            item["x"] = (left + float(observation["x"]) * crop_w) / image.width
            item["y"] = (crop_top + float(observation["y"]) * crop_h) / image.height
            item["width"] = float(observation["width"]) * crop_w / image.width
            item["height"] = float(observation["height"]) * crop_h / image.height
        except (KeyError, TypeError, ValueError):
            continue
        mapped.append(item)
    return mapped


def _leader_observation(observations: list[dict], label: str,
                        bounds: tuple[int, int, int, int],
                        *, selected: bool = False) -> dict | None:
    screen = desktop_size()
    if screen is None:
        return None
    screen_w, screen_h = screen
    x, y, w, h = bounds
    wanted = _normalized_label(label)
    for observation in observations:
        if _normalized_label(str(observation.get("text", ""))) != wanted:
            continue
        point = _observation_point(observation)
        if point is None:
            continue
        px, py = point[0] * screen_w, point[1] * screen_h
        rx, ry = (px - x) / w, (py - y) / h
        if 0.40 <= rx <= 0.62 and (
            0.23 <= ry <= 0.31 if selected else 0.30 <= ry <= 0.76
        ):
            return observation
    return None


def _leader_ocr(path: Path, bounds: tuple[int, int, int, int],
                *, top: float = 0.30, bottom: float = 0.76) -> list[dict]:
    """Recognize the narrow leader column at readable scale.

    Vision skipped Jadwiga in the full 3024x1964 desktop capture even though
    it read the adjacent Hojo and Jayavarman rows. A 4x crop of the same pixels
    reads it consistently. Map the normalized crop observations back into full
    desktop coordinates so the existing click validation remains unchanged.
    """
    try:
        from PIL import Image

        screen = desktop_size()
        if screen is None:
            raise ValueError("desktop size unavailable")
        image = Image.open(path)
        screen_w, screen_h = screen
        x, y, w, h = bounds
        scale_x, scale_y = image.width / screen_w, image.height / screen_h
        rect = (
            int((x + w * 0.40) * scale_x),
            int((y + h * top) * scale_y),
            int((x + w * 0.62) * scale_x),
            int((y + h * bottom) * scale_y),
        )
        crop = image.crop(rect)
        crop = crop.resize((crop.width * 4, crop.height * 4))
        crop_path = path.with_name(path.stem + "-leader-crop.png")
        crop.save(crop_path)
        observations = macos_ocr.recognize(crop_path)
    except (OSError, ValueError):
        # A shot that never landed has nothing to read at any resolution —
        # the fallback would hand the native OCR a missing file and die on
        # `OCRUnavailable`. An empty read lets the caller's own retry poll.
        if not path.exists():
            return []
        return macos_ocr.recognize(path)

    left, crop_top, right, crop_bottom = rect
    crop_w, crop_h = right - left, crop_bottom - crop_top
    mapped = []
    for observation in observations:
        item = dict(observation)
        try:
            item["x"] = (left + float(observation["x"]) * crop_w) / image.width
            item["y"] = (crop_top + float(observation["y"]) * crop_h) / image.height
            item["width"] = float(observation["width"]) * crop_w / image.width
            item["height"] = float(observation["height"]) * crop_h / image.height
        except (KeyError, TypeError, ValueError):
            continue
        mapped.append(item)
    return mapped


def _leader_picker_open(path: Path, bounds: tuple[int, int, int, int]) -> bool:
    """Prove the picker is open by reading a real roster entry from it."""
    try:
        roster = json.loads((REPO_ROOT / "data" / "civs.json").read_text())
    except (OSError, json.JSONDecodeError):
        roster = {}
    names = {
        _normalized_label(civilization.get("leader", ""))
        for civilization in roster.values() if isinstance(civilization, dict)
    }
    names.discard("")
    return any(
        _normalized_label(str(observation.get("text", ""))) in names
        for observation in _leader_ocr(path, bounds, top=0.15, bottom=0.80)
    )


def _leader_intro_visible(path: Path, bounds: tuple[int, int, int, int],
                          leader: str | None) -> bool:
    """Prove that the requested leader introduction is on screen.

    Civ VI pauses at a leader card after the Create Game click.  The control
    mod is already loaded there, so its auto-close records are not proof that
    the map is ready.  OCR of the requested leader alone is insufficient: the
    Create Game page also displays that leader.  Require the intro's rendered
    ``BEGIN GAME`` control in its lower card band before using the fixed click.
    """
    if not leader:
        return False
    screen = desktop_size()
    if screen is None:
        return False
    wanted = _normalized_label(leader_display_name(leader))
    screen_w, screen_h = screen
    x, y, w, h = bounds
    try:
        observations = macos_ocr.recognize(path)
        observations.extend(_leader_ocr(path, bounds, top=0.08, bottom=0.45))
        button_observations = _leader_intro_button_ocr(path, bounds)
    except (OSError, ValueError, macos_ocr.OCRUnavailable):
        # `OCRUnavailable` included: a missing or zero-dimensioned shot is an
        # unreadable poll, not a reason to end the attempt.
        return False
    begin_game = False
    for observation in button_observations:
        if _normalized_label(str(observation.get("text", ""))) != "begingame":
            continue
        point = _observation_point(observation)
        if point is None:
            continue
        px, py = point[0] * screen_w, point[1] * screen_h
        rx, ry = (px - x) / w, (py - y) / h
        if 0.30 <= rx <= 0.70 and 0.65 <= ry <= 0.90:
            begin_game = True
            break
    if not begin_game:
        return False
    for observation in observations:
        if _normalized_label(str(observation.get("text", ""))) != wanted:
            continue
        point = _observation_point(observation)
        if point is None:
            continue
        px, py = point[0] * screen_w, point[1] * screen_h
        rx, ry = (px - x) / w, (py - y) / h
        if 0.20 <= rx <= 0.80 and 0.05 <= ry <= 0.45:
            return True
    return False


def _leader_intro_button_ocr(path: Path,
                             bounds: tuple[int, int, int, int]) -> list[dict]:
    """Read the small Begin Game button from an enlarged lower-card crop.

    Full-desktop Vision sees the leader card text but often drops this small
    button.  The Create Game page's similarly placed ``Start Game`` control is
    intentionally not accepted: the exact label is the page discriminator.
    """
    try:
        from PIL import Image

        screen = desktop_size()
        if screen is None:
            raise ValueError("desktop size unavailable")
        image = Image.open(path)
        screen_w, screen_h = screen
        x, y, w, h = bounds
        scale_x, scale_y = image.width / screen_w, image.height / screen_h
        rect = (
            int((x + w * 0.30) * scale_x),
            int((y + h * 0.68) * scale_y),
            int((x + w * 0.70) * scale_x),
            int((y + h * 0.88) * scale_y),
        )
        crop = image.crop(rect)
        crop = crop.resize((crop.width * 8, crop.height * 8))
        crop_path = path.with_name(path.stem + "-begin-game-crop.png")
        crop.save(crop_path)
        observations = macos_ocr.recognize(crop_path)
    except (OSError, ValueError):
        return []

    left, crop_top, right, crop_bottom = rect
    crop_w, crop_h = right - left, crop_bottom - crop_top
    mapped = []
    for observation in observations:
        item = dict(observation)
        try:
            item["x"] = (left + float(observation["x"]) * crop_w) / image.width
            item["y"] = (crop_top + float(observation["y"]) * crop_h) / image.height
            item["width"] = float(observation["width"]) * crop_w / image.width
            item["height"] = float(observation["height"]) * crop_h / image.height
        except (KeyError, TypeError, ValueError):
            continue
        mapped.append(item)
    return mapped


def advance_leader_intro(bounds: tuple[int, int, int, int],
                         leader: str | None, run_dir: Path, attempt: int,
                         *, retries: int = 4, poll_s: float = 1.0,
                         board_ready=None) -> bool:
    """Click the leader card's Begin Game control after visual confirmation.

    ``board_ready`` is checked only after the screen has failed the exact intro
    proof.  It therefore cannot bypass a rendered leader card just because the
    in-game agent loaded behind it, but it can end the remaining probe budget
    when a direct host transition has already opened the board.
    """
    x, y, w, h = bounds
    for retry in range(retries):
        shot = run_dir / f"leader-intro-attempt{attempt}-{retry}.png"
        screenshot(shot)
        if _leader_intro_visible(shot, bounds, leader):
            click_at(int(x + w * LEADER_INTRO_BEGIN[0]),
                     int(y + h * LEADER_INTRO_BEGIN[1]))
            print(f"[setup] verified {leader_display_name(leader or '')} intro; "
                  "clicked Begin Game", flush=True)
            return True
        if board_ready is not None and board_ready():
            print("[setup] live board arrived before a leader intro was visible; "
                  "stopping redundant intro probes", flush=True)
            return False
        time.sleep(poll_s)
    return False


def select_requested_leader(bounds: tuple[int, int, int, int], leader: str | None,
                            run_dir: Path) -> bool:
    """Select and visually verify a leader from Firaxis's DLC-dependent list."""
    if leader is None:
        return True
    label = leader_display_name(leader)
    x, y, w, h = bounds
    closed_shot = run_dir / "leader-picker-closed.png"
    open_shot = run_dir / "leader-picker-open.png"

    for attempt in (1, 2):
        screenshot(closed_shot)
        current = _setup_current_leader(closed_shot, bounds)
        if current is None:
            print(f"[setup] leader value was not readable (attempt {attempt})",
                  flush=True)
            continue
        current_label, current_point = current
        if _normalized_label(current_label) == _normalized_label(label):
            print(f"[setup] leader: already verified {label} ({leader})", flush=True)
            return True
        click_at(*current_point)
        time.sleep(1.2)
        screenshot(open_shot)
        if _leader_picker_open(open_shot, bounds):
            break
        print(f"[setup] leader list did not open (attempt {attempt})", flush=True)
    else:
        return False

    # Firaxis retains the list's scroll position between openings. Reset to A
    # first; otherwise a retry can begin at Victoria and never encounter
    # Jadwiga. Thirteen -30 wheel ticks only reached Harald on this install, so
    # retain that overlapping step but cover the entire installed roster.
    macos_input.move(int(x + w * SETUP_X), int(y + h * 0.55))
    macos_input.scroll(LEADER_SCROLL_RESET)
    time.sleep(1.0)
    for scroll_step in range(LEADER_SCROLL_STEPS):
        shot = run_dir / f"leader-picker-{scroll_step:02d}.png"
        screenshot(shot)
        observations = _leader_ocr(shot, bounds)
        match = _leader_observation(observations, label, bounds)
        if match is not None:
            point = _observation_point(match)
            screen = desktop_size()
            if point is None or screen is None:
                return False
            # Use the stable centre of the picker column and only OCR's row.
            click_at(int(x + w * SETUP_X), int(point[1] * screen[1]))
            time.sleep(1.2)
            selected_shot = run_dir / "leader-selected.png"
            screenshot(selected_shot)
            selected = _leader_ocr(selected_shot, bounds, top=0.20, bottom=0.36)
            if _leader_observation(selected, label, bounds, selected=True) is not None:
                print(f"[setup] leader: selected and verified {label} ({leader})", flush=True)
                return True
            print(f"[setup] leader click did not select {label}", flush=True)
            return False
        macos_input.move(int(x + w * SETUP_X), int(y + h * 0.55))
        macos_input.scroll(LEADER_SCROLL_AMOUNT)
        time.sleep(0.8)

    press_escape(1)
    print(f"[setup] requested leader {label} ({leader}) was not in the picker", flush=True)
    return False


def _main_menu_visible(path: Path) -> bool:
    """Return whether a screenshot visibly contains Firaxis's Single Player row."""
    return any(
        _menu_label_matches(str(observation.get("text", "")), "Single Player")
        for observation in macos_ocr.recognize(path)
    )


def _observed_label_point(path: Path, label: str,
                          bounds: tuple[int, int, int, int],
                          strip: tuple[float, float, float, float] | None = None,
                          ) -> tuple[int, int] | None:
    """Read a visible menu label center inside the game window."""
    points = _observed_label_points(path, label, bounds, strip=strip)
    return points[0] if points else None


def _observed_label_points(path: Path, label: str,
                           bounds: tuple[int, int, int, int],
                           strip: tuple[float, float, float, float] | None = None,
                           ) -> list[tuple[int, int]]:
    """Read every matching visible label center inside the game window.

    ``strip`` names one more window region (left/top/right/bottom fractions) to
    crop and enlarge when both standard passes miss — for labels that live
    outside the general menu band, like the Start Game button at the panel's
    bottom edge.
    """
    screen = desktop_size()
    if screen is None:
        return []
    screen_w, screen_h = screen
    x, y, w, h = bounds
    def collect(observations: list[dict]) -> list[tuple[int, int]]:
        found = []
        for observation in observations:
            if not _menu_label_matches(str(observation.get("text", "")), label):
                continue
            point = _observation_point(observation)
            if point is None:
                continue
            px, py = int(point[0] * screen_w), int(point[1] * screen_h)
            if x <= px <= x + w and y <= py <= y + h:
                found.append((px, py))
        return found

    points = collect(macos_ocr.recognize(path))
    if not points:
        points = collect(_menu_crop_ocr(path, bounds))
    if not points and strip is not None:
        points = collect(_menu_crop_ocr(path, bounds, strip=strip, tag="strip"))
    return points


def _main_menu_point(path: Path, bounds: tuple[int, int, int, int]) -> tuple[int, int] | None:
    """Read the Single Player row center instead of assuming a window-height ratio."""
    return _observed_label_point(path, "Single Player", bounds)


def return_to_main_menu(bounds: tuple[int, int, int, int], run_dir: Path,
                        attempt: int) -> bool:
    """Back out of setup dialogs and prove the main menu is visible.

    A setup failure may leave the leader list, map picker, or Create Game page
    open. Retrying the main-menu coordinates from any of those screens changes
    an unrelated setting. Only a screenshot containing Single Player licenses
    another bootstrap attempt.
    """
    x, y, w, h = bounds
    for depth in range(4):
        shot = run_dir / f"recover-attempt{attempt}-{depth}.png"
        screenshot(shot)
        if _main_menu_visible(shot):
            return True
        click_at(int(x + w * BACK[0]), int(y + h * BACK[1]))
        time.sleep(1.5)
    shot = run_dir / f"recover-attempt{attempt}-final.png"
    screenshot(shot)
    return _main_menu_visible(shot)


def _loading_probe(run_dir: Path, attempt: int, patience: dict, grant: float):
    """Answer "is the game still somewhere other than the main menu?".

    The startup gate needs to tell a slow map generation from a click that did
    nothing, and those look identical in the log -- both are silent. They do not
    look identical on the SCREEN: a launched game is on a loading or in-game
    view, and a missed click leaves Single Player sitting right where it was.

    The same `_main_menu_visible` read that licenses a retry answers it, so this
    adds no new way to be wrong: a false "menu" costs one early give-up that the
    caller was about to make anyway, and a false "not menu" costs one more
    budget. Each call keeps its screenshot, numbered by the wait it belongs to,
    so a run that dies here can be looked at afterwards.

    ``patience`` is spent from ONE pool shared by every attempt, because the
    per-wait hard bound alone does not bound the bootstrap: 16 attempts each
    allowed six budgets is over three hours of a screen that is neither a menu
    nor a game. A shared pool keeps the worst case to a single extra bound no
    matter how the attempts divide it, and a game that really is generating a
    map only ever needs it once.
    """
    looks = {"n": 0}

    def still_loading() -> bool:
        looks["n"] += 1
        shot = run_dir / f"startup-wait{attempt}-{looks['n']}.png"
        screenshot(shot)
        if _main_menu_visible(shot):
            print(f"attempt {attempt}: the main menu is back after "
                  f"{looks['n']} silent wait(s) -- the launch did not take",
                  file=sys.stderr)
            return False
        if patience["left"] < grant:
            print(f"attempt {attempt}: the main menu is gone but nothing has "
                  f"loaded in {patience['spent']:.0f}s of waiting -- this is "
                  "not a map still generating", file=sys.stderr)
            return False
        patience["left"] -= grant
        patience["spent"] += grant
        print(f"attempt {attempt}: silent, but the main menu is gone -- the "
              f"game is still coming up (wait {looks['n']}, "
              f"{patience['left']:.0f}s of patience left)")
        return True

    return still_loading


def configure_and_start(bounds: tuple[int, int, int, int], args: argparse.Namespace,
                        run_dir: Path) -> bool:
    """Set this run's game up on the Create Game screen and start it.

    The agent's first report from inside the game remains the check on the VALUES
    -- it names the difficulty, map size and speed the game actually has, so a
    misclick shows up as a run that says so rather than a Deity result quietly
    recorded as Settler.

    ⚠ But that readback was never sufficient on its own, and this used to say it
    was. It can only report what the setting IS; it cannot undo what a stray click
    changed on the way. On a Small map the option click landed on the Dramatic Ages
    toggle -- see `set_dropdown`, which now verifies the list opened and declines to
    click at all rather than click blind.
    """
    for name, value in (("difficulty", args.difficulty),
                        ("map_size", args.map_size),
                        ("speed", args.speed)):
        if not set_dropdown(bounds, name, value, run_dir):
            print(f"[setup] {name} was NOT set; refusing to start an unverified game",
                  flush=True)
            return False
    if not select_requested_leader(bounds, args.leader, run_dir):
        print("[setup] requested leader was NOT selected; refusing to start", flush=True)
        return False
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
    if args.map != "Continents.lua":
        print(f"map selection is disabled pending verification; refusing to claim "
              f"the default Continents map is {args.map}", file=sys.stderr)
        return False
    setup_shot = run_dir / "setup.png"
    screenshot(setup_shot)
    start_point = _observed_label_point(setup_shot, "Start Game", bounds,
                                        strip=START_GAME_STRIP)
    if start_point is None:
        print("[setup] Start Game was NOT visible; refusing to launch",
              file=sys.stderr)
        return False
    # OCR and the preceding setup clicks can leave another desktop window
    # frontmost. Make the target application key immediately before sending
    # the launch event; otherwise a correct screen coordinate can be consumed
    # by the browser beside the game.
    focus_game(GAME_SIDE, GAME_FRACTION)
    click_at(*start_point)
    return True


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
    def started(seconds: float, still_loading=None) -> bool:
        return wait_for_agent_start(tail, on_event, seconds,
                                    still_loading=still_loading)

    board_seen = {"value": False}

    def board_is_ready() -> bool:
        """Drain a full log batch and retain the first playable-board proof.

        ``LogTail.poll`` can return loaded, seat, and state together.  Draining
        it here is intentional: returning immediately at ``loaded`` would lose
        the state consumed while the intro probe is yielding, leaving the brain
        waiting on a board the launcher had already seen.
        """
        ready = False
        for event in tail.poll():
            on_event(event)
            if board_event_proves_intro_is_gone(event):
                ready = True
        board_seen["value"] = board_seen["value"] or ready
        return board_seen["value"]

    # One pool of extra waiting for the whole bootstrap, spent only against a
    # screen that shows no main menu. See `_loading_probe`.
    patience = {"left": verify_s * LOADING_PATIENCE, "spent": 0.0}

    # The mod scan lands in Modding.log minutes before the menu can be clicked
    # -- the 2K and Firaxis logos play over the top of it -- so "main menu
    # reached" is not the same as "main menu ready". Rather than guess a settle
    # time, each attempt clicks Single Player and looks for the submenu it
    # should have opened. No submenu means the menu was not up yet, which is a
    # reason to wait, not a reason to give up: an earlier run bailed with "no
    # game window found" while the game was still showing the 2K logo.
    # Consecutive attempts that saw neither a menu nor a submenu. Three in a
    # row means "stuck on a full screen" (the Additional Content screen shows
    # ≤1 row to BOTH readers — indistinguishable from a menu that has not
    # loaded yet), not "still booting"; the BACK click is empty artwork on a
    # genuinely unready menu, so a false strike costs nothing.
    blind_strikes = 0
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

        # ★ Read the TOP-LEVEL menu off the screen before aiming. The 2026-08
        # "Civ VII" promo banner shifted the whole column ~0.11 of the window
        # DOWN, so the fixed fraction clicked empty artwork ABOVE the menu —
        # sixteen attempts of nothing, a whole batch dead on arrival. The
        # first row is "Single Player" on this build (Continue lives inside
        # the submenu as "Resume"). The fixed fraction stays as the fallback
        # when the read fails, so a vision-less host behaves exactly as before.
        menushot = run_dir / f"menu-attempt{attempt}.png"
        screenshot(menushot)
        menu_point = _main_menu_point(menushot, bounds)
        toprows = vision.menu_rows(menushot, bounds) if vision.available() else []
        sp_y = MENU["single_player"][1]
        pitch = 0.029
        if menu_point is not None:
            sp_y = (menu_point[1] - y) / h
            if len(toprows) >= 2:
                pitch = toprows[1] - toprows[0]
            print(f"attempt {attempt}: Single Player label read at {menu_point}",
                  file=sys.stderr)
        elif len(toprows) >= 4:
            sp_y = toprows[0]
            pitch = toprows[1] - toprows[0]
            menu_point = (int(x + w * MENU["single_player"][0]),
                          int(y + h * sp_y))
            print(f"attempt {attempt}: menu read at {sp_y:.3f} "
                  f"(pitch {pitch:.3f}, {len(toprows)} rows)", file=sys.stderr)
        else:
            print(f"attempt {attempt}: top menu not readable "
                  f"({len(toprows)} rows) -- refusing a blind menu click",
                  file=sys.stderr)
            blind_strikes += 1
            if blind_strikes >= 3:
                print("three blind attempts -- assuming a stuck full screen "
                      "and clicking BACK", file=sys.stderr)
                click_at(int(x + w * 0.723), int(y + h * 0.177))
                blind_strikes = 0
                time.sleep(3.0)
            else:
                time.sleep(20.0)
            continue
        click_at(*menu_point)
        time.sleep(2.5)
        # Read the submenu rather than assume its shape. Which entries it has
        # depends on whether there is a save and a game to resume, so where
        # "Create Game" lands moves between runs. The crop follows the read
        # menu position for the same reason the aim does.
        submenu = run_dir / f"submenu-attempt{attempt}.png"
        screenshot(submenu)
        target = _observed_label_point(submenu, "Create Game", bounds)
        rows = (vision.submenu_rows(submenu, bounds, near=sp_y, pitch=pitch)
                if vision.available() else [])
        if target is None and len(rows) < 3:
            blind_strikes = blind_strikes + 1 if len(toprows) < 4 else 0
            print(f"attempt {attempt}: no submenu ({len(rows)} rows) -- "
                  f"the menu is not ready yet (blind strike {blind_strikes})",
                  file=sys.stderr)
            if not env.game_pids():
                print("the game exited while starting", file=sys.stderr)
                return False
            if blind_strikes >= 3:
                print("three blind attempts -- assuming a stuck full screen "
                      "and clicking BACK", file=sys.stderr)
                click_at(int(x + w * 0.723), int(y + h * 0.177))
                blind_strikes = 0
                time.sleep(3.0)
            else:
                time.sleep(20.0)
            continue
        blind_strikes = 0
        if len(rows) > 6:
            # A full-screen LIST -- the Additional Content screen a mis-aimed
            # click can walk into -- not a menu submenu. Its exit is the BACK
            # button at the top right; Escape does nothing there (measured
            # 2026-08-01 on the live screen). The same click is empty artwork
            # on the plain menu, so a false positive is a no-op.
            print(f"attempt {attempt}: {len(rows)} rows is a list screen, not "
                  "a submenu -- clicking BACK", file=sys.stderr)
            click_at(int(x + w * 0.723), int(y + h * 0.177))
            time.sleep(2.0)
            continue
        if target is None and len(rows) >= 3:
            target = (int(x + w * SUBMENU_X), int(y + h * rows[-3]))
        if target is None:
            print(f"attempt {attempt}: Create Game is not visible yet",
                  file=sys.stderr)
            if not env.game_pids():
                print("the game exited while starting", file=sys.stderr)
                return False
            time.sleep(20.0)
            continue
        print(f"create game point: {target} (read from the visible label)")
        click_at(*target)
        time.sleep(2.5)
        screenshot(run_dir / f"create-attempt{attempt}.png")
        if not configure_and_start(bounds, args, run_dir):
            print(f"attempt {attempt}: setup could not be verified; backing out",
                  file=sys.stderr)
            if not return_to_main_menu(bounds, run_dir, attempt):
                print("setup recovery could not verify the main menu; refusing "
                      "unsafe coordinate retries", file=sys.stderr)
                return False
            continue
        # Hosting a game opens a leader introduction before the map.  It is a
        # real modal gate on this install, so waiting for agent telemetry here
        # would leave a valid setup stranded behind its Begin Game button.
        # Failure to recognize it remains safe: the lifecycle gate below still
        # refuses to accept auto-close-only startup.
        # The first screenshot can still be the Create Game page while Firaxis
        # opens the post-host modal. Keep proving the exact page for the whole
        # startup window rather than allowing a transient false negative to
        # strand the run behind Begin Game.
        intro_retries = max(4, min(60, int(verify_s / 2)))
        advance_leader_intro(bounds, args.leader, run_dir, attempt,
                             retries=intro_retries, poll_s=2.0,
                             board_ready=board_is_ready)
        if board_seen["value"]:
            # A direct host transition can open the board without ever drawing
            # the leader card.  Its state has already been relayed above, so do
            # not spend the entire normal startup budget waiting for a second
            # event that may not arrive until the brain has that state.
            return True
        if started(verify_s, still_loading=_loading_probe(run_dir, attempt,
                                                          patience, verify_s)):
            return True
        if not env.game_pids():
            print("the game exited while starting", file=sys.stderr)
            return False
        # Back out of whatever that opened and try again.
        if not return_to_main_menu(bounds, run_dir, attempt):
            print("start recovery could not verify the main menu; refusing "
                  "unsafe coordinate retries", file=sys.stderr)
            return False
        print(f"attempt {attempt}: no game started after Create Game",
              file=sys.stderr)
    return False


# Firaxis's rolling autosaves on this build: `AutoSave_NNNN.Civ6Save`, one per
# turn, the newest ten kept, older ones rotated into `prev/`. The Load Game
# screen lists them only while its "Autosaves" filter is ticked.
AUTOSAVE_DIR = (Path.home() / "Library" / "Application Support"
                / "Sid Meier's Civilization VI" / "Sid Meier's Civilization VI"
                / "Saves" / "Single" / "auto")


def latest_autosave(directory: Path = AUTOSAVE_DIR,
                    newer_than: float | None = None) -> Path | None:
    """The most recently written autosave, or None.

    ⚠ By modification time, not by number: the numbering wraps and the newest
    file after a rotation can carry a lower number than an older one. Bounded
    below by ``newer_than`` (a POSIX timestamp) so a resume never loads a save
    from some earlier game that happens to be the newest file on disk.
    """
    try:
        candidates = [path for path in directory.glob("AutoSave_*.Civ6Save")
                      if path.is_file()]
    except OSError:
        return None
    if newer_than is not None:
        candidates = [path for path in candidates
                      if path.stat().st_mtime >= newer_than]
    if not candidates:
        return None
    return max(candidates, key=lambda path: path.stat().st_mtime)


def bootstrap_saved_game(tail: watch.LogTail, on_event, run_dir: Path,
                         args: argparse.Namespace, verify_s: float = 120.0) -> bool:
    """Load a named save after proving each rendered menu target.

    A save replay is the shortest reliable regression test for behavior that
    appeared late in a real game. The file must already be in Firaxis's Single
    Player save directory; the path supplies the exact rendered filename to
    select, so this never guesses which row happens to be first.
    """
    def started(seconds: float, still_loading=None) -> bool:
        return wait_for_agent_start(tail, on_event, seconds,
                                    still_loading=still_loading)

    patience = {"left": verify_s * LOADING_PATIENCE, "spent": 0.0}
    save_label = Path(args.load_save).stem
    for attempt in range(1, BOOTSTRAP_ATTEMPTS + 1):
        focus_game(GAME_SIDE, GAME_FRACTION)
        time.sleep(2.0)
        bounds = game_window()
        if bounds is None:
            time.sleep(20.0)
            continue
        x, y, w, h = bounds
        click_at(int(x + w * 0.15), int(y + h * 0.85))
        time.sleep(1.5)

        menu = run_dir / f"load-menu-attempt{attempt}.png"
        screenshot(menu)
        target = _observed_label_point(menu, "Load Game", bounds)
        menu_point = _main_menu_point(menu, bounds)
        if target is None and menu_point is not None:
            click_at(*menu_point)
            time.sleep(2.5)
            submenu = run_dir / f"load-submenu-attempt{attempt}.png"
            screenshot(submenu)
            target = _observed_label_point(submenu, "Load Game", bounds)
        if target is None:
            print(f"attempt {attempt}: Load Game is not visible yet", file=sys.stderr)
            if not env.game_pids():
                return False
            time.sleep(20.0)
            continue

        click_at(*target)
        time.sleep(3.0)
        panel = run_dir / f"load-panel-attempt{attempt}.png"
        screenshot(panel)
        save_target = _observed_label_point(panel, save_label, bounds)
        # ⚠ AUTOSAVES ARE HIDDEN UNTIL THEIR FILTER IS TICKED. The Load Game
        # list opens with the "Autosaves" checkbox off, showing only manual and
        # quick saves — measured on `civvis-20260815T230003Z-cont`, whose
        # `load-panel-attempt3.png` shows the filter unticked, "(Quick Save)"
        # alone in the list, and the resume that was meant to reload
        # AutoSave_0098 refusing "not visible" three times. Tick the filter by
        # its own read label, once, and look again; a filter that cannot be
        # read leaves the existing refusal in force.
        if save_target is None and _normalized_label(save_label).startswith("autosave"):
            toggle = _observed_label_point(panel, "Autosaves", bounds)
            if toggle is not None:
                click_at(*toggle)
                time.sleep(1.5)
                panel = run_dir / f"load-panel-autosaves-attempt{attempt}.png"
                screenshot(panel)
                save_target = _observed_label_point(panel, save_label, bounds)
                print(f"ticked the Autosaves filter; {save_label!r} "
                      f"{'found' if save_target else 'still not visible'}", flush=True)
        if save_target is None:
            print(f"saved game {save_label!r} is not visible; refusing to select a row",
                  file=sys.stderr)
            return False
        click_at(*save_target)
        time.sleep(1.5)

        selected = run_dir / f"load-selected-attempt{attempt}.png"
        screenshot(selected)
        # The screen contains a LOAD GAME heading and a lower action button.
        # The button is the lowest matching label; selecting the first match
        # would click the inert heading and wait forever.
        action_points = _observed_label_points(selected, "Load Game", bounds)
        if not action_points:
            print("the saved-game action button is not visible", file=sys.stderr)
            return False
        action = max(action_points, key=lambda point: point[1])
        if action[1] < y + int(h * 0.75):
            print("only the Load Game heading is visible; refusing to click it",
                  file=sys.stderr)
            return False
        print(f"loading saved game {save_label} from observed row {save_target}")
        click_at(*action)

        # A save made with an older revision of the same mod can produce a
        # compatibility confirmation. Give a normal load a head start, then
        # acknowledge only a visibly read YES button. This first wait stays
        # deliberately blind to the screen: a confirmation dialog also hides the
        # main menu, so a loading probe here would extend the budget instead of
        # letting the run get on with answering the dialog.
        if started(min(10.0, verify_s)):
            return True
        confirmation = run_dir / f"load-confirmation-attempt{attempt}.png"
        screenshot(confirmation)
        yes = _observed_label_point(confirmation, "Yes", bounds)
        if yes is not None:
            click_at(*yes)
        if started(verify_s, still_loading=_loading_probe(run_dir, attempt,
                                                          patience, verify_s)):
            return True
        if not env.game_pids():
            return False
        print(f"attempt {attempt}: saved game emitted no agent state", file=sys.stderr)
    return False


def dismiss_leader_dialogue(clicks: int = 6) -> bool:
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
    # ⚠ FOUR SHAPES NOW. A TWO-option first-contact screen sits between the bands
    # already covered and was missed by all of them. Measured off `stalled-3.png`
    # (run civvis-20260730T192135Z, turn 175, Cyrus): window (864,33,864,542), the
    # two options at roughly (1011,492) and (1011,519) — fy 0.847 and 0.897, where
    # the nearest existing target was 0.913 and the stack pair sat at 0.73/0.68.
    # Thirty-five pixels of miss cost a run holding FIVE cities and score 209, the
    # best of the day.
    # ★★★★ SWEEP THE COLUMN, DO NOT ENUMERATE THE SHAPES.
    #
    # The list above this comment grew one entry per lost run — three shapes, then
    # four, each added after a stall that a thirty-five pixel miss had caused. Run
    # civvis-20260730T223506Z died the same way at turn 88 on a three-option
    # delegation from John Curtin. Enumerating variants cannot converge: Civilization
    # VI composes these screens from a variable number of options, so the Nth shape
    # is always one run away.
    #
    # Every variant shares a geometry, and that is the thing worth encoding: the
    # options are a VERTICAL STACK at the lower-left of the game window, x ~= 0.17.
    # So sweep that column densely enough that no option can fall between two clicks.
    # Measured off stalled-1.png of the run above: options at fy 0.828, 0.876 and
    # 0.920, which a 0.02 step covers with room to spare.
    #
    # ⚠ Clicks that miss land on the leader art, which does nothing. This runs ONLY
    # after a stall is confirmed and one of these screens is therefore up; it is not
    # safe to sweep a live map, where a click can select a unit and the next can
    # order it to move.
    targets = [("refuse deal", 0.222, 0.174)]
    step = 0.02
    band = int(round((0.95 - 0.60) / step))
    targets += [
        ("stack sweep %.2f" % (0.60 + i * step), 0.170, 0.60 + i * step)
        for i in range(band + 1)
    ]
    print(f"[dialogue] window {rect}")
    # ⚠ EACH TARGET NEEDS SEVERAL CLICKS, NOT ONE. `clicks // len(targets)` gave
    # exactly one click per position once the target list grew, and a leader
    # conversation is a CHAIN: choosing an option can open the next statement, so one
    # click opens a new question rather than ending anything. Measured on
    # stalled-1.png of run civvis-20260730T200543Z — a three-option delegation offer
    # that survived three full rescue rounds.
    # ⚠ PASSES OUTSIDE, POSITIONS INSIDE. This used to click one position six times
    # before moving on, which is the wrong order for a CHAIN: when an option is not
    # at that spot the five extra clicks do nothing, and when one is, the statement it
    # opens is somewhere else by the time the next click lands. Walking the whole
    # column once per pass advances a chain one link per pass, which is what a person
    # does. Same number of clicks, and the sweep now finishes in about 6s of a 240s
    # stall budget instead of 46s.
    passes = max(1, clicks // 2)
    for attempt in range(passes):
        print(f"[dialogue] pass {attempt + 1}/{passes} over {len(targets)} positions")
        for name, fx, fy in targets:
            x, y = int(wx + ww * fx), int(wy + wh * fy)
            click_at(x, y)
            time.sleep(0.1)
    return True


def dismiss_visually_confirmed_popup() -> tuple[bool, str]:
    """Click one safe target only when the current pixels prove a modal exists."""
    rect = game_window()
    if rect is None:
        return False, "game window unavailable"
    focus_game(GAME_SIDE, GAME_FRACTION)
    time.sleep(0.25)
    try:
        window, scale = popup_clear.capture(rect)
        surface, targets, _dark = popup_clear.classify(window)
    except (OSError, subprocess.SubprocessError):
        return False, "popup classification failed"
    if surface not in ("leader", "notice"):
        return False, f"no safe visible dialogue ({surface})"
    target = popup_clear.click_target(surface, targets, window.size[0])
    if target is None:
        return False, f"visible {surface} has no confirmed button"
    popup_clear.held_click(target, rect, scale)
    return True, f"confirmed {surface} button"


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
        result = macos_input.press_key("escape")
        ok = ok and result.returncode == 0
        time.sleep(0.8)
    return ok


def dismiss_world_congress_between_turns() -> bool:
    """Click the shipped close control on the between-turns Congress screen."""
    focus_game(GAME_SIDE, GAME_FRACTION)
    bounds = game_window()
    if bounds is None:
        return False
    wx, wy, ww, wh = bounds
    # Measured against the stock Gathering Storm context: its close button is
    # inset six points from the right and seven percent of the window height
    # below the title-bar origin. This is window-relative so the mandated
    # upper-right layout and a different Retina scale do not change the target.
    click_at(wx + ww - 6, wy + int(wh * 0.07))
    return True


def play(args: argparse.Namespace) -> int:
    # One run at a time against this installation. Two harnesses share one mod
    # directory, one log and one process; the second one's install lands in the
    # middle of the first one's game and neither notices.
    if not vision.available():
        print(
            "Pillow is required for verified Civ VI menu navigation; install it with "
            "python3 -m pip install --user Pillow",
            file=sys.stderr,
        )
        return 2
    hold_macos_awake()
    wait_for_unlocked_session()
    if not gamelock.acquire(args.tag, wait_s=args.lock_wait):
        foreign = gamelock.foreign_run(args.tag)
        print(f"another run holds the game: {foreign or gamelock.describe()}",
              file=sys.stderr)
        return 6
    try:
        return _play(args)
    finally:
        # The installation is exclusively ours while this lock is held. An
        # interrupt or unexpected exception must not leave the game advancing
        # after its event stream and mirror have stopped.
        launcher.stop()
        gamelock.release()


def seat_matches_requested(event: dict, args: argparse.Namespace) -> tuple[bool, bool]:
    """Return (full_config_match, modes_match) from the game's own seat report."""
    modes = event.get("modes")
    modes_match = modes is not None and sorted(modes) == sorted(args.game_mode)
    return (
        event.get("difficulty") == args.difficulty
        and event.get("size") == args.map_size
        and event.get("speed") == args.speed
        and event.get("map") == args.map
        and (args.leader is None or event.get("leader") == args.leader)
        and modes_match,
        modes_match,
    )


def _play(args: argparse.Namespace) -> int:
    run_dir = RUN_ROOT / args.tag
    run_dir.mkdir(parents=True, exist_ok=True)
    if args.load_save and not Path(args.load_save).is_file():
        print(f"saved game does not exist: {args.load_save}", file=sys.stderr)
        return 8
    using_default_orders_db = not args.orders_db
    orders_db = orders_db_path(run_dir, args.orders_db)
    args.orders_db = str(orders_db)
    config = build_config(args)
    events_path = run_dir / "events.jsonl"
    events = events_path.open("a")

    if not launcher.stop():
        print("could not stop the previous Civilization VI process", file=sys.stderr)
        return 3
    if using_default_orders_db:
        # The game is stopped above, so this removes only a prior incarnation of
        # this tag.  Never reset an explicitly shared path behind an operator's
        # back: an attached SQLite handle would keep reading the old inode.
        reset_orders_db(orders_db)
    target = modinstall.install(config)
    print(f"installed {target}")
    print(f"  difficulty {config['Difficulty']}  map {config['MapSize']}"
          f"  speed {config['GameSpeed']}  max turns {config['MaxTurns']}")

    brain = None
    brain_log = None
    if args.civvis_decides:
        binary = Path(args.civvis_bin).expanduser() if args.civvis_bin else (
            REPO_ROOT / "target" / "release" / "civvis_orders"
        )
        if not binary.is_file():
            print(f"CIVVIS decision binary does not exist: {binary}", file=sys.stderr)
            return 4
        brain_log = (run_dir / "brain.log").open("a", buffering=1)
        command = supervised_brain_command(args, run_dir, orders_db, binary)
        brain = subprocess.Popen(
            command,
            cwd=REPO_ROOT,
            stdout=brain_log,
            stderr=subprocess.STDOUT,
            text=True,
        )
        # ⚠ Print the WHOLE configuration, not a chosen subset. `--war-from-plan`
        # was carried by a second, undeclared brain for as long as it existed, and a
        # banner naming only strategy and victory could not have shown that.
        refresh = ("brain-default" if args.civvis_refresh_seconds is None
                   else args.civvis_refresh_seconds)
        print(f"CIVVIS decision worker pid={brain.pid} strategy={args.civvis_strategy} "
              f"victory={args.civvis_victory} "
              f"war_from_plan={args.civvis_war_from_plan} "
              f"refresh_seconds={refresh} bin={binary}")

    def stop_brain() -> None:
        nonlocal brain, brain_log
        if brain is not None and brain.poll() is None:
            brain.terminate()
            try:
                brain.wait(timeout=15)
            except subprocess.TimeoutExpired:
                brain.kill()
                brain.wait(timeout=5)
        if brain_log is not None:
            brain_log.close()
            brain_log = None

    # Covers KeyboardInterrupt and unexpected exceptions between the explicit
    # cleanup sites below. Calling it again after a normal run is harmless.
    atexit.register(stop_brain)

    launcher.clear_run_logs()
    launcher.launch(stdout=run_dir / "stdout.log")
    if not launcher.wait_for_main_menu(args.startup_timeout):
        stop_brain()
        print("the game did not reach the main menu", file=sys.stderr)
        return 3
    # Establish the requested operator layout before taking any measurements.
    # Menu rows and setup controls are now read from this final geometry.
    place_game(GAME_SIDE, GAME_FRACTION, GAME_VFRACTION)
    print("main menu reached; the setup context should host the game now")

    tail = watch.LogTail()
    state = {
        "hosted": False, "seat": None, "turn": -1, "score": -1,
        "outcome": None, "last_progress": time.monotonic(), "configured": False,
        "modes": None, "mode_mismatch": False,
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
            # An older mod does not report `modes` at all, and a missing
            # field is not the same answer as an empty list: it means nobody
            # looked. Treat it as unknown rather than as clean, or this gate
            # passes for exactly the runs it was added to catch.
            configured, modes_match = seat_matches_requested(event, args)
            modes = event.get("modes")
            state["modes"] = modes
            state["mode_mismatch"] = not modes_match
            state["configured"] = configured
            if not state["configured"]:
                print("[agent] the game does not match what was asked for",
                      file=sys.stderr)
            if not modes_match:
                print(f"[agent] game modes are {modes if modes is not None else 'UNREPORTED'}, "
                      f"asked for {args.game_mode or '[]'} -- this is not the "
                      f"ruleset CIVVIS is compared against", file=sys.stderr)
            print(f"[agent] seat {event.get('local_player')} {event.get('civ')} "
                  f"difficulty={event.get('difficulty')} size={event.get('size')} "
                  f"speed={event.get('speed')} max_turns={event.get('max_turns')} "
                  f"players={event.get('players')} modes={modes} "
                  f"setup={event.get('setup')}")
        elif kind == "turn":
            state["turn"] = event.get("turn", -1)
            state["score"] = event.get("score", -1)
            state["last_progress"] = time.monotonic()
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
        elif kind in ("autoclose_desktop", "autoclose_stuck"):
            # Every desktop request is pixel-classified before any click. A
            # DiplomacyActionView context can remain technically visible while
            # the ordinary map is in front; treating its counter alone as proof
            # caused a live t68 fallback to sweep clicks across an uncovered map.
            #
            # `autoclose_stuck` means twenty close attempts failed. Photograph the
            # exact variant before clicking: dialogue geometry has changed several
            # times, and without the frame a miss cannot be repaired honestly.
            #
            # A leader conversation needs a dialogue option CHOSEN; everything
            # else on this list just needs dismissing. Escape was tried for the
            # conversation case and does nothing at all on it — verified by hand
            # against a live stuck screen — so the two get different treatment.
            screen = event.get("screen")
            shot = run_dir / f"autoclose-stuck-turn-{state['turn']}.png"
            screenshot(shot)
            reason = (
                "requested desktop help after"
                if kind == "autoclose_desktop" else "gave up after"
            )
            print(f"[{kind}] {screen} {reason} "
                  f"{event.get('attempts')} attempts; photographed to {shot}")
            # ⚠⚠ ESCAPE WITH NOTHING TO CLOSE OPENS THE PAUSE MENU, AND THAT KILLS THE
            # RUN. Photographed at the moment of a stall (run civvis-20260730T181327Z,
            # turn 69, three healthy cities at loyalty 100): Civilization VI showing
            # RETURN TO GAME / SAVE / OPTIONS / RETIRE / EXIT TO DESKTOP. A paused game
            # advances no turns, so the harness then recorded its own keystroke as
            # "stalled".
            #
            # The screens that had "given up" were TradeRouteChooser and
            # TechCivicCompletedPopup, and the run had already reached turn 69 WITH them
            # stuck — they were never blocking anything. The blind Escape was more
            # dangerous than the screen it was aimed at.
            #
            # So the key is pressed only for screens known to hold the game. Everything
            # else is reported and left alone, which is the honest response to "the shim
            # gave up on a screen that is not stopping us".
            BLOCKING = ("DiplomacyActionView", "LeaderView", "DiplomacyDealView",
                        "WorldCongressBetweenTurns", "GreatWorkShowcase",
                        "ChooseArtifact")
            if screen in ("DiplomacyActionView", "LeaderView", "DiplomacyDealView"):
                ok, how = dismiss_visually_confirmed_popup()
            elif screen == "WorldCongressBetweenTurns":
                ok = dismiss_world_congress_between_turns()
                how = "World Congress close control"
            elif screen in BLOCKING:
                ok = press_escape()
                how = "escape"
            else:
                ok = True
                how = "left alone (not a blocking screen)"
            safe_skip = not ok and how.startswith("no safe visible dialogue")
            result = "sent" if ok else "skipped safely" if safe_skip else "FAILED"
            print(f"[{kind}] {how} {result} for {screen}",
                  file=sys.stderr if not ok and not safe_skip else sys.stdout)
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
        # A game with an optional mode on is not the game CIVVIS is compared
        # against, and 250 turns of it is 250 turns of nothing. Stop at the
        # seat event rather than at the end.
        #
        # ⚠ This is the only thing standing between a wrong game and a full
        # run, because the setter cannot be relied on: the setup context that
        # would apply it does not host on this installation -- `configured` is
        # absent on all twenty recent runs -- so the modes are whatever the
        # Create Game screen carries, and nothing drives its Advanced Setup
        # tab. Detection is the guarantee here; setting is best effort.
        if kind == "seat" and not state["configured"]:
            return True
        return False

    if args.load_save:
        bootstrapped = bootstrap_saved_game(tail, record, run_dir, args,
                                            verify_s=args.load_wait)
    else:
        bootstrapped = bootstrap_game(tail, record, run_dir, args)
    if not bootstrapped:
        stop_brain()
        print("could not load the saved game" if args.load_save else
              "could not start a game from the main menu", file=sys.stderr)
        return 5
    print("in a configured game; the agent holds the seat from here")

    # Hold the foreground for the whole game. Anything else taking focus --
    # a browser, another agent's automation -- throttles the game to almost no
    # frames, and the turn loop runs off game-core events, so the run stops
    # without a single log line saying why.
    last_focus = [0.0]
    session_was_locked = [False]

    def console_locked() -> bool:
        locked = screen_locked()
        if locked and not session_was_locked[0]:
            print("[session] macOS locked; GUI upkeep paused while the live "
                  "event/decision loop continues", flush=True)
        elif not locked and session_was_locked[0]:
            print("[session] macOS unlocked; refocusing Firaxis and continuing",
                  flush=True)
            last_focus[0] = 0.0
        session_was_locked[0] = locked
        return locked

    def keep_foreground() -> None:
        now = time.monotonic()
        if now - last_focus[0] < args.focus_every:
            return
        last_focus[0] = now
        focus_game()
        # Safe here and only here: the game is in play, so there is no menu
        # being read off the screen for a resize to invalidate.
        place_game(GAME_SIDE, GAME_FRACTION, GAME_VFRACTION)

    # ⚠ THE POLL INTERVAL IS THE OUTBOUND LEG OF THE DECISION LOOP. With CIVVIS
    # deciding, the mod holds its turn open until orders arrive, and orders cannot
    # be computed until the board reaches `events.jsonl` through this tail. At the
    # 2 s default the round trip lost the race against the mod's tick budget on 6
    # of 10 turns of run `smoke-20260730T105241Z` — the brain had written every one
    # of them in 0.00 s. Polling is cheap; a stalled decision loop is not.
    poll_s = 0.25 if args.civvis_decides else 2.0
    # ⚠ A STALLED RUN IS DEAD, AND WAITING TEN MINUTES FOR IT COSTS A WHOLE ATTEMPT.
    # Run civvis-20260730T140023Z wedged at turn 87 and burned the full 600 s before the
    # ladder could start the next game. The mod emits at least one event per turn and a
    # turn takes a few seconds, so silence for a couple of minutes already means wedged.
    # Shorter is only wrong if the machine is so loaded that turns take minutes — see
    # the contention note — so it is a flag, not a constant.
    # ⚠ THE WALL CLOCK ONLY EVER KILLS A HEALTHY RUN. `stall_s` and `frozen_s`
    # between them catch every way a run dies -- silence, and a turn that stops
    # moving while events keep arriving -- so anything still alive at `--timeout`
    # is alive and merely SLOW. Three such runs died in the day to 2026-08-11 at
    # turns 209, 197 and 189 of 250, on a host at load 53, and each wrote its
    # partial score into the ladder as if it were a result. `finish_turn` lets
    # the budget ask "can this still get there?" instead of "is the clock up?".
    ceiling_s = (args.timeout_ceiling if args.timeout_ceiling is not None
                 else args.timeout * 1.5)
    reason = watch.follow(tail, args.timeout, record, stop_when=finished,
                          each_poll=keep_foreground, poll_s=poll_s,
                          stall_s=args.stall_seconds,
                          frozen_s=args.frozen_seconds,
                          pause_when=console_locked,
                          finish_turn=args.max_turns, ceiling_s=ceiling_s)
    # ★★★ PHOTOGRAPH A STALL BEFORE KILLING IT. Stalls are now the dominant way runs
    # end — t87, t95, t106, t184 — and the event stream goes silent by definition, so
    # it cannot say what is on screen. One screen (`DiplomacyDealView`) was already
    # found and fixed this way; the rest are invisible without a picture. Every large
    # bug in this project was found in the event stream or in a screenshot.
    # ★★★★★ A STALL IS RECOVERABLE, AND GIVING UP ON THE FIRST ONE THROWS RUNS AWAY.
    #
    # `autoclose_stuck` fires ONCE per screen: the shim calls ClearUpdate and never
    # retries. So the harness's dialogue-click rescue runs once too — and it WORKS,
    # measured on run civvis-20260730T185710Z, where turns 112 and 113 followed
    # immediately after "dialogue clicks sent". When the same leader screen returns
    # later, nothing dismisses it, nobody reports it, and the run dies at turn 120 with
    # three healthy cities.
    #
    # So on a stall: photograph it, try the rescue that already works, and keep
    # watching. Bounded, because a rescue that never rescues must not loop forever.
    # ⚠⚠ THE BUDGET IS CONSECUTIVE FAILURES, NOT RESCUES EVER. This counter used to
    # run for the life of the game, so a rescue that WORKED at turn 60 still spent
    # one of three, and an unrelated leader screen ninety turns later found the budget
    # already gone. Both long runs of 2026-07-31 died that way with the game perfectly
    # healthy: `civvis-20260731T040858Z` at t199 and `civvis-20260731T055749Z` at t179,
    # each after three rescues spread across the whole game. stalled-3.png of the
    # second is Hammurabi asking about barbarians with a single Goodbye button — a
    # screen the column sweep covers and would have cleared, on the fourth try it was
    # not allowed to make.
    #
    # `--stall-rescues` reads "times to try dismissing a blocking screen before giving
    # up", and that is what it now means: give up on a screen the sweep cannot clear,
    # not on a game that has been rescued before.
    consecutive = 0
    rescues = 0
    while reason.startswith("stalled") and consecutive < args.stall_rescues:
        consecutive += 1
        rescues += 1
        shot = run_dir / f"stalled-{rescues}.png"
        screenshot(shot)
        print(f"stalled — photographed to {shot}; rescue attempt {consecutive} "
              f"of {args.stall_rescues} on this screen ({rescues} this run)",
              flush=True)
        dismiss_leader_dialogue()
        reason = watch.follow(tail, args.timeout, record, stop_when=finished,
                              each_poll=keep_foreground, poll_s=poll_s,
                              stall_s=args.stall_seconds,
                              frozen_s=args.frozen_seconds,
                              pause_when=console_locked,
                              finish_turn=args.max_turns, ceiling_s=ceiling_s)
        if not reason.startswith("stalled"):
            print(f"recovered from stall after {consecutive} attempt(s)", flush=True)
            consecutive = 0
    events.close()

    # A terminal run must not leave a live game advancing beside a frozen event
    # stream. That produced a turn-259 Firaxis window next to a turn-191 CIVVIS
    # board, while the old agreement checker compared only the two stale files
    # and reported success. Stop the process before publishing the summary so a
    # completed run has one unambiguous last frame.
    # ⚠⚠⚠ HOLD THE FINAL SCREEN HERE, NOT IN THE MOD — THE MOD CANNOT DO IT.
    #
    # `CivvisControlAutoClose` gained an `EndGameSeconds` clock for `EndGameMenu` in
    # #1041 and it is INERT. Measured across every run on this machine:
    #
    #     250 runs, 145 `autoclose_armed` for EndGameMenu, ZERO `autoclose`
    #
    # The shim never closes that screen, and the reason is structural: Civilization
    # VI halts the Game Core when it shows the end-of-game screen, so the shim is
    # ticking off a frame loop that has just stopped. A Lua clock cannot run there.
    #
    # ⚠ The same absent event is what `civ6_civvis_climb.outcome_of` keys
    # `reached_end_screen` on, which is why that column has been `None` on all 250
    # runs while claiming to record whether the game reached its end screen.
    #
    # So the thing that actually decides how long the operator sees the result is
    # THIS gap — between `finished()` breaking the loop above and `launcher.stop()`
    # killing the game below. Unheld it is a second or two.
    #
    # ⚠ Only when the game actually ENDED. A stall, a timeout, or a wrong-modes
    # refusal has nothing on screen worth looking at, and holding there would add
    # ten seconds to every failure in a batch.
    if state["outcome"] and args.end_game_seconds > 0:
        print(f"holding the final screen for {args.end_game_seconds:.0f}s",
              flush=True)
        time.sleep(args.end_game_seconds)
    game_stopped = launcher.stop()
    stop_brain()
    if not game_stopped:
        print("could not stop Civilization VI after the run", file=sys.stderr)

    outcome = state["outcome"] or {}
    summary = {
        "tag": args.tag,
        "finished_utc": utc_stamp(),
        "difficulty": config["Difficulty"],
        "map_size": config["MapSize"],
        "speed": config["GameSpeed"],
        # A normal real-Civ6 run has no seed value: the control setup has no
        # working world-generation channel. Keep a probe request distinct from
        # a game-world seed so downstream reports cannot mistake it for one.
        "seed_probe": args.seed_probe,
        "seed_request": config["MapSeed"],
        "max_turns": config["MaxTurns"],
        # A run stopped because the game had the wrong modes never played; it
        # is a refusal, not a result. Recording it as `stopped` would file it
        # beside games that were played and lost.
        "reason": ("wrong_game_modes" if state["mode_mismatch"]
                   else "wrong_game_configuration" if state["seat"] and not state["configured"]
                   else reason),
        # Whether the game actually played was the one this run asked for.
        # A summary that reports the requested difficulty without this is a
        # claim about the command line, not about the game.
        "configured": state["configured"],
        # Which game this actually was. A run whose modes are not `[]` was not
        # playing the ruleset any CIVVIS comparison assumes, and the summary is
        # the artefact those comparisons are read from.
        "modes": state["modes"],
        "modes_requested": sorted(args.game_mode),
        "last_turn": state["turn"],
        "last_score": state["score"],
        "seat": state["seat"],
        "outcome": outcome or None,
        "game_stopped": game_stopped,
    }
    # Bridge health rides in the summary: how much of what CIVVIS said the
    # engine actually did. Summed from this run's own turn events so the
    # number describes the run being recorded, not a tool's later reading of
    # it; `civ6_ladder.py check --min-applied` floors it on the ledger.
    try:
        import civ6_ladder
        totals = civ6_ladder.orders_totals(run_dir / "events.jsonl")
        if totals:
            summary["orders_seen"], summary["orders_applied"] = totals
        # The score gap to the best rival is the climb's primary progress
        # metric: our own score doubling means nothing while rival_best
        # holds a four-hundred-point lead at the cap.
        standing = civ6_ladder.final_standing(run_dir / "events.jsonl")
        if standing:
            summary["rival_best"] = standing[1]
        # Which code actually decided this run: the brain's start row plus
        # every mid-game origin/main handoff. On the ledger, so "was the
        # verification game testing the latest code" is a column, not a log
        # excavation.
        revisions = civ6_ladder.decider_revisions(
            run_dir / "runtime_updates.jsonl")
        if revisions:
            summary["decider_revisions"] = revisions
    except Exception as exc:  # noqa: BLE001 — health must not fail the run
        print(f"bridge-health totals unavailable: {exc}", file=sys.stderr)
    (run_dir / "summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True))
    print(json.dumps(summary, indent=2, sort_keys=True))

    # The ladder records itself. This used to be a by-hand step, and the
    # by-hand step simply stopped happening: 211 summaries piled up
    # unrecorded between July 31 and August 16, 2026 while the committed
    # ladder said the last attempt was July 30. The ledger written here is
    # the live one beside the runs directory — never the repository copy, so
    # a finishing game cannot dirty the management worktree it plays from.
    # A recording failure must not fail the run: the summary on disk is the
    # evidence, and `civ6_ladder.py sync` recovers it later.
    try:
        import civ6_ladder
        civ6_ladder.record_summary(run_dir / "summary.json")
    except Exception as exc:  # noqa: BLE001 — deliberately broad, see above
        print(f"ladder record failed (summary is on disk; "
              f"`civ6_ladder.py sync` will recover it): {exc}", file=sys.stderr)

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
    raw_argv = sys.argv[1:] if argv is None else argv
    # `--war-from-plan` is still available on the lower-level replay tools, where
    # comparing the bridge override with CIVVIS's actual decision is useful.  It is
    # deliberately not a live-game option.  The override turns a plan's preferred
    # rival into an immediate declaration even when the planner declined war: live
    # run `live-loop-rome-20260802-0800` forced that declaration under a Religion
    # plan on turn 37, spent the remaining 213 turns in Recovery asking for peace,
    # and finished 400-1081.  A production launcher must not be able to bypass the
    # decider whose behavior it claims to measure.
    if "--civvis-war-from-plan" in raw_argv:
        print(
            "--civvis-war-from-plan is replay-only: it bypasses CIVVIS's war "
            "decision and cannot be used for a live game",
            file=sys.stderr,
        )
        return 2
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--tag", default=None, help="run tag; names the run directory")
    ap.add_argument("--difficulty", default="DIFFICULTY_SETTLER", choices=LADDER)
    ap.add_argument("--ruleset", default="RULESET_EXPANSION_2")
    ap.add_argument("--game-mode", action="append", default=[], choices=GAME_MODES,
                    help="enable an optional game mode (repeatable; default none). "
                         "CIVVIS models none of them, so a run with one on is not "
                         "measuring the game CIVVIS is compared against.")
    ap.add_argument("--map", default="Continents.lua")
    # ★★★★ THE MAP SIZE IS THE PLAYER COUNT, so this is the lobby, not a detail.
    #
    # Civilization VI derives majors and city-states from the size — Duel 2, Tiny 4,
    # Small 6, Standard 8 — and the competitive lobby CIVVIS aims at pins "Firaxis
    # default map size and city-states for the player count" (docs/COMPETITIVE.md,
    # from cpl.gg/rules/in-game-rules). So a size IS a player count, and Duel was
    # measuring a two-player game against rules written for six.
    #
    # Small is the six-player default. The `seat` event reads the size and the
    # player count back from inside the running game and `configured` is false
    # unless they match what was asked, so this is checked rather than assumed.
    ap.add_argument("--map-size", default="MAPSIZE_SMALL")
    ap.add_argument("--speed", default="GAMESPEED_ONLINE")
    ap.add_argument("--seed", type=int, default=424242,
                    help="candidate map/game seed, written only with --seed-probe")
    ap.add_argument("--seed-probe", action="store_true", default=False,
                    help="write requested map/game seeds for civ6_seed_check; does not "
                         "make the real-Civ6 world reproducible")
    ap.add_argument("--max-turns", type=int, default=150)
    ap.add_argument("--city-target", type=int, default=6)
    ap.add_argument("--leader", help="exact Firaxis leader type to select and verify")
    # The game must stay frontmost to get frames, which makes it unwatchable if
    # it also owns the whole screen. Half is enough for the agent and leaves the
    # other half for a terminal.
    ap.add_argument("--window-side", choices=["left", "right", "bottomright", "none"],
                    default="left")
    ap.add_argument("--window-frac", type=float, default=0.5)
    # Taking a walled capital needs a real army, not a garrison. Four units is
    # the Lua default and is thin for a capture even at Settler.
    ap.add_argument("--war-from-turn", type=int, default=25)
    ap.add_argument("--war-army", type=int, default=4)
    ap.add_argument("--military-per-city", type=float, default=1.5)
    ap.add_argument("--peace-deterrence", action="store_true", default=True,
                    help="the fallback ladder's army row weighs the strongest met "
                         "major in peacetime; below half its strength the army "
                         "grows two units at a time, still under ArmyCap")
    ap.add_argument("--no-peace-deterrence", dest="peace_deterrence",
                    action="store_false",
                    help="withhold the peacetime deterrence lift (the A/B arm)")
    ap.add_argument("--explore-until-turn", type=int, default=12)
    ap.add_argument("--make-war", dest="make_war", action="store_true", default=True)
    ap.add_argument("--no-war", dest="make_war", action="store_false")
    # The envoy lane. OFF by default because `chooseEnvoy` is blamed for three
    # game-core SIGSEGVs (3-for-3 against 0-for-2 across repeated requested-seed
    # runs) and the fix for the handle defect behind them is a hypothesis, not a
    # verified result.
    # Until an isolation batch clears it, the known-stable skip stands.
    #
    # It is exposed here because the Lua comment asks for exactly this
    # experiment -- "a CONFIG change, not a code change: place-only, then
    # consider-only, across independent random-world samples" -- and there was
    # no way to run it without
    # hand-editing the mod. `EnvoyPlace`/`EnvoyLevy`/`EnvoyConsider` switch the
    # three mutations independently, so one variable moves at a time.
    ap.add_argument("--envoys", dest="envoys", action="store_true", default=False)
    ap.add_argument("--no-envoys", dest="envoys", action="store_false")
    ap.add_argument("--envoy-place", dest="envoy_place", action="store_true", default=True)
    ap.add_argument("--no-envoy-place", dest="envoy_place", action="store_false")
    ap.add_argument("--envoy-levy", dest="envoy_levy", action="store_true", default=True)
    ap.add_argument("--no-envoy-levy", dest="envoy_levy", action="store_false")
    ap.add_argument(
        "--envoy-consider", dest="envoy_consider", action="store_true", default=True
    )
    ap.add_argument("--no-envoy-consider", dest="envoy_consider", action="store_false")
    ap.add_argument("--assault-width", type=int, default=2)
    ap.add_argument("--settlers-in-flight", type=int, default=1)
    ap.add_argument("--strength-weight", type=int, default=20)
    ap.add_argument("--army-cap", type=int, default=18)
    ap.add_argument("--siege-units", type=int, default=4)
    ap.add_argument("--develop-every", type=int, default=3)
    ap.add_argument("--max-target-ratio", type=float, default=1.3)
    ap.add_argument("--max-empire-distance", type=int, default=6)
    ap.add_argument("--garrison-per-city", type=int, default=2)
    ap.add_argument("--export-state", action="store_true", default=False)
    ap.add_argument("--probe-channels", action="store_true", default=False,
                    help="ask every candidate inbound API what it holds, once a turn")
    ap.add_argument("--campus-specialist", action="store_true", default=False,
                    help="move one citizen into a Campus specialist slot in cities that "
                         "already hold a Library (at most one per city per firing)")
    ap.add_argument("--probe-citizens", action="store_true", default=False,
                    help="ask whether this UI context may assign a citizen to a district "
                         "(read-only; emits civvis_citizen_probe and issues no command)")
    ap.add_argument("--orders-db", default=None,
                    help="SQLite file offered to the mod via ATTACH as the inbound channel; "
                         "defaults to <run-dir>/orders.sqlite")
    ap.add_argument("--tile-export-every", type=int, default=25,
                    help="turns between map exports (turn 1 always exports)")
    ap.add_argument("--no-explore-unassigned", dest="explore_unassigned",
                    action="store_false", default=True,
                    help="leave units CIVVIS did not order standing still")
    ap.add_argument("--no-great-people", dest="great_people",
                    action="store_false", default=True,
                    help="leave earned Great People standing instead of "
                         "walking them to a legal plot and activating them")
    ap.add_argument("--no-automate-stuck-builders", dest="automate_stuck_builders",
                    action="store_false", default=True,
                    help="leave a builder idle when CIVVIS's improvement is refused")
    ap.add_argument("--emergency-wall-radius", type=int, default=3,
                    help="how close a VISIBLE enemy must be, in tiles, before the "
                         "emergency wall override takes a city's queue away from "
                         "CIVVIS. A city already taking damage overrides at any "
                         "distance. This half of the gate used to be unbounded: "
                         "160 overrides on 2026-08-11 ran at a median enemy "
                         "distance of 6 and up to 14, 94%% of them with zero "
                         "damage. Pass a large value to restore that behaviour "
                         "and withhold the fix.")
    ap.add_argument("--stall-rescues", type=int, default=3,
                    help="times to try dismissing a blocking screen before giving up")
    # ⚠ A FALSE STALL IS NOT FREE. The rescue SWEEPS CLICKS across the game window,
    # which is safe only because a stall means a leader screen is up; on a live map a
    # click selects a unit and the next one orders it to move. Overnight on 2026-07-31
    # a contended machine took ~100 s a turn for long stretches, which puts 240 s at
    # under three quiet turns. 420 s is four, and the cost of the longer window is a
    # few extra minutes on a genuinely wedged run against the cost of the shorter one
    # being clicks landing on a live game.
    ap.add_argument("--stall-seconds", type=float, default=420.0,
                    help="give up on a run that has emitted nothing for this long")
    # ⚠ A DIFFERENT DEATH FROM `--stall-seconds`, and now the common one. That flag
    # watches for SILENCE; this one watches for a turn that stops advancing while
    # events keep arriving. Run civvis-20260802T033552Z wedged on the World Congress
    # at turn 209 with `events.jsonl` still growing every poll, so the silence
    # watchdog could never fire and the attempt sat until a human looked at it.
    #
    # 480s rather than the 420s of its sibling: a real turn can legitimately take a
    # while late in a large game (rival AI turns, a long animation), and this one
    # kills a run that is still emitting, so it must be the more patient of the two.
    ap.add_argument("--frozen-seconds", type=float, default=480.0,
                    help="give up on a run whose TURN has not advanced for this "
                         "long, even though it is still emitting events")
    ap.add_argument("--civvis-decides", action="store_true", default=False,
                    help="CIVVIS makes every decision; the mod only actuates")
    ap.add_argument("--civvis-bin", default=None,
                    help="civvis_orders binary; defaults to target/release/civvis_orders")
    ap.add_argument("--civvis-victory", default=DEFAULT_CIVVIS_VICTORY,
                    choices=["domination", "science", "score", "civvis"],
                    help="victory objective passed to the supervised CIVVIS worker; "
                         "defaults to non-religious Science")
    ap.add_argument("--civvis-strategy", default=DEFAULT_CIVVIS_STRATEGY,
                    help="rated CIVVIS strategy name; empty keeps stock AdvancedAi. "
                         "auto is an uncalibrated opt-in")
    # ⚠ This flag existed on `civ6_brain.py` and had no route here, so the only way
    # to turn it on was to start a SECOND brain beside this one — which is exactly
    # what `civ6_civvis_climb.py` did, and the two then raced over one orders.sqlite.
    # A decider option with no path through its own launcher grows a second launcher.
    ap.add_argument("--civvis-war-from-plan", action="store_true", default=False,
                    help="declare on CIVVIS's plan target, since a board rebuilt "
                         "each turn can never mature a casus belli")
    # Same lesson one flag up: `civ6_brain.py` has taken `--github-refresh-seconds`
    # all along, and with no route through this launcher a pinned batch was never
    # pinned — the brain re-execs itself onto every origin/main advance mid-game.
    ap.add_argument("--civvis-refresh-seconds", type=float, default=None,
                    help="forwarded to the decision worker as "
                         "--github-refresh-seconds; 0 freezes the decider on its "
                         "launch revision for the whole run. Default leaves the "
                         "brain's own live-upgrade cadence alone")
    ap.add_argument("--governor-appoint", action="store_true", default=False,
                    help="spend governor titles (KNOWN to segfault the Game Core)")
    ap.add_argument("--governor-assign", action="store_true", default=False,
                    help="post governors to cities (untested since the crash)")
    ap.add_argument("--orders-poll-ticks", type=int, default=30,
                    help="game ticks between SQL polls for orders")
    ap.add_argument("--orders-wait-polls", type=int, default=40,
                    help="polls to wait for THIS turn before accepting a stale answer")
    ap.add_argument("--orders-fallback-polls", type=int, default=120,
                    help="polls before giving up on CIVVIS and running the built-ins")
    ap.add_argument("--orders-max-stale", type=int, default=4,
                    help="how many turns behind a reusable CIVVIS answer may be")
    ap.add_argument("--window-vfrac", type=float, default=1.0,
                    help="share of screen height for the game window; 0.5 puts "
                         "it in a quadrant so CIVVIS can own the other half")
    ap.add_argument("--announcement-seconds", type=float, default=1.0)
    ap.add_argument("--era-announcement-seconds", type=float, default=0.5)
    # ⚠ Deliberately NOT tied to --announcement-seconds. Every other screen is made
    # fast because something is waiting behind it; nothing is waiting behind this
    # one, because the game is over. The operator's standing brief asks for ten
    # seconds on the final screen and this is the only knob that can grant it.
    ap.add_argument("--end-game-seconds", type=float, default=10.0,
                    help="how long the victory/defeat screen is held before the "
                         "autoclose shim dismisses it")
    ap.add_argument("--survey", action="store_true", default=True)
    ap.add_argument("--no-survey", dest="survey", action="store_false")
    ap.add_argument("--survey-enums", action="store_true",
                    help="dump every action enum this build defines (one-off)")
    ap.add_argument("--start-delay-frames", type=int, default=240)
    ap.add_argument("--tick-frames", type=int, default=12)
    ap.add_argument("--startup-timeout", type=float, default=420.0)
    ap.add_argument("--host-timeout", type=float, default=300.0)
    ap.add_argument("--load-wait", type=float, default=90.0)
    ap.add_argument("--load-save", default=None,
                    help="load this visible single-player save instead of creating a game")
    ap.add_argument("--timeout", type=float, default=7200.0)
    ap.add_argument("--timeout-ceiling", type=float, default=None,
                    help="hard wall-clock bound, in seconds. Between --timeout "
                         "and this, a run that is still advancing and can "
                         "still REACH --max-turns at the rate it has managed "
                         "keeps going; one that cannot is stopped at --timeout "
                         "exactly as before. Defaults to 1.5x --timeout. Pass "
                         "the same value as --timeout to disable extensions.")
    ap.add_argument("--report-every", type=int, default=5)
    ap.add_argument("--lock-wait", type=float, default=0.0,
                    help="seconds to wait for another run to finish")
    ap.add_argument("--focus-every", type=float, default=15.0,
                    help="seconds between raising the game window (0 disables)")
    ap.add_argument("--status", action="store_true")
    args = ap.parse_args(raw_argv)
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
