#!/usr/bin/env python3
"""Report rules-data effect keys the engine never reads.

CIVVIS' data files name effects; the engine consumes them by string. Nothing
enforces the join, so a key can sit in ``data/*.json`` doing nothing -- because
it was mistyped, because the code that read it was refactored away, or because
a rebase in a shared checkout dropped the consumer and left the data behind.
That last case is not hypothetical: the Sphinx's Floodplains Culture and
Wonder-adjacency Faith survived in data for fourteen iterations after their
engine arm was lost, and no test noticed.

Usage::

    python tools/civvis_inert.py            # report
    python tools/civvis_inert.py --max 0    # CI ratchet

Both directions, because only one of them was ever checked. The audit above
asks "which data key does the engine ignore"; ``founder_belief_yields`` asks the
mirror question, "which key does the engine price that no data supplies", and
that direction had four dead arms in the belief economy on 2026-08-18. Three of
them were dead because a belief was wired to the *domestic* form of a key that
Civilization VI defines on foreign followers, so the engine's correct arm sat
unreachable while the belief paid the wrong thing. See ``docs/FIDELITY.md``.

A key counts as consumed when the engine contains it as a string literal, or
when a ``format!`` template could build it. The template check is deliberately
loose in one direction: a single-segment placeholder such as ``"{prefix}food"``
matches any key ending in ``food``, so a key that only *looks* like one of
those will be credited. It is tight in the other -- a template must carry at
least three literal characters, so a bare ``"{}"`` credits nothing.
"""

from __future__ import annotations

import argparse
import collections
import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
WAIVERS = Path(__file__).resolve().parent / "inert_waivers.json"

# Keys nested under these names are effect keys rather than data structure.
EFFECT_HOLDERS = ("effects", "adjacency", "adjacent_yields", "great_person_points")

# governments.json deserialises into the GovEffects struct by field name, so
# its keys are checked by the compiler rather than by string lookup.
SKIP_FILES = {"governments.json"}


def effect_keys() -> dict[str, set[str]]:
    keys: dict[str, set[str]] = collections.defaultdict(set)

    def walk(node, source: str) -> None:
        if isinstance(node, dict):
            for key, value in node.items():
                if key in EFFECT_HOLDERS:
                    if isinstance(value, dict):
                        for effect in value:
                            keys[effect].add(source)
                    continue
                walk(value, source)
        elif isinstance(node, list):
            for value in node:
                walk(value, source)

    for path in sorted((REPO / "data").glob("*.json")):
        if path.name in SKIP_FILES:
            continue
        walk(json.loads(path.read_text(encoding="utf-8")), path.name)
    return keys


def engine_source() -> str:
    return "".join(
        path.read_text(encoding="utf-8", errors="replace")
        for path in sorted((REPO / "src").rglob("*.rs"))
    )


# The engine function whose effect lookups are checked in the reverse direction.
# One named function rather than a hand-written key list: the list would be
# complete the day it was written, and this repository has paid for that three
# times. Renaming the function fails loudly below rather than silently checking
# nothing.
BELIEF_YIELD_FN = "fn founder_belief_yields"


def belief_yield_keys(source: str) -> set[str]:
    """Effect keys ``founder_belief_yields`` prices.

    ⚠ A census that returns nothing is a broken census. The caller raises when
    this comes back empty, because "the engine reads no keys" is never the
    truth and would make the whole check pass by not running.
    """
    start = source.find(BELIEF_YIELD_FN)
    if start < 0:
        raise SystemExit(
            f"civvis_inert: {BELIEF_YIELD_FN!r} is gone; the reverse audit is "
            "reading nothing. Point BELIEF_YIELD_FN at its replacement."
        )
    body = source[start:]
    end = body.find("\n    fn ", 1)
    if end > 0:
        body = body[:end]
    keys = set(re.findall(r'\beffect\(\s*"([a-z0-9_]+)"', body))
    if not keys:
        raise SystemExit(
            "civvis_inert: found the belief-yield function and no effect keys "
            "in it; the scrape broke rather than finding an empty function"
        )
    return keys


def format_templates(source: str) -> list[re.Pattern]:
    patterns = []
    for template in re.findall(r'format!\(\s*"([^"]+)"', source):
        literal = re.sub(r"\{[^}]*\}", "", template)
        if "{" not in template or len(literal) < 3:
            continue
        body = re.sub(r"\{[^}]*\}", "[a-z0-9_]+", re.escape(template).replace(r"\{", "{").replace(r"\}", "}"))
        patterns.append(re.compile(f"^{body}$"))
    return patterns


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--max", type=int, default=None, help="exit 1 above this many")
    args = parser.parse_args()

    source = engine_source()
    templates = format_templates(source)
    waivers = (
        json.loads(WAIVERS.read_text(encoding="utf-8"))["waivers"] if WAIVERS.exists() else {}
    )

    inert = []
    keys = effect_keys()
    for key, sources in sorted(keys.items()):
        if f'"{key}"' in source or any(pattern.match(key) for pattern in templates):
            continue
        if key in waivers:
            continue
        inert.append((key, sorted(sources)))

    print(f"{len(keys)} effect keys, {len(inert)} with no consumer, {len(waivers)} waived")
    for key, sources in inert:
        print(f"  {key:44} {', '.join(sources)}")

    # The mirror question. A key the engine prices and no data supplies is an
    # arm that cannot fire — the same defect from the other side, and until
    # 2026-08-18 nothing asked it.
    priced = belief_yield_keys(source)
    unsupplied = sorted(key for key in priced if key not in keys)
    print(
        f"{len(priced)} belief-yield keys priced by the engine, "
        f"{len(unsupplied)} supplied by no belief"
    )
    for key in unsupplied:
        print(f"  {key:44} no belief sets this")
    if args.max is not None and len(unsupplied) > args.max:
        print(
            f"FAIL: {len(unsupplied)} belief-yield keys exceed the ratchet of {args.max}",
            file=sys.stderr,
        )
        return 1

    if args.max is not None and len(inert) > args.max:
        print(f"FAIL: {len(inert)} exceeds the ratchet of {args.max}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
