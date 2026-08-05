#!/usr/bin/env python3
"""Retired real-Civ6 war/no-war paired experiment.

Civilization VI's requested seed values do not currently reproduce the map,
start, or civilization. Running this former harness would label unrelated games
as a paired treatment comparison.
"""

from __future__ import annotations

import sys


def main() -> int:
    print(
        "civ6_ab.py is retired: real-Civ6 worlds are not reproducible, so this "
        "cannot produce a paired result. Run tools/civ6_seed_check.py after any "
        "seed-channel repair, then write a new preregistered experiment.",
        file=sys.stderr,
    )
    return 2


if __name__ == "__main__":
    sys.exit(main())
