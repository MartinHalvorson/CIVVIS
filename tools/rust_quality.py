#!/usr/bin/env python3
"""Run incremental Rust quality checks for files a change actually owns.

The repository has a large, historically formatted crate. A repository-wide
format gate makes every small change inherit unrelated backlog, while a
repository-wide clippy deny-warnings gate is blocked by warnings that predate
the change. This gate keeps the useful part of both checks scoped to the
LINES a change touched: format diffs overlapping those lines and clippy
warnings whose spans land on them fail the job. Whole-file matching was tried
first and inherited the backlog anyway — the AI hotspot files carry dozens of
standing warnings and format diffs, so any change to them failed for debt it
did not write. Existing debt remains visible in full local commands, but
cannot grow through a PR. A rustfmt that cannot physically run on the host —
`src/game.rs` needs about 24 GB — is skipped loudly rather than failed.

Usage:
    python3 tools/rust_quality.py --base <merge-base> --head <revision>
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from typing import Iterable


def run(repo: Path, args: list[str], *, check: bool = False) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=repo,
        text=True,
        encoding="utf-8",
        errors="replace",
        capture_output=True,
        check=check,
    )


def revision(repo: Path, value: str | None, fallback: str) -> str:
    """Return a usable revision, falling back when GitHub supplied no base."""
    candidate = (value or "").strip()
    if not candidate or set(candidate) == {"0"}:
        candidate = fallback
    checked = run(repo, ["git", "rev-parse", "--verify", f"{candidate}^{{commit}}"])
    if checked.returncode == 0:
        return checked.stdout.strip()
    fallback_checked = run(
        repo, ["git", "rev-parse", "--verify", f"{fallback}^{{commit}}"]
    )
    if fallback_checked.returncode != 0:
        raise RuntimeError(
            f"cannot resolve quality base {value!r} or fallback {fallback!r}: "
            f"{checked.stderr.strip()}"
        )
    return fallback_checked.stdout.strip()


def merge_base(repo: Path, base: str, head: str) -> str:
    """The revision this head's changes should be read against.

    On a pull_request event the workflow's head is GitHub's test **merge
    commit** — the branch merged into main *as of the run* — while
    `QUALITY_BASE` is the base tip frozen at the last synchronize event. On
    this trunk main moves every few minutes, so tip..merge-commit contains
    every line other PRs landed in between, and the gate blames them on
    whoever's run came next (observed on #1852: four format chunks in
    regions only main had edited, surviving a fresh synchronize). For a
    merge-commit head the honest base is its **first parent** — the exact
    main the merge was built against. A single-parent head (push event)
    falls back to the merge base with the supplied revision, and a clone
    that can answer neither keeps the supplied base unchanged.
    """
    first_parent = run(repo, ["git", "rev-parse", "--verify", f"{head}^2"])
    if first_parent.returncode == 0:
        merged_against = run(repo, ["git", "rev-parse", "--verify", f"{head}^1"])
        if merged_against.returncode == 0:
            return merged_against.stdout.strip()
    result = run(repo, ["git", "merge-base", base, head])
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip()
    return base


def changed_rust_files(repo: Path, base: str, head: str) -> list[Path]:
    result = run(
        repo,
        ["git", "diff", "--name-only", "--diff-filter=ACMR", base, head, "--", "*.rs"],
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "git diff failed")
    paths = []
    for line in result.stdout.splitlines():
        path = Path(line.strip())
        if path.suffix == ".rs" and (repo / path).is_file():
            paths.append(path)
    return sorted(set(paths))


def changed_line_ranges(repo: Path, base: str, head: str) -> dict[Path, list[tuple[int, int]]]:
    """Head-side line ranges each changed Rust file gained, keyed by resolved path.

    Both checks scope their findings to these ranges, so a change to a large,
    historically formatted file answers only for the lines it touched. A pure
    deletion contributes no head-side lines and therefore no range.
    """
    result = run(
        repo,
        ["git", "diff", "--unified=0", "--diff-filter=ACMR", base, head, "--", "*.rs"],
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "git diff failed")
    ranges: dict[Path, list[tuple[int, int]]] = {}
    current: Path | None = None
    for line in result.stdout.splitlines():
        if line.startswith("+++ b/"):
            current = (repo / line[len("+++ b/") :].strip()).resolve()
            continue
        if line.startswith("+++"):
            current = None
            continue
        if current is None or not line.startswith("@@"):
            continue
        try:
            added = line.split("+", 1)[1].split(" ", 1)[0]
        except IndexError:
            continue
        start, _, count = added.partition(",")
        length = int(count) if count else 1
        if length == 0:
            continue
        ranges.setdefault(current, []).append((int(start), int(start) + length - 1))
    return ranges


def _overlaps(ranges: list[tuple[int, int]], start: int, end: int) -> bool:
    return any(start <= high and end >= low for low, high in ranges)


_ANSI = re.compile("\x1b\\[[0-9;]*m|\x1b\\(B")


def _rustfmt_chunks(output: str) -> list[tuple[set[int], str]]:
    """Split `rustfmt --check` output into (changed-line-set, text) chunks.

    A chunk header is `Diff in <path>:<line>:`; the body mixes context lines
    with colored -/+ lines. Context and `-` lines consume positions in the
    file on disk — the same side of the diff `changed_line_ranges` reports —
    while `+` lines are only rustfmt's proposal. The set holds the on-disk
    lines rustfmt actually wants to change, so a chunk whose context merely
    brushes an edit does not drag that edit into standing debt beside it.
    """
    chunks: list[tuple[set[int], str]] = []
    cursor: int | None = None
    changed: set[int] = set()
    body: list[str] = []

    def flush() -> None:
        if body:
            chunks.append((set(changed), "\n".join(body)))

    for raw in output.splitlines():
        line = _ANSI.sub("", raw)
        if line.startswith("Diff in ") and line.rstrip().endswith(":"):
            flush()
            body[:] = [raw]
            changed.clear()
            try:
                cursor = int(line.rstrip().rstrip(":").rsplit(":", 1)[1])
            except (IndexError, ValueError):
                cursor = None
            continue
        if cursor is None:
            continue
        body.append(raw)
        if line.startswith("-"):
            changed.add(cursor)
            cursor += 1
        elif line.startswith("+"):
            # An insertion sits between on-disk lines; blame the position it
            # lands at without consuming one.
            changed.add(cursor)
        else:
            cursor += 1
    flush()
    return chunks


def rustfmt(
    repo: Path, paths: Iterable[Path], ranges: dict[Path, list[tuple[int, int]]]
) -> tuple[list[str], list[str]]:
    """Format failures overlapping the change, and loud skips.

    A rustfmt that dies instead of answering — the hosted runner cannot format
    `src/game.rs`, whose check needs about 24 GB — is reported as a skip, not a
    failure: the gate cannot demand a check the machine cannot physically run.
    """
    failures: list[str] = []
    skipped: list[str] = []
    for path in paths:
        # Formatting an out-of-line module declaration recursively formats all
        # of its children. `src/lib.rs` is the root of this crate, and
        # `src/foo.rs` owns children in `src/foo/`; either would make an
        # otherwise local parent edit inherit unrelated legacy formatting
        # diffs. Keep parent invocations scoped to themselves and check
        # changed child files independently.
        fmt_args = ["rustfmt", "--check", "--edition", "2021"]
        if path == Path("src/lib.rs") or (repo / path).with_suffix("").is_dir():
            fmt_args.extend(["--config", "skip_children=true"])
        result = run(repo, [*fmt_args, str(path)])
        if not result.returncode:
            continue
        detail = (result.stdout + result.stderr).strip()
        if result.returncode < 0 or "memory allocation of" in detail:
            skipped.append(
                f"{path}: rustfmt aborted (exit {result.returncode}) before it "
                "could answer; the file is too large for this runner's memory, "
                "so its formatting is not checked here"
            )
            continue
        file_ranges = ranges.get((repo / path).resolve(), [])
        chunks = _rustfmt_chunks(result.stdout)
        touched = [
            text
            for changed, text in chunks
            if any(_overlaps(file_ranges, line, line) for line in changed)
        ]
        if touched:
            failures.append(f"{path}:\n" + "\n".join(touched))
        elif not chunks:
            # Not a diff at all — an operational rustfmt error is a failure.
            failures.append(f"{path}:\n{detail}")
    return failures, skipped


def _span_paths(message: dict, repo: Path) -> set[Path]:
    paths = set()
    for span in message.get("spans", []):
        name = span.get("file_name")
        if not name:
            continue
        path = Path(name)
        if not path.is_absolute():
            path = repo / path
        try:
            paths.add(path.resolve())
        except OSError:
            paths.add(path)
    return paths


def _span_hits_change(
    message: dict, repo: Path, ranges: dict[Path, list[tuple[int, int]]]
) -> bool:
    """Whether any diagnostic span lands on a line the change touched.

    Matching whole files inherits their standing debt — `src/ai/advanced.rs`
    carries dozens of warnings that predate any given change — so a warning
    counts only when one of its spans overlaps a changed line range.
    """
    for span in message.get("spans", []):
        name = span.get("file_name")
        if not name:
            continue
        path = Path(name)
        if not path.is_absolute():
            path = repo / path
        try:
            path = path.resolve()
        except OSError:
            pass
        file_ranges = ranges.get(path)
        if not file_ranges:
            continue
        start = span.get("line_start")
        end = span.get("line_end", start)
        if start is None:
            continue
        if _overlaps(file_ranges, int(start), int(end or start)):
            return True
    return False


def clippy(
    repo: Path, ranges: dict[Path, list[tuple[int, int]]]
) -> tuple[list[str], str]:
    """Return changed-line diagnostics and raw output for compiler failures."""
    result = run(
        repo,
        [
            "cargo",
            "clippy",
            "--locked",
            "--all-targets",
            "--all-features",
            "--message-format=json",
            "--",
            "-W",
            "clippy::all",
        ],
    )
    diagnostics: list[str] = []
    for line in result.stdout.splitlines():
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        if payload.get("reason") != "compiler-message":
            continue
        message = payload.get("message", {})
        if message.get("level") != "warning":
            continue
        if not _span_hits_change(message, repo, ranges):
            continue
        code = (message.get("code") or {}).get("code") or "warning"
        rendered = (message.get("rendered") or message.get("message") or "").rstrip()
        diagnostics.append(f"{code}:\n{rendered}")

    raw = result.stdout + result.stderr
    return diagnostics, raw if result.returncode else ""


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--base", help="base revision, normally the PR merge base")
    parser.add_argument("--head", help="head revision, normally the workflow SHA")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    repo = args.repo.resolve()
    head = revision(repo, args.head or os.environ.get("QUALITY_HEAD"), "HEAD")
    base = revision(repo, args.base or os.environ.get("QUALITY_BASE"), f"{head}^")
    base = merge_base(repo, base, head)
    paths = changed_rust_files(repo, base, head)
    if not paths:
        print("rust-quality: no changed Rust files; format and clippy checks skipped")
        return 0

    print("rust-quality: checking " + ", ".join(map(str, paths)))
    ranges = changed_line_ranges(repo, base, head)
    failures, skipped = rustfmt(repo, paths, ranges)
    for notice in skipped:
        print(f"rust-quality: SKIPPED {notice}")
    clippy_failures, clippy_output = clippy(repo, ranges)
    failures.extend(clippy_failures)
    if clippy_output:
        print(clippy_output, file=sys.stderr)
        failures.append("cargo clippy failed to compile the changed revision")
    if failures:
        print("rust-quality: failed", file=sys.stderr)
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print("rust-quality: the changed lines are formatted and warning-free")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
