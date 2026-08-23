#!/usr/bin/env python3
"""A gene that cannot fire is not a null, and this is the CI ratchet that says so.

★★★★ THE SIGNATURE WAS ALREADY WRITTEN DOWN; NOTHING CHECKED FOR IT.

`docs/GENE_SCREEN.md` records it in a section of its own -- *"A Δ of exactly
zero is a gene that never fired, not a null"*. `step-and-reassess` screened
**+0.0 [+0.0, +0.0]** on both axes over 204 pairs because its only code path
lived on a parallel unit planner no evaluator installs, so every pair's two
games ended identically. `treatment_flags.rs` carries the same warning from the
other direction: a repair gene whose enable is missing from
the bundle is off in BOTH arms, "the two arms play byte-identical games", and
*"three tags reached the tables before this line and burned 30 games saying
nothing"*. `competition-victory-points` is in the tables today and cannot fire
at all, because `native_competitions` ships off.

Four instruments already implement a fires-check for themselves -- `ai_eval`'s
"nothing differed" note, `battle_bench` and `doctrine_arena`'s `diverged`
counts, and `gene_census`, whose docstring states the principle: *does this gene
change anything at all -- which is far cheaper to establish than a win rate and
is a precondition for the win rate meaning anything*. What was missing is a
**gate**. Nothing stopped a tag reaching the three gene tables with no evidence
it fires, and nothing failed when a committed screen contained a zero-width row.
A gene in that state consumes screen games and returns nothing.

So: **every gene in the tables must be shown to have fired in at least one game,
or carry a waiver that says why it has not been.** `--max 0` is the CI ratchet,
in the shape `civvis_inert.py --max 0` already runs at.

WHAT COUNTS AS PROOF, and why it is read rather than asserted
-------------------------------------------------------------
A screen's own JSON. A gene whose two arms played different games has a
non-zero paired statistic; a gene that never fired has `win_delta_pp`,
`win_se_pp`, `share_delta_pp`, `share_se_pp` and `adjusted_se_pp` all exactly
zero, which is the documented signature and arises no other way at these
sample sizes. So proof is a committed screen row with any of those non-zero --
a number this tool reads out of the artifact, never a sentence somebody wrote.
A single-gene probe under `docs/gene_screens/fires/` is the cheapest artifact
that can carry it: `--genes <tag>` holds everything else at the baseline, so
any divergence between the arms is that gene and nothing else.

⚠ A gene priced by a MULTI-gene screen is proven too, but for a weaker reason:
its arms differ on other genes as well, so a non-zero contrast there is not
attributable. It is still evidence the tag reached an analysis with real
variance, and every such gene here also has non-zero variance in its own row.
The single-gene probe is what a NEW gene should bring.

WHAT A WAIVER IS FOR
--------------------
`tools/gene_fire_waivers.json`, in the shape of `tools/inert_waivers.json`: a
flat `{tag: reason}` map. Two kinds of entry live there and the reasons say
which -- a gene that predates this gate and has not been screened yet, and a
gene that genuinely cannot fire in the regime the screen plays. Unlike
`inert_waivers.json` the reason is enforced: it must be long enough to be a
reason. A waiver goes **stale** the moment its gene is proven or leaves the
tables, and a stale waiver fails the same ratchet -- so the list can only
shrink, and cannot quietly outlive what it was written for.

Usage:
    python3 tools/gene_fires.py                # report
    python3 tools/gene_fires.py --max 0        # CI ratchet
    python3 tools/gene_fires.py --json         # machine-readable
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
import genes  # noqa: E402

REGISTRY = ROOT / genes.REGISTRY
SCREENS = ROOT / "docs" / "gene_screens"
WAIVERS = Path(__file__).resolve().parent / "gene_fire_waivers.json"

# The paired statistics a screen writes per gene. All five are exactly zero
# when, and only when, the two arms played the same games.
STATISTICS = ("win_delta_pp", "win_se_pp", "share_delta_pp", "share_se_pp",
              "adjusted_se_pp")

# Long enough to be a reason rather than a shrug. `test_ci_wiring.py` uses the
# same bar for `CANNOT_RUN_IN_CI`.
REASON_CHARACTERS = 40

def gene_tables() -> dict[str, str]:
    """Every gene `gene_screen` can vary, tag -> the kind of registry row it is.

    ⚠ Discovered, never listed. This is `gene_screen::gene_table` in Python:
    every screenable row of the gene registry (`src/ai/advanced/genes.rs`). A
    gene added there reaches this gate without touching this file, which is
    the whole point -- a hand-written list here would be complete on the day
    it was written and silently shrink afterwards.
    """
    try:
        rows = genes.genes_from_text(REGISTRY.read_text(encoding="utf-8"))
    except (ValueError, IndexError) as error:
        raise SystemExit(f"{REGISTRY} yielded no registry rows; the scrape broke: {error}")
    return {row.tag: row.kind for row in rows if row.screenable}


def _screen_files() -> list[Path]:
    files = sorted(SCREENS.rglob("*.json"))
    if not files:
        raise SystemExit(
            f"{SCREENS} holds no screen JSON; the glob came up empty rather "
            "than finding a repository with no screens")
    return files


def firing_evidence() -> tuple[dict[str, list[str]], list[tuple[str, str]]]:
    """(tag -> screens whose row for it is non-zero, [(screen, tag) zero-width]).

    A row missing every statistic is not evidence either way and is ignored;
    a row carrying them and reading exactly zero on all five is the never-fired
    signature and is reported.
    """
    fired: dict[str, list[str]] = {}
    flat: list[tuple[str, str]] = []
    for path in _screen_files():
        try:
            screen = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as broken:
            raise SystemExit(f"{path} is not readable JSON: {broken}") from broken
        name = str(path.relative_to(SCREENS))
        for row in screen.get("genes", []):
            tag = row.get("tag")
            if not tag:
                continue
            present = [row[key] for key in STATISTICS if key in row]
            if not present:
                continue
            if any(value for value in present):
                fired.setdefault(tag, []).append(name)
            else:
                flat.append((name, tag))
    return fired, flat


def waivers() -> dict[str, str]:
    if not WAIVERS.exists():
        return {}
    return json.loads(WAIVERS.read_text(encoding="utf-8"))["waivers"]


def survey() -> dict[str, Any]:
    genes = gene_tables()
    fired, flat = firing_evidence()
    waived = waivers()

    unproven = sorted(tag for tag in genes if tag not in fired and tag not in waived)
    # A waiver outlives its reason two ways, and both are failures: the gene has
    # since been shown to fire, or it is not a gene any more.
    stale = sorted(
        (tag, "now proven by " + fired[tag][0] if tag in fired else "no longer a gene")
        for tag in waived if tag in fired or tag not in genes)
    thin = sorted(tag for tag, reason in waived.items()
                  if len(reason.strip()) <= REASON_CHARACTERS)
    return {
        "genes": len(genes),
        "proven": sorted(tag for tag in genes if tag in fired),
        "unproven": unproven,
        "waived": sorted(tag for tag in genes if tag in waived),
        "stale_waivers": stale,
        "reasonless_waivers": thin,
        "zero_width_rows": flat,
        "tables": genes,
        "evidence": {tag: fired[tag] for tag in genes if tag in fired},
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--max", type=int, default=None,
                        help="fail when more than this many genes are unproven, "
                             "or any waiver has gone stale (CI runs --max 0)")
    parser.add_argument("--json", action="store_true",
                        help="write the survey as JSON instead of prose")
    args = parser.parse_args()

    found = survey()
    if args.json:
        print(json.dumps({key: value for key, value in found.items()
                          if key != "tables"}, indent=2, sort_keys=True))
    else:
        print(f"{found['genes']} genes in the tables, "
              f"{len(found['proven'])} shown to fire, "
              f"{len(found['waived'])} waived, "
              f"{len(found['unproven'])} with neither")
        for tag in found["unproven"]:
            print(f"  {tag:44} no screen row and no waiver")
        for tag, why in found["stale_waivers"]:
            print(f"  stale waiver {tag:38} {why}")
        for tag in found["reasonless_waivers"]:
            print(f"  waiver too short to be a reason: {tag}")
        if found["zero_width_rows"]:
            print(f"{len(found['zero_width_rows'])} zero-width screen rows "
                  "(a gene that never fired in that run, not a null):")
            for screen, tag in found["zero_width_rows"]:
                print(f"  {tag:44} {screen}")

    if args.max is None:
        return 0
    failed = 0
    if len(found["unproven"]) > args.max:
        print(f"FAIL: {len(found['unproven'])} genes have no evidence they fire "
              f"and no waiver, which exceeds the ratchet of {args.max}. Run "
              "`gene_screen --genes <tag> --games 6`, then `--analyze … --json "
              "docs/gene_screens/fires/<tag>.json` and commit the result, or "
              "add a waiver naming the reason it cannot fire.", file=sys.stderr)
        failed = 1
    if found["stale_waivers"]:
        print(f"FAIL: {len(found['stale_waivers'])} waivers have outlived their "
              "reason and must be deleted.", file=sys.stderr)
        failed = 1
    if found["reasonless_waivers"]:
        print(f"FAIL: {len(found['reasonless_waivers'])} waivers give no reason "
              f"longer than {REASON_CHARACTERS} characters.", file=sys.stderr)
        failed = 1
    return failed


if __name__ == "__main__":
    raise SystemExit(main())
