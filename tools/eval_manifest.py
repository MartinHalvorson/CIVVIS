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

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gene_registry  # noqa: E402



# The one list still read out of `src/elo.rs`: the built-in agents. Every gene
# list — live, host-only, repair (and its war/economy halves) — is a view of
# the gene registry (`src/ai/advanced/genes.rs`) read through
# `gene_registry.py`, and is published under the names the old `elo.rs` lists
# had so the manifest's shape and the page's rows stay readable.
REGISTRY_NAMES = ("BUILTIN_AIS",)
GENE_LISTS = (
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
    genes = gene_registry.genes_from_text(
        (repo / gene_registry.REGISTRY).read_text(encoding="utf-8"))
    views = {
        "LIVE_BRIDGE_TREATMENTS": [g.tag for g in genes if g.live],
        "FIRAXIS_ONLY_TREATMENTS": [g.tag for g in genes if g.host_only],
        "ENGINE_REPAIR_WAR_TREATMENTS": [g.tag for g in genes if g.axis == "War"],
        "ENGINE_REPAIR_ECONOMY_TREATMENTS": [g.tag for g in genes if g.axis == "Economy"],
        "ENGINE_REPAIR_TREATMENTS": [g.tag for g in genes if g.repair],
    }
    for name in GENE_LISTS:
        items = views[name]
        found[name] = {"declared_count": len(items), "items": items}
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
    stolen_turns: dict[str, list[int]] = {}
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
        stolen_turns.setdefault(name, []).append(turns)
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
        "stolen_turns": {
            name: {
                "earliest": min(turns),
                "median": sorted(turns)[len(turns) // 2],
                "latest": max(turns),
            }
            for name, turns in sorted(stolen_turns.items())
        },
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

    Three spellings: the registry tag (`bounded-recovery`), the flag in Rust
    spelling (`bounded_recovery`) that rounds routinely write, and the
    `live_without_*` arm name the retired paired evaluator used until
    2026-08-23 — rounds recorded before then name treatments that way, and the
    history still counts as having named them.
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


def genome_coverage(repo: Path) -> dict[str, Any]:
    """How much of the controller the genome instrument can actually see.

    ⚠⚠ THE MIRROR OF `bundle_coverage`, AND IT ERRS THE OTHER WAY ON PURPOSE.
    That function publishes an UNDER-count of the debt, because a tag it cannot
    find in the evidence certainly has no result. This one publishes an
    OVER-count: some capability toggles are host-only plumbing that no native
    screen could ever price, and this makes no attempt to tell those apart. So
    the number below is a ceiling on the debt, not a floor.

    Both are the same discipline. A count that can only be wrong in the
    direction of *more work than there really is* cannot be flattered either,
    and it is the only kind of count a regex is entitled to publish.

    Why it is worth publishing: `precise_evacuation` reached `main` in #2059
    ON for every major, city-state and barbarian, holding roughly half of the
    simulator's main thread — and with no gene row and no mention in any
    recorded round. No gate could address it, and nothing
    said so. `docs/GENE_SCREEN.md` names the growth direction as "hundreds of
    genes"; this is the denominator that direction is measured against.

    Three sources, none of them a hand-maintained list:

    * `treatment_flags.rs` — every `enable_*`/`disable_*` capability toggle.
      The file's own guard keeps them all there and nothing else there.
    * the gene registry (`src/ai/advanced/genes.rs`) — every screenable row
      carries its field and its toggle. This is exactly what
      `gene_screen::gene_table` walks, so "reachable" here means reachable
      by the binary rather than by a list somebody kept in step.
    * `docs/gene_ledger.json` — which of those genes a screen has measured.
    """
    flags_source = _strip_comments(
        (repo / "src" / "ai" / "advanced" / "treatment_flags.rs").read_text(
            encoding="utf-8"))
    toggles = set(re.findall(r"pub fn (?:enable|disable)_([a-z0-9_]+)\s*\(",
                             flags_source))
    if not toggles:
        raise ValueError(
            "src/ai/advanced/treatment_flags.rs yielded no capability toggles; "
            "the scrape broke rather than finding an empty file")

    # ⚠⚠ A ROW'S FIELD AND ITS TOGGLE'S NAME ARE NOT ALWAYS THE SAME WORD, and
    # joining on the wrong one invents debt: `army-target-weighs-enemy` is the
    # field `army_target_weighs_enemy` toggled through
    # `disable_army_target_weighs_the_enemy` — note "the" — so matching the
    # toggle set against the field alone reported a measured gene as
    # unreachable on this function's first run. A toggle counts as reachable
    # under EITHER spelling.
    genes = gene_registry.genes_from_text(
        (repo / gene_registry.REGISTRY).read_text(encoding="utf-8"))
    screenable_tags = {g.tag for g in genes if g.screenable}
    screenable_spellings: set[str] = set()
    for g in genes:
        if g.screenable:
            screenable_spellings |= {g.field, g.toggle}

    ledger = json.loads(
        (repo / "docs" / "gene_ledger.json").read_text(encoding="utf-8"))
    measured = {row["tag"] for row in ledger["genes"]}
    resolved = {row["tag"] for row in ledger["genes"]
                if row.get("verdict") in ("helps", "hurts")}

    unreachable = sorted(toggles - screenable_spellings)
    return {
        "capability_toggles": len(toggles),
        "reachable_as_a_gene": len(screenable_tags),
        "measured_by_a_screen": len(screenable_tags & measured),
        "resolved_by_a_screen": len(screenable_tags & resolved),
        "unreachable_by_any_screen": len(unreachable),
        "unreachable_toggles": unreachable,
    }


def build_manifest(repo: Path) -> dict[str, Any]:
    registry = read_registry(repo)
    live = registry["LIVE_BRIDGE_TREATMENTS"]["items"]
    firaxis = set(registry["FIRAXIS_ONLY_TREATMENTS"]["items"])
    return {
        "schema_version": 1,
        "genome_coverage": genome_coverage(repo),
        "source": {
            "registry": "src/ai/advanced/genes.rs",
            "agents": "src/elo.rs",
            "ladder": "docs/civ6_ladder.json",
        },
        "registry": registry,
        "derived": {
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
    windows = ladder.get("stolen_turns") or {}
    if windows:
        # ⚠ The count says which lane to work on; only the turn says whether a
        # simulated lane is behaving like the real one. A rival's diplomatic
        # victory has never landed before turn 202 on this ladder, so a native
        # game that produces none before then is not necessarily wrong, and one
        # that ends at the turn limit has not had the chance to be.
        detail = ", ".join(
            f"{name} {edge['earliest']}\u2013{edge['latest']} (median {edge['median']})"
            for name, edge in windows.items()
        )
        lines.append(f"- The turns those landed on: {detail}")
    return lines


def render_status(manifest: dict[str, Any]) -> str:
    derived = manifest["derived"]
    ladder = manifest["ladder"]
    registry = manifest["registry"]
    rows = [
        ("Built-in agents", len(registry["BUILTIN_AIS"]["items"])),
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
        "This page is generated from the gene registry (`src/ai/advanced/genes.rs`),",
        "`src/elo.rs` (the built-in agents) and `docs/civ6_ladder.json`.",
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
        "The native half of this bundle is priced by the gene screen",
        "(`docs/GENE_SCREEN.md`, `HEURISTIC_GENE_RANKING.md`); the host-only",
        "half can only be priced on the live seat, by `civvis_orders --without`",
        "over ladder games. This list is the debt neither has touched.",
        "",
        "Never named:",
        "",
    ]
    if coverage["never_named_treatments"]:
        lines.append(", ".join(f"`{tag}`" for tag in coverage["never_named_treatments"]))
    else:
        lines.append("_None — every withholdable treatment has been named._")
    genome = manifest["genome_coverage"]
    lines += [
        "",
        "## Genome coverage",
        "",
        "How much of the controller the genome instrument can vary at all.",
        "`docs/GENE_SCREEN.md` names the growth direction as \"hundreds of",
        "genes\"; this is the denominator that direction is measured against.",
        "",
        f"- Capability toggles on the controller: **{genome['capability_toggles']}**",
        f"- Reachable as a gene `gene_screen` can vary: **{genome['reachable_as_a_gene']}**",
        f"- Measured by at least one screen: **{genome['measured_by_a_screen']}**",
        f"- Resolved by the ledger (helps or hurts): **{genome['resolved_by_a_screen']}**",
        f"- **Unreachable by any screen: {genome['unreachable_by_any_screen']}**",
        "",
        "⚠ This is the mirror of the section above and it errs the other way.",
        "`Never named` under-counts the live-bundle debt; this OVER-counts the",
        "genome debt, because some toggles are host-only or bundle plumbing",
        "that no native screen could price and nothing here tries to tell them",
        "apart. So the last number is a ceiling on the work, not a floor — and",
        "a count that can only be wrong in the direction of more work cannot be",
        "flattered either.",
        "",
        "Why it is published: `precise_evacuation` shipped in #2059 ON for",
        "every major, city-state and barbarian, holding roughly half of the",
        "simulator's main thread, with no gene row and no",
        "mention in any recorded round. Neither gate could address it and",
        "nothing said so.",
        "",
        "Unreachable:",
        "",
    ]
    if genome["unreachable_toggles"]:
        lines.append("".join(
            f"`{flag}`, " for flag in genome["unreachable_toggles"]).rstrip(", "))
    else:
        lines.append("_None — every capability toggle is a gene._")
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


#: The artifacts this tool owns, relative to the repository root.
#:
#: One definition rather than two string literals per function, because
#: `tools/civvis_collab.py` resolves a merge conflict in these files by
#: regenerating them and has to be reading the same list the generator writes.
#: A path added here reaches both the writer and the checker, or neither.
GENERATED_OUTPUTS: tuple[str, ...] = (
    "docs/eval_manifest.json",
    "docs/EVAL_STATUS.md",
)


def canonical_json(value: Any) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def rendered_outputs(manifest: dict[str, Any]) -> dict[str, str]:
    """Every generated artifact's exact content, keyed by repo-relative path."""
    return {
        "docs/eval_manifest.json": canonical_json(manifest),
        "docs/EVAL_STATUS.md": render_status(manifest),
    }


def write_outputs(repo: Path, manifest: dict[str, Any]) -> None:
    for name, content in rendered_outputs(manifest).items():
        (repo / name).write_text(content, encoding="utf-8")


def check_outputs(repo: Path, manifest: dict[str, Any]) -> int:
    failures = []
    for name, expected in rendered_outputs(manifest).items():
        path = repo / name
        actual = path.read_text(encoding="utf-8") if path.exists() else None
        if actual != expected:
            failures.append(name)
    if failures:
        print("stale generated evaluation outputs: " + ", ".join(failures), file=sys.stderr)
        print("run: python3 tools/eval_manifest.py --write", file=sys.stderr)
        return 1
    print(
        f"evaluation manifest current: {manifest['derived']['live_bridge_count']} live-bridge treatments, "
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
                f"({manifest['derived']['live_bridge_count']} live-bridge treatments)"
            )
            return 0
        return check_outputs(args.repo, manifest)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"eval manifest: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
