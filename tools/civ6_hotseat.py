#!/usr/bin/env python3
"""Play real Civilization VI against CIVVIS's flagship: a hotseat game.

One command starts Civ VI on this Mac, sets up a Multiplayer -> Hot Seat game
and plays it: **you** play seat 0 ("Player 1", the host row), and CIVVIS's
flagship strategy ``jack`` (the deployment genome, `src/strategies.rs` ROSTER
row ``jack``) plays every other seat, each with its own decision worker. Those
seats are named "CIVVIS jack 1..N" in the game.

    tools/civ6_hotseat.py                      # 4 players, Tiny Pangaea, Online, Prince
    tools/civ6_hotseat.py --leader LEADER_SALADIN --difficulty DIFFICULTY_KING
    tools/civ6_hotseat.py --duel --name dev --leader LEADER_GILGAMESH   # 1v1, see below
    tools/civ6_hotseat.py --stop               # stop the automation, leave the game up
    tools/civ6_hotseat.py --stop --quit-game   # ...and close Civ VI (it saves every 10 turns)
    tools/civ6_hotseat.py --stop --quit-game --resign   # ...and give the game up (--name: recorded)
    tools/civ6_hotseat.py --resume             # load the newest hotseat save and play on
    tools/civ6_hotseat.py --attach             # pick a running hotseat game back up
    tools/civ6_hotseat.py --status

How a turn goes. Civ VI's hotseat plays the seats in order. It plays like
single player: from the moment you end your turn a cover hides the screen
("CIVVIS is taking its turn") until your next turn begins, and then your
hand-over "Start Turn" is pressed for you, the cover lifts, and you are on your
own map where you left it. Esc on the cover opens the game menu (save, load,
quit). While it is up the jack seats' sounds are silenced and the soundtrack
plays on (`--cover-sound silent` for no sound at all, `all` for everything),
and a bar on it shows about how long is left, from how long your recent waits
took (`--no-cover-timer` for none). On YOUR turn the terminal prints a banner (and macOS shows a
notification): nothing of ours clicks, types, moves the camera or closes a
screen until you end your turn. `--no-cover` shows the jack seats' turns as
they play; `--keep-handover` leaves Civ VI's hand-over screen for you to press.
On a jack seat's turn
the mod exports that seat's board, its worker (`tools/civ6_brain.py` ->
`civvis_orders`, the same chain as the live verification seat) decides, the
orders are issued in-game, and the turn ends. Every line of what happened is in
``~/civvis-civ6-runs/hotseat/<tag>/`` (``hotseat.log``, ``turns.jsonl``, one
directory per jack seat with its ``events.jsonl``/``brain.log``/``why.log``).

Stopping. ``--stop`` (or Ctrl-C in the harness's terminal) stops the decision
workers and the harness and leaves Civ VI and the control mod up: until a
worker is back, a jack seat's turn waits out the mod's order poll and then
plays on the mod's own fallback ladder, so save or ``--attach`` again soon.
``--stop --quit-game`` also closes Civ VI (when this harness started it) and
removes the mod; closing Civ VI yourself ends the harness the same way.

Saves and resuming. Civ VI autosaves, and the mod also saves ``<tag>-T<turn>``
every ``--save-every`` turns (Saves/Hotseat). ``--resume`` starts Civ VI and
loads the newest hotseat save, autosave or quicksave (``--resume PATH`` for a
particular one) through the Hot Seat page's Load Game, then plays on exactly
like a new game: the game the save is of, as the run that played it set it up
(its ``table.json``, or for an older run its ``config.json``): your name and
leader, the agent on each seat, the cover, the turn limit, when it began. A
flag given again wins (``--agents``, ``--name``, the cover's), except that the
table itself is the save's (players, map, speed, difficulty, your leader). A
person's bare ``--resume`` passes over a newer save a test made. ``--attach``
picks up a hotseat game that is still running with the mod (after ``--stop``
or a closed terminal).

A duel, under your name, for your Elo. ``--duel`` is one against one: two
players on a Duel map, the other seat played by the strongest roster agent
(``--agents strongest``: the highest Elo in ``data/strategy_elo.json``; a seat's
agent is the live decider in that agent's victory lane, named "CIVVIS
<Handle>"). ``--agents holy,tourist`` names the agents seat by seat at any
table. ``--name dev`` puts your name on seat 0 (the hand-over screen and the
score board say it), and when the game ends with a victory or an elimination
it is put on the people's record on the ledger branch
(``tools/people_elo.py record``), which the site's ELO Rankings page rates you
from, among the agents: the moment the game is decided, not when Civ VI is
closed. A game you give up has no ending of the game's own: ``--stop --resign``
(with ``--quit-game`` to close Civ VI too) records it as CIVVIS's, and
``tools/people_elo.py record <run_dir> --winner civvis`` (or ``person``) says
who won one that was simply left. ``--hold-start`` sets the staging room up and leaves START GAME to you
(to pick Teams, which have no text to read).

Both sides, nobody at the Mac. ``--self-play`` seats a CIVVIS agent on every
seat (``--civvis-seats`` = ``--players``): ``--duel --self-play`` is the same
duel with the flagship in your seat and the strongest agent across the table
(``--agents jack,holy``, seat 0 first), played to its ending, the result in
``summary.json``, Civ VI closed after it. It is the rehearsal for the game you
play: the same setup, staging room, hand-over screen and saves. It takes no
``--name``: a game CIVVIS played is never put on the people's record.

Known limits: you are always seat 0 (the host row). Diplomacy between you and a
jack seat works the way hotseat diplomacy does (proposals wait for the other
seat's turn). The setup is driven through Civ VI's menus by screen reading, so
keep hands off the mouse for the minute it takes; the window may be moved
afterwards. Game modes are left as Civ VI shows them (the in-game seat record
says which are on).

Testing: ``--auto-human --turns N`` replaces you with a stub (the mod's own
fallback ladder playing seat 0) and stops after turn N with a per-seat report.
``--auto-human gated`` keeps seat 0 "the person" for the cover and the pressed
hand-over, so a person's game runs with nobody at the Mac; only the stub's own
announcements are closed for it, and its quiet turns ended, as nobody is there.

You set up the game (``--manual-lobby``). Nothing touches the screen: no
launch, click, keystroke, screenshot, OCR or window move, and none of the
modules that could are even loaded, so it needs no Screen Recording or
Accessibility permission, no Pillow, no Swift compiler and no ``cliclick``.

    tools/civ6_hotseat.py --manual-lobby

It installs the mod, prints what to do (start Civ VI -> Multiplayer -> Hot
Seat -> make every player CIVVIS should play Human, the first row is you ->
Start Game), and reads the game you made back from the game itself: every
Human row after yours is a CIVVIS seat (up to five), AI rows are Civ VI's own.
A stalled CIVVIS turn is reported here with what to do, never clicked.
``--stop`` leaves the game alone, and the mod is removed only when Civ VI is
not running. Installed and with no harness running, the mod does nothing: no
screen closed, nothing answered, ordered, re-defaulted or logged (see "The
person-facing install" in docs/CIV6_COMPUTER_CONTROL.md).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import shutil
import signal
import sqlite3
import subprocess
import sys
import threading
import time
import io
from contextlib import closing, redirect_stdout
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE / "civ6_control"))
#: `--manual-lobby` is screen-free from the first import on: with this set
#: before `civ6_control` loads, every module that clicks, types, captures,
#: reads the screen or moves a window imports as a stand-in that refuses all
#: use, Pillow is never loaded, and the decision workers inherit the same
#: (`civ6_control/screen_free.py`).
SCREEN_FREE_FLAG = "--manual-lobby"
if SCREEN_FREE_FLAG in sys.argv[1:]:
    os.environ["CIVVIS_SCREEN_FREE"] = "1"
import civ6_env as env  # noqa: E402
import civ6_setup  # noqa: E402
import civ6_play as play  # noqa: E402
from civ6_control import gamelock, launcher, operator_presence, popup_clear, watch  # noqa: E402
from civ6_control import install as modinstall  # noqa: E402
from civ6_control import screen_free  # noqa: E402
from civ6_control.orders import reset_orders_db  # noqa: E402

RUN_ROOT = Path(os.environ.get("CIVVIS_HOTSEAT_RUNS", "")
                or Path.home() / "civvis-civ6-runs" / "hotseat")
#: The running harness, for `--stop`, `--status` and `--attach`.
CURRENT = RUN_ROOT / "current.json"
HUMAN_SEAT = 0
#: The flagship. `civvis_orders` plays the deployment genome when it is given no
#: `--strategy` (it refuses the flag outright: the league is retired, #2357), and
#: the deployment genome IS `jack` (`strategies.rs`: chassis Deployment, no genes).
STRATEGY = "jack"
SPEED_TURNS = {"GAMESPEED_ONLINE": 250, "GAMESPEED_QUICK": 330,
               "GAMESPEED_STANDARD": 500, "GAMESPEED_EPIC": 750,
               "GAMESPEED_MARATHON": 1500}
#: Binaries tried in order when `--orders-bin` is not given: the verification
#: lane's current build of main, then its newest frozen snapshot, then this
#: checkout's release build. The chosen file is copied into the run directory so
#: a rebuild elsewhere cannot change the program mid-game.
VERIFICATION_BIN = (Path.home() / "civvis-verification-clean-runtime" / "target"
                    / "release" / "civvis_orders")
SNAPSHOTS = Path.home() / "civvis-civ6-runs" / "decider-snapshots"
CHECKOUT_BIN = HERE.parent / "target" / "release" / "civvis_orders"
#: Frozen by `tools/package_hotseat.py`, this file runs from a bundle that
#: ships its own decider; it is tried before everything above. Windows needs
#: the `.exe`: CreateProcess appends it to a name without one, so a frozen copy
#: named `civvis_orders` would not start.
FROZEN = bool(getattr(sys, "frozen", False))
ORDERS_NAME = "civvis_orders.exe" if os.name == "nt" else "civvis_orders"
BUNDLED_BIN = Path(getattr(sys, "_MEIPASS", HERE)) / ORDERS_NAME if FROZEN else None
#: How the bundle's one program is asked to be a decision worker
#: (`tools/civvis_hotseat_app.py`). The script's own name on purpose: the
#: stale-worker check and `gamelock` recognise a worker by it.
WORKER_COMMAND = "civ6_brain.py"


def utc() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


class Log:
    """Print and keep: the terminal is the operator's view, the file the record."""

    def __init__(self, path: Path | None = None):
        self.handle = path.open("a", buffering=1) if path else None

    def __call__(self, message: str) -> None:
        line = f"[{datetime.now():%H:%M:%S}] {message}"
        print(line, flush=True)
        if self.handle is not None:
            self.handle.write(line + "\n")


# --------------------------------------------------------------- the settings

#: The options files a hotseat launch may touch: only the front end's. A person
#: plays this game, so how it plays and looks is theirs -- historic-moment
#: animations, the autosave cadence and the shadow passes the verification seat
#: cuts all stay as they set them (before 2026-11-19 a hotseat run applied the
#: verification lane's whole table and left autosave at every fourth turn and
#: historic moments off afterwards). The intro video and the two one-time
#: front-end acknowledgements remain, because the setup reads the main menu
#: and those dialogs cover it.
PERSON_OPTIONS = {"AppOptions.txt": dict(civ6_setup.VERIFICATION_OPTIONS["AppOptions.txt"])}


def apply_person_options(log: Log) -> dict:
    """The front-end options a launch needs, applied while Civ VI is closed."""
    if env.game_pids():
        return {}
    try:
        applied = civ6_setup.apply_verification(env.user_dir(), PERSON_OPTIONS)
    except Exception as error:  # noqa: BLE001 - a cosmetic cut must never cost a launch
        log(f"could not apply the front-end options: {error}")
        return {}
    for name, changes in applied.items():
        for key, (old, new) in changes.items():
            if old is not None:
                log(f"options: {name} {key} {old} -> {new}")
    return applied


def jack_seats(players: int, civvis_seats: int | None = None) -> list[int]:
    """The seats CIVVIS plays. Unattended (`--civvis-seats N`, operator
    2026-11-21: "2 civvis agents and 4 deity ais"): seats 0..N-1, no person at
    the Mac, and the rest of the table Firaxis AI."""
    if civvis_seats:
        return list(range(civvis_seats))
    return [seat for seat in range(players) if seat != HUMAN_SEAT]


def agent_seats(players: int, auto_human: bool, civvis_seats: int | None = None) -> list[int]:
    """Every seat with an agent context: the jack seats, and the test stub."""
    if civvis_seats:
        return jack_seats(players, civvis_seats)
    return jack_seats(players) + ([HUMAN_SEAT] if auto_human else [])


#: Roster agents a live seat can play: each is the deployment chassis with no
#: genes, so the live decider IS that agent when given the agent's victory lane
#: (`src/strategies.rs` ROSTER: jack = no target, holy = Religion, ...).
AGENT_LANES = {"jack": "civvis", "holy": "religious", "tourist": "culture",
               "moonshot": "science", "siege": "domination", "swing": "diplomatic"}
AGENT_HANDLES = {"jack": "JackOfAllTrades", "holy": "HolyRoller", "tourist": "TouristTrap",
                 "moonshot": "MoonshotMaven", "siege": "SiegeTheDay", "swing": "SwingVote"}


#: `--cover-sound`: what is heard while the cover is up (CivvisHotseatCover.lua,
#: `BUSES`); the first is the default.
COVER_SOUNDS = ("music", "silent", "all")
COVER_SOUND = COVER_SOUNDS[0]
#: `--fast-animations [N]`: the engine's debug timescale for the CIVVIS seats'
#: covered turns. The default is the ladder's (`deploy/live-mod-arms.txt`
#: `debug-timescale-3`); the mod takes 1 < N <= 4 (`CivvisClock.fastWant`), the
#: scales the ladder has played (`civ6_play.TREE_MOD_ARMS`).
FAST_ANIMATIONS = 3.0
FAST_ANIMATIONS_MAX = 4.0
#: `--agents strongest`: the roster agent the ELO Rankings page rates highest.
STRONGEST = "strongest"
PUBLISHED_ELO = HERE.parent / "data" / "strategy_elo.json"
#: A person's name as the game and the people's record take it (`--name`).
PERSON_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9 _.-]{0,23}")


def strongest_agent(published: Path = PUBLISHED_ELO) -> str:
    """The roster agent with the highest published Elo (`data/strategy_elo.json`,
    the fit the ELO Rankings page shows); jack when there is no fit to read."""
    try:
        ratings = json.loads(published.read_text(encoding="utf-8"))["ratings"]
        rated = {agent: float(ratings[agent]["elo"]) for agent in AGENT_LANES if agent in ratings}
    except (OSError, ValueError, KeyError, TypeError):
        rated = {}
    return max(rated, key=rated.get) if rated else STRATEGY


def checkout_revision(checkout: Path = HERE.parent) -> str | None:
    """The revision this checkout is at: the harness's, and the decider's when
    it is this checkout's own build."""
    try:
        shown = subprocess.run(["git", "-C", str(checkout), "rev-parse", "--short=8", "HEAD"],
                               capture_output=True, text=True, check=False, timeout=10)
    except (OSError, subprocess.SubprocessError):
        return None
    return shown.stdout.strip() or None if shown.returncode == 0 else None


def record_person_game(run_dir: Path, log, resigned: bool = False) -> None:
    """A named person's finished game goes on the people's record
    (`tools/people_elo.py record`), which the ELO Rankings page rates them
    from. In its own process: a push that fails must not take the harness's
    cleanup with it, and what it printed is in the log either way. `resigned`:
    the person gave the game up (`--stop --resign`), so it is CIVVIS's."""
    command = [sys.executable, str(HERE / "people_elo.py"), "record", str(run_dir)]
    if resigned:
        command += ["--winner", "civvis", "--victory", "resignation"]
    try:
        done = subprocess.run(command, capture_output=True, text=True, check=False, timeout=300)
    except (OSError, subprocess.SubprocessError) as error:
        log(f"the game was NOT put on the people's record: {error}")
        return
    for line in (done.stdout + done.stderr).strip().splitlines():
        log(f"[people] {line}")
    if done.returncode != 0:
        log(f"the game was NOT put on the people's record (exit {done.returncode}); "
            f"`{' '.join(['tools/people_elo.py', *command[2:]])}` tries again")


def read_people_flags(args: argparse.Namespace) -> str | None:
    """`--name` checked and `--agents` read into roster agents, for a launched
    table and a `--manual-lobby` one alike; what is wrong, if anything. Before
    2026-11-28 `--manual-lobby` skipped both: its `--agents holy` named the
    seats `h`, `o`, `l`, `y` and its seats were recorded under no name check."""
    if args.name is not None and not PERSON_NAME.fullmatch(args.name):
        return ("--name: 1 to 24 letters, digits, spaces, . _ or -, starting with a "
                "letter or digit")
    if args.agents and isinstance(args.agents, str):
        agents = [a.strip().lower() for a in args.agents.split(",") if a.strip()]
        agents = [strongest_agent() if a == STRONGEST else a for a in agents]
        bad = [a for a in agents if a not in AGENT_LANES]
        if bad:
            return f"--agents: {', '.join(bad)} not one of {', '.join(AGENT_LANES)}"
        args.agents = agents
    return None


def person_name(args: argparse.Namespace) -> str | None:
    """The person at seat 0, when the table has one and `--name` named them."""
    if getattr(args, "civvis_seats", None) or getattr(args, "auto_human", None):
        return None
    return getattr(args, "name", None) or None


def seat_agents(args: argparse.Namespace, seats: list[int]) -> dict[int, str]:
    """Seat -> roster agent: `--agents` in seat order, else jack everywhere."""
    agents = list(getattr(args, "agents", None) or [])
    return {seat: (agents[i] if i < len(agents) else "jack") for i, seat in enumerate(seats)}


def seat_lanes(args: argparse.Namespace, seats: list[int]) -> dict[int, str]:
    """Seat -> victory lane its decider plays: the agent's own when `--agents`
    names one, else `--victory`."""
    if not getattr(args, "agents", None):
        return {seat: args.victory for seat in seats}
    return {seat: AGENT_LANES[agent] for seat, agent in seat_agents(args, seats).items()}


def seat_name(index: int, agent: str | None = None) -> str:
    if agent:
        return f"CIVVIS {AGENT_HANDLES.get(agent, agent)}"
    return f"CIVVIS jack {index}"


def victory_switch_key(victory: str) -> str:
    """`VICTORY_DIPLOMATIC` -> `diplomatic`, the key of the seat event's
    `victories` table (CivvisControlAgent.lua, `Game.IsVictoryEnabled`)."""
    return victory.removeprefix("VICTORY_").lower()


def default_max_turns(speed: str) -> int:
    return SPEED_TURNS.get(speed, 500)


def mod_path(path: Path) -> str:
    """A path as the mod will put it into SQL (`OrdersDb`, its ATTACH target).

    ⚠ The mod's `sqlSafe` (CivvisControlAgent.lua) strips every apostrophe and
    every backslash, so a Windows path would reach ATTACH as
    `C:Usersnamecivvis-civ6-runs...` and the mod would open some other file.
    Windows and SQLite both accept forward slashes, so on Windows it goes in
    with those; elsewhere it is `str(path)`, as it always was. A path holding an
    apostrophe cannot be attached on any system (`civvis_hotseat_app` doctor).
    """
    return path.as_posix() if os.name == "nt" else str(path)


def hotseat_config(args: argparse.Namespace, run_dir: Path, orders_db: Path) -> dict:
    """The mod's settings: the live seat's own, re-pointed at a hotseat game.

    Built from `civ6_play`'s parser defaults so every jack seat plays with the
    exact controller settings the verification seat uses; only what hotseat
    changes is overridden below.
    """
    flags = ["--tag", args.tag, "--difficulty", args.difficulty,
             "--map", args.map, "--map-size", args.map_size, "--speed", args.speed,
             "--max-turns", str(args.max_turns), "--civvis-decides",
             "--civvis-victory", args.victory, "--orders-db", mod_path(orders_db)]
    if args.leader:
        flags += ["--leader", args.leader]
    # `--play-flags`: civ6_play switches every jack seat plays with (Tournament 6
    # passes the policy arms with a measured gain, e.g. --dv-join-winner).
    flags += shlex.split(getattr(args, "play_flags", None) or "")
    # The live ladder's decider genes (`live_genome`): through `--civvis-with`,
    # as the ladder hands them to civ6_play, so the mod keys some of them
    # switch on (`CultureAllies`, `ScienceCongressRacer`, ...) follow them
    # exactly as they do there (`civ6_play.build_config`).
    for tag in getattr(args, "treatments", None) or []:
        flags += ["--civvis-with", tag]
    settings = play.build_parser().parse_args(flags)
    if getattr(args, "genome", None) == "live":
        # ...and the mod arms the ladder's tree lists (`deploy/live-mod-arms.txt`,
        # `civ6_play.apply_tree_mod_arms`: the release of a stalled move, the
        # 20 ms orders peek). A debug timescale it lists is set aside below
        # with the ladder's other timescales: the game is a person's.
        with redirect_stdout(io.StringIO()):
            play.apply_tree_mod_arms(settings, MOD_ARMS_FILE)
    if not args.leader:
        settings.leader = None
    # `--victories-off`: civ6_play takes this list from the host policy
    # (`CIVVIS_VICTORIES_OFF`), which a hotseat table does not read; the table
    # names its own, and `Monitor` reads the started game's switches back.
    settings.victories_off = list(getattr(args, "victories_off", None) or [])
    config = play.build_config(settings)
    civvis_seats = getattr(args, "civvis_seats", None)
    seats = jack_seats(args.players, civvis_seats)
    # A person plays seat 0 (or `--auto-human gated` stands in for them): their
    # game plays like single player unless they ask otherwise.
    person = not civvis_seats and args.auto_human != "stub"
    config.update({
        "Hotseat": True,
        "HotseatSeats": seats,
        # `--auto-human gated` keeps seat 0 the operator's in every gate, with
        # the stub playing it once its hand-over is pressed: the operator path,
        # testable without a person. Unattended, no seat is a person's.
        "HotseatHumanSeats": ([] if civvis_seats or args.auto_human == "stub" else [HUMAN_SEAT]),
        "HotseatStubSeats": [HUMAN_SEAT] if args.auto_human and not civvis_seats else [],
        "HotseatNames": {str(seat): seat_name(i, getattr(args, "agents", None) and agent)
                         for i, (seat, agent) in enumerate(seat_agents(args, seats).items(), 1)},
        "HotseatSeatAgents": ({str(k): v for k, v in seat_agents(args, seats).items()}
                              if getattr(args, "agents", None) else {}),
        # Each seat's own lane, for its context in the mod (`hotseat_seat_config`):
        # the lane its decision worker is started with (`seat_lanes`).
        "HotseatSeatLanes": ({str(k): v for k, v in seat_lanes(args, seats).items()}
                             if getattr(args, "agents", None) else {}),
        # `--name`: the hand-over screen names the person's seat with it.
        "HotseatPersonName": person_name(args) or "",
        "HotseatPlayers": args.players,
        "HotseatSaveEvery": args.save_every,
        "HotseatSaveName": args.tag,
        # The cover over every turn that is not the person's
        # (CivvisHotseatCover.lua), and their hand-over pressed for them
        # (CivvisControlAutoClose.lua, `CivvisHotseatScreen.autoStart`).
        "HotseatCover": person and not getattr(args, "no_cover", False),
        "HotseatAutoStart": person and not getattr(args, "keep_handover", False),
        # Diagnostic: CIVVIS-turn screens keep their shipped queueing under the
        # cover (CivvisControlAutoClose.lua, `CivvisCoverQueue`).
        "HotseatCoverQueue": person and not getattr(args, "no_cover_queue", False),
        # What is heard through the cover (`--cover-sound`): `music` silences a
        # jack seat's units, fights, chimes and voices and lets the soundtrack
        # play on, `silent` turns everything down, `all` leaves the sound alone.
        # And the wait bar with its time left (`--no-cover-timer`). Both are the
        # cover's own: without it there is nothing to quiet or to wait behind.
        "HotseatCoverSound": (getattr(args, "cover_sound", None) or COVER_SOUND
                              if person and not getattr(args, "no_cover", False) else "all"),
        "HotseatCoverTimer": (person and not getattr(args, "no_cover", False)
                              and not getattr(args, "no_cover_timer", False)),
        # `--fast-turns` (off unless asked; tests only so far): a covered
        # hand-over is pressed on its first frame and landed orders are peeked
        # every 20 ms (CivvisControlAutoClose.lua `firstPause`, the Heartbeat's
        # peek, the agent's `CivvisQueue.peekSeconds`).
        "HotseatFastTurns": bool(getattr(args, "fast_turns", False)),
        # The operator's seat's difficulty, and every jack seat's: hotseat
        # difficulty is per player (the staging room's Difficulty column).
        "PlayerDifficulty": args.difficulty,
        "Leader": args.leader,
        "HumanPlayers": civvis_seats or args.players,
        # Nothing that changes how the game runs for the person at it: no debug
        # timescale, no VSync A/B, no camera (the map view stands down itself).
        "DebugTimeScale": None,
        "DebugTimeScaleAB": None,
        "VSyncABTurns": None,
    })
    # `--fast-animations` (off unless asked; tests only so far): the CIVVIS
    # seats' turns under the cover at the console's `timescale N`, every
    # person's turn at 1 (CivvisControlAgent.lua `CivvisClock.fastApply` and
    # its resets; the cover and the HUD guard it). Only over a person's covered
    # seat, and set only when asked, so every other install is as it was.
    fast = getattr(args, "fast_animations", None)
    if fast and config["HotseatCover"]:
        config.update({
            "HotseatFastAnimations": float(fast),
            # `--fast-animations-rollover`: through the game's own processing
            # after the last CIVVIS seat too, until the person's turn begins.
            "HotseatFastRollover": bool(getattr(args, "fast_animations_rollover", False)),
        })
    if args.auto_human:
        config["HotseatNames"][str(HUMAN_SEAT)] = "Test stub"
        # `--stand-in-deal` / `--stand-in-deal-hold`: the check games' deals
        # from and to the stand-in (CivvisControlAgent.lua
        # `CivvisTrade.standInDeals`, `standInHolds`). Test tables only.
        deals = stand_in_deals(str(getattr(args, "stand_in_deal", None) or "").split())
        if deals:
            config["StandInDeals"] = deals
        hold = getattr(args, "stand_in_deal_hold", None)
        if hold:
            config["StandInDealHoldSeconds"] = float(hold)
        if getattr(args, "stand_in_deal_answer", None):
            config["StandInDealAnswer"] = args.stand_in_deal_answer
    return config


#: One `--stand-in-deal` deal, TURN:SEAT:VERB: the `deal` order grammar's verb
#: (`PROPOSE;give=GOLD=40;get=`); SEAT `*` is any CIVVIS seat. The flag takes
#: several, space-separated, in one argument (an `append` flag would not survive
#: `typed_options`, which `--resume` reads the command line with).
STAND_IN_DEAL = re.compile(r"^(\d+):(\d+|\*):((?:PROPOSE|WITHDRAW)(?:;(?:give|get)=[A-Z0-9_=@#,]*)*)$")


def stand_in_deals(specs: list[str]) -> list[dict]:
    """`--stand-in-deal` specs as the mod's `StandInDeals` rows
    (`{turn, to, verb}`), in the order given. A spec that does not read raises
    ValueError naming it."""
    rows = []
    for spec in specs:
        match = STAND_IN_DEAL.match(spec.strip())
        if not match:
            raise ValueError(f"--stand-in-deal {spec!r}: expected TURN:SEAT:VERB, e.g. "
                             "8:3:PROPOSE;give=GOLD=40;get=")
        # `*`: the lowest CIVVIS seat the stand-in may ask then (-1 to the mod).
        rows.append({"turn": int(match[1]), "to": -1 if match[2] == "*" else int(match[2]),
                     "verb": match[3]})
    return rows


def fast_animations_note(args: argparse.Namespace, config: dict) -> str | None:
    """What the log says about `--fast-animations`, when it was asked for."""
    if getattr(args, "fast_animations", None) is None:
        return None
    if not config.get("HotseatFastAnimations"):
        return ("--fast-animations needs the cover over your own seat (not with --no-cover, "
                "--auto-human stub or an unattended table): off for this game")
    rollover = (", and through the game's own turn processing after them"
                if config.get("HotseatFastRollover") else "")
    return (f"fast animations: the CIVVIS seats' turns under the cover run at timescale "
            f"{config['HotseatFastAnimations']:g}{rollover}; your own turn at 1 (tests only so far)")


# --------------------------------------------------------------- the decider

def resolve_orders_bin(requested: str | None) -> Path | None:
    if requested:
        path = Path(requested).expanduser()
        return path if path.is_file() else None
    candidates = ([BUNDLED_BIN] if BUNDLED_BIN else []) + [VERIFICATION_BIN]
    if SNAPSHOTS.is_dir():
        candidates += sorted(SNAPSHOTS.glob("*/civvis_orders"),
                             key=lambda p: p.stat().st_mtime, reverse=True)
    candidates.append(CHECKOUT_BIN)
    for path in candidates:
        if path.is_file() and os.access(path, os.X_OK):
            return path
    return None


def freeze_binary(source: Path, run_dir: Path) -> Path:
    target = run_dir / ORDERS_NAME
    if target.resolve() != source.resolve():
        shutil.copy2(source, target)
    provenance = {"source": str(source), "frozen": str(target), "utc": utc()}
    snapshot = source.with_name(source.name + ".provenance.json")
    if snapshot.is_file():
        try:
            provenance["snapshot"] = json.loads(snapshot.read_text())
        except ValueError:
            pass
    # Beside the copy under the copy's name, where `civ6_brain.binary_provenance` looks.
    (run_dir / f"{ORDERS_NAME}.provenance.json").write_text(json.dumps(provenance, indent=2))
    return target


# ----------------------------------------------------------- the live genome
#
# The live ladder does not play the bare deployment genome: its supervisor
# arms the decider with `deploy/live-force-on.txt` (`--with`, each tag) and the
# mod with `deploy/live-mod-arms.txt`. Before 2026-11-28 a hotseat seat got
# neither. In the live six-player game every seat's brain.log said
# `forced=none` and every why.log the same stock genes on all five seats; 182
# of the list's 285 tags were off, `recon-replacement` among them (no seat ever
# built a Scout and by turn 16 none had seen a rival city), and a stalled move
# held seat 3 for 7.9 s of the person's wait on turn 15 that the ladder's
# `stalled-operation-release` answers at once. `--genome stock` plays the bare
# genome, as before, for comparison.

#: `--genome`: the live ladder's arms (the default), or none.
GENOMES = ("live", "stock")
#: The live list, in the order the ladder's supervisor resolves it
#: (`tools/ops/civvis-game-supervisor.sh`, `resolve_forced_arm`): the
#: `CIVVIS_WITH` environment, else the file `CIVVIS_WITH_FILE` names (an empty
#: one is the stock genome), else this host's `~/.civvis-live-force-on` while
#: it is not empty, else the checkout's own `deploy/live-force-on.txt`.
#: Where both live lists are: the checkout's `deploy/`, or in the frozen bundle
#: its own copy (`tools/package_hotseat.py` ships them under `_internal/deploy/`
#: from the revision its decider is built from), as `BUNDLED_BIN` is its own
#: decider. Before 2026-11-28 a bundle shipped neither: its log said "no live
#: list in this install", and its seats played the bare genome with no mod arms.
DEPLOY_DIR = (Path(getattr(sys, "_MEIPASS", HERE)) / "deploy" if FROZEN
              else HERE.parent / "deploy")
LIVE_FORCE_FILE = DEPLOY_DIR / "live-force-on.txt"
#: Genes armed for a person's hotseat game on top of the ladder's list
#: (`hotseat_treatments`): read only here, never by the ladder.
HOTSEAT_FORCE_FILE = DEPLOY_DIR / "hotseat-force-on.txt"
LOCAL_FORCE_FILE = Path.home() / ".civvis-live-force-on"
#: The ladder's mod arms (`civ6_play.TREE_MOD_ARMS_FILE`, the same file from a checkout).
MOD_ARMS_FILE = DEPLOY_DIR / "live-mod-arms.txt" if FROZEN else play.TREE_MOD_ARMS_FILE
#: In the run directory: what the seats were armed with, for `--attach`.
GENOME_RECORD = "genome.json"


def force_tags(text: str) -> list[str]:
    """The tags of a force-on line: comma-separated, each once, in order."""
    tags: list[str] = []
    for tag in text.replace("\n", ",").split(","):
        tag = tag.strip()
        if tag and tag not in tags:
            tags.append(tag)
    return tags


def live_treatments(environ=None, repo_file: Path | None = None,
                    local_file: Path | None = None) -> tuple[list[str], str]:
    """The genes the live ladder would arm on this host, and where they came from."""
    environ = os.environ if environ is None else environ
    repo_file = LIVE_FORCE_FILE if repo_file is None else repo_file
    local_file = LOCAL_FORCE_FILE if local_file is None else local_file

    def read(path: Path) -> str | None:
        try:
            return path.read_text(encoding="utf-8")
        except OSError:
            return None

    if environ.get("CIVVIS_WITH"):
        return force_tags(environ["CIVVIS_WITH"]), "the CIVVIS_WITH environment"
    if environ.get("CIVVIS_WITH_FILE"):
        named = environ["CIVVIS_WITH_FILE"]
        return force_tags(read(Path(named)) or ""), f"CIVVIS_WITH_FILE ({named})"
    local = read(local_file)
    if local and local.strip():
        return force_tags(local), f"this host's override {local_file}"
    repo = read(repo_file)
    if repo is not None:
        return force_tags(repo), "deploy/live-force-on.txt"
    return [], "no live list in this install"


def hotseat_treatments(path: Path | None = None) -> list[str]:
    """The hotseat-only genes (`deploy/hotseat-force-on.txt`): one tag a line,
    `#` to the end of a line a note. The gameplay lanes' genes for a person's
    game (findings 2026-11-28): before this the only way to arm one for a
    hotseat seat was `CIVVIS_WITH`, which replaces the ladder's whole list."""
    try:
        text = (HOTSEAT_FORCE_FILE if path is None else path).read_text(encoding="utf-8")
    except OSError:
        return []
    return force_tags("\n".join(line.split("#", 1)[0] for line in text.splitlines()))


def genome_forced(stderr: str) -> list[str] | None:
    """The genes a decider says it plays beyond its deployment genome: the
    `forced` list of the `genome` line it writes to stderr as it starts."""
    for line in stderr.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            said = json.loads(line)
        except ValueError:
            continue
        if isinstance(said, dict) and said.get("kind") == "genome":
            forced = said.get("forced")
            return [str(tag) for tag in forced] if isinstance(forced, list) else None
    return None


#: The tag `civvis_orders` names when it refuses a `--with` (`unknown --with
#: treatment "x"`, `--with treatment "x" is not a ledger-held ...`): one per
#: refusal, the first it met.
REFUSED_TAG = re.compile(r'--with treatment "([^"]+)"')
#: The most refusals one list is allowed before the stock genome plays.
MOST_REFUSED = 40


def armed_treatments(binary: Path, wanted: list[str], scratch: Path, run=None) -> dict:
    """Arm `wanted` on this decider, keeping what it takes.

    `civvis_orders` checks every `--with` before it plays and exits 2 on a
    name it does not know or cannot force, naming what it can: a worker
    started with such a list would die at once, and its seat play on the
    mod's stand-in ladder for the whole game. So the binary is asked first,
    with an empty board (it answers in about 10 ms): a list it refuses loses
    the tag it names, and is asked again. (Its refusal also lists what it can
    force, but that list leaves out the tags its deployment genome already
    plays, which it accepts and skips: 103 of the 285 live tags against the
    live game's decider.) Returns the tags
    to pass (`with`), the decider's own word for what that plays beyond its
    deployment genome (`forced`), the tags it could not take (`dropped`), and
    a note when the answer is not a plain yes."""
    run = run or subprocess.run
    tags, dropped = list(wanted), []
    probe = scratch / "genome-check"
    for _ in range(MOST_REFUSED):
        if not tags:
            return {"with": [], "forced": [], "dropped": dropped, "note": ""}
        probe.mkdir(parents=True, exist_ok=True)
        command = [str(binary), "--mirror", str(probe)]
        for tag in tags:
            command += ["--with", tag]
        try:
            # Never the harness's own stdin: the empty board is all it is given.
            done = run(command, capture_output=True, text=True, timeout=60, check=False,
                       stdin=subprocess.DEVNULL)
        except (OSError, subprocess.SubprocessError) as error:
            return {"with": [], "forced": [], "dropped": dropped + tags,
                    "note": f"the decider could not be asked ({error}); its stock genome plays"}
        finally:
            shutil.rmtree(probe, ignore_errors=True)
        said = done.stderr or ""
        if done.returncode == 0:
            forced = genome_forced(said)
            return {"with": tags, "forced": tags if forced is None else forced,
                    "dropped": dropped, "note": ""}
        if "already ship in the deployment genome" in said:
            return {"with": [], "forced": [], "dropped": dropped,
                    "note": "every listed gene already ships in this decider's deployment genome"}
        named = REFUSED_TAG.search(said)
        if not named or named[1] not in tags:
            break
        dropped.append(named[1])
        tags = [tag for tag in tags if tag != named[1]]
    return {"with": [], "forced": [], "dropped": dropped + tags,
            "note": "the decider refused the list ("
                    + (said.strip().splitlines() or ["no reason given"])[0][:200]
                    + "); its stock genome plays"}


def live_genome(args: argparse.Namespace, binary: Path, run_dir: Path, log) -> dict:
    """What the CIVVIS seats are armed with, decided once per run, said in the
    log and kept in the run directory (`GENOME_RECORD`); `args.treatments` is
    then the `--with` list for every seat's worker and the mod's keys."""
    if getattr(args, "genome", "live") == "stock":
        record = {"genome": "stock", "source": "--genome stock", "listed": 0, "hotseat": [],
                  "with": [], "forced": [], "dropped": [], "note": ""}
    else:
        wanted, source = live_treatments()
        # The hotseat-only genes, after the ladder's, each once.
        extra = [tag for tag in hotseat_treatments() if tag not in wanted]
        if extra:
            wanted = wanted + extra
            source = f"{source} + deploy/hotseat-force-on.txt ({len(extra)})"
        record = {"genome": "live", "source": source, "listed": len(wanted), "hotseat": extra,
                  **armed_treatments(binary, wanted, run_dir)}
    args.treatments = record["with"]
    args.genome_record = {key: record[key] for key in ("genome", "source", "listed", "hotseat",
                                                       "forced", "dropped", "note")}
    (run_dir / GENOME_RECORD).write_text(json.dumps(record, indent=2))
    if record["genome"] == "stock":
        log("decider genome: stock (--genome stock): the deployment genome with no live genes "
            "and no live mod arms")
    elif not record["listed"]:
        log(f"decider genome: no live genes ({source}): the deployment genome, with the live "
            "mod arms this install lists")
    else:
        log(f"decider genome: the live ladder's, from {source}: {len(record['with'])} of "
            f"{record['listed']} listed genes armed, {len(record['forced'])} of them beyond this "
            f"decider's deployment genome; and the live mod arms")
    if record["dropped"]:
        log(f"⚠ this decider cannot play {len(record['dropped'])} listed gene(s), so they are off: "
            + ", ".join(record["dropped"][:12]) + (" ..." if len(record["dropped"]) > 12 else ""))
    if record["note"]:
        log(f"⚠ {record['note']}")
    return record


def kept_genome(args: argparse.Namespace, run_dir: Path) -> None:
    """`--attach`: the workers it restarts play what the run's first workers
    did. A run from before `GENOME_RECORD` played the stock genome."""
    try:
        record = json.loads((run_dir / GENOME_RECORD).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        record = {"genome": "stock", "source": "a run from before live genes", "listed": 0,
                  "with": [], "forced": [], "dropped": [], "note": ""}
    args.genome = record.get("genome") or "stock"
    args.treatments = list(record.get("with") or [])
    args.genome_record = {key: record.get(key) for key in ("genome", "source", "listed", "hotseat",
                                                           "forced", "dropped", "note")}


def brain_command(seat_dir: Path, orders_db: Path, binary: Path, victory: str,
                  treatments: list[str] | None = None, poll_s: float | None = None) -> list[str]:
    """One jack seat's decision worker: the live seat's brain, pinned binary,
    with the live genes it is armed with (`--with`, as the ladder's
    `civ6_play.civvis_brain_command` passes them).

    Frozen, `sys.executable` is the bundle's program and there is no script to
    hand it, so it is asked to be the worker by name (`WORKER_COMMAND`).
    """
    worker = WORKER_COMMAND if FROZEN else str(HERE / "civ6_brain.py")
    command = [sys.executable, worker,
               "--run-dir", str(seat_dir), "--orders-db", str(orders_db),
               "--mode", "civvis", "--bin", str(binary), "--victory", victory,
               # One program for the whole game, and its heartbeat kept out of the
               # live lane's cache (`write_disabled_heartbeat`).
               "--github-refresh-seconds", "0",
               "--github-cache", str(seat_dir.parent / "runtime-cache"),
               "--seconds", str(14 * 24 * 3600)]
    for tag in treatments or []:
        command += ["--with", tag]
    # `--fast-turns`: the worker reads its journal every `FAST_POLL_S`.
    if poll_s:
        command += ["--poll-seconds", f"{poll_s:g}"]
    return command


def stop_stale_worker(seat_dir: Path, *, grace_s: float = 5.0) -> int | None:
    """Stop the seat's previous decision worker if it is still running.

    Workers run in their own sessions so a terminal's Ctrl-C cannot take one
    down mid-decision, which also means a harness that dies hard (killed, or a
    laptop that slept through a crash) leaves them running. A later `--attach`
    then started a second worker on the same seat directory, and two deciders
    read one board and both answered it. The pid recorded at start is checked
    against its command line before anything is signalled, so a pid the system
    has since reused is never touched. Returns the pid stopped, if any.
    """
    try:
        pid = int((seat_dir / "brain.pid").read_text().strip())
    except (OSError, ValueError):
        return None
    command = env.process_command(pid)
    if "civ6_brain.py" not in command or str(seat_dir) not in command:
        return None
    if os.name == "nt":
        # No SIGTERM to wait out: `os.kill` here is TerminateProcess, and
        # `os.kill(pid, 0)` would be a console Ctrl-C, not a probe.
        try:
            os.kill(pid, signal.SIGTERM)
        except OSError:
            pass
        deadline = time.monotonic() + grace_s
        while time.monotonic() < deadline and env.pid_alive(pid):
            time.sleep(0.2)
        return pid
    try:
        os.kill(pid, signal.SIGTERM)
        deadline = time.monotonic() + grace_s
        while time.monotonic() < deadline:
            os.kill(pid, 0)
            time.sleep(0.2)
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    return pid


def worker_session() -> dict:
    """A worker outside the terminal's Ctrl-C: its own session on POSIX, its
    own process group on Windows (where `start_new_session` is ignored)."""
    if os.name == "nt":
        return {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
    return {"start_new_session": True}


#: A worker that keeps dying is restarted this many times, the wait before each
#: doubling from 1 s up to this cap, and then left down (and said so).
WORKER_RESTARTS = 20
WORKER_BACKOFF_MAX_S = 60.0


def last_words(seat_dir: Path) -> str:
    """The last line of a seat's brain.log, for the line that says it died."""
    try:
        with (seat_dir / "brain.log").open("rb") as handle:
            handle.seek(0, os.SEEK_END)
            handle.seek(max(0, handle.tell() - 2048))
            tail = handle.read().decode("utf-8", "replace").strip().splitlines()
    except OSError:
        return ""
    return f"; its last words: {tail[-1][:160]}" if tail else ""


class Brains:
    """One `civ6_brain.py` per jack seat, restarted if one dies."""

    def __init__(self, run_dir: Path, tag: str, seats: list[int], orders_db: Path,
                 binary: Path, victory: str, log: Log, treatments: list[str] | None = None,
                 poll_s: float | None = None):
        self.run_dir, self.tag, self.seats = run_dir, tag, seats
        self.orders_db, self.binary, self.victory, self.log = orders_db, binary, victory, log
        self.treatments = list(treatments or [])
        self.poll_s = poll_s
        self.procs: dict[int, subprocess.Popen] = {}
        self.restarts: dict[int, int] = {seat: 0 for seat in seats}
        self.retry_at: dict[int, float] = {}
        self.given_up: set[int] = set()
        self.retired: set[int] = set()

    def seat_dir(self, seat: int) -> Path:
        return self.run_dir / modinstall.hotseat_seat_tag(self.tag, seat)

    def start(self, seat: int) -> None:
        seat_dir = self.seat_dir(seat)
        seat_dir.mkdir(parents=True, exist_ok=True)
        (seat_dir / "events.jsonl").touch(exist_ok=True)
        stopped = stop_stale_worker(seat_dir)
        if stopped:
            self.log(f"seat {seat}: stopped a decision worker left running (pid {stopped})")
        out = (seat_dir / "brain.log").open("a", buffering=1)
        self.procs[seat] = subprocess.Popen(
            brain_command(seat_dir, self.orders_db, self.binary,
                          self.victory[seat] if isinstance(self.victory, dict) else self.victory,
                          self.treatments, self.poll_s),
            cwd=str(HERE.parent), stdout=out, stderr=subprocess.STDOUT, text=True,
            **worker_session())
        out.close()
        (seat_dir / "brain.pid").write_text(f"{self.procs[seat].pid}\n")

    def start_all(self) -> None:
        # Create the shared database, WAL and all, before three workers race to:
        # concurrent `PRAGMA journal_mode=WAL` calls fail "database is locked"
        # (measured: two of three workers died on their first line).
        import civ6_brain
        civ6_brain.connect(self.orders_db).close()
        for seat in self.seats:
            if seat in self.retired:
                continue
            self.start(seat)
            time.sleep(0.3)
        self.log(f"decision workers up for seats {self.seats} "
                 f"(strategy {STRATEGY}, victory {self.victory}, bin {self.binary}, "
                 f"{len(self.treatments)} live genes)")

    def retire(self, seat: int) -> None:
        """Stop a seat's worker for good: the game has no such CIVVIS seat."""
        self.retired.add(seat)
        proc = self.procs.pop(seat, None)
        if proc is not None and proc.poll() is None:
            proc.terminate()

    def keep_alive(self, clock=None) -> None:
        """Restart a worker that exited, a little later each time (a worker
        that dies at once, on a bad flag or a locked database, used to spend
        its 20 restarts in ten seconds), and say so when one is given up: its
        seat then plays the mod's stand-in ladder, and the person should know
        why their opponent went quiet. It used to stop restarting silently."""
        now = (clock or time.monotonic)()
        for seat, proc in list(self.procs.items()):
            if proc.poll() is None or seat in self.given_up:
                continue
            if self.restarts[seat] >= WORKER_RESTARTS:
                self.given_up.add(seat)
                self.log(f"⚠ seat {seat}'s decision worker died {WORKER_RESTARTS} times and is left "
                         f"down: from here that seat plays the mod's stand-in ladder, not CIVVIS. "
                         f"Why it died is at the end of {self.seat_dir(seat) / 'brain.log'}; "
                         "`civ6-hotseat.sh --stop`, then `civ6-hotseat.sh --attach`, starts it afresh.")
                continue
            due = self.retry_at.setdefault(seat, now + min(WORKER_BACKOFF_MAX_S,
                                                           2.0 ** self.restarts[seat]))
            if now < due:
                continue
            self.retry_at.pop(seat, None)
            self.restarts[seat] += 1
            self.log(f"seat {seat}'s decision worker exited ({proc.returncode}); restarting "
                     f"({self.restarts[seat]} of {WORKER_RESTARTS}){last_words(self.seat_dir(seat))}")
            self.start(seat)

    def stop(self) -> None:
        for proc in self.procs.values():
            if proc.poll() is None:
                proc.terminate()
        for proc in self.procs.values():
            try:
                proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                proc.kill()


# ------------------------------------------------------------ the menu setup

def _norm(text: str) -> str:
    return re.sub(r"[^a-z0-9 ]", "", str(text).lower()).strip()


#: The game window as `read_screen` last got it from the host, for a read the
#: host does not answer.
LAST_WINDOW: dict = {"bounds": None}


def read_screen(run_dir: Path, name: str,
                fallback: tuple | None = None) -> tuple[tuple | None, list[dict]]:
    """The game window and every text line in it, in screen points. `fallback`
    is the window as last read, for a step that already knows it: on a loaded
    host the geometry query (System Events) can go unanswered while the frame
    itself can still be captured and read. With neither, the window this
    process last read stands in (`LAST_WINDOW`)."""
    bounds = play.game_window()
    if bounds is not None:
        LAST_WINDOW["bounds"] = bounds
    # Live 2026-11-27, twice in one evening on a loaded Mac: System Events
    # "did not answer in 10s", then "Can't get process Civ6_Exe_Child". The
    # window had not moved; the frame could be captured and read all along.
    bounds = bounds or fallback or LAST_WINDOW["bounds"]
    if bounds is None:
        return None, []
    shot = run_dir / "setup" / f"{name}.png"
    shot.parent.mkdir(exist_ok=True)
    if not play.screenshot(shot) and not shot.is_file():
        return bounds, []
    screen = play.desktop_size()
    if screen is None:
        return bounds, []
    rows = []
    for observation in play.recognize_once(shot, bounds):
        point = play._observation_point(observation)
        if point is None:
            continue
        x, y = int(point[0] * screen[0]), int(point[1] * screen[1])
        if bounds[0] <= x <= bounds[0] + bounds[2] and bounds[1] <= y <= bounds[1] + bounds[3]:
            rows.append({"text": str(observation.get("text", "")), "x": x, "y": y,
                         "norm": _norm(observation.get("text", ""))})
    return bounds, rows


#: Labels shorter than this match exactly only: "OK" against "OR", or "King"
#: against "Kong", is a different word, not a misread. Vision misreads a letter
#: now and then, and more often at smaller text: on a 1710x1107 MacBook Air
#: screen "Confirm Settings" came back "Confirm Settinas" and setup stalled on
#: the very page it was looking for (2026-11-19).
NEAR_MATCH_MIN_LEN = 5


def edit_distance(a: str, b: str) -> int:
    """Levenshtein distance: letters inserted, dropped or changed."""
    previous = list(range(len(b) + 1))
    for i, left in enumerate(a, 1):
        current = [i]
        for j, right in enumerate(b, 1):
            current.append(min(previous[j] + 1, current[j - 1] + 1,
                               previous[j - 1] + (left != right)))
        previous = current
    return previous[-1]


def near(text: str, want: str) -> bool:
    """Whether normalised `text` is `want` give or take an OCR misread: one
    letter in a short label, two in one of nine letters or more."""
    if text == want:
        return True
    if len(want) < NEAR_MATCH_MIN_LEN:
        return False
    allowed = 1 if len(want) < 9 else 2
    return abs(len(text) - len(want)) <= allowed and edit_distance(text, want) <= allowed


def find(rows: list[dict], label: str, *, exact: bool = True) -> list[dict]:
    """The lines reading `label`. An exact reading wins; failing one, a line
    `label` would be but for a misread letter or two (`near`)."""
    want = _norm(label)
    if exact:
        return ([r for r in rows if r["norm"] == want]
                or [r for r in rows if near(r["norm"], want)])
    return [r for r in rows if want in r["norm"]]


def value_beside(rows: list[dict], label: str, tolerance: int = 7) -> str | None:
    """A setup row's value: the text right of its label on the same line."""
    for head in find(rows, label):
        beside = [r for r in rows if abs(r["y"] - head["y"]) <= tolerance
                  and r["x"] > head["x"] + 20]
        if beside:
            return min(beside, key=lambda r: r["x"])["text"]
    return None


def wait_for_hands_off(log: Log, quiet_s: float = 3.0, limit_s: float = 300.0) -> None:
    """Wait until nobody has touched the Mac for `quiet_s`: setup clicks would
    land in whatever the person is doing."""
    told = False
    deadline = time.monotonic() + limit_s
    while time.monotonic() < deadline and operator_presence.operator_active(quiet_s):
        if not told:
            log("hands off the mouse and keyboard for a moment: setting the game up")
            told = True
        time.sleep(1.0)


def click(log: Log, point: tuple[int, int]) -> None:
    wait_for_hands_off(log)
    play.click_at(int(point[0]), int(point[1]))


def fit_on_screen(get_window=None, get_desktop=None, place=None) -> tuple | None:
    """Lift the game window until its bottom edge is on the screen.

    Civ VI in windowed mode keeps its window exactly as tall as the display
    and will not shrink it, so parked below the menu bar its bottom 33 points
    -- the setup page's Load Game / Confirm Settings row -- are off-screen:
    on a 1710x1107 MacBook Air setup read every option and never found the
    button (2026-11-19, after the operator had switched Civ VI to windowed
    in the previous game). Moved to y=0 the whole frame shows, and the game
    covers the menu bar the way its full-screen mode does. Returns the frame
    asked for, or None when nothing needed to move.
    """
    get_window = get_window or play.game_window
    get_desktop = get_desktop or play.desktop_size
    place = place or (lambda frame: play.macos_window.place_game(
        play.GAME_PROCESS, frame=frame, get_desktop_size=get_desktop,
        get_game_window=get_window))
    bounds, screen = get_window(), get_desktop()
    if bounds is None or screen is None:
        return None
    x, y, width, height = bounds
    if y + height <= screen[1]:
        return None
    frame = (x, max(0, screen[1] - height), width, height)
    place(frame)
    return frame


def park_upper_left(log: Log, get_window=None, get_desktop=None, place=None,
                    settle_s: float = 60.0, poll_s: float = 1.0,
                    clock=time.monotonic, sleep=time.sleep) -> tuple | None:
    """Setup is over: the game goes to the operator's saved upper-left frame.

    Setup runs on the computed left half at full height: its readers are
    calibrated to that frame (see `civ6_play.GAME_CONFIGURED`) and the setup
    page's bottom row has to show (`fit_on_screen`). Nothing moved the game
    back afterwards, so every table, and every relaunch after a freeze, left
    Civ VI on the whole left side of the screen (operator 2026-11-21: "the
    firaxis civ window keeps getting repositioned to the full left side of
    the screen. should be only upper left"). The game is still loading here
    and may resize its window when the map opens, so the frame is held for
    `settle_s`. Without a saved frame for this screen nothing moves. Returns
    the frame, or None.
    """
    get_window = get_window or play.game_window
    get_desktop = get_desktop or play.desktop_size
    frame = play.window_layout.frame_for("upper-left", get_desktop() or None)
    if frame is None:
        return None
    place = place or (lambda target: play.macos_window.place_game(
        play.GAME_PROCESS, frame=target, get_desktop_size=get_desktop,
        get_game_window=get_window))
    play.GAME_CONFIGURED = True
    deadline = clock() + settle_s
    moves = 0
    while True:
        bounds = get_window()
        # A point or two of rounding is the window already in place.
        if bounds is not None and max(abs(a - b) for a, b in zip(bounds, frame)) > 4:
            place(frame)
            moves += 1
        if clock() >= deadline:
            break
        sleep(poll_s)
    if moves:
        log(f"[setup] setup over: the game window went to the saved upper-left frame {frame}")
    return frame


def open_hotseat_setup(run_dir: Path, log: Log, attempts: int = 12) -> tuple | None:
    """Main menu -> Multiplayer -> Hot Seat, to the MULTIPLAYER GAME SETUP page."""
    for attempt in range(1, attempts + 1):
        wait_for_hands_off(log)
        play.ensure_game_frontmost()
        moved = fit_on_screen()
        if moved:
            log(f"[setup] the game window ran off the bottom of the screen; moved it to {moved}")
        time.sleep(1.0)
        bounds, rows = read_screen(run_dir, f"menu-{attempt}")
        if bounds is None:
            time.sleep(5.0)
            continue
        if find(rows, "Confirm Settings"):
            return bounds
        if find(rows, "Connection Issue", exact=False):
            play.dismiss_connection_issue(run_dir / "setup" / f"menu-{attempt}.png", bounds)
            time.sleep(2.0)
            continue
        hot = find(rows, "Hot Seat")
        if hot:
            click(log, (hot[0]["x"], hot[0]["y"]))
            time.sleep(4.0)
            continue
        multi = find(rows, "Multiplayer")
        if multi:
            click(log, (multi[0]["x"], multi[0]["y"]))
            time.sleep(2.0)
            continue
        time.sleep(5.0)
    return None


def check_host_settings(rows: list[dict], args: argparse.Namespace) -> list[str]:
    """Read the setup page back; return what is not as asked."""
    wanted = {
        "Game Speed": play._setup_option_label(args.speed),
        "Map": play._setup_option_label(args.map),
        "Map Size": play._setup_option_label(args.map_size),
    }
    problems = []
    for label, value in wanted.items():
        shown = value_beside(rows, label)
        if shown is None or (_norm(value) not in _norm(shown)
                             and not near(_norm(shown), _norm(value))):
            problems.append(f"{label} shows {shown!r}, wanted {value!r}")
    return problems


SLOT_WORDS = {"host", "computer", "player", "human", "closed", "open"}


def slot_rows(rows: list[dict], bounds: tuple) -> list[dict]:
    """The staging room's seat-type labels, top to bottom.

    Each row reads two lines in the Players column: its name ("Player 2",
    "Random Leader") and under it the seat type ("Host", "Computer", or "Player"
    for a hotseat human). Only the second line is a bare seat word.
    """
    x, _, w, _ = bounds
    header = find(rows, "Players")
    top = header[0]["y"] if header else 0
    add = find(rows, "ADD PLAYER")
    bottom = add[0]["y"] if add else 10 ** 6
    out = []
    for row in rows:
        word = slot_word(row["norm"].split()[-1] if row["norm"] else "")
        if (word and len(row["norm"].split()) <= 2
                and x + 0.12 * w <= row["x"] <= x + 0.36 * w and top < row["y"] < bottom):
            out.append(dict(row, slot=word))
    return sorted(out, key=lambda r: r["y"])


def slot_word(word: str) -> str | None:
    """The seat type a word reads as, give or take an OCR misread (`near`).

    Live 2026-11-27, the first 6-player table (Small map): the fifth row's
    "Computer" came back "Comouter" on every reading. The row was not counted,
    five seats read as too few, ADD PLAYER was pressed, and the seventh, open
    row that made was never closed: setup gave up after 29 tries. A seat word
    one letter off is still that seat.
    """
    if word in SLOT_WORDS:
        return word
    return next((known for known in sorted(SLOT_WORDS) if near(word, known)), None)


#: An open seat's list is under its name line, this share of the window's
#: height below it: a seated row reads its type on that line and is clicked
#: there, an open one reads only "Open", a line higher. Live 2026-11-27
#: (hotseat-20261128T031456Z, a 1107-point window): "Open" at y 495, a click at
#: 513 opened the list ("Closed" at 559, "Human" at 582); a click at 495 never did.
OPEN_LIST_DROP = 0.0163


def choose_from_dropdown(run_dir: Path, log: Log, row: dict, option: str,
                         bounds: tuple, step: int) -> bool:
    _, _, _, h = bounds
    drop = int(OPEN_LIST_DROP * h) if row.get("slot") == "open" else 0
    click(log, (row["x"] + 30, row["y"] + drop))
    time.sleep(1.2)
    _, rows = read_screen(run_dir, f"slot-{step}-open")
    picks = [r for r in find(rows, option)
             if row["y"] < r["y"] <= row["y"] + 0.15 * h and abs(r["x"] - row["x"]) < 120]
    if not picks:
        log(f"[setup] the seat list did not offer {option!r}; closing it")
        play.macos_input.press_key("escape")
        time.sleep(0.8)
        play.park_setup_pointer(bounds)
        return False
    pick = min(picks, key=lambda r: r["y"])
    click(log, (pick["x"], pick["y"]))
    time.sleep(1.2)
    # Off the row before the room is read again: the pointer left on a seat's
    # list raises its tooltip ("AI Player"), which read as one more seat.
    play.park_setup_pointer(bounds)
    return True


def wanted_slots(players: int, humans: int | None = None) -> list[str]:
    """The staging room's seat column as asked: the host, then hotseat humans
    up to `humans` (every seat by default), then Firaxis AI ("Computer")."""
    humans = players if humans is None else humans
    return ["host"] + ["player"] * (humans - 1) + ["computer"] * (players - humans)


def slots_match(kinds: list[str], want: list[str]) -> bool:
    return (len(kinds) == len(want) and kinds[0] in ("host", "player")
            and kinds[1:] == want[1:])


#: The staging room's seat column as `seat_the_players` last read it: what a
#: person finishing the setup by hand is told to change (`seat_fixes`).
LAST_ROOM: dict = {"kinds": None}


def seat_the_players(run_dir: Path, bounds: tuple, players: int, log: Log,
                     humans: int | None = None, steps: int = 29, first: int = 1) -> bool:
    """Make rows 1..humans-1 hotseat humans, the rest up to `players` Firaxis
    AI, and close every row after them (every seat human by default)."""
    want = wanted_slots(players, humans)
    short = 0
    for step in range(first, first + steps):
        # A staging room behind another window reads as that window.
        play.ensure_game_frontmost()
        _, rows = read_screen(run_dir, f"staging-{step}")
        seats = slot_rows(rows, bounds)
        kinds = [s["slot"] for s in seats]
        if not seats:
            time.sleep(2.0)
            continue
        LAST_ROOM["kinds"] = kinds
        if slots_match(kinds, want):
            log(f"[setup] staging room: {want.count('player') + 1} hotseat seats and "
                f"{want.count('computer')} Firaxis AI, verified")
            return True
        if len(seats) > players:
            short = 0
            choose_from_dropdown(run_dir, log, seats[-1], "Closed", bounds, step)
            continue
        if len(seats) < players:
            # One reading is not a count. A row the screen reader dropped for a
            # frame made five seats of six, and the seat added for it was one
            # too many (2026-11-27): a seat is added on the second short reading.
            short += 1
            if short < 2:
                time.sleep(1.5)
                continue
            short = 0
            add = find(rows, "ADD PLAYER")
            if not add:
                log("[setup] fewer seats than wanted and no ADD PLAYER button")
                return False
            click(log, (add[0]["x"], add[0]["y"]))
            time.sleep(1.5)
            continue
        short = 0
        index = next(i for i in range(1, len(want)) if kinds[i] != want[i])
        if want[index] == "player":
            choose_from_dropdown(run_dir, log, seats[index], "Human", bounds, step)
        elif not choose_from_dropdown(run_dir, log, seats[index], "Computer", bounds, step):
            choose_from_dropdown(run_dir, log, seats[index], "AI", bounds, step)
    log("[setup] could not seat the players; the staging room is left as it is")
    return False


#: A seat type as the staging room's Players column shows it.
SLOT_SHOWN = {"host": "Host", "player": "Player", "computer": "Computer", "open": "Open",
              "closed": "Closed", "human": "Player"}


def seat_fixes(kinds: list[str] | None, want: list[str]) -> list[str]:
    """What to click in the staging room to turn the seat column `kinds` (as
    read) into `want`, one step a line, rows numbered as the room shows them.
    A row's type is its list under its name: Human makes a hotseat seat (it
    then reads "Player"), Computer one of Civ VI's own AI, Closed removes it."""
    choose = {"player": "Human", "computer": "Computer"}
    if not kinds:
        humans = want.count("player") + 1
        fixes = [f"rows 2-{humans}: open each row's list and choose Human"] if humans > 1 else []
        if want.count("computer"):
            fixes.append(f"rows {humans + 1}-{len(want)}: choose Computer")
        return fixes + [f"any row after row {len(want)}: choose Closed"]
    fixes = []
    for i, kind in enumerate(kinds):
        if i >= len(want):
            fixes.append(f"row {i + 1} ({SLOT_SHOWN.get(kind, kind)}): open its list and choose Closed")
        elif i and kind != want[i]:
            fixes.append(f"row {i + 1} ({SLOT_SHOWN.get(kind, kind)}): open its list and choose "
                         f"{choose[want[i]]}")
    for i in range(len(kinds), len(want)):
        fixes.append(f"ADD PLAYER, then row {i + 1}: choose {choose[want[i]]}")
    return fixes


def setup_help(failed: dict, args: argparse.Namespace) -> list[str]:
    """What the person does to finish a setup that stopped at `failed["step"]`,
    each line one thing to do in Civ VI, ending with START GAME."""
    want = wanted_slots(args.players, getattr(args, "civvis_seats", None))
    level = play._setup_option_label(args.difficulty)
    room = seat_fixes(failed.get("kinds"), want)
    steps = {
        "menu": [f"main menu -> Multiplayer -> Hot Seat; set Map {play._setup_option_label(args.map)}, "
                 f"Map Size {play._setup_option_label(args.map_size)}, Game Speed "
                 f"{play._setup_option_label(args.speed)}; Confirm Settings", *seat_fixes(None, want),
                 f"every row's Difficulty: {level}"],
        "settings": [*(f"on the setup page: {problem}" for problem in failed.get("problems") or []),
                     "Confirm Settings", *seat_fixes(None, want), f"every row's Difficulty: {level}"],
        "seats": [*room, f"every row's Difficulty: {level}"],
        "difficulty": [f"every row's Difficulty: {level} (its list, right of the leader)"],
        "start": [],
        "load": ["on the Hot Seat setup page: Load Game -> " + RESUME_STEM + " -> Load Game"],
    }.get(failed.get("step"), [*seat_fixes(None, want), f"every row's Difficulty: {level}"])
    return [*steps, "START GAME (the orb at the bottom)"]


DIFFICULTY_LABELS = {_norm(play._setup_option_label(d)) for d in play.LADDER}


def difficulty_cells(rows: list[dict], bounds: tuple) -> list[dict]:
    """Each seat's Difficulty cell, top to bottom, paired with its seat row."""
    x, _, w, _ = bounds
    cells = [r for r in rows if r["norm"] in DIFFICULTY_LABELS
             and x + 0.52 * w <= r["x"] <= x + 0.72 * w]
    out = []
    for seat in slot_rows(rows, bounds):
        near = [c for c in cells if abs(c["y"] - seat["y"]) <= 14]
        if near:
            out.append(min(near, key=lambda c: abs(c["y"] - seat["y"])))
    return out


def dropdown_picks(listed: list[dict], cells: list[dict], cell: dict, want: str,
                   height: int) -> list[dict]:
    """The open list's `want` entries: near the clicked cell, and not another
    seat's own cell showing the same word (measured: King on the row above
    read as the list's King, and the click landed there)."""
    def is_cell(row: dict) -> bool:
        return any(abs(row["x"] - c["x"]) <= 6 and abs(row["y"] - c["y"]) <= 4 for c in cells)
    return [r for r in find(listed, want) if abs(r["x"] - cell["x"]) < 90
            and 4 < abs(r["y"] - cell["y"]) <= 0.2 * height and not is_cell(r)]


def set_difficulties(run_dir: Path, bounds: tuple, difficulty: str, log: Log) -> bool:
    """Set every seat's Difficulty (hotseat difficulty is per player; the setup
    page has no game row for it) and read the column back."""
    want = _norm(play._setup_option_label(difficulty))
    _, _, _, h = bounds
    for step in range(1, 16):
        _, rows = read_screen(run_dir, f"difficulty-{step}")
        cells = difficulty_cells(rows, bounds)
        wrong = [c for c in cells if c["norm"] != want]
        if cells and not wrong:
            log(f"[setup] every seat at {play._setup_option_label(difficulty)}, verified")
            return True
        if not cells:
            time.sleep(1.5)
            continue
        cell = wrong[0]
        click(log, (cell["x"] + 20, cell["y"]))
        time.sleep(1.2)
        _, listed = read_screen(run_dir, f"difficulty-{step}-open")
        picks = dropdown_picks(listed, cells, cell, want, h)
        if not picks:
            log(f"[setup] the difficulty list did not offer {want!r}; closing it")
            play.macos_input.press_key("escape")
            time.sleep(0.8)
            continue
        pick = min(picks, key=lambda r: abs(r["y"] - cell["y"]))
        click(log, (pick["x"], pick["y"]))
        time.sleep(1.2)
    log(f"[setup] could not set every seat to {difficulty}")
    return False


def start_game(run_dir: Path, bounds: tuple, log: Log, attempts: int = 10) -> bool:
    """Press START GAME (the orb above its label) and see the staging room go.

    Gone is something read, never something not read. Live 2026-11-27
    (duel-dev-20261127T0643Z, load average 40): System Events did not answer
    the window query, the read came back with no text at all, the empty read
    was taken for a staging room that had gone, and the harness sat on "game
    starting" with START GAME never pressed. So the room has gone only when a
    screen that could be read shows neither label after a press; an unreadable
    one is read again, and the game is brought to the front before each read
    (a staging room behind a Terminal reads as that Terminal).
    """
    _, _, _, h = bounds
    pressed = 0
    for attempt in range(1, attempts + 1):
        wait_for_hands_off(log)
        play.ensure_game_frontmost()
        _, rows = read_screen(run_dir, f"start-{attempt}", bounds)
        if not rows:
            log(f"[setup] the screen could not be read (START GAME, attempt {attempt}); "
                "reading it again")
            time.sleep(3.0)
            continue
        label = find(rows, "START GAME")
        if not label:
            if pressed and not find(rows, "STAGING ROOM"):
                return True
            time.sleep(2.0)
            continue
        x, y = label[0]["x"], label[0]["y"]
        click(log, (x, y) if pressed % 2 else (x, y - int(0.02 * h)))
        pressed += 1
        time.sleep(6.0)
    _, rows = read_screen(run_dir, "start-final", bounds)
    return bool(rows and pressed) and not find(rows, "STAGING ROOM") \
        and not find(rows, "START GAME")


#: A started game's first event is due well inside this many seconds; with
#: none, the staging room is looked at again (`press_start_again`).
START_CHECK_SECONDS = 60.0
#: With no game yet this long after the setup (or after the person was handed
#: it), the screen is looked at and what to do is said (`not_started_advice`).
NOT_STARTED_S = 300.0
NOT_STARTED_HANDED_S = 120.0


def hand_over_setup(args: argparse.Namespace, failed: dict, log: Log, tell=None) -> list[str]:
    """The setup stopped short: tell the person, in the terminal, the log and a
    notification, exactly what is left to click, step by step, from where it
    stopped. Before 2026-11-28 this said "make Players 2-N 'Human'" whatever
    the room held: the six-player room that night held a Computer row and an
    extra Open row, and the person had no idea what to do (10.5 minutes from
    launch to turn 1, finished by hand from another session)."""
    steps = setup_help(failed, args)
    listed = "\n".join(f"  {i}. {step}" for i, step in enumerate(steps, 1))
    print(f"\a\nThe setup could not finish by itself. In Civ VI, please:\n{listed}\n"
          "CIVVIS takes its seats the moment the game starts; this window says so.\n", flush=True)
    log("setup handed to the person: " + "; ".join(steps))
    if not getattr(args, "no_notify", False):
        (tell or notify)("CIVVIS: please finish the setup in Civ VI", "; ".join(steps)[:220])
    return steps


def not_started_advice(run_dir: Path, args: argparse.Namespace, waited_s: float) -> str:
    """No game yet: what the screen shows, and what to do about it. Read only
    (nothing clicked or brought forward), and not while someone is at the
    keyboard. Replaces "no hand-over event five minutes after START GAME; is
    the game up?", which nobody could act on."""
    waited = f"{waited_s / 60:.0f} min"
    if operator_presence.operator_active(5.0):
        return f"the game has not started after {waited}; the screen was not read (the Mac is in use)"
    bounds, rows = read_screen(run_dir, "not-started")
    want = wanted_slots(args.players, getattr(args, "civvis_seats", None))
    if bounds is None:
        return (f"the game has not started after {waited} and Civ VI's window was not found: is it "
                "still running? `civ6-hotseat.sh --stop --quit-game` and the same command again "
                "starts over")
    if find(rows, "STAGING ROOM") or find(rows, "START GAME"):
        kinds = [seat["slot"] for seat in slot_rows(rows, bounds)]
        fixes = seat_fixes(kinds, want) if kinds else []
        if kinds and not fixes:
            return f"the staging room reads right ({len(kinds)} seats): press START GAME"
        return ("the staging room is still up: " + "; ".join(fixes or setup_help({"step": "seats"}, args)[:-1])
                + "; then START GAME")
    if find(rows, "Confirm Settings"):
        return ("Civ VI is on the Hot Seat setup page: " + "; ".join(setup_help({"step": "settings"}, args)))
    if find(rows, "Multiplayer") or find(rows, "Single Player"):
        free = host_strain().get("memory_free_gb")
        return ("Civ VI is at its main menu: the game did not start, or it closed back to the menu"
                + (f" (memory free now: {free:.1f} GB; Civ VI needs about {CIV_MEMORY_GB - 1:.0f})"
                   if free is not None else "")
                + ". Start over: `civ6-hotseat.sh --stop --quit-game`, then the same command again")
    if find(rows, "Loading", exact=False) or find(rows, "Please Wait", exact=False):
        return f"Civ VI is still loading the game after {waited}"
    return (f"the game has not started after {waited}, and the screen shows neither the staging "
            "room, the setup page nor the main menu: bring Civ VI to the front and look at it")


def press_start_again(run_dir: Path, log: Log, waited_s: float) -> bool:
    """The backstop behind `start_game`: no event from the game a minute after
    START GAME, so look, and press it if the staging room is still up (a press
    that only brought the window forward, a read that was wrong). Whether it
    pressed."""
    if operator_presence.operator_active(3.0):
        return False
    play.ensure_game_frontmost()
    bounds, rows = read_screen(run_dir, "start-again")
    label = find(rows, "START GAME")
    if bounds is None or not label:
        return False
    log(f"[setup] the staging room is still up {waited_s:.0f}s after START GAME; pressing it again")
    click(log, (label[0]["x"], label[0]["y"] - int(0.02 * bounds[3])))
    return True


#: Where Civ VI keeps hotseat saves (`auto/` holds its autosaves). A save to
#: resume is copied here under one constant name, so the Load Game list shows it
#: as a row the screen reader can find without scrolling or filters.
HOTSEAT_SAVES = play.AUTOSAVE_DIR.parent.parent / "Hotseat"
RESUME_STEM = "civvis-hotseat-resume"


def hotseat_saves(directory: Path = HOTSEAT_SAVES) -> list[Path]:
    """Every hotseat save, newest first: the mod's own (`<tag>-T<turn>`),
    Civ VI's autosaves, and the person's quicksave (Saves/Hotseat/quick/,
    which a person who presses Quick Save before quitting means to go on from:
    the live 6-player game of 2026-11-28 made one at 21:44)."""
    saves = []
    for pattern in ("*.Civ6Save", "auto/*.Civ6Save", "quick/*.Civ6Save"):
        for path in directory.glob(pattern):
            try:
                if path.is_file() and path.stem != RESUME_STEM:
                    saves.append((path.stat().st_mtime, path))
            except OSError:
                continue
    return [path for _, path in sorted(saves, key=lambda pair: pair[0], reverse=True)]


def newest_save(directory: Path = HOTSEAT_SAVES) -> Path | None:
    saves = hotseat_saves(directory)
    return saves[0] if saves else None


#: In each run directory: the game as that run plays it, every setting below
#: (`table_settings`). A later `--resume` of one of its saves reads it back.
TABLE = "table.json"
#: What a saved game is: the table, how it was set up, the person's leader.
#: No flag changes a saved game, so these always come from the run that made
#: the save (a differing flag is said and not used). The wrapper that starts a
#: person's games passes `--players 2 --map-size MAPSIZE_DUEL` before every
#: command, `--resume` too, so a flag here is not always a person's choice.
SAVE_OWN = ("players", "map", "map_size", "speed", "difficulty", "leader", "victories_off")
#: What the first run chose for the game: brought back unless asked again.
CHOICES = ("name", "agents", "civvis_seats", "auto_human", "victory", "max_turns",
           "save_every", "no_cover", "keep_handover", "cover_sound", "no_cover_timer",
           "no_cover_queue", "play_flags")
#: How far back a `--resume` looks for the run that made a save.
RUNS_LOOKED_AT = 60
#: A save is read this far for the names its seats carry (they stand in the
#: first ~170 KB of a 6-player save).
SAVE_HEAD_BYTES = 1 << 20
#: A CIVVIS seat's name as a save holds it (`seat_name`); the mod's own name,
#: "CIVVIS Control", is in every save too and is not a seat.
SAVED_SEAT_NAME = re.compile(rb"CIVVIS (?:jack \d+|[A-Z][A-Za-z]+)")


def settings_from_config(config: dict) -> dict:
    """The game's settings from the mod's installed config (`config.json`), for
    a run from before `table.json`. Only what the config holds is returned."""
    players, civvis_seats = attached_table(config)
    agents = config.get("HotseatSeatAgents") or {}
    settings = {
        "tag": config.get("RunTag"),
        "players": players, "civvis_seats": civvis_seats,
        "auto_human": (None if not config.get("HotseatStubSeats")
                       else "gated" if config.get("HotseatHumanSeats") else "stub"),
        "map": config.get("MapScript"), "map_size": config.get("MapSize"),
        "speed": config.get("GameSpeed"),
        "difficulty": config.get("PlayerDifficulty") or config.get("Difficulty"),
        "leader": config.get("Leader") or None,
        "victories_off": list(config.get("VictoriesOff") or []),
        "name": config.get("HotseatPersonName") or None,
        "agents": [agents[seat] for seat in sorted(agents, key=int)] or None,
        "victory": config.get("VictoryLane"),
        "max_turns": config.get("MaxTurns"),
        "save_every": config.get("HotseatSaveEvery"),
        "no_cover": not config.get("HotseatCover"),
        "keep_handover": not config.get("HotseatAutoStart"),
        "cover_sound": config.get("HotseatCoverSound"),
        "no_cover_timer": not config.get("HotseatCoverTimer"),
        "no_cover_queue": not config.get("HotseatCoverQueue"),
        "started": config.get("HotseatStarted") or None,
        "names": dict(config.get("HotseatNames") or {}),
    }
    # A value the config did not carry is not a setting; `None` is one only
    # where it means something (a random leader, no name, jack everywhere).
    meaningful = {"leader", "name", "agents", "civvis_seats", "auto_human", "started"}
    return {key: value for key, value in settings.items()
            if value is not None or key in meaningful}


def table_settings(args: argparse.Namespace, began: dict, resumed: dict | None = None) -> dict:
    """What `TABLE` keeps: every setting a resumed game takes back, the seats'
    names in the game (how a save is told to be this run's), and when the
    game began."""
    settings = {key: getattr(args, key, None) for key in SAVE_OWN + CHOICES}
    seats = jack_seats(args.players, getattr(args, "civvis_seats", None))
    settings.update({
        "tag": args.tag, "started": began.get("started"),
        "names": {str(seat): seat_name(i, getattr(args, "agents", None) and agent)
                  for i, (seat, agent) in enumerate(seat_agents(args, seats).items(), 1)},
    })
    if args.auto_human:
        settings["names"][str(HUMAN_SEAT)] = "Test stub"
    if resumed:
        settings["resumed_from"] = resumed.get("from")
    return settings


def run_settings(run_dir: Path) -> dict | None:
    """A run's game: its `table.json`, or for an older run what its
    `config.json` says. None when the run directory holds neither."""
    try:
        table = json.loads((run_dir / TABLE).read_text(encoding="utf-8"))
        if isinstance(table, dict):
            return table
    except (OSError, ValueError):
        pass
    try:
        config = json.loads((run_dir / "config.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    if not isinstance(config, dict) or not config.get("Hotseat"):
        return None
    return settings_from_config(config)


def save_head(save: Path) -> bytes:
    try:
        with save.open("rb") as handle:
            return handle.read(SAVE_HEAD_BYTES)
    except OSError:
        return b""


def save_seat_names(head: bytes) -> list[str]:
    """The CIVVIS seats' names a save holds, in the order they stand there
    (not seat order: the save keeps them in a table of its own)."""
    seen: list[str] = []
    for found in SAVED_SEAT_NAME.findall(head):
        name = found.decode("ascii", "replace")
        if name != "CIVVIS Control" and name not in seen:
            seen.append(name)
    return seen


def save_is_of(head: bytes, settings: dict) -> bool:
    """Whether a save is this run's game: every name the run gave a seat, the
    person's included, is in it. A run that names no seat cannot be told."""
    names = [str(name) for name in (settings.get("names") or {}).values() if name]
    if settings.get("name"):
        names.append(str(settings["name"]))
    return bool(names) and all(name.encode("utf-8") in head for name in names)


#: A save the mod made: `<tag>-T<turn>` (`HotseatSaveName`, `HotseatSaveEvery`).
MOD_SAVE = re.compile(r"^(?P<tag>.+)-T\d{3,}$")


def save_run(save: Path, root: Path | None = None) -> Path | None:
    """The run that played the game a save is of.

    A save the mod made carries its run's tag. An autosave or a quicksave
    carries none, so it is the newest run that had started when the save was
    written and whose seat names the save holds: one game runs on a Mac at a
    time (`gamelock`), and a run whose game never got going names seats no
    save of it holds."""
    root = RUN_ROOT if root is None else root
    tagged = MOD_SAVE.match(save.stem)
    if tagged and run_settings(root / tagged["tag"]) is not None:
        return root / tagged["tag"]
    try:
        written = save.stat().st_mtime
    except OSError:
        return None
    runs = []
    for config in root.glob("*/config.json"):
        try:
            began = config.stat().st_mtime
        except OSError:
            continue
        if began <= written + 5:
            runs.append((began, config.parent))
    head = save_head(save)
    for _, run_dir in sorted(runs, key=lambda pair: pair[0], reverse=True)[:RUNS_LOOKED_AT]:
        settings = run_settings(run_dir)
        if settings is not None and save_is_of(head, settings):
            return run_dir
    return None


def persons_game(settings: dict | None) -> bool:
    """A run a person played (not a test stub's, not an unattended table)."""
    return bool(settings) and not settings.get("civvis_seats") and not settings.get("auto_human")


def resume_save(requested: str, person: bool, directory: Path | None = None,
                root: Path | None = None) -> tuple[Path | None, list[Path]]:
    """The save `--resume` loads, and the newer saves passed over for it.

    `--resume PATH` is that save. A bare `--resume` is the newest, except that
    a person's command passes over a save a test or an unattended table made
    (other sessions on this Mac rehearse hotseat games between a person's):
    it goes on from that person's newest game instead."""
    if requested != "latest":
        return Path(requested).expanduser(), []
    saves = hotseat_saves(HOTSEAT_SAVES if directory is None else directory)
    if not person:
        return (saves[0] if saves else None), []
    passed = []
    for save in saves[:RUNS_LOOKED_AT]:
        run_dir = save_run(save, root)
        settings = run_settings(run_dir) if run_dir else None
        if settings is None or persons_game(settings):
            return save, passed
        passed.append(save)
    return (saves[0] if saves else None), []


def flag(key: str) -> str:
    return "--" + key.replace("_", "-")


def shown(key: str, value) -> str:
    if isinstance(value, list):
        return ",".join(str(v) for v in value) or "none"
    return str(value)


def restore_resumed(args: argparse.Namespace, typed: set[str], save: Path,
                    root: Path | None = None) -> tuple[list[str], bool]:
    """Give `args` the game the save is of, from the run that played it.

    The save's own settings (`SAVE_OWN`) always; the first run's choices
    (`CHOICES`) wherever the command did not ask again (`typed`). Returns what
    to say, and whether the resume can go ahead."""
    run_dir = save_run(save, root)
    settings = run_settings(run_dir) if run_dir is not None else None
    if settings is None:
        return gone_record(args, typed, save, root)
    said = []
    unused = []
    for key in SAVE_OWN:
        if key not in settings:
            continue
        value = settings[key]
        if key in typed and normalised(key, getattr(args, key)) != value:
            unused.append(f"{flag(key)} {shown(key, getattr(args, key))} "
                          f"(the game has {shown(key, value)})")
        setattr(args, key, raw(key, value))
    asked = []
    for key in CHOICES:
        if key not in settings:
            continue
        if key in typed:
            if normalised(key, getattr(args, key)) != settings[key]:
                asked.append(flag(key))
            continue
        setattr(args, key, raw(key, settings[key]))
    # The save's table is the table: `--duel`'s two seats and Duel map are not
    # this game's unless they already were.
    if getattr(args, "duel", False) and (args.players, args.map_size) != (2, "MAPSIZE_DUEL"):
        unused.append(f"--duel (the game has {args.players} players on {args.map_size})")
    args.duel = False
    args.resumed = {"from": run_dir.name, "started": settings.get("started"), "save": str(save)}
    seats = jack_seats(int(args.players), args.civvis_seats)
    agents = normalised("agents", args.agents) or []
    table = ", ".join(f"seat {seat} {agents[i] if i < len(agents) else STRATEGY}"
                      for i, seat in enumerate(seats))
    if args.civvis_seats or args.auto_human:
        who = "nobody's seat (" + ("a test stub" if args.auto_human else "CIVVIS on every seat") + ")"
    elif args.name:
        who = f"{args.name} as {args.leader or 'a random leader'}, recorded for the people's Elo"
    else:
        who = (f"you as {args.leader or 'a random leader'}; no name, so not recorded for the "
               "people's Elo")
    said.append(f"resuming {save.name}, the game of {run_dir.name}"
                + (f" (begun {settings['started']})" if settings.get("started") else "")
                + f". Restored: {args.players} players on {args.map} {args.map_size}, "
                  f"{args.speed}, {args.difficulty}; {who}; {table}"
                + (f"; as asked now: {', '.join(asked)}" if asked else ""))
    if unused:
        said.append("not used, the save decides them: " + "; ".join(unused))
    return said, True


def normalised(key: str, value):
    """A flag's value as `table.json` keeps it (`--agents` and
    `--victories-off` are comma lists on the command line)."""
    if key == "agents" and isinstance(value, str):
        return [part.strip().lower() for part in value.split(",") if part.strip()] or None
    if key == "victories_off" and isinstance(value, str):
        return [part.strip().upper() for part in value.split(",") if part.strip()]
    if key == "leader" and isinstance(value, str) and value.lower() == play.RANDOM_LEADER:
        return None
    return value


def raw(key: str, value):
    """A kept value as the command line gives it, for `main` to read on."""
    if key in ("agents", "victories_off") and isinstance(value, list):
        return ",".join(value) or (None if key == "agents" else "")
    return value


def gone_record(args: argparse.Namespace, typed: set[str], save: Path,
                root: Path | None) -> tuple[list[str], bool]:
    """A save no run on this Mac recorded (its run directory was removed, or
    the save came from elsewhere): played as the command says, when the
    command's table fits the save; otherwise stopped, saying what to type."""
    args.resumed = {"from": None, "started": None, "save": str(save)}
    names = save_seat_names(save_head(save))
    players = 2 if args.duel else args.players
    seats = len(jack_seats(players, players if args.self_play else args.civvis_seats))
    where = RUN_ROOT if root is None else root
    if names and len(names) != seats:
        guess = [next((agent for agent, handle in AGENT_HANDLES.items()
                       if name == f"CIVVIS {handle}"), STRATEGY) for name in names]
        return [f"⚠ no run in {where} recorded the game in {save.name}, so who plays which "
                f"seat is not known, and the save has {len(names)} CIVVIS seats "
                f"({', '.join(names)}) where this command plays {seats}. Resume it with its "
                "table spelled out, seat 1 first, in the order you set it up, e.g.:\n"
                f"    civ6-hotseat.sh --resume {shlex.quote(str(save))} --players {len(names) + 1} "
                f"--agents {','.join(guess)} --name YOU"], False
    agents = getattr(args, "agents", None) or (STRONGEST if args.duel else STRATEGY)
    return [f"⚠ no run in {where} recorded the game in {save.name}: it is played as this "
            f"command says ({players} players, agents {shown('agents', agents)}"
            + (f", name {args.name}" if args.name else
               "; no --name, so it is not recorded for the people's Elo") + ")"], True


def stage_save(save: Path, directory: Path = HOTSEAT_SAVES) -> Path:
    directory.mkdir(parents=True, exist_ok=True)
    staged = directory / f"{RESUME_STEM}.Civ6Save"
    if save.resolve() != staged.resolve():
        shutil.copy2(save, staged)
    return staged


def load_saved_game(run_dir: Path, bounds: tuple, log: Log) -> bool:
    """From the Hot Seat setup page: Load Game -> the staged save -> Load Game."""
    _, rows = read_screen(run_dir, "load-open")
    button = find(rows, "Load Game")
    if not button:
        log("[setup] the setup page shows no Load Game button")
        return False
    click(log, (button[0]["x"], button[0]["y"]))
    time.sleep(3.0)
    for attempt in range(1, 6):
        shot = run_dir / "setup" / f"load-{attempt}.png"
        _, rows = read_screen(run_dir, f"load-{attempt}")
        row = find(rows, RESUME_STEM, exact=False)
        if not row:
            time.sleep(2.0)
            continue
        click(log, (row[0]["x"], row[0]["y"]))
        time.sleep(1.5)
        play.screenshot(shot)
        actions = play._observed_label_points(shot, "Load Game", bounds,
                                              strip=play.LOAD_GAME_ACTION_STRIP)
        low = [p for p in actions if p[1] >= bounds[1] + 0.75 * bounds[3]]
        if not low:
            time.sleep(1.5)
            continue
        click(log, max(low, key=lambda p: p[1]))
        time.sleep(5.0)
        _, rows = read_screen(run_dir, f"load-{attempt}-after")
        if find(rows, "START GAME") or find(rows, "STAGING ROOM"):
            return True
    log("[setup] the staged save never loaded into the staging room")
    return False


#: A second pass at the seats after the first gave up: the room is read anew at
#: every step, so a pass that ran out of tries on one row can still finish.
SEAT_RETRY_STEPS = 12


def set_up_game(run_dir: Path, args: argparse.Namespace, log: Log) -> bool:
    """Set the game up through Civ VI's menus. When a step cannot be finished,
    `args.setup_failed` says which (`step`) and, for the seats, how the room
    last read (`kinds`), for the person to finish it from (`setup_help`)."""
    args.setup_failed = None
    LAST_ROOM["kinds"] = None

    def failed(step: str, **more) -> bool:
        args.setup_failed = {"step": step, **more}
        return False

    bounds = open_hotseat_setup(run_dir, log)
    if bounds is None:
        log("[setup] never reached the Hot Seat setup page")
        return failed("menu")
    if args.resume_save is not None:
        if not load_saved_game(run_dir, bounds, log):
            return failed("load")
        _, rows = read_screen(run_dir, "resume-staging")
        kinds = [s["slot"] for s in slot_rows(rows, bounds)]
        log(f"[setup] the save's seats: {kinds}")
        if len(kinds) != args.players or any(k not in ("host", "player") for k in kinds):
            log(f"[setup] ⚠ the save does not read as {args.players} hotseat humans; "
                "the CIVVIS seats are the seats after yours")
        if not start_game(run_dir, bounds, log):
            log("[setup] START GAME did not start the saved game")
            return failed("start")
        log("[setup] saved game starting")
        return True
    _, rows = read_screen(run_dir, "host-settings")
    problems = check_host_settings(rows, args)
    if problems:
        log("[setup] the setup page is not what was asked: " + "; ".join(problems))
        return failed("settings", problems=problems)
    log("[setup] setup page verified: " + ", ".join(
        f"{label} {value_beside(rows, label)}" for label in ("Game Speed", "Map", "Map Size")))
    confirm = find(rows, "Confirm Settings")
    click(log, (confirm[0]["x"], confirm[0]["y"]))
    time.sleep(4.0)
    humans = getattr(args, "civvis_seats", None)
    if not seat_the_players(run_dir, bounds, args.players, log, humans=humans):
        # Once more, shorter, from where the room now is (each step reads it
        # again), with the pointer off the rows: tonight's room sat on one row
        # whose list did not open for 20 tries (hotseat-20261128T031456Z).
        log("[setup] seating the players once more")
        play.park_setup_pointer(bounds)
        if not seat_the_players(run_dir, bounds, args.players, log, humans=humans,
                                steps=SEAT_RETRY_STEPS, first=30):
            return failed("seats", kinds=LAST_ROOM["kinds"])
    if not set_difficulties(run_dir, bounds, args.difficulty, log):
        return failed("difficulty")
    if getattr(args, "hold_start", False):
        log("[setup] staging room ready; HOLDING for the person: set Teams, then START GAME")
        print("\a\nStaging room is set. Set the Team column, then press START GAME.\n",
              flush=True)
        return True
    if not start_game(run_dir, bounds, log):
        log("[setup] START GAME did not start the game")
        return failed("start")
    log("[setup] game starting")
    return True


# ------------------------------------------------------------- the turn loop

class Journal:
    """New complete lines of one events.jsonl."""

    def __init__(self, path: Path):
        self.path, self.offset, self.partial = path, 0, ""

    def read(self) -> list[dict]:
        try:
            with self.path.open("r", errors="replace") as handle:
                handle.seek(self.offset)
                chunk = handle.read()
                self.offset = handle.tell()
        except OSError:
            return []
        lines = (self.partial + chunk).split("\n")
        self.partial = lines.pop()
        out = []
        for line in lines:
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if isinstance(event, dict):
                out.append(event)
        return out


#: The hotseat loops' upkeep pass (the harness beat, the event monitor, the
#: workers kept alive, the stall and liveness checks), and how often the
#: game's log is relayed to the decision workers in between (`relay_for`).
PASS_S = 0.5
RELAY_S = 0.02
#: `--fast-turns`: the relay's and each worker's read every 10 ms instead of 20
#: and 25. A board waits half a read on average at each, about 10 ms in all a
#: board, a dozen boards a round at turn 30 (hotseat-20261128T045523Z: one
#: opening and 1.4 extra frames per seat-turn, five seats). Each read is one
#: stat-and-read of a log's new bytes.
FAST_RELAY_S = 0.01
FAST_POLL_S = 0.01


def relay_seconds(args: argparse.Namespace) -> float:
    """How often this table's log is relayed to its workers."""
    return FAST_RELAY_S if getattr(args, "fast_turns", False) else RELAY_S


def poll_seconds(args: argparse.Namespace) -> float | None:
    """How often each worker reads its journal; None: the worker's own 25 ms."""
    return FAST_POLL_S if getattr(args, "fast_turns", False) else None


def relay_for(bridges: list, seconds: float = PASS_S, read_s: float = RELAY_S,
              clock=None, sleep=None) -> None:
    """Relay the game's log to the workers every `read_s` for `seconds`
    (`watch.relay_for`, #6003).

    A decider reads its board from its seat's events.jsonl (`civvis_orders
    --mirror`), which only these bridges write. Relayed once per half-second
    pass, every board waited for the next pass: in the live six-player game
    (hotseat-20261128T031456Z, turns 1-15, the seats' brain.log stamps) one
    frame's answer to the next was 523 ms median (45 of 47 within 471-579 ms,
    one pass), and one seat's last answer to the next seat's first 1057 ms
    median (two passes), where the decider answers in 10 ms. The ladder relays
    every 50 ms between its passes for the same reason (`civ6_play.py`,
    `read_s`). Only the relay runs this often; everything else keeps the pass.
    The clock and the sleep are read when called, not when `watch` was
    imported, so a caller (and a test) that replaces `time`'s gets its own."""
    watch.relay_for(bridges, seconds, read_s, clock=clock or time.monotonic,
                    sleep=sleep or time.sleep)


#: `quit_civ`'s clock: the whole attempt, the seconds a question that is up
#: is always given to be answered, how many unanswered looks before the menu
#: quit is asked again, and the launcher's own stop behind it all.
QUIT_SECONDS = 150.0
QUIT_ANSWER_SECONDS = 30.0
#: ...and the most that giving can add: the clock has an end whatever is read.
QUIT_OVERTIME_SECONDS = 60.0
QUIT_ASK_AGAIN = 3
QUIT_STOP_SECONDS = 90.0


def mirrored_yes(rows: list[dict], sure: list[dict]) -> list[dict]:
    """Where the quit question's YES stands when the screen reader did not
    return it: its NO, mirrored about the question's heading (the topmost
    ARE YOU SURE row). Nothing without both a heading and a NO beneath it."""
    if not sure:
        return []
    heading = min(sure, key=lambda row: row["y"])
    below = [row for row in find(rows, "NO") if row["y"] > heading["y"]]
    if not below:
        return []
    no = min(below, key=lambda row: row["y"])
    return [{"text": "YES", "x": 2 * heading["x"] - no["x"], "y": no["y"], "norm": _norm("YES")}]


def quit_civ(run_dir: Path, log: Log, timeout_s: float = QUIT_SECONDS) -> bool:
    """Close Civ VI from its own menu and answer its question.

    In a hotseat game the menu quit opens the in-game pause menu with an inline
    ARE YOU SURE? / YES (InGameTopOptionsMenu); nothing answers it on the
    operator's turn, because every CIVVIS context stands down then. This is the
    operator's own request, so the harness answers it, then falls back to the
    launcher's ordinary stop.

    The clock is a loaded host's. Live 2026-11-27 (selfplay-duel-20261127T0654Z,
    load average 40, the Victory screen up): a look took ten seconds, not two,
    the menu came up only on the second asking, and ARE YOU SURE? appeared on
    the sixth look as the sixty seconds ran out; the launcher's stop then sent
    a TERM that cannot cross that question, and Civ VI stayed up on a finished
    game. So the quit is asked again every few unanswered looks, a question
    that is up is always given the time to be answered, and the stop behind it
    waits longer.

    The YES is pressed where it stands when it cannot be read. Live the same
    night (duel-dev-smoke-20261127T0845Z, the person's turn, a full-screen
    window): the screen reader returned ARE YOU SURE?, the sentence under it
    and "No", and never the white-on-red YES, in nine looks out of nine. With
    no YES to press the pause menu's EXIT TO DESKTOP behind the question was
    pressed again each look, each press renewing the clock, and the harness
    never came back. YES and NO stand either side of the heading
    (InGameTopOptionsMenu's inline question), so an unread YES is NO mirrored
    about it (pressed by hand there: Civ VI closed at once). And the time a
    question is given is bounded: the clock runs out `QUIT_OVERTIME_SECONDS`
    past its length at the latest.
    """
    if not env.game_pids():
        return True
    env.request_macos_quit()
    deadline = time.monotonic() + timeout_s
    limit = deadline + QUIT_OVERTIME_SECONDS

    def given_time() -> float:
        return min(limit, max(deadline, time.monotonic() + QUIT_ANSWER_SECONDS))

    step = unanswered = 0
    while time.monotonic() < deadline:
        time.sleep(2.0)
        if not env.game_pids():
            return True
        step += 1
        _, rows = read_screen(run_dir, f"quit-{step}")
        # In a game: the pause menu's inline ARE YOU SURE? / YES.
        sure = find(rows, "ARE YOU SURE", exact=False)
        yes = find(rows, "YES") or mirrored_yes(rows, sure)
        if yes and sure:
            play.click_at(yes[0]["x"], yes[0]["y"])
            deadline = given_time()
            continue
        # On a setup screen: the EXIT TO DESKTOP / OK / Cancel modal, whose OK
        # the live lane finds as Cancel mirrored about the heading.
        if find(rows, "EXIT TO DESKTOP") and not find(rows, "RETURN TO GAME"):
            ok = find(rows, "OK")
            if ok:
                play.click_at(ok[0]["x"], ok[0]["y"])
            else:
                play.confirm_exit_dialog(run_dir / "setup")
            deadline = given_time()
            continue
        # The pause menu without its question yet.
        exit_row = find(rows, "EXIT TO DESKTOP")
        if exit_row and find(rows, "RETURN TO GAME"):
            play.click_at(exit_row[0]["x"], exit_row[0]["y"])
            deadline = given_time()
            continue
        unanswered += 1
        if unanswered % QUIT_ASK_AGAIN == 0:
            log(f"[quit] Civ VI has shown no quit menu in {unanswered} looks; asking again")
            env.request_macos_quit()
    if env.game_pids():
        launcher.stop(timeout_s=QUIT_STOP_SECONDS)
    return not env.game_pids()


def notify(title: str, message: str) -> None:
    """A macOS notification: on screen, touching no window."""
    script = (f"display notification {json.dumps(message)} "
              f"with title {json.dumps(title)} sound name \"Glass\"")
    try:
        subprocess.run(["osascript", "-e", script], capture_output=True, timeout=10)
    except (OSError, subprocess.SubprocessError):
        pass


#: An end-turn blocker as the HUD's seat watch names it, in words.
BLOCKER_WORDS = {
    "ENDTURN_BLOCKING_UNITS": "a unit waiting for orders",
    "ENDTURN_BLOCKING_UNIT_PROMOTION": "a unit promotion",
    "ENDTURN_BLOCKING_PRODUCTION": "a city's production",
    "ENDTURN_BLOCKING_RESEARCH": "the research choice",
    "ENDTURN_BLOCKING_CIVIC": "the civic choice",
    "ENDTURN_BLOCKING_POLICY_SLOT": "a policy slot",
    "ENDTURN_BLOCKING_FILL_CIVIC_SLOT": "a policy slot",
    "ENDTURN_BLOCKING_GOVERNOR_APPOINTMENT": "a governor appointment",
    "ENDTURN_BLOCKING_PANTHEON": "the pantheon choice",
    "ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN": "an envoy to send",
    "ENDTURN_BLOCKING_STACKED_UNITS": "units stacked on one tile",
}


def blocker_words(blocker) -> str:
    if blocker in (None, "", "NO_ENDTURN_BLOCKING"):
        return "nothing the game names (no end-turn blocker)"
    return BLOCKER_WORDS.get(str(blocker), str(blocker))


def seat_stuck_line(monitor: "Monitor", event: dict) -> str:
    """The HUD's `seat_stuck` (CivvisControlHeartbeat.lua `CivvisSeatWatch`): a
    watched seat's turn that has stood past its limit with its controller
    beating or not, in words a person can act on."""
    why = event.get("why")
    what = {"stub_turn": "past the 30 s a test stub's turn may take",
            "long_turn": "longer than any CIVVIS turn takes (180 s)"}.get(why, str(why))
    player = event.get("player")
    name = monitor.name(player) if isinstance(player, int) else f"seat {player}"
    waits = [f"the game waits on {blocker_words(event.get('blocker'))}"]
    if event.get("ready"):
        waits.append(f"first unit still waiting for orders {event['ready']}")
    if event.get("locks"):
        waits.append(f"{event['locks']} event lock(s) held by a screen")
    if event.get("sent"):
        waits.append("its end of turn already sent")
    if event.get("processing"):
        waits.append("the game still processing messages")
    if event.get("controller_ran") is False:
        then = ("No CIVVIS controller has run on this seat, so the HUD leaves the turn alone: "
                "it waits for a person to play it (Start Turn, then NEXT TURN)")
    else:
        then = ("The HUD ends it (every 20 s, up to six times a turn); if it still stands, "
                "look at the game for a screen waiting on a click and close it")
    return (f"⚠ turn {event.get('turn')}: {name}'s turn has stood {event.get('age')} s, {what}, "
            f"its controller quiet {event.get('quiet')} s; " + ", ".join(waits) + f". {then}")


def seat_between_line(monitor: "Monitor", event: dict) -> str:
    """The HUD's `seat_between`: a watched seat that is the local player with its
    turn not active, and the next seat not begun, for a minute or more."""
    player = event.get("player")
    name = monitor.name(player) if isinstance(player, int) else f"seat {player}"
    state = [f"its end of turn {'sent' if event.get('sent') else 'NOT sent'}",
             f"game core busy {event.get('busy')}", f"processing messages {event.get('processing')}",
             f"{event.get('locks') or 0} event lock(s) held by a screen"]
    return (f"⚠ turn {event.get('turn')}: {name} has been between turns {event.get('between')} s and "
            f"the next seat has not begun (" + ", ".join(state) + "). Nothing in the game can end a "
            "turn that is not active: if Civ VI shows a screen, close it; if this is said again a "
            "minute from now, save from the game menu (Esc), quit, and resume the save (--resume).")


def stand_in_stuck_line(monitor: "Monitor", event: dict) -> str:
    """The test stub's own `stand_in_stuck` (CivvisControlAgent.lua
    `CivvisSeat.standInStep`): what holds its turn, one line a record."""
    refusing = []
    for unit in event.get("refusing") or []:
        if not isinstance(unit, dict):
            continue
        detail = [str(unit.get("activity") or "?"), f"moves {unit.get('moves')}/{unit.get('max')}"]
        if unit.get("why"):
            detail.append(str(unit["why"]))
        refusing.append(f"{unit.get('kind')} {unit.get('id')} ({', '.join(detail)})")
    held = [f"{lock.get('ctx') or '?'}" + (f" (turn {lock['turn']})" if lock.get("turn") is not None else "")
            for lock in event.get("held_locks") or [] if isinstance(lock, dict)]
    parts = [f"the game waits on {blocker_words(event.get('blocker'))}",
             f"answered {event.get('answered')}"]
    if refusing:
        parts.append("refusing every hold: " + "; ".join(refusing))
    if event.get("visible"):
        parts.append("screens up: " + ", ".join(str(v) for v in event["visible"]))
    if held:
        parts.append("event locks held by: " + ", ".join(held))
    parts.append(f"end of turn forced {event.get('forced')} time(s), sent {event.get('sent')}")
    if event.get("game_paused"):
        parts.append("the game still paused")
    return (f"⚠ turn {event.get('turn')}: the test stub's turn stands after {event.get('seconds')} s "
            f"(the stand-in's step {event.get('step')}): " + ", ".join(parts))


def ended_at(event: dict) -> dict:
    """When the game said it ended, from the event's own clock (`t`, the
    mod's seconds): the same answer however often the record is read back, so
    a game's line on the people's record is written once. Nothing for an event
    with no clock."""
    try:
        at = datetime.fromtimestamp(int(event["t"]), timezone.utc)
    except (KeyError, TypeError, ValueError, OverflowError, OSError):
        return {}
    return {"at": at.strftime("%Y-%m-%dT%H:%M:%SZ")}


class Monitor:
    """Who is playing, from the mod's own events, and how long each seat took."""

    def __init__(self, run_dir: Path, tag: str, seats: list[int], args: argparse.Namespace,
                 log: Log):
        self.run_dir, self.tag, self.seats, self.args, self.log = run_dir, tag, seats, args, log
        self.names = {seat: seat_name(i, getattr(args, "agents", None) and agent)
                      for i, (seat, agent) in enumerate(seat_agents(args, seats).items(), 1)}
        self.unattended = bool(getattr(args, "civvis_seats", None))
        watched = list(seats) if self.unattended else agent_seats(len(seats) + 1, args.auto_human)
        # The game's ending, for an unattended table: each CIVVIS seat's own
        # `seat` event (civ, leader, team, the host's victory table), the
        # victory event, and which of our seats were eliminated.
        self.seat_info: dict[int, dict] = {}
        self.defeated: set[int] = set()
        self.result: dict | None = None
        # `--victories-off`, read back: what the game's own switches say, and
        # the first asked-off victory a seat found still on (or unreadable).
        self.victories_off = list(getattr(args, "victories_off", None) or [])
        self.victories: dict | None = None
        self.setup_fault: str | None = None
        if args.auto_human:
            self.names[HUMAN_SEAT] = "Test stub"
        self.base = Journal(run_dir / "events.jsonl")
        self.seat_journals = {seat: Journal(run_dir / modinstall.hotseat_seat_tag(tag, seat)
                                            / "events.jsonl") for seat in watched}
        self.active: int | None = None
        self.active_human = False
        self.active_since = time.monotonic()
        self.last_event = time.monotonic()
        self.turn = -1
        self.started = False
        self.stats: dict[int, dict] = {seat: {"turns": 0, "orders_applied": 0, "found": 0,
                                              "last": {}, "errors": 0} for seat in watched}
        self.timings = (run_dir / "turns.jsonl").open("a", buffering=1)
        self.notified_turn = -1
        self.rescued_at = 0.0
        self.quiet = False
        self.seat_stalls = 0
        # `--manual-lobby`: the game the person set up, as the HUD read it
        # back (`hotseat_lobby`, CivvisControlHeartbeat.lua), oldest first.
        self.lobbies: list[dict] = []
        # `--manual-lobby`: when the last stall note was said (None: never).
        self.stall_noted_at: float | None = None
        # The cover over the jack seats' turns (CivvisHotseatCover.lua), as its
        # own `hotseat_cover` events last said, and since when.
        self.cover_up = False
        self.cover_since = time.monotonic()
        self.cover_yields = 0
        # Which seats began a turn as Hot Seat humans, by game turn: a CIVVIS
        # seat that never does is Civ VI's AI (or closed), whatever the setup
        # meant (`note_turn`).
        self.round_seats: dict[int, set[int]] = {}
        self.seats_checked = False
        self.unseated: list[int] = []
        self.foreign_said: set[int] = set()

    def catch_up(self) -> None:
        """Read the record so far without replaying it (`--attach`): learn whose
        turn it is, then say so once."""
        self.quiet = True
        try:
            self.poll()
        finally:
            self.quiet = False
        if self.active is None:
            self.log(f"attached at turn {self.turn}; waiting for the next seat")
        elif self.active_human:
            self.notified_turn = -1
            self.your_turn(self.turn)
        else:
            self.log(f"attached: turn {self.turn}, {self.name(self.active)} is playing")

    def name(self, player: int) -> str:
        if player == HUMAN_SEAT and not self.unattended:
            if self.args.auto_human:
                return "test stub (seat 0)"
            person = person_name(self.args)
            return f"{person} (you, seat 0)" if person else "you (Player 1)"
        return f"{self.names.get(player, f'seat {player}')} (seat {player})"

    def poll(self) -> None:
        for event in self.base.read():
            self.last_event = time.monotonic()
            self.on_base(event)
        for seat, journal in self.seat_journals.items():
            for event in journal.read():
                self.last_event = time.monotonic()
                self.on_seat(seat, event)

    def say(self, message: str) -> None:
        if not self.quiet:
            self.log(message)

    def our_teams(self) -> dict:
        """Team -> our seat. A free-for-all puts each player on its own team,
        so a seat whose event names no team is taken as its own."""
        return {int(info.get("team", seat)) if info.get("team") is not None else seat: seat
                for seat, info in ((seat, self.seat_info.get(seat, {})) for seat in self.seats)}

    def victory_result(self, event: dict) -> dict | None:
        """A `victory` record as the table's result. The table's own record
        (CivvisControlHeartbeat.lua, `CivvisEnding`) names the winning team's
        players (`members`), and the result is ours when one of them is one of
        our seats, the person's when theirs is; a seat's own record names a
        team only, taken as its seat's (`our_teams`)."""
        team = event.get("team")
        players = None
        try:
            if isinstance(event.get("members"), list) and event["members"]:
                players = sorted({int(player) for player in event["members"]})
            if team is not None:
                team = int(team)
        except (TypeError, ValueError):
            return None
        if players is not None:
            won = [seat for seat in self.seats if seat in players]
            ours, winner_seat = bool(won), (won[0] if won else None)
        else:
            teams = self.our_teams()
            ours = team is not None and team in teams
            winner_seat = teams.get(team) if ours else None
        result = {"kind": "victory", "team": team, "victory": event.get("victory"),
                  "turn": event.get("turn"), "ours": ours, "winner_seat": winner_seat}
        if players is not None:
            result["members"] = players
            if not self.unattended:
                result["person"] = HUMAN_SEAT in players
        if event.get("victory_type"):
            # The victory by name (`victory` is the host's own index).
            result["victory_type"] = event["victory_type"]
        result.update(ended_at(event))
        return result

    def on_ending(self, seat: int | None, event: dict) -> None:
        """A victory ends the game for every seat; an elimination is one seat's.
        The first ending stands: a game played on after "one more turn" keeps
        the victory it had. Each ending can arrive twice (the table writes it,
        and so does the seat context that was local, if any)."""
        kind = event.get("kind")
        if kind == "victory":
            won = self.victory_result(event)
            if won is None:
                return
            if self.result is None:
                self.result = won
                who = (self.name(won["winner_seat"]) if won["ours"]
                       else self.name(HUMAN_SEAT) if won.get("person") else f"team {won['team']}")
                self.log(f"game over at turn {event.get('turn')}: {who} won "
                         f"({'ours' if won['ours'] else 'a rival'}), victory "
                         f"{won.get('victory_type') or event.get('victory')}")
            elif (self.result.get("kind") == "victory" and "members" not in self.result
                  and "members" in won and self.result.get("team") == won["team"]):
                # The same victory, the table's word after a seat's: its
                # players say who won.
                self.result.update({key: won[key] for key in ("ours", "winner_seat", "members",
                                                              "person") if key in won})
        elif kind == "defeat":
            self.on_defeat(seat, event)

    def time_defeat(self, event: dict) -> bool:
        """Whether a `defeat` is the turn limit's with the Score victory off
        (`DEFEAT_TIME`, every seat's at once: Victories.xml:31 `Global="true"`,
        its requirements the Score victory disabled and the max turn reached),
        an ending nobody won. The table's record names the type; a seat's own
        names only the host's index, and is that defeat when it falls on the
        turn limit of a game whose Score victory is off."""
        if event.get("defeat_type"):
            return event["defeat_type"] == "DEFEAT_TIME"
        score_off = "VICTORY_SCORE" in self.victories_off or (
            isinstance(self.victories, dict) and self.victories.get("score") is False)
        try:
            return bool(score_off and int(event.get("turn")) >= int(self.args.max_turns))
        except (AttributeError, TypeError, ValueError):
            return False

    def on_defeat(self, seat: int | None, event: dict) -> None:
        """One player's elimination: a seat's own (its context's record,
        `ours`), or any watched seat's as the table writes it
        (CivvisControlHeartbeat.lua `CivvisEnding`: `player`, `defeat_type`,
        `by`). A seat's record of another player is left to the table's.
        Every CIVVIS seat gone with the game still on is the person's game
        (`defeat`); the person gone while CIVVIS seats play on is theirs lost
        (`eliminated`), to the seat that took their last city where the table
        says (`by`); the turn limit with no Score victory is nobody's (`time`)."""
        try:
            if seat is not None:
                if not event.get("ours"):
                    return
                player = seat
            else:
                player = int(event["player"])
        except (KeyError, TypeError, ValueError):
            return
        turn = event.get("turn")
        if self.time_defeat(event):
            if self.result is None:
                self.result = {"kind": "time", "turn": turn, "ours": False, **ended_at(event)}
                self.log(f"game over at turn {turn}: the turn limit with the Score victory off; "
                         "nobody won")
            return
        if player in self.seats:
            if player not in self.defeated:
                self.defeated.add(player)
                self.log(f"{self.name(player)} was eliminated at turn {turn}")
            if self.result is None and self.defeated >= set(self.seats):
                self.result = {"kind": "defeat", "turn": turn, "ours": False, **ended_at(event)}
                self.log("every CIVVIS seat is eliminated")
        elif player == HUMAN_SEAT and not self.unattended and self.result is None:
            try:
                taker = int(event["by"]) if event.get("by") is not None else None
            except (TypeError, ValueError):
                taker = None
            ours = taker in self.seats
            self.result = {"kind": "eliminated", "player": HUMAN_SEAT, "turn": turn, "ours": ours,
                           "winner_seat": taker if ours else None, "by": taker,
                           "defeat_type": event.get("defeat_type"), **ended_at(event)}
            self.log(f"{self.name(HUMAN_SEAT)} eliminated at turn {turn}"
                     + (f"; the last city fell to {self.name(taker)}" if taker is not None else "")
                     + "; the CIVVIS seats play on")

    def on_base(self, event: dict) -> None:
        kind = event.get("kind")
        if kind in ("victory", "defeat"):
            self.on_ending(None, event)
            return
        if kind == "hotseat_lobby":
            self.lobbies.append(event)
        elif kind == "hotseat_armed":
            self.started = True
            self.say("the hand-over screen is under CIVVIS control")
        elif kind == "hotseat_person":
            self.say(f"seat {event.get('seat')} is named {event.get('shown')!r} in the game"
                     + ("" if event.get("applied") else " (the name was NOT taken)"))
        elif kind == "hotseat_turn":
            self.started = True
            player, turn = int(event.get("player", -1)), int(event.get("turn", -1))
            now = time.monotonic()
            if event.get("phase") == "begin":
                self.note_turn(player, turn)
                self.active, self.turn, self.active_since = player, turn, now
                self.active_human = bool(event.get("human"))
                if self.active_human:
                    if not self.quiet:
                        self.your_turn(turn)
                else:
                    self.say(f"turn {turn}: {self.name(player)} is playing")
            elif event.get("phase") == "end":
                took = now - self.active_since if self.active == player else None
                row = {"utc": utc(), "turn": turn, "player": player,
                       "human": bool(event.get("human")),
                       "seconds": round(took, 1) if took is not None else None}
                if not self.quiet:
                    self.timings.write(json.dumps(row) + "\n")
                took_text = f" in {took:.1f} s" if took is not None else ""
                if event.get("human"):
                    self.say(f"turn {turn}: you ended your turn{took_text}; the CIVVIS seats play now")
                else:
                    self.say(f"turn {turn}: {self.name(player)} ended its turn{took_text}")
                if self.active == player:
                    self.active = None
        elif kind == "hotseat_saved":
            self.say(f"saved the game as {event.get('name')!r} (Load Game -> Hot Seat)")
        elif kind == "hotseat_start_turn" and event.get("pressed") is False:
            seat = event.get("player")
            self.say(f"⚠ could not press Start Turn for seat {seat}: "
                     + ("press it on your hand-over screen" if seat == HUMAN_SEAT and not self.unattended
                        else f"its hand-over screen is waiting; press Start Turn there for "
                             f"{self.name(seat) if isinstance(seat, int) else 'it'}"))
        # The HUD's seat watch (CivvisControlHeartbeat.lua, `CivvisSeatWatch`): a
        # CIVVIS seat whose turn is active while its controller does not move.
        elif kind == "seat_quiet":
            self.seat_stalls += 1
            self.say(f"⚠ turn {event.get('turn')}: seat {event.get('player')} quiet for "
                     f"{event.get('quiet')} s on its own turn (paused {event.get('paused')}, "
                     f"turn complete sent {event.get('sent')}, blocker {event.get('blocker')}, "
                     f"hand-over hidden {event.get('handover_hidden')}, controller ran "
                     f"{event.get('controller_ran')})")
        elif kind == "seat_quiet_end_turn":
            self.say(f"turn {event.get('turn')}: the HUD ended seat {event.get('player')}'s quiet "
                     f"turn (asked {event.get('asked')}, request {event.get('forces')})")
        # The watch's turn-age rule (#6007): a turn that stands though its
        # controller beats, and a seat stuck between turns.
        elif kind == "seat_stuck":
            self.seat_stalls += 1
            self.say(seat_stuck_line(self, event))
        elif kind == "seat_stuck_end_turn":
            how = ("unreadied its end of turn, which the game had dropped, so it is sent again"
                   if event.get("how") == "unready" else "asked the end of turn by force")
            self.say(f"turn {event.get('turn')}: the HUD {how} for seat {event.get('player')}'s "
                     f"stuck turn (asked {event.get('asked')}, request {event.get('forces')})")
        elif kind == "seat_between":
            self.seat_stalls += 1
            self.say(seat_between_line(self, event))
        elif kind == "hotseat_pause_released":
            self.say(f"⚠ turn {event.get('turn')}: seat {event.get('player')}'s hand-over pause stood "
                     f"{event.get('waited')} s with no Start Turn on screen; released "
                     f"(still paused {event.get('still_paused')})")
        elif kind == "hotseat_cover":
            up = bool(event.get("up"))
            if up != self.cover_up:
                self.cover_up, self.cover_since = up, time.monotonic()
            if str(event.get("reason", "")).startswith("yield"):
                self.cover_yields += 1
                self.say(f"⚠ turn {event.get('turn')}: the cover stood aside for seat "
                         f"{event.get('player')} ({event.get('reason')}: quiet "
                         f"{event.get('quiet')} s, between turns {event.get('between')} s); "
                         "the game is on screen until that turn ends")

    def note_turn(self, player: int, turn: int) -> None:
        """A seat's turn began on the hand-over screen, so it is a Hot Seat
        human in this game: Civ VI's own AI seats never get one.

        The game's own word on which seats CIVVIS really plays, whoever set the
        room up: on 2026-11-28 the harness gave up seating a six-player room,
        the person finished it by hand, and the workers had been started for
        five seats before anything said which of them were seated. Checked on
        the first whole round (from seat 0's turn to its next): a CIVVIS seat
        that took no turn in it is not ours to play, and its worker goes
        (`take_unseated`); a Human seat that is not CIVVIS's is said at once,
        as its turns wait for a person."""
        if player < 0 or turn < 0:
            return
        ours = set(self.seats)
        if (player not in ours and (self.unattended or player != HUMAN_SEAT)
                and player not in self.foreign_said):
            self.foreign_said.add(player)
            self.log(f"⚠ row {player + 1} is a Human seat in this game that CIVVIS does not play: "
                     "each of its turns waits on its hand-over screen for a person to press Start "
                     "Turn and play it. To leave it to Civ VI's AI instead, quit to the main menu and "
                     "set the game up again with that row set to Computer.")
        self.round_seats.setdefault(turn, set()).add(player)
        whole = self.round_seats.get(turn - 1)
        if player != 0 or self.seats_checked or not whole or 0 not in whole:
            return
        self.seats_checked = True
        missing = [seat for seat in self.seats if seat not in whole]
        if not missing:
            self.say(f"seats verified: every CIVVIS seat ({_rows(self.seats)}) took turn {turn - 1} "
                     "as a Hot Seat human")
            return
        self.unseated += missing
        # A seat this run saw eliminated takes no turn either: no fault of the setup.
        gone = [seat for seat in missing if seat in self.defeated]
        if gone:
            self.say(f"{_rows(gone)}: eliminated, so {'its decision worker is' if len(gone) == 1 else 'their decision workers are'} stopped")
        missing = [seat for seat in missing if seat not in self.defeated]
        if not missing:
            return
        one = len(missing) == 1
        if turn - 1 > 1:
            # A game picked up part-way (`--resume`, `--attach`): a seat eliminated
            # before this run began has no turn and no record of its end here.
            self.log(f"⚠ {_rows(missing)} took no turn {turn - 1} on the hand-over screen: "
                     f"eliminated earlier in this game, or not a Hot Seat human (Civ VI's own AI "
                     f"plays {'it' if one else 'them'}, or the row is closed). CIVVIS does not play "
                     f"{'it' if one else 'them'}, and {'its decision worker is' if one else 'their decision workers are'} "
                     "stopped. If a CIVVIS seat should still be playing, quit to the main menu and "
                     "load the game again with the row set to Human.")
            return
        self.log(f"⚠ {_rows(missing)} {'is' if one else 'are'} not a Hot Seat human in this game "
                 f"(no turn {turn - 1} on the hand-over screen): Civ VI's own AI plays "
                 f"{'it' if one else 'them'}, or the row is closed, so CIVVIS does not, and "
                 f"{'its decision worker is' if one else 'their decision workers are'} stopped. "
                 "For CIVVIS to play it, quit to the main menu and set the game up again with the "
                 "row set to Human.")

    def take_unseated(self) -> list[int]:
        """The CIVVIS seats found not to be Hot Seat humans, each once."""
        taken, self.unseated = self.unseated, []
        return taken

    def your_turn(self, turn: int) -> None:
        if turn == self.notified_turn:
            return
        self.notified_turn = turn
        bar = "=" * 64
        if getattr(self.args, "keep_handover", False):
            line = "Press \"Start Turn\" in Civ VI."
        else:
            line = "Your map is on the screen."
        print(f"\a\n{bar}\n  YOUR TURN — turn {turn}.  {line}\n"
              f"  Nothing of CIVVIS's touches the screen until you end it.\n{bar}\n",
              flush=True)
        self.log(f"turn {turn}: your turn")
        if not self.args.no_notify:
            notify("Civ VI — your turn", f"Turn {turn}: the CIVVIS seats are done. Your move.")

    def on_seat(self, seat: int, event: dict) -> None:
        kind = event.get("kind")
        stats = self.stats[seat]
        if kind in ("victory", "defeat"):
            self.on_ending(seat, event)
            return
        if kind == "seat":
            self.seat_info[seat] = {k: event.get(k) for k in
                                    ("civ", "leader", "team", "player", "difficulty", "map",
                                     "size", "speed", "victory_types", "players")}
            self.read_back_victories(seat, event)
        if kind == "hotseat_seat":
            self.say(f"seat {seat} is {event.get('shown')!r} "
                     f"(asked {event.get('name')!r}, applied {event.get('applied')})")
        elif kind == "seat":
            self.say(f"{self.name(seat)} plays {event.get('leader')} / {event.get('civ')} "
                     f"at {event.get('difficulty')}, {event.get('speed')}, "
                     f"{event.get('map')} {event.get('size')}, modes {event.get('modes')}")
        elif kind == "turn":
            stats["turns"] += 1
            stats["orders_applied"] += int(event.get("orders_applied") or 0)
            stats["last"] = {k: event.get(k) for k in
                             ("turn", "cities", "units", "army", "score", "met",
                              "orders_source", "orders_applied", "gold")}
            # `orders_applied` is the turn's first frame; the orders of its
            # later frames are counted from the `orders` rows below.
            self.say(f"turn {event.get('turn')}: {self.name(seat)} — "
                     f"{event.get('orders_applied') or 0} opening orders applied "
                     f"({event.get('orders_source')}), cities {event.get('cities')}, "
                     f"units {event.get('units')}, score {event.get('score')}")
        elif kind == "orders":
            stats["orders_all"] = stats.get("orders_all", 0) + int(event.get("applied") or 0)
        elif kind == "end_turn_wait":
            # A turn its own controller had to end by force (`forced` requests).
            if int(event.get("forced") or 0) > 0:
                stats["forced_turns"] = stats.get("forced_turns", 0) + 1
        elif kind == "stand_in_stuck":
            # The test stub's stand-in stepped in (its first step, then every
            # 30 s while the turn stands): counted once a turn.
            stepped = stats.setdefault("stand_in_turns", [])
            if event.get("turn") not in stepped:
                stepped.append(event.get("turn"))
            self.say(stand_in_stuck_line(self, event))
        elif kind == "found":
            stats["found"] += 1
        elif kind == "error":
            stats["errors"] += 1
            if stats["errors"] <= 5:
                self.say(f"⚠ {self.name(seat)} mod error: {str(event)[:200]}")

    def read_back_victories(self, seat: int, event: dict) -> None:
        """A victory asked off is off only when the game says so: the seat's
        `seat` event carries `Game.IsVictoryEnabled` per type (the shipped
        WorldRankings.lua check). Anything but a plain `false` is a fault."""
        switches = event.get("victories")
        if isinstance(switches, dict):
            self.victories = switches
        if not self.victories_off:
            return
        wrong = [victory for victory in self.victories_off
                 if not isinstance(switches, dict)
                 or switches.get(victory_switch_key(victory)) is not False]
        if wrong:
            if self.setup_fault is None:
                self.setup_fault = ("victory switch not applied: " + ", ".join(wrong)
                                    + f" asked off, the game reads {switches!r}")
                self.log(f"⚠ {self.name(seat)}: {self.setup_fault}")
        else:
            self.say(f"{self.name(seat)}: victories off, read back from the game: "
                     + ", ".join(self.victories_off))

    def stalled(self, seconds: float) -> bool:
        return (self.active is not None and not self.active_human
                and time.monotonic() - self.last_event > seconds)

    #: The longest a cover is believed up with a seat stalled under it. The
    #: mod's cover stands aside after 90 s of a quiet seat (and says so, flush
    #: line and all); past this the screen rescue looks anyway.
    COVER_TRUST_S = 600.0

    def covered(self) -> bool:
        """The cover is up: a screenshot shows it, not the game."""
        return self.cover_up and time.monotonic() - self.cover_since < self.COVER_TRUST_S

    def close(self) -> None:
        self.timings.close()


def rescue_popup(monitor: Monitor, log: Log) -> None:
    """Our seat has gone quiet: try the visual popup rescue, but only on our turn,
    with Civ VI already in front and nobody at the keyboard."""
    if time.monotonic() - monitor.rescued_at < 60:
        return
    monitor.rescued_at = time.monotonic()
    monitor.poll()
    if monitor.active is None or monitor.active_human:
        return
    if monitor.covered():
        # Under the cover the screen shows the cover. The HUD's seat watch ends
        # a quiet turn by itself, and the cover stands aside for a seat that
        # stays quiet; the rescue reads the screen once it does.
        log(f"{monitor.name(monitor.active)} has been quiet for a while under the cover; "
            "the mod's seat watch has it, and the cover stands aside if it stays quiet")
        return
    try:
        front = popup_clear.frontmost()
    except (OSError, subprocess.SubprocessError):
        front = ""
    if not front.startswith("Civ6"):
        log(f"{monitor.name(monitor.active)} has been quiet for a while; Civ VI is not in "
            "front, so nothing is clicked (bring it forward if a screen is waiting)")
        return
    if operator_presence.operator_active():
        return
    _, rows = read_screen(monitor.run_dir, "rescue")
    if find(rows, "Start Turn"):
        log("a hand-over screen is up; the mod owns it, nothing is clicked")
        return
    handled, reason = play.dismiss_visually_confirmed_popup(
        diagnostic_path=monitor.run_dir / "setup" / "rescue-popup.png")
    log(f"stalled seat {monitor.active}: popup rescue — {reason}")
    if not handled:
        end_stalled_turn(monitor, log, rows)


def end_stalled_turn(monitor: Monitor, log: Log, rows: list[dict]) -> bool:
    """A CIVVIS seat that sits on its own turn with no popup up: press NEXT TURN.

    Measured on the first unattended table (civvis-t6-20261121T131243Z, 2026-11-21):
    a World Congress session on seat 1's turn 61 was voted by both CIVVIS seats and
    the seat's ENDTURN_BLOCKING_WORLD_CONGRESS_SESSION force-dismissed, and then the
    turn never ended: the map showed NEXT TURN, nothing to dismiss, the AI phase
    stalled for 11 minutes. Pressing NEXT TURN played on at once. Only a CIVVIS
    seat's turn (never a person's), with the caller's guards (Civ VI in front,
    nobody at the keyboard) already passed."""
    if monitor.active is None or monitor.active_human or monitor.active not in monitor.seats:
        return False
    hits = find(rows, "NEXT TURN")
    if not hits:
        return False
    click(log, (hits[0]["x"], hits[0]["y"]))
    log(f"stalled seat {monitor.active}: nothing to dismiss and NEXT TURN on the map; pressed it")
    return True


def smoke_report(monitor: Monitor, log: Log) -> dict:
    report = {"utc": utc(), "turn": monitor.turn, "seats": {}}
    for seat, stats in monitor.stats.items():
        report["seats"][str(seat)] = dict(stats, name=monitor.names.get(seat))
        last = stats["last"]
        log(f"report: {monitor.names.get(seat)}: {stats['turns']} turns played, "
            f"{stats.get('orders_all') or stats['orders_applied']} orders applied, "
            f"{stats['found']} cities founded, "
            f"now {last.get('cities')} cities / {last.get('units')} units, "
            f"score {last.get('score')}, source {last.get('orders_source')}"
            + (f", {stats['forced_turns']} turns ended by force" if stats.get("forced_turns") else "")
            + (f", the stand-in stepped in on {len(stats.get('stand_in_turns') or [])} turns"
               + (f" ({', '.join(str(t) for t in stats['stand_in_turns'][:12])})"
                  if stats.get("stand_in_turns") else "")
               if seat == HUMAN_SEAT and monitor.args.auto_human else ""))
    return report


# ------------------------------------------------------------ the lifecycle

def table_record(monitor: Monitor, args: argparse.Namespace, seats: list[int],
                 began: dict) -> dict:
    """What a table's summary says of its game: the ending and the setup, for
    an unattended table and for a person's (theirs is what rates them,
    `tools/people_elo.py record`), with the person, their leader and when the
    game began on theirs. A test stub's game has no ending to keep."""
    if args.auto_human and not monitor.unattended:
        return {}
    record = {
        "result": monitor.result, "seats": {str(k): v for k, v in monitor.seat_info.items()},
        "eliminated": sorted(monitor.defeated), "turn": monitor.turn,
        "difficulty": args.difficulty, "players": args.players, "map": args.map,
        "map_size": args.map_size, "speed": args.speed, "max_turns": args.max_turns,
        "civvis_seats": seats, "victory_target": args.victory,
        "seat_agents": seat_agents(args, seats) if getattr(args, "agents", None) else None,
        "victories_off": monitor.victories_off, "victories": monitor.victories,
        "setup_fault": monitor.setup_fault,
        "seat_stalls": monitor.seat_stalls,
        # What the CIVVIS seats played: the live ladder's genes or the stock
        # genome (`live_genome`), so a rated game says what it was against.
        "genome": getattr(args, "genome_record", None)}
    record.update(began)
    if not monitor.unattended:
        record.update({"person": person_name(args), "person_seat": HUMAN_SEAT,
                       "leader": args.leader})
    return record


#: What Civ VI was seen to hold with a late duel loaded (6.87 GB, 2026-11-27)
#: and a little to breathe: with less memory than this to spare, macOS closes
#: it. And the load average, per core, past which a look at the screen takes
#: ten seconds and System Events stops answering (measured the same night).
CIV_MEMORY_GB = 8.0
LOAD_PER_CORE = 2.0


def host_strain(run=subprocess.run) -> dict:
    """What this Mac has to spare as a game starts: memory free (macOS's own
    `memory_pressure` figure, as a share and in GB), swap in use, the load
    averages and the cores. A reading the host does not give is `None`."""
    strain: dict = {"memory_free": None, "memory_free_gb": None, "swap_used_mb": None,
                    "swap_total_mb": None, "load": play.host_load(), "cores": os.cpu_count()}

    def said(*command: str) -> str:
        try:
            return run(list(command), capture_output=True, text=True, check=False, timeout=20).stdout
        except (OSError, subprocess.SubprocessError):
            return ""

    free = re.search(r"free percentage:\s*(\d+)%", said("memory_pressure"))
    total = re.search(r"\d+", said("sysctl", "-n", "hw.memsize"))
    if free:
        strain["memory_free"] = int(free[1]) / 100
        if total:
            strain["memory_free_gb"] = round(strain["memory_free"] * int(total[0]) / 2 ** 30, 1)
    swap = said("sysctl", "-n", "vm.swapusage")
    for key, label in (("swap_total_mb", "total"), ("swap_used_mb", "used")):
        found = re.search(rf"{label} = ([\d.]+)M", swap)
        if found:
            strain[key] = round(float(found[1]))
    return strain


def strain_warnings(strain: dict) -> list[str]:
    """What to say before a game starts on a Mac with too little to spare.

    Live 2026-11-27, one evening, a dozen sessions compiling beside the game:
    macOS ran out of application memory with Civ VI at 6.87 GB and offered to
    force-quit it (the game fell back to its main menu from the staging
    room), and at a load average of 40 the window query went unanswered and
    START GAME was taken as pressed. Neither looked like what it was."""
    warnings = []
    free = strain.get("memory_free_gb")
    if free is not None and free < CIV_MEMORY_GB:
        warnings.append(f"only {free:.1f} GB of memory is free ({strain['memory_free']:.0%}); Civ VI "
                        f"takes about {CIV_MEMORY_GB - 1:.0f} GB and macOS closes it when there "
                        "is none: quit other apps, or stop the builds, first")
    load, cores = strain.get("load"), strain.get("cores")
    if load and cores and load[0] > LOAD_PER_CORE * cores:
        warnings.append(f"the load average is {load[0]:.0f} on {cores} cores; the setup's "
                        "screen reads and clicks will be slow and may need a second try")
    return warnings


def strain_line(strain: dict) -> str:
    free, load = strain.get("memory_free_gb"), strain.get("load")
    return ("host: "
            + (f"{free:.1f} GB memory free ({strain['memory_free']:.0%})" if free is not None
               else "memory unread")
            + (f", swap {strain['swap_used_mb']} of {strain['swap_total_mb']} MB"
               if strain.get("swap_used_mb") is not None and strain.get("swap_total_mb") else "")
            + (f", load {load[0]:.0f} on {strain.get('cores')} cores" if load else ""))


#: In the run directory: the Civ VI process this run launched.
LAUNCHED = "launched.json"


def note_launched(run_dir: Path, pid: int) -> None:
    (run_dir / LAUNCHED).write_text(json.dumps({"pid": int(pid), "utc": utc()}))


def launched_here(run_dir: Path, running: list[int] | None = None) -> bool:
    """Whether the Civ VI now running is the one this run launched: a harness
    that comes up by `--attach` (the supervisor's, after the first one died)
    closes it at the game's ending or on `--stop --quit-game` as the first
    would have. A Civ VI the person opened and loaded a save into is not ours
    to close, and a run with no record of a launch never closes one.

    Live 2026-11-27 (selfplay-duel-20261127T0654Z): the harness was killed at
    turn 31 and re-attached in four seconds, the game ended at turn 111, and
    the re-attached harness, which had launched nothing itself, left Civ VI on
    the Victory screen for the supervisor to re-attach to every 25 seconds."""
    try:
        pid = int(json.loads((run_dir / LAUNCHED).read_text())["pid"])
    except (OSError, ValueError, KeyError, TypeError):
        return False
    return pid in (env.game_pids() if running is None else running)


def write_current(payload: dict) -> None:
    RUN_ROOT.mkdir(parents=True, exist_ok=True)
    CURRENT.write_text(json.dumps(payload, indent=2))


def read_current() -> dict | None:
    try:
        return json.loads(CURRENT.read_text())
    except (OSError, ValueError):
        return None


#: The bundle's program as a harness: its own name, not one of its workers.
_APP_HARNESS = re.compile(r'(?:^|[\\/"\s])civvis-hotseat(?:\.exe)?(?:"|\s|$)')


def is_harness_command(command: str) -> bool:
    """Whether a command line is this harness: the script, or the bundle's
    program (`tools/package_hotseat.py`, or `tools/civvis_hotseat_app.py` run
    from a checkout) when it is not a decision worker."""
    if "civ6_hotseat.py" in command:
        return True
    if WORKER_COMMAND in command:
        return False
    return bool(_APP_HARNESS.search(command)) or "civvis_hotseat_app.py" in command


def harness_alive(pid: int) -> bool:
    return is_harness_command(env.process_command(pid))


#: How long `--stop` waits for the harness to finish: its workers, the
#: record, and with `--quit-game` the whole of `quit_civ` (which can take
#: minutes on a loaded host) on top.
STOP_WAIT_S = 120.0


def stop_wait(quit_game: bool) -> float:
    return STOP_WAIT_S + (QUIT_SECONDS + QUIT_OVERTIME_SECONDS + QUIT_STOP_SECONDS if quit_game else 0.0)


#: Lines of a stopping harness's log that the person who ran `--stop` needs to
#: see: what became of the record, and of Civ VI.
STOP_NEWS = ("[people]", "people's record", "closing Civ VI", "did not close", "resignation")


def stopped_news(log_path: Path, offset: int) -> list[str]:
    try:
        with log_path.open("r", errors="replace") as handle:
            handle.seek(offset)
            text = handle.read()
    except OSError:
        return []
    return [line for line in text.splitlines() if any(news in line for news in STOP_NEWS)]


def installed_game_config() -> dict:
    """The control mod's installed config, or {} where there is none to read:
    no mod installed, or no Civilization VI found at all (`civ6_env.install_dir`
    exits then). `--status` and `--stop --resign` answer on a computer with no
    Civ VI where they expect one: the bundle's own check runs them on a bare
    runner, and a player's game can sit where no search looks. From #6015 to
    2026-11-28 both exited 1 there with "Civilization VI install not found"
    (`hotseat-bundle` runs 46110252483 and 46111136386, which then uploaded no
    bundle)."""
    try:
        return modinstall.installed_config() or {}
    except SystemExit:
        return {}


def resign_unattended(current: dict | None, say=print) -> int:
    """`--stop --resign` with no harness running: the game's record is put
    on the people's record from the run's own summary, which every harness
    writes as it stops (`--stop`, Ctrl-C, Civ VI closed). Before 2026-11-28
    this printed "no hotseat harness is running" and the resignation was lost."""
    run_dir = Path(current["run_dir"]) if current and current.get("run_dir") else None
    if run_dir is None:
        installed = installed_game_config()
        if installed.get("Hotseat") and installed.get("RunTag"):
            run_dir = RUN_ROOT / installed["RunTag"]
    if run_dir is None:
        say("no hotseat game to resign: no harness is running and no hotseat game is installed. "
            "To record a game you gave up: tools/people_elo.py record <run_dir> --winner civvis")
        return 2
    try:
        summary = json.loads((run_dir / "summary.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        say(f"{run_dir.name} has no summary yet (its harness did not finish). Attach to the game "
            "and stop it again (`--attach`, then `--stop --resign`), or record it by hand: "
            f"tools/people_elo.py record {run_dir} --winner civvis --victory resignation")
        return 2
    if not summary.get("person"):
        say(f"{run_dir.name} was played under no --name, so there is no people's record to put the "
            "resignation on")
        return 0
    say(f"no harness is running; putting {run_dir.name} on the people's record as "
        + ("its ending says" if summary.get("result") else "given up (CIVVIS's)"))
    record_person_game(run_dir, say, resigned=not summary.get("result"))
    return 0


def stop_command(args: argparse.Namespace) -> int:
    current = read_current()
    if not current or not harness_alive(current.get("pid", -1)):
        if args.resign:
            return resign_unattended(current)
        print("no hotseat harness is running")
        if args.quit_game and env.game_pids():
            print("Civ VI is still up; it was not started by a running harness, so it is "
                  "left alone (quit it from its menu)")
        return 0
    run_dir = Path(current["run_dir"])
    if args.quit_game and current.get("manual_lobby"):
        print(f"{SCREEN_FREE_FLAG}: Civ VI and the game are yours, so they stay up "
              "(quit Civ VI from its own menu)")
    try:
        offset = (run_dir / "hotseat.log").stat().st_size
    except OSError:
        offset = 0
    (run_dir / "stop.json").write_text(json.dumps({"quit_game": bool(args.quit_game),
                                                   "resign": bool(args.resign),
                                                   "utc": utc()}))
    # Windows has no SIGTERM to send (`os.kill` is TerminateProcess, which
    # skips the teardown below); the harness polls for stop.json instead.
    if os.name != "nt":
        os.kill(int(current["pid"]), signal.SIGTERM)
    if args.quit_game:
        print("stopping, and closing Civ VI from its menu (a minute or two)", flush=True)
    deadline = time.monotonic() + stop_wait(bool(args.quit_game))
    while time.monotonic() < deadline and harness_alive(current["pid"]):
        time.sleep(1.0)
    for line in stopped_news(run_dir / "hotseat.log", offset):
        print(line)
    if harness_alive(current["pid"]):
        print(f"the harness (pid {current['pid']}) is still shutting down; its log: "
              f"{run_dir / 'hotseat.log'}")
        return 1
    print(f"stopped hotseat {current.get('tag')}; log: {run_dir / 'hotseat.log'}")
    return 0


def describe_table(run_dir: Path) -> str | None:
    """The game a run plays, in one line, from its own record."""
    settings = run_settings(run_dir)
    if not settings:
        return None
    seats = jack_seats(int(settings.get("players") or 0), settings.get("civvis_seats"))
    agents = settings.get("agents") or []
    table = ", ".join(f"seat {seat} {agents[i] if i < len(agents) else STRATEGY}"
                      for i, seat in enumerate(seats))
    who = ("nobody's seat (" + ("a test stub" if settings.get("auto_human") else "CIVVIS on every seat") + ")"
           if settings.get("civvis_seats") or settings.get("auto_human")
           else f"{settings.get('name') or 'you'} (seat 0, {settings.get('leader') or 'random leader'})")
    return (f"{settings.get('players')} players on {settings.get('map')} {settings.get('map_size')}: "
            f"{who}; {table}")


def last_lobby(run_dir: Path) -> dict | None:
    """A `--manual-lobby` run's game, as the HUD read it back (`hotseat_lobby`)."""
    found = None
    for event in Journal(run_dir / "events.jsonl").read():
        if event.get("kind") == "hotseat_lobby":
            found = event
    return found


def status_command() -> int:
    """What runs, said for a person: the record as JSON (scripts read it),
    then the game in a line, and what to do when the harness and Civ VI
    disagree. A `--manual-lobby` run says the game the person set up."""
    current = read_current()
    civ = bool(env.game_pids())
    if not current:
        # Only a running game makes an installed one worth naming.
        installed = installed_game_config() if civ else {}
        if installed.get("Hotseat") and installed.get("RunTag"):
            print(f"no harness is running, but Civ VI is up with the hotseat game "
                  f"{installed['RunTag']} installed: CIVVIS's seats are not being played. "
                  + ("Run the same `--manual-lobby` command again to pick it up."
                     if installed.get("RequireHarness") else
                     "`civ6-hotseat.sh --attach` picks it up."))
            return 0
        print("no hotseat game on record")
        return 0
    alive = harness_alive(current.get("pid", -1))
    print(json.dumps(dict(current, harness_running=alive, civ_running=civ), indent=2))
    run_dir = Path(current["run_dir"])
    if current.get("manual_lobby"):
        lobby = last_lobby(run_dir)
        if lobby is None:
            print("a --manual-lobby run: waiting for the Hot Seat game you set up to start")
        else:
            civvis, _ = lobby_seats(lobby)
            print("a --manual-lobby run: " + describe_lobby(lobby, civvis,
                                                           person=current.get("name"),
                                                           agents=current.get("agents")))
    else:
        table = describe_table(run_dir)
        if table:
            print(f"the game: {table}")
    if not alive and civ:
        print(f"the harness (pid {current.get('pid')}) is gone and Civ VI is up: CIVVIS's seats "
              "are not being played. `civ6-hotseat.sh --attach` picks the game up.")
    elif alive and not civ:
        print("Civ VI is not running; the harness ends by itself once it sees that")
    turns = run_dir / "turns.jsonl"
    if turns.is_file():
        for line in turns.read_text().splitlines()[-8:]:
            print(line)
    return 0


class Stop(Exception):
    pass


#: summary.json's `reason` for a table stopped because its game was not set up
#: as asked. Not a game: a runner replays it from a fresh start, never a save.
SETUP_FAULT_REASON = "setup fault"


# ------------------------------------------------------------ the manual lobby
#
# `--manual-lobby`: the person sets the Hot Seat game up and starts it; this
# installs the mod, reads the log and writes orders, and nothing else. It runs
# screen-free (`civ6_control/screen_free.py`), and every message is written for
# somebody who has never seen this project.

#: The seats a person-facing install can play: every Human row after the
#: first, up to five. All five get an agent context whatever the lobby holds
#: (a copy of the ~1 MB agent each), so the person never declares a player
#: count up front. A count would have to match the lobby, and a different count
#: means a different file list, which Civ VI notices only on its next launch
#: (`Mods.sqlite` is an mtime index). A context whose seat is not a Human row
#: never acts: the hotseat local player is always a Human row, and each context
#: acts only on its own seat (`CivvisSeat.gate`).
MANUAL_SEATS = [1, 2, 3, 4, 5]
#: The harness rewrites its beat this often; the mod treats it as live while it
#: is at most HARNESS_FRESH_S old (`CivvisHarness.live`, SQLite's clock).
HARNESS_BEAT_S = 2.0
HARNESS_FRESH_S = 20
#: The mod's freshness read, as its three scripts send it (`CivvisHarness.read`;
#: `tools/test_civ6_hotseat_manual_lobby.py` holds the Lua to this text).
HARNESS_AGE_SQL = ("SELECT CAST(strftime('%s','now') AS INTEGER) - beat AS age"
                   " FROM civvis.harness WHERE run = '{run}'")
#: A CIVVIS turn this quiet gets a note in the terminal, at most this often.
MANUAL_STALL_NOTE_S = 300.0
#: The game loaded CIVVIS but no lobby record came this long after.
LOBBY_LATE_S = 90.0

START_CIV = "Start Civilization VI."
RESTART_CIV = ("Civ VI is already running, and it looks for mods only when it starts: "
               "quit it completely (Civ VI menu -> Quit) and start it again.")
MANUAL_STEPS = """\
CIVVIS is installed. Now set up the game in Civilization VI yourself:
  1. {start}
  2. Main menu -> Multiplayer -> Hot Seat.
  3. Choose the map, speed and anything else you like, then Confirm Settings.
  4. In the staging room the first row is you. Add a player for each seat
     CIVVIS should play and set it to Human (up to {most}); AI rows are
     Civ VI's own AI.
  5. Start Game.
CIVVIS plays its seats from the first turn. Keep this window open while you
play: Ctrl-C here (or `tools/civ6_hotseat.py --stop`) stops CIVVIS and leaves
Civ VI and your game alone.
"""


def manual_lobby_config(args: argparse.Namespace, run_dir: Path, orders_db: Path) -> dict:
    """The person-facing install: every seat in MANUAL_SEATS, gated on this
    harness (`RequireHarness`), and nothing that sets anything on their game.

    The controller settings are the hotseat table's own (`hotseat_config`);
    only what the lobby decides is taken out: no turn limit imposed in game,
    no Create Game defaults (the installer drops the FrontEnd block), no
    leader, no difficulty, no victories switched off.
    """
    table = argparse.Namespace(**vars(args))
    table.players = len(MANUAL_SEATS) + 1
    table.civvis_seats = None
    table.leader = None
    table.victories_off = []
    # Only to satisfy the controller's parser; `MaxTurns` is dropped below.
    table.max_turns = table.max_turns or default_max_turns(table.speed)
    config = hotseat_config(table, run_dir, orders_db)
    config.update({
        "RequireHarness": True,
        "HarnessRun": args.tag,
        "HarnessFreshSeconds": HARNESS_FRESH_S,
        "MaxTurns": None, "Difficulty": None, "PlayerDifficulty": None, "GameSpeed": None,
        "MapScript": None, "MapSize": None, "Leader": None, "HumanPlayers": None,
        "VictoriesOff": [],
    })
    return config


class HarnessBeat:
    """This harness's pulse in the orders database.

    The person-facing mod plays and writes only while this row is fresh
    (`CivvisHarness`). Stopping deletes it and a crash lets it go stale, so
    either way the game is the person's again within seconds.
    """

    SCHEMA = ("CREATE TABLE IF NOT EXISTS harness "
              "(run TEXT PRIMARY KEY, beat INTEGER NOT NULL, pid INTEGER)")

    def __init__(self, path: Path, run: str, every_s: float = HARNESS_BEAT_S,
                 clock=time.monotonic):
        self.path, self.run, self.every_s, self.clock = Path(path), run, every_s, clock
        self.last: float | None = None

    def beat(self) -> bool:
        now = self.clock()
        if self.last is not None and now - self.last < self.every_s:
            return True
        self.last = now
        try:
            with closing(sqlite3.connect(str(self.path), timeout=2.0)) as db, db:
                db.execute(self.SCHEMA)
                db.execute("INSERT OR REPLACE INTO harness (run, beat, pid) VALUES (?, ?, ?)",
                           (self.run, int(time.time()), os.getpid()))
            return True
        except sqlite3.Error:
            return False

    def age(self) -> int | None:
        """The beat's age in seconds, read exactly as the mod reads it."""
        try:
            with closing(sqlite3.connect(":memory:")) as db:
                db.execute("ATTACH DATABASE ? AS civvis", (str(self.path),))
                rows = db.execute(HARNESS_AGE_SQL.format(
                    run=self.run.replace("'", "").replace("\\", ""))).fetchall()
        except sqlite3.Error:
            return None
        return int(rows[0][0]) if rows and rows[0][0] is not None else None

    def clear(self) -> None:
        try:
            with closing(sqlite3.connect(str(self.path), timeout=2.0)) as db, db:
                db.execute(self.SCHEMA)
                db.execute("DELETE FROM harness WHERE run = ?", (self.run,))
        except sqlite3.Error:
            pass


class ModdingLog:
    """What Civ VI itself says about each game it sets up (`Logs/Modding.log`).

    Independent of the mod, so it can say what the mod cannot: that a game
    started without CIVVIS in it, or as something other than Hot Seat. A game
    is a block opened by `Stage: Configure Content`, carrying `Game Mode:`
    (`HOTSEAT`, `Single-Player`), `Map Script:`, `Human Players:` and `AI
    Players:`, then one `Applying Component - <id> (<action>)` line per mod
    action applied (`CivvisControlAgentUI` is ours), and closed by the next
    `Stage:` line (`Initializing a new game`).
    """

    LINE = re.compile(r"^\[[\d.]+\]\s?(.*)$")
    FIELDS = (("Game Mode:", "mode", str), ("Map Script:", "map", str),
              ("Human Players:", "humans", int), ("AI Players:", "ais", int))
    OURS = "Applying Component - CivvisControlAgentUI"

    def __init__(self, path: Path | None = None, clock=time.monotonic, settle_s: float = 30.0):
        self.path = Path(path) if path else env.logs_dir() / "Modding.log"
        self.clock, self.settle_s = clock, settle_s
        self.offset, self.inode = self._stat()
        self.partial = ""
        self.block: dict | None = None

    def _stat(self) -> tuple[int, int | None]:
        try:
            stat = self.path.stat()
        except OSError:
            return 0, None
        return stat.st_size, stat.st_ino

    def poll(self) -> list[dict]:
        """The games set up since the last poll, each once."""
        size, inode = self._stat()
        if inode != self.inode or size < self.offset:
            # Civ VI starts a fresh log each launch.
            self.offset, self.inode, self.partial = 0, inode, ""
        chunk = ""
        if size > self.offset:
            try:
                with self.path.open("r", errors="replace") as handle:
                    handle.seek(self.offset)
                    chunk = handle.read()
                    self.offset = handle.tell()
            except OSError:
                chunk = ""
        lines = (self.partial + chunk).split("\n")
        self.partial = lines.pop()
        done = []
        for raw in lines:
            match = self.LINE.match(raw.strip())
            text = (match.group(1) if match else raw).strip()
            if text == "Stage: Configure Content":
                if self.block is not None:
                    done.append(self.block)
                self.block = {"mode": None, "map": None, "humans": None, "ais": None,
                              "civvis": False, "stage": None, "at": self.clock()}
            elif self.block is None:
                continue
            elif text.startswith("Stage:"):
                self.block["stage"] = text[len("Stage:"):].strip()
                done.append(self.block)
                self.block = None
            elif text.startswith(self.OURS):
                self.block["civvis"] = True
            else:
                for prefix, key, cast in self.FIELDS:
                    if text.startswith(prefix) and self.block[key] is None:
                        try:
                            self.block[key] = cast(text[len(prefix):].strip())
                        except ValueError:
                            pass
        if self.block is not None and self.clock() - self.block["at"] > self.settle_s:
            done.append(self.block)
            self.block = None
        return done


def modding_verdict(block: dict) -> str | None:
    """What a person needs to hear about a game Civ VI set up, if anything."""
    mode = str(block.get("mode") or "")
    if mode and mode.upper() != "HOTSEAT":
        return (f"Civ VI started a {mode} game; CIVVIS plays only Hot Seat games "
                "(main menu -> Multiplayer -> Hot Seat). This window keeps waiting.")
    if mode and not block.get("civvis"):
        return ("⚠ Civ VI started this Hot Seat game without the CIVVIS mod, because it was "
                "already running when the mod was installed: quit Civ VI completely, start it "
                "again, and set the game up again. This window keeps waiting.")
    return None


def _rows(seats: list[int]) -> str:
    """Seats as the staging room numbers them: row N is seat N-1."""
    named = [f"row {seat + 1}" for seat in seats]
    return named[0] if len(named) == 1 else ", ".join(named[:-1]) + " and " + named[-1]


def lobby_seats(lobby: dict, installed: list[int] = MANUAL_SEATS) -> tuple[list[int], list[str]]:
    """The CIVVIS seats in the game the person set up, and what is wrong with it.

    Every Human row after the first is a CIVVIS seat; AI rows are Civ VI's own.
    Each problem is one sentence saying what to do about it.
    """
    players = [p for p in lobby.get("players") or [] if isinstance(p, dict)]
    humans = sorted(int(p["id"]) for p in players
                    if p.get("slot") == "human" and isinstance(p.get("id"), (int, float)))
    problems = []
    if HUMAN_SEAT not in humans:
        problems.append("The first row is not Human, but it is yours: quit to the main menu, "
                        "set it to Human, and start the game again.")
    civvis = [seat for seat in humans if seat in installed]
    beyond = [seat for seat in humans if seat != HUMAN_SEAT and seat not in installed]
    if beyond:
        one = len(beyond) == 1
        problems.append(
            f"{_rows(beyond)} {'is' if one else 'are'} Human, but CIVVIS plays at most "
            f"{len(installed)} seats (rows 2-{max(installed) + 1}), so {'its' if one else 'their'} "
            f"turns will wait for a person: set {'it' if one else 'them'} to AI and start "
            "the game again.")
    if not civvis:
        problems.append("Only your row is Human, so CIVVIS has no seat to play: quit to the "
                        "main menu, set the Hot Seat game up again with at least one more "
                        "player set to Human, and start it. This window keeps waiting.")
    return civvis, problems


def describe_lobby(lobby: dict, civvis: list[int], stub: bool = False,
                   person: str | None = None, agents: dict | None = None) -> str:
    """The game as Civ VI reports it, in one line: the person by `--name`
    and each CIVVIS seat by its agent where the run names them."""
    agents = {int(seat): agent for seat, agent in (agents or {}).items()}
    players = sorted((p for p in lobby.get("players") or [] if isinstance(p, dict)),
                     key=lambda p: p.get("id", 99))
    seats = []
    for player in players:
        seat = int(player.get("id", -1))
        if seat == HUMAN_SEAT:
            who = "test stub" if stub else f"{person} (you)" if person else "you"
        elif seat in civvis:
            who = seat_name(seat, agents.get(seat))
        elif player.get("slot") == "human":
            who = "a person"
        else:
            who = "Civ VI AI"
        detail = ", ".join(str(v) for v in (player.get("slot"), player.get("difficulty"),
                                             player.get("leader")) if v)
        seats.append(f"row {seat + 1} {who} ({detail})")
    limit = lobby.get("max_turns")
    return (f"the game: {'Hot Seat' if lobby.get('hotseat') else 'NOT Hot Seat'}, "
            f"{len(players)} players on {lobby.get('map')} {lobby.get('size')} at "
            f"{lobby.get('speed')}, turn limit {limit if limit not in (None, -1) else '?'}; "
            + "; ".join(seats))


def manual_stall_note(monitor: Monitor, log: Log, now: float) -> bool:
    """A CIVVIS turn gone quiet, said in the terminal; nothing is clicked."""
    # Never noted is not "noted at 0": `time.monotonic()` counts from boot, so
    # a machine up less than MANUAL_STALL_NOTE_S would never say anything.
    if monitor.stall_noted_at is not None and now - monitor.stall_noted_at < MANUAL_STALL_NOTE_S:
        return False
    monitor.stall_noted_at = now
    log(f"⚠ {monitor.name(monitor.active)} has made no move for "
        f"{int(now - monitor.last_event)} s on its turn. Nothing here clicks in --manual-lobby "
        "mode: if Civ VI shows a screen waiting for a click, close it (its own button, or Esc); "
        "if the map shows NEXT TURN, press it. CIVVIS carries on once the turn moves.")
    return True


def manual_seat_agents(args: argparse.Namespace, civvis: list[int]) -> dict[int, str]:
    """Each CIVVIS seat's roster agent at a `--manual-lobby` table, as its install
    names it: `--agents` in the order of every seat the install can play
    (`MANUAL_SEATS`, `manual_lobby_config`), so row 3 is the third agent whether
    or not row 2 is seated. Empty without `--agents` (jack on every seat)."""
    if not getattr(args, "agents", None):
        return {}
    named = seat_agents(args, MANUAL_SEATS)
    return {seat: named[seat] for seat in civvis if seat in named}


def manual_table_record(monitor: Monitor, args: argparse.Namespace, civvis: list[int],
                        lobby: dict, began: dict) -> dict:
    """A `--manual-lobby` game's ending and setup, as `table_record` says a
    launched one's, from the game the person set up (`hotseat_lobby`, the HUD's
    read-back) and not the command line, whose game settings are not used:
    the players in the game, its map, size, speed and turn limit, and the
    person's difficulty and leader (row 1, seat 0). Before 2026-11-28 this run
    wrote no ending at all, so a named person's game never reached the
    people's record."""
    players = sorted((p for p in lobby.get("players") or [] if isinstance(p, dict)),
                     key=lambda p: p.get("id", 99))
    person = next((p for p in players if p.get("id") == HUMAN_SEAT), {})
    limit = lobby.get("max_turns")
    table = argparse.Namespace(**vars(args))
    table.players = len(players) or None
    table.map, table.map_size, table.speed = lobby.get("map"), lobby.get("size"), lobby.get("speed")
    table.max_turns = limit if isinstance(limit, int) and limit > 0 else None
    table.difficulty, table.leader = person.get("difficulty"), person.get("leader")
    agents = manual_seat_agents(args, civvis)
    table.agents = [agents[seat] for seat in civvis] if agents else None
    return table_record(monitor, table, civvis, began)


def run_manual(args: argparse.Namespace) -> int:
    """`--manual-lobby`: install, say what to do, read the person's game back,
    and play its CIVVIS seats until Civ VI closes or the harness is stopped.

    A named person's game (`--name`) ends as a launched one does: the ending
    and the table are in `summary.json` (`manual_table_record`), it goes on the
    people's record the moment the game is decided, and `--stop --resign`
    gives it to CIVVIS."""
    if not screen_free.engaged() or screen_free.loaded_for_real():
        print(f"{SCREEN_FREE_FLAG} runs screen-free from its first import: spell the flag out "
              f"in full ({SCREEN_FREE_FLAG})", file=sys.stderr)
        return 2
    args.tag = args.tag or "hotseat-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_dir = RUN_ROOT / args.tag
    orders_db = run_dir / "orders.sqlite"
    # The path as the mod is handed it (`mod_path`): forward slashes on Windows.
    if "'" in str(orders_db) or "\\" in mod_path(orders_db):
        print(f"{run_dir} has a quote or a backslash in it, which the game cannot open its "
              "orders database through: set CIVVIS_HOTSEAT_RUNS to a plain path", file=sys.stderr)
        return 2
    run_dir.mkdir(parents=True, exist_ok=True)
    (run_dir / "stop.json").unlink(missing_ok=True)
    log = Log(run_dir / "hotseat.log")
    if not gamelock.acquire(args.tag, require_verification_intent=False,
                            honour_operator_halt=False):
        log(f"another run holds Civ VI: {gamelock.foreign_run(args.tag) or gamelock.describe()}")
        return 6
    stub = bool(args.auto_human)
    agents = MANUAL_SEATS + ([HUMAN_SEAT] if stub else [])
    beat = HarnessBeat(orders_db, args.tag)
    brains: Brains | None = None
    monitor: Monitor | None = None
    bridges: list[watch.EventLogBridge] = []
    stop_reason = "finished"
    civvis: list[int] = []
    lobby: dict | None = None
    installed = False
    # When the game began (the HUD's clock as it read the game back) and the
    # revision that played it, for the people's record.
    began: dict = {"started": None, "revision": checkout_revision()}
    posted = False

    def on_term(signum, _frame):  # noqa: ANN001
        raise Stop(f"signal {signum}")

    signal.signal(signal.SIGTERM, on_term)
    signal.signal(signal.SIGINT, on_term)
    write_current({"pid": os.getpid(), "tag": args.tag, "run_dir": str(run_dir),
                   "started": utc(), "manual_lobby": True, "name": person_name(args),
                   "agents": {str(seat): agent
                              for seat, agent in manual_seat_agents(args, MANUAL_SEATS).items()}})
    try:
        binary = resolve_orders_bin(args.orders_bin)
        if binary is None:
            log("no civvis_orders binary found; pass --orders-bin or build one "
                "(cargo build --release --locked --bin civvis_orders)")
            return 4
        binary = freeze_binary(binary, run_dir)
        live_genome(args, binary, run_dir, log)
        civ_at_install = set(env.game_pids())
        config = manual_lobby_config(args, run_dir, orders_db)
        note = fast_animations_note(args, config)
        if note:
            log(note)
        reset_orders_db(orders_db)
        import civ6_brain  # noqa: PLC0415 - screen-free here: the guard is engaged
        civ6_brain.connect(orders_db).close()
        beat.beat()
        age = beat.age()
        if age is None or not -5 <= age <= HARNESS_FRESH_S:
            log(f"the orders database at {orders_db} does not read back this harness's beat "
                f"(age {age}); the game could not tell that CIVVIS is running")
            stop_reason = "beat unreadable"
            return 4
        target = modinstall.install(config)
        installed = True
        (run_dir / "config.json").write_text(json.dumps(config, indent=2, sort_keys=True))
        log(f"installed the CIVVIS mod for a Hot Seat game you set up, at {target}; "
            f"CIVVIS seats play with {binary}")
        log(f"screen-free: {len(screen_free.SCREEN_MODULES)} screen modules are refusing "
            f"stand-ins, {len(screen_free.loaded_for_real())} loaded for real, Pillow absent")
        restart_wanted = bool(civ_at_install)
        print("\n" + MANUAL_STEPS.format(start=RESTART_CIV if restart_wanted else START_CIV,
                                         most=len(MANUAL_SEATS)), flush=True)
        bridges = [watch.EventLogBridge(run_dir, tag=args.tag)]
        for seat in agents:
            bridges.append(watch.EventLogBridge(
                run_dir / modinstall.hotseat_seat_tag(args.tag, seat),
                tag=modinstall.hotseat_seat_tag(args.tag, seat)))
        monitor = Monitor(run_dir, args.tag, MANUAL_SEATS, args, log)
        modding = ModdingLog()
        civ_up = bool(civ_at_install)
        # Whether Civ VI runs, without a `ps` every pass, as `run` asks it
        # (#6005): the relay reads nothing while one runs. A new or a closed
        # game is still confirmed by a real listing (`watch.GameLiveness`).
        liveness = watch.GameLiveness(clock=time.monotonic)
        loaded_at: float | None = None
        late_said = False
        while True:
            beat.beat()
            if os.name == "nt" and (run_dir / "stop.json").is_file():
                raise Stop("stop requested")  # `--stop` sends no SIGTERM on Windows
            for bridge in bridges:
                bridge.pump()
            monitor.poll()
            now = time.monotonic()
            for block in modding.poll():
                log(f"Civ VI set up a game: {block.get('mode')}, {block.get('humans')} human and "
                    f"{block.get('ais')} AI players on {block.get('map')}, CIVVIS mod "
                    f"{'loaded' if block.get('civvis') else 'NOT loaded'}")
                verdict = modding_verdict(block)
                if verdict:
                    log(verdict)
                elif block.get("civvis") and lobby is None:
                    loaded_at, late_said = now, False
            while monitor.lobbies:
                event = monitor.lobbies.pop(0)
                if brains is not None:
                    stop_reason = "another game"
                    log("Civ VI started another game while CIVVIS was playing this one, so CIVVIS "
                        "stops here (its orders belong to the first game). To play the new one "
                        f"with CIVVIS, run `tools/civ6_hotseat.py {SCREEN_FREE_FLAG}` again.")
                    raise Stop("another game")
                lobby, loaded_at = event, None
                began["started"] = ended_at(event).get("at") or utc()
                if isinstance(event.get("max_turns"), int) and event["max_turns"] > 0:
                    # The game's own limit, for a turn-limit ending (`time_defeat`).
                    args.max_turns = event["max_turns"]
                civvis, problems = lobby_seats(event)
                log(describe_lobby(event, civvis, stub, person_name(args),
                                   manual_seat_agents(args, civvis)))
                for problem in problems:
                    log(("⚠ " if civvis else "") + problem)
                if civvis:
                    monitor.seats = civvis
                    # Each seat plays its own agent's lane, as its install names it.
                    agents = manual_seat_agents(args, civvis)
                    lanes = ({seat: AGENT_LANES[agent] for seat, agent in agents.items()}
                             if agents else args.victory)
                    brains = Brains(run_dir, args.tag, civvis, orders_db, binary, lanes, log,
                                    treatments=args.treatments)
                    brains.poll_s = poll_seconds(args)
                    brains.start_all()
                    log(f"CIVVIS plays {_rows(civvis)}"
                        + (" and the test stub row 1" if stub else "")
                        + ". Ctrl-C here, or `tools/civ6_hotseat.py --stop`, stops CIVVIS and "
                          "leaves the game as it is.")
            if (loaded_at is not None and not late_said and lobby is None
                    and now - loaded_at > LOBBY_LATE_S):
                late_said = True
                log(f"⚠ Civ VI loaded the CIVVIS mod {int(now - loaded_at)} s ago and it has not "
                    f"answered. It reads this harness through {orders_db}; if that file is gone "
                    "or unreadable, CIVVIS cannot see that it should play. Quit to the main menu, "
                    f"restart `tools/civ6_hotseat.py {SCREEN_FREE_FLAG}`, and start the game again.")
            if brains is not None:
                for seat in monitor.take_unseated():
                    brains.retire(seat)
                brains.keep_alive()
            if (monitor.result is not None and not posted and lobby is not None and civvis
                    and person_name(args)):
                # The person's game is decided: on the record now, as `run`
                # posts it, not when they get round to closing Civ VI (the
                # Victory screen offers "one more turn").
                posted = True
                (run_dir / "summary.json").write_text(json.dumps(
                    {"tag": args.tag, "utc": utc(), "reason": "game over", "manual_lobby": True,
                     "lobby": lobby, **manual_table_record(monitor, args, civvis, lobby, began)},
                    indent=2))
                record_person_game(run_dir, log)
            pids = set(liveness.known) if liveness.alive() else set()
            if pids and not civ_up:
                civ_up = True
                if restart_wanted and pids.isdisjoint(civ_at_install):
                    restart_wanted = False
                    log("Civ VI started again with the CIVVIS mod: set up the Hot Seat game now.")
            elif not pids and civ_up:
                civ_up = False
                if brains is not None:
                    stop_reason = "Civ VI closed"
                    log("Civ VI has closed")
                    break
                log("Civ VI has closed; start it again when you are ready. This window keeps "
                    "waiting.")
            if (args.turns is not None and monitor.turn > args.turns
                    and monitor.active is not None):
                stop_reason = f"reached turn {args.turns}"
                log(f"turn {args.turns} is complete")
                break
            if brains is not None and monitor.stalled(args.stall_seconds):
                manual_stall_note(monitor, log, now)
            relay_for(bridges, read_s=relay_seconds(args))
    except Stop as stop:
        if stop_reason == "finished":
            stop_reason = f"stopped ({stop})"
        log(f"stopping: {stop}")
    finally:
        signal.signal(signal.SIGINT, signal.SIG_IGN)
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        if brains is not None:
            brains.stop()
        # The game is the person's again from here: the mod stands down within
        # seconds (`CivvisHarness.live`).
        beat.clear()
        for bridge in bridges:
            try:
                bridge.pump()
            except OSError:
                pass
        summary = {"tag": args.tag, "utc": utc(), "reason": stop_reason, "manual_lobby": True,
                   "civvis_seats": civvis, "lobby": lobby}
        if monitor is not None:
            monitor.poll()
            # The seats this game had, not every seat the install could play.
            monitor.stats = {seat: stats for seat, stats in monitor.stats.items()
                             if seat in civvis or (stub and seat == HUMAN_SEAT)}
            summary["report"] = smoke_report(monitor, log)
            if lobby is not None and civvis:
                summary.update(manual_table_record(monitor, args, civvis, lobby, began))
            monitor.close()
        request = {}
        try:
            request = json.loads((run_dir / "stop.json").read_text())
        except (OSError, ValueError):
            pass
        # The record first, as `run` writes it: people_elo reads the summary.
        (run_dir / "summary.json").write_text(json.dumps(summary, indent=2))
        if summary.get("person") and (summary.get("result") or request.get("resign")):
            record_person_game(run_dir, log, resigned=not summary.get("result"))
        elif request.get("resign"):
            log("the game was played under no --name, so the resignation is not put on the "
                "people's record" if lobby is not None and civvis else
                "no Hot Seat game with a CIVVIS seat had started, so there is nothing to resign")
        if installed and env.game_pids():
            log("Civ VI is still running, so the CIVVIS mod stays installed; it does nothing "
                "without this harness. To remove it after you quit Civ VI: "
                "python3 tools/civ6_control/install.py --uninstall")
        elif installed:
            try:
                if modinstall.uninstall():
                    log("removed the CIVVIS mod")
            except Exception as error:  # noqa: BLE001 - cleanup must finish
                log(f"could not remove the CIVVIS mod: {error}")
        gamelock.release()
        current = read_current()
        if current and current.get("pid") == os.getpid():
            CURRENT.unlink(missing_ok=True)
        log(f"done ({stop_reason}); record in {run_dir}")
    return 0


#: An option's value while `typed_options` reads which ones the command gave.
_UNTYPED = object()


def typed_options(parser: argparse.ArgumentParser, argv: list[str] | None) -> set[str]:
    """The options the command line gave, by destination. argparse fills a
    default in only where the namespace has no value yet, so a namespace
    holding a marker for every option keeps the marker wherever none was given."""
    blank = argparse.Namespace(**{action.dest: _UNTYPED for action in parser._actions
                                  if action.dest != argparse.SUPPRESS})
    parsed = parser.parse_args(argv, namespace=blank)
    return {dest for dest, value in vars(parsed).items() if value is not _UNTYPED}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0],
                                 formatter_class=argparse.RawDescriptionHelpFormatter,
                                 epilog="\n".join(__doc__.splitlines()[1:]))
    ap.add_argument("--leader", default=play.ROMAN_LEADER,
                    help="YOUR leader (seat 0), e.g. LEADER_SALADIN; 'random' for any "
                         "(default %(default)s). The jack seats get random leaders.")
    ap.add_argument("--difficulty", default="DIFFICULTY_PRINCE", choices=play.LADDER,
                    help="every seat's difficulty (default %(default)s)")
    ap.add_argument("--speed", default="GAMESPEED_ONLINE",
                    choices=sorted(SPEED_TURNS), help="default %(default)s")
    ap.add_argument("--map", default="Pangaea.lua", help="default %(default)s")
    ap.add_argument("--map-size", default="MAPSIZE_TINY",
                    choices=["MAPSIZE_DUEL", "MAPSIZE_TINY", "MAPSIZE_SMALL"],
                    help="default %(default)s")
    ap.add_argument("--players", type=int, default=4, choices=range(2, 7),
                    metavar="2-6", help="seats including yours (default %(default)s)")
    ap.add_argument("--max-turns", type=int, default=None,
                    help="turn limit (default: the speed's own, 250 at Online)")
    ap.add_argument("--victory", default="civvis", choices=play.VICTORY_LANES,
                    help="what jack plays for; `civvis` lets it take whichever victory "
                         "is open (default)")
    ap.add_argument("--save-every", type=int, default=10,
                    help="save the game every N turns (0: Civ VI's autosaves only)")
    ap.add_argument("--orders-bin", default=None,
                    help="civvis_orders to play with (default: the verification lane's "
                         "current build of main)")
    ap.add_argument("--tag", default=None, help="run name (default hotseat-<utc>)")
    ap.add_argument("--stall-seconds", type=float, default=180.0,
                    help="a jack seat silent this long gets the popup rescue")
    ap.add_argument("--no-notify", action="store_true", help="no macOS notifications")
    ap.add_argument("--no-cover", action="store_true",
                    help="show the jack seats' turns as they play (default: a cover hides "
                         "the screen from the end of your turn to the start of your next)")
    ap.add_argument("--keep-handover", action="store_true",
                    help="leave Civ VI's hand-over screen for you to press Start Turn on "
                         "(default: it is pressed for you and your turn simply begins)")
    ap.add_argument("--cover-sound", choices=COVER_SOUNDS, default=COVER_SOUND,
                    help="what you hear while the cover is up: `music` (default) only the "
                         "soundtrack, the jack seats' units, fights, chimes and voices "
                         "silenced; `silent` nothing at all; `all` everything, as before. "
                         "Your own volumes are back the moment your turn begins")
    ap.add_argument("--no-cover-timer", action="store_true",
                    help="no wait bar on the cover (default: a bar and about how long is "
                         "left, from how long your recent waits took)")
    ap.add_argument("--fast-turns", action="store_true",
                    help="off by default: under the cover every hand-over (the CIVVIS seats' "
                         "and yours) is pressed on its first frame instead of 0.3 s in. It "
                         "also has the CIVVIS seats look for their orders every 20 ms, which "
                         "the default `--genome live` already does (its mod arms' poll-20ms), "
                         "so that part adds something only with `--genome stock`. And each "
                         "board reaches its seat's worker sooner: the game's log is relayed, "
                         "and read by every worker, every 10 ms instead of 20 and 25, and "
                         "the HUD offers the orders peek every 20 ms on average rather than "
                         "every other frame. Played in live games since 2026-11-28")
    ap.add_argument("--fast-animations", nargs="?", type=float, const=FAST_ANIMATIONS,
                    default=None, metavar="N",
                    help="off by default: while the cover is up over the CIVVIS seats' turns "
                         f"the game runs N times faster (default {FAST_ANIMATIONS:g}, the "
                         "ladder's own debug timescale; above 1, at most "
                         f"{FAST_ANIMATIONS_MAX:g}): their fights, walks and the waits on "
                         "them. Your own turn always runs at normal speed: the scale is put "
                         "back to 1 before it is shown, and if that ever fails the feature "
                         "switches itself off for the rest of the game. Needs the cover; "
                         "give it again with --resume. Tests only so far")
    ap.add_argument("--fast-animations-rollover", action="store_true",
                    help="with --fast-animations: also through the game's own turn "
                         "processing after the last CIVVIS seat (barbarians, city-states, "
                         "the autosave), until your turn begins")
    ap.add_argument("--no-cover-queue", action="store_true",
                    help="DIAGNOSTIC: under the cover, the jack seats' announcements keep "
                         "their shipped queueing (they wait until the cover stands aside)")
    ap.add_argument("--resume", nargs="?", const="latest", default=None, metavar="SAVE",
                    help="start Civ VI and load a hotseat save instead of a new game: "
                         "a .Civ6Save path, or the newest hotseat save/autosave/quicksave; "
                         "the game is played as its first run set it up (name, agents, "
                         "cover...), any of those given again wins")
    ap.add_argument("--attach", action="store_true",
                    help="drive a hotseat game that is already running (a resumed save, "
                         "or after --stop or a closed terminal)")
    ap.add_argument("--stop", action="store_true", help="stop the running harness")
    ap.add_argument("--quit-game", action="store_true",
                    help="with --stop: also close Civ VI if the harness started it")
    ap.add_argument("--resign", action="store_true",
                    help="with --stop: you give the game up. A game under a --name that the "
                         "game itself has not ended goes on the people's record as CIVVIS's")
    ap.add_argument("--status", action="store_true")
    ap.add_argument("--civvis-seats", type=int, default=None, metavar="N",
                    help="unattended: CIVVIS plays seats 0..N-1 and the other seats up to "
                         "--players are Firaxis AI (no person at the Mac); the run stops at "
                         "the game's ending with the result in summary.json and closes Civ VI")
    ap.add_argument("--self-play", action="store_true",
                    help="unattended, CIVVIS on every seat (`--civvis-seats` = --players): "
                         "`--duel --self-play --agents jack,holy` is the duel with nobody "
                         "at the Mac, seat 0 first in --agents")
    ap.add_argument("--victories-off", default="", metavar="VICTORY_TYPE[,...]",
                    help="victory types switched off at setup, e.g. VICTORY_DIPLOMATIC "
                         "(the host policy's CIVVIS_VICTORIES_OFF is not read here); each "
                         "is read back from the started game, and an unattended table "
                         "whose game still has one on stops without a result")
    ap.add_argument("--play-flags", default="", metavar="FLAGS",
                    help="civ6_play switches every jack seat plays with, as one string "
                         "(--play-flags='--dv-join-winner --luxury-quote-proposals')")
    ap.add_argument("--genome", choices=GENOMES, default="live",
                    help="what the CIVVIS seats play: `live` (default) the live ladder's "
                         "genes (deploy/live-force-on.txt, or this host's override), the "
                         "hotseat-only genes on top (deploy/hotseat-force-on.txt) and the "
                         "ladder's mod arms (deploy/live-mod-arms.txt), as far as the decider "
                         "takes them; `stock` the bare deployment genome, for comparison")
    ap.add_argument("--agents", default=None, metavar="AGENT[,...]",
                    help="roster agents for the CIVVIS seats in seat order (seat 1 first), "
                         f"from {', '.join(AGENT_LANES)}, or `{STRONGEST}` for the one the "
                         "ELO Rankings rate highest; default jack on every seat")
    ap.add_argument("--name", default=None, metavar="NAME",
                    help="YOUR name: seat 0 carries it in the game, and a finished game "
                         "is recorded under it for the people's Elo (tools/people_elo.py)")
    ap.add_argument("--duel", action="store_true",
                    help="one against one: 2 players on a Duel map against the strongest "
                         "agent (`--agents` names another)")
    ap.add_argument("--hold-start", action="store_true",
                    help="set up the staging room but leave START GAME to the person "
                         "(e.g. to pick Teams, which have no text to read)")
    ap.add_argument("--auto-human", nargs="?", const="stub", choices=["stub", "gated"],
                    default=None,
                    help="TEST: a stub plays seat 0 instead of you; `gated` keeps seat 0 "
                         "the operator's for the cover and the hand-over (covered on the jack "
                         "seats' turns, its Start Turn pressed only as yours would be), while "
                         "the stub's own announcements are closed and its quiet turns ended")
    ap.add_argument("--turns", type=int, default=None,
                    help="stop after this game turn (with --auto-human: then report)")
    ap.add_argument("--stand-in-deal", default="", metavar="'TURN:SEAT:VERB ...'",
                    help="TEST (with --auto-human): on its first turn from TURN the stand-in "
                         "puts this deal to seat SEAT (`*`: any CIVVIS seat it has met) through the "
                         "game's own deal screen calls, e.g. '8:3:PROPOSE;give=GOLD=40;get=' (the "
                         "`deal` order's verb); several deals separated by spaces, in one argument")
    ap.add_argument("--stand-in-deal-answer", choices=["ACCEPT", "REJECT"], default=None,
                    help="TEST (with --auto-human): a deal another seat puts to the stand-in is "
                         "answered so on its next turn after the one it was first seen on")
    ap.add_argument("--stand-in-deal-hold", type=float, default=None, metavar="SECONDS",
                    help="TEST (with --auto-human): while another seat's deal waits for the "
                         "stand-in, its end of turn is held this long (to photograph that turn)")
    ap.add_argument(SCREEN_FREE_FLAG, dest="manual_lobby", action="store_true",
                    help="you set up and start the Hot Seat game in Civ VI; this only installs "
                         "the mod, reads the log and writes orders: nothing is launched, "
                         "clicked, typed, captured or moved (no Screen Recording or "
                         "Accessibility permission, Pillow, swiftc or cliclick needed). The "
                         "game's settings are yours: --players/--map/--speed/--difficulty/"
                         "--leader are not used")
    args = ap.parse_args(argv)
    if (args.resign or args.quit_game) and not args.stop:
        # Alone they are no command at all, and the rest of `main` would start
        # a new game (the wrapper adds its duel defaults to anything else).
        given = " and ".join(flag for flag, on in (("--resign", args.resign),
                                                   ("--quit-game", args.quit_game)) if on)
        print(f"{given} go{'es' if ' and ' not in given else ''} with --stop: "
              f"`--stop {given}`", file=sys.stderr)
        return 2
    if args.fast_animations is not None and not 1 < args.fast_animations <= FAST_ANIMATIONS_MAX:
        print(f"--fast-animations takes a scale above 1 and at most {FAST_ANIMATIONS_MAX:g} "
              f"(default {FAST_ANIMATIONS:g})", file=sys.stderr)
        return 2
    if args.fast_animations_rollover and args.fast_animations is None:
        print("--fast-animations-rollover goes with --fast-animations", file=sys.stderr)
        return 2
    if (args.stand_in_deal or args.stand_in_deal_hold or args.stand_in_deal_answer) \
            and not args.auto_human:
        print("--stand-in-deal, --stand-in-deal-answer and --stand-in-deal-hold are for test "
              "tables: they go with --auto-human", file=sys.stderr)
        return 2
    try:
        stand_in_deals(args.stand_in_deal.split())
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    if args.stop:
        return stop_command(args)
    if args.status:
        return status_command()
    if args.manual_lobby:
        clash = [flag for flag, given in (("--attach", args.attach),
                                          ("--resume", args.resume is not None),
                                          ("--civvis-seats", args.civvis_seats is not None),
                                          ("--victories-off", bool(args.victories_off)))
                 if given]
        if clash:
            print(f"{SCREEN_FREE_FLAG} leaves the game to you, so {', '.join(clash)} has "
                  "nothing to do: load a save or switch victories off in Civ VI's own setup",
                  file=sys.stderr)
            return 2
        wrong = read_people_flags(args)
        if wrong:
            print(wrong, file=sys.stderr)
            return 2
        if argv is not None and not screen_free.engaged():
            print(f"{SCREEN_FREE_FLAG} must run screen-free from its first import "
                  "(CIVVIS_SCREEN_FREE=1 before civ6_control loads)", file=sys.stderr)
            return 2
        if not screen_free.engaged():
            # Abbreviated on the command line (`--manual`), so the module-level
            # switch missed it: start again screen-free rather than run with
            # the screen modules loaded (nothing of theirs has been called).
            # Frozen (`tools/package_hotseat.py`) the program is the harness:
            # it takes no script path.
            script = [] if FROZEN else [str(Path(__file__).resolve())]
            os.execve(sys.executable, [sys.executable, *script, *sys.argv[1:]],
                      dict(os.environ, CIVVIS_SCREEN_FREE="1"))
        current = read_current()
        if current and harness_alive(current.get("pid", -1)):
            print(f"a hotseat harness is already running (pid {current['pid']}, "
                  f"{current.get('tag')}); `--stop` it first", file=sys.stderr)
            return 6
        return run_manual(args)
    # `--resume`: the save's game, as the run that played it set it up (a
    # resumed run is a fresh harness: before 2026-11-28 a bare `--resume` of
    # the person's six-player game would have played it as a duel of jack
    # seats, under no name, recorded nowhere).
    args.resume_save, args.resumed, args.resume_notes = None, None, []
    if args.resume is not None:
        person = not (args.auto_human or args.self_play or args.civvis_seats)
        save, passed = resume_save(args.resume, person)
        if save is None or not save.is_file():
            print(f"no hotseat save to resume ({args.resume}); saves live in {HOTSEAT_SAVES}",
                  file=sys.stderr)
            return 2
        args.resume_notes += [f"passed over {newer.name}, a test's or an unattended table's "
                              f"save (`--resume {shlex.quote(str(newer))}` loads it)"
                              for newer in passed]
        notes, go = restore_resumed(args, typed_options(ap, argv), save)
        if not go:
            print("\n".join(notes), file=sys.stderr)
            return 2
        args.resume_notes += notes
        args.resume_save = save
    if args.leader and args.leader.lower() == play.RANDOM_LEADER:
        args.leader = None
    if args.max_turns is None:
        args.max_turns = default_max_turns(args.speed)
    named = {part.strip().upper() for part in args.victories_off.split(",") if part.strip()}
    unknown = sorted(named - set(play.VICTORY_TYPES))
    if unknown:
        print(f"--victories-off: {', '.join(unknown)} is not one of "
              f"{', '.join(play.VICTORY_TYPES)}", file=sys.stderr)
        return 2
    args.victories_off = [victory for victory in play.VICTORY_TYPES if victory in named]
    if args.duel:
        args.players, args.map_size = 2, "MAPSIZE_DUEL"
        # The strongest agent in the seat across the table: seat 1, with the
        # flagship in the person's seat when nobody is at the Mac.
        args.agents = args.agents or (f"{STRATEGY},{STRONGEST}" if args.self_play or args.civvis_seats
                                      else STRONGEST)
    wrong = read_people_flags(args)
    if wrong:
        print(wrong, file=sys.stderr)
        return 2
    if args.self_play:
        if args.civvis_seats not in (None, args.players):
            print(f"--self-play seats CIVVIS on every one of the {args.players} seats; "
                  f"--civvis-seats {args.civvis_seats} leaves some to Firaxis AI: ask for one",
                  file=sys.stderr)
            return 2
        args.civvis_seats = args.players
    if args.civvis_seats is not None and not 1 <= args.civvis_seats <= args.players:
        print(f"--civvis-seats is 1..{args.players} of --players {args.players} "
              f"(every seat: --self-play)", file=sys.stderr)
        return 2
    if args.civvis_seats and args.name:
        print("--name is a person's, and an unattended table has nobody at it: a game "
              "CIVVIS played must not stand on the people's record", file=sys.stderr)
        return 2
    current = read_current()
    if current and harness_alive(current.get("pid", -1)):
        print(f"a hotseat harness is already running (pid {current['pid']}, "
              f"{current.get('tag')}); `--stop` it first", file=sys.stderr)
        return 6
    return run(args)


def attached_table(config: dict) -> tuple[int, int | None]:
    """The table an installed hotseat game is, for `--attach`: its seats in
    all, and how many of them are CIVVIS's when nobody is at it (`None` for a
    person's table or a test stub's, which are their CIVVIS seats and seat 0).
    An unattended table has neither a person's seat nor a stub's, and its
    CIVVIS seats start at seat 0; `HotseatPlayers` says how many seats there
    are, Firaxis AI included (an install from before it was written is read
    as its CIVVIS seats alone)."""
    ours = len(config.get("HotseatSeats") or [])
    unattended = not (config.get("HotseatHumanSeats") or config.get("HotseatStubSeats"))
    players = int(config.get("HotseatPlayers") or (ours if unattended else ours + 1))
    return players, (ours if unattended else None)


def run(args: argparse.Namespace) -> int:
    attach_config = None
    if args.attach:
        attach_config = modinstall.installed_config()
        if not attach_config or not attach_config.get("Hotseat"):
            print("no hotseat install to attach to (start a new game without --attach)",
                  file=sys.stderr)
            return 2
        if not env.game_pids():
            print("Civ VI is not running: start it and load the hotseat save "
                  "(Multiplayer -> Hot Seat -> Load Game) first", file=sys.stderr)
            return 2
        args.tag = attach_config["RunTag"]
        args.players, args.civvis_seats = attached_table(attach_config)
        args.auto_human = (None if not attach_config.get("HotseatStubSeats")
                           else "gated" if attach_config.get("HotseatHumanSeats") else "stub")
        args.victory = attach_config.get("VictoryLane") or args.victory
        args.name = attach_config.get("HotseatPersonName") or args.name
        # A resumed game's install names no leader (the save has them all),
        # so the person's comes from the run's own record of its table.
        kept = run_settings(RUN_ROOT / args.tag) or {}
        args.leader = kept.get("leader") or attach_config.get("Leader") or None
        seat_agent_map = attach_config.get("HotseatSeatAgents") or {}
        args.agents = [seat_agent_map[k] for k in sorted(seat_agent_map, key=int)] or None
        args.victories_off = list(attach_config.get("VictoriesOff") or [])
        # The installed game's own: an install from before these keys had neither.
        args.no_cover = not attach_config.get("HotseatCover")
        args.keep_handover = not attach_config.get("HotseatAutoStart")
        args.cover_sound = attach_config.get("HotseatCoverSound") or "all"
        args.no_cover_timer = not attach_config.get("HotseatCoverTimer")
        args.fast_turns = attach_config.get("HotseatFastTurns") is True
        args.fast_animations = attach_config.get("HotseatFastAnimations") or None
        args.fast_animations_rollover = attach_config.get("HotseatFastRollover") is True
    elif env.game_pids():
        print("Civ VI is already running. Quit it first (this starts a fresh game), "
              "or pass --attach to drive a hotseat game you loaded.", file=sys.stderr)
        return 2
    args.tag = args.tag or "hotseat-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_dir = RUN_ROOT / args.tag
    run_dir.mkdir(parents=True, exist_ok=True)
    # A stop request is for the harness it was sent to, not a later --attach.
    (run_dir / "stop.json").unlink(missing_ok=True)
    log = Log(run_dir / "hotseat.log")
    orders_db = (Path(attach_config["OrdersDb"]) if attach_config
                 else run_dir / "orders.sqlite")
    if not gamelock.acquire(args.tag, require_verification_intent=False,
                            honour_operator_halt=False):
        log(f"another run holds Civ VI: {gamelock.foreign_run(args.tag) or gamelock.describe()}")
        return 6
    halt = gamelock.operator_halt_description()
    if halt:
        log(f"note: the unattended lanes stay halted ({halt}); this game is yours")
    launched = bool(args.attach) and launched_here(run_dir)
    # When the game began and at which revision: a re-attached harness keeps
    # the first one's answer (the installed config carries both).
    # A resumed game began when the run that started it did.
    resumed = getattr(args, "resumed", None) or {}
    began = {"started": ((attach_config or {}).get("HotseatStarted") or resumed.get("started")
                         or utc()),
             "revision": ((attach_config or {}).get("HotseatRevision") if args.attach
                          else checkout_revision())}
    seats = jack_seats(args.players, getattr(args, "civvis_seats", None))
    brains = None
    monitor = None
    bridges: list[watch.EventLogBridge] = []
    stop_reason = "finished"

    def on_term(signum, _frame):  # noqa: ANN001
        raise Stop(f"signal {signum}")

    signal.signal(signal.SIGTERM, on_term)
    signal.signal(signal.SIGINT, on_term)
    write_current({"pid": os.getpid(), "tag": args.tag, "run_dir": str(run_dir),
                   "started": utc(), "players": args.players, "attach": bool(args.attach)})
    try:
        binary = resolve_orders_bin(args.orders_bin)
        if binary is None:
            log("no civvis_orders binary found; pass --orders-bin or build one "
                "(heavy.sh 4 cargo build --release --locked --bin civvis_orders)")
            return 4
        if not args.attach:
            binary = freeze_binary(binary, run_dir)
        elif (run_dir / ORDERS_NAME).is_file():
            binary = run_dir / ORDERS_NAME
        log(f"jack seats {seats} play with {binary}")
        if args.attach:
            kept_genome(args, run_dir)
        else:
            live_genome(args, binary, run_dir, log)
        if not args.attach:
            for note in getattr(args, "resume_notes", None) or []:
                log(note)
            installing = args
            if args.resume_save is not None:
                # The save names every seat's leader; a pinned one is "Invalid
                # Option" in its staging room (measured: one row, NO HUMAN
                # PLAYERS). The person's stays on the record.
                installing = argparse.Namespace(**vars(args))
                installing.leader = None
            config = hotseat_config(installing, run_dir, orders_db)
            config.update({"HotseatStarted": began["started"],
                           "HotseatRevision": began["revision"] or ""})
            reset_orders_db(orders_db)
            target = modinstall.install(config)
            (run_dir / "config.json").write_text(json.dumps(config, indent=2, sort_keys=True))
            (run_dir / TABLE).write_text(json.dumps(table_settings(args, began, resumed or None),
                                                    indent=2, sort_keys=True))
            log(f"installed the control mod for hotseat at {target}")
            note = fast_animations_note(args, config)
            if note:
                log(note)
            if args.resume_save is not None:
                staged = stage_save(args.resume_save)
                log(f"resuming {args.resume_save} (staged as {staged.name})")
            log(f"game: {args.players} players on {args.map} {args.map_size}, "
                f"{args.speed}, {args.difficulty}, turn limit {args.max_turns}; "
                f"you: {args.leader or 'random leader'}; save every {args.save_every} turns")
            # What the Mac has to spare, said before the game is started on it.
            began["host"] = host_strain()
            log(strain_line(began["host"]))
            for warning in strain_warnings(began["host"]):
                log(f"⚠ {warning}")
            apply_person_options(log)
            launcher.clear_run_logs()
            process = launcher.launch(stdout=run_dir / "stdout.log")
            launched = True
            note_launched(run_dir, process.pid)
            log(f"Civ VI starting (pid {process.pid})")
            write_current({"pid": os.getpid(), "tag": args.tag, "run_dir": str(run_dir),
                           "started": utc(), "players": args.players, "launched_civ": True})
            if not launcher.wait_for_launched_main_menu(process, 420):
                log("⚠ Civ VI did not reach its main menu in 7 minutes. Look at it: an update, a "
                    "Steam or 2K dialog, or a crash report can stand in the way. Close what is in "
                    "the way, then `civ6-hotseat.sh --stop --quit-game` and the same command again.")
                return 3
            # The verification seat's half-screen window leaves room for its
            # viewer; a person playing wants the whole screen. The stub test
            # keeps the seat's layout.
            if args.auto_human or getattr(args, "civvis_seats", None):
                play.place_game(play.GAME_SIDE, play.GAME_FRACTION, play.GAME_VFRACTION)
            else:
                play.place_game("left", 1.0, 1.0)
                fit_on_screen()
        bridges = [watch.EventLogBridge(run_dir, tag=args.tag)]
        for seat in agent_seats(args.players, args.auto_human, getattr(args, "civvis_seats", None)):
            bridges.append(watch.EventLogBridge(
                run_dir / modinstall.hotseat_seat_tag(args.tag, seat),
                tag=modinstall.hotseat_seat_tag(args.tag, seat)))
        # Whether START GAME was this harness's to press (and so to press again).
        ours_to_start = not args.attach and not getattr(args, "hold_start", False)
        # A setup the person finishes: what was left, and the workers wait for
        # the game rather than start for seats nobody has seated yet.
        handed: dict | None = None
        if not args.attach and not set_up_game(run_dir, args, log):
            ours_to_start = False
            if args.auto_human or getattr(args, "civvis_seats", None):
                log("setup failed; the game window is left as it is for a look")
                stop_reason = "setup failed"
                return 5
            # A person is at the Mac: the rest of the setup is a few clicks, and
            # everything after it is the same game. Wait for it instead of quitting.
            handed = getattr(args, "setup_failed", None) or {"step": None}
            hand_over_setup(args, handed, log)
        elif not args.attach and (args.auto_human or getattr(args, "civvis_seats", None)):
            # In the background: the brains start while the map loads.
            threading.Thread(target=park_upper_left, args=(log,), name="upper-left",
                             daemon=True).start()
        brains = Brains(run_dir, args.tag, seats, orders_db, binary,
                        seat_lanes(args, seats), log, treatments=args.treatments)
        brains.poll_s = poll_seconds(args)
        workers_up = handed is None
        if workers_up:
            brains.start_all()
        monitor = Monitor(run_dir, args.tag, seats, args, log)
        if args.attach:
            for bridge in bridges:
                bridge.pump()
            monitor.catch_up()
        log("running. `tools/civ6_hotseat.py --stop` (or Ctrl-C) stops the automation; "
            "the game itself stays up.")
        waited_for_game = began_waiting = checked_start = time.monotonic()
        posted = False
        # Whether Civ VI still runs, without a `ps` every pass: the relay reads
        # nothing while one runs (~28 ms, and seconds on a loaded host). An
        # exit is still confirmed by a real listing (`watch.GameLiveness`).
        liveness = watch.GameLiveness()
        while True:
            for bridge in bridges:
                bridge.pump()
            monitor.poll()
            if not workers_up and monitor.started:
                workers_up = True
                log("the game has started: CIVVIS takes its seats")
                brains.start_all()
            for seat in monitor.take_unseated():
                brains.retire(seat)
            brains.keep_alive()
            if os.name == "nt" and (run_dir / "stop.json").is_file():
                raise Stop("stop requested")
            if not liveness.alive():
                stop_reason = "Civ VI closed"
                log("Civ VI has closed")
                break
            if (args.turns is not None and monitor.turn > args.turns
                    and monitor.active is not None):
                stop_reason = f"reached turn {args.turns}"
                log(f"turn {args.turns} is complete")
                break
            if monitor.setup_fault and monitor.unattended:
                stop_reason = SETUP_FAULT_REASON
                log(f"stopping the table: {monitor.setup_fault}")
                break
            if monitor.result is not None and monitor.unattended:
                stop_reason = "game over"
                # The end screen stays up a little, as civ6_play holds it, so a
                # recording of the table ends on Civ VI's own verdict.
                for _ in range(24):
                    for bridge in bridges:
                        bridge.pump()
                    monitor.poll()
                    time.sleep(0.5)
                break
            if monitor.result is not None and not posted and not monitor.unattended \
                    and person_name(args):
                # The person's game is decided: on the record now, not when
                # they get round to closing Civ VI (the Victory screen offers
                # "one more turn").
                posted = True
                (run_dir / "summary.json").write_text(json.dumps(
                    {"tag": args.tag, "utc": utc(), "reason": "game over",
                     **table_record(monitor, args, seats, began)}, indent=2))
                record_person_game(run_dir, log)
            if not monitor.started and ours_to_start \
                    and time.monotonic() - checked_start > START_CHECK_SECONDS:
                checked_start = time.monotonic()
                press_start_again(run_dir, log, checked_start - began_waiting)
            if not monitor.started and time.monotonic() - waited_for_game > (
                    NOT_STARTED_HANDED_S if handed else NOT_STARTED_S):
                waited_for_game = time.monotonic()
                log("⚠ " + not_started_advice(run_dir, args, waited_for_game - began_waiting))
            if monitor.stalled(args.stall_seconds):
                rescue_popup(monitor, log)
            relay_for(bridges, read_s=relay_seconds(args))
    except Stop as stop:
        stop_reason = f"stopped ({stop})"
        log(f"stopping: {stop}")
    finally:
        signal.signal(signal.SIGINT, signal.SIG_IGN)
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        if brains is not None:
            brains.stop()
        for bridge in bridges:
            try:
                bridge.pump()
            except OSError:
                pass
        summary = {"tag": args.tag, "utc": utc(), "reason": stop_reason}
        if monitor is not None:
            monitor.poll()
            summary["report"] = smoke_report(monitor, log)
            summary.update(table_record(monitor, args, seats, began))
            monitor.close()
        request = {}
        try:
            request = json.loads((run_dir / "stop.json").read_text())
        except (OSError, ValueError):
            pass
        # The record first: closing Civ VI can take minutes on a loaded host,
        # and the supervisor reads the summary to know a game is over.
        (run_dir / "summary.json").write_text(json.dumps(summary, indent=2))
        if summary.get("person") and (summary.get("result") or request.get("resign")):
            record_person_game(run_dir, log, resigned=not summary.get("result"))
        elif request.get("resign") and not summary.get("person"):
            log("the game was played under no --name, so the resignation is not put on the "
                "people's record")
        # An unattended table always closes Civ VI: the next table starts clean.
        quit_game = launched and (bool(request.get("quit_game"))
                                  or (args.auto_human and args.turns is not None)
                                  or bool(getattr(args, "civvis_seats", None)))
        if quit_game and env.game_pids():
            log("closing Civ VI")
            if not quit_civ(run_dir, log):
                log("Civ VI did not close; quit it from its menu")
        if not env.game_pids():
            try:
                modinstall.uninstall()
            except Exception as error:  # noqa: BLE001 - cleanup must finish
                log(f"could not remove the control mod: {error}")
        gamelock.release()
        current = read_current()
        if current and current.get("pid") == os.getpid():
            CURRENT.unlink(missing_ok=True)
        log(f"done ({stop_reason}); record in {run_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
