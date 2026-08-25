#!/usr/bin/env python3
"""Read the live run ledger from the `ledger` branch, on any machine.

The seat that plays Civilization VI appends every finished run — its
`summary.json` and gzipped `events.jsonl` — to an append-only orphan branch
of this repository (`civ6_ladder.py publish-run`, called by `civ6_play.py` the
moment a summary is written). This is the reader for a machine that never sat
beside that runs directory:

    python tools/live_ledger.py pull            # origin/ledger -> ~/.cache/civvis/ledger/
    python tools/live_ledger.py runs --last 10  # the newest runs, one row each

`pull` needs no worktree and checks nothing out: it fetches the branch tip
and copies each run it has not yet seen with `git show`. The cache is laid out
exactly as the branch is (`runs/<tag>/summary.json`, `runs/<tag>/events.jsonl.gz`),
so a tool that reads a local runs directory reads the pulled ledger the same way.
"""

from __future__ import annotations

import argparse
import gzip
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_ladder  # noqa: E402

CACHE_DEFAULT = Path.home() / ".cache" / "civvis" / "ledger"


def _git_bytes(repo: Path, *args: str, env: dict | None = None) -> bytes:
    result = subprocess.run(["git", "-C", str(repo), *args],
                            capture_output=True, check=False,
                            env={**os.environ, **(env or {})})
    if result.returncode != 0:
        raise RuntimeError(
            f"git {' '.join(args)} failed ({result.returncode}): "
            f"{result.stderr.decode(errors='replace').strip()}")
    return result.stdout


def pull(cache: Path = CACHE_DEFAULT, *, repo: Path | None = None,
         remote: str = "origin", branch: str = civ6_ladder.LEDGER_BRANCH,
         env: dict | None = None) -> list[str]:
    """Copy every run on the ledger branch the cache lacks. Returns the new tags."""
    repo = Path(repo or civ6_ladder.REPO)
    cache = Path(cache)
    tip = civ6_ladder.ledger_tip(repo, remote, branch, env=env)
    if tip is None:
        raise RuntimeError(f"{remote} has no `{branch}` branch yet")
    listing = _git_bytes(repo, "ls-tree", "-r", "--name-only", tip, env=env)
    fresh: list[str] = []
    for line in listing.decode().splitlines():
        parts = line.split("/")
        if len(parts) != 3 or parts[0] != "runs":
            continue
        target = cache / line
        if target.is_file():
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(_git_bytes(repo, "show", f"{tip}:{line}", env=env))
        if parts[2] == "summary.json":
            fresh.append(parts[1])
    (cache / "TIP").write_text(tip + "\n")
    return fresh


def run_dirs(root: Path) -> list[Path]:
    """Run directories under a pulled ledger cache OR a live runs directory."""
    root = Path(root)
    base = root / "runs" if (root / "runs").is_dir() else root
    return [path.parent for path in base.glob("*/summary.json")]


def events_path(run_dir: Path) -> Path | None:
    """The run's events, plain or gzipped, whichever the directory holds."""
    for name in ("events.jsonl", "events.jsonl.gz"):
        if (run_dir / name).is_file():
            return run_dir / name
    return None


def open_events(path: Path):
    """Text handle over events.jsonl or events.jsonl.gz."""
    if path.suffix == ".gz":
        return gzip.open(path, "rt")
    return path.open()


def summaries(root: Path, last: int | None = None) -> list[dict]:
    """Summaries under `root`, oldest first (newest `last` when given).
    Each carries `_dir`, the directory it was read from."""
    rows = []
    for run_dir in run_dirs(root):
        try:
            body = json.loads((run_dir / "summary.json").read_text())
        except (OSError, json.JSONDecodeError):
            continue
        if not isinstance(body, dict):
            continue
        body["_dir"] = run_dir
        rows.append(body)

    def stamp(body: dict) -> str:
        return body.get("finished_utc") or datetime.fromtimestamp(
            (body["_dir"] / "summary.json").stat().st_mtime,
            tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    rows.sort(key=stamp)
    return rows[-last:] if last else rows


def deals_cell(deals: dict | None) -> str:
    if not deals:
        return "-"
    return (f"s{deals.get('sessions_opened', 0)}/"
            f"a{deals.get('sessions_answered', 0)}/"
            f"u{deals.get('sessions_unanswered', 0)} "
            f"c{deals.get('closed', 0)} d{deals.get('declined', 0)} "
            f"e{deals.get('expired', 0)} "
            f"p+{deals.get('peace_accepted', 0)}/-{deals.get('peace_refused', 0)}"
            + (" stood_down" if deals.get("stood_down") else ""))


def run_row(body: dict) -> list[str]:
    outcome = body.get("outcome") or {}
    victory = civ6_ladder.victory_type(body) or outcome.get("kind") or "-"
    if victory and civ6_ladder.is_win(body):
        victory = f"WON {victory}"
    applied = civ6_ladder.applied_pct(body)
    return [
        str(body.get("tag") or body["_dir"].name),
        str(body.get("finished_utc") or "-"),
        civ6_ladder.NAMES.get(body.get("difficulty"), str(body.get("difficulty") or "-")),
        str(body.get("last_turn") if body.get("last_turn") is not None else "-"),
        str(body.get("last_score") if body.get("last_score") is not None else "-"),
        str(body.get("rival_best") if body.get("rival_best") is not None else "-"),
        str(victory),
        f"{applied:.1f}%" if applied is not None else "-",
        deals_cell(body.get("deals")),
    ]


HEADER = ["tag", "finished", "difficulty", "turns", "score", "rival_best",
          "victory", "applied", "deals"]


def table(rows: list[list[str]], header: list[str] = HEADER) -> str:
    widths = [max(len(r[i]) for r in [header, *rows]) for i in range(len(header))]
    lines = ["  ".join(cell.ljust(widths[i]) for i, cell in enumerate(header))]
    for row in rows:
        lines.append("  ".join(cell.ljust(widths[i]) for i, cell in enumerate(row)))
    return "\n".join(lines)


def runs_table(root: Path, last: int) -> str:
    return table([run_row(body) for body in summaries(root, last)])


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--cache", type=Path, default=CACHE_DEFAULT,
                    help="where pulled runs live (default ~/.cache/civvis/ledger)")
    sub = ap.add_subparsers(dest="command", required=True)
    pl = sub.add_parser("pull", help="fetch origin/ledger into the cache")
    pl.add_argument("--remote", default="origin")
    pl.add_argument("--branch", default=civ6_ladder.LEDGER_BRANCH)
    rn = sub.add_parser("runs", help="one row per run, newest last")
    rn.add_argument("--last", type=int, default=10)
    rn.add_argument("--runs", type=Path, default=None,
                    help="read a live runs directory instead of the cache")
    args = ap.parse_args(argv)
    if args.command == "pull":
        fresh = pull(args.cache, remote=args.remote, branch=args.branch)
        print(f"{len(fresh)} new run(s) -> {args.cache}")
        for tag in fresh:
            print(f"  {tag}")
        return 0
    print(runs_table(args.runs or args.cache, args.last))
    return 0


if __name__ == "__main__":
    sys.exit(main())
