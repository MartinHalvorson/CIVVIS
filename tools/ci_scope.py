#!/usr/bin/env python3
"""Classify a pull-request diff for CIVVIS's fast validation lane.

Only documentation, repository tooling, and top-level Markdown are allowed to
skip Rust compilation.  Every other path defaults to the full Rust suite.  The
allowlist is deliberately narrow: a new kind of tracked file must pay for the
full suite until somebody has explicitly shown that it cannot affect the Rust
artifact.

The workflow consumes the one-line output through ``GITHUB_OUTPUT``:

    git diff --name-only -z BASE...HEAD | python3 tools/ci_scope.py --paths0
"""

from __future__ import annotations

import argparse
import sys
from collections.abc import Iterable


# The tooling suite in collaboration-policy.yml covers every ``tools/test_*.py``
# file, and cargo-test still runs its repository-integrity ratchets for this
# lane.  These are therefore safe *candidates* for omitting only Rust's clean
# compilation and nextest run.  Everything outside this small set is fail-closed
# into the Rust suite.
FAST_PREFIXES = ("docs/", "tools/")


def is_fast_path(path: str) -> bool:
    """Whether ``path`` is eligible to omit Rust compilation.

    Git itself produces normalized relative paths, but reject malformed input
    here as well: this classifier protects a merge gate and must never turn an
    unexpected path spelling into a fast-lane decision.
    """
    normalized = path.replace("\\", "/")
    if (
        not normalized
        or normalized.startswith("/")
        or normalized.startswith("../")
        or "/../" in normalized
        or normalized.endswith("/..")
    ):
        return False
    if normalized.startswith(FAST_PREFIXES):
        return True
    return "/" not in normalized and normalized.endswith(".md")


def requires_rust_gate(paths: Iterable[str]) -> bool:
    """Return whether any changed path requires the full Rust suite.

    An empty or malformed file list is not evidence that Rust is irrelevant.
    Treat it as full scope rather than letting an Actions checkout, git diff,
    or future workflow edit accidentally convert a real change into a bypass.
    """
    changed = list(paths)
    return not changed or any(not is_fast_path(path) for path in changed)


def read_paths(data: bytes, *, nul_delimited: bool) -> list[str]:
    """Decode Git's path stream, preserving unusual valid filenames."""
    separator = b"\0" if nul_delimited else b"\n"
    return [
        item.decode("utf-8", "surrogateescape")
        for item in data.split(separator)
        if item
    ]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--paths0",
        action="store_true",
        help="read NUL-delimited paths from stdin (the git-safe form)",
    )
    args = parser.parse_args(argv)
    paths = read_paths(sys.stdin.buffer.read(), nul_delimited=args.paths0)
    print("rust_gate=" + ("true" if requires_rust_gate(paths) else "false"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
