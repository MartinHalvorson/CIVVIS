#!/usr/bin/env python3
"""Run and publish fail-closed completed-game gene-screen rotations.

This is deliberately stricter than a shell loop around ``gene_screen``.  A
six-major all-seats game writes six ``kind: game`` JSONL records, so a physical
line count is not a game count.  The scheduler only crosses its rotation
boundary after :mod:`continuous_screen_status` has proved every seat group is
complete and after the frozen analyzer says the same thing.

One durable state file owns the sequence.  It reserves a seed range *before*
starting a segment, persists the next seed before invoking the binary, and
never reuses the unused tail of an interrupted segment.  It also refuses to
start the next batch until the completed batch has been published through a
fresh isolated CIVVIS task and its green squash PR.  That is intentionally
slower than an optimistic loop: a table that was not safely published must not
be represented by a silently advancing tournament.

The game command has no profile or game-rule flags.  It only sets the standard
screen's game count, target count, fresh seed, output path, and worker cap
(floor 85% of logical cores).  Each batch pins a detached clean ``origin/main``
source worktree and its release binary; the next batch refreshes that source
only after the previous table publication has merged.
"""
from __future__ import annotations

import argparse
import datetime as dt
import fcntl
import hashlib
import json
import os
import re
import secrets
import signal
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from continuous_screen_status import LedgerError, summarize, validate_analysis


ROOT = Path(__file__).resolve().parent.parent
SCHEMA = "continuous_batch_scheduler/v1"
CONTINUOUS_BATCH_TIMING_SCHEMA = "continuous_batch_timing/v1"
CONTINUOUS_BATCH_DEADLINE_SCHEMA = "continuous_batch_deadline/v1"

# ``tools/genes.py write`` deliberately updates the ranking table, its
# supporting evidence, and the generated default-selection mirror. Keep this
# one explicit list shared by the ownership claim, changed-path guard, and
# staging command: a new generated artifact must be reviewed here instead of
# being silently swept into a publication.
PUBLICATION_GENERATED_FILES = (
    "docs/gene_ledger.json",
    "GENE_HEURISTIC_RANKING.md",
    "docs/GENE_RANKING_EVIDENCE.md",
    "src/ai/advanced/genes.rs",
    # ⚠ Added 2026-08-26 after #2584 made `genes.py write` record the compute
    # bill in the same command that moves the genome. The guard below is
    # fail-closed, so from that merge until this one EVERY continuous batch
    # publication on EVERY machine refused with "publishing a report would
    # change an unexpected path: tools/genome_cost_floor.json". That is the
    # guard working; what failed is that this tuple is maintained by hand in
    # one file while the writer lives in another.
    # `the_guard_knows_every_path_genes_py_write_records` now derives the
    # expected set from the writers' own path constants instead.
    "tools/genome_cost_floor.json",
)
STANDARD_PLAYERS = 6
DEFAULT_GOAL_GAMES = 5_000
DEFAULT_POLL_SECONDS = 300.0
INITIAL_CHECK_TIMEOUT_SECONDS = 20 * 60.0
DEADLINE_TERMINATION_GRACE_SECONDS = 30.0
ACTIVE_SEGMENT_POLL_SECONDS = 5.0
# An operator's ``cut`` request is a file beside the state, adopted by the
# running daemon within this many seconds.  It never needs the daemon's lock.
CUT_REQUEST_SCHEMA = "continuous_batch_cut_request/v1"
CUT_REQUEST_NAME = "cut-request.json"
CUT_REQUEST_POLL_SECONDS = 1.0
DEADLINE_REQUESTED_VIA_CUT = "cut_request"


class SchedulerError(RuntimeError):
    """The scheduler cannot prove it is safe to advance."""


@dataclass(frozen=True)
class SegmentResult:
    """How one owned ``gene_screen`` process ended."""

    returncode: int
    stopped_at_deadline: bool
    stopped_at: str | None = None


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace(
        "+00:00", "Z")


def utc_timestamp(value: dt.datetime) -> str:
    """Render one explicit instant canonically, never as an implicit local time."""
    if value.tzinfo is None:
        raise SchedulerError("deadline must name its timezone")
    return value.astimezone(dt.timezone.utc).replace(microsecond=0).isoformat().replace(
        "+00:00", "Z")


def batch_id() -> str:
    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    return f"{stamp}-{secrets.token_hex(2)}"


def positive_int(value: Any, *, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        raise SchedulerError(f"{name} must be a positive integer")
    return value


def checked_relative(value: Any, *, name: str) -> str:
    if not isinstance(value, str) or not value or Path(value).is_absolute():
        raise SchedulerError(f"{name} must be a non-empty relative path")
    if any(part == ".." for part in Path(value).parts):
        raise SchedulerError(f"{name} may not escape the state directory")
    return value


def state_path(root: Path, relative: str) -> Path:
    checked_relative(relative, name="state path")
    candidate = (root / relative).resolve()
    resolved_root = root.resolve()
    if candidate != resolved_root and resolved_root not in candidate.parents:
        raise SchedulerError(f"state path escapes {resolved_root}: {relative}")
    return candidate


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    """Durably replace a state snapshot; never leave a half-written JSON file."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.replace(temporary, path)


def logical_cores() -> int:
    """Read the host's logical CPU count, with a portable conservative fallback."""
    try:
        result = subprocess.run(
            ["/usr/sbin/sysctl", "-n", "hw.logicalcpu"],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if result.returncode == 0:
            count = int(result.stdout.strip())
            if count > 0:
                return count
    except (OSError, ValueError):
        pass
    return max(1, os.cpu_count() or 1)


def workers_for_cores(cores: int) -> int:
    """Use at most the operator-approved 85% cap, never zero workers."""
    return max(1, positive_int(cores, name="logical core count") * 85 // 100)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def new_batch(seed_floor: int, goal_games: int, *, ident: str | None = None) -> dict[str, Any]:
    """Make an unstarted batch whose only progress currency is completed games."""
    positive_int(seed_floor, name="seed floor")
    positive_int(goal_games, name="goal games")
    ident = ident or batch_id()
    if not re.fullmatch(r"[0-9A-Za-z-]+", ident):
        raise SchedulerError(f"unsafe batch id {ident!r}")
    directory = f"batches/{ident}"
    return {
        "id": ident,
        "directory": directory,
        "rows": f"{directory}/rows-continuous.jsonl",
        "analysis": f"{directory}/analysis.json",
        "goal_completed_games": goal_games,
        "phase": "running",
        "complete_games": 0,
        "complete_seats": 0,
        "wins": 0,
        "reservations": [],
        "source": None,
        "publication": {"stage": "not_started"},
        "created_at": utc_now(),
    }


def new_state(seed_floor: int, goal_games: int = DEFAULT_GOAL_GAMES, *,
              deadline_at: dt.datetime | None = None,
              next_goal_games: int | None = None) -> dict[str, Any]:
    """Initial state. ``next_seed`` moves before a game process is launched."""
    positive_int(seed_floor, name="seed floor")
    positive_int(goal_games, name="goal games")
    if deadline_at is None and next_goal_games is not None:
        raise SchedulerError("a successor goal needs a deadline")
    if deadline_at is not None and next_goal_games is None:
        raise SchedulerError("a deadline needs an explicit successor goal")
    current = new_batch(seed_floor, goal_games)
    if deadline_at is not None:
        current["deadline"] = {
            "schema": CONTINUOUS_BATCH_DEADLINE_SCHEMA,
            "deadline_at": utc_timestamp(deadline_at),
            "next_goal_completed_games": positive_int(
                next_goal_games, name="successor goal games"),
        }
    return {
        "schema": SCHEMA,
        "goal_completed_games": goal_games,
        "next_seed": seed_floor,
        "current": current,
        "history": [],
    }


def validate_state(state: dict[str, Any]) -> None:
    if state.get("schema") != SCHEMA:
        raise SchedulerError(
            f"state schema is {state.get('schema')!r}; expected {SCHEMA!r}. "
            "Refuse to guess across formats.")
    goal = positive_int(state.get("goal_completed_games"), name="state goal_completed_games")
    positive_int(state.get("next_seed"), name="state next_seed")
    current = state.get("current")
    if not isinstance(current, dict):
        raise SchedulerError("state has no current batch")
    if positive_int(current.get("goal_completed_games"), name="current goal") != goal:
        raise SchedulerError("state and current batch disagree on the completed-game goal")
    checked_relative(current.get("directory"), name="current directory")
    checked_relative(current.get("rows"), name="current rows")
    checked_relative(current.get("analysis"), name="current analysis")
    if current.get("phase") not in {"running", "frozen", "publishing", "published", "blocked"}:
        raise SchedulerError(f"unknown batch phase {current.get('phase')!r}")
    deadline = current.get("deadline")
    if deadline is not None:
        if not isinstance(deadline, dict):
            raise SchedulerError("batch deadline must be an object")
        if deadline.get("schema") != CONTINUOUS_BATCH_DEADLINE_SCHEMA:
            raise SchedulerError(
                f"batch deadline schema is {deadline.get('schema')!r}; expected "
                f"{CONTINUOUS_BATCH_DEADLINE_SCHEMA!r}")
        parse_utc_timestamp(deadline.get("deadline_at"), name="batch deadline")
        positive_int(deadline.get("next_goal_completed_games"), name="deadline successor goal")
        if deadline.get("cutoff_at") is not None:
            cutoff_at = parse_utc_timestamp(deadline["cutoff_at"], name="deadline cutoff")
            if cutoff_at < parse_utc_timestamp(deadline["deadline_at"], name="batch deadline"):
                raise SchedulerError("deadline cutoff precedes its requested deadline")
            original_goal = positive_int(
                deadline.get("original_goal_completed_games"), name="deadline original goal")
            actual_goal = positive_int(
                deadline.get("actual_completed_games"), name="deadline actual games")
            if actual_goal > original_goal:
                raise SchedulerError("deadline actual games exceed its original goal")
            if goal != actual_goal:
                raise SchedulerError("deadline cutoff and state goal disagree")
            raw_rows = checked_relative(deadline.get("raw_rows"), name="deadline raw rows")
            frozen_rows = checked_relative(deadline.get("frozen_rows"), name="deadline frozen rows")
            if current.get("rows") != frozen_rows or current.get("raw_rows") != raw_rows:
                raise SchedulerError("deadline cutoff row paths disagree with the current batch")
            raw_hash = deadline.get("raw_rows_sha256")
            if not isinstance(raw_hash, str) or not re.fullmatch(r"[0-9a-f]{64}", raw_hash):
                raise SchedulerError("deadline raw row hash is not a SHA-256 digest")
            dropped = deadline.get("dropped_trailing_records")
            if isinstance(dropped, bool) or not isinstance(dropped, int) or dropped < 0:
                raise SchedulerError("deadline dropped trailing record count is invalid")
        elif current.get("phase") != "running":
            raise SchedulerError("an uncut deadline batch cannot leave the running phase")
    reservations = current.get("reservations")
    if not isinstance(reservations, list):
        raise SchedulerError("current reservations must be a list")
    previous_last = 0
    for index, reservation in enumerate(reservations):
        if not isinstance(reservation, dict):
            raise SchedulerError(f"reservation {index} is not an object")
        first = positive_int(reservation.get("seed_first"), name=f"reservation {index} seed_first")
        last = positive_int(reservation.get("seed_last"), name=f"reservation {index} seed_last")
        games = positive_int(reservation.get("target_games"), name=f"reservation {index} target_games")
        if last - first + 1 != games:
            raise SchedulerError(f"reservation {index} seed window does not equal its target games")
        launch_state = reservation.get("launch_state")
        if launch_state is not None and launch_state not in {"reserved", "running", "finished"}:
            raise SchedulerError(f"reservation {index} has an unknown launch state")
        if reservation.get("returncode") is None and launch_state == "finished":
            raise SchedulerError(f"reservation {index} is finished but has no exit record")
        if reservation.get("returncode") is not None and launch_state in {"reserved", "running"}:
            raise SchedulerError(f"reservation {index} has both an exit record and a live launch state")
        if index and first <= previous_last:
            raise SchedulerError("state reservations overlap or are out of order")
        previous_last = last
    if reservations and state["next_seed"] <= previous_last:
        raise SchedulerError("state next_seed reuses a reserved seed")
    if not isinstance(state.get("history"), list):
        raise SchedulerError("state history must be a list")


def load_state(path: Path, *, seed_floor: int | None, goal_games: int,
               deadline_at: dt.datetime | None = None,
               next_goal_games: int | None = None) -> dict[str, Any]:
    if path.exists():
        try:
            state = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as error:
            raise SchedulerError(f"state {path} is invalid JSON: {error}") from error
        if not isinstance(state, dict):
            raise SchedulerError(f"state {path} is not an object")
        validate_state(state)
        if state["goal_completed_games"] != goal_games:
            deadline = state["current"].get("deadline")
            consumed_deadline = (
                isinstance(deadline, dict)
                and deadline.get("original_goal_completed_games") == goal_games
                and deadline.get("next_goal_completed_games") == next_goal_games
                and deadline.get("cutoff_at") is not None
            )
            successor_rotation = (
                next_goal_games is not None
                and state.get("history")
                and state["goal_completed_games"] == next_goal_games
                and deadline is None
            )
            # A ``cut`` deadline was never a command-line contract: the daemon
            # wrote it itself, its successor goal is the original goal, so the
            # unchanged service arguments must keep loading the frozen state.
            cut_deadline = (
                isinstance(deadline, dict)
                and deadline.get("requested_via") == DEADLINE_REQUESTED_VIA_CUT
                and deadline.get("original_goal_completed_games") == goal_games
                and deadline.get("cutoff_at") is not None
            )
            if not consumed_deadline and not successor_rotation and not cut_deadline:
                raise SchedulerError(
                    f"state has a {state['goal_completed_games']:,}-game boundary, not "
                    f"the requested {goal_games:,}. Start a distinct state directory instead.")
        return state
    if seed_floor is None:
        raise SchedulerError(
            f"no state exists at {path}; pass --seed-floor once so its first range is explicit")
    return new_state(seed_floor, goal_games, deadline_at=deadline_at,
                     next_goal_games=next_goal_games)


def batch_directory(state_root: Path, batch: dict[str, Any]) -> Path:
    return state_path(state_root, batch["directory"])


def rows_path(state_root: Path, batch: dict[str, Any]) -> Path:
    return state_path(state_root, batch["rows"])


def analysis_path(state_root: Path, batch: dict[str, Any]) -> Path:
    return state_path(state_root, batch["analysis"])


def empty_status() -> dict[str, Any]:
    return {
        "complete_games": 0,
        "complete_seats": 0,
        "wins": 0,
        "records": 0,
        "reserved_seed_windows": [],
        "partial": True,
    }


def known_windows(batch: dict[str, Any]) -> set[tuple[int, int, int, int]]:
    return {
        (
            reservation["seed_first"],
            reservation["seed_last"],
            reservation["target_games"],
            reservation["target_games"] * STANDARD_PLAYERS,
        )
        for reservation in batch["reservations"]
    }


def refresh_status(state_root: Path, state: dict[str, Any]) -> dict[str, Any]:
    """Read rows through the one validated reader and bind them to state reservations."""
    batch = state["current"]
    rows = rows_path(state_root, batch)
    if not rows.exists():
        status = empty_status()
    else:
        try:
            status = summarize(rows)
        except LedgerError as error:
            raise SchedulerError(f"rows {rows} are not safe to count: {error}") from error
    declared = {
        (
            item["seed_first"], item["seed_last"], item["target_games"], item["target_seats"]
        )
        for item in status["reserved_seed_windows"]
    }
    unknown = declared - known_windows(batch)
    if unknown:
        raise SchedulerError(
            "rows contain a header outside this scheduler state: "
            + ", ".join(f"{first}..{last}" for first, last, _, _ in sorted(unknown)))
    if any(item["players"] != STANDARD_PLAYERS for item in status["reserved_seed_windows"]):
        raise SchedulerError(
            f"rows are not the six-major standard screen (expected {STANDARD_PLAYERS} seats/game)")
    games = int(status["complete_games"])
    seats = int(status["complete_seats"])
    if seats != games * STANDARD_PLAYERS:
        raise SchedulerError("validated status does not conserve six seats per completed game")
    if int(status["wins"]) != games:
        raise SchedulerError("validated status does not conserve exactly one win per game")
    goal = int(batch["goal_completed_games"])
    if games > goal:
        raise SchedulerError(
            f"rows contain {games:,} completed games, beyond this batch's hard {goal:,}-game boundary")
    batch["complete_games"] = games
    batch["complete_seats"] = seats
    batch["wins"] = int(status["wins"])
    batch["last_status"] = status
    return status


def reserve_segment(state: dict[str, Any], status: dict[str, Any]) -> dict[str, Any]:
    """Reserve all remaining games before spawning them; do not reuse interrupted seeds."""
    batch = state["current"]
    games = int(status["complete_games"])
    goal = int(batch["goal_completed_games"])
    remaining = goal - games
    if remaining < 1:
        raise SchedulerError("cannot reserve a segment after the completed-game boundary")
    first = int(state["next_seed"])
    last = first + remaining - 1
    reservation = {
        "seed_first": first,
        "seed_last": last,
        "target_games": remaining,
        "target_seats": remaining * STANDARD_PLAYERS,
        "reserved_at": utc_now(),
        "launch_state": "reserved",
        "returncode": None,
    }
    batch["reservations"].append(reservation)
    # Persist this before launching. A crash now wastes seeds, never repeats them.
    state["next_seed"] = last + 1
    return reservation


def run_checked(command: Iterable[str], *, cwd: Path, description: str,
                env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        list(command), cwd=cwd, text=True, capture_output=True, check=False,
        env=None if env is None else {**os.environ, **env},
    )
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        raise SchedulerError(f"{description} failed ({result.returncode}): {detail[-2000:]}")
    return result


def git_output(repo: Path, *args: str) -> str:
    return run_checked(["git", "-C", str(repo), *args], cwd=repo, description="git command").stdout.strip()


def ensure_source(state_root: Path, state: dict[str, Any], repo: Path) -> dict[str, str]:
    """Pin one clean detached source and binary for the *current* batch only."""
    batch = state["current"]
    source = batch.get("source")
    if isinstance(source, dict):
        required = ("commit", "worktree", "binary", "binary_sha256")
        if not all(isinstance(source.get(key), str) and source[key] for key in required):
            raise SchedulerError("stored batch source is incomplete")
        binary = Path(source["binary"])
        if not binary.is_file() or sha256(binary) != source["binary_sha256"]:
            raise SchedulerError(
                "the pinned batch binary is missing or changed; do not silently rebuild a live batch")
        return {key: str(source[key]) for key in required}

    run_checked(["git", "-C", str(repo), "fetch", "origin", "main"], cwd=repo, description="fetch origin/main")
    commit = git_output(repo, "rev-parse", "origin/main")
    source_dir = state_root / "sources" / commit
    if source_dir.exists():
        if not (source_dir / ".git").exists():
            raise SchedulerError(f"source path exists but is not a Git worktree: {source_dir}")
        actual = git_output(source_dir, "rev-parse", "HEAD")
        if actual != commit:
            raise SchedulerError(f"source worktree {source_dir} is {actual}, expected {commit}")
    else:
        source_dir.parent.mkdir(parents=True, exist_ok=True)
        run_checked(
            ["git", "-C", str(repo), "worktree", "add", "--detach", str(source_dir), commit],
            cwd=repo,
            description="create detached batch source",
        )
    if git_output(source_dir, "status", "--porcelain"):
        raise SchedulerError(f"detached batch source is dirty: {source_dir}")
    run_checked(
        ["cargo", "build", "--release", "--locked", "--bin", "gene_screen"],
        cwd=source_dir,
        description="build pinned gene_screen",
    )
    binary = source_dir / "target" / "release" / "gene_screen"
    if not binary.is_file():
        raise SchedulerError(f"release build did not produce {binary}")
    source = {
        "commit": commit,
        "worktree": str(source_dir),
        "binary": str(binary),
        "binary_sha256": sha256(binary),
    }
    batch["source"] = source
    return source


def terminate_owned_process(process: subprocess.Popen[bytes]) -> int:
    """Stop one scheduler-owned process tree, escalating only after a grace period.

    ``gene_screen`` writes one game as a short ordered group of rows.  The
    deadline path snapshots only fully written groups afterwards, so this
    signal never turns a half-written group into a counted game.  A separate
    session makes the PID a process-group leader, which lets the scheduler
    stop ``caffeinate`` and its child together without touching its own
    launchd process group.
    """
    if process.poll() is not None:
        return int(process.returncode)
    try:
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGTERM)
        else:
            process.terminate()
    except ProcessLookupError:
        pass
    try:
        return process.wait(timeout=DEADLINE_TERMINATION_GRACE_SECONDS)
    except subprocess.TimeoutExpired:
        try:
            if os.name == "posix":
                os.killpg(process.pid, signal.SIGKILL)
            else:
                process.kill()
        except ProcessLookupError:
            pass
        return process.wait()


def active_reservation(batch: dict[str, Any]) -> dict[str, Any] | None:
    """Return the one pre-reserved child which did not report an exit yet."""
    active = [item for item in batch.get("reservations", [])
              if isinstance(item, dict) and item.get("returncode") is None]
    if len(active) > 1:
        raise SchedulerError("more than one scheduler reservation is marked live")
    return active[0] if active else None


def process_is_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def terminate_recorded_process(reservation: dict[str, Any]) -> str:
    """Stop a child left behind by an interrupted scheduler process."""
    pid = positive_int(reservation.get("pid"), name="live reservation pid")
    group = positive_int(reservation.get("process_group", pid), name="live reservation process group")
    if process_is_alive(pid):
        try:
            if os.name == "posix":
                os.killpg(group, signal.SIGTERM)
            else:
                os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        deadline = time.monotonic() + DEADLINE_TERMINATION_GRACE_SECONDS
        while process_is_alive(pid) and time.monotonic() < deadline:
            time.sleep(0.2)
        if process_is_alive(pid):
            try:
                if os.name == "posix":
                    os.killpg(group, signal.SIGKILL)
                else:
                    os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    return utc_now()


def run_segment(state_root: Path, state_pathname: Path, state: dict[str, Any],
                batch: dict[str, Any], reservation: dict[str, Any], *, jobs: int,
                deadline_at: dt.datetime | None = None) -> SegmentResult:
    """Run one reservation, stopping at a durable absolute deadline when set."""
    source = batch.get("source")
    if not isinstance(source, dict):
        raise SchedulerError("cannot run a segment without a pinned source")
    binary = Path(str(source.get("binary", "")))
    if not binary.is_file():
        raise SchedulerError(f"pinned binary is missing: {binary}")
    rows = rows_path(state_root, batch)
    directory = batch_directory(state_root, batch)
    logs = directory / "logs"
    logs.mkdir(parents=True, exist_ok=True)
    log = logs / f"segment-{reservation['seed_first']}.log"
    command = [
        str(binary),
        "--games", str(reservation["target_games"]),
        "--target-games", str(reservation["target_games"]),
        "--start-seed", str(reservation["seed_first"]),
        "--jobs", str(jobs),
        "--out", str(rows),
        "--append",
    ]
    caffeinate = Path("/usr/bin/caffeinate")
    if caffeinate.is_file():
        command = [str(caffeinate), "-dims", *command]
    with log.open("ab") as output:
        process = subprocess.Popen(
            command, cwd=Path(str(source["worktree"])), stdout=output,
            stderr=subprocess.STDOUT, start_new_session=True)
        reservation.update({
            "pid": process.pid,
            "process_group": process.pid,
            "started_at": utc_now(),
            "launch_state": "running",
            "complete_games_at_start": int(batch.get("complete_games") or 0),
        })
        atomic_json(state_pathname, state)
        print(f"{utc_now()} segment_started seed_first={reservation['seed_first']} "
              f"target_games={reservation['target_games']} pid={process.pid}", flush=True)
        stopped_at_deadline = False
        stopped_at: str | None = None
        try:
            while True:
                adopted = adopt_cut_request(state_root, state_pathname, state)
                if adopted is not None:
                    deadline_at = adopted
                wait_seconds = CUT_REQUEST_POLL_SECONDS
                if deadline_at is not None:
                    seconds_left = (deadline_at - dt.datetime.now(dt.timezone.utc)).total_seconds()
                    if seconds_left <= 0:
                        stopped_at_deadline = True
                        stopped_at = utc_now()
                        returncode = terminate_owned_process(process)
                        break
                    wait_seconds = min(seconds_left, CUT_REQUEST_POLL_SECONDS)
                try:
                    returncode = process.wait(timeout=wait_seconds)
                    break
                except subprocess.TimeoutExpired:
                    continue
        except KeyboardInterrupt:
            reservation["returncode"] = terminate_owned_process(process)
            reservation["finished_at"] = utc_now()
            reservation["launch_state"] = "finished"
            atomic_json(state_pathname, state)
            raise
    reservation["returncode"] = returncode
    reservation["finished_at"] = utc_now()
    reservation["launch_state"] = "finished"
    if stopped_at_deadline:
        reservation["deadline_stopped_at"] = stopped_at
    return SegmentResult(
        returncode=returncode,
        stopped_at_deadline=stopped_at_deadline,
        stopped_at=stopped_at,
    )


def parse_utc_timestamp(value: Any, *, name: str) -> dt.datetime:
    """Read one scheduler timestamp without accepting a local-time guess."""
    if not isinstance(value, str) or not value:
        raise SchedulerError(f"{name} must be a non-empty UTC timestamp")
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise SchedulerError(f"{name} is not an ISO-8601 timestamp: {value!r}") from error
    if parsed.tzinfo is None:
        raise SchedulerError(f"{name} must name its timezone: {value!r}")
    return parsed.astimezone(dt.timezone.utc)


def batch_deadline(batch: dict[str, Any]) -> dt.datetime | None:
    """Return the one unconsumed deadline attached to a batch, if any."""
    deadline = batch.get("deadline")
    if deadline is None:
        return None
    if not isinstance(deadline, dict):
        raise SchedulerError("batch deadline must be an object")
    if deadline.get("cutoff_at") is not None:
        return None
    return parse_utc_timestamp(deadline.get("deadline_at"), name="batch deadline")


def verify_deadline_invocation(state: dict[str, Any], *, deadline_at: dt.datetime | None,
                               next_goal_games: int | None) -> None:
    """Bind a running service to the deadline it originally persisted.

    A launchd restart must not quietly lose the stop time, and a consumed
    deadline must not be attached to successor 3,000-game rotations again.
    """
    deadline = state["current"].get("deadline")
    if deadline_at is None:
        if isinstance(deadline, dict) and deadline.get("cutoff_at") is None:
            if deadline.get("requested_via") == DEADLINE_REQUESTED_VIA_CUT:
                # The daemon persisted this one itself from a cut request; a
                # restart re-reads it from state, so nothing can be lost.
                return
            raise SchedulerError("this running state has a deadline; pass its --deadline-at again")
        return
    if next_goal_games is None:
        raise SchedulerError("--deadline-at requires --next-goal-games")
    if deadline is None:
        # The requested deadline was already consumed and rotation deliberately
        # created an ordinary successor batch.  Keep the stable launchd
        # arguments harmless rather than re-attaching a deadline forever.
        if state.get("history"):
            return
        raise SchedulerError("state exists without the requested deadline")
    if not isinstance(deadline, dict):
        raise SchedulerError("batch deadline must be an object")
    if deadline.get("cutoff_at") is not None:
        return
    if deadline.get("deadline_at") != utc_timestamp(deadline_at):
        raise SchedulerError("stored batch deadline differs from --deadline-at")
    if deadline.get("next_goal_completed_games") != next_goal_games:
        raise SchedulerError("stored deadline successor goal differs from --next-goal-games")


def relative_to_state_root(state_root: Path, path: Path) -> str:
    """Store only checked state-relative paths in the durable scheduler file."""
    try:
        return str(path.resolve().relative_to(state_root.resolve()))
    except ValueError as error:
        raise SchedulerError(f"path {path} escapes state root {state_root}") from error


def has_nonblank_after(lines: list[bytes], index: int) -> bool:
    return any(line.strip() for line in lines[index + 1:])


def snapshot_complete_prefix(raw_rows: Path, target: Path) -> tuple[dict[str, Any], int]:
    """Freeze only whole terminal game groups from an interrupted raw ledger.

    The generator appends one ordered six-seat group at a time.  A hard
    deadline can catch its final write halfway through a JSON line or halfway
    through that group.  Preserve the raw ledger untouched and make a stable
    analysis input containing every prior whole group; any malformed or
    short *trailing* group is deliberately excluded.  Corruption anywhere
    else is an error, never an invitation to silently repair history.
    """
    try:
        lines = raw_rows.read_bytes().splitlines(keepends=True)
    except OSError as error:
        raise SchedulerError(f"cannot read deadline rows {raw_rows}: {error}") from error
    kept: list[bytes] = []
    pending: list[bytes] = []
    players: int | None = None
    pending_key: tuple[int, int] | None = None
    saw_header = False
    total_records = 0
    stopped = False
    for index, line in enumerate(lines):
        if not line.strip():
            continue
        total_records += 1
        if stopped:
            raise SchedulerError("nonblank ledger data appears after a truncated trailing record")
        # A row writer emits a newline with every committed record.  Treat an
        # unterminated tail as incomplete even if its JSON happened to parse.
        if not line.endswith(b"\n"):
            if has_nonblank_after(lines, index):
                raise SchedulerError("unterminated row is not the terminal ledger record")
            stopped = True
            continue
        try:
            decoded = line.decode("utf-8")
            record = json.loads(decoded)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            if has_nonblank_after(lines, index):
                raise SchedulerError(f"malformed nonterminal ledger record: {error}") from error
            stopped = True
            continue
        if not isinstance(record, dict):
            raise SchedulerError("ledger record is not an object")
        kind = record.get("kind")
        if kind == "header":
            if pending:
                if players is None or len(pending) != players:
                    raise SchedulerError("a header interrupts an unfinished game group")
                kept.extend(pending)
                pending = []
                pending_key = None
            try:
                players = positive_int(record.get("players"), name="deadline header players")
            except SchedulerError as error:
                raise SchedulerError("deadline header has no valid player count") from error
            kept.append(line)
            saw_header = True
            continue
        if kind != "game":
            raise SchedulerError(f"ledger record has unexpected kind {kind!r}")
        if players is None:
            raise SchedulerError("game row appears before its header")
        seed = record.get("seed")
        arm = record.get("arm", 0)
        if isinstance(seed, bool) or not isinstance(seed, int) or isinstance(arm, bool) or not isinstance(arm, int):
            raise SchedulerError("game row has no integer seed/arm key")
        key = (seed, arm)
        if pending_key is not None and key != pending_key:
            if len(pending) != players:
                raise SchedulerError("an incomplete game group is followed by another game")
            kept.extend(pending)
            pending = []
            pending_key = None
        if pending_key is None:
            pending_key = key
        pending.append(line)
        if len(pending) > players:
            raise SchedulerError("a game group has more rows than its declared player count")
    if pending and len(pending) == players:
        kept.extend(pending)
    if not saw_header:
        raise SchedulerError("deadline ledger contains no header")
    kept_records = sum(1 for line in kept if line.strip())
    dropped_records = total_records - kept_records
    temporary = target.with_name(f".{target.name}.{os.getpid()}.tmp")
    target.parent.mkdir(parents=True, exist_ok=True)
    try:
        temporary.write_bytes(b"".join(kept))
        status = summarize(temporary)
        os.replace(temporary, target)
    except (OSError, LedgerError) as error:
        try:
            temporary.unlink(missing_ok=True)
        except OSError:
            pass
        raise SchedulerError(f"deadline ledger cannot be safely frozen: {error}") from error
    return status, dropped_records


def cut_request_path(state_root: Path) -> Path:
    return state_root / CUT_REQUEST_NAME


def write_cut_request(state_root: Path, *, deadline_at: dt.datetime,
                      note: str | None = None) -> Path:
    """Ask the running scheduler to stop at ``deadline_at`` and freeze what it has.

    The request is a file beside the state, so it never needs the daemon's
    exclusive lock, a restart, or a hand-edited state file.  The daemon adopts
    it within a second into the same ``continuous_batch_deadline/v1`` block a
    ``--deadline-at`` launch persists, so the snapshot, validation and audit
    trail are the tool's.  The successor rotation keeps the original goal.
    """
    path = cut_request_path(state_root)
    if path.exists():
        raise SchedulerError(
            f"a cut request is already pending at {path}; the scheduler has not adopted it "
            "(is it running?)")
    request: dict[str, Any] = {
        "schema": CUT_REQUEST_SCHEMA,
        "deadline_at": utc_timestamp(deadline_at),
        "requested_at": utc_now(),
    }
    if note:
        request["note"] = note
    atomic_json(path, request)
    return path


def adopt_cut_request(state_root: Path, state_pathname: Path,
                      state: dict[str, Any]) -> dt.datetime | None:
    """Install a pending cut request as the running batch's deadline.

    Returns the adopted deadline, or ``None`` when nothing is pending.  A
    request that cannot apply is kept beside the state under a ``.rejected``
    name with its reason: the daemon never dies over an operator file, and the
    operator can read why nothing happened.
    """
    path = cut_request_path(state_root)
    if not path.exists():
        return None
    batch = state["current"]
    raw = path.read_text(encoding="utf-8")
    try:
        try:
            request = json.loads(raw)
        except json.JSONDecodeError as error:
            raise SchedulerError(f"cut request is not JSON: {error}") from error
        if not isinstance(request, dict) or request.get("schema") != CUT_REQUEST_SCHEMA:
            raise SchedulerError(f"cut request schema must be {CUT_REQUEST_SCHEMA!r}")
        deadline_at = parse_utc_timestamp(request.get("deadline_at"), name="cut request deadline")
        if batch["phase"] != "running":
            raise SchedulerError(f"batch is {batch['phase']!r}, not running")
        if batch_deadline(batch) is not None:
            raise SchedulerError("batch already has an unconsumed deadline")
    except SchedulerError as error:
        rejected = path.with_name(
            f"cut-request.rejected-{utc_now().replace(':', '')}-{secrets.token_hex(2)}.json")
        atomic_json(rejected, {"rejected": str(error), "request": raw})
        path.unlink()
        print(f"{utc_now()} cut_request_rejected {error}", flush=True)
        return None
    batch["deadline"] = {
        "schema": CONTINUOUS_BATCH_DEADLINE_SCHEMA,
        "deadline_at": utc_timestamp(deadline_at),
        "next_goal_completed_games": positive_int(
            batch.get("goal_completed_games"), name="cut successor goal"),
        "requested_via": DEADLINE_REQUESTED_VIA_CUT,
        "requested_at": request.get("requested_at"),
    }
    if request.get("note"):
        batch["deadline"]["note"] = str(request["note"])
    # Durable first: once the state names the deadline a restart honours it.
    atomic_json(state_pathname, state)
    adopted = batch_directory(state_root, batch) / CUT_REQUEST_NAME
    adopted.parent.mkdir(parents=True, exist_ok=True)
    path.replace(adopted)
    print(f"{utc_now()} cut_request_adopted deadline_at={batch['deadline']['deadline_at']}",
          flush=True)
    return deadline_at


def read_state(state_root: Path) -> dict[str, Any]:
    """Read an existing state file without the daemon's lock or goal reconciliation."""
    path = state_root / "scheduler-state.json"
    if not path.exists():
        raise SchedulerError(f"no scheduler state at {path}")
    try:
        state = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise SchedulerError(f"state {path} is invalid JSON: {error}") from error
    if not isinstance(state, dict):
        raise SchedulerError(f"state {path} is not an object")
    validate_state(state)
    return state


def scheduler_is_running(state_root: Path) -> bool:
    """True when another process holds this state directory's exclusive lock."""
    lock_path = state_root / "scheduler.lock"
    if not lock_path.exists():
        return False
    with lock_path.open("a+") as lock:
        try:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return True
        fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
    return False


def seal_deadline_cutoff(state_root: Path, state_pathname: Path, state: dict[str, Any], *,
                         stopped_at: str | None = None) -> None:
    """Turn a passed deadline into one auditable, validated partial screen.

    The raw header retains the intentionally enormous reservation.  The
    frozen snapshot therefore remains explicitly partial in its analyzer
    artefact, while its scheduler boundary and table sample size are the
    actual validated games/seats played by the requested deadline.
    """
    batch = state["current"]
    deadline = batch.get("deadline")
    if not isinstance(deadline, dict):
        raise SchedulerError("cannot seal a batch without a deadline")
    if deadline.get("cutoff_at") is not None:
        status = refresh_status(state_root, state)
        if status["complete_games"] != batch["goal_completed_games"]:
            raise SchedulerError("previous deadline cutoff no longer matches its frozen boundary")
        freeze_analysis(state_root, state, state_pathname=state_pathname)
        atomic_json(state_pathname, state)
        return
    raw_relative = checked_relative(batch.get("rows"), name="raw deadline rows")
    raw_rows = state_path(state_root, raw_relative)
    frozen_rows = batch_directory(state_root, batch) / "rows-deadline-cutoff.jsonl"
    status, dropped_records = snapshot_complete_prefix(raw_rows, frozen_rows)
    complete_games = positive_int(status.get("complete_games"), name="deadline completed games")
    original_goal = positive_int(batch.get("goal_completed_games"), name="deadline original goal")
    if complete_games > original_goal:
        raise SchedulerError("deadline snapshot exceeds its original completed-game goal")
    frozen_relative = relative_to_state_root(state_root, frozen_rows)
    batch["raw_rows"] = raw_relative
    batch["rows"] = frozen_relative
    batch["goal_completed_games"] = complete_games
    state["goal_completed_games"] = complete_games
    deadline.update({
        "cutoff_at": stopped_at or utc_now(),
        "original_goal_completed_games": original_goal,
        "actual_completed_games": complete_games,
        "raw_rows": raw_relative,
        "raw_rows_sha256": sha256(raw_rows),
        "frozen_rows": frozen_relative,
        "dropped_trailing_records": dropped_records,
    })
    # The process stopped at this instant.  Do not let analysis or publication
    # retry time make the rate look slower than the tournament itself.
    batch.setdefault("completed_at", deadline["cutoff_at"])
    refresh_status(state_root, state)
    atomic_json(state_pathname, state)
    freeze_analysis(state_root, state, state_pathname=state_pathname)
    atomic_json(state_pathname, state)


def deadline_reporting_metadata(batch: dict[str, Any]) -> dict[str, Any] | None:
    """Return the immutable audit trail for a deadline-finalized report."""
    deadline = batch.get("deadline")
    if not isinstance(deadline, dict) or deadline.get("cutoff_at") is None:
        return None
    return {
        "schema": CONTINUOUS_BATCH_DEADLINE_SCHEMA,
        "deadline_at": deadline["deadline_at"],
        "cutoff_at": deadline["cutoff_at"],
        "original_goal_completed_games": deadline["original_goal_completed_games"],
        "actual_completed_games": deadline["actual_completed_games"],
        "raw_rows": deadline["raw_rows"],
        "raw_rows_sha256": deadline["raw_rows_sha256"],
        "frozen_rows": deadline["frozen_rows"],
        "dropped_trailing_records": deadline["dropped_trailing_records"],
    }


def continuous_batch_timing(batch: dict[str, Any]) -> dict[str, Any] | None:
    """Return the immutable whole-batch wall-clock measure when it is known.

    A pre-timing scheduler state may already be frozen when this code lands.
    Keep that historical publication viable, but never invent a rate: reports
    without both ends of the interval simply retain ``games/min=not recorded``.
    New batches always receive ``completed_at`` before their analyzer runs.
    """
    started_at = batch.get("created_at")
    completed_at = batch.get("completed_at") or batch.get("frozen_at")
    if started_at is None or completed_at is None:
        return None
    started = parse_utc_timestamp(started_at, name="batch created_at")
    completed = parse_utc_timestamp(completed_at, name="batch completed_at")
    elapsed_seconds = int((completed - started).total_seconds())
    if elapsed_seconds < 1:
        raise SchedulerError("batch timing must span at least one wall-clock second")
    return {
        "schema": CONTINUOUS_BATCH_TIMING_SCHEMA,
        "started_at": str(started_at),
        "completed_at": str(completed_at),
        "elapsed_seconds": elapsed_seconds,
        "completed_games": positive_int(batch.get("complete_games"), name="completed games"),
    }


def write_reporting_artifact(source: Path, target: Path, batch: dict[str, Any]) -> None:
    """Copy a frozen analysis and add scheduler-owned batch timing provenance.

    The analyzer output remains immutable under the run state.  The report copy
    is where the publication pipeline records the scheduler interval that the
    analyzer cannot know: one batch can contain several non-overlapping
    resumed segments, while its rate is deliberately the whole batch's games
    divided by its whole wall-clock duration.
    """
    try:
        analysis = json.loads(source.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SchedulerError(f"cannot read frozen analysis {source}: {error}") from error
    if analysis.get("kind") != "gene_screen_analysis":
        raise SchedulerError(f"frozen analysis {source} is not a gene-screen analysis")
    games = positive_int(analysis.get("games"), name="frozen analysis games")
    expected_games = positive_int(batch.get("complete_games"), name="completed games")
    if games != expected_games:
        raise SchedulerError(
            f"frozen analysis says {games:,} games but batch says {expected_games:,}")
    timing = continuous_batch_timing(batch)
    if timing is not None:
        analysis["continuous_batch_timing"] = timing
    deadline = deadline_reporting_metadata(batch)
    if deadline is not None:
        analysis["continuous_batch_deadline"] = deadline
    atomic_json(target, analysis)


def freeze_analysis(state_root: Path, state: dict[str, Any], *,
                    state_pathname: Path | None = None) -> None:
    """Freeze only the exact validated completed-game boundary into an artifact."""
    batch = state["current"]
    status = refresh_status(state_root, state)
    if status["complete_games"] != batch["goal_completed_games"]:
        raise SchedulerError("cannot freeze a batch before exactly its completed-game boundary")
    source = batch.get("source")
    if not isinstance(source, dict):
        raise SchedulerError("cannot analyze without a pinned batch source")
    # Capture the boundary before analysis starts.  A failed analyzer retry
    # must not make a completed tournament look slower just because its
    # reporting work took longer. Persist it immediately when a durable state
    # path is available so an interruption cannot lose the real endpoint.
    if batch.get("completed_at") is None:
        batch["completed_at"] = utc_now()
        if state_pathname is not None:
            atomic_json(state_pathname, state)
    binary = Path(str(source.get("binary", "")))
    rows = rows_path(state_root, batch)
    analysis = analysis_path(state_root, batch)
    analysis.parent.mkdir(parents=True, exist_ok=True)
    run_checked(
        [str(binary), "--analyze", str(rows), "--json", str(analysis)],
        cwd=Path(str(source["worktree"])),
        description="freeze gene-screen analysis",
    )
    try:
        validate_analysis(status, analysis)
    except LedgerError as error:
        raise SchedulerError(f"frozen analysis disagrees with validated rows: {error}") from error
    batch["phase"] = "frozen"
    batch["frozen_at"] = utc_now()


def reporting_filename(batch: dict[str, Any]) -> str:
    seats = positive_int(batch.get("complete_seats"), name="complete seats")
    return f"{dt.datetime.now(dt.timezone.utc):%Y-%m-%d}-standard-continuous-{seats}-total-seats-{batch['id']}.json"


def parse_start_output(output: str) -> tuple[Path, int]:
    worktree = re.search(r"^worktree:\s+(.+)$", output, flags=re.MULTILINE)
    pull = re.search(r"^draft PR:\s+.+/(\d+)\s*$", output, flags=re.MULTILINE)
    if not worktree or not pull:
        raise SchedulerError(f"could not parse task launcher output:\n{output[-2000:]}")
    return Path(worktree.group(1).strip()), int(pull.group(1))


def computer_name() -> str:
    try:
        result = subprocess.run(["scutil", "--get", "ComputerName"], text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False)
        if result.returncode == 0 and result.stdout.strip():
            return result.stdout.strip()
    except OSError:
        pass
    return os.uname().nodename


def check_quiet(worktree: Path, number: int, *, timeout_seconds: float = INITIAL_CHECK_TIMEOUT_SECONDS) -> None:
    """Do not edit a PR body or push its head while any of its checks are live."""
    deadline = time.monotonic() + timeout_seconds
    while True:
        result = run_checked(
            ["gh", "pr", "view", str(number), "--json", "statusCheckRollup"],
            cwd=worktree,
            description=f"read initial PR #{number} checks",
        )
        try:
            rollup = json.loads(result.stdout).get("statusCheckRollup") or []
        except json.JSONDecodeError as error:
            raise SchedulerError(f"could not decode PR #{number} checks: {error}") from error
        active = [str(item.get("name", "unnamed")) for item in rollup
                  if str(item.get("status", "")).upper() not in {"COMPLETED", ""}]
        if not active:
            return
        if time.monotonic() >= deadline:
            raise SchedulerError(
                f"initial PR #{number} checks are still in flight: {', '.join(active)}")
        time.sleep(10.0)


def reporting_write_command(report: str) -> list[str]:
    """Publish a batch and reselect defaults from the latest three reports.

    A completed reporting batch is evidence for the operator's recorded
    completed-seat-weighted selection rule.  The generator owns the strict
    threshold and family resolution, so the scheduler must ask it to reselect
    rather than carrying a stale genome from the publication task's base.
    """
    return [
        sys.executable, "tools/genes.py", "write",
        "--reselect-deployment-defaults",
        "--reporting-batch", report,
    ]


def publication_body(batch: dict[str, Any], report: str, *, machine: str, agent: str,
                     coordinated: str, computer: str) -> str:
    source = batch["source"]
    return "\n".join((
        "## Ownership claim",
        "",
        f"- Machine ID: `{machine}`",
        f"- Computer: `{computer}`",
        f"- Agent/session ID: `{agent}`",
        f"- Task: publish validated {batch['complete_games']:,}-completed-game continuous batch `{batch['id']}`",
        "- Claimed paths: `" + report + "`, "
        + ", ".join(f"`{path}`" for path in PUBLICATION_GENERATED_FILES)
        + ", `tools/test_genes.py`",
        f"- Coordinated with: {coordinated or 'none'}",
        "- Related issue/request: operator-directed automatic completed-game rotation",
        "",
        "## What changed",
        "",
        f"- Publishes exactly **{batch['complete_games']:,} validated completed games / "
        f"{batch['complete_seats']:,} seats / {batch['wins']:,} wins** from the frozen "
        f"continuous batch `{batch['id']}`.",
        "- The table rotation derives its batch header sample size from the immutable analyzer "
        "artifact; raw JSONL records are never treated as games.",
        f"- The games were pinned to clean source `{source['commit']}` / binary "
        f"`{source['binary_sha256']}`. This publication changes no game mechanics; it updates "
        "the generated table evidence and reselects the default genome under the recorded "
        "completed-seat-weighted latest-three-batch policy.",
        "",
        "overwrite-guard: allow this report deliberately regenerates the complete ranking snapshot from "
        "a frozen batch, replacing its current table, ledger, and evidence rows.",
        "",
        "## Validation",
        "",
        "- [x] Batch boundary reached only after `continuous_screen_status.py` validated "
        "complete seat groups and one winner per game",
        "- [x] Frozen analyzer cross-checked the same completed games and seats",
        "- [x] `python3 tools/genes.py check`",
        "- [x] `python3 tools/test_genes.py`",
        "- [x] `cargo test --profile ci --locked`",
        "- [x] `git diff --check origin/main...`",
        "- [x] No game-mechanic source change; the reporting batch mechanically reselects defaults "
        "under the recorded policy, so a soak is not applicable",
        "- [x] No unrelated runtime artifacts",
        "",
        "## Notes for integration",
        "",
        "Squash merge only. Delete the branch after merge.",
        "",
    ))


def merged_publication_details(worktree: Path, number: int) -> dict[str, str] | None:
    """Return durable merge metadata only when GitHub confirms this PR merged.

    A human or a separate integration worker can merge a publication between
    the scheduler's ``push`` and ``ship`` transitions.  Looking up that state
    makes the final transition idempotent instead of treating an already
    successful publication as an error that stalls the next batch.
    """
    # ``civvis_collab ship`` removes a merged task worktree.  GitHub remains
    # the durable authority for that terminal state, so fall back to this
    # scheduler's management checkout when the retained task directory is no
    # longer available.
    query_cwd = worktree if worktree.is_dir() else ROOT
    result = run_checked(
        ["gh", "pr", "view", str(number), "--json", "state,mergedAt,mergeCommit"],
        cwd=query_cwd,
        description=f"read publication PR #{number} merge state",
    )
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise SchedulerError(f"could not decode publication PR #{number} merge state: {error}") from error
    if payload.get("state") != "MERGED":
        return None
    details = {"merged_at": str(payload.get("mergedAt") or utc_now())}
    merge_commit = payload.get("mergeCommit")
    if isinstance(merge_commit, dict) and isinstance(merge_commit.get("oid"), str):
        details["merge_commit"] = merge_commit["oid"]
    return details


def mark_published_if_already_merged(batch: dict[str, Any], publication: dict[str, Any], *, worktree: Path,
                                     number: int) -> bool:
    """Persist an externally completed merge and return whether one was found."""
    details = merged_publication_details(worktree, number)
    if details is None:
        return False
    publication.update({"stage": "merged", **details})
    batch["phase"] = "published"
    return True


def publish_batch(state_root: Path, state_pathname: Path, state: dict[str, Any], *, repo: Path,
                  machine: str, agent: str) -> None:
    """Publish a frozen batch via a fresh isolated PR before rotating again.

    Ambiguous interruptions while creating a new task deliberately block rather
    than creating a duplicate publication.  Later stages are idempotent or can
    be inspected in the retained task worktree.
    """
    batch = state["current"]
    if batch["phase"] not in {"frozen", "publishing"}:
        raise SchedulerError(f"cannot publish batch in phase {batch['phase']!r}")
    status = refresh_status(state_root, state)
    if status["complete_games"] != batch["goal_completed_games"]:
        raise SchedulerError("frozen batch no longer has exactly its completed-game boundary")
    try:
        validate_analysis(status, analysis_path(state_root, batch))
    except LedgerError as error:
        raise SchedulerError(f"cannot publish an analysis that no longer matches its rows: {error}") from error
    publication = batch.setdefault("publication", {"stage": "not_started"})
    if not isinstance(publication, dict):
        raise SchedulerError("batch publication state is malformed")
    stage = publication.get("stage", "not_started")
    if stage == "claiming":
        raise SchedulerError(
            "the scheduler was interrupted while creating a publication task; "
            "refusing to create a duplicate PR")
    if stage == "not_started":
        report_name = reporting_filename(batch)
        report = f"docs/gene_screens/{report_name}"
        publication.update({"stage": "claiming", "report": report, "started_at": utc_now()})
        batch["phase"] = "publishing"
        atomic_json(state_pathname, state)
        title = f"Publish {batch['complete_games']:,}-game continuous batch"
        command = [
            sys.executable, str(repo / "tools" / "civvis_collab.py"), "start",
            "publish-continuous-batch", "--machine", machine, "--agent", agent,
            "--path", report,
            *(item for path in PUBLICATION_GENERATED_FILES for item in ("--path", path)),
            "--path", "tools/test_genes.py",
            "--title", title,
        ]
        result = run_checked(command, cwd=repo, description="create batch publication task")
        worktree, number = parse_start_output(result.stdout)
        claim = run_checked(
            ["gh", "pr", "view", str(number), "--json", "body"], cwd=worktree,
            description="read publication ownership claim",
        )
        try:
            claimed_body = str(json.loads(claim.stdout).get("body") or "")
        except json.JSONDecodeError as error:
            raise SchedulerError(f"could not read publication claim: {error}") from error
        coordinated = re.search(r"^- Coordinated with:\s*(.+)$", claimed_body, re.MULTILINE)
        publication.update({
            "stage": "claimed",
            "worktree": str(worktree),
            "pr_number": number,
            "coordinated": coordinated.group(1).strip() if coordinated else "none",
        })
        atomic_json(state_pathname, state)
        stage = "claimed"

    worktree_value = publication.get("worktree")
    number_value = publication.get("pr_number")
    report_value = publication.get("report")
    if not isinstance(worktree_value, str) or not isinstance(number_value, int) or not isinstance(report_value, str):
        raise SchedulerError("publication claim is incomplete")
    worktree = Path(worktree_value)
    number = number_value
    report = report_value

    # An operator may merge the publication while this scheduler is stopped or
    # while it is retrying a prior stage. Check GitHub before requiring the
    # task worktree: a normal successful ship deletes that temporary directory.
    # Otherwise a merged ``claimed`` PR would be rejected as missing instead
    # of permitting the next batch to start.
    if mark_published_if_already_merged(batch, publication, worktree=worktree, number=number):
        atomic_json(state_pathname, state)
        return

    if not worktree.is_dir():
        raise SchedulerError(f"publication worktree is missing: {worktree}")

    if stage == "claimed":
        target = worktree / report
        target.parent.mkdir(parents=True, exist_ok=True)
        write_reporting_artifact(analysis_path(state_root, batch), target, batch)
        generated = list(PUBLICATION_GENERATED_FILES)
        write = reporting_write_command(report)
        first = subprocess.run(write, cwd=worktree, text=True, capture_output=True, check=False)
        if first.returncode != 0:
            reason = (
                f"The immutable {batch['id']} continuous result was built from clean "
                f"{batch['source']['commit']}; it remains latest-three evidence for the "
                "completed-seat-weighted deployment policy, but that historic build cannot "
                "be re-verified locally."
            )
            run_checked(
                [*write, "--reporting-unverified-build", reason], cwd=worktree,
                description="record historical reporting-build exception",
            )
            publication["build_exception"] = reason
        changed = set(
            run_checked(["git", "status", "--porcelain"], cwd=worktree,
                        description="inspect publication worktree").stdout.splitlines())
        changed_paths = {line[3:] for line in changed if len(line) >= 4}
        allowed = {report, *generated}
        unexpected = sorted(changed_paths - allowed)
        if unexpected:
            raise SchedulerError(
                "publishing a report would change an unexpected path: " + ", ".join(unexpected))
        run_checked([sys.executable, "tools/genes.py", "check"], cwd=worktree,
                    description="verify generated ranking")
        run_checked([sys.executable, "tools/test_genes.py"], cwd=worktree,
                    description="run generated-ranking regressions")
        run_checked(["cargo", "test", "--profile", "ci", "--locked"], cwd=worktree,
                    description="test publication source", env={"RUST_TEST_THREADS": "1"})
        run_checked(["git", "diff", "--check", "origin/main..."], cwd=worktree,
                    description="check publication diff")
        publication["stage"] = "prepared"
        atomic_json(state_pathname, state)
        stage = "prepared"

    if stage == "prepared":
        computer = computer_name()
        run_checked(
            ["git", "add", "--", report, *PUBLICATION_GENERATED_FILES],
            cwd=worktree,
            description="stage publication",
        )
        run_checked(
            ["git", "commit", "-m", f"Publish {batch['complete_games']:,}-game continuous batch",
             "-m", f"Computer: {computer}"],
            cwd=worktree,
            description="commit publication",
        )
        publication.update({"stage": "committed", "computer": computer})
        atomic_json(state_pathname, state)
        stage = "committed"

    if stage == "committed":
        check_quiet(worktree, number)
        body = publication_body(batch, report, machine=machine, agent=agent,
                                coordinated=str(publication.get("coordinated") or "none"),
                                computer=str(publication["computer"]))
        run_checked(["gh", "pr", "edit", str(number), "--body", body], cwd=worktree,
                    description="write publication PR body")
        # A pull_request.edited workflow is allowed to start; wait through it
        # before the code push so we never cancel a check we just caused.
        check_quiet(worktree, number)
        publication["stage"] = "body_written"
        atomic_json(state_pathname, state)
        stage = "body_written"

    if stage == "body_written":
        run_checked(["git", "push", "origin", "HEAD"], cwd=worktree, description="push publication")
        publication["stage"] = "pushed"
        atomic_json(state_pathname, state)
        stage = "pushed"

    if stage == "pushed":
        if mark_published_if_already_merged(
                batch, publication, worktree=worktree, number=number):
            atomic_json(state_pathname, state)
            return
        try:
            run_checked([sys.executable, "tools/civvis_collab.py", "ship"], cwd=worktree,
                        description="ship publication PR")
        except SchedulerError:
            # The PR may have merged in the narrow interval after the lookup.
            # Only absorb this error if GitHub now proves that exact outcome.
            if mark_published_if_already_merged(
                    batch, publication, worktree=worktree, number=number):
                atomic_json(state_pathname, state)
                return
            raise
        publication["stage"] = "merged"
        publication["merged_at"] = utc_now()
        batch["phase"] = "published"
        atomic_json(state_pathname, state)


def rotate(state_pathname: Path, state: dict[str, Any]) -> None:
    """Start a new empty batch only after its predecessor's publication merged."""
    previous = state["current"]
    if previous["phase"] != "published":
        raise SchedulerError("cannot rotate before the previous batch is published")
    state["history"].append({
        "id": previous["id"],
        "complete_games": previous["complete_games"],
        "complete_seats": previous["complete_seats"],
        "wins": previous["wins"],
        "source": previous["source"],
        "publication": previous["publication"],
        "deadline": previous.get("deadline"),
        "closed_at": utc_now(),
    })
    deadline = previous.get("deadline")
    if isinstance(deadline, dict) and deadline.get("cutoff_at") is not None:
        state["goal_completed_games"] = positive_int(
            deadline.get("next_goal_completed_games"), name="deadline successor goal")
    state["current"] = new_batch(state["next_seed"], state["goal_completed_games"])
    atomic_json(state_pathname, state)


def tick(state_root: Path, state_pathname: Path, state: dict[str, Any], *, repo: Path,
         jobs: int, machine: str | None, agent: str | None, publish: bool) -> str:
    """Advance one durable boundary: a segment, freeze, publication, or rotation."""
    batch = state["current"]
    if batch["phase"] == "running":
        adopt_cut_request(state_root, state_pathname, state)
    deadline = batch_deadline(batch)
    now = dt.datetime.now(dt.timezone.utc)
    live = active_reservation(batch) if batch["phase"] == "running" else None
    if live is not None:
        launch_state = live.get("launch_state")
        if launch_state == "reserved":
            # A crash between seed reservation and Popen means no child can
            # exist.  Retire the range rather than ever replaying it.
            live["returncode"] = "not_started_after_scheduler_restart"
            live["finished_at"] = utc_now()
            live["launch_state"] = "finished"
            atomic_json(state_pathname, state)
        elif launch_state != "running":
            raise SchedulerError("live reservation has no recognized launch state")
        else:
            pid = positive_int(live.get("pid"), name="live reservation pid")
            if process_is_alive(pid):
                if deadline is not None and now >= deadline:
                    stopped_at = terminate_recorded_process(live)
                    live["returncode"] = "deadline_after_scheduler_restart"
                    live["finished_at"] = stopped_at
                    live["launch_state"] = "finished"
                    live["deadline_stopped_at"] = stopped_at
                    atomic_json(state_pathname, state)
                    seal_deadline_cutoff(state_root, state_pathname, state, stopped_at=stopped_at)
                    return "frozen_deadline"
                return "active_segment"
            # The scheduler died after it had persisted the reservation but before
            # it could record the child exit.  The reserved seeds remain spent;
            # the next segment can only begin after them.
            live["returncode"] = "unobserved_after_scheduler_restart"
            live["finished_at"] = utc_now()
            live["launch_state"] = "finished"
            atomic_json(state_pathname, state)
    if batch["phase"] == "running" and deadline is not None and now >= deadline:
        # This also recovers the narrow interruption after the deadline
        # signal: it snapshots the raw prefix before asking the normal reader
        # to parse a possible terminal half-row.
        seal_deadline_cutoff(state_root, state_pathname, state)
        return "frozen_deadline"
    status = refresh_status(state_root, state)
    atomic_json(state_pathname, state)
    phase = batch["phase"]
    if phase == "running":
        if status["complete_games"] == batch["goal_completed_games"]:
            freeze_analysis(state_root, state, state_pathname=state_pathname)
            atomic_json(state_pathname, state)
            return "frozen"
        source = ensure_source(state_root, state, repo)
        del source
        atomic_json(state_pathname, state)
        reservation = reserve_segment(state, status)
        atomic_json(state_pathname, state)
        result = run_segment(
            state_root, state_pathname, state, batch, reservation, jobs=jobs,
            deadline_at=deadline)
        atomic_json(state_pathname, state)
        if result.stopped_at_deadline:
            seal_deadline_cutoff(
                state_root, state_pathname, state, stopped_at=result.stopped_at)
            return "frozen_deadline"
        latest = refresh_status(state_root, state)
        atomic_json(state_pathname, state)
        if latest["complete_games"] == batch["goal_completed_games"]:
            freeze_analysis(state_root, state, state_pathname=state_pathname)
            atomic_json(state_pathname, state)
            return "frozen"
        if result.returncode == 0:
            raise SchedulerError(
                "gene_screen exited successfully before its pre-registered remaining games; "
                "refusing to invent a replacement segment")
        return "partial_segment"
    if phase in {"frozen", "publishing"}:
        if not publish:
            return "awaiting_publication"
        if not machine or not agent:
            raise SchedulerError("publishing needs --machine and --publisher-agent")
        publish_batch(state_root, state_pathname, state, repo=repo, machine=machine, agent=agent)
        return "published"
    if phase == "published":
        rotate(state_pathname, state)
        return "rotated"
    if phase == "blocked":
        raise SchedulerError("batch is blocked; inspect its retained state and logs before retrying")
    raise SchedulerError(f"unsupported batch phase {phase!r}")


def status_report(state_root: Path, state: dict[str, Any]) -> dict[str, Any]:
    """One machine-readable status: progress, rate, ETA, daemon and cut state.

    Reads rows through the validated reader only; it never persists anything,
    so it is safe while the daemon owns the state directory.
    """
    batch = state["current"]
    status = refresh_status(state_root, state)
    goal = int(batch["goal_completed_games"])
    complete = int(status["complete_games"])
    live = active_reservation(batch) if batch["phase"] == "running" else None
    rate_per_hour: float | None = None
    eta_at: str | None = None
    if live is not None and live.get("launch_state") == "running" \
            and isinstance(live.get("complete_games_at_start"), int):
        started = parse_utc_timestamp(live.get("started_at"), name="live segment start")
        elapsed = (dt.datetime.now(dt.timezone.utc) - started).total_seconds()
        played = complete - int(live["complete_games_at_start"])
        if elapsed >= 60 and played > 0:
            rate_per_hour = round(played * 3600.0 / elapsed, 1)
            remaining = goal - complete
            eta = dt.datetime.now(dt.timezone.utc) + dt.timedelta(
                seconds=remaining * elapsed / played)
            eta_at = utc_timestamp(eta.replace(microsecond=0))
    deadline = batch.get("deadline") if isinstance(batch.get("deadline"), dict) else None
    return {
        "schema": "continuous_batch_status/v1",
        "state_dir": str(state_root),
        "batch": batch["id"],
        "phase": batch["phase"],
        "complete_games": complete,
        "complete_seats": int(status["complete_seats"]),
        "goal_completed_games": goal,
        "remaining_games": goal - complete,
        "reserved_segments": len(batch["reservations"]),
        "next_seed": state["next_seed"],
        "publication": batch["publication"].get("stage", "not_started"),
        "publication_pr": batch["publication"].get("pr_number"),
        "source_commit": (batch.get("source") or {}).get("commit"),
        "scheduler_running": scheduler_is_running(state_root),
        "cut_request_pending": cut_request_path(state_root).exists(),
        "deadline_at": deadline.get("deadline_at") if deadline else None,
        "deadline_cutoff_at": deadline.get("cutoff_at") if deadline else None,
        "games_per_hour": rate_per_hour,
        "eta_at": eta_at,
    }


def print_status(state_root: Path, state: dict[str, Any], *, as_json: bool = False) -> None:
    report = status_report(state_root, state)
    if as_json:
        print(json.dumps(report, indent=2, sort_keys=True))
        return
    print("continuous completed-game scheduler")
    print(f"  batch: {report['batch']} ({report['phase']})")
    print(f"  complete: {report['complete_games']:,} games / {report['complete_seats']:,} seats")
    print(f"  boundary: {report['goal_completed_games']:,} validated completed games "
          f"({report['remaining_games']:,} remaining)")
    if report["games_per_hour"] is not None:
        print(f"  rate: {report['games_per_hour']:,} games/hour; boundary ETA {report['eta_at']}")
    print(f"  reserved segments: {report['reserved_segments']}; next seed: {report['next_seed']:,}")
    print(f"  source: {report['source_commit'] or 'not pinned yet'}")
    pr = f" (PR #{report['publication_pr']})" if report["publication_pr"] else ""
    print(f"  publication: {report['publication']}{pr}")
    print(f"  scheduler: {'running (lock held)' if report['scheduler_running'] else 'not running'}")
    if report["cut_request_pending"]:
        print("  cut request: pending, not yet adopted")
    if report["deadline_at"] is not None:
        suffix = f" (cut off at {report['deadline_cutoff_at']})" if report["deadline_cutoff_at"] else ""
        print(f"  deadline: {report['deadline_at']}{suffix}")


class OutcomeLog:
    """Print a tick outcome when it changes, not on every 5-second poll.

    After a daemon restart under a live segment every tick returns
    ``active_segment`` for hours; logging each one buries the lifecycle lines
    an operator reads (``segment_started``, ``cut_request_adopted``,
    ``frozen_deadline``, ``published``).  Repeats are counted and the count is
    reported when the outcome finally changes.
    """

    def __init__(self) -> None:
        self.last: str | None = None
        self.repeats = 0

    def note(self, outcome: str) -> str | None:
        """Return the line to print for ``outcome``, or ``None`` for a silent repeat."""
        if outcome == self.last:
            self.repeats += 1
            return None
        suffix = f" (previous outcome repeated {self.repeats} more times)" if self.repeats else ""
        self.last, self.repeats = outcome, 0
        return f"{utc_now()} {outcome}{suffix}"


def machine_from_config(repo: Path) -> str | None:
    result = subprocess.run(["git", "-C", str(repo), "config", "--get", "civvis.machine"],
                            text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False)
    return result.stdout.strip() or None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("run", "status", "cut", "publish"), nargs="?",
                        default="run",
                        help=("run: serve rotations (the launchd service); status: read-only "
                              "progress, safe while the service runs; cut: ask the running "
                              "service to freeze at --at/--in and stop; publish: open the "
                              "table PR for a frozen batch once (used after --no-publish)"))
    parser.add_argument("--state-dir", type=Path, required=True,
                        help="private durable scheduler state and batch rows")
    parser.add_argument("--repo", type=Path, default=ROOT,
                        help="clean main-management checkout used only to fetch/build detached sources")
    parser.add_argument("--seed-floor", type=int,
                        help="first seed, required only while creating a new state file")
    parser.add_argument("--goal-games", type=int, default=DEFAULT_GOAL_GAMES,
                        help="validated completed-game rotation boundary (default: 5000)")
    parser.add_argument("--deadline-at", metavar="UTC",
                        help=("absolute ISO-8601 cutoff for this initial batch; its raw "
                              "partial ledger is frozen and published at the deadline"))
    parser.add_argument("--next-goal-games", type=int,
                        help=("completed-game size for all rotations after --deadline-at; "
                              "required with a deadline"))
    parser.add_argument("--jobs", type=int,
                        help="game workers; default is floor(85%% of logical cores)")
    parser.add_argument("--publisher-agent", default="continuous-batch",
                        help="agent id for isolated table-publication tasks")
    parser.add_argument("--machine", help="fleet machine id; defaults to Git config")
    parser.add_argument("--no-publish", action="store_true",
                        help="freeze at the boundary instead of creating a publication PR")
    parser.add_argument("--once", action="store_true",
                        help="perform one durable transition instead of serving forever")
    parser.add_argument("--poll-seconds", type=float, default=DEFAULT_POLL_SECONDS,
                        help="retry delay after a clean interrupted segment")
    parser.add_argument("--json", action="store_true",
                        help="status: print one JSON object instead of text")
    parser.add_argument("--at", metavar="UTC",
                        help="cut: absolute ISO-8601 instant to stop at (default: now)")
    parser.add_argument("--in", dest="in_minutes", type=float, metavar="MINUTES",
                        help="cut: stop this many minutes from now")
    parser.add_argument("--note", help="cut: free-text reason kept in the deadline record")
    args = parser.parse_args(argv)
    try:
        if args.command == "status":
            state_root = args.state_dir.expanduser().resolve()
            print_status(state_root, read_state(state_root), as_json=args.json)
            return 0
        if args.command == "cut":
            state_root = args.state_dir.expanduser().resolve()
            if args.at is not None and args.in_minutes is not None:
                raise SchedulerError("cut takes --at or --in, not both")
            if args.in_minutes is not None and args.in_minutes < 0:
                raise SchedulerError("--in must not be negative")
            deadline_at = (parse_utc_timestamp(args.at, name="--at") if args.at is not None
                           else dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
                           + dt.timedelta(minutes=args.in_minutes or 0.0))
            state = read_state(state_root)
            batch = state["current"]
            if batch["phase"] != "running":
                raise SchedulerError(
                    f"batch {batch['id']} is {batch['phase']!r}; only a running batch can be cut")
            if batch_deadline(batch) is not None:
                raise SchedulerError("this batch already has a pending deadline")
            path = write_cut_request(state_root, deadline_at=deadline_at, note=args.note)
            running = scheduler_is_running(state_root)
            print(f"cut request written: {path}")
            print(f"  stop at: {utc_timestamp(deadline_at)}")
            print("  the running scheduler adopts it within a few seconds, stops the games at "
                  "that instant, freezes the validated prefix and exits awaiting publication."
                  if running else
                  "  WARNING: no scheduler holds this state directory; the request waits until "
                  "one starts.")
            print(f"  watch: python3 {Path(__file__).name} status --state-dir {state_root}")
            return 0
        goal = positive_int(args.goal_games, name="--goal-games")
        deadline_at = (parse_utc_timestamp(args.deadline_at, name="--deadline-at")
                       if args.deadline_at else None)
        next_goal_games = (positive_int(args.next_goal_games, name="--next-goal-games")
                           if args.next_goal_games is not None else None)
        if (deadline_at is None) != (next_goal_games is None):
            raise SchedulerError("--deadline-at and --next-goal-games must be used together")
        jobs = positive_int(args.jobs, name="--jobs") if args.jobs is not None else workers_for_cores(logical_cores())
        if args.poll_seconds <= 0:
            raise SchedulerError("--poll-seconds must be positive")
        state_root = args.state_dir.expanduser().resolve()
        repo = args.repo.expanduser().resolve()
        if not (repo / ".git").exists():
            raise SchedulerError(f"--repo is not a Git worktree: {repo}")
        lock_path = state_root / "scheduler.lock"
        lock_path.parent.mkdir(parents=True, exist_ok=True)
        with lock_path.open("a+") as lock:
            try:
                fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as error:
                raise SchedulerError("another scheduler process already owns this state directory") from error
            # State creation and its first seed reservation are inside the
            # lock. Two launchd restarts can therefore never both accept the
            # same fresh --seed-floor before either sees the other state file.
            state_file = state_root / "scheduler-state.json"
            state = load_state(
                state_file, seed_floor=args.seed_floor, goal_games=goal,
                deadline_at=deadline_at, next_goal_games=next_goal_games)
            if not state_file.exists():
                atomic_json(state_file, state)
            verify_deadline_invocation(
                state, deadline_at=deadline_at, next_goal_games=next_goal_games)
            machine = args.machine or machine_from_config(repo)
            publish = not args.no_publish
            once = args.once
            if args.command == "publish":
                phase = state["current"]["phase"]
                if phase not in {"frozen", "publishing"}:
                    raise SchedulerError(
                        f"batch {state['current']['id']} is {phase!r}; publish applies to a "
                        "frozen batch (cut or complete one first)")
                publish, once = True, True
            outcomes = OutcomeLog()
            while True:
                outcome = tick(
                    state_root, state_file, state, repo=repo, jobs=jobs,
                    machine=machine, agent=args.publisher_agent, publish=publish,
                )
                line = outcomes.note(outcome)
                if line is not None:
                    print(line, flush=True)
                if outcome == "published":
                    print("next: restart the scheduler service (launchctl kickstart -k gui/$UID/<label>) "
                          "so it rotates onto the merge commit and starts the next batch", flush=True)
                if once or outcome in {"awaiting_publication"}:
                    return 0
                if outcome == "partial_segment":
                    time.sleep(args.poll_seconds)
                if outcome == "active_segment":
                    time.sleep(ACTIVE_SEGMENT_POLL_SECONDS)
    except SchedulerError as error:
        print(f"continuous batch scheduler: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
