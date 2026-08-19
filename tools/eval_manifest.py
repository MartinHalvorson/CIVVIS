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


# A game that reached this turn is one the ladder actually finished, so its
# score is a result rather than the state it was in when something stopped it.
LADDER_TERMINAL_TURNS = 248


def withholding_arms(
    repo: Path, registered: list[str], withholdable: set[str] | None = None
) -> dict[str, str]:
    """Published tag -> the evaluator arm that withholds it, looked up not guessed.

    ⚠⚠ THERE IS NO RULE, AND BOTH OBVIOUS ONES ARE WRONG SOMEWHERE. This was
    `live_without_{tag.replace('-', '_')}`, which names an arm that does not
    exist for `ranged-line-of-sight` — its arm is
    `live_without_ranged_needs_line_of_sight`, after the flag. Deriving from the
    flag instead is wrong the other way: `army-target-weighs-enemy` sets
    `army_target_weighs_the_enemy` and its arm is
    `live_without_army_target_weighs_enemy`, after the tag.

    So each tag's arm is whichever candidate `EVAL_ONLY_AIS` actually contains,
    and a tag with neither is an error rather than a guess. That matters twice
    over: `docs/EVAL_STATUS.md` publishes this list as the debt roadmap
    objective 3 asks the fleet to work through, and the evidence search below
    looks for the arm name as one of its spellings — so a wrong guess both
    prints a name nobody can run and over-counts the debt by missing a round
    that used the real one.
    """
    source = (repo / "src" / "ai" / "advanced" / "treatments.rs").read_text(encoding="utf-8")
    rows = re.findall(
        r'\(\s*"([a-z0-9_]+)"\s*,\s*"([a-z0-9-]+)"\s*,\s*AdvancedAi::disable_([a-z0-9_]+)\s*\)',
        source,
    )
    if not rows:
        raise ValueError(
            "src/ai/advanced/treatments.rs yielded no LIVE_TREATMENTS rows; the scrape "
            "broke rather than finding an empty table"
        )
    known = set(registered)
    arms: dict[str, str] = {}
    unrunnable = []
    for _name, tag, disabler in rows:
        # ⚠ Only the withholdable tags. `LIVE_TREATMENTS` also carries rows
        # that are not in `LIVE_BRIDGE_TREATMENTS` — `strategic-wonders` is
        # one — and those correctly have no `live_without_*` arm. Checking
        # every row instead raised on them, and "fixing" that by registering
        # an arm broke three tests that know better:
        # `each_live_without_arm_holds_exactly_one_treatment_off` refuses an
        # arm whose treatment is not a live-bridge treatment.
        if withholdable is not None and tag not in withholdable:
            continue
        candidates = [f"live_without_{tag.replace('-', '_')}", f"live_without_{disabler}"]
        arm = next((candidate for candidate in candidates if candidate in known), None)
        if arm is None:
            unrunnable.append(f"{tag} (tried {', '.join(candidates)})")
        else:
            arms[tag] = arm
    if unrunnable:
        raise ValueError(
            "these withholdable treatments have no evaluator arm, so the debt list would "
            "name something nobody can run: " + "; ".join(unrunnable)
        )
    return arms


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
        **ladder_distance(attempts),
    }


def ladder_distance(attempts: list[dict[str, Any]]) -> dict[str, Any]:
    """How far the ladder is from a win, not only whether it got one.

    ★★★★★ THE WIN COUNT IS THE LEAST INFORMATIVE STATISTIC THIS LEDGER HOLDS.
    Three wins in 313 attempts is a number that moves about twice a month, so
    it cannot show whether the bridge is improving between wins -- and it has
    been improving. The terminal score is recorded on every attempt and says
    so immediately: over the games that reached the turn limit, the daily
    median ran 200-520 through 2026-08-15 and then 716, 977, 946 on the three
    days after the actuation-gap work landed. None of that was visible here.

    `rival_best` is the number that turns a score into a distance, and it has
    only been recorded since 2026-08-16, so most terminal games are ungraded.
    That is reported rather than hidden: a lead computed from six games is
    worth having and worth labelling.
    """
    scored = sorted(
        attempt["score"]
        for attempt in attempts
        if attempt.get("configured")
        and isinstance(attempt.get("turns"), int)
        and attempt["turns"] >= LADDER_TERMINAL_TURNS
        and isinstance(attempt.get("score"), (int, float))
        and attempt["score"] > 0
    )
    graded = [
        (attempt["score"], attempt["rival_best"])
        for attempt in attempts
        if attempt.get("configured")
        and isinstance(attempt.get("score"), (int, float))
        and isinstance(attempt.get("rival_best"), (int, float))
        and attempt["rival_best"] > 0
    ]
    out: dict[str, Any] = {
        "full_length": len(scored),
        "graded": len(graded),
    }
    if scored:
        out["score_median"] = scored[len(scored) // 2]
        out["score_best"] = scored[-1]
    if graded:
        leads = sorted(score - rival for score, rival in graded)
        out["lead_median"] = leads[len(leads) // 2]
        out["lead_best"] = leads[-1]
        out["rival_bar_median"] = sorted(rival for _, rival in graded)[len(graded) // 2]
        out["ahead"] = sum(1 for lead in leads if lead > 0)
    out.update(ladder_endings(attempts))
    return out


# `victory_types` in the ledger, by index. Kept here rather than read from the
# file so a ledger written by an older harness still renders.
LADDER_VICTORY_NAMES = {
    0: "score",
    1: "default",
    2: "conquest",
    3: "culture",
    4: "religious",
    5: "technology",
    6: "diplomatic",
}


def ladder_endings(attempts: list[dict[str, Any]]) -> dict[str, Any]:
    """What actually ends these games, and whether we were ahead when it did.

    ★★★★★ THE LADDER IS NOT MOSTLY LOSING ON THE CLOCK. Of the graded attempts
    on record, most end **before** turn 250 because a rival completed a culture
    or diplomatic victory, and in several of those our own score was the
    highest on the board -- once by 409. A seat that is ahead on score and
    loses to somebody else's victory condition has a denial problem, not a
    development problem, and the win count cannot tell those apart because both
    read as `won=False`.

    `AdvancedAi::new` already carries `deny_while_targeted` and
    `stock_denial_lead_time` for exactly this, added on the same observation.
    Reporting it here is how anyone will notice whether they worked.
    """
    stolen: dict[str, int] = {}
    stolen_while_ahead = 0
    for attempt in attempts:
        if not attempt.get("configured") or attempt.get("won"):
            continue
        victory = attempt.get("victory")
        turns = attempt.get("turns")
        if not isinstance(victory, int) or not isinstance(turns, int):
            continue
        # A game that ran the clock out ends on score by construction; only an
        # early finish is somebody else completing a condition.
        if turns >= LADDER_TERMINAL_TURNS or victory == 0:
            continue
        name = LADDER_VICTORY_NAMES.get(victory, f"type {victory}")
        stolen[name] = stolen.get(name, 0) + 1
        rival = attempt.get("rival_best")
        score = attempt.get("score")
        if (
            isinstance(rival, (int, float))
            and isinstance(score, (int, float))
            and score > rival
        ):
            stolen_while_ahead += 1
    return {
        "stolen": dict(sorted(stolen.items(), key=lambda item: (-item[1], item[0]))),
        "stolen_while_ahead": stolen_while_ahead,
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


def bundle_coverage(
    live: list[str],
    firaxis: set[str],
    evidence: str,
    arms: dict[str, str] | None = None,
) -> dict[str, Any]:
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
        arm = (arms or {}).get(tag, f"live_without_{flag}")
        # The arm is the fourth spelling and the only one that is authoritative
        # for a tag published shorter than its flag.
        spellings = (tag, flag, f"live_without_{flag}", arm)
        found = any(spelling in evidence for spelling in spellings)
        (named if found else never).append(tag)
    return {
        "withholdable": len(withholdable),
        "named": len(named),
        "never_named": len(never),
        "never_named_treatments": sorted(never),
        "never_named_arms": {
            tag: (arms or {}).get(tag, f"live_without_{tag.replace('-', '_')}")
            for tag in sorted(never)
        },
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
        "coverage": bundle_coverage(
            live,
            firaxis,
            read_evidence(repo),
            withholding_arms(
                repo,
                registry["EVAL_ONLY_AIS"]["items"],
                {tag for tag in live if tag not in firaxis},
            ),
        ),
        "ladder": read_ladder(repo),
    }


def _ladder_distance_lines(ladder: dict[str, Any]) -> list[str]:
    """The distance block. Absent numbers are said to be absent."""
    lines = [""]
    if not ladder.get("full_length"):
        return lines + ["- Distance to a win: **no finished attempt on record**"]
    lines.append(
        f"- Attempts that ran the full clock: **{ladder['full_length']}**, "
        f"median score **{ladder['score_median']:.0f}**, best **{ladder['score_best']:.0f}**"
    )
    if not ladder.get("graded"):
        lines.append(
            "- Distance to a win: **unknown** — no attempt records `rival_best`, "
            "so a near miss and a rout are the same row here"
        )
        return lines
    lines.append(
        f"- Graded against the best rival: **{ladder['graded']}** of "
        f"{ladder['full_length']} finished attempts; rival bar median "
        f"**{ladder['rival_bar_median']:.0f}**, our lead median "
        f"**{ladder['lead_median']:+.0f}**, best **{ladder['lead_best']:+.0f}**, "
        f"ahead in **{ladder['ahead']}**"
    )
    stolen = ladder.get("stolen") or {}
    if stolen:
        detail = ", ".join(f"{name} {count}" for name, count in stolen.items())
        total = sum(stolen.values())
        lines.append(
            f"- Lost to a rival's victory before the clock: **{total}** "
            f"({detail}), of which **{ladder['stolen_while_ahead']}** while our "
            "own score was the highest on the board"
        )
    return lines


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
            f"`{tag}` (`{coverage['never_named_arms'][tag]}`), "
            for tag in coverage["never_named_treatments"]
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
    ]
    lines += _ladder_distance_lines(ladder)
    lines += [
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
