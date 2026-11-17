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

Saves. Civ VI autosaves every turn, and the mod also saves
``<tag>-T<turn>`` every ``--save-every`` turns (Load Game in the Hot Seat
setup). ``--attach`` picks up a hotseat game that is still running with this
mod (after ``--stop`` or a closed terminal). A save loaded into a fresh Civ VI
needs the mod installed for that run first, so a resumed save is: start the
harness's game as usual, then Load Game from the Hot Seat setup page instead of
the staging room; the run's mod reads the save's seats the same way.

Known limits: you are always seat 0 (the host row). Diplomacy between you and a
jack seat works the way hotseat diplomacy does (proposals wait for the other
seat's turn). The setup is driven through Civ VI's menus by screen reading, so
keep hands off the mouse for the minute it takes; the window may be moved
afterwards. Game modes are left as Civ VI shows them (the in-game seat record
says which are on).

Testing: ``--auto-human --turns N`` replaces you with a stub (the mod's own
fallback ladder playing seat 0) and stops after turn N with a per-seat report.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE / "civ6_control"))
import civ6_env as env  # noqa: E402
import civ6_play as play  # noqa: E402
from civ6_control import gamelock, launcher, operator_presence, popup_clear, watch  # noqa: E402
from civ6_control import install as modinstall  # noqa: E402
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

def jack_seats(players: int) -> list[int]:
    return [seat for seat in range(players) if seat != HUMAN_SEAT]


def agent_seats(players: int, auto_human: bool) -> list[int]:
    """Every seat with an agent context: the jack seats, and the test stub."""
    return jack_seats(players) + ([HUMAN_SEAT] if auto_human else [])


def seat_name(index: int) -> str:
    return f"CIVVIS jack {index}"


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
    settings = play.build_parser().parse_args(flags)
    if not args.leader:
        settings.leader = None
    config = play.build_config(settings)
    seats = jack_seats(args.players)
    config.update({
        "Hotseat": True,
        "HotseatSeats": seats,
        # `--auto-human gated` keeps seat 0 the operator's in every gate, with
        # the stub playing it once its hand-over is pressed: the operator path,
        # testable without a person.
        "HotseatHumanSeats": ([] if args.auto_human == "stub" else [HUMAN_SEAT]),
        "HotseatStubSeats": [HUMAN_SEAT] if args.auto_human else [],
        "HotseatNames": {str(seat): seat_name(i) for i, seat in enumerate(seats, 1)},
        "HotseatSaveEvery": args.save_every,
        "HotseatSaveName": args.tag,
        # The operator's seat's difficulty, and every jack seat's: hotseat
        # difficulty is per player (the staging room's Difficulty column).
        "PlayerDifficulty": args.difficulty,
        "Leader": args.leader,
        "HumanPlayers": args.players,
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
        out = (seat_dir / "brain.log").open("a", buffering=1)
        self.procs[seat] = subprocess.Popen(
            brain_command(seat_dir, self.orders_db, self.binary, self.victory),
            cwd=str(HERE.parent), stdout=out, stderr=subprocess.STDOUT, text=True,
            start_new_session=True)
        out.close()

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


def find(rows: list[dict], label: str, *, exact: bool = True) -> list[dict]:
    want = _norm(label)
    if exact:
        return [r for r in rows if r["norm"] == want]
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


def open_hotseat_setup(run_dir: Path, log: Log, attempts: int = 12) -> tuple | None:
    """Main menu -> Multiplayer -> Hot Seat, to the MULTIPLAYER GAME SETUP page."""
    for attempt in range(1, attempts + 1):
        wait_for_hands_off(log)
        play.ensure_game_frontmost()
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
        if shown is None or _norm(value) not in _norm(shown):
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


def seat_the_players(run_dir: Path, bounds: tuple, players: int, log: Log) -> bool:
    """Make rows 1..players-1 hotseat humans and close every row after them."""
    for step in range(1, 25):
        _, rows = read_screen(run_dir, f"staging-{step}")
        seats = slot_rows(rows, bounds)
        kinds = [s["slot"] for s in seats]
        if not seats:
            time.sleep(2.0)
            continue
        if len(seats) == players and kinds[0] in ("host", "player") \
                and all(k == "player" for k in kinds[1:]):
            log(f"[setup] staging room: {players} hotseat seats, verified")
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
        target = next(s for s in seats[1:] if s["slot"] != "player")
        choose_from_dropdown(run_dir, log, target, "Human", bounds, step)
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
    if not seat_the_players(run_dir, bounds, args.players, log):
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
        # On a setup screen: the EXIT TO DESKTOP / OK / Cancel modal.
        ok = find(rows, "OK")
        if ok and find(rows, "EXIT TO DESKTOP"):
            play.click_at(ok[0]["x"], ok[0]["y"])
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
        watched = agent_seats(len(seats) + 1, args.auto_human)
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
        if player == HUMAN_SEAT:
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

    def on_base(self, event: dict) -> None:
        kind = event.get("kind")
        if kind == "hotseat_armed":
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
    ap.add_argument("--auto-human", nargs="?", const="stub", choices=["stub", "gated"],
                    default=None,
                    help="TEST: a stub plays seat 0 instead of you; `gated` keeps seat 0 "
                         "the operator's in every gate (its Start Turn is not pressed)")
    ap.add_argument("--turns", type=int, default=None,
                    help="stop after this game turn (with --auto-human: then report)")
    args = ap.parse_args(argv)
    if args.stop:
        return stop_command(args)
    if args.status:
        return status_command()
    if args.leader and args.leader.lower() == play.RANDOM_LEADER:
        args.leader = None
    if args.max_turns is None:
        args.max_turns = default_max_turns(args.speed)
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
    elif env.game_pids():
        print("Civ VI is already running. Quit it first (this starts a fresh game), "
              "or pass --attach to drive a hotseat game you loaded.", file=sys.stderr)
        return 2
    args.tag = args.tag or "hotseat-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_dir = RUN_ROOT / args.tag
    run_dir.mkdir(parents=True, exist_ok=True)
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
    seats = jack_seats(args.players)
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
            config = hotseat_config(args, run_dir, orders_db)
            reset_orders_db(orders_db)
            target = modinstall.install(config)
            (run_dir / "config.json").write_text(json.dumps(config, indent=2, sort_keys=True))
            log(f"installed the control mod for hotseat at {target}")
            if args.resume_save is not None:
                staged = stage_save(args.resume_save)
                log(f"resuming {args.resume_save} (staged as {staged.name})")
            log(f"game: {args.players} players on {args.map} {args.map_size}, "
                f"{args.speed}, {args.difficulty}, turn limit {args.max_turns}; "
                f"you: {args.leader or 'random leader'}; save every {args.save_every} turns")
            play.apply_verification_options()
            launcher.clear_run_logs()
            process = launcher.launch(stdout=run_dir / "stdout.log")
            launched = True
            log(f"Civ VI starting (pid {process.pid})")
            write_current({"pid": os.getpid(), "tag": args.tag, "run_dir": str(run_dir),
                           "started": utc(), "players": args.players, "launched_civ": True})
            if not launcher.wait_for_launched_main_menu(process, 420):
                log("Civ VI never reached its main menu")
                return 3
            play.place_game(play.GAME_SIDE, play.GAME_FRACTION, play.GAME_VFRACTION)
        bridges = [watch.EventLogBridge(run_dir, tag=args.tag)]
        for seat in agent_seats(args.players, args.auto_human):
            bridges.append(watch.EventLogBridge(
                run_dir / modinstall.hotseat_seat_tag(args.tag, seat),
                tag=modinstall.hotseat_seat_tag(args.tag, seat)))
        if not args.attach and not set_up_game(run_dir, args, log):
            if args.auto_human:
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
            monitor.close()
        request = {}
        try:
            request = json.loads((run_dir / "stop.json").read_text())
        except (OSError, ValueError):
            pass
        quit_game = launched and (bool(request.get("quit_game"))
                                  or (args.auto_human and args.turns is not None))
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
