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
from civ6_control import launcher, vision, watch  # noqa: E402

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

# "Play Now" is the last entry of the Single Player submenu, and where that
# lands depends on how many entries there are: "Resume Game" only appears once
# a game has been played and "Load Game" only once a save exists, so the same
# click is right in one run and one row off in the next. Rather than predict the
# count, the submenu is scanned from the most likely row outwards and each click
# is checked. Rows are about 0.030 of the window's height apart.
#
# A wrong click here is cheap: "Scenarios", "Create Game" and "Load Game" open a
# screen that Escape backs out of, and "Resume Game" starts a game, which is all
# the bootstrap wants anyway.
SUBMENU_X = 0.528
PLAY_NOW_Y = [0.614, 0.630, 0.598, 0.646, 0.582, 0.662]

# "Create Game" is the third entry from the bottom of that submenu. With a save
# on disk and a game to resume -- which is true from the first run onwards --
# the submenu has five rows and this is where it lands.
CREATE_GAME_Y = [0.573, 0.555, 0.540, 0.601]

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


def focus_game() -> None:
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

    for attempt, create_y in enumerate(CREATE_GAME_Y, start=1):
        focus_game()
        time.sleep(2.0)
        bounds = game_window()
        if bounds is None:
            print("no game window found", file=sys.stderr)
            return False
        x, y, w, h = bounds
        # Make the window key with a click on empty artwork, well clear of the
        # menu: the first click on a background window is consumed activating
        # it, and spending that click on a menu entry loses the entry.
        click_at(int(x + w * 0.15), int(y + h * 0.85))
        time.sleep(1.5)

        click_menu("single_player", bounds)
        time.sleep(2.0)
        # Look at the submenu rather than assume its shape. Which entries it
        # has depends on whether there is a save and a game to resume, so the
        # position of "Create Game" moves between runs; the measured fraction
        # is only the fallback for when the screen cannot be read.
        submenu = run_dir / f"submenu-attempt{attempt}.png"
        screenshot(submenu)
        seen = vision.create_game_row(submenu, bounds) if vision.available() else None
        target = seen if seen is not None else create_y
        print(f"create game row: {target:.4f}"
              f" ({'read from the screen' if seen is not None else 'measured fallback'})")
        click_at(int(x + w * SUBMENU_X), int(y + h * target))
        time.sleep(2.5)
        screenshot(run_dir / f"create-attempt{attempt}.png")
        configure_and_start(bounds, args, run_dir)
        if started(verify_s):
            return True
        if not env.game_pids():
            print("the game exited while starting", file=sys.stderr)
            return False
        # That row was not Create Game. Back out and try the next candidate.
        click_at(int(x + w * BACK[0]), int(y + h * BACK[1]))
        time.sleep(1.0)
        press_escape(1)
        time.sleep(1.5)
        print(f"attempt {attempt}: row {create_y} was not Create Game", file=sys.stderr)
    return False


def press_escape(times: int = 2) -> bool:
    """Dismiss the load screen, for when the mod's own dismissal does not land.

    The agent raises the two dismissals the shipped screen offers from Lua and
    they do work; this stays as the fallback because a run stuck here writes no
    log line at all and looks exactly like a slow map. It is deliberately not
    on the normal path: Escape during play opens the pause menu.
    """
    focus_game()
    time.sleep(1.0)
    ok = True
    for _ in range(times):
        result = subprocess.run(["cliclick", "kp:esc"], capture_output=True)
        ok = ok and result.returncode == 0
        time.sleep(0.8)
    return ok


def play(args: argparse.Namespace) -> int:
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
                      f"blocker={event.get('blocker')} | {summary}")
        elif kind == "blocked":
            print(f"[turn {event.get('turn')}] blocked on {event.get('blocker')} "
                  f"({event.get('attempts')} attempts)")
        elif kind in ("victory", "defeat", "error"):
            print(f"[{kind}] {json.dumps(event, sort_keys=True)}")
            if kind in ("victory", "defeat"):
                state["outcome"] = event

    def finished(event: dict) -> bool:
        return event.get("kind") in ("victory", "defeat")

    if not bootstrap_game(tail, record, run_dir, args):
        print("could not start a game from the main menu", file=sys.stderr)
        return 5
    print("in a configured game; the agent holds the seat from here")

    reason = watch.follow(tail, args.timeout, record, stop_when=finished)
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
    ap.add_argument("--status", action="store_true")
    args = ap.parse_args(argv)

    if args.status:
        return status()
    if args.tag is None:
        args.tag = (args.difficulty.replace("DIFFICULTY_", "").lower()
                    + "-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ"))
    return play(args)


if __name__ == "__main__":
    sys.exit(main())
