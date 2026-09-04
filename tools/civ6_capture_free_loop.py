#!/usr/bin/env python3
"""Run recorded CIVVIS games through a capture-free, supervised owner.

The normal ``civ6_civvis_climb.py`` path intentionally uses screenshots to
support arbitrary Civ VI menus.  During a desktop recording the operator needs
the inverse trade-off: a verified fixed profile with no screen capture or OCR.
This driver owns that profile from Create Game through the attached CIVVIS
player, so a completed or genuinely wedged game returns to the existing
fresh-head supervisor instead of leaving an unowned ``--attach-running``
process behind.

It supports exactly the known capture-free profile: Rome (Trajan), Emperor,
Online speed, Continents, Small, Gathering Storm, no game modes.  Rejecting a
different profile is intentional.  A wrong game is worse than a refused game,
and the visual launcher remains available for arbitrary configurations.
"""

from __future__ import annotations

import argparse
import json
import os
import signal
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import civ6_brain  # noqa: E402
import civ6_capture_free_setup as setup  # noqa: E402
import civ6_env as env  # noqa: E402
from civ6_control import gamelock, install, launcher  # noqa: E402
from civ6_control.orders import orders_db_path, reset_orders_db  # noqa: E402


RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"
LOG_ROOT = Path.home() / "civvis-climb-logs"
GAME_MODES = (
    "GAMEMODE_APOCALYPSE",
    "GAMEMODE_BARBARIAN_CLANS",
    "GAMEMODE_DRAMATICAGES",
    "GAMEMODE_HEROES",
    "GAMEMODE_MONOPOLIES",
    "GAMEMODE_RANDOM",
    "GAMEMODE_SECRETSOCIETIES",
    "GAMEMODE_TOWERDEFENSE",
    "GAMEMODE_TREE_RANDOMIZER",
)

PROFILE = {
    "difficulty": "DIFFICULTY_EMPEROR",
    "leader": "LEADER_TRAJAN",
    "ruleset": "RULESET_EXPANSION_2",
    "map": "Continents.lua",
    "map_size": "MAPSIZE_SMALL",
    "speed": "GAMESPEED_ONLINE",
}

STOP_REQUESTED = False


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def run_tag(index: int) -> str:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    return f"civvis-{stamp}-capture-free-{index}"


def atomic_json(path: Path, payload: dict) -> None:
    """Write a run-local receipt without exposing a partial JSON document."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "w", encoding="utf-8", dir=path.parent,
            prefix=f".{path.name}.", suffix=".tmp", delete=False,
        ) as handle:
            temporary = handle.name
            json.dump(payload, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    except OSError:
        if temporary:
            Path(temporary).unlink(missing_ok=True)
        raise


def build_config(args: argparse.Namespace, tag: str, orders_db: Path) -> dict:
    """The exact control-mod configuration for the fixed capture-free profile."""
    return {
        "RunTag": tag,
        "AutoStart": True,
        "Play": True,
        "Rehost": False,
        "SetToDefaults": True,
        "RuleSet": args.ruleset,
        "MapScript": args.map,
        "MapSize": args.map_size,
        "Difficulty": args.difficulty,
        "GameSpeed": args.speed,
        "MaxTurns": args.max_turns,
        "HumanPlayers": 1,
        "Leader": args.leader,
        "GameModes": {mode: False for mode in GAME_MODES},
        "OrdersDb": str(orders_db),
        "CivvisDecides": True,
        "ExportState": True,
        "DealSessions": False,
    }


def validate_profile(args: argparse.Namespace) -> None:
    """Fail closed rather than click fixed controls for different settings."""
    for name, expected in PROFILE.items():
        actual = getattr(args, name)
        if actual != expected:
            raise ValueError(
                f"capture-free profile requires {name}={expected}, got {actual}"
            )
    if args.game_mode:
        raise ValueError("capture-free profile requires every game mode disabled")


def prepare_run(args: argparse.Namespace, tag: str, root: Path = RUN_ROOT) -> tuple[Path, Path]:
    """Install the exact mod config and initialize its SQLite channel before play."""
    run_dir = root / tag
    run_dir.mkdir(parents=True, exist_ok=False)
    orders_db = orders_db_path(run_dir)
    reset_orders_db(orders_db)
    # Civ VI ATTACHes this database during its first turn.  Initializing the
    # schema before Start Game prevents the first board being stranded while a
    # worker races to create its tables.
    connection = civ6_brain.connect(orders_db)
    connection.close()
    config = build_config(args, tag, orders_db)
    install.install(config)
    atomic_json(run_dir / "capture-free-config.json", config)
    return run_dir, orders_db


def start_game(args: argparse.Namespace, tag: str, run_dir: Path) -> bool:
    """Launch Civ VI and press only the recorded profile's known controls."""
    launcher.clear_run_logs()
    launcher.launch(stdout=run_dir / "civ6-launch.log")
    if not launcher.wait_for_main_menu(args.launch_timeout):
        return False
    setup.start_direct_game(restore_defaults=True, emperor_online=True)
    return True


def attach_command(args: argparse.Namespace, tag: str, orders_db: Path) -> list[str]:
    """Build the non-visual player command that owns CIVVIS's decision worker."""
    binary = Path(args.civvis_bin).expanduser()
    command = [
        sys.executable, str(HERE / "civ6_play.py"),
        "--tag", tag,
        "--orders-db", str(orders_db),
        "--difficulty", args.difficulty,
        "--ruleset", args.ruleset,
        "--map", args.map,
        "--map-size", args.map_size,
        "--speed", args.speed,
        "--leader", args.leader,
        "--max-turns", str(args.max_turns),
        "--timeout", str(args.timeout),
        "--timeout-ceiling", str(args.timeout_ceiling),
        "--lock-wait", "30",
        "--attach-running",
        "--focus-every", "10",
        "--export-state",
        "--no-deal-sessions",
        "--civvis-decides",
        "--civvis-bin", str(binary),
        "--civvis-victory", args.victory,
        "--civvis-refresh-seconds", str(args.refresh_seconds),
        "--restart-below-leader-ratio", "0",
    ]
    for treatment in args.with_:
        command.extend(("--civvis-with", treatment))
    for treatment in args.without:
        command.extend(("--civvis-without", treatment))
    return command


def start_attached_player(args: argparse.Namespace, tag: str, run_dir: Path,
                          orders_db: Path) -> subprocess.Popen:
    """Start the attached player with its output retained beside the game."""
    command = attach_command(args, tag, orders_db)
    with (run_dir / "attach.log").open("a", buffering=1) as log:
        return subprocess.Popen(
            command, cwd=HERE.parent, stdout=log, stderr=subprocess.STDOUT,
            text=True,
        )


def event_mtime(events: Path) -> int | None:
    try:
        return events.stat().st_mtime_ns
    except OSError:
        return None


def latest_turn(events: Path) -> int | None:
    """Return the highest structured state/turn number retained by the run."""
    best: int | None = None
    try:
        stream = events.open(errors="replace")
    except OSError:
        return None
    with stream:
        for raw in stream:
            try:
                event = json.loads(raw)
                turn = event.get("turn")
            except (ValueError, AttributeError):
                continue
            if isinstance(turn, int):
                best = turn if best is None else max(best, turn)
    return best


def appended_turn(events: Path, offset: int) -> tuple[int, int | None]:
    """Read complete newly appended events and return their highest turn.

    The capture-free control mod deliberately keeps emitting ``ui_heartbeat``
    records while a native popup close ladder is trying again.  That proves Civ
    VI is alive, but it must not indefinitely impersonate turn progress.  Keep
    an append-only cursor so the supervisor can distinguish a living UI from a
    game that has made no strategic progress.

    A log write can be observed halfway through its final JSON line.  Do not
    advance the cursor past that line: the next poll will read it whole instead
    of losing the turn that finally proves recovery.
    """
    try:
        if events.stat().st_size < offset:
            offset = 0
        stream = events.open(errors="replace")
    except OSError:
        return offset, None

    best: int | None = None
    with stream:
        stream.seek(offset)
        while True:
            raw = stream.readline()
            if not raw:
                break
            if not raw.endswith("\n"):
                break
            offset = stream.tell()
            try:
                event = json.loads(raw)
                turn = event.get("turn")
            except (ValueError, AttributeError):
                continue
            if isinstance(turn, int):
                best = turn if best is None else max(best, turn)
    return offset, best


def sample_game(run_dir: Path) -> None:
    """Leave a short stack sample before abandoning a proven silent game."""
    if not Path("/usr/bin/sample").is_file():
        return
    pids = env.game_pids()
    if not pids:
        return
    try:
        subprocess.run(
            ["/usr/bin/sample", str(pids[0]), "1", "-file",
             str(run_dir / "capture-free-wedge-sample.txt")],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            timeout=20, check=False,
        )
    except (OSError, subprocess.SubprocessError):
        pass


def monitor_player(
    player: subprocess.Popen,
    events: Path,
    *,
    silence_s: float,
    frozen_turn_s: float | None = None,
    poll_s: float,
    should_stop: Callable[[], bool] = lambda: STOP_REQUESTED,
    now: Callable[[], float] = time.monotonic,
    sleep: Callable[[float], None] = time.sleep,
) -> str:
    """Wait for a player, escalating after a silent or turn-frozen game.

    A turn can be slow, but a healthy game keeps publishing state, await, or
    blocker records.  A silent stream is therefore a wedge.  Conversely, a
    native popup close ladder may keep publishing heartbeats forever while the
    turn never changes; that is a separate wedge.  Both cases return control to
    the supervisor for a clean successor, never to a visual or blind-input
    recovery path.
    """
    last_mtime = event_mtime(events)
    last_activity = now()
    last_turn = latest_turn(events)
    last_turn_progress = last_activity
    turn_offset = 0
    while player.poll() is None:
        if should_stop():
            return "stopped"
        current_mtime = event_mtime(events)
        if current_mtime is not None and current_mtime != last_mtime:
            last_mtime = current_mtime
            last_activity = now()
            turn_offset, observed_turn = appended_turn(events, turn_offset)
            if (observed_turn is not None
                    and (last_turn is None or observed_turn > last_turn)):
                last_turn = observed_turn
                last_turn_progress = last_activity
        current = now()
        if current - last_activity >= silence_s:
            return "wedge"
        if (frozen_turn_s is not None and last_turn is not None
                and current - last_turn_progress >= frozen_turn_s):
            return "frozen-turn"
        sleep(poll_s)
    return "completed" if player.returncode == 0 else "player-exited"


def owns_game(tag: str) -> bool:
    """Whether the installed control configuration still names this attempt."""
    try:
        config = install.installed_config()
    except (OSError, ValueError, json.JSONDecodeError):
        return False
    return isinstance(config, dict) and config.get("RunTag") == tag


def stop_owned_attempt(player: subprocess.Popen, tag: str) -> bool:
    """Give the attached player a clean exit, then stop only its known game."""
    if player.poll() is None:
        try:
            player.send_signal(signal.SIGINT)
        except ProcessLookupError:
            pass
        try:
            player.wait(timeout=30)
        except subprocess.TimeoutExpired:
            # The game is the player’s only reason to remain.  Stopping the
            # known owned game lets its normal absent-core timeout clean up,
            # without a destructive SIGKILL of either process.
            pass
    if not owns_game(tag):
        return False
    return launcher.stop()


def write_play_marker(logs: Path, tag: str, turn: int | None, reason: str) -> None:
    """Keep the supervisor's existing ‘played a turn’ contract intact."""
    if turn is None or turn < 1:
        return
    logs.mkdir(parents=True, exist_ok=True)
    (logs / f"{tag}-play.log").write_text(
        f"[turn {turn}] capture-free {reason}\n", encoding="utf-8"
    )


def run_once(args: argparse.Namespace, index: int) -> tuple[bool, str, str]:
    """Run one complete capture-free attempt; the supervisor owns successors."""
    if gamelock.operator_halt_description() is not None:
        return False, "halted", "no run was prepared while an operator halt is active"
    if gamelock.verification_intent() != "running":
        return False, "not-authorized", "verification intent is not running"
    if env.game_pids():
        return False, "foreign-game", "refusing to replace a game this loop did not start"
    tag = run_tag(index)
    run_dir: Path | None = None
    player: subprocess.Popen | None = None
    reason = "launch-failed"
    try:
        run_dir, orders_db = prepare_run(args, tag)
        if not start_game(args, tag, run_dir):
            return False, reason, f"{tag}: Civ VI did not reach the main menu"
        player = start_attached_player(args, tag, run_dir, orders_db)
        reason = monitor_player(
            player, run_dir / "events.jsonl", silence_s=args.wedge_silence,
            frozen_turn_s=args.frozen_turn_seconds, poll_s=args.poll,
        )
        if reason in ("wedge", "frozen-turn"):
            sample_game(run_dir)
        turn = latest_turn(run_dir / "events.jsonl")
        write_play_marker(Path(args.logs), tag, turn, reason)
        atomic_json(run_dir / "capture-free-result.json", {
            "tag": tag,
            "finished_utc": utc_now(),
            "reason": reason,
            "turn": turn,
            "wedge_detected": reason == "wedge",
            "frozen_turn_detected": reason == "frozen-turn",
            "player_returncode": player.poll(),
        })
        return turn is not None and turn >= 1, reason, tag
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
        return False, "error", f"{tag}: {error}"
    finally:
        if run_dir is not None:
            if player is not None:
                stop_owned_attempt(player, tag)
            # A failed menu start still has this run's control configuration
            # installed.  It is safe to close that known game; leaving it up
            # would make the next supervisor cycle look like a foreign owner.
            elif owns_game(tag):
                launcher.stop()


def request_stop(_signum, _frame) -> None:
    global STOP_REQUESTED
    STOP_REQUESTED = True


def parser() -> argparse.ArgumentParser:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--attempts", type=int, default=1)
    ap.add_argument("--difficulty", default=PROFILE["difficulty"])
    ap.add_argument("--leader", default=PROFILE["leader"])
    ap.add_argument("--ruleset", default=PROFILE["ruleset"])
    ap.add_argument("--map", default=PROFILE["map"])
    ap.add_argument("--map-size", dest="map_size", default=PROFILE["map_size"])
    ap.add_argument("--speed", default=PROFILE["speed"])
    ap.add_argument("--game-mode", action="append", default=[])
    ap.add_argument("--max-turns", type=int, default=650)
    ap.add_argument("--timeout", type=float, default=10_800.0)
    ap.add_argument("--timeout-ceiling", type=float, default=14_400.0)
    ap.add_argument("--launch-timeout", type=float, default=420.0)
    ap.add_argument("--victory", default="science")
    ap.add_argument("--refresh-seconds", type=float, default=0.0)
    ap.add_argument("--with", dest="with_", action="append", default=[])
    ap.add_argument("--without", action="append", default=[])
    ap.add_argument("--logs", default=str(LOG_ROOT))
    ap.add_argument("--civvis-bin",
                    default=str(HERE.parent / "target" / "release" / "civvis_orders"))
    ap.add_argument("--wedge-silence", type=float, default=120.0)
    ap.add_argument("--frozen-turn-seconds", type=float, default=1800.0,
                    help="restart only when a live event stream has not advanced "
                         "a turn for this long (default: 30 minutes)")
    ap.add_argument("--poll", type=float, default=2.0)
    return ap


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        validate_profile(args)
    except ValueError as error:
        parser().error(str(error))
    if args.attempts < 1 or args.max_turns < 1:
        parser().error("--attempts and --max-turns must be positive")
    if (args.wedge_silence <= 0 or args.frozen_turn_seconds <= 0
            or args.poll <= 0):
        parser().error("wedge timing values must be positive")
    if not Path(args.civvis_bin).is_file():
        print(f"CIVVIS decision binary does not exist: {args.civvis_bin}",
              file=sys.stderr)
        return 2

    signal.signal(signal.SIGINT, request_stop)
    signal.signal(signal.SIGTERM, request_stop)
    played = False
    for index in range(1, args.attempts + 1):
        if STOP_REQUESTED:
            break
        advanced, reason, detail = run_once(args, index)
        played = played or advanced
        print(f"[capture-free] {detail}: {reason}", flush=True)
        if STOP_REQUESTED:
            break
    return 0 if played else 1


if __name__ == "__main__":
    raise SystemExit(main())
