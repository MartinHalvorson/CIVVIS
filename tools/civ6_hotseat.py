#!/usr/bin/env python3
"""Play real Civilization VI against CIVVIS's flagship: a hotseat game.

One command starts Civ VI on this Mac, sets up a Multiplayer -> Hot Seat game
and plays it: **you** play seat 0 ("Player 1", the host row), and CIVVIS's
flagship strategy ``jack`` (the deployment genome, `src/strategies.rs` ROSTER
row ``jack``) plays every other seat, each with its own decision worker. Those
seats are named "CIVVIS jack 1..N" in the game.

    tools/civ6_hotseat.py                      # 4 players, Tiny Pangaea, Online, Prince
    tools/civ6_hotseat.py --leader LEADER_SALADIN --difficulty DIFFICULTY_KING
    tools/civ6_hotseat.py --stop               # stop the automation, leave the game up
    tools/civ6_hotseat.py --stop --quit-game   # ...and close Civ VI (it saves every 10 turns)
    tools/civ6_hotseat.py --resume             # load the newest hotseat save and play on
    tools/civ6_hotseat.py --attach             # pick a running hotseat game back up
    tools/civ6_hotseat.py --status

How a turn goes. Civ VI's hotseat plays the seats in order. On YOUR turn the
terminal prints a banner (and macOS shows a notification): nothing of ours
clicks, types, moves the camera or closes a screen until you end your turn --
press "Start Turn" on Civ VI's hand-over screen yourself. On a jack seat's turn
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
loads the newest hotseat save or autosave (``--resume PATH`` for a particular
one) through the Hot Seat page's Load Game, then plays on exactly like a new
game. ``--attach`` picks up a hotseat game that is still running with the mod
(after ``--stop`` or a closed terminal).

Known limits: you are always seat 0 (the host row). Diplomacy between you and a
jack seat works the way hotseat diplomacy does (proposals wait for the other
seat's turn). The setup is driven through Civ VI's menus by screen reading, so
keep hands off the mouse for the minute it takes; the window may be moved
afterwards. Game modes are left as Civ VI shows them (the in-game seat record
says which are on).

Testing: ``--auto-human --turns N`` replaces you with a stub (the mod's own
fallback ladder playing seat 0) and stops after turn N with a per-seat report.

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
from contextlib import closing
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


def seat_name(index: int) -> str:
    return f"CIVVIS jack {index}"


def victory_switch_key(victory: str) -> str:
    """`VICTORY_DIPLOMATIC` -> `diplomatic`, the key of the seat event's
    `victories` table (CivvisControlAgent.lua, `Game.IsVictoryEnabled`)."""
    return victory.removeprefix("VICTORY_").lower()


def default_max_turns(speed: str) -> int:
    return SPEED_TURNS.get(speed, 500)


def hotseat_config(args: argparse.Namespace, run_dir: Path, orders_db: Path) -> dict:
    """The mod's settings: the live seat's own, re-pointed at a hotseat game.

    Built from `civ6_play`'s parser defaults so every jack seat plays with the
    exact controller settings the verification seat uses; only what hotseat
    changes is overridden below.
    """
    flags = ["--tag", args.tag, "--difficulty", args.difficulty,
             "--map", args.map, "--map-size", args.map_size, "--speed", args.speed,
             "--max-turns", str(args.max_turns), "--civvis-decides",
             "--civvis-victory", args.victory, "--orders-db", str(orders_db)]
    if args.leader:
        flags += ["--leader", args.leader]
    # `--play-flags`: civ6_play switches every jack seat plays with (Tournament 6
    # passes the policy arms with a measured gain, e.g. --dv-join-winner).
    flags += shlex.split(getattr(args, "play_flags", None) or "")
    settings = play.build_parser().parse_args(flags)
    if not args.leader:
        settings.leader = None
    # `--victories-off`: civ6_play takes this list from the host policy
    # (`CIVVIS_VICTORIES_OFF`), which a hotseat table does not read; the table
    # names its own, and `Monitor` reads the started game's switches back.
    settings.victories_off = list(getattr(args, "victories_off", None) or [])
    config = play.build_config(settings)
    civvis_seats = getattr(args, "civvis_seats", None)
    seats = jack_seats(args.players, civvis_seats)
    config.update({
        "Hotseat": True,
        "HotseatSeats": seats,
        # `--auto-human gated` keeps seat 0 the operator's in every gate, with
        # the stub playing it once its hand-over is pressed: the operator path,
        # testable without a person. Unattended, no seat is a person's.
        "HotseatHumanSeats": ([] if civvis_seats or args.auto_human == "stub" else [HUMAN_SEAT]),
        "HotseatStubSeats": [HUMAN_SEAT] if args.auto_human and not civvis_seats else [],
        "HotseatNames": {str(seat): seat_name(i) for i, seat in enumerate(seats, 1)},
        "HotseatSaveEvery": args.save_every,
        "HotseatSaveName": args.tag,
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
    if args.auto_human:
        config["HotseatNames"][str(HUMAN_SEAT)] = "Test stub"
    return config


# --------------------------------------------------------------- the decider

def resolve_orders_bin(requested: str | None) -> Path | None:
    if requested:
        path = Path(requested).expanduser()
        return path if path.is_file() else None
    candidates = [VERIFICATION_BIN]
    if SNAPSHOTS.is_dir():
        candidates += sorted(SNAPSHOTS.glob("*/civvis_orders"),
                             key=lambda p: p.stat().st_mtime, reverse=True)
    candidates.append(CHECKOUT_BIN)
    for path in candidates:
        if path.is_file() and os.access(path, os.X_OK):
            return path
    return None


def freeze_binary(source: Path, run_dir: Path) -> Path:
    target = run_dir / "civvis_orders"
    if target.resolve() != source.resolve():
        shutil.copy2(source, target)
    provenance = {"source": str(source), "frozen": str(target), "utc": utc()}
    snapshot = source.with_name(source.name + ".provenance.json")
    if snapshot.is_file():
        try:
            provenance["snapshot"] = json.loads(snapshot.read_text())
        except ValueError:
            pass
    (run_dir / "civvis_orders.provenance.json").write_text(json.dumps(provenance, indent=2))
    return target


def brain_command(seat_dir: Path, orders_db: Path, binary: Path, victory: str) -> list[str]:
    """One jack seat's decision worker: the live seat's brain, pinned binary."""
    return [sys.executable, str(HERE / "civ6_brain.py"),
            "--run-dir", str(seat_dir), "--orders-db", str(orders_db),
            "--mode", "civvis", "--bin", str(binary), "--victory", victory,
            # One program for the whole game, and its heartbeat kept out of the
            # live lane's cache (`write_disabled_heartbeat`).
            "--github-refresh-seconds", "0",
            "--github-cache", str(seat_dir.parent / "runtime-cache"),
            "--seconds", str(14 * 24 * 3600)]


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
    try:
        command = subprocess.run(["ps", "-o", "command=", "-p", str(pid)],
                                 capture_output=True, text=True, timeout=5).stdout
    except (OSError, subprocess.SubprocessError):
        return None
    if "civ6_brain.py" not in command or str(seat_dir) not in command:
        return None
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


class Brains:
    """One `civ6_brain.py` per jack seat, restarted if one dies."""

    def __init__(self, run_dir: Path, tag: str, seats: list[int], orders_db: Path,
                 binary: Path, victory: str, log: Log):
        self.run_dir, self.tag, self.seats = run_dir, tag, seats
        self.orders_db, self.binary, self.victory, self.log = orders_db, binary, victory, log
        self.procs: dict[int, subprocess.Popen] = {}
        self.restarts: dict[int, int] = {seat: 0 for seat in seats}

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
            brain_command(seat_dir, self.orders_db, self.binary, self.victory),
            cwd=str(HERE.parent), stdout=out, stderr=subprocess.STDOUT, text=True,
            start_new_session=True)
        out.close()
        (seat_dir / "brain.pid").write_text(f"{self.procs[seat].pid}\n")

    def start_all(self) -> None:
        # Create the shared database, WAL and all, before three workers race to:
        # concurrent `PRAGMA journal_mode=WAL` calls fail "database is locked"
        # (measured: two of three workers died on their first line).
        import civ6_brain
        civ6_brain.connect(self.orders_db).close()
        for seat in self.seats:
            self.start(seat)
            time.sleep(0.3)
        self.log(f"decision workers up for seats {self.seats} "
                 f"(strategy {STRATEGY}, victory {self.victory}, bin {self.binary})")

    def keep_alive(self) -> None:
        for seat, proc in list(self.procs.items()):
            if proc.poll() is not None and self.restarts[seat] < 20:
                self.restarts[seat] += 1
                self.log(f"seat {seat}'s decision worker exited ({proc.returncode}); "
                         f"restarting ({self.restarts[seat]})")
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


def read_screen(run_dir: Path, name: str) -> tuple[tuple | None, list[dict]]:
    """The game window and every text line in it, in screen points."""
    bounds = play.game_window()
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
        word = row["norm"].split()[-1] if row["norm"] else ""
        if (word in SLOT_WORDS and len(row["norm"].split()) <= 2
                and x + 0.12 * w <= row["x"] <= x + 0.36 * w and top < row["y"] < bottom):
            out.append(dict(row, slot=word))
    return sorted(out, key=lambda r: r["y"])


def choose_from_dropdown(run_dir: Path, log: Log, row: dict, option: str,
                         bounds: tuple, step: int) -> bool:
    click(log, (row["x"] + 30, row["y"]))
    time.sleep(1.2)
    _, rows = read_screen(run_dir, f"slot-{step}-open")
    _, _, _, h = bounds
    picks = [r for r in find(rows, option)
             if row["y"] < r["y"] <= row["y"] + 0.15 * h and abs(r["x"] - row["x"]) < 120]
    if not picks:
        log(f"[setup] the seat list did not offer {option!r}; closing it")
        play.macos_input.press_key("escape")
        time.sleep(0.8)
        return False
    pick = min(picks, key=lambda r: r["y"])
    click(log, (pick["x"], pick["y"]))
    time.sleep(1.2)
    return True


def wanted_slots(players: int, humans: int | None = None) -> list[str]:
    """The staging room's seat column as asked: the host, then hotseat humans
    up to `humans` (every seat by default), then Firaxis AI ("Computer")."""
    humans = players if humans is None else humans
    return ["host"] + ["player"] * (humans - 1) + ["computer"] * (players - humans)


def slots_match(kinds: list[str], want: list[str]) -> bool:
    return (len(kinds) == len(want) and kinds[0] in ("host", "player")
            and kinds[1:] == want[1:])


def seat_the_players(run_dir: Path, bounds: tuple, players: int, log: Log,
                     humans: int | None = None) -> bool:
    """Make rows 1..humans-1 hotseat humans, the rest up to `players` Firaxis
    AI, and close every row after them (every seat human by default)."""
    want = wanted_slots(players, humans)
    for step in range(1, 30):
        _, rows = read_screen(run_dir, f"staging-{step}")
        seats = slot_rows(rows, bounds)
        kinds = [s["slot"] for s in seats]
        if not seats:
            time.sleep(2.0)
            continue
        if slots_match(kinds, want):
            log(f"[setup] staging room: {want.count('player') + 1} hotseat seats and "
                f"{want.count('computer')} Firaxis AI, verified")
            return True
        if len(seats) > players:
            choose_from_dropdown(run_dir, log, seats[-1], "Closed", bounds, step)
            continue
        if len(seats) < players:
            add = find(rows, "ADD PLAYER")
            if not add:
                log("[setup] fewer seats than wanted and no ADD PLAYER button")
                return False
            click(log, (add[0]["x"], add[0]["y"]))
            time.sleep(1.5)
            continue
        index = next(i for i in range(1, len(want)) if kinds[i] != want[i])
        if want[index] == "player":
            choose_from_dropdown(run_dir, log, seats[index], "Human", bounds, step)
        elif not choose_from_dropdown(run_dir, log, seats[index], "Computer", bounds, step):
            choose_from_dropdown(run_dir, log, seats[index], "AI", bounds, step)
    log("[setup] could not seat the players; the staging room is left as it is")
    return False


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


def start_game(run_dir: Path, bounds: tuple, log: Log) -> bool:
    """Press START GAME (the orb above its label) and see the staging room go."""
    _, _, _, h = bounds
    for attempt in range(1, 5):
        _, rows = read_screen(run_dir, f"start-{attempt}")
        if not find(rows, "STAGING ROOM") and not find(rows, "START GAME"):
            return True
        label = find(rows, "START GAME")
        if not label:
            time.sleep(2.0)
            continue
        x, y = label[0]["x"], label[0]["y"]
        click(log, (x, y - int(0.02 * h)) if attempt % 2 else (x, y))
        time.sleep(6.0)
    _, rows = read_screen(run_dir, "start-final")
    return not find(rows, "STAGING ROOM")


#: Where Civ VI keeps hotseat saves (`auto/` holds its autosaves). A save to
#: resume is copied here under one constant name, so the Load Game list shows it
#: as a row the screen reader can find without scrolling or filters.
HOTSEAT_SAVES = play.AUTOSAVE_DIR.parent.parent / "Hotseat"
RESUME_STEM = "civvis-hotseat-resume"


def newest_save(directory: Path = HOTSEAT_SAVES) -> Path | None:
    saves = [p for p in list(directory.glob("*.Civ6Save"))
             + list(directory.glob("auto/*.Civ6Save"))
             if p.is_file() and p.stem != RESUME_STEM]
    return max(saves, key=lambda p: p.stat().st_mtime) if saves else None


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


def set_up_game(run_dir: Path, args: argparse.Namespace, log: Log) -> bool:
    bounds = open_hotseat_setup(run_dir, log)
    if bounds is None:
        log("[setup] never reached the Hot Seat setup page")
        return False
    if args.resume_save is not None:
        if not load_saved_game(run_dir, bounds, log):
            return False
        _, rows = read_screen(run_dir, "resume-staging")
        kinds = [s["slot"] for s in slot_rows(rows, bounds)]
        log(f"[setup] the save's seats: {kinds}")
        if len(kinds) != args.players or any(k not in ("host", "player") for k in kinds):
            log(f"[setup] ⚠ the save does not read as {args.players} hotseat humans; "
                "the jack seats are the seats after yours")
        if not start_game(run_dir, bounds, log):
            log("[setup] START GAME did not start the saved game")
            return False
        log("[setup] saved game starting")
        return True
    _, rows = read_screen(run_dir, "host-settings")
    problems = check_host_settings(rows, args)
    if problems:
        log("[setup] the setup page is not what was asked: " + "; ".join(problems))
        return False
    log("[setup] setup page verified: " + ", ".join(
        f"{label} {value_beside(rows, label)}" for label in ("Game Speed", "Map", "Map Size")))
    confirm = find(rows, "Confirm Settings")
    click(log, (confirm[0]["x"], confirm[0]["y"]))
    time.sleep(4.0)
    if not seat_the_players(run_dir, bounds, args.players, log,
                            humans=getattr(args, "civvis_seats", None)):
        return False
    if not set_difficulties(run_dir, bounds, args.difficulty, log):
        return False
    if not start_game(run_dir, bounds, log):
        log("[setup] START GAME did not start the game")
        return False
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


def quit_civ(run_dir: Path, log: Log, timeout_s: float = 60.0) -> bool:
    """Close Civ VI from its own menu and answer its question.

    In a hotseat game the menu quit opens the in-game pause menu with an inline
    ARE YOU SURE? / YES (InGameTopOptionsMenu); nothing answers it on the
    operator's turn, because every CIVVIS context stands down then. This is the
    operator's own request, so the harness answers it, then falls back to the
    launcher's ordinary stop.
    """
    if not env.game_pids():
        return True
    env.request_macos_quit()
    deadline = time.monotonic() + timeout_s
    step = 0
    while time.monotonic() < deadline:
        time.sleep(2.0)
        if not env.game_pids():
            return True
        step += 1
        _, rows = read_screen(run_dir, f"quit-{step}")
        # In a game: the pause menu's inline ARE YOU SURE? / YES.
        yes = find(rows, "YES")
        if yes and find(rows, "ARE YOU SURE", exact=False):
            play.click_at(yes[0]["x"], yes[0]["y"])
            continue
        # On a setup screen: the EXIT TO DESKTOP / OK / Cancel modal, whose OK
        # the live lane finds as Cancel mirrored about the heading.
        if find(rows, "EXIT TO DESKTOP") and not find(rows, "RETURN TO GAME"):
            ok = find(rows, "OK")
            if ok:
                play.click_at(ok[0]["x"], ok[0]["y"])
            else:
                play.confirm_exit_dialog(run_dir / "setup")
            continue
        # The pause menu without its question yet.
        exit_row = find(rows, "EXIT TO DESKTOP")
        if exit_row and find(rows, "RETURN TO GAME"):
            play.click_at(exit_row[0]["x"], exit_row[0]["y"])
            continue
        if step % 5 == 0:
            env.request_macos_quit()
    if env.game_pids():
        launcher.stop(timeout_s=30)
    return not env.game_pids()


def notify(title: str, message: str) -> None:
    """A macOS notification: on screen, touching no window."""
    script = (f"display notification {json.dumps(message)} "
              f"with title {json.dumps(title)} sound name \"Glass\"")
    try:
        subprocess.run(["osascript", "-e", script], capture_output=True, timeout=10)
    except (OSError, subprocess.SubprocessError):
        pass


class Monitor:
    """Who is playing, from the mod's own events, and how long each seat took."""

    def __init__(self, run_dir: Path, tag: str, seats: list[int], args: argparse.Namespace,
                 log: Log):
        self.run_dir, self.tag, self.seats, self.args, self.log = run_dir, tag, seats, args, log
        self.names = {seat: seat_name(i) for i, seat in enumerate(seats, 1)}
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
            return "test stub (seat 0)" if self.args.auto_human else "you (Player 1)"
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

    def on_ending(self, seat: int | None, event: dict) -> None:
        """A victory ends the game for every seat; an elimination is one seat's.
        The table's result is ours when the winning team is one of our seats'."""
        kind = event.get("kind")
        if kind == "victory" and self.result is None:
            team = event.get("team")
            teams = self.our_teams()
            ours = team is not None and int(team) in teams
            self.result = {"kind": "victory", "team": team, "victory": event.get("victory"),
                           "turn": event.get("turn"), "ours": ours,
                           "winner_seat": teams.get(int(team)) if ours else None}
            self.log(f"game over at turn {event.get('turn')}: team {team} won "
                     f"({'ours' if ours else 'a rival'}), victory {event.get('victory')}")
        elif kind == "defeat" and event.get("ours") and seat is not None:
            self.defeated.add(seat)
            self.log(f"{self.name(seat)} was eliminated at turn {event.get('turn')}")
            if self.result is None and self.defeated >= set(self.seats):
                self.result = {"kind": "defeat", "turn": event.get("turn"), "ours": False}
                self.log("every CIVVIS seat is eliminated")

    def on_base(self, event: dict) -> None:
        kind = event.get("kind")
        if kind in ("victory", "defeat"):
            self.on_ending(None, event)
            return
        if kind == "hotseat_lobby":
            self.lobbies.append(event)
        elif kind == "hotseat_armed":
            self.say("the hand-over screen is under CIVVIS control")
        elif kind == "hotseat_turn":
            self.started = True
            player, turn = int(event.get("player", -1)), int(event.get("turn", -1))
            now = time.monotonic()
            if event.get("phase") == "begin":
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
                    self.say(f"turn {turn}: you ended your turn{took_text}; the jack seats play now")
                else:
                    self.say(f"turn {turn}: {self.name(player)} ended its turn{took_text}")
                if self.active == player:
                    self.active = None
        elif kind == "hotseat_saved":
            self.say(f"saved the game as {event.get('name')!r} (Load Game -> Hot Seat)")
        elif kind == "hotseat_start_turn" and event.get("pressed") is False:
            self.say(f"⚠ could not press Start Turn for seat {event.get('player')}")
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
        elif kind == "hotseat_pause_released":
            self.say(f"⚠ turn {event.get('turn')}: seat {event.get('player')}'s hand-over pause stood "
                     f"{event.get('waited')} s with no Start Turn on screen; released "
                     f"(still paused {event.get('still_paused')})")

    def your_turn(self, turn: int) -> None:
        if turn == self.notified_turn:
            return
        self.notified_turn = turn
        bar = "=" * 64
        print(f"\a\n{bar}\n  YOUR TURN — turn {turn}.  Press \"Start Turn\" in Civ VI.\n"
              f"  Nothing of CIVVIS's touches the screen until you end it.\n{bar}\n",
              flush=True)
        self.log(f"turn {turn}: your turn")
        if not self.args.no_notify:
            notify("Civ VI — your turn", f"Turn {turn}: the jack seats are done. Your move.")

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
            self.say(f"turn {event.get('turn')}: {self.name(seat)} — "
                     f"{event.get('orders_applied')} orders applied "
                     f"({event.get('orders_source')}), cities {event.get('cities')}, "
                     f"units {event.get('units')}, score {event.get('score')}")
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
            f"{stats['orders_applied']} orders applied, {stats['found']} cities founded, "
            f"now {last.get('cities')} cities / {last.get('units')} units, "
            f"score {last.get('score')}, source {last.get('orders_source')}")
    return report


# ------------------------------------------------------------ the lifecycle

def write_current(payload: dict) -> None:
    RUN_ROOT.mkdir(parents=True, exist_ok=True)
    CURRENT.write_text(json.dumps(payload, indent=2))


def read_current() -> dict | None:
    try:
        return json.loads(CURRENT.read_text())
    except (OSError, ValueError):
        return None


def harness_alive(pid: int) -> bool:
    try:
        out = subprocess.run(["ps", "-o", "command=", "-p", str(int(pid))],
                             capture_output=True, text=True, timeout=5).stdout
    except (OSError, subprocess.SubprocessError, ValueError):
        return False
    return "civ6_hotseat.py" in out


def stop_command(args: argparse.Namespace) -> int:
    current = read_current()
    if not current or not harness_alive(current.get("pid", -1)):
        print("no hotseat harness is running")
        if args.quit_game and env.game_pids():
            print("Civ VI is still up; it was not started by a running harness, so it is "
                  "left alone (quit it from its menu)")
        return 0
    run_dir = Path(current["run_dir"])
    if args.quit_game and current.get("manual_lobby"):
        print(f"{SCREEN_FREE_FLAG}: Civ VI and the game are yours, so they stay up "
              "(quit Civ VI from its own menu)")
    (run_dir / "stop.json").write_text(json.dumps({"quit_game": bool(args.quit_game),
                                                   "utc": utc()}))
    os.kill(int(current["pid"]), signal.SIGTERM)
    deadline = time.monotonic() + 120
    while time.monotonic() < deadline and harness_alive(current["pid"]):
        time.sleep(1.0)
    if harness_alive(current["pid"]):
        print(f"the harness (pid {current['pid']}) is still shutting down")
        return 1
    print(f"stopped hotseat {current.get('tag')}; log: {run_dir / 'hotseat.log'}")
    return 0


def status_command() -> int:
    current = read_current()
    if not current:
        print("no hotseat game on record")
        return 0
    alive = harness_alive(current.get("pid", -1))
    print(json.dumps(dict(current, harness_running=alive,
                          civ_running=bool(env.game_pids())), indent=2))
    turns = Path(current["run_dir"]) / "turns.jsonl"
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


def describe_lobby(lobby: dict, civvis: list[int], stub: bool = False) -> str:
    """The game as Civ VI reports it, in one line."""
    players = sorted((p for p in lobby.get("players") or [] if isinstance(p, dict)),
                     key=lambda p: p.get("id", 99))
    seats = []
    for player in players:
        seat = int(player.get("id", -1))
        if seat == HUMAN_SEAT:
            who = "test stub" if stub else "you"
        elif seat in civvis:
            who = seat_name(seat)
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
    if now - monitor.rescued_at < MANUAL_STALL_NOTE_S:
        return False
    monitor.rescued_at = now
    log(f"⚠ {monitor.name(monitor.active)} has made no move for "
        f"{int(now - monitor.last_event)} s on its turn. Nothing here clicks in --manual-lobby "
        "mode: if Civ VI shows a screen waiting for a click, close it (its own button, or Esc); "
        "if the map shows NEXT TURN, press it. CIVVIS carries on once the turn moves.")
    return True


def run_manual(args: argparse.Namespace) -> int:
    """`--manual-lobby`: install, say what to do, read the person's game back,
    and play its CIVVIS seats until Civ VI closes or the harness is stopped."""
    if not screen_free.engaged() or screen_free.loaded_for_real():
        print(f"{SCREEN_FREE_FLAG} runs screen-free from its first import: spell the flag out "
              f"in full ({SCREEN_FREE_FLAG})", file=sys.stderr)
        return 2
    args.tag = args.tag or "hotseat-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_dir = RUN_ROOT / args.tag
    orders_db = run_dir / "orders.sqlite"
    if "'" in str(orders_db) or "\\" in str(orders_db):
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

    def on_term(signum, _frame):  # noqa: ANN001
        raise Stop(f"signal {signum}")

    signal.signal(signal.SIGTERM, on_term)
    signal.signal(signal.SIGINT, on_term)
    write_current({"pid": os.getpid(), "tag": args.tag, "run_dir": str(run_dir),
                   "started": utc(), "manual_lobby": True})
    try:
        binary = resolve_orders_bin(args.orders_bin)
        if binary is None:
            log("no civvis_orders binary found; pass --orders-bin or build one "
                "(cargo build --release --locked --bin civvis_orders)")
            return 4
        binary = freeze_binary(binary, run_dir)
        civ_at_install = set(env.game_pids())
        config = manual_lobby_config(args, run_dir, orders_db)
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
        loaded_at: float | None = None
        late_said = False
        while True:
            beat.beat()
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
                civvis, problems = lobby_seats(event)
                log(describe_lobby(event, civvis, stub))
                for problem in problems:
                    log(("⚠ " if civvis else "") + problem)
                if civvis:
                    monitor.seats = civvis
                    brains = Brains(run_dir, args.tag, civvis, orders_db, binary, args.victory, log)
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
                brains.keep_alive()
            pids = set(env.game_pids())
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
            time.sleep(0.5)
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
            monitor.close()
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
        (run_dir / "summary.json").write_text(json.dumps(summary, indent=2))
        gamelock.release()
        current = read_current()
        if current and current.get("pid") == os.getpid():
            CURRENT.unlink(missing_ok=True)
        log(f"done ({stop_reason}); record in {run_dir}")
    return 0


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
    ap.add_argument("--resume", nargs="?", const="latest", default=None, metavar="SAVE",
                    help="start Civ VI and load a hotseat save instead of a new game: "
                         "a .Civ6Save path, or the newest hotseat save/autosave")
    ap.add_argument("--attach", action="store_true",
                    help="drive a hotseat game that is already running (a resumed save, "
                         "or after --stop or a closed terminal)")
    ap.add_argument("--stop", action="store_true", help="stop the running harness")
    ap.add_argument("--quit-game", action="store_true",
                    help="with --stop: also close Civ VI if the harness started it")
    ap.add_argument("--status", action="store_true")
    ap.add_argument("--civvis-seats", type=int, default=None, metavar="N",
                    help="unattended: CIVVIS plays seats 0..N-1 and the other seats up to "
                         "--players are Firaxis AI (no person at the Mac); the run stops at "
                         "the game's ending with the result in summary.json and closes Civ VI")
    ap.add_argument("--victories-off", default="", metavar="VICTORY_TYPE[,...]",
                    help="victory types switched off at setup, e.g. VICTORY_DIPLOMATIC "
                         "(the host policy's CIVVIS_VICTORIES_OFF is not read here); each "
                         "is read back from the started game, and an unattended table "
                         "whose game still has one on stops without a result")
    ap.add_argument("--play-flags", default="", metavar="FLAGS",
                    help="civ6_play switches every jack seat plays with, as one string "
                         "(--play-flags='--dv-join-winner --luxury-quote-proposals')")
    ap.add_argument("--auto-human", nargs="?", const="stub", choices=["stub", "gated"],
                    default=None,
                    help="TEST: a stub plays seat 0 instead of you; `gated` keeps seat 0 "
                         "the operator's in every gate (its Start Turn is not pressed)")
    ap.add_argument("--turns", type=int, default=None,
                    help="stop after this game turn (with --auto-human: then report)")
    ap.add_argument(SCREEN_FREE_FLAG, dest="manual_lobby", action="store_true",
                    help="you set up and start the Hot Seat game in Civ VI; this only installs "
                         "the mod, reads the log and writes orders: nothing is launched, "
                         "clicked, typed, captured or moved (no Screen Recording or "
                         "Accessibility permission, Pillow, swiftc or cliclick needed). The "
                         "game's settings are yours: --players/--map/--speed/--difficulty/"
                         "--leader are not used")
    args = ap.parse_args(argv)
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
        if argv is not None and not screen_free.engaged():
            print(f"{SCREEN_FREE_FLAG} must run screen-free from its first import "
                  "(CIVVIS_SCREEN_FREE=1 before civ6_control loads)", file=sys.stderr)
            return 2
        if not screen_free.engaged():
            # Abbreviated on the command line (`--manual`), so the module-level
            # switch missed it: start again screen-free rather than run with
            # the screen modules loaded (nothing of theirs has been called).
            os.execve(sys.executable, [sys.executable, str(Path(__file__).resolve()),
                                       *sys.argv[1:]],
                      dict(os.environ, CIVVIS_SCREEN_FREE="1"))
        current = read_current()
        if current and harness_alive(current.get("pid", -1)):
            print(f"a hotseat harness is already running (pid {current['pid']}, "
                  f"{current.get('tag')}); `--stop` it first", file=sys.stderr)
            return 6
        return run_manual(args)
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
    if args.civvis_seats is not None and not 1 <= args.civvis_seats < args.players:
        print(f"--civvis-seats must leave at least one Firaxis AI seat "
              f"(1..{args.players - 1} of --players {args.players})", file=sys.stderr)
        return 2
    args.resume_save = None
    if args.resume is not None:
        save = newest_save() if args.resume == "latest" else Path(args.resume).expanduser()
        if save is None or not save.is_file():
            print(f"no hotseat save to resume ({args.resume}); saves live in {HOTSEAT_SAVES}",
                  file=sys.stderr)
            return 2
        args.resume_save = save
    current = read_current()
    if current and harness_alive(current.get("pid", -1)):
        print(f"a hotseat harness is already running (pid {current['pid']}, "
              f"{current.get('tag')}); `--stop` it first", file=sys.stderr)
        return 6
    return run(args)


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
        args.players = len(attach_config.get("HotseatSeats") or []) + 1
        args.auto_human = (None if not attach_config.get("HotseatStubSeats")
                           else "gated" if attach_config.get("HotseatHumanSeats") else "stub")
        args.victory = attach_config.get("VictoryLane") or args.victory
        args.victories_off = list(attach_config.get("VictoriesOff") or [])
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
    launched = False
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
        elif (run_dir / "civvis_orders").is_file():
            binary = run_dir / "civvis_orders"
        log(f"jack seats {seats} play with {binary}")
        if not args.attach:
            if args.resume_save is not None:
                args.leader = None
            config = hotseat_config(args, run_dir, orders_db)
            reset_orders_db(orders_db)
            target = modinstall.install(config)
            (run_dir / "config.json").write_text(json.dumps(config, indent=2, sort_keys=True))
            log(f"installed the control mod for hotseat at {target}")
            if args.resume_save is not None:
                # The save names every seat's leader; a pinned one is "Invalid
                # Option" in its staging room (measured: one row, NO HUMAN PLAYERS).
                staged = stage_save(args.resume_save)
                log(f"resuming {args.resume_save} (staged as {staged.name})")
            log(f"game: {args.players} players on {args.map} {args.map_size}, "
                f"{args.speed}, {args.difficulty}, turn limit {args.max_turns}; "
                f"you: {args.leader or 'random leader'}; save every {args.save_every} turns")
            apply_person_options(log)
            launcher.clear_run_logs()
            process = launcher.launch(stdout=run_dir / "stdout.log")
            launched = True
            log(f"Civ VI starting (pid {process.pid})")
            write_current({"pid": os.getpid(), "tag": args.tag, "run_dir": str(run_dir),
                           "started": utc(), "players": args.players, "launched_civ": True})
            if not launcher.wait_for_launched_main_menu(process, 420):
                log("Civ VI never reached its main menu")
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
        if not args.attach and not set_up_game(run_dir, args, log):
            if args.auto_human or getattr(args, "civvis_seats", None):
                log("setup failed; the game window is left as it is for a look")
                stop_reason = "setup failed"
                return 5
            # A person is at the Mac: the rest of the setup is a few clicks, and
            # everything after it is the same game. Wait for it instead of quitting.
            print(f"\a\nSetup could not finish by itself (see above). Finish it in Civ VI:\n"
                  f"  Hot Seat setup -> Confirm Settings; in the staging room make Players 2-"
                  f"{args.players} 'Human', then START GAME.\n"
                  f"  CIVVIS takes over the jack seats as soon as the game starts.\n",
                  flush=True)
            log("setup handed to the operator; waiting for the game to start")
        elif not args.attach and (args.auto_human or getattr(args, "civvis_seats", None)):
            # In the background: the brains start while the map loads.
            threading.Thread(target=park_upper_left, args=(log,), name="upper-left",
                             daemon=True).start()
        brains = Brains(run_dir, args.tag, seats, orders_db, binary, args.victory, log)
        brains.start_all()
        monitor = Monitor(run_dir, args.tag, seats, args, log)
        if args.attach:
            for bridge in bridges:
                bridge.pump()
            monitor.catch_up()
        log("running. `tools/civ6_hotseat.py --stop` (or Ctrl-C) stops the automation; "
            "the game itself stays up.")
        waited_for_game = time.monotonic()
        while True:
            for bridge in bridges:
                bridge.pump()
            monitor.poll()
            brains.keep_alive()
            if not env.game_pids():
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
            if not monitor.started and time.monotonic() - waited_for_game > 300:
                log("⚠ no hand-over event five minutes after START GAME; is the game up?")
                waited_for_game = time.monotonic()
            if monitor.stalled(args.stall_seconds):
                rescue_popup(monitor, log)
            time.sleep(0.5)
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
            if monitor.unattended:
                summary.update({
                    "result": monitor.result, "seats": {str(k): v for k, v in monitor.seat_info.items()},
                    "eliminated": sorted(monitor.defeated), "turn": monitor.turn,
                    "difficulty": args.difficulty, "players": args.players, "map": args.map,
                    "map_size": args.map_size, "speed": args.speed, "max_turns": args.max_turns,
                    "civvis_seats": seats, "victory_target": args.victory,
                    "victories_off": monitor.victories_off, "victories": monitor.victories,
                    "setup_fault": monitor.setup_fault,
                    "seat_stalls": monitor.seat_stalls})
            monitor.close()
        request = {}
        try:
            request = json.loads((run_dir / "stop.json").read_text())
        except (OSError, ValueError):
            pass
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
        (run_dir / "summary.json").write_text(json.dumps(summary, indent=2))
        gamelock.release()
        current = read_current()
        if current and current.get("pid") == os.getpid():
            CURRENT.unlink(missing_ok=True)
        log(f"done ({stop_reason}); record in {run_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
