#!/usr/bin/env python3
"""Request a recorded, in-game retirement from the live Civ VI harness.

``civvis-games retire`` is deliberately a request, not a process kill.  The
active ``civ6_play`` process sees the sidecar below, writes an out-of-band
``retire`` order, and the installed control mod invokes Civilization VI's
native ``ACTION_RETIRE`` action.  Its ``retired`` acknowledgement is then
written into both the run summary and the ladder row.  Keeping the request on
disk gives the operator a durable audit trail and lets the harness refuse a
stale or foreign run instead of guessing which game to end.

This module contains the on-disk protocol and the narrow process-to-run
ownership check.  ``civ6_play.py`` translates the request into the native
control-mod order through ``civ6_control.orders``; no desktop menu automation
is involved.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"
REQUEST_FILE = "operator-retire-request.json"
RESULT_FILE = "operator-retire.json"
STATUS_FILE = "operator-retire-status.json"


class RetireRequestError(RuntimeError):
    """A requested retirement could not be safely bound to one live game."""


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _atomic_json(path: Path, payload: dict[str, Any]) -> None:
    """Replace one sidecar atomically, so readers never see half a request."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "w", encoding="utf-8", dir=path.parent,
            prefix=f".{path.name}.", suffix=".tmp", delete=False,
        ) as handle:
            temporary_name = handle.name
            json.dump(payload, handle, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary_name, path)
    except OSError:
        if temporary_name is not None:
            try:
                Path(temporary_name).unlink()
            except OSError:
                pass
        raise


def _read_json(path: Path) -> dict[str, Any] | None:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


def request_path(run_dir: Path) -> Path:
    return run_dir / REQUEST_FILE


def result_path(run_dir: Path) -> Path:
    return run_dir / RESULT_FILE


def status_path(run_dir: Path) -> Path:
    return run_dir / STATUS_FILE


def read_pending_request(run_dir: Path, tag: str) -> dict[str, Any] | None:
    """Return a request only when it belongs to this exact live run.

    The tag check is intentional.  Run directories live forever, while a
    harness tag is the ownership boundary shared by the control mod, player and
    climb.  An old request must never carry into a different game.
    """
    request = _read_json(request_path(run_dir))
    if request is None or request.get("tag") != tag:
        return None
    if result_path(run_dir).is_file():
        return None
    return request


def record_attempt(run_dir: Path, request: dict[str, Any], detail: str) -> None:
    """Expose a safe deferral without treating it as a completed retirement."""
    payload = {
        "tag": request.get("tag"),
        "requested_utc": request.get("requested_utc"),
        "state": "pending",
        "attempted_utc": utc_stamp(),
        "detail": detail,
    }
    _atomic_json(status_path(run_dir), payload)


def record_retired(run_dir: Path, request: dict[str, Any], detail: str) -> dict[str, Any]:
    """Persist the control mod's native in-game retirement acknowledgement."""
    payload = {
        "tag": request.get("tag"),
        "reason": request.get("reason"),
        "requested_utc": request.get("requested_utc"),
        "harness_pid": request.get("harness_pid"),
        "state": "retired",
        "retired_utc": utc_stamp(),
        "detail": detail,
    }
    _atomic_json(result_path(run_dir), payload)
    return payload


_TAG = re.compile(r"(?:^|\s)--tag(?:=|\s+)([A-Za-z0-9][A-Za-z0-9._-]*)")
_PYTHON = re.compile(r"(?:python|pypy)(?:[0-9]+(?:\.[0-9]+)*)?", re.IGNORECASE)


def _is_python_harness(command: str) -> bool:
    """Whether a process is the Python player, not a shell mentioning it."""
    try:
        argv = shlex.split(command)
    except ValueError:
        return False
    if not argv or _PYTHON.fullmatch(Path(argv[0]).name) is None:
        return False
    return any(Path(word).name == "civ6_play.py" for word in argv[1:])


def live_harnesses(ps_output: str | None = None) -> list[dict[str, Any]]:
    """Return the tagged Civ VI players visible in a ``ps`` listing.

    A global process name is not enough: an untagged player, a shell merely
    mentioning it, or two concurrent player processes is ambiguous.  The
    caller must decline rather than risk retiring somebody else's game.
    """
    if ps_output is None:
        try:
            done = subprocess.run(
                ["ps", "-axo", "pid=,command="], capture_output=True,
                text=True, timeout=10.0, check=False,
            )
        except (OSError, subprocess.SubprocessError) as error:
            raise RetireRequestError(f"could not list live harnesses: {error}") from error
        if done.returncode:
            raise RetireRequestError("could not list live harnesses")
        ps_output = done.stdout

    found: list[dict[str, Any]] = []
    for line in ps_output.splitlines():
        fields = line.strip().split(None, 1)
        if (len(fields) != 2 or not fields[0].isdigit()
                or not _is_python_harness(fields[1])):
            continue
        tagged = _TAG.search(fields[1])
        if tagged is None:
            continue
        found.append({"pid": int(fields[0]), "tag": tagged.group(1), "command": fields[1]})
    return found


def _has_recorded_turn(events: Path) -> bool:
    try:
        lines = events.read_text(errors="replace").splitlines()
    except OSError:
        return False
    for line in lines:
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if (isinstance(event, dict) and event.get("kind") == "turn"
                and isinstance(event.get("turn"), int)):
            return True
    return False


def request_active_run(root: Path, reason: str, *, ps_output: str | None = None,
                       now: str | None = None) -> dict[str, Any]:
    """Write one operator-retire request for exactly one proven live game."""
    harnesses = live_harnesses(ps_output)
    if not harnesses:
        raise RetireRequestError("no tagged civ6_play harness is active")
    if len(harnesses) != 1:
        tags = ", ".join(f"{item['tag']} (pid {item['pid']})" for item in harnesses)
        raise RetireRequestError(f"refusing to choose among {len(harnesses)} live harnesses: {tags}")

    harness = harnesses[0]
    tag = str(harness["tag"])
    run_dir = Path(root) / tag
    if not run_dir.is_dir():
        raise RetireRequestError(f"live harness {tag!r} has no run directory at {run_dir}")
    if (run_dir / "summary.json").is_file():
        raise RetireRequestError(f"run {tag!r} is already complete")
    if result_path(run_dir).is_file():
        raise RetireRequestError(f"run {tag!r} already has a recorded retirement")
    if request_path(run_dir).is_file():
        raise RetireRequestError(f"run {tag!r} already has a pending retirement request")
    if not _has_recorded_turn(run_dir / "events.jsonl"):
        raise RetireRequestError(f"run {tag!r} has no recorded turn; refusing to touch setup")

    request = {
        "tag": tag,
        "reason": reason,
        "requested_utc": now or utc_stamp(),
        "harness_pid": harness["pid"],
        "state": "requested",
    }
    _atomic_json(request_path(run_dir), request)
    return request


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    request = commands.add_parser("request", help="request retirement of one live run")
    request.add_argument("--runs-root", type=Path, default=RUN_ROOT)
    request.add_argument("--reason", required=True)
    args = parser.parse_args(argv)

    if args.command == "request":
        try:
            result = request_active_run(args.runs_root, args.reason)
        except RetireRequestError as error:
            print(f"retire request refused: {error}", file=sys.stderr)
            return 2
        print(f"retirement requested for {result['tag']} (harness pid {result['harness_pid']})")
        return 0
    return 64  # pragma: no cover - argparse owns invalid commands


if __name__ == "__main__":
    raise SystemExit(main())
