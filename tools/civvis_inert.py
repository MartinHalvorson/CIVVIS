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
mirror question, "which key does the engine price that no data supplies".

⚠ An unsupplied arm is not automatically a defect here, and #2049 is the
expensive proof. Three of them are the **base-game** forms of Founder beliefs
that Gathering Storm replaces outright — Vanilla pays Tithe per follower and
World Church and Pilgrimage on what is foreign, and
``Expansion2_RemoveData.xml`` deletes all three modifiers. CIVVIS models
Gathering Storm, so the data supplies the per-city and per-follower forms and
the base-game arms stay unreachable **on purpose**. They are waived by name in
``BELIEF_YIELD_WAIVERS`` with that reason, so a genuinely new arm with no belief
behind it still fails. See ``docs/FIDELITY.md``.

Three more questions, all the same defect: a name that is spelled in one place
and looked up in another, with nothing to say the two disagree.

- ``duplicate_keys``: a key written twice in one JSON object. ``serde_json``
  keeps the last and says nothing, so the first copy reads like data and is
  not. Four improvements carried a stale first copy for three months.
- ``name_literals``: a ``name!("x")`` naming something no data file contains.
  ``name!`` interns any text it is given, so ``name!("theater")`` compiled, ran
  and never equalled ``theater_square``.
- ``effect_reads``: an effect key the engine looks up and no data file carries.
  It reads 0 for ever, so it looks exactly like an effect that is merely small.

Each has a waiver section in ``inert_waivers.json``. A waiver states why the
name is right, and a waiver for a name nothing reads any more fails too.

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

# Belief-yield arms with no belief behind them, and why that is correct.
#
# ⚠⚠ EVERY ENTRY MUST NAME THE RULESET REASON. This list is the one place a
# reader can check that an unreachable arm is deliberate, and #2049 shipped the
# base game's belief values into a Gathering Storm ruleset precisely because
# nothing wrote that distinction down.
BELIEF_YIELD_WAIVERS = {
    "gold_per_followers": (
        "Vanilla Tithe (TITHE_GOLD_FOLLOWER, +1 Gold per 4 followers). "
        "Expansion2_RemoveData.xml deletes it; Gathering Storm's Tithe is "
        "TITHE_GOLD_CITY, +3 Gold per following city, which `tithe` supplies."
    ),
    "culture_per_foreign_followers": (
        "Vanilla World Church (WORLD_CHURCH_CULTURE_FOREIGN_FOLLOWER, +1 "
        "Culture per 5 foreign followers). Deleted by Expansion2; Gathering "
        "Storm's is WORLD_CHURCH_CULTURE_FOLLOWER, +1 per 4 followers."
    ),
    "faith_per_foreign_city": (
        "Vanilla Pilgrimage (PILGRIMAGE_FAITH_FOREIGN_CITY, +2 Faith per "
        "foreign following city). Deleted by Expansion2; Gathering Storm's is "
        "PILGRIMAGE_FAITH_CITY, +2 Faith per following city."
    ),
}


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


def duplicate_keys(root: Path | None = None) -> list[str]:
    """Every key written twice in one JSON object, as ``file: key``.

    ``json.loads`` keeps the last value just as ``serde_json`` does, so this
    has to look at the pairs before they become a dict.
    """
    root = root or REPO / "data"
    paths = sorted(root.rglob("*.json"))
    if not paths:
        raise SystemExit(f"civvis_inert: no JSON under {root}; the check would pass by not running")
    found: list[str] = []
    for path in paths:

        def note(pairs: list) -> dict:
            counts = collections.Counter(key for key, _ in pairs)
            found.extend(f"{path.relative_to(root)}: {key}" for key, n in counts.items() if n > 1)
            return dict(pairs)

        json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=note)
    return found


def data_strings(root: Path | None = None) -> set[str]:
    """Every object key and string value in the top-level data files."""
    strings: set[str] = set()

    def walk(node) -> None:
        if isinstance(node, dict):
            for key, value in node.items():
                strings.add(key)
                walk(value)
        elif isinstance(node, list):
            for value in node:
                walk(value)
        elif isinstance(node, str):
            strings.add(node)

    for path in sorted((root or REPO / "data").glob("*.json")):
        walk(json.loads(path.read_text(encoding="utf-8")))
    if not strings:
        raise SystemExit("civvis_inert: the data files held no strings; the scrape broke")
    return strings


INLINE_TEST_MODULE = re.compile(r"#\[cfg\(test\)\]\s*(?:pub(?:\([a-z]+\))?\s+)?mod\s+\w+\s*\{")
TEST_MODULE_FILE = re.compile(
    r"#\[cfg\(test\)\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([a-z]+\))?\s+)?mod\s+(\w+)\s*;"
)


def production_sources(root: Path | None = None) -> dict[str, str]:
    """Rust source that ships, keyed by repo-relative path, comments removed.

    Test code is dropped because a test may name a fixture nothing else has.
    Three shapes of it: a file named for tests, a module a ``#[cfg(test)] mod x;``
    declares (``src/game/victory_conditions.rs`` is one, and its name does not
    say so), and an inline ``#[cfg(test)] mod x { .. }``, which runs to the end
    of its file by this repository's convention.
    """
    root = root or REPO / "src"
    files = {path: path.read_text(encoding="utf-8", errors="replace") for path in root.rglob("*.rs")}
    if not files:
        raise SystemExit(f"civvis_inert: no Rust under {root}; the check would pass by not running")
    test_modules = {name for text in files.values() for name in TEST_MODULE_FILE.findall(text)}
    sources = {}
    for path, text in sorted(files.items()):
        if path.stem == "tests" or path.stem.endswith("_tests") or "tests" in path.parts:
            continue
        if path.stem in test_modules:
            continue
        inline = INLINE_TEST_MODULE.search(text)
        if inline:
            text = text[: inline.start()]
        kept = "\n".join("" if line.lstrip().startswith("//") else line for line in text.split("\n"))
        sources[str(path.relative_to(root.parent))] = kept
    return sources


NAME_MACRO = re.compile(r'\bname!\(\s*"([^"]+)"\s*\)')
EFFECTS_LOOKUP = re.compile(r'\beffects\s*\.\s*(?:get|contains_key)\(\s*"([a-z][a-z0-9_]*)"')
EFFECT_ACCESSOR = re.compile(r"\b(fn\s+)?([a-z_]*effect[a-z_]*)\s*\(")

# Functions with "effect" in the name whose text argument is not an effect key.
NOT_EFFECT_ACCESSORS = {
    "congress_effect_active",  # a World Congress resolution id, defined in code
}


def _where(path: str, text: str, offset: int) -> str:
    return f"{path}:{text.count(chr(10), 0, offset) + 1}"


def name_literals(sources: dict[str, str]) -> dict[str, list[str]]:
    """Every ``name!("x")`` in shipping code, with where it is."""
    found: dict[str, list[str]] = collections.defaultdict(list)
    for path, text in sources.items():
        for match in NAME_MACRO.finditer(text):
            found[match.group(1)].append(_where(path, text, match.start()))
    return found


def _first_text_argument(text: str, start: int) -> str | None:
    """The first string literal directly inside the call whose body opens at
    ``start``, or None if the call has none. Bounded, so a bad parse is a miss
    rather than a walk over the whole file."""
    depth = 1
    index = start
    limit = min(len(text), start + 400)
    while index < limit and depth:
        char = text[index]
        if char == '"':
            end = index + 1
            while end < limit and text[end] != '"':
                end += 2 if text[end] == "\\" else 1
            if depth == 1:
                return text[index + 1 : end]
            index = end + 1
            continue
        if char in "([{":
            depth += 1
        elif char in ")]}":
            depth -= 1
        index += 1
    return None


def effect_reads(sources: dict[str, str]) -> dict[str, list[str]]:
    """Every effect key shipping code looks up, with where.

    Two shapes: ``effects.get("key")``, and a call to anything named for an
    effect with the key as its first text argument. Discovered from the name
    rather than listed, so a new accessor is covered the day it is written.
    """
    found: dict[str, list[str]] = collections.defaultdict(list)
    for path, text in sources.items():
        for match in EFFECTS_LOOKUP.finditer(text):
            found[match.group(1)].append(_where(path, text, match.start()))
        for match in EFFECT_ACCESSOR.finditer(text):
            definition, accessor = match.groups()
            if definition or accessor in NOT_EFFECT_ACCESSORS:
                continue
            key = _first_text_argument(text, match.end())
            if key and re.fullmatch(r"[a-z][a-z0-9_]*", key):
                found[key].append(_where(path, text, match.start()))
    return found


def audit_names(
    label: str,
    read: dict[str, list[str]],
    known: set[str],
    waivers: dict[str, str],
) -> list[str]:
    """Print one name audit and return its problems: unknown names, stale waivers."""
    unknown = sorted(name for name in read if name not in known and name not in waivers)
    stale = sorted(name for name in waivers if name not in read)
    print(f"{len(read)} {label}, {len(unknown)} in no data file, {len(waivers)} waived")
    problems = []
    for name in unknown:
        print(f"  {name:44} {read[name][0]}")
        problems.append(name)
    for name in stale:
        print(f"  {name:44} waived, but nothing reads it any more")
        problems.append(name)
    return problems


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
    unsupplied = sorted(
        key for key in priced if key not in keys and key not in BELIEF_YIELD_WAIVERS
    )
    waived_arms = sorted(key for key in priced if key in BELIEF_YIELD_WAIVERS)
    print(
        f"{len(priced)} belief-yield keys priced by the engine, "
        f"{len(unsupplied)} supplied by no belief, {len(waived_arms)} waived"
    )
    for key in waived_arms:
        print(f"  waived {key:37} {BELIEF_YIELD_WAIVERS[key]}")
    for key in unsupplied:
        print(f"  {key:44} no belief sets this")
    if args.max is not None and len(unsupplied) > args.max:
        print(
            f"FAIL: {len(unsupplied)} belief-yield keys exceed the ratchet of {args.max}",
            file=sys.stderr,
        )
        return 1

    duplicates = duplicate_keys()
    print(f"{len(duplicates)} keys written twice in one JSON object")
    for duplicate in duplicates:
        print(f"  {duplicate}")

    sections = json.loads(WAIVERS.read_text(encoding="utf-8")) if WAIVERS.exists() else {}
    sources = production_sources()
    known = data_strings()
    named = name_literals(sources)
    looked_up = effect_reads(sources)
    if not named or not looked_up:
        raise SystemExit("civvis_inert: a name scrape found nothing; it broke, the tree did not")
    bad_names = audit_names("name! literals", named, known, sections.get("names", {}))
    bad_reads = audit_names(
        "effect keys read",
        looked_up,
        known,
        {**sections.get("effect_reads", {}), **BELIEF_YIELD_WAIVERS},
    )

    if args.max is not None:
        for label, problems in (
            ("duplicate JSON keys", duplicates),
            ("name! literals", bad_names),
            ("effect reads", bad_reads),
        ):
            if len(problems) > args.max:
                print(
                    f"FAIL: {len(problems)} {label} exceed the ratchet of {args.max}",
                    file=sys.stderr,
                )
                return 1

    if args.max is not None and len(inert) > args.max:
        print(f"FAIL: {len(inert)} exceeds the ratchet of {args.max}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
