"""Play Civilization VI games back to back with CIVVIS deciding, until one is won.

The retired ladder walker (`civ6_climb.py`, in git history) climbed every rung but
never started a brain, so it could not host a CIVVIS-driven attempt. This does one
rung, repeatedly, with the decision loop attached, and keeps a ledger.

    python3 tools/civ6_civvis_climb.py --attempts 12

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
import math
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
DEFAULT_VICTORY = "science"
DEFAULT_STRATEGY = ""

sys.path.insert(0, str(HERE))
from civ6_control import gamelock, install, launcher  # noqa: E402
# The objective list is not restated here. This launcher's `--victory` is
# forwarded verbatim to `civ6_play.py --civvis-victory`, which forwards it
# verbatim to `civvis_orders --victory`; a second copy of the names is a second
# place for them to go stale, which is exactly how three of the six lanes stayed
# unreachable. `test_civ6_play.py` pins this list against the Rust const.
from civ6_play import VICTORY_LANES  # noqa: E402

# Backoff between blocked starts. The first steps are short because the usual cause
# is a Steam client that is coming back up on its own; the last is long because if
# it is still down after four minutes a human has to do something, and a loop
# polling every fifteen seconds for an hour teaches nobody anything.
BLOCKED_BACKOFF_S = (15.0, 30.0, 60.0, 120.0, 240.0)


def heal_the_ladder() -> None:
    """Record any summary the live ledger is missing, before the next attempt.

    ★★★★★ RECORDING IS BEST-EFFORT AND NOTHING WAS COMING BACK FOR THE MISSES.
    `civ6_play.py` deliberately swallows a ladder-recording failure — a
    finished game must never be lost to a bookkeeping error, and the summary
    on disk is the evidence. That trade is only sound if something routinely
    replays what was skipped, and until now nothing did: on 2026-08-17 the
    runs directory held **41 summaries the live ledger had never seen**,
    spanning three days, and one of them was the project's first Settler
    victory (`civvis-20260816T054344Z`, turn 251, score 1022).
    `civ6_ladder.py check` had been reporting it the whole time, to nobody.

    So the backfill runs here, at the top of every attempt: the one place in
    the fleet that is guaranteed to execute between games, on the machine that
    owns the ledger. It is idempotent and takes milliseconds, and a failure
    must not cost an attempt — the record is worth less than the game.
    """
    try:
        import civ6_ladder  # noqa: PLC0415 — optional, and only needed here
        civ6_ladder.sync(RUN_ROOT, civ6_ladder.live_ledger_for(RUN_ROOT),
                         quiet=True)
    except Exception as exc:  # noqa: BLE001 — see above: never cost an attempt
        print(f"ladder backfill skipped: {exc}", flush=True)


def stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def run(cmd: list[str], timeout: float = 60.0) -> str:
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return (proc.stdout or "") + (proc.stderr or "")
    except (subprocess.SubprocessError, OSError) as exc:
        return f"error: {exc}"


def refresh_orders_binary(orders_bin: Path, enabled: bool = True) -> str:
    """Rebuild the decider when it is this checkout's own release build.

    ★★★★ EVERY GAME STARTED ON A STALE PROGRAM. `civ6_play` hands the brain
    `target/release/civvis_orders` from this checkout, and the brain only hot-swaps
    to a published build NEWER than the checkout's HEAD at launch — which first has
    to be compiled, tens of turns into the game. So a checkout that was pulled but
    not rebuilt plays its first turns on whatever `cargo build --release` last
    produced here: on 2026-08-16 that binary was six hours and four merged repairs
    behind HEAD, run civvis-20260816T070212Z was stamped `code=21969cab` yet
    reported `unmapped: schema:state.dvp` and fired a branch two of those repairs
    had removed, until its first handoff at turn 101.

    An incremental release build of an unchanged tree costs a second; a changed one
    costs the minutes the game would otherwise spend on the wrong program. Only the
    default path is rebuilt — a binary the operator supplied is theirs to manage.
    """
    if not enabled:
        return "decider: --no-build, using the binary as it is"
    default = (HERE.parent / "target" / "release" / "civvis_orders").resolve()
    try:
        supplied = orders_bin.resolve()
    except OSError:
        return "decider: unresolvable path; using it as it is"
    if supplied != default:
        return "decider: supplied binary; not rebuilt"
    try:
        proc = subprocess.run(
            ["cargo", "build", "--release", "--locked", "--bin", "civvis_orders"],
            cwd=str(HERE.parent), capture_output=True, text=True, timeout=1800.0,
        )
    except (subprocess.SubprocessError, OSError) as exc:
        return f"decider: rebuild failed to run ({exc}); using the binary as it is"
    if proc.returncode != 0:
        tail = ((proc.stderr or proc.stdout or "").strip().splitlines() or ["no output"])[-1]
        return f"decider: rebuild FAILED ({tail}); using the binary as it is"
    return f"decider: release build current for {code_state()}"


def dismiss_crash_dialogs() -> None:
    """Close the "quit unexpectedly" / "Game configuration unavailable" dialogs.

    ⚠ These are not cosmetic. Civilization VI segfaults (see the governor-appointment
    note) and every teardown `pkill`s it, so Steam and macOS both leave modal
    ⚠⚠ THE PROCESS IS "Problem Reporter", NOT "ReportCrash", AND THAT ONE WORD
    COST A BATCH. `ReportCrash` is the background daemon; it has no windows. The
    dialog a human actually sees — titled "Problem Report for Civilization VI",
    with buttons Hide Details / OK / Reopen — belongs to a process literally named
    `Problem Reporter`. This list named only the daemon, so it walked windows that
    never exist and the real modal was never closed.

    Measured 2026-08-02: four Civ 6 segfaults left a Problem Reporter window up.
    Every later attempt then reported "NO GAME — could not start a game from the
    main menu", because the modal was taking the click the Create Game vision pass
    was about to make. `pgrep -lf "Problem Reporter"` is how to confirm it.

    dialogs behind. A modal left on screen steals the click the NEXT attempt's
    vision pass is about to make on the Create Game screen — and this project has
    already had a stray click land on "Exit to Desktop".

    Best effort by design: it must never raise, and it must never click anything
    that is not a dialog button it can name.
    """
    script = """
    tell application "System Events"
        repeat with procName in {"Steam", "Civilization VI", "Civ6", "ReportCrash", "Problem Reporter"}
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


def installed_run_tag() -> str | None:
    """Read the installed control mod's tag without changing the installation.

    A tag is the one identity shared by the game, its `civ6_play` controller, and
    the control mod.  Treat an absent or unreadable tag as *not ours*: cleanup is
    allowed to leave an orphan for an operator, but never to stop an unproven game.
    """
    config = install.install_dir() / "config.json"
    if not config.is_file():
        return None
    try:
        tag = json.loads(config.read_text()).get("RunTag")
    except (OSError, json.JSONDecodeError) as exc:
        print(f"[teardown] could not read the installed run tag: {exc}", flush=True)
        return None
    return tag if isinstance(tag, str) and tag else None


def teardown(expected_tag: str | None = None) -> bool:
    """Stop only the completed run named by ``expected_tag``.

    The old no-argument implementation killed every `civ6_play`, `civ6_brain`, and
    Civilization VI process it could find.  An attempt that failed before creating a
    game could therefore race a supervisor's next run and close that foreign game.
    A run tag alone is not enough to establish ownership, so this acquires the shared
    game lock before touching the game and rechecks the installed tag immediately
    before the global stop.
    """
    if expected_tag is None:
        if busy():
            print("[teardown] refusing to stop an unowned running game", flush=True)
        return False

    actual_tag = installed_run_tag()
    if actual_tag != expected_tag:
        found = repr(actual_tag) if actual_tag is not None else "no readable tag"
        print(f"[teardown] refusing to stop foreign run {found}; expected "
              f"{expected_tag!r}", flush=True)
        return False

    # A live `civ6_play` keeps this lock until its own finally block has stopped the
    # game.  If it still holds it, leave that controller to finish rather than
    # terminating it from outside.  If it died, acquire() clears its stale lock and
    # prevents a new controller from starting between our ownership check and stop.
    if not gamelock.acquire(expected_tag):
        print(f"[teardown] {expected_tag!r} still has a live controller; leaving "
              "cleanup to it", flush=True)
        return False

    try:
        if installed_run_tag() != expected_tag:
            print(f"[teardown] run tag changed while acquiring cleanup ownership; "
                  f"leaving {expected_tag!r} alone", flush=True)
            return False

        escaped_tag = re.escape(expected_tag)
        # A dead play process can leave its brain behind, but it is safe to sweep
        # only a process whose command line names this exact run.  Never match all
        # brains/players again: another checkout may be driving the same install.
        for pattern in (rf"civ6_brain\.py.*{escaped_tag}",
                        rf"civ6_play\.py.*--tag {escaped_tag}"):
            run(["pkill", "-f", pattern])
        time.sleep(1)

        # launcher.stop is deliberately global, so put the second ownership check
        # immediately beside it while this harness holds gamelock.  A managed foreign
        # controller cannot start between these lines; an unexpected tag change is a
        # hard refusal, not an excuse to clear or quit somebody else's game.
        if busy():
            if installed_run_tag() != expected_tag:
                print(f"[teardown] refusing to stop a run that changed from "
                      f"{expected_tag!r}", flush=True)
                return False
            if not launcher.stop(timeout_s=45.0):
                print("[teardown] the owned game is STILL running after stop(); "
                      "preserving its run tag", flush=True)
                return False

        # Do not erase the identity while a process remains.  Besides making the
        # next attempt's diagnostics honest, that keeps a still-running game guarded
        # if stop() ever reports success before a late child exits.
        if busy():
            print("[teardown] an owned process is still running; preserving its "
                  "run tag", flush=True)
            return False
        if installed_run_tag() == expected_tag and install.clear_run_tag():
            print("[teardown] cleared the installed run tag", flush=True)
        dismiss_crash_dialogs()
        return True
    finally:
        gamelock.release()


def busy() -> str | None:
    out = run(["pgrep", "-f", "Civ6_Exe|civ6_play.py|civ6_brain.py"])
    return out.strip() or None


def _detach(cmd: list[str], log_path: Path, what: str) -> None:
    """Start a helper that must outlive this batch's process group.

    `start_new_session` is the load-bearing argument. Without it the child joins
    this batch's group and dies with it — which is exactly how the fifteen-turn
    mirror hole described in `ensure_mirror` was opened.

    Never fatal. A batch that cannot raise its helpers is still a batch that
    measures play, and refusing to start one because a window is missing would
    trade the measurement for the picture.
    """
    try:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        with log_path.open("a") as handle:
            subprocess.Popen(
                cmd, stdout=handle, stderr=subprocess.STDOUT,
                stdin=subprocess.DEVNULL, start_new_session=True,
            )
        print(f"[{what}] started; it logs to {log_path}", flush=True)
    except OSError as exc:
        print(f"[{what}] could not start: {exc}", flush=True)


def ensure_popup_clear() -> None:
    """Back the mod's autoclose shim with the out-of-game clearer.

    ⚠ **AUTOCLOSE GIVES UP PERMANENTLY.** The shim calls `ClearUpdate` on its
    twentieth failed attempt at a screen, which kills the CONTEXT — so a screen it
    cannot close stays on the map for the rest of the game and no later turn
    retries it. `popup_clear.py` is the backstop written for precisely that, and
    nothing has ever started it.

    Measured 2026-08-02 on run civvis-20260802T014139Z: the shim closed 167 popups
    across eight screens and reported `autoclose_stuck` twice — `DiplomacyActionView`
    and `WorldCongressPopup`. The Diplomacy one was a Barbarossa leader scene, and it
    sat over the whole game window until this was started by hand, at which point it
    cleared in one held click and caught a second scene fifteen seconds later.

    ⚠ A leader asking a QUESTION ignores every in-Lua rung and ignores Escape too —
    a question needs an ANSWER, so only a held click on the dialogue button resolves
    it. That is the case the mod structurally cannot reach.

    2.5s rather than the 6s default: the operator watches this window, and six
    seconds of a leader portrait over the map is the difference they notice. The
    tool's own guards make a tight interval safe — it refuses to click unless
    Civilization VI is frontmost AND the map is positively covered AND the harness
    has already recorded a turn, so it cannot touch the setup screens.
    """
    # ⚠ AN INHERITED CLEARER RUNS THE CODE OF WHATEVER BATCH STARTED IT.
    # Measured 2026-08-15: one clearer started 2026-08-14T22:46 guarded twelve
    # later batches with day-old code, so two shipped leader-scene stall fixes
    # (#1595, #1631) never reached a live game and the outer watchdog kept
    # spending attempts on screens the current build already handles. Same
    # invariant as `retire_mirror` above: a fresh batch retires the helper and
    # starts its own from this checkout.
    stale = [int(pid) for pid in run(["pgrep", "-f", "popup_clear.py"]).split()
             if pid.isdecimal()]
    if stale:
        print(f"[popups] retiring {len(stale)} inherited clearer(s) so this "
              "batch gets the current build", flush=True)
        for pid in stale:
            try:
                os.kill(pid, signal.SIGTERM)
            except (ProcessLookupError, PermissionError):
                pass
        deadline = time.monotonic() + 10.0
        while any(process_running(pid) for pid in stale) and time.monotonic() < deadline:
            time.sleep(0.25)
    clearer = HERE / "civ6_control" / "popup_clear.py"
    if not clearer.exists():
        print(f"[popups] no clearer at {clearer}; stuck screens will sit on the map",
              flush=True)
        return
    _detach(
        [sys.executable, "-u", str(clearer), "--interval", "2.5",
         "--runs", str(RUN_ROOT), "--log", str(RUN_ROOT.parent / "popup_clear.log")],
        RUN_ROOT.parent / "popup_clear.log", "popups",
    )


MIRROR_HOME = Path.home() / "civvis-civ6-mirror"
MIRROR_FOLLOW_LOG = MIRROR_HOME / "follow.log"
MIRROR_PORT = 8610
MIRROR_RETIRE_SECONDS = 8.0


def matching_pids(pattern: str) -> list[int]:
    """Return just the process ids that a narrow mirror pattern found."""
    return [int(pid) for pid in run(["pgrep", "-f", pattern]).split() if pid.isdecimal()]


def follower_output_path(pid: int) -> Path | None:
    """The file a follower owns as stdout, if `lsof` can prove one.

    A process name is deliberately not an ownership boundary: many CIVVIS
    worktrees run ``tools/follow.py``.  The live desktop follower is distinct
    because it writes beneath the one shared ``civvis-civ6-mirror`` runtime.
    """
    out = run(["lsof", "-a", "-p", str(pid), "-d", "1", "-Fn"])
    for line in out.splitlines():
        if line.startswith("n"):
            return Path(line[1:])
    return None


def follower_owns_mirror(pid: int) -> bool:
    """Whether this follower is one of the shared desktop mirror owners."""
    output = follower_output_path(pid)
    if output is None:
        return False
    try:
        output.relative_to(MIRROR_HOME)
    except ValueError:
        return False
    return True


def mirror_listener_pids() -> set[int]:
    """The processes that actually own the dedicated visible mirror port."""
    out = run(["lsof", f"-tiTCP:{MIRROR_PORT}", "-sTCP:LISTEN"])
    return {int(pid) for pid in out.split() if pid.isdecimal()}


def owned_mirror_pids() -> list[int]:
    """Return only helpers that own this machine's shared desktop mirror.

    A fresh batch must retire its old follower and detached server so they do
    not pair a new game with an old protocol or JavaScript build.  The old
    global name match also stopped unrelated worktrees' followers.  Followers
    are scoped by their dedicated runtime output, while servers are scoped by
    the dedicated port, which keeps the fresh-build invariant without crossing
    worktree boundaries.
    """
    pids = [
        pid for pid in matching_pids("tools/follow.py")
        if follower_owns_mirror(pid)
    ]
    listeners = mirror_listener_pids()
    for pid in matching_pids("civvis play --mirror"):
        if pid in listeners and pid not in pids:
            pids.append(pid)
    return pids


def process_running(pid: int) -> bool:
    """Whether a process we previously found has not exited yet."""
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        # It is still there; the normal signal path will name the permission
        # problem rather than treating a live stale mirror as gone.
        return True
    return True


def retire_mirror() -> list[int]:
    """Stop the old follower *and* its detached visible server.

    ``follow.py`` and ``civvis play --mirror`` both load their code at process
    start.  Keeping either one through a new verification batch can therefore
    pair today's Civ VI controller with yesterday's JavaScript and mirror
    protocol.  The server is in its own session, so stopping the follower alone
    deliberately does not stop it.
    """
    pids = owned_mirror_pids()
    if not pids:
        return []

    print(f"[mirror] retiring {len(pids)} inherited mirror helper(s) so this "
          "verification batch gets the current build", flush=True)
    for pid in pids:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        except PermissionError as exc:
            print(f"[mirror] could not stop stale helper {pid}: {exc}", flush=True)

    deadline = time.monotonic() + MIRROR_RETIRE_SECONDS
    remaining = [pid for pid in pids if process_running(pid)]
    while remaining and time.monotonic() < deadline:
        time.sleep(0.25)
        remaining = [pid for pid in remaining if process_running(pid)]
    if remaining:
        print("[mirror] stale helper(s) did not exit before the new follower "
              f"starts: {', '.join(str(pid) for pid in remaining)}", flush=True)
    return pids


def ensure_mirror() -> None:
    """Start the current-build follower before every verification batch.

    ⚠⚠ THE SERVER BEING UP IS NOT THE MIRROR BEING LIVE, and the two failure modes
    look identical from `/status`. `civvis play --serve` on :8610 reads a staged
    board out of `civvis-civ6-mirror/stage`; `tools/follow.py` is what rebuilds that
    stage from the running attempt's `events.jsonl`. The server is long-lived and
    reparented to init, so it answers `/status` cheerfully with LAST NIGHT'S BOARD
    while follow.py is dead — turn, frames_painted and frames_missed all stay frozen
    at whatever they were, which reads as a healthy idle mirror.

    Measured 2026-08-02 on run civvis-20260802T014139Z: follow.py exited, the game
    ran on to turn 97, and :8610 kept reporting `turn 82, frames_painted 82,
    frames_missed 0` — a fifteen-turn hole that no status field named. The operator
    saw a CIVVIS window that had simply stopped agreeing with the game.

    The batch used to accept any already-running `follow.py`. That process can be
    many revisions old: its Python code, its detached `civvis play --mirror`
    process, and Chrome's loaded JavaScript all survive a new build. A fresh
    verification game then visibly couples Firaxis to a blank or stale CIVVIS
    window. Retire that pair before the game begins and start the follower from
    this checkout, so the live mirror is always part of the same verified build.

    Deliberately not fatal. A batch that cannot raise the viewer is still a batch
    that measures play, and refusing to start one because a window is missing would
    trade the measurement for the picture.
    """
    follow = HERE / "follow.py"
    if not follow.exists():
        print(f"[mirror] no stager at {follow}; the window will not track the game",
              flush=True)
        return
    retire_mirror()
    _detach([sys.executable, "-u", str(follow)],
            MIRROR_FOLLOW_LOG, "mirror")


def commits_behind_main() -> int | None:
    """How many commits this tree is behind `origin/main`, or None if unknown.

    ⚠⚠ WHY THIS EXISTS. On 2026-08-02 the batch tree spent hours at `#868` while
    four merged fixes — including the two the session's whole diagnosis rested on —
    sat unbuilt. It had briefly reached `#880` and was moved BACK. Nothing in the log
    said so: `batch pinned to 1000a13` reads identically whether that commit is main
    or eighteen behind it, and the resulting batch measured an engine nobody meant to
    test.

    Pinning to an old commit is legitimate — it is how a controlled batch stays
    comparable — so this reports and never refuses. What it removes is the case where
    a tree is stale by ACCIDENT and the log looks exactly the same.

    ⚠ Does not fetch. A batch must not depend on the network, and a stale
    `origin/main` ref simply under-reports rather than blocking anything.
    """
    # ⚠ The module's own `run()`, not `subprocess.run` — it merges stdout and stderr
    # and never raises, which is what every other git call here relies on, and what
    # the tests fake.
    out = run(["git", "-C", str(HERE.parent), "rev-list", "--count",
               "HEAD..origin/main"]).strip()
    try:
        return int(out)
    except ValueError:
        # No such ref, not a repository, or git said something else entirely.
        return None


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


def settings_mismatch(asked: dict, dealt: dict) -> dict:
    """Which requested lobby settings the game did NOT deal.

    ⚠⚠ THIS LIVED IN TWO PLACES AND THE TEST OWNED THE SECOND COPY. `main` built the
    comparison inline and `test_civ6_civvis_climb` re-declared it "in the same
    shape" — so adding `leader` to the shipped tuple left the test asserting on the
    old one, green, and blind. Same failure as [[civvis-civ6-one-table-one-formatter]]:
    a rule with two spellings drifts, and the copy that drifts is the one nobody
    runs. One function, called by both.

    `asked` is keyed by ledger field name, `dealt` by the `seat` event's own key,
    because those two vocabularies genuinely differ (`map_size` vs `size`) and
    pretending otherwise is how the wrong field gets compared.
    """
    return {
        field: {"asked": want, "dealt": dealt.get(key)}
        for field, key, want in (
            ("difficulty", "difficulty", asked.get("difficulty")),
            ("map_size", "size", asked.get("map_size")),
            ("speed", "speed", asked.get("speed")),
            # The picker is OCR over a DLC-dependent list and `civ6_play` refuses to
            # start when it cannot verify the pick — but "refused to start" and
            # "started as someone else" are different failures, and only this line
            # can tell them apart after the fact.
            ("leader", "leader", asked.get("leader")),
        )
        # A falsy `want` is a deliberate "deal me anything" (`--leader ""`), or a
        # field this caller did not ask about. Neither can mismatch.
        # ⚠ Absent is not wrong either: an export naming nothing must not be
        # reported as the game having dealt the wrong rung.
        if want and dealt.get(key) is not None and dealt.get(key) != want
    }


def longest_idle_settler(events: Path) -> int:
    """The longest run of turns a settler sat still while it still had movement.

    ⚠⚠⚠ MEASURED ACROSS THE WHOLE ARCHIVE 2026-08-03: 54 of 142 runs of >=50 turns
    (38%) park a settler for >=15 consecutive turns at FULL movement, median streak
    37 turns, worst 143. It is invisible on every instrument the ladder already has.

    The tell is `moves > 0` with an UNCHANGED position. A settler that is BLOCKED
    spends its movement and fails; one that is never asked to move keeps it. So a
    high number here is not "the map was hard", it is "nobody told this unit
    anything" — on run `civvis-20260803T231038Z` the settler's last order was on
    turn 70 and it sat at full movement for the remaining 100+ turns while
    `desired_cities` was 7 and the empire finished with 4.

    ⚠ It also freezes PRODUCTION: `advanced_units` computes
    `decline_settlers = counts.settlers > 0`, so one idle settler stops CIVVIS
    building another. That run ordered no settler between turns 51 and 134.

    ⚠ And the journal reports it as working — "Settler marching to (6, 34) | 1 tiles
    away, the site is worth 152.4" — so `why.log` cannot be used to detect it and
    the applied-order rate never dips, because no order is issued to fail.
    """
    best, current = 0, {}
    previous: dict[int, tuple] = {}
    try:
        stream = events.read_text(errors="replace").splitlines()
    except OSError:
        return 0
    for line in stream:
        if '"state"' not in line:
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if event.get("kind") != "state":
            continue
        for unit in event.get("units") or []:
            if unit.get("kind") != "UNIT_SETTLER":
                continue
            uid = unit.get("id")
            here = (unit.get("x"), unit.get("y"))
            has_moves = (unit.get("moves") or 0) > 0
            if previous.get(uid) == here and has_moves:
                current[uid] = current.get(uid, 0) + 1
            else:
                current[uid] = 0
            previous[uid] = here
            best = max(best, current[uid])
    return best


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
                # The mod's terminal event is emitted by the game core BEFORE it
                # halts, so unlike the end-screen autoclose below it actually
                # arrives. See the note under `reached_end_screen`.
                ended_on_screen = event
            # ★★★★ A GAME THAT ENDED IS NOT A GAME THAT HUNG. Civilization VI's
            # end-game screen halts the game core, so the agent stops exporting and
            # the harness times out — and every such run was written down as
            # `stalled: no event for 240s` with a blank outcome. Attempt 2 was
            # photographed on the DEFEAT screen and recorded exactly that way.
            #
            # ⚠ This records that the game ENDED, never who won. `won()` still
            # demands the mod's victory event naming us.
            #
            # ⚠⚠⚠ AND IT WAS KEYED ON AN EVENT THAT HAS NEVER ONCE BEEN EMITTED.
            # Measured across every run on this machine:
            #
            #     250 runs, 145 `autoclose_armed` for EndGameMenu, ZERO `autoclose`
            #
            # `armed` fires when the context registers its handler, which happens at
            # game START — line 19 of a real run's `events.jsonl`. The `autoclose`
            # that this looked for cannot happen at all: Civilization VI halts the
            # Game Core when it shows the end-of-game screen, so the shim is ticking
            # off a frame loop that has stopped.
            #
            # So `reached_end_screen` read `None` on all 250 rows while its name
            # promised the opposite, and the paragraph above — a real finding —
            # described a column that could not deliver it. The evidence that DOES
            # exist is the mod's own terminal event, emitted by the game core before
            # it halts. Take that.
            #
            # ⚠ Deliberately ALSO counts a rival's victory: the question this
            # answers is "did the game reach its end", not "did we win". The
            # terminal event is captured in the `victory` branch above — this is
            # an `elif` chain, so setting it here as well would be unreachable.
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
        # A settler nobody ordered. See `longest_idle_settler` — 38% of runs
        # park one, and no other column in this row can show it.
        "idle_settler_turns": longest_idle_settler(events) or None,
        # ⚠ The civ is the largest single covariate on a row and it was only ever
        # reachable by digging into the nested `seat` blob, so no ledger query ever
        # grouped by it. Promote both to columns: `civ`/`leader` are what Civ 6
        # DEALT, and the caller records `leader_asked` beside them, so a row where
        # the picker missed reads as a mismatch instead of as a clean sample.
        "civ": (seat or {}).get("civ"),
        "leader": (seat or {}).get("leader"),
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


# How much lock time an attempt may be credited before it is abandoned.
LOCK_CREDIT_CAP_S = 7200.0


def screen_locked() -> bool:
    """Whether the macOS console session is locked.

    Same probe as `civ6_play.screen_locked`, duplicated rather than imported
    because this script runs `civ6_play` as a SUBPROCESS and does not import it.
    """
    try:
        result = subprocess.run(["ioreg", "-n", "Root", "-d1"],
                                capture_output=True, text=True, check=False)
    except OSError:
        return False
    return 'CGSSessionScreenIsLocked"=Yes' in result.stdout


def attempt_ceiling(args) -> float:
    """The hard wall-clock bound the child will actually honour.

    One number, derived once, so the child's ceiling and the outer watchdog
    cannot drift apart. They did: `civ6_play --timeout` stopped being a hard stop
    in #1532 and this watchdog stayed at `--timeout + 600`, which put a 6000 s
    kill underneath 8100 s of legitimate budget. Mirrors `civ6_play`'s own
    default so the two agree when neither is passed.
    """
    if args.timeout_ceiling is not None:
        return float(args.timeout_ceiling)
    return float(args.timeout) * 1.5


def wait_watching_the_turn(play, tag: str, hard_timeout_s: float,
                           frozen_s: float, locked_probe=None) -> str:
    """Wait for the attempt, and kill it if its TURN stops advancing.

    ★★★★★ AN IN-LOOP WATCHDOG CANNOT CATCH A BLOCKED HARNESS. `civ6_play` has
    `--frozen-seconds`, which watches the turn from inside its own poll loop —
    and on 2026-08-02 that did not save run civvis-20260802T064240Z. It wedged
    at turn 206 on `WorldCongressBetweenTurns` and sat there for over ten
    minutes with the flag armed: no rescue line, no `summary.json`, so `follow`
    had never returned. Replaying that run's own `events.jsonl` shows the in-loop
    logic would have fired, so the logic was right and simply never ran.

    ⚠ THE MOD APPENDS TO `events.jsonl` FROM INSIDE THE GAME, independently of
    whether the Python harness is executing. A file that keeps growing proves the
    GAME is alive, not the HARNESS. If `civ6_play`'s loop is blocked — in
    `on_event`, in `keep_foreground`, in an AppleScript that never returns — the
    code that would fire its watchdog is not running. Only a watcher in ANOTHER
    process can see that, and the climb is already that process.

    The attempt had to be cleared by hand. `play.wait(timeout=...)` would
    otherwise have burned its full hundred minutes.

    ⚠ Deliberately MORE PATIENT than the in-loop watchdog (480s). This is the
    last resort, it kills a run that may merely be slow, and the cheaper guard
    gets first refusal.

    ⚠ Arms only once a turn has been SEEN — setup emits none, and killing every
    attempt before it starts is the failure mode this guard must not have.
    """
    events = RUN_ROOT / tag / "events.jsonl"
    deadline = time.time() + hard_timeout_s
    last_turn, last_turn_at, offset = None, time.time(), 0
    # ⚠ CAP THE CREDIT. Pausing the clock while locked is right, but an
    # UNBOUNDED pause means a machine left locked overnight holds the game
    # forever and the attempt never resolves — the first version of this fix
    # spun exactly that way in its own test. Past the cap, give up and SAY it
    # was the lock, so the row is not filed as a mysterious timeout.
    locked_total, said_locked = 0.0, False
    lock_credit_cap = LOCK_CREDIT_CAP_S
    # ⚠ INJECTED, not read from the module. Reading the real screen made every
    # OTHER test in this file depend on whether the developer's machine happened
    # to be locked — and on a locked machine they spun forever, because each
    # locked slice extends the deadline it is racing.
    is_locked = locked_probe if locked_probe is not None else screen_locked
    while time.time() < deadline:
        slice_started = time.time()
        try:
            play.wait(timeout=20.0)
            return "exited"
        except subprocess.TimeoutExpired:
            pass
        # ⚠⚠ A LOCKED SCREEN IS NOT A STALLED GAME, AND BOTH CLOCKS READ IT AS ONE.
        #
        # `civ6_play` waits at the macOS authentication boundary rather than
        # scripting past it, which is correct — but neither timer here knew that.
        # `deadline` is wall clock, and `last_turn_at` cannot advance while the
        # game is not being driven, so a lock long enough to cross `frozen_s`
        # killed an attempt that was doing nothing wrong.
        #
        # Live: run `civvis-20260804T102440Z` reached **turn 82, 4 cities, score
        # 185** and was stopped by "attempt exceeded its own timeout" after the
        # session locked mid-game. The game was healthy; only the operator's
        # screen was not.
        #
        # So credit the locked interval back to BOTH timers. When the screen is
        # never locked — the normal case — this changes nothing at all.
        #
        # ⚠ `continue`, not merely a credit: while the screen is locked the turn
        # CANNOT advance, so freshness is unjudgeable and the frozen check below
        # must not run at all. Crediting the interval and falling through still
        # killed the attempt — the first version of this fix did exactly that,
        # and `test_a_locked_screen_does_not_kill_a_healthy_attempt` caught it.
        if is_locked():
            locked_for = time.time() - slice_started
            if locked_total + locked_for > lock_credit_cap:
                print(f"[watchdog] screen has been locked for "
                      f"{locked_total / 60.0:.0f} min, past the "
                      f"{lock_credit_cap / 60.0:.0f} min credit cap; giving up "
                      f"on this attempt", flush=True)
                play.send_signal(signal.SIGTERM)
                try:
                    play.wait(timeout=60.0)
                except subprocess.TimeoutExpired:
                    play.kill()
                return "locked"
            deadline += locked_for
            last_turn_at += locked_for
            locked_total += locked_for
            if not said_locked:
                print("[watchdog] screen is locked; the attempt clock is paused "
                      "until it unlocks", flush=True)
                said_locked = True
            continue
        if said_locked:
            print(f"[watchdog] screen unlocked; {locked_total / 60.0:.1f} min of "
                  f"lock time was credited back to the attempt", flush=True)
            said_locked = False
        # Read only what is new. The file reaches tens of megabytes on a long
        # run and this loop runs every twenty seconds for the whole attempt.
        try:
            with events.open("rb") as handle:
                handle.seek(offset)
                fresh = handle.read()
                offset = handle.tell()
        except OSError:
            continue
        for line in fresh.splitlines():
            try:
                turn = json.loads(line).get("turn")
            except (ValueError, AttributeError):
                continue
            if isinstance(turn, int) and (last_turn is None or turn > last_turn):
                last_turn, last_turn_at = turn, time.time()
        if last_turn is not None and time.time() - last_turn_at > frozen_s:
            print(f"[watchdog] turn {last_turn} has not advanced in "
                  f"{frozen_s:.0f}s and the harness has not noticed; killing the "
                  f"attempt from outside", flush=True)
            play.send_signal(signal.SIGTERM)
            try:
                play.wait(timeout=60.0)
            except subprocess.TimeoutExpired:
                play.kill()
            return "frozen"
    return "timeout"


# A game frozen before this turn is redone from scratch faster than it is
# reloaded (the load flow itself takes minutes and a fresh map is cheap early).
RESUME_MIN_TURN = 20


def resolvable_win_rate(attempts: int, baseline: float = 0.25,
                        alpha: float = 1.959963984540054,
                        beta: float = 0.8416212335729143) -> float | None:
    """The smallest win rate a batch of this size could tell apart from
    `baseline`, at 5% significance and 80% power, one arm each.

    ⚠⚠ A BATCH THAT REPORTS A RESULT WITHOUT ITS POWER IS HOW AN UNDERPOWERED
    NUMBER GETS QUOTED. The live ladder finishes roughly 24 games a day, and
    separating the measured 25% Settler win rate from 40% needs about 152
    games PER ARM — twelve days of the whole ladder for one paired
    comparison. Eight games settle a MECHANISM (did the treatment fire, did
    the pin hold); they settle no effect at all, and the difference has to be
    printed where the operator reads the batch, not left to be worked out
    later from a table nobody opens.

    Returns None when no rate below certainty is separable at this size.
    """
    if attempts < 1:
        return None
    for step in range(1, 1000):
        rate = baseline + step / 1000.0
        if rate >= 1.0:
            return None
        pooled = (baseline + rate) / 2
        need = ((alpha * math.sqrt(2 * pooled * (1 - pooled))
                 + beta * math.sqrt(baseline * (1 - baseline)
                                    + rate * (1 - rate)))
                / (rate - baseline)) ** 2
        if need <= attempts:
            return rate
    return None


def batch_power_line(attempts: int, baseline: float = 0.25) -> str:
    """One line naming what this batch can and cannot settle."""
    if attempts <= 1:
        return ("single attempt: a smoke test — one game distinguishes "
                "nothing (live score stdev ~398 on the current build)")
    resolvable = resolvable_win_rate(attempts, baseline)
    if resolvable is None:
        return (f"batch of {attempts}: too small to separate any win rate "
                f"from {baseline:.0%} — a mechanism check, not a measurement")
    return (f"batch of {attempts}: at 80% power this separates "
            f"{baseline:.0%} from {resolvable:.0%} or better, and nothing "
            f"finer — smaller effects need more games PER ARM")


def batch_refresh_seconds(refresh_seconds: float | None, pinned: str | None,
                          attempts: int) -> float | None:
    """The decider-refresh cadence a batch should play under.

    An operator's explicit choice always stands. Otherwise a pinned batch of
    more than one attempt freezes the decider (0): the batch exists to put N
    games on ONE program, and the brain's turn-boundary self-upgrade would
    re-point every game at whatever origin/main had become — the ledger
    stamped `code=3658227b` on a run whose decider walked through four
    revisions. A single attempt keeps the brain's live-upgrade default: the
    ambient loop's whole design is that a merge reaches the very next game.
    """
    if refresh_seconds is not None:
        return refresh_seconds
    if pinned is not None and attempts > 1:
        return 0.0
    return None


def play_command(args, tag: str, orders_db: Path, orders_bin: Path,
                 load_save: Path | None = None) -> list[str]:
    """The `civ6_play` command line for one attempt — or its continuation.

    One builder for both, so a resumed game is driven by exactly the flags the
    original was: the ONLY differences are the run tag (`<tag>-contN`) and
    `--load-save`, which swaps the Create Game flow for the Load Game flow.
    """
    return (
        [sys.executable, "-u", str(HERE / "civ6_play.py"),
         "--tag", tag,
         "--orders-db", str(orders_db),
         "--difficulty", args.difficulty,
         "--map-size", args.map_size,
         "--speed", args.speed]
        + (["--leader", args.leader] if args.leader else [])
        + (["--load-save", str(load_save)] if load_save is not None else [])
        + [
         "--max-turns", str(args.max_turns),
         "--timeout", str(args.timeout),
         "--timeout-ceiling", str(attempt_ceiling(args)),
         "--lock-wait", "30",
         "--report-every", "10",
         "--export-state"]
        + (["--probe-citizens"] if args.probe_citizens else [])
        + (["--campus-specialist"] if args.campus_specialist else [])
        + (["--envoys"] if args.envoys else [])
        + (["--no-envoy-place"] if args.envoys and not args.envoy_place else [])
        + (["--no-envoy-levy"] if args.envoys and not args.envoy_levy else [])
        + (["--no-envoy-consider"] if args.envoys and not args.envoy_consider else [])
        + [
         "--announcement-seconds", "0.05",
         "--era-announcement-seconds", "0.05",
         "--civvis-decides",
         "--civvis-bin", str(orders_bin),
         "--civvis-victory", args.victory,
         "--civvis-strategy", args.strategy]
        + (["--civvis-refresh-seconds", str(args.refresh_seconds)]
           if args.refresh_seconds is not None else [])
        + (["--no-peace-deterrence"] if args.no_peace_deterrence else [])
        + (["--no-counter-resolutions"] if args.no_counter_resolutions else [])
        + [flag for treatment in args.without
           for flag in ("--civvis-without", treatment)]
        + (["--civvis-war-from-plan"] if args.war_from_plan else [])
        + [
         "--tile-export-every", str(args.tile_export_every),
         "--window-side", "right",
         "--window-frac", "0.5", "--window-vfrac", "0.5"]
    )


def resume_from_autosave(record: dict, why: str | None, resumes_so_far: int, args,
                         started_at: float, latest=None) -> Path | None:
    """The autosave a frozen attempt should be reloaded from, or None.

    ★★★★★ A FROZEN GAME WAS SCORED AS A LOSS WITH ITS SAVE ON DISK. Three
    leading games died on the 900 s watchdog on 2026-08-16 alone:
    `civvis-20260816T115139Z` at t178 (804 vs 715, leading), `T175306Z` at
    t207 (731 vs 958) and `T192522Z` at t102 (340 vs 324, leading, the lane's
    first-ever capture) — a late first-contact scene, then the same, then a
    plain map reading "PLEASE WAIT" with the game core spinning on the other
    players' turns. Each was killed from outside and written into the ledger
    with its last score, and each had `AutoSave_NNNN.Civ6Save`, one per turn,
    sitting in Firaxis's autosave folder — the exact turn it froze on. The
    engine-side hang is not ours to fix; the reload is. `civ6_play
    --load-save` already drives the Load Game screen (and since this change
    ticks the Autosaves filter the list hides them behind).

    Resumes only what is worth resuming: the attempt was killed as `frozen`
    (a timeout or a locked screen is a different story), reached
    `--resume-min-turn`, did not already reach an end screen, the resume
    budget is not spent, and an autosave written since the attempt began
    exists (never one from an earlier game). Everything else falls through to
    the ledger exactly as before.
    """
    if why != "frozen" or resumes_so_far >= args.max_resumes:
        return None
    turn = record.get("last_turn")
    if not isinstance(turn, int) or turn < args.resume_min_turn:
        return None
    if record.get("end_screen_turn") is not None:
        return None
    finder = latest if latest is not None else _latest_autosave
    return finder(newer_than=started_at)


def _latest_autosave(newer_than: float | None = None) -> Path | None:
    import civ6_play  # noqa: PLC0415 — the play harness owns the save folder
    return civ6_play.latest_autosave(newer_than=newer_than)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--attempts", type=int, default=10)
    ap.add_argument("--without", action="append", default=[], metavar="TREATMENT",
                    help="withhold one live treatment for every attempt in "
                         "this batch — the control half of a live A/B. Pair "
                         "with --attempts N on a pinned revision, then run the "
                         "same N without this flag for the treatment half")
    ap.add_argument("--refresh-seconds", type=float, default=None,
                    help="forwarded to civ6_play as --civvis-refresh-seconds; "
                         "defaults to 0 for a pinned batch of more than one "
                         "attempt and to the brain's live-upgrade cadence "
                         "otherwise (see batch_refresh_seconds)")
    ap.add_argument("--no-counter-resolutions", action="store_true", default=False,
                    help="withhold counter-resolution targeting for this batch "
                         "— the control half of its live A/B")
    ap.add_argument("--no-peace-deterrence", action="store_true", default=False,
                    help="withhold the fallback ladder's peacetime deterrence "
                         "for this batch — the control arm of the Lua army "
                         "target's met-major lift")
    ap.add_argument("--campus-specialist", action="store_true", default=False,
                    help="move one citizen into a Campus specialist slot where a Library "
                         "already stands; read the outcome from civvis_campus_specialist")
    ap.add_argument("--probe-citizens", action="store_true", default=False,
                    help="ask once per batch whether this UI context may assign a "
                         "citizen to a district; read-only, issues no command")
    # ⚠ OFF BY DEFAULT AND IT MUST STAY OFF until the SIGSEGVs are cleared. This
    # adds the missing LINK, not the decision to use it: `civ6_play.py` has taken
    # `--envoys` all along, this loop builds a fixed argument list, and so the
    # flag could not reach a live game from here at all. That is the same
    # four-link failure the `--probe-citizens` comment below records for #1098.
    ap.add_argument("--envoys", action="store_true", default=False,
                    help="enable the envoy lane (cfg.EnvoyEnabled) for ONE isolated "
                         "run; OFF by default because chooseEnvoy has an unresolved "
                         "SIGSEGV history — do not use in a deployment batch")
    # `civ6_play.py` keeps each mutation independently switchable.  The harness
    # must preserve that boundary: `--envoys` alone remains the historical
    # all-on experiment, while a `--no-envoy-*` flag makes a one-variable run
    # possible without editing the installed mod.
    ap.add_argument("--envoy-place", action=argparse.BooleanOptionalAction, default=True,
                    help="allow influence-token placement when --envoys is enabled; "
                         "use --no-envoy-place to isolate another envoy mutation")
    ap.add_argument("--envoy-levy", action=argparse.BooleanOptionalAction, default=True,
                    help="allow city-state levies when --envoys is enabled; use "
                         "--no-envoy-levy for an isolated run")
    ap.add_argument("--envoy-consider", action=argparse.BooleanOptionalAction,
                    default=True,
                    help="allow prompt-clearing when --envoys is enabled; use "
                         "--no-envoy-consider for an isolated run")
    ap.add_argument("--difficulty", default="DIFFICULTY_SETTLER")
    # Six players, because the size IS the player count — see `civ6_play.py`.
    ap.add_argument("--map-size", default="MAPSIZE_SMALL")
    ap.add_argument("--speed", default="GAMESPEED_ONLINE")
    ap.add_argument("--max-turns", type=int, default=250)
    # ⚠⚠⚠ THE SEAT WAS RANDOM FOR 190 RUNS, AND NOTHING SAID SO.
    #
    # `civ6_play.py` has taken `--leader` (and verifies the pick off the rendered
    # picker) for as long as this script has existed, and this script never passed
    # it. A census of every `seat` event under `civvis-civ6-runs/control`:
    #
    #     190 runs, 49 distinct civilizations
    #     Trajan   4 / 190  (2.1%)      Rome  5 / 190  (2.6%)
    #     modal seat: Philip II of Spain, 8
    #
    # Two things were silently lost. The operator's standing brief is Rome and
    # Trajan, and the ladder was answering a different question 97.9% of the time.
    # And `civ6_brain.seat_civ` narrows `--strategy auto` by the civ Civ 6 DEALT —
    # `--civ Rome` picks `g56-48` at a 0.510 bound where the overall fallback is
    # `adv-religious` at 0.410 — so the narrowing that exists could almost never
    # fire, because the league rates only four civs and the deal was rarely one.
    #
    # ⚠ The civ effect is large enough to swamp a code change ([[civvis-rating-and-fleet]]
    # measured Rome at 37.7% of league wins against Sumeria's 14.6%), so 190 rows
    # dealt 49 different civs are not one experiment; pinning the seat is what makes
    # consecutive rows comparable at all. Rows recorded before this change carry a
    # random seat and cannot be pooled with rows recorded after it.
    #
    # Pass `--leader ""` to restore the old random deal deliberately.
    ap.add_argument("--leader", default="LEADER_TRAJAN",
                    help="Firaxis leader type to select and verify on the picker; "
                         "empty string takes whatever the lobby deals")
    ap.add_argument("--timeout", type=float, default=5400.0)
    # ⚠⚠⚠ THE OUTER WATCHDOG MUST SIT ABOVE THE INNER CEILING, NOT ABOVE
    # `--timeout`, AND FOR TEN DAYS IT DID BOTH BECAUSE THEY WERE THE SAME
    # NUMBER.
    #
    # `civ6_play --timeout` used to be a hard stop, so `timeout + 600` was
    # comfortably the last resort. #1532 made that budget EXTEND while a run can
    # still reach `--max-turns`, up to `--timeout-ceiling` (1.5x by default) --
    # and left this watchdog where it was. 5400 * 1.5 = 8100 s of legitimate
    # inner budget underneath a 6000 s outer kill.
    #
    # So the backstop started firing first, on healthy games. Run
    # civvis-20260811T115348Z was stopped at **turn 194 of 250, score 490, still
    # advancing**, exactly 100 min 41 s after it started -- `timeout + 600` to
    # the second. And because the outer path SIGTERMs the child, the run never
    # writes its summary: the ladder keeps a row carrying `last_turn` and
    # `last_score` with NO `reason`, which reads like a finished game to anything
    # that does not check for the missing field.
    #
    # The two budgets have to be derived from one number. This is passed to the
    # child as `--timeout-ceiling` and the watchdog is set above it, so raising
    # either one cannot silently invert them again.
    ap.add_argument("--timeout-ceiling", type=float, default=None,
                    help="hard wall-clock bound handed to civ6_play as "
                         "--timeout-ceiling; the outer watchdog is set above it. "
                         "Defaults to 1.5x --timeout, matching civ6_play's own "
                         "default. Pass a value equal to --timeout to restore a "
                         "non-extending budget.")
    # ⚠ The LAST resort, not the first. `civ6_play --frozen-seconds` (480s)
    # watches the same thing from inside its own loop and gets first refusal;
    # this one exists for when that loop is itself blocked and therefore cannot
    # fire. More patient for exactly that reason.
    ap.add_argument("--frozen-seconds", type=float, default=900.0,
                    help="kill an attempt whose TURN has not advanced for this "
                         "long, from outside its harness")
    # ★★★★ A FROZEN GAME IS RESUMED, NOT SCORED. See `resume_from_autosave`:
    # three leading games died on the clock in one day, each with a
    # turn-fresh autosave on disk.
    ap.add_argument("--max-resumes", type=int, default=2,
                    help="how many times a frozen attempt is reloaded from its "
                         "latest autosave under <tag>-contN before it is scored "
                         "as it stands (0 disables)")
    ap.add_argument("--resume-min-turn", type=int, default=RESUME_MIN_TURN,
                    help="a game frozen before this turn is not worth reloading")
    # ⚠⚠ THE DEFAULT WAS `domination`, AND THE COMMENT SAYING SO WAS ALREADY HERE.
    #
    # "A ladder left on this default measures a plan that cannot complete" was
    # written on 2026-07-31 against a different mechanism — `findWarTarget` needs a
    # REVEALED rival city, and meeting a civilization reveals none of its land, so
    # the target set stayed empty (`met: 1`, `cities_SEEN: 0` at t125, zero
    # declarations in every unforced run). The diagnosis landed; the default did
    # not move. All 104 rows of `civvis_ladder.jsonl` then carried `domination`
    # with ZERO wins, and the 21 `victory` events in them are RIVALS winning.
    #
    # ★★★★★ It is also out of budget, independently of that mechanism.
    # `victory_eval`'s own per-target turn limits are 650 for Domination and 300
    # for Score; this ladder runs `--max-turns 250`. Measured at exactly those
    # settings (`victory_eval --target domination,score --games 8 --players 6
    # --turns 250`): eight of eight domination-targeted games ended by score at the
    # turn limit, none by conquest. Score is the only lane whose budget is near 250.
    #
    # ★★★★★ NATIVE STRENGTH SAYS NOT TO PIN A LANE, BUT THE FIRAXIS BRIEF NOW
    # EXPLICITLY EXCLUDES THE LANE THAT MAKES ADAPTIVE WIN.
    #
    # Mirrored head-to-head at this ladder's own profile, 30 map pairs, 6 players,
    # 250 turns, using the arms added in #857:
    #
    #     score-target   vs domination-target   68.3% / 70.0%   +147 Elo  CONFIRMED
    #     adaptive       vs score-target        98.3%           -708 for score
    #     adaptive       vs domination-target   100.0%  60-0    p=0.0000
    #
    # So the ordering is **adaptive >> score > domination**, and the deployed
    # setting is the worst of the three: untargeted `advanced` beat it 60-0, with
    # `advanced_target_domination` recording ZERO victories of any type in 60 games.
    #
    # ⚠ THE REASONING THAT MOTIVATED PINNING A LANE AT ALL DOES NOT HOLD. The note
    # on `--victory` in civvis_orders.rs says that left to itself the agent "picked
    # `religion` with `victory=None`, unreachable in 250 turns". Untargeted
    # `advanced` wins 48 of its 60 BY RELIGION at a 250-turn cap in the run above.
    # Whatever made religion look unreachable, it was not the turn budget.
    #
    # `civvis` is the value that restores letting the agent choose; it maps to a
    # plain `AdvancedAi::new()` with no `VictoryTarget`, which is the `advanced`
    # controller both evaluations above were run against. It stays available.
    # But 48 of its 60 native wins at this exact profile were Religious Victory,
    # so it does not answer a brief that excludes religion. Of the two requested
    # lanes, Science is the viable first attempt: it needs no contact, whereas
    # Domination has both the revealed-city blocker and the 250-turn budget failure
    # above. This pins OUR plan only; rivals retain every Firaxis victory condition.
    #
    # ⚠ `domination`, `score` and `civvis` stay available and unchanged; only the
    # default moves. Rows either side of this change are NOT comparable.
    #
    # ★★★★★ CULTURE, RELIGIOUS AND DIPLOMATIC WERE MISSING FROM THIS LIST, and
    # the reasoning above was written as though the choice were between the four
    # that were here. It was not a menu: `advanced.rs` gates each lane's
    # machinery on being TARGETED at it, and the gate is NEGATIVE — a targeted
    # agent aiming anywhere else prices great-work buildings
    # (`advanced.rs:18365`) and Missionaries (`advanced.rs:18115`) at -10_000 and
    # abstains from non-emergency World Congress ballots
    # (`advanced.rs:12830`). So every ladder attempt run at this default has been
    # playing with three of Civilization VI's five victory conditions switched
    # off in its own production valuation. All six of `VictoryTarget`'s variants
    # are selectable now; the names are the enum's own `as_str` spellings and
    # `civvis_orders` parses them through its `FromStr`, so the two lists cannot
    # drift.
    #
    # ⚠ The default does NOT move with this change. Science stays the default
    # until a lane is measured to beat it; rows are still separated by
    # `code_rev`, and an attempt that passes `--victory culture` is a different
    # experiment from the rows above it.
    ap.add_argument("--victory", default=DEFAULT_VICTORY,
                    choices=VICTORY_LANES,
                    help="victory objective; defaults to Science. Every "
                         "VictoryTarget the engine implements is selectable; "
                         "`civvis` lets the agent choose its own")
    # The rated genome is an opt-in experiment. Its internal league strength does
    # not establish Firaxis transfer, while stock AdvancedAi has the powered
    # deployment-shaped result and makes concurrent controller work attributable.
    ap.add_argument("--strategy", default=DEFAULT_STRATEGY,
                    help="strategy the decider loads; empty keeps stock AdvancedAi. "
                         "`auto` is an uncalibrated opt-in")
    # ★★★★★ ON BY DEFAULT since 2026-08-03, on the operator's call.
    #
    # 96 of 123 corpus runs reaching turn 50 NEVER DECLARED WAR, and the corpus
    # holds only 58 `cannot_declare` refusals — all of them inside two runs. So
    # the declaration was not being refused, it was not being ATTEMPTED. CIVVIS's
    # own diplomacy wants a casus belli or a denouncement matured over five
    # turns, and NOTHING matures on a board that `--fresh-board` rebuilds every
    # turn, so the decline is an artefact of the reconstruction rather than a
    # judgement about the war (see the `Decider` docstring in civ6_brain.py).
    #
    # ⚠ The default lives HERE rather than in ~/civvis-batch-loop.sh because the
    # supervisor is a long-running zsh process that holds its script's inode for
    # its whole life: editing that file — even atomically — does not reach the
    # running loop, and a batch launched 18 minutes after such an edit still ran
    # the old command line. The runner tree re-checkouts `origin/main` between
    # batches, so a default that ships in the repo DOES take effect without
    # restarting anything.
    #
    # ⚠⚠ This changes what the ladder measures. Rows are separated by
    # `code_rev`, but the code_rev boundary at this commit also carries a
    # configuration change — do not read a before/after difference as a code
    # effect.
    # ⚠⚠ DEFAULT FLIPPED TO OFF 2026-08-03, and the flip is the whole point: this
    # defaulted to TRUE, so every live batch carried an override that `civ6_play`
    # refuses outright as replay-only. It only worked because this script ran its own
    # decider; with one decider it cannot work, and defaulting to a value that always
    # aborts the batch would be a worse failure than the one being fixed. Passing it
    # explicitly is refused, loudly, by the check at the top of `main`.
    ap.add_argument("--war-from-plan", action=argparse.BooleanOptionalAction,
                    default=False,
                    help="REFUSED for live games; see the guard in civ6_play.main. "
                         "Kept so a batch that asks for it fails instead of quietly "
                         "measuring something else")
    ap.add_argument("--tile-export-every", type=int, default=4,
                    help="turns between map exports; the operator watches this against the game")
    ap.add_argument("--orders-bin", default=str(HERE.parent / "target" / "release" / "civvis_orders"))
    ap.add_argument("--no-build", dest="build", action="store_false", default=True,
                    help="do not rebuild the checkout's release decider before attempts")
    ap.add_argument("--logs", default=None, help="directory for per-attempt logs")
    ap.add_argument("--no-pin", action="store_true", default=False,
                    help="allow the code to change mid-batch; rows stop being "
                         "comparable and the ledger can only say so afterwards")
    args = ap.parse_args()

    logs = Path(args.logs).expanduser() if args.logs else Path.cwd() / "civvis-climb-logs"
    logs.mkdir(parents=True, exist_ok=True)
    # ⚠⚠⚠ THE SECOND DECIDER WAS ROUTING AROUND A DELIBERATE SAFETY GUARD.
    #
    # `civ6_play.main` refuses `--civvis-war-from-plan` outright, and its comment
    # gives the evidence: the override turns a plan's preferred rival into an
    # immediate declaration even when the planner DECLINED war, and live run
    # `live-loop-rome-20260802-0800` forced that declaration under a Religion plan
    # on turn 37, spent the remaining 213 turns in Recovery asking for peace, and
    # finished 400-1081. "A production launcher must not be able to bypass the
    # decider whose behavior it claims to measure."
    #
    # This script bypassed it anyway — not on purpose, but because it started its
    # OWN `civ6_brain.py`, which takes the flag directly and never sees that guard.
    # So `--war-from-plan`, enabled here on 2026-08-03, has been doing live exactly
    # what `civ6_play` forbids, and the guard has been reading as enforced.
    #
    # Now that there is one decider, the conflict cannot hide, and it is not this
    # script's to settle: one of the two deliberate decisions has to be withdrawn by
    # whoever owns them. Refusing is the honest failure — silently dropping the flag
    # would change what a batch measures without saying so, and silently keeping it
    # is what got us here.
    if args.war_from_plan:
        print("--war-from-plan is refused for live games by civ6_play.main, which "
              "calls it replay-only:\n"
              "  'the override turns a plan's preferred rival into an immediate "
              "declaration even when\n   the planner declined war' — live run "
              "live-loop-rome-20260802-0800 forced one under a\n   Religion plan on "
              "turn 37, spent 213 turns in Recovery, and finished 400-1081.\n"
              "This script used to bypass that guard by running a second decider. It "
              "no longer does.\n"
              "Run with --no-war-from-plan, or lift the guard in civ6_play.main "
              "deliberately.", file=sys.stderr)
        return 4

    orders_bin = Path(args.orders_bin)
    if not orders_bin.exists():
        print(f"no civvis-orders binary at {orders_bin}", file=sys.stderr)
        return 2

    if busy():
        print("something already holds the game; refusing to stop an unowned run",
              flush=True)
        return 3

    # ★★★★ GATE THE BATCH ON PREFLIGHT. `civ6_preflight.py` says in its own docstring
    # that "exit status is 0 only when every check passes, so this can gate a ladder"
    # -- and nothing called it. A gate nothing invokes is a gate that does not exist.
    #
    # Every check it runs corresponds to a defect that actually shipped, and the
    # newest one is the reason this wiring is worth the second it costs: a diagnostic
    # `println` on the decider's stdout shifts the whole --serve protocol by one line,
    # so CIVVIS decides correctly into a pipe nobody reads and the run silently falls
    # back to the hand-written ladder. That cost a live run 236 turns in.
    #
    # ⚠ `--skip-engine`: the ladder is being launched, not the test suite, and cargo
    # test here would add minutes to every batch. The engine is gated at merge.
    preflight = run([sys.executable, str(HERE / "civ6_preflight.py"), "--skip-engine",
                     "--orders-bin", str(orders_bin)], timeout=300.0)
    print(preflight.rstrip(), flush=True)
    if "PREFLIGHT FAILED" in preflight:
        print("refusing to start a batch on a broken bridge; fix the failures above",
              flush=True)
        return 4

    # A batch is a COMPARISON, so it is pinned to one program by default. Opting out
    # is a deliberate act with a name, not the silent default it used to be.
    # Raise the viewer and the popup backstop before the first attempt, not after:
    # the opening is the part of the game the operator most needs to see against the
    # real one, and a stuck screen in it costs the whole attempt.
    ensure_mirror()
    ensure_popup_clear()

    pinned = None if args.no_pin else code_state()
    if pinned is not None:
        behind = commits_behind_main()
        # Loud above a handful, because that is where "deliberately pinned" stops
        # being the likely explanation and "nobody rebuilt this tree" starts.
        staleness = ""
        if behind:
            staleness = (f"  ⚠ {behind} commits behind origin/main"
                         if behind >= 5 else f"  ({behind} behind origin/main)")
        print(f"batch pinned to {pinned}"
              + ("  ⚠ working tree is dirty; the fingerprint tracks it"
                 if "+" in pinned else "")
              + staleness, flush=True)

    print(batch_power_line(args.attempts), flush=True)
    args.refresh_seconds = batch_refresh_seconds(
        args.refresh_seconds, pinned, args.attempts)
    if args.refresh_seconds == 0.0:
        print("decider frozen for the batch (--civvis-refresh-seconds 0): "
              "a pinned batch must measure one program", flush=True)

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

        # Between games is the only safe moment to touch the ledger, and it is
        # the moment this loop is standing in. See `heal_the_ladder`.
        heal_the_ladder()

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
        # ★★★★ The program that will actually play: see `refresh_orders_binary`.
        print(refresh_orders_binary(orders_bin, args.build), flush=True)
        print(f"\n=== attempt {attempt}/{args.attempts}  {tag}  code={code_rev} ===",
              flush=True)
        # The database path is as much part of a run as its event log.  SQLite
        # keeps an ATTACH bound to an inode, so deleting a global path while a
        # game still has it open creates a new, invisible order channel.
        orders_db = RUN_ROOT / tag / "orders.sqlite"

        play_log = (logs / f"{tag}-play.log").open("w")
        # ⚠ No `<tag>-brain.log` any more, and its absence is the point: an empty
        # file next to a real one reads as "the decider said nothing". The single
        # decider logs to `<run-dir>/brain.log`, where `civ6_play` puts it.
        attempt_started_at = time.time()
        play = subprocess.Popen(
            play_command(args, tag, orders_db, orders_bin),
            stdout=play_log, stderr=subprocess.STDOUT,
        )

        # Bound before the `try` so the ledger stamp below reads a variable that
        # always exists, rather than relying on the exact control flow that
        # happens to reach it today.
        why = None
        # The run dir the attempt's outcome is read from: the original tag, or
        # the last `<tag>-contN` it was resumed under. See `resume_from_autosave`.
        run_tag = tag
        resumes: list[dict] = []
        torn_down = False
        record: dict = {}
        # The frozen run's own row, kept in case the reload never reaches a turn:
        # a resume that fails must not erase the game it was trying to save.
        before_resume: dict = {}
        try:
            while True:
                # Above the child's own ceiling, never above its base budget. See
                # `--timeout-ceiling`: these two numbers inverted once and killed
                # healthy games at turn 194.
                why = wait_watching_the_turn(play, run_tag, attempt_ceiling(args) + 600,
                                             args.frozen_seconds)
                if why == "timeout":
                    print("attempt exceeded its own timeout; stopping it", flush=True)
                    play.send_signal(signal.SIGTERM)
                elif why == "locked":
                    # Already signalled inside the watcher. Named separately so the
                    # log does not blame a timeout for an unattended machine.
                    print("attempt abandoned: the screen stayed locked", flush=True)
                # `civ6_play` owns the decider now and stops it on the way out
                # (`stop_brain`, also registered with atexit), so there is nothing to
                # signal from here. `teardown()` still sweeps a brain that outlived a
                # play process killed by signal, which is the case that motivated the
                # explicit terminate in the first place.
                play_log.close()
                teardown(run_tag)
                torn_down = True
                # Read once per run: this is the attempt's row unless it resumes.
                record = outcome_of(run_tag)
                save = resume_from_autosave(record, why, len(resumes), args,
                                            attempt_started_at)
                if save is None:
                    break
                cont = f"{tag}-cont{len(resumes) + 1}"
                print(f"[resume] {run_tag} froze at turn {record.get('last_turn')}; "
                      f"reloading {save.name} under {cont} (resume {len(resumes) + 1} "
                      f"of {args.max_resumes})", flush=True)
                resumes.append({"tag": cont, "from_turn": record.get("last_turn"),
                                "save": save.name})
                before_resume = record
                run_tag = cont
                play_log = (logs / f"{cont}-play.log").open("w")
                torn_down = False
                play = subprocess.Popen(
                    play_command(args, cont, RUN_ROOT / cont / "orders.sqlite",
                                 orders_bin, load_save=save),
                    stdout=play_log, stderr=subprocess.STDOUT,
                )
        finally:
            # The loop tears each run down before it resumes or breaks; this
            # is the safety net for an exception between those two points.
            if not play_log.closed:
                play_log.close()
            if not torn_down:
                teardown(run_tag)

        if not record:
            record = outcome_of(run_tag)
        if resumes and record.get("last_turn") is None:
            # ⚠ THE RELOAD NEVER REACHED A TURN (the Load Game flow refused, the
            # save would not open, the game did not come back). The attempt is
            # the frozen game as it stood, not a hole: keep its row and say the
            # resume failed, so nothing that was played is lost from the ledger.
            failed = record.get("blocked") or tail_of(logs / f"{run_tag}-play.log")
            record = before_resume
            record["reason"] = "attempt frozen; resume failed"
            record["resume_failed"] = {"tag": run_tag, "why": failed or "no turn observed"}
        if resumes:
            # ★ SAY IT WAS RESUMED. The row's score is the continuation's; the
            # original tag and each freeze turn are kept so a reader can tell a
            # game that ran through from one that was reloaded, and find both
            # run dirs.
            record["resumed_from"] = tag
            record["resumes"] = resumes
        # ★★★★★ A KILLED ATTEMPT MUST SAY SO IN THE LEDGER.
        #
        # `outcome_of` reads what `civ6_play` wrote on its way out. A run this
        # loop SIGTERMs never gets there, so the row carries `last_turn` and
        # `last_score` from the event stream and NO `reason` — and a row with a
        # turn count and a score reads like a finished game to anything that does
        # not check for the missing field. One is in the ladder from 2026-08-11:
        # `civvis-20260811T115348Z`, turn 194 of 250, score 490, stopped by the
        # outer watchdog while still advancing. I included it in medians myself
        # before noticing.
        #
        # This loop knows exactly why it killed the attempt and was throwing that
        # away. Stamp it, and only when the harness supplied nothing — a reason
        # `civ6_play` wrote is the better answer and must never be overwritten.
        if record.get("reason") is None and why is not None:
            record["reason"] = f"attempt {why}"
        record["utc"] = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        record["victory_target"] = args.victory
        record["difficulty_asked"] = args.difficulty
        record["leader_asked"] = args.leader or None
        record["code_rev"] = code_rev

        # ★★★★★ A ROW THAT WAS DEALT SOMETHING ELSE MUST SAY SO, WIN OR NOT.
        #
        # `is_win` already refuses to count a VICTORY at the wrong rung, and the
        # reasoning above it is right: `setup: "(absent)"` on this build means
        # several requested settings never applied, so the `seat` event read back
        # from inside the game is the only trustworthy witness.
        #
        # But that check only ever fires on a win. Every LOSING row — which is all
        # of them so far — was written with `difficulty_asked: DIFFICULTY_SETTLER`
        # beside a seat that said something else, and nothing said a word. Those
        # rows then sit in the ledger being compared against each other as though
        # they were the same experiment.
        #
        # Measured 2026-08-02: 25 consecutive runs dealt DIFFICULTY_SETTLER and the
        # 26th dealt DIFFICULTY_PRINCE, on identical setup code. So this is rare,
        # not chronic — which is exactly why it needs to be recorded rather than
        # watched for. A one-in-twenty-six silent substitution is the kind of thing
        # that is never noticed and quietly explains a whole batch.
        #
        # ⚠ Recorded, NOT fatal. A game at the wrong rung is still a game, and the
        # data is still worth having; what it must not do is masquerade as the rung
        # that was asked for. `settings_dealt` is the field a reader can filter on.
        mismatch = settings_mismatch(
            {"difficulty": args.difficulty, "map_size": args.map_size,
             "speed": args.speed, "leader": args.leader},
            record.get("seat") or {})
        if mismatch:
            record["settings_mismatch"] = mismatch
            for field, pair in mismatch.items():
                print(f"  ⚠ {field}: asked {pair['asked']}, the game dealt "
                      f"{pair['dealt']} — this row is NOT comparable with the rest "
                      f"of the batch", flush=True)

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
