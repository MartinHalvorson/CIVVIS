#!/usr/bin/env python3
"""Run incremental Rust quality checks for files a change actually owns.

The repository has a large, historically formatted crate. A repository-wide
format gate makes every small change inherit unrelated backlog, while a
repository-wide clippy deny-warnings gate is blocked by warnings that predate
the change. This gate keeps the useful part of both checks: changed Rust files
must be formatted, and clippy warnings whose spans land in those files fail
the job. Existing debt remains visible in full local commands, but cannot grow
through a PR.

Usage:
    python3 tools/rust_quality.py --base <merge-base> --head <revision>
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
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


def rustfmt(repo: Path, paths: Iterable[Path]) -> list[str]:
    failures: list[str] = []
    for path in paths:
        result = run(repo, ["rustfmt", "--check", "--edition", "2021", str(path)])
        if result.returncode:
            detail = (result.stdout + result.stderr).strip()
            failures.append(f"{path}:\n{detail}")
    return failures


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


def clippy(repo: Path, paths: Iterable[Path]) -> tuple[list[str], str]:
    """Return changed-file diagnostics and raw output for compiler failures."""
    changed = {(repo / path).resolve() for path in paths}
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
        if not (_span_paths(message, repo) & changed):
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
    paths = changed_rust_files(repo, base, head)
    if not paths:
        print("rust-quality: no changed Rust files; format and clippy checks skipped")
        return 0

    print("rust-quality: checking " + ", ".join(map(str, paths)))
    failures = rustfmt(repo, paths)
    clippy_failures, clippy_output = clippy(repo, paths)
    failures.extend(clippy_failures)
    if clippy_output:
        print(clippy_output, file=sys.stderr)
        failures.append("cargo clippy failed to compile the changed revision")
    if failures:
        print("rust-quality: failed", file=sys.stderr)
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print("rust-quality: changed Rust files are formatted and warning-free")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
