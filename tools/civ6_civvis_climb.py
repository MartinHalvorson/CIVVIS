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

⚠⚠ AN ATTEMPT THAT NEVER STARTED A GAME IS NOT AN ATTEMPT. On 2026-07-31 a login
killed Steam mid-batch; `launcher.launch` then raised "the Steam client is not
running" on every remaining iteration, each one took twelve seconds, and the loop
spent attempts 14 through 24 in two minutes before printing "no win in the attempts
given" — a conclusion drawn from eleven games that were never played. It destroyed
the thirteen-attempt frozen-build batch the whole night's work was to be judged on.

The budget counts MEASUREMENTS, not iterations. A precondition failure waits and
retries; it does not spend a rung. This is the same defect as every instrument this
project has had to repair: a number that reports a result where nothing happened.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"
LEDGER = Path.home() / "civvis-civ6-runs" / "civvis_ladder.jsonl"

sys.path.insert(0, str(HERE))
from civ6_control import launcher  # noqa: E402

# Backoff between blocked starts. The first steps are short because the usual cause
# is a Steam client that is coming back up on its own; the last is long because if
# it is still down after four minutes a human has to do something, and a loop
# polling every fifteen seconds for an hour teaches nobody anything.
BLOCKED_BACKOFF_S = (15.0, 30.0, 60.0, 120.0, 240.0)


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


def code_state() -> str:
    """A name for the program this attempt will actually run.

    ⚠ `rev + "+dirty"` DOES NOT IDENTIFY CODE. Most of the ledger's rows are stamped
    `0d6cc28+dirt`, and two rows carrying that string can be two different programs —
    the flag says only that something was uncommitted, never what. Uncommitted work is
    normal here and is not the problem; being unable to tell one uncommitted state
    from the next is.

    So a dirty tree is fingerprinted by its CONTENT, not by a boolean.
    `abc1234+9f2c1e04` changes the moment the working tree does, which is what makes a
    pinned batch enforceable at all.

    `diff HEAD` covers staged and unstaged changes to tracked files; `status
    --porcelain` is folded in because it lists UNTRACKED paths, and a brand new
    `tools/civ6_*.py` is exactly the kind of thing that appears mid-session and
    changes what runs. ⚠ The limit worth knowing: an untracked file's CONTENT is not
    hashed, only its existence, so editing one in place will not break a pin.

    ★★★★★ THE TREE THAT PLAYS THE GAME IS OFTEN NOT A GIT CHECKOUT AT ALL.
    `/Users/martin/civvis-settler-harness` — the copy most real attempts run from —
    has no `.git`, and `run()` returns stdout+stderr, so `git rev-parse` handed back
    its own failure and the ledger recorded rows stamped:

        "code_rev": "fatal: not a git repository (or any of the parent
                     directories): .git+036c4dc7"

    Worse than ugly: the `+036c4dc7` half is a hash of that same error text, so it is
    IDENTICAL for every non-repo tree and two genuinely different programs pin alike —
    the exact failure this function exists to prevent, wearing a different costume.

    So a revision is accepted only if it LOOKS like one, and a tree with no revision
    is named by hashing what will actually run.
    """
    root = HERE.parent
    rev = run(["git", "-C", str(root), "rev-parse", "--short", "HEAD"]).strip()
    if not re.fullmatch(r"[0-9a-f]{7,40}", rev):
        return f"nogit+{_tree_fingerprint(root)}"
    tree = (run(["git", "-C", str(root), "status", "--porcelain"])
            + run(["git", "-C", str(root), "diff", "HEAD"]))
    if tree.strip():
        return f"{rev}+{hashlib.sha1(tree.encode()).hexdigest()[:8]}"
    return rev


def _tree_fingerprint(root: Path) -> str:
    """Hash what a non-git tree will actually run: the harness and the mod.

    Only the files that decide behaviour, so an unrelated log or screenshot dropped
    beside them does not invent a new program. Read failures are folded in by name
    rather than skipped — a file that cannot be read is itself a difference.
    """
    digest = hashlib.sha1()
    targets = sorted(
        path
        for pattern in ("civ6_*.py", "civ6_control/*.py", "civ6_control/mod/*")
        for path in (root / "tools").glob(pattern)
        if path.is_file()
    )
    for path in targets:
        digest.update(str(path.relative_to(root)).encode())
        try:
            digest.update(path.read_bytes())
        except OSError as exc:
            digest.update(f"unreadable:{exc}".encode())
    return digest.hexdigest()[:8]


def blocked_reason() -> str | None:
    """Why an attempt cannot BEGIN, or None if it can. Checked before spending a rung.

    ⚠ THIS IS A DIFFERENT QUESTION FROM "DID THE ATTEMPT LOSE", and conflating the two
    is what cost the frozen-build batch. A game that starts and is conquered on turn
    240 is evidence. A game that never starts is the absence of evidence, and the two
    were indistinguishable to a loop that counted iterations.

    Deliberately cheap and deliberately narrow: it names only preconditions this
    harness cannot itself supply, so it can be polled every few seconds without
    becoming its own reason for a slow run. `launcher` is imported rather than
    re-implemented so there is ONE definition of "Steam is up" — the check here and
    the `SystemExit` inside `launcher.launch` must never be able to disagree.
    """
    if not launcher.steam_running():
        return "the Steam client is not running"
    binary = launcher.game_binary()
    if not binary.is_file():
        return f"game binary not found: {binary}"
    return None


def wake_steam() -> None:
    """Ask Steam to come back, in the background so it cannot steal the game's focus.

    Best effort. The harness already quits Civ 6, `pkill`s its children and clicks
    modal buttons through System Events, so starting the client it depends on is well
    inside what it already does — and the alternative, discovered the hard way, is a
    machine that sits idle all night because one app exited at a login.
    """
    run(["open", "-ga", "Steam"], timeout=30.0)


def tail_of(path: Path, limit: int = 200) -> str:
    """The last thing a dead attempt said, for the ledger row that records the hole.

    A blocked row whose reason is "no turn observed" tells the next reader nothing
    they could act on. The play log's final line is usually the whole diagnosis —
    "the Steam client is not running; the game cannot initialise" — and it costs one
    read to carry it into the ledger instead of leaving it in a file nobody opens.
    """
    try:
        lines = [ln.strip() for ln in path.read_text(errors="replace").splitlines()]
    except OSError:
        return ""
    for line in reversed(lines):
        if line:
            return line[:limit]
    return ""


def outcome_of(tag: str) -> dict:
    """The run's own summary, MERGED with what its event stream says.

    ⚠ THIS USED TO RETURN THE SUMMARY AND STOP. The summary has no `cities`, so every
    ledger row read `cities: None` — the one column that would have shown the empire
    growing from 1 to 4 was blank for the whole day, and comparisons fell back to
    reading individual runs by hand. Preferring one source over the other loses
    whatever only the other one holds; take both.
    """
    merged: dict = {}
    summary = RUN_ROOT / tag / "summary.json"
    if summary.exists():
        try:
            merged = json.loads(summary.read_text())
        except ValueError:
            merged = {}
    events = RUN_ROOT / tag / "events.jsonl"
    last_turn, seat, victory, ended_on_screen = None, None, None, None
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
            # ★★★★ A GAME THAT ENDED IS NOT A GAME THAT HUNG. Civilization VI's
            # end-game screen halts the game core, so the agent stops exporting and
            # the harness times out — and every such run was written down as
            # `stalled: no event for 240s` with a blank outcome. Attempt 2 was
            # photographed on the DEFEAT screen and recorded exactly that way.
            #
            # ⚠ This records that the game ENDED, never who won. `won()` still
            # demands the mod's victory event naming us.
            elif kind == "autoclose" and event.get("screen") == "EndGameMenu":
                ended_on_screen = event
    from_events = {
        "tag": tag,
        "last_turn": (last_turn or {}).get("turn"),
        "last_score": (last_turn or {}).get("score"),
        "rival_best": (last_turn or {}).get("rival_best"),
        "lead": (last_turn or {}).get("lead"),
        "cities": (last_turn or {}).get("cities"),
        "army": (last_turn or {}).get("army"),
        "met": (last_turn or {}).get("met"),
        "seat": seat,
        "victory": victory,
        # True means Civilization VI showed its end-of-game screen: the run
        # FINISHED. It says nothing about who won.
        "reached_end_screen": bool(ended_on_screen) or None,
        "end_screen_turn": (ended_on_screen or {}).get("turn"),
    }
    # The event stream wins where it has an answer; the summary keeps `reason` and
    # anything else only it records.
    for key, value in from_events.items():
        if value is not None or key not in merged:
            merged[key] = value
    return merged


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
    ap.add_argument("--no-pin", action="store_true", default=False,
                    help="allow the code to change mid-batch; rows stop being "
                         "comparable and the ledger can only say so afterwards")
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

    # A batch is a COMPARISON, so it is pinned to one program by default. Opting out
    # is a deliberate act with a name, not the silent default it used to be.
    pinned = None if args.no_pin else code_state()
    if pinned is not None:
        print(f"batch pinned to {pinned}"
              + ("  ⚠ working tree is dirty; the fingerprint tracks it"
                 if "+" in pinned else ""), flush=True)

    played = 0          # attempts that produced a MEASUREMENT — the only budget
    started = 0         # iterations, for the log line only
    blocked_streak = 0

    while played < args.attempts:
        # ⚠ Gate BEFORE the tag, the mod sync and the log files. A blocked start that
        # gets that far leaves a run directory and two empty logs behind, which is
        # what made the dead-Steam batch look like eleven real attempts on disk.
        reason = blocked_reason()
        if reason is not None:
            if blocked_streak >= len(BLOCKED_BACKOFF_S):
                print(f"cannot start a game: {reason}. Gave up after "
                      f"{blocked_streak} waits; {played}/{args.attempts} attempts "
                      f"were played. NO CONCLUSION IS AVAILABLE FROM THIS BATCH.",
                      flush=True)
                return 3
            wait = BLOCKED_BACKOFF_S[blocked_streak]
            blocked_streak += 1
            print(f"blocked: {reason} — waiting {wait:.0f}s "
                  f"(no attempt spent; {played}/{args.attempts} played)", flush=True)
            if "Steam" in reason:
                wake_steam()
            time.sleep(wait)
            continue

        attempt = played + 1
        started += 1
        tag = f"civvis-{stamp()}"
        # ⚠ THE CODE CHANGES BETWEEN ATTEMPTS AND THE LEDGER COULD ONLY SAY SO
        # AFTERWARDS. The harness re-installs the mod at the start of every attempt,
        # so a fix landed mid-batch takes effect on the next row — and every earlier
        # row then describes a different program under the same column headings.
        # Recording `code_rev` made that visible in hindsight and did nothing to stop
        # it: on 2026-07-31 a commit at 12:45 silently unfroze a batch that had been
        # deliberately frozen at `1ee5dcb` with eleven attempts still queued, and the
        # freeze existed only as an intention in an operator's head.
        # Captured HERE, at attempt start, because that is when the mod is synced.
        code_rev = code_state()
        if pinned is not None and code_rev != pinned:
            print(f"\nTHE BUILD CHANGED MID-BATCH — pinned {pinned}, now {code_rev}.\n"
                  f"  {played} attempt(s) were played on {pinned}. This batch ends "
                  f"here so its rows stay comparable;\n  the remaining "
                  f"{args.attempts - played} would have measured a different program "
                  f"under the same heading.\n  Start a new batch to measure "
                  f"{code_rev}, or pass --no-pin to mix revisions deliberately.",
                  flush=True)
            return 4
        print(f"\n=== attempt {attempt}/{args.attempts}  {tag}  code={code_rev} ===",
              flush=True)
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
        record["victory_target"] = args.victory
        record["difficulty_asked"] = args.difficulty
        record["code_rev"] = code_rev

        # ⚠ NO TURN WAS EVER OBSERVED, so this row is not a loss — it is a run that
        # did not happen. It is still written down, because a batch with holes in it
        # has to SHOW the holes, but it is marked and it does not spend a rung. The
        # game can also die after the gate above passed (Steam quitting between the
        # check and the launch is the exact case that started this), so the same
        # judgement has to be made here, on evidence, and not only up front.
        if record.get("last_turn") is None:
            record["blocked"] = tail_of(logs / f"{tag}-play.log") or "no turn observed"
            record["attempt"] = None
            with LEDGER.open("a") as handle:
                handle.write(json.dumps(record, sort_keys=True) + "\n")
            blocked_streak += 1
            print(f"  NO GAME — {record['blocked']}", flush=True)
            if blocked_streak >= len(BLOCKED_BACKOFF_S):
                print(f"{blocked_streak} starts in a row produced no game; stopping. "
                      f"{played}/{args.attempts} attempts were played. "
                      f"NO CONCLUSION IS AVAILABLE FROM THIS BATCH.", flush=True)
                return 3
            time.sleep(BLOCKED_BACKOFF_S[blocked_streak - 1])
            continue

        played += 1
        blocked_streak = 0
        record["attempt"] = played
        with LEDGER.open("a") as handle:
            handle.write(json.dumps(record, sort_keys=True) + "\n")

        print(f"  turn={record.get('last_turn')} score={record.get('last_score')} "
              f"rival_best={record.get('rival_best')} cities={record.get('cities')}",
              flush=True)
        if won(record):
            print(f"*** WON on attempt {played} ({tag}) ***", flush=True)
            return 0

    print(f"no win in {played} attempts played "
          f"({started - played} starts produced no game)", flush=True)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
