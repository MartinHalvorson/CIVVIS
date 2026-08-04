#!/usr/bin/env python3
"""Retired real-Civ6 settlement-window paired experiment.

The static settlement-plan route and its near-window treatment depended on a
reproducible real-Civ6 world. That prerequisite is currently false.
"""

from __future__ import annotations

import sys


def main() -> int:
    print(
        "civ6_near_ab.py is retired: the static settle-plan path was removed and "
        "real-Civ6 worlds are not reproducible. Run tools/civ6_seed_check.py after "
        "a seed-channel repair, then write a new preregistered experiment.",
        file=sys.stderr,
    )
    return 2


if __name__ == "__main__":
    sys.exit(main())
