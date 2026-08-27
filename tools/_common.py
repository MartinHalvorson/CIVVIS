#!/usr/bin/env python3
"""Small helpers shared by tools/ scripts.

`tools/` grew to 139 scripts with no shared module: dozens of them
`sys.path.insert` the tools directory and re-derive the same handful of
things -- where the repo root is, how to shell out to `git` and swallow its
failures the same way, and how to find the most recent run directory under a
runs root. This module exists so those get written once.

It is imported the same way every other tool-to-tool import in this
directory works (see how 16 files import `civ6_env`): the importer inserts
`tools/` onto `sys.path` before importing, so this file has no import-time
dependency on being run from a particular working directory.

    import sys
    from pathlib import Path
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import _common  # noqa: E402

This module is intentionally small and dependency-free (standard library
only) so it stays safe to import from anywhere in `tools/`, including from
scripts invoked as plain `python3 tools/x.py` with no package context.
"""

from __future__ import annotations

import datetime
import json
import subprocess
from pathlib import Path


def repo_root() -> Path:
    """Return the repository root (the parent of the `tools/` directory).

    Computed from this file's own location -- the same
    `Path(__file__).resolve().parent.parent` convention every standalone
    tool in this directory already uses -- rather than shelling out to git,
    so it works even when a script is run outside a git checkout (e.g. from
    an extracted archive) and never blocks on the git executable.
    """
    return Path(__file__).resolve().parent.parent


def run(
    cmd,
    *,
    cwd: Path | str | None = None,
    check: bool = True,
    capture: bool = True,
    env: dict | None = None,
) -> subprocess.CompletedProcess:
    """Run a subprocess in text mode and return its `CompletedProcess`.

    A thin wrapper around `subprocess.run` with this fleet's common
    defaults: `text=True` always, `capture_output=True` unless
    `capture=False` (to let a long-running child stream to the parent's own
    stdout/stderr), and `check=True` by default so a nonzero exit raises
    `CalledProcessError` unless the caller opts out.
    """
    kwargs: dict = {"text": True, "cwd": cwd, "env": env, "check": check}
    if capture:
        kwargs["capture_output"] = True
    return subprocess.run(cmd, **kwargs)


def git(*args: str, cwd: Path | str | None = None) -> str:
    """Run a git command and return its stdout.

    Mirrors the idiom already duplicated in `tools/overwrite_guard.py` and
    `tools/stranded_work_report.py`: `check=False`, UTF-8 decoding with
    `errors="replace"`, and the raw stdout is returned even on failure
    (often empty) rather than raising -- callers that treat "no output" as
    "no answer" keep working unchanged.
    """
    return subprocess.run(
        ["git", *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
        cwd=cwd,
    ).stdout


def newest_run(run_dir: Path | str, pattern: str) -> Path | None:
    """Return the most recently modified immediate subdirectory of
    `run_dir` that contains a file named `pattern`, ranked by that file's
    mtime -- or `None` if no subdirectory qualifies.

    Matches the shape duplicated in `tools/civ6_watchdogs.py` and
    `tools/civ6_civvis_status.py`, both of which scan a runs root
    (`~/civvis-civ6-runs/control`) for the newest run directory that has
    already written an `events.jsonl`.
    """
    run_dir = Path(run_dir)
    candidates = [p for p in run_dir.iterdir() if (p / pattern).exists()]
    if not candidates:
        return None
    return max(candidates, key=lambda p: (p / pattern).stat().st_mtime)


def read_events(path: Path | str) -> list:
    """Parse a JSON-lines events file into a list of dicts, skipping any
    line that fails to parse.

    `path` may be the events file itself, or a run directory containing
    `events.jsonl` (the layout `~/civvis-civ6-runs/control/<run>/` uses) --
    a directory is resolved to `<path>/events.jsonl` automatically.
    """
    path = Path(path)
    if path.is_dir():
        path = path / "events.jsonl"
    events = []
    for line in path.read_text(errors="replace").splitlines():
        try:
            events.append(json.loads(line))
        except ValueError:
            continue
    return events


def utc_now() -> datetime.datetime:
    """Return the current time as a timezone-aware UTC `datetime`.

    Matches the idiom already used throughout `tools/` (`datetime.now(
    timezone.utc)`) in preference to the deprecated, naive
    `datetime.utcnow()`.
    """
    return datetime.datetime.now(datetime.timezone.utc)
