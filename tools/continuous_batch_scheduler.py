#!/usr/bin/env python3
"""Run and publish fail-closed 5,000-completed-game gene-screen rotations.

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
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Iterable

from continuous_screen_status import LedgerError, summarize, validate_analysis


ROOT = Path(__file__).resolve().parent.parent
SCHEMA = "continuous_batch_scheduler/v1"
CONTINUOUS_BATCH_TIMING_SCHEMA = "continuous_batch_timing/v1"

# ``tools/genes.py write`` deliberately updates the ranking table together
# with its supporting evidence.  Keep this one explicit list shared by the
# ownership claim, changed-path guard, and staging command: a new generated
# artifact must be reviewed here instead of being silently swept into a
# publication.
PUBLICATION_GENERATED_FILES = (
    "docs/gene_ledger.json",
    "GENE_HEURISTIC_RANKING.md",
    "docs/GENE_RANKING_EVIDENCE.md",
)
STANDARD_PLAYERS = 6
DEFAULT_GOAL_GAMES = 5_000
DEFAULT_POLL_SECONDS = 300.0
INITIAL_CHECK_TIMEOUT_SECONDS = 20 * 60.0


class SchedulerError(RuntimeError):
    """The scheduler cannot prove it is safe to advance."""


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace(
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


def new_state(seed_floor: int, goal_games: int = DEFAULT_GOAL_GAMES) -> dict[str, Any]:
    """Initial state. ``next_seed`` moves before a game process is launched."""
    positive_int(seed_floor, name="seed floor")
    positive_int(goal_games, name="goal games")
    return {
        "schema": SCHEMA,
        "goal_completed_games": goal_games,
        "next_seed": seed_floor,
        "current": new_batch(seed_floor, goal_games),
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
        if index and first <= previous_last:
            raise SchedulerError("state reservations overlap or are out of order")
        previous_last = last
    if reservations and state["next_seed"] <= previous_last:
        raise SchedulerError("state next_seed reuses a reserved seed")
    if not isinstance(state.get("history"), list):
        raise SchedulerError("state history must be a list")


def load_state(path: Path, *, seed_floor: int | None, goal_games: int) -> dict[str, Any]:
    if path.exists():
        try:
            state = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as error:
            raise SchedulerError(f"state {path} is invalid JSON: {error}") from error
        if not isinstance(state, dict):
            raise SchedulerError(f"state {path} is not an object")
        validate_state(state)
        if state["goal_completed_games"] != goal_games:
            raise SchedulerError(
                f"state has a {state['goal_completed_games']:,}-game boundary, not "
                f"the requested {goal_games:,}. Start a distinct state directory instead.")
        return state
    if seed_floor is None:
        raise SchedulerError(
            f"no state exists at {path}; pass --seed-floor once so its first range is explicit")
    return new_state(seed_floor, goal_games)


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
        "returncode": None,
    }
    batch["reservations"].append(reservation)
    # Persist this before launching. A crash now wastes seeds, never repeats them.
    state["next_seed"] = last + 1
    return reservation


def run_checked(command: Iterable[str], *, cwd: Path, description: str) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(list(command), cwd=cwd, text=True, capture_output=True, check=False)
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


def run_segment(state_root: Path, batch: dict[str, Any], reservation: dict[str, Any], *, jobs: int) -> int:
    """Run exactly the pre-reserved remaining game count, without profile overrides."""
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
        process = subprocess.Popen(command, cwd=Path(str(source["worktree"])), stdout=output,
                                   stderr=subprocess.STDOUT)
        try:
            returncode = process.wait()
        except KeyboardInterrupt:
            process.terminate()
            process.wait()
            raise
    reservation["returncode"] = returncode
    reservation["finished_at"] = utc_now()
    return returncode


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
        f"`{source['binary_sha256']}`. This publication changes no game rules or default genes.",
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
        "- [x] No game-rule or runtime-default change; soak is not applicable",
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
    result = run_checked(
        ["gh", "pr", "view", str(number), "--json", "state,mergedAt,mergeCommit"],
        cwd=worktree,
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
    if not worktree.is_dir():
        raise SchedulerError(f"publication worktree is missing: {worktree}")

    # An operator may merge the publication while this scheduler is stopped or
    # while it is retrying a prior stage. Check every durable post-claim stage
    # before touching the worktree: otherwise a merged ``claimed`` PR would be
    # regenerated and reach a needless "nothing to commit" failure instead of
    # permitting the next batch to start.
    if mark_published_if_already_merged(batch, publication, worktree=worktree, number=number):
        atomic_json(state_pathname, state)
        return

    if stage == "claimed":
        target = worktree / report
        target.parent.mkdir(parents=True, exist_ok=True)
        write_reporting_artifact(analysis_path(state_root, batch), target, batch)
        generated = list(PUBLICATION_GENERATED_FILES)
        write = [sys.executable, "tools/genes.py", "write", "--reporting-batch", report]
        first = subprocess.run(write, cwd=worktree, text=True, capture_output=True, check=False)
        if first.returncode != 0:
            reason = (
                f"The immutable {batch['id']} continuous result was built from clean "
                f"{batch['source']['commit']}; it is a report-only historical display batch, "
                "not a source of runtime defaults."
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
                    description="test publication source")
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
        "closed_at": utc_now(),
    })
    state["current"] = new_batch(state["next_seed"], state["goal_completed_games"])
    atomic_json(state_pathname, state)


def tick(state_root: Path, state_pathname: Path, state: dict[str, Any], *, repo: Path,
         jobs: int, machine: str | None, agent: str | None, publish: bool) -> str:
    """Advance one durable boundary: a segment, freeze, publication, or rotation."""
    batch = state["current"]
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
        returncode = run_segment(state_root, batch, reservation, jobs=jobs)
        atomic_json(state_pathname, state)
        latest = refresh_status(state_root, state)
        atomic_json(state_pathname, state)
        if latest["complete_games"] == batch["goal_completed_games"]:
            freeze_analysis(state_root, state, state_pathname=state_pathname)
            atomic_json(state_pathname, state)
            return "frozen"
        if returncode == 0:
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


def print_status(state_root: Path, state: dict[str, Any]) -> None:
    batch = state["current"]
    status = refresh_status(state_root, state)
    print("continuous 5,000-game scheduler")
    print(f"  batch: {batch['id']} ({batch['phase']})")
    print(f"  complete: {status['complete_games']:,} games / {status['complete_seats']:,} seats")
    print(f"  boundary: {batch['goal_completed_games']:,} validated completed games")
    print(f"  reserved segments: {len(batch['reservations'])}; next seed: {state['next_seed']:,}")
    print(f"  publication: {batch['publication'].get('stage', 'not_started')}")


def machine_from_config(repo: Path) -> str | None:
    result = subprocess.run(["git", "-C", str(repo), "config", "--get", "civvis.machine"],
                            text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False)
    return result.stdout.strip() or None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("run", "status"), nargs="?", default="run")
    parser.add_argument("--state-dir", type=Path, required=True,
                        help="private durable scheduler state and batch rows")
    parser.add_argument("--repo", type=Path, default=ROOT,
                        help="clean main-management checkout used only to fetch/build detached sources")
    parser.add_argument("--seed-floor", type=int,
                        help="first seed, required only while creating a new state file")
    parser.add_argument("--goal-games", type=int, default=DEFAULT_GOAL_GAMES,
                        help="validated completed-game rotation boundary (default: 5000)")
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
    args = parser.parse_args(argv)
    try:
        goal = positive_int(args.goal_games, name="--goal-games")
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
            state = load_state(state_file, seed_floor=args.seed_floor, goal_games=goal)
            if not state_file.exists():
                atomic_json(state_file, state)
            if args.command == "status":
                print_status(state_root, state)
                return 0
            machine = args.machine or machine_from_config(repo)
            while True:
                outcome = tick(
                    state_root, state_file, state, repo=repo, jobs=jobs,
                    machine=machine, agent=args.publisher_agent, publish=not args.no_publish,
                )
                print(f"{utc_now()} {outcome}", flush=True)
                if args.once or outcome in {"awaiting_publication"}:
                    return 0
                if outcome == "partial_segment":
                    time.sleep(args.poll_seconds)
    except SchedulerError as error:
        print(f"continuous batch scheduler: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
