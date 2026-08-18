#!/usr/bin/env python3
"""Generate and verify the repository's current evaluation status.

The evaluator registry is Rust because it is part of the executable contract,
while the ladder is a JSON ledger written by the live bridge.  Historically
their counts were repeated in prose and in tests, so a new arm or a new live
attempt could leave the paper trail behind without making a build fail.

This tool is the single read-only projection of those two authoritative
sources.  ``--write`` refreshes the checked-in JSON manifest and the compact
status page; ``--check`` is the CI gate and fails if either generated artifact
is stale.  It deliberately does not rewrite the append-only ``docs/EVAL.md``:
that file is evidence, while ``docs/EVAL_STATUS.md`` is the current snapshot.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


REGISTRY_NAMES = (
    "BUILTIN_AIS",
    "EVAL_ONLY_AIS",
    "LIVE_BRIDGE_TREATMENTS",
    "FIRAXIS_ONLY_TREATMENTS",
    "ENGINE_REPAIR_WAR_TREATMENTS",
    "ENGINE_REPAIR_ECONOMY_TREATMENTS",
    "ENGINE_REPAIR_TREATMENTS",
)

# ⚠ THE REGISTRY LISTS NO LONGER CARRY A LENGTH, AND THAT IS THE POINT. They
# were `[&str; N]` with N typed by hand — the largest 188 — and on 2026-08-17
# #1865 added an arm, missed the count, and left `main` unable to build for
# wasm until #1869 fixed the number. They are `&[&str]` now, which cannot go
# stale. This reads both shapes so a lane pinned to an older revision still
# parses; `count` is simply absent from the newer one.
ARRAY_RE = re.compile(
    r"pub const (?P<name>[A-Z][A-Z0-9_]*)\s*:\s*"
    r"(?:\[&str;\s*(?P<count>\d+)\]\s*=\s*\[|&\[&str\]\s*=\s*&\[)"
    r"(?P<body>.*?)\];",
    re.DOTALL,
)
STRING_RE = re.compile(r'"((?:\\.|[^"\\])*)"')


def _strip_comments(text: str) -> str:
    """Remove Rust comments from a registry body before reading literals."""
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    return re.sub(r"//[^\n]*", "", text)


def _rust_string(value: str) -> str:
    # The registry strings are ordinary Rust strings.  JSON's decoder handles
    # the same escapes and keeps this parser independent of a Rust toolchain.
    return json.loads('"' + value + '"')


def read_registry(repo: Path) -> dict[str, dict[str, Any]]:
    source = (repo / "src" / "elo.rs").read_text(encoding="utf-8")
    found: dict[str, dict[str, Any]] = {}
    for match in ARRAY_RE.finditer(source):
        name = match.group("name")
        if name not in REGISTRY_NAMES:
            continue
        body = _strip_comments(match.group("body"))
        items = [_rust_string(value) for value in STRING_RE.findall(body)]
        # A hand-typed length is the thing that used to go stale; where one is
        # still present it is still checked, and where the declaration counts
        # itself there is nothing left to disagree with.
        declared = int(match.group("count")) if match.group("count") else len(items)
        if declared != len(items):
            raise ValueError(
                f"{name}: declaration says {declared}, found {len(items)} strings"
            )
        duplicate = next((item for item in items if items.count(item) > 1), None)
        if duplicate is not None:
            raise ValueError(f"{name}: duplicate registry item {duplicate!r}")
        found[name] = {"declared_count": declared, "items": items}
    missing = [name for name in REGISTRY_NAMES if name not in found]
    if missing:
        raise ValueError(f"src/elo.rs is missing registry constants: {', '.join(missing)}")
    return found


def read_ladder(repo: Path) -> dict[str, Any]:
    path = repo / "docs" / "civ6_ladder.json"
    state = json.loads(path.read_text(encoding="utf-8"))
    attempts = state.get("attempts")
    if not isinstance(attempts, list):
        raise ValueError("docs/civ6_ladder.json: attempts must be a list")
    terminal = [attempt for attempt in attempts if attempt.get("victory") is not None]
    configured = [attempt for attempt in attempts if attempt.get("configured")]
    wins = [attempt for attempt in attempts if attempt.get("configured") and attempt.get("won")]
    latest = max((attempt.get("utc", "") for attempt in attempts), default=None)
    return {
        "attempts": len(attempts),
        "configured": len(configured),
        "terminal": len(terminal),
        "wins": len(wins),
        "latest_utc": latest,
    }


def read_evidence(repo: Path) -> str:
    """Every word of recorded evaluation evidence, as one blob.

    Two homes since 2026-08-18: `docs/EVAL.md` holds the 168 historical rounds
    and is frozen, and each new round is its own file under `docs/eval/`. Both
    are read, and the directory is globbed rather than listed — a round that
    exists but is not searched would be counted as evidence nobody has.
    """
    parts = [(repo / "docs" / "EVAL.md").read_text(encoding="utf-8")]
    rounds = sorted((repo / "docs" / "eval").glob("*.md"))
    if not rounds:
        raise ValueError("docs/eval/ holds no rounds; the glob stopped matching")
    parts.extend(path.read_text(encoding="utf-8") for path in rounds)
    return "\n".join(parts)


def bundle_coverage(live: list[str], firaxis: set[str], evidence: str) -> dict[str, Any]:
    """How much of the shipped live-bridge bundle the ledger has ever discussed.

    ⚠⚠ THE COUNT THIS PUBLISHES IS "NEVER NAMED", AND THAT IS DELIBERATELY THE
    WEAKER HALF OF THE QUESTION. Whether a treatment was *priced* is a judgement
    about what a round concluded and no string search can make it. Whether a
    treatment has ever been *named* in any recorded round is mechanical, and it
    bounds the answer from one side only: a tag that appears nowhere in the
    evidence certainly has no recorded result, while a tag that appears may only
    have been mentioned in passing. So `named` is an over-count of coverage and
    `never_named` is an under-count of the debt — the number to act on is the
    one that cannot be flattered.

    Why it is worth publishing at all: on 2026-08-18 fifty of the seventy-four
    live-bridge treatments had never been named in any round, and nobody could
    see it. `docs/ROADMAP.md` objective 3 asks for exactly this bundle to be
    priced by withholding, and the inventory above counted the arms that
    *exist* rather than the ones that have been *used*. The repository has
    already paid for that blind spot once, in `city_target_floor`: retained on
    a composite result, and removed from production once it was priced alone.

    ⚠ ALL THREE SPELLINGS, and the third was found by checking the instrument
    against a treatment already known to be priced. The registry tag is
    hyphenated (`bounded-recovery`), the evaluator arm derived from it is
    `live_without_bounded_recovery`, and rounds routinely write the flag itself
    in Rust spelling — `bounded_recovery` — which is how the confirmed-null
    result that got it deleted from production is recorded. Searching only the
    first two called it never-named and would have overstated the debt by a
    fifth on the first run.
    """
    withholdable = [item for item in live if item not in firaxis]
    named, never = [], []
    for tag in withholdable:
        flag = tag.replace("-", "_")
        spellings = (tag, flag, f"live_without_{flag}")
        found = any(spelling in evidence for spelling in spellings)
        (named if found else never).append(tag)
    return {
        "withholdable": len(withholdable),
        "named": len(named),
        "never_named": len(never),
        "never_named_treatments": sorted(never),
    }


def build_manifest(repo: Path) -> dict[str, Any]:
    registry = read_registry(repo)
    live = registry["LIVE_BRIDGE_TREATMENTS"]["items"]
    firaxis = set(registry["FIRAXIS_ONLY_TREATMENTS"]["items"])
    return {
        "schema_version": 1,
        "source": {
            "registry": "src/elo.rs",
            "ladder": "docs/civ6_ladder.json",
        },
        "registry": registry,
        "derived": {
            "eval_only_count": len(registry["EVAL_ONLY_AIS"]["items"]),
            "live_bridge_count": len(live),
            "firaxis_only_count": len(firaxis),
            "native_engine_repair_count": len(
                registry["ENGINE_REPAIR_TREATMENTS"]["items"]
            ),
            "withholdable_live_count": len(
                [item for item in live if item not in firaxis]
            ),
        },
        "coverage": bundle_coverage(live, firaxis, read_evidence(repo)),
        "ladder": read_ladder(repo),
    }


def render_status(manifest: dict[str, Any]) -> str:
    derived = manifest["derived"]
    ladder = manifest["ladder"]
    registry = manifest["registry"]
    rows = [
        ("Built-in agents", len(registry["BUILTIN_AIS"]["items"])),
        ("Evaluator-only agents", derived["eval_only_count"]),
        ("Live-bridge treatments", derived["live_bridge_count"]),
        ("Firaxis-only treatments", derived["firaxis_only_count"]),
        ("Native engine-repair treatments", derived["native_engine_repair_count"]),
        ("Withholdable live treatments", derived["withholdable_live_count"]),
    ]
    lines = [
        "# Current evaluation status",
        "",
        "<!-- GENERATED FILE: python3 tools/eval_manifest.py --write -->",
        "",
        "This page is generated from `src/elo.rs` and `docs/civ6_ladder.json`.",
        "The append-only experiment evidence remains in `docs/EVAL.md`; this",
        "page is the current inventory and live-bridge snapshot.",
        "",
        "## Registry",
        "",
        "| inventory | count |",
        "|---|---:|",
    ]
    lines.extend(f"| {label} | {count} |" for label, count in rows)
    coverage = manifest["coverage"]
    lines += [
        "",
        "## Bundle coverage",
        "",
        "How much of the shipped live-bridge bundle the evaluation evidence has",
        "ever *named* — `docs/EVAL.md` plus every round under `docs/eval/`.",
        "",
        f"- Withholdable live treatments: **{coverage['withholdable']}**",
        f"- Named somewhere in the evidence: **{coverage['named']}**",
        f"- **Never named in any round: {coverage['never_named']}**",
        "",
        "⚠ This is deliberately the weaker half of the question. Whether a",
        "treatment was *priced* is a judgement about what a round concluded and",
        "no string search can make it; whether it has ever been *named* is",
        "mechanical. So the middle number over-counts coverage and the last one",
        "under-counts the debt — act on the last one, which cannot be flattered.",
        "",
        "`docs/ROADMAP.md` objective 3 asks for this bundle to be priced by",
        "withholding, *before the next effect hides inside a composite the way",
        "`city_target_floor` did*. The inventory above counts the arms that",
        "exist; this counts the ones that have been used, and the gap between",
        "them is what stayed invisible.",
        "",
        "Never named:",
        "",
    ]
    if coverage["never_named_treatments"]:
        lines.append("".join(
            f"`{tag}`, " for tag in coverage["never_named_treatments"]
        ).rstrip(", "))
    else:
        lines.append("_None — every withholdable treatment has been named._")
    lines += [
        "",
        "## Live ladder",
        "",
        f"- Attempts recorded: **{ladder['attempts']}**",
        f"- Configured attempts: **{ladder['configured']}**",
        f"- Terminal outcomes: **{ladder['terminal']}**",
        f"- Configured wins: **{ladder['wins']}**",
        f"- Latest ledger entry: **{ladder['latest_utc'] or '—'}**",
        "",
        "Regenerate with `python3 tools/eval_manifest.py --write`; CI runs",
        "`--check` so registry or ledger changes cannot silently leave this",
        "snapshot stale.",
        "",
    ]
    return "\n".join(lines)


def canonical_json(value: Any) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def write_outputs(repo: Path, manifest: dict[str, Any]) -> None:
    (repo / "docs" / "eval_manifest.json").write_text(
        canonical_json(manifest), encoding="utf-8"
    )
    (repo / "docs" / "EVAL_STATUS.md").write_text(
        render_status(manifest), encoding="utf-8"
    )


def check_outputs(repo: Path, manifest: dict[str, Any]) -> int:
    expected_json = canonical_json(manifest)
    expected_status = render_status(manifest)
    actual_json = (repo / "docs" / "eval_manifest.json").read_text(encoding="utf-8") \
        if (repo / "docs" / "eval_manifest.json").exists() else None
    actual_status = (repo / "docs" / "EVAL_STATUS.md").read_text(encoding="utf-8") \
        if (repo / "docs" / "EVAL_STATUS.md").exists() else None
    failures = []
    if actual_json != expected_json:
        failures.append("docs/eval_manifest.json")
    if actual_status != expected_status:
        failures.append("docs/EVAL_STATUS.md")
    if failures:
        print("stale generated evaluation outputs: " + ", ".join(failures), file=sys.stderr)
        print("run: python3 tools/eval_manifest.py --write", file=sys.stderr)
        return 1
    print(
        f"evaluation manifest current: {manifest['derived']['eval_only_count']} evaluator arms, "
        f"{manifest['ladder']['attempts']} ladder attempts"
    )
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parent.parent)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true", help="write generated outputs")
    mode.add_argument("--check", action="store_true", help="fail when outputs are stale")
    args = parser.parse_args(argv)
    try:
        manifest = build_manifest(args.repo)
        if args.write:
            write_outputs(args.repo, manifest)
            print(
                f"wrote docs/eval_manifest.json and docs/EVAL_STATUS.md "
                f"({manifest['derived']['eval_only_count']} evaluator arms)"
            )
            return 0
        return check_outputs(args.repo, manifest)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"eval manifest: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
