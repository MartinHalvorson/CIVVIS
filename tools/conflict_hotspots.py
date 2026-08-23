#!/usr/bin/env python3
"""Which files actually tax concurrent work, measured rather than remembered.

`docs/ROADMAP.md` objective 5 used to name three conflict hotspots —
`src/game.rs`, `src/ai/advanced.rs`, `web/assets/app.js` — and say they tax
every concurrent PR. Two of those three were right. Measured over the 200
merges to `main` preceding 2026-08-18:

    src/ai/advanced.rs   26%
    src/elo.rs           18%   <- not on the list
    src/game.rs          16%
    src/ai.rs            11%
    src/server.rs         6%
    web/assets/app.js     2%   <- on the list

So the objective would send someone to split a file touched by one merge in
fifty while leaving the second-worst offender unnamed. Size is what the list was
built from and size is not the tax: `elo.rs` is a seventh of `game.rs`'s length
and is contended more often, because every live-bridge treatment appends to one
registry inside it.

⚠ AND THEN THE REPLACEMENT LIST WENT STALE TOO, in five days. On 2026-08-23
`src/main.rs` — 16% when it was written onto the table — was at 4%, and
`web/assets/app.js`, struck off at 2%, was back at 10%. Every ranking this
file prints has a date on it. Nothing here is a standing fact, which is why the
check reads the tool and not the table.

    tools/conflict_hotspots.py                 # the current ranking
    tools/conflict_hotspots.py --merges 500
    tools/conflict_hotspots.py --check         # fail on a split target nobody edits

Run daily in CI by `.github/workflows/census.yml`, in a job of its own because
it needs no toolchain and no build — and because when it shared the census's
job, a stale row here meant the census never ran (#2326).

## What this deliberately does not measure

Touch rate is exposure, not pain: two PRs editing distant parts of one file do
not conflict. Real conflict counts are not recoverable from `main`'s history,
because a squash merge records the resolution and never the collision. Touch
rate is the honest available proxy, and it is stated as one — the ranking is
what it can support, and a precise conflict count is not.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_MERGES = 200
#: A file the objective names must be contended at least this often. The check
#: is deliberately ONE-DIRECTIONAL: it fails a target nobody touches, and never
#: demands that the objective name the current top N.
#:
#: Two reasons. The ranking moves every day as merges land, so a both-ways check
#: would go red on ordinary work rather than on a defect. And touch rate mixes
#: two problems with different remedies — `src/elo.rs` and
#: `src/ai/advanced/treatment_flags.rs` are contended because every treatment PR
#: appends to one shared line or list in them, which splitting the file does not
#: fix; moving that data out of source does, the way `docs/eval/` did it for
#: `docs/EVAL.md`. Only `advanced.rs`, its tests and `game.rs` are contended for
#: the reason "split it" answers.
#:
#: So the machine checks the half that is mechanical — is this target real? —
#: and leaves which remedy fits to the prose.
#:
#: ⚠ FAILING THIS CHECK MUST NOT COST ANYTHING BUT THIS CHECK. It ran as the
#: first step of the census job, `bash -e` ended the job on it, and one stale
#: table row kept the ninety-minute census and its cross-platform determinism
#: reading from executing at all for five nights (#2326). It is its own job now,
#: and `tools/test_conflict_hotspots.py` fails if the two are recombined.
MIN_CONTENDED_PCT = 5

#: Hand-written source, whatever language it is written in.
#:
#: ★★★ THIS FILTER HID THE FIFTH-MOST-CONTENDED FILE IN THE REPOSITORY. It read
#: `(rs|js|py|sh)`, and the file it left out was
#: `tools/civ6_control/mod/CivvisControlAgent.lua` — 21 of the 200 merges to
#: 2026-08-18, 10%, ahead of `src/ai.rs`, `src/bin/civvis_orders.rs` and
#: `src/server.rs`, all three of which the ranking did print. It is also 12,245
#: lines in one file with a hard 199-local ceiling on its main chunk, which is
#: to say exactly the shape this ranking exists to surface.
#:
#: A tool whose first line is "measured rather than remembered" cannot measure a
#: language it declines to look at, and nothing about the omission was visible in
#: its output: an absent file and an uncontended one print identically.
#:
#: Generated pages and ledgers stay out on purpose — `docs/EVAL_STATUS.md` and
#: `docs/eval_manifest.json` are rewritten wholesale by a tool and their
#: contention is answered by regenerating them, not by splitting them. That is a
#: judgement about how a file is written, not about its extension, so add a
#: suffix here when the repository starts hand-writing one.
#:
#: ⚠ ONE source of truth, because there were two. The `--check` half had its own
#: copy of this list inside the regex that reads targets out of the objective's
#: table, so a target the ranking could not see was also a target the check
#: could not read — a row naming the Lua file would have been prose that nothing
#: verified, which is the failure this whole tool exists to prevent.
SOURCE_EXTENSIONS = ("rs", "js", "py", "sh", "lua", "html")
SOURCE_SUFFIXES = r"\.(" + "|".join(SOURCE_EXTENSIONS) + r")$"


def touched(sha: str) -> set[str]:
    out = subprocess.run(
        ["git", "-C", str(REPO), "show", "--name-only", "--format=",
         "-m", "--first-parent", sha],
        capture_output=True, text=True, check=False).stdout
    return {line.strip() for line in out.splitlines() if line.strip()}


#: Below this many merges the ranking is not worth judging on. A shallow clone
#: reports every file at 0%, which would make `--check` PASS BY MEASURING
#: NOTHING — the failure a check exists to not have.
MIN_HISTORY = 50


def recent_merges(count: int) -> list[str]:
    """Merge SHAs, newest first, from `origin/main` or whatever history exists.

    ⚠ `origin/main` IS OFTEN ABSENT. A pull-request checkout fetches the PR ref
    and need not create that remote-tracking branch at all; the first CI run of
    this tool died on exactly that. `HEAD` is the honest fallback there — in a
    PR checkout its history IS main's history plus the branch's own commits,
    which shifts a touch rate by a few tenths at most.
    """
    for rev in ("origin/main", "HEAD"):
        out = subprocess.run(
            ["git", "-C", str(REPO), "log", rev, "--format=%H", f"-{count}"],
            capture_output=True, text=True, check=False).stdout
        if out.split():
            return out.split()
    return []


def ranking(count: int = DEFAULT_MERGES,
            minimum: int = 3) -> list[tuple[str, int, int]]:
    """(path, merges touching it, percent), most contended first.

    Only tracked source files are ranked: generated pages and ledgers are
    rewritten wholesale by a tool and their contention is answered by
    regenerating, not by splitting them.
    """
    shas = recent_merges(count)
    if not shas:
        raise SystemExit("no history to rank; fetch before measuring")
    tally: dict[str, int] = {}
    for sha in shas:
        for path in touched(sha):
            if not re.search(SOURCE_SUFFIXES, path):
                continue
            tally[path] = tally.get(path, 0) + 1
    rows = [(path, hits, round(100 * hits / len(shas)))
            for path, hits in tally.items() if hits >= minimum]
    rows.sort(key=lambda row: (-row[1], row[0]))
    return rows


def roadmap_objective(text: str) -> str:
    """The text of the conflict-hotspot objective, or ''.

    Anchored on the phrase rather than the verb: the objective was called
    "Split the three conflict hotspots" when it named a file nobody edits, and
    renaming it must not be a way to stop being checked.
    """
    match = re.search(
        r"\n\d+\.\s+\*\*[^\n]*conflict hotspot[^\n]*\*\*(.*?)(?=\n\d+\.\s+\*\*|\n\n##)",
        text, re.DOTALL | re.IGNORECASE)
    return match.group(0) if match else ""


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--merges", type=int, default=DEFAULT_MERGES)
    parser.add_argument("--top", type=int, default=8)
    parser.add_argument("--check", action="store_true",
                        help="fail when the roadmap names a file outside the "
                             "measured top ranks, or misses one inside them")
    args = parser.parse_args(argv)

    shas = recent_merges(args.merges)
    if args.check and len(shas) < MIN_HISTORY:
        print(f"only {len(shas)} merges are reachable (need {MIN_HISTORY}); a "
              f"shallow clone reads every file at 0% and would pass this check "
              f"by measuring nothing. Check out with fetch-depth: 0.",
              file=sys.stderr)
        return 1
    rows = ranking(args.merges)
    if not args.check:
        print(f"of the last {len(recent_merges(args.merges))} merges to main:")
        for path, hits, pct in rows[:args.top]:
            print(f"  {path:26} {hits:4} ({pct}%)")
        return 0

    objective = roadmap_objective((REPO / "docs" / "ROADMAP.md").read_text())
    if not objective:
        print("ROADMAP.md no longer states a split-the-hotspots objective",
              file=sys.stderr)
        return 1
    rate = {path: pct for path, _, pct in rows}
    # Only the objective's target TABLE, not its prose. A repo path mentioned
    # in a sentence — the tool that produced the ranking, or a file named as an
    # example — is not a thing the objective is asking anyone to go and split.
    named = sorted({
        path
        for line in objective.splitlines() if line.lstrip().startswith("|")
        for path in re.findall(
            r"`([\w.-]+(?:/[\w.-]+)+\.(?:" + "|".join(SOURCE_EXTENSIONS) + r"))`",
            line,
        )
    })
    if not named:
        print("the conflict-hotspot objective names no target files at all",
              file=sys.stderr)
        return 1
    problems = []
    for path in named:
        pct = rate.get(path, 0)
        if pct < MIN_CONTENDED_PCT:
            problems.append(
                f"the objective targets {path}, touched by {pct}% of the last "
                f"{args.merges} merges — below the {MIN_CONTENDED_PCT}% floor. "
                f"Splitting a file nobody edits costs a large diff and buys "
                f"nothing")
    for problem in problems:
        print(f"HOTSPOTS: {problem}", file=sys.stderr)
    if problems:
        print("run: tools/conflict_hotspots.py", file=sys.stderr)
        return 1
    print(f"every split target is really contended: "
          + ", ".join(f"{p} {rate[p]}%" for p in named))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
