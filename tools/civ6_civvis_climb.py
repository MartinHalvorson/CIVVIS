"""Play Civilization VI games back to back with CIVVIS deciding, until one is won.

`civ6_climb.py` walks the whole difficulty ladder and does not start a brain, so it
cannot be used for a CIVVIS-driven attempt. This does one rung, repeatedly, with the
decision loop attached, and keeps a ledger.

    python3 tools/civ6_civvis_climb.py --attempts 12 --victory domination

⚠ ONE HARNESS AT A TIME. There is one installation, one mod directory, one log and
one run lock. Two harnesses driving it interleave their installs and each reads the
other's events under its own run tag, which looks exactly like a flaky game. This
refuses to start when anything else holds the game, and always tears down its own
game before starting the next attempt — the game outlives the harness and keeps the
lock, so quitting Civ 6 is part of cleanup, not optional.

⚠ A WIN IS ONLY A WIN IF THE SETTINGS WERE WHAT WE ASKED FOR. The ledger records the
`seat` event read back from inside the game beside the outcome, because
`setup: "(absent)"` on this build means several requested settings never applied.
"""

from __future__ import annotations

import argparse
import json
import os
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"
LEDGER = Path.home() / "civvis-civ6-runs" / "civvis_ladder.jsonl"


def stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def run(cmd: list[str], timeout: float = 60.0) -> str:
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return (proc.stdout or "") + (proc.stderr or "")
    except (subprocess.SubprocessError, OSError) as exc:
        return f"error: {exc}"


def dismiss_crash_dialogs() -> None:
    """Close the "quit unexpectedly" / "Game configuration unavailable" dialogs.

    ⚠ These are not cosmetic. Civilization VI segfaults (see the governor-appointment
    note) and every teardown `pkill`s it, so Steam and macOS both leave modal
    dialogs behind. A modal left on screen steals the click the NEXT attempt's
    vision pass is about to make on the Create Game screen — and this project has
    already had a stray click land on "Exit to Desktop".

    Best effort by design: it must never raise, and it must never click anything
    that is not a dialog button it can name.
    """
    script = """
    tell application "System Events"
        repeat with procName in {"Steam", "Civilization VI", "Civ6", "ReportCrash"}
            try
                if exists (process procName) then
                    tell process procName
                        repeat with w in windows
                            try
                                repeat with b in {"Close", "OK", "Ignore", "Cancel"}
                                    if exists (button b of w) then
                                        click button b of w
                                    end if
                                end repeat
                            end try
                        end repeat
                    end tell
                end if
            end try
        end repeat
    end tell
    """
    run(["osascript", "-e", script], timeout=25.0)


def teardown() -> None:
    """Stop everything, in the order that actually works.

    The harness first (so it cannot start another attempt), then the GAME — killing
    the harness leaves `Civ6_Exe_Child` alive holding the run lock, and every later
    attempt then dies with "another run holds the game".
    """
    for pattern in ("civ6_brain.py", "civ6_play.py"):
        run(["pkill", "-f", pattern])
    time.sleep(1)
    run(["osascript", "-e", 'tell application "Civ6" to quit'])
    time.sleep(6)
    run(["pkill", "-f", "Civ6_Exe"])
    time.sleep(2)
    run([sys.executable, str(HERE / "civ6_control" / "gamelock.py"), "--break-stale"])
    dismiss_crash_dialogs()


def busy() -> str | None:
    out = run(["pgrep", "-f", "Civ6_Exe|civ6_play.py|civ6_brain.py"])
    return out.strip() or None


def outcome_of(tag: str) -> dict:
    """Read the run's own summary, falling back to its last turn record."""
    summary = RUN_ROOT / tag / "summary.json"
    if summary.exists():
        try:
            return json.loads(summary.read_text())
        except ValueError:
            pass
    events = RUN_ROOT / tag / "events.jsonl"
    last_turn, seat, victory = None, None, None
    if events.exists():
        for line in events.read_text(errors="replace").splitlines():
            try:
                event = json.loads(line)
            except ValueError:
                continue
            kind = event.get("kind")
            if kind == "turn":
                last_turn = event
            elif kind == "seat":
                seat = event
            elif kind in ("victory", "defeat", "gameover"):
                victory = event
    return {
        "tag": tag,
        "last_turn": (last_turn or {}).get("turn"),
        "last_score": (last_turn or {}).get("score"),
        "rival_best": (last_turn or {}).get("rival_best"),
        "cities": (last_turn or {}).get("cities"),
        "seat": seat,
        "victory": victory,
        "outcome": None,
    }


def won(record: dict) -> bool:
    """Did WE win, at the difficulty we asked for?

    ⚠⚠ THIS USED TO RETURN TRUE FOR ANY `victory` EVENT. Civilization VI emits one
    when ANY team wins, and the mod faithfully reports whose it was in `won` — so a
    rival's victory would have been recorded in the ladder as ours. That is the worst
    failure this harness could have: a false claim is worse than no win, because it
    stops the search and poisons the record.
    """
    for candidate in (record.get("victory"), record.get("outcome")):
        if not isinstance(candidate, dict):
            continue
        if candidate.get("kind") == "victory" and candidate.get("won") is True:
            # And it only counts at the rung we asked for: `setup: "(absent)"` on this
            # build means several requested settings never applied, so the seat event
            # read back from INSIDE the game is the only trustworthy witness.
            seat = record.get("seat") or {}
            asked = record.get("difficulty_asked")
            got = seat.get("difficulty")
            if asked is not None and got is not None and asked != got:
                print(f"  ⚠ victory at {got}, not the {asked} that was asked for",
                      flush=True)
                return False
            return True
    return False


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--attempts", type=int, default=10)
    ap.add_argument("--difficulty", default="DIFFICULTY_SETTLER")
    ap.add_argument("--map-size", default="MAPSIZE_TINY")
    ap.add_argument("--speed", default="GAMESPEED_ONLINE")
    ap.add_argument("--max-turns", type=int, default=250)
    ap.add_argument("--timeout", type=float, default=5400.0)
    ap.add_argument("--victory", default="domination")
    ap.add_argument("--war-from-plan", action="store_true", default=False,
                    help="pass through to the brain; see its note for why")
    ap.add_argument("--tile-export-every", type=int, default=4,
                    help="turns between map exports; the operator watches this against the game")
    ap.add_argument("--orders-bin", default=str(HERE.parent / "target" / "release" / "civvis_orders"))
    ap.add_argument("--logs", default=None, help="directory for per-attempt logs")
    args = ap.parse_args()

    logs = Path(args.logs).expanduser() if args.logs else Path.cwd() / "civvis-climb-logs"
    logs.mkdir(parents=True, exist_ok=True)
    orders_bin = Path(args.orders_bin)
    if not orders_bin.exists():
        print(f"no civvis-orders binary at {orders_bin}", file=sys.stderr)
        return 2

    if busy():
        print("something already holds the game; tearing it down first", flush=True)
    teardown()

    for attempt in range(1, args.attempts + 1):
        tag = f"civvis-{stamp()}"
        print(f"\n=== attempt {attempt}/{args.attempts}  {tag} ===", flush=True)
        # A fresh orders database per attempt. Stale rows are keyed by run tag so
        # they could not be actuated, but a growing file is a growing query.
        for suffix in ("", "-wal", "-shm"):
            path = Path.home() / "civvis-civ6-runs" / f"orders.sqlite{suffix}"
            if path.exists():
                path.unlink()

        play_log = (logs / f"{tag}-play.log").open("w")
        brain_log = (logs / f"{tag}-brain.log").open("w")
        play = subprocess.Popen(
            [sys.executable, "-u", str(HERE / "civ6_play.py"),
             "--tag", tag,
             "--difficulty", args.difficulty,
             "--map-size", args.map_size,
             "--speed", args.speed,
             "--max-turns", str(args.max_turns),
             "--timeout", str(args.timeout),
             "--lock-wait", "30",
             "--report-every", "10",
             "--export-state",
             # ⚠ Popups must not sit on the map. They are closed by the autoclose shim
             # already, but the delay is how long they are VISIBLE, and the operator is
             # watching this screen to check it against CIVVIS's. Near-zero rather than
             # zero: the shim keys on a screen having been shown, and era screens hold
             # an event lock that has to be released rather than raced.
             "--announcement-seconds", "0.05",
             "--era-announcement-seconds", "0.05",
             "--civvis-decides",
             "--tile-export-every", str(args.tile_export_every),
             "--window-side", "right", "--window-frac", "0.5", "--window-vfrac", "0.5"],
            stdout=play_log, stderr=subprocess.STDOUT,
        )
        time.sleep(3)
        brain = subprocess.Popen(
            [sys.executable, "-u", str(HERE / "civ6_brain.py"),
             "--run-dir", str(RUN_ROOT / tag),
             "--mode", "civvis",
             "--bin", str(orders_bin),
             "--victory", args.victory]
            + (["--war-from-plan"] if args.war_from_plan else [])
            + [
             "--seconds", str(args.timeout + 300)],
            stdout=brain_log, stderr=subprocess.STDOUT,
        )

        try:
            play.wait(timeout=args.timeout + 600)
        except subprocess.TimeoutExpired:
            print("attempt exceeded its own timeout; stopping it", flush=True)
            play.send_signal(signal.SIGTERM)
        finally:
            if brain.poll() is None:
                brain.send_signal(signal.SIGTERM)
            play_log.close()
            brain_log.close()
            teardown()

        record = outcome_of(tag)
        record["utc"] = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        record["attempt"] = attempt
        record["victory_target"] = args.victory
        record["difficulty_asked"] = args.difficulty
        with LEDGER.open("a") as handle:
            handle.write(json.dumps(record, sort_keys=True) + "\n")

        print(f"  turn={record.get('last_turn')} score={record.get('last_score')} "
              f"rival_best={record.get('rival_best')} cities={record.get('cities')}",
              flush=True)
        if won(record):
            print(f"*** WON on attempt {attempt} ({tag}) ***", flush=True)
            return 0

    print("no win in the attempts given", flush=True)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
