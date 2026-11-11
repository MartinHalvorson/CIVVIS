#!/usr/bin/env python3
"""Read a node CPU profile of the browser engine: where WebAssembly spends a site game.

`tools/profile_civvis.py` profiles the native build, and the native build is
not what a visitor runs. The published module is the same Rust compiled for
`wasm32`, and the two do not agree about what is expensive. On 2026-11-11 the
native profile of a site game put `memcmp` at 9.6% of self time and the ordered
maps well below it; the module's own profile put **39.7% of self time inside
`BTreeMap` and `BTreeSet`** and `memcmp` at 0.4%, because the comparison is
inlined into the tree search there and a tree walk is a chain of bounds-checked
loads with no wide compare to lean on. A lever chosen from the native table
would have been the wrong one.

By hand only: it needs a module built with function names, and a published
module has none (`wasm-opt` strips the name section).

    # a named module: no LTO and sixteen codegen units build in minutes
    CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \\
      cargo rustc --lib --locked --target wasm32-unknown-unknown --release \\
      --crate-type cdylib
    # the page's own frame loop, sampled by V8
    node --cpu-prof --cpu-prof-dir=/tmp/prof --cpu-prof-interval 500 \\
      tools/site_wasm_frames.mjs target/wasm32-unknown-unknown/release/civvis.wasm \\
      --turns 130 --seed 7
    tools/site_wasm_profile.py /tmp/prof

It prints three tables:

1. **Self time by function**, demangled far enough to read.
2. **Roll-ups** a per-function table hides: the ordered maps, the allocator,
   hashing, the memory primitives, formatting.
3. **The ordered maps by what they hold and what was asked of them.** A
   `BTreeMap` method is monomorphised per key and value type, so its symbol
   names both: `search` of a `Name` key is a tech, civic or building lookup,
   `iterate` over `Unit` is a walk of every unit in the world.

Shares are of sampled time, which for this driver is the engine: the module
imports nothing, so there is no host time to subtract.
"""

from __future__ import annotations

import argparse
import collections
import json
import re
import sys
from pathlib import Path
from typing import Counter, Dict, List, Sequence

sys.path.insert(0, str(Path(__file__).resolve().parent))

import profile_civvis  # noqa: E402

#: A symbol that is part of an ordered map's machinery, whatever it holds.
ORDERED_MAP = re.compile(r"btree|LeafOrInternal|BTreeMap|BTreeSet")

#: What the roll-up table adds up, in the order it prints them.
ROLLUPS = (
    ("ordered maps (BTreeMap, BTreeSet)", ORDERED_MAP),
    ("allocator (dlmalloc)", re.compile(r"dlmalloc|__rust_(?:alloc|dealloc|realloc)|__rdl_")),
    ("hashing", re.compile(r"SipHasher|Sip13Rounds|hash_key|hashbrown")),
    ("memcmp, bcmp", re.compile(r"memcmp|bcmp")),
    ("memcpy, memmove, memset", re.compile(r"memcpy|memmove|memset")),
    ("formatting", re.compile(r"\bfmt\b|format_inner")),
)

#: Types worth naming when they appear in an ordered map's symbol.
HELD = ("Name", "String", "Unit", "City", "Pos", "Tile", "Value", "Yields",
        "Player", "Item")

#: What was asked of the map, by a fragment of the method's name. First match
#: wins, so `search` is tried before the `next` a range search ends in.
OPERATIONS = (
    ("search", ("search", "find_key")),
    ("insert", ("insert", "split", "push")),
    ("remove", ("remove",)),
    ("clone", ("clone",)),
    ("drop", ("drop", "dying", "dealloc")),
    ("iterate", ("next", "leaf_edge", "full_range", "into_iter", "Iter",
                 "Range", "Keys", "Values")),
)


def identifiers(symbol: str) -> List[str]:
    """The length-prefixed identifiers of a Rust v0 symbol, in order.

    `6civvis4game4Unit` is `civvis`, `game`, `Unit`. Reading the length is
    what makes this exact: a pattern that takes "letters after a digit" runs
    `4Unit` on into the hash that follows it and never finds the type.
    """
    found: List[str] = []
    at = 0
    while at < len(symbol):
        digits = re.match(r"\d+", symbol[at:])
        if digits:
            length = int(digits.group(0))
            start = at + len(digits.group(0))
            if (0 < length < 80 and start + length <= len(symbol)
                    and re.match(r"[A-Za-z_]", symbol[start])):
                found.append(symbol[start:start + length])
                at = start + length
                continue
        at += 1
    return found


def readable(symbol: str) -> str:
    """A symbol as a path a person can read, by the native profiler's rules."""
    return profile_civvis.shorten(profile_civvis.demangle(symbol), 92)


def self_time(profile: dict) -> Counter[str]:
    """Microseconds whose innermost frame is each function."""
    names: Dict[int, str] = {
        node["id"]: node["callFrame"].get("functionName") or "(anonymous)"
        for node in profile.get("nodes", [])}
    totals: Counter[str] = collections.Counter()
    for node, delta in zip(profile.get("samples", []),
                           profile.get("timeDeltas", [])):
        totals[names.get(node, "(unknown)")] += delta
    return totals


def held_and_asked(symbol: str) -> str:
    """`search  Name`, `iterate  Unit/City`: what one map symbol was doing."""
    parts = identifiers(symbol)
    asked = next((label for label, fragments in OPERATIONS
                  if any(fragment in part for part in parts
                         for fragment in fragments)), "other")
    held = [part for part in dict.fromkeys(parts) if part in HELD]
    return "%-8s %s" % (asked, "/".join(held) if held else "a primitive key")


def ordered_maps(totals: Counter[str]) -> Counter[str]:
    by_kind: Counter[str] = collections.Counter()
    for symbol, spent in totals.items():
        if ORDERED_MAP.search(symbol):
            by_kind[held_and_asked(symbol)] += spent
    return by_kind


def newest_profile(where: Path) -> Path:
    if where.is_file():
        return where
    found = sorted(where.glob("*.cpuprofile"), key=lambda path: path.stat().st_mtime)
    if not found:
        raise SystemExit(
            "no .cpuprofile in %s. Run the driver under `node --cpu-prof "
            "--cpu-prof-dir=%s` first." % (where, where))
    return found[-1]


def report(totals: Counter[str], top: int = 30, out=sys.stdout) -> None:
    total = sum(totals.values())
    if not total:
        raise SystemExit("this profile holds no samples")
    share = lambda spent: 100.0 * spent / total
    named = sum(spent for symbol, spent in totals.items()
                if identifiers(symbol))
    print("%.1f s sampled" % (total / 1e6), file=out)
    if share(named) < 50.0:
        print("⚠ only %.0f%% of it lands in a named function. A published "
              "module has no name section; build one with names (see this "
              "file's header)." % share(named), file=out)

    print("\n== SELF  (share of sampled time)", file=out)
    for symbol, spent in totals.most_common(top):
        print("%7.2f%%  %s" % (share(spent), readable(symbol)), file=out)

    print("\n== ROLL-UPS  (self time; a per-function table hides these)", file=out)
    for label, pattern in ROLLUPS:
        spent = sum(value for symbol, value in totals.items()
                    if pattern.search(symbol))
        print("%7.2f%%  %s" % (share(spent), label), file=out)

    print("\n== ORDERED MAPS, BY WHAT THEY HOLD AND WHAT WAS ASKED", file=out)
    for kind, spent in ordered_maps(totals).most_common(min(top, 16)):
        print("%7.2f%%  %s" % (share(spent), kind), file=out)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__.splitlines()[0],
        formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("profile", type=Path,
                        help="a .cpuprofile, or the directory node wrote one to")
    parser.add_argument("--top", type=int, default=30)
    args = parser.parse_args(argv)
    path = newest_profile(args.profile)
    print("profile: %s" % path, file=sys.stderr)
    report(self_time(json.loads(path.read_text(encoding="utf-8"))), args.top)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
