#!/usr/bin/env python3
"""Which files actually tax concurrent work, measured rather than remembered.

`docs/ROADMAP.md` objective 5 names three conflict hotspots — `src/game.rs`,
`src/ai/advanced.rs`, `web/assets/app.js` — and says they tax every concurrent
PR. Two of those three are right. Measured over the 200 merges to `main`
preceding 2026-08-18:

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

    tools/conflict_hotspots.py                 # the current ranking
    tools/conflict_hotspots.py --merges 500
    tools/conflict_hotspots.py --check         # fail on a split target nobody edits
    tools/conflict_hotspots.py --modes         # WHICH of the two problems each file has

Run daily in CI by `.github/workflows/census.yml`, first in that job because it
needs no toolchain and no build.

## Two problems, and the ranking above cannot tell them apart

`docs/ROADMAP.md` objective 5 says there are two reasons a file is contended
and they take opposite remedies. Splitting along a seam answers **size**. It
does nothing for a file where every change appends to **one shared list**:
those two changes conflict whatever the file's length, and only moving the
data relieves them. The touch-rate ranking scores both the same, and acting on
it alone is how `advanced.rs`'s `LIVE_TREATMENTS` table was moved into
`advanced/treatments.rs` — where, five days later, it was the third-worst
hotspot in the repository. **The anchor moved; it was not removed.**

`--modes` measures which problem a file has. It replays every consecutive pair
of merges that touch the file as if the two had been written concurrently
(`replay`), using git's own three-way merge on the real bytes, and then splits
the collisions two ways:

* both sides only INSERTED lines, at a place where collisions REPEAT — two
  pull requests appending to one shared list. Move the data out;
* anything else — two pull requests editing the same code. Split the file.

Measured over the 200 merges ending at `2c570f4f` (2026-08-23), that separates
files the ranking prints side by side:

    src/ai/advanced/treatments.rs      10/10 anchored  ANCHOR  two list literals
    src/elo.rs                         15/18 anchored  ANCHOR  four registries
    src/ai/advanced.rs                  8/16 anchored  BOTH    `configured`, the struct
    src/ai/advanced/tests.rs            0/10 anchored  SPREAD  ten different tests
    src/ai/advanced/treatment_flags.rs   0/7 anchored  SPREAD  182 toggles, never twice
    src/ai.rs                           2/21 anchored  SPREAD
    src/game.rs                         2/25 anchored  SPREAD
    web/assets/app.js                    0/3 anchored  SPREAD

⚠ Every one of those numbers is a reading with a date on it, which is why
`--modes` prints the merge its window ends at. The RANK moves daily — this is
the objective whose table went stale in five days and took a CI job with it —
but the VERDICT has not: re-measured across three windows on 2026-08-23, the
percentages moved and no file changed sides.

Two of those are worth reading twice. `treatment_flags.rs` and `treatments.rs`
were created by the SAME relief effort, and only one of them worked: the 182
toggles now collide at 182 different places, which is no anchor at all, while
the two tables still collide at exactly two lines. And `advanced.rs`, whose
shared-anchor half the roadmap recorded as done, holds the two largest single
anchors left — `fn configured` and `pub struct AdvancedAi`, the flag field and
its initialiser, which nobody has ever named.

## What this deliberately does not measure

Touch rate is exposure, not pain: two PRs editing distant parts of one file do
not conflict. Real conflict counts are not recoverable from `main`'s history,
because a squash merge records the resolution and never the collision. Touch
rate is the honest available proxy, and it is stated as one.

`--modes` is a reconstruction, not a log, for the same reason, and `replay`
states its three caveats where it makes them. One more belongs here: a
collision is located by the innermost item it sits inside, so a 400-line
function or a 1,000-line struct reports its whole span as one place. That is
precise enough to name an anchor and too coarse to say two collisions hit the
same LINE.
"""

from __future__ import annotations

import argparse
import collections
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_MERGES = 200
#: A file the objective names must be contended at least this often. The check
#: is deliberately ONE-DIRECTIONAL: it fails a target nobody touches, and never
#: demands that the objective name the current top N.
#:
#: Two reasons. The ranking moves every day as merges land, so a both-ways check
#: would go red on ordinary work rather than on a defect. And touch rate mixes
#: two problems with different remedies — `src/main.rs` (20%) and `src/elo.rs`
#: (18%) are contended because every treatment PR appends to one shared line or
#: list in them, which splitting the file does not fix; moving that data out of
#: source does, the way `docs/eval/` did it for `docs/EVAL.md`. Only
#: `advanced.rs` and `game.rs` are contended for the reason "split it" answers.
#:
#: So the machine checks the half that is mechanical — is this target real? —
#: and leaves which remedy fits to the prose.
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


#: ★★★ TWO AXES, BECAUSE ONE OF THEM CALLS A TEST FILE A SHARED LIST.
#:
#: An append anchor has two properties and needs both. First, the two sides of
#: the collision only INSERTED lines — neither touched a line the other had —
#: which is what two pull requests appending a row to a table look like.
#: Second, it happens AT THE SAME PLACE more than once: a list every change
#: appends to collides there again and again.
#:
#: The first test alone was measured and rejected. It scored
#: `src/ai/advanced/tests.rs` at 10 of 10 "appends", because two pull requests
#: each adding a whole new `#[test]` function are also two pure insertions —
#: but at ten DIFFERENT functions, so there is no list to move anywhere and
#: the collisions are resolved by keeping both. On the same reading
#: `treatments.rs` put all ten of its collisions on two lines. The second test
#: separates them: a place is an anchor when at least this many distinct
#: replayed pairs collide there.
ANCHOR_REPEATS = 2

#: Two thirds rather than a bare majority, because the real cases are not
#: close. Measured 2026-08-23: `treatments.rs` and `elo.rs` are at 100%,
#: `tests.rs`, `treatment_flags.rs`, `game.rs` and `app.js` at 0-16%, and
#: `advanced.rs` in between — which is the honest answer for it, and prints as
#: BOTH.
ANCHOR_SHARE = 2 / 3

#: A file needs at least this many replayed collisions before `--modes` calls
#: it anything. One collision is a coin toss, not a mode.
MIN_COLLISIONS = 3

#: The innermost thing a conflicted region sits inside, for the four languages
#: this repository hand-writes. It names the anchor — `LIVE_TREATMENTS`,
#: `pub struct AdvancedAi`, `fn configured` — which is what a reader needs in
#: order to go and move it. Deliberately loose: a name that is slightly wrong
#: still points at the right part of the file, and an unrecognised line falls
#: back to `(top of file)` rather than guessing.
ITEM = re.compile(
    r"""^(?:
          [^\S\n]*(?:
              (?:pub(?:\([^)]*\))?\s+)?
              (?:default\s+|async\s+|unsafe\s+|extern\s+"[^"]*"\s+)*
              (?:fn|struct|enum|union|trait|impl|mod|type|const|static
                |macro_rules!)\b
            | (?:export\s+)?(?:async\s+)?(?:function|class)\s
            | (?:local\s+)?function\s
            | def\s+[A-Za-z_]
            )
          # A JavaScript module-level binding, and ONLY at column zero: an
          # indented `let` is a Rust local, and treating one as the enclosing
          # item names a variable where the reader needs the function.
        | (?:export\s+)?(?:const|let|var)\s+[A-Za-z_$][\w$]*\s*=
        )""",
    re.VERBOSE,
)


def blob(rev: str, path: str) -> str | None:
    """`path`'s content at `rev`, or None where it did not exist."""
    done = subprocess.run(["git", "-C", str(REPO), "show", f"{rev}:{path}"],
                          capture_output=True, check=False)
    if done.returncode:
        return None
    return done.stdout.decode("utf-8", "replace")


def _only_inserted(before: list[str], after: list[str]) -> bool:
    """True when `after` is `before` with whole lines inserted and none touched.

    An ordered-subsequence test, which is what "nobody edited an existing line"
    means once both sides are reduced to their text.
    """
    remaining = iter(after)
    return all(any(line == candidate for candidate in remaining)
               for line in before)


def _enclosing_item(lines: list[str], index: int) -> str:
    for line in reversed(lines[:index + 1]):
        if ITEM.match(line):
            return line.strip()[:72]
    return "(top of file)"


def _regions(merged: str) -> list[tuple[bool, str]]:
    """(both sides only appended, what the conflict sits inside) per region.

    `--diff3` prints the base between `|||||||` and `=======`, which is the
    whole point: without it there is no way to tell an append from an edit.
    """
    lines = merged.splitlines()
    out: list[tuple[bool, str]] = []
    start = None
    for index, line in enumerate(lines):
        if line.startswith("<<<<<<<"):
            start = index
        elif line.startswith(">>>>>>>") and start is not None:
            block = lines[start + 1:index]
            try:
                mid = block.index(next(x for x in block
                                       if x.startswith("|||||||")))
                sep = block.index("=======", mid)
            except (StopIteration, ValueError):
                out.append((False, _enclosing_item(lines, start)))
                start = None
                continue
            undone, base, later = (block[:mid], block[mid + 1:sep],
                                   block[sep + 1:])
            # ⚠ THE TWO SIDES ARE NOT SYMMETRIC AND THE FIRST VERSION OF THIS
            # SCORED EVERY FILE AT ZERO. `replay` puts the EARLIER merge
            # UNDONE on the ours side, so that merge's pure insertion appears
            # here as a pure deletion; only the later merge appears as an
            # insertion. An append collision is therefore: the earlier side
            # took whole lines out of the base and put none back, and the
            # later side put whole lines in and touched none.
            append = (_only_inserted(undone, base)
                      and _only_inserted(base, later))
            out.append((append, _enclosing_item(lines, start)))
            start = None
    return out


def replay(path: str, shas: list[str]) -> dict:
    """Replay consecutive merges touching `path` as if they were concurrent.

    ★★★ TOUCH RATE IS EXPOSURE; THIS IS THE COLLISION. Take two merges A then
    B that both touch `path` with nothing in between that does — so `path` at
    B's parent IS `path` at A — and ask git to undo A while merging B in:

        merge-file  ours = path@A^   base = path@A   theirs = path@B

    Reverting A out of the base and taking B on top is the three-way merge Git
    would have run had the two branched together, and it is run by the same
    algorithm on the real bytes rather than by a proxy for it. A conflict means
    A's edit and B's edit landed in the same place.

    ⚠ IT IS STILL A COUNTERFACTUAL, and the honest caveats are three. Those two
    merges were not necessarily written at the same time, though at a hundred
    merges a day two consecutive touches of one file usually are. It sees pairs
    and never the three-way pile-up. And a squash merge records the resolution
    and never the collision, so this reconstructs what would have happened
    rather than reading what did — the same limit the touch-rate ranking above
    states about itself.
    """
    hits = [sha for sha in reversed(shas) if path in touched(sha)]
    collisions = pairs = 0
    regions: list[tuple[bool, str, int]] = []
    for earlier, later in zip(hits, hits[1:]):
        base = blob(earlier, path)
        undone = blob(earlier + "^", path)
        after = blob(later, path)
        if base is None or undone is None or after is None:
            continue          # created or deleted in one of the two
        pairs += 1
        with tempfile.TemporaryDirectory() as scratch:
            sides = []
            for name, text in (("ours", undone), ("base", base),
                               ("theirs", after)):
                target = Path(scratch) / name
                target.write_text(text, encoding="utf-8")
                sides.append(str(target))
            done = subprocess.run(
                ["git", "merge-file", "-q", "-p", "--diff3", *sides],
                capture_output=True, text=True, errors="replace", check=False)
        if done.returncode > 0:
            collisions += 1
            regions.extend((append, where, pairs)
                           for append, where in _regions(done.stdout))
    return {"collisions": collisions, "pairs": pairs, "regions": regions}


def anchors(regions: list[tuple[bool, str, int]]) -> dict[str, int]:
    """Places where appends collided in more than one replayed pair.

    Counted in DISTINCT PAIRS, not regions: one pair can conflict twice inside
    the same function, and two conflicts from one collision are one event.
    """
    seen: dict[str, set[int]] = collections.defaultdict(set)
    for append, where, pair in regions:
        if append:
            seen[where].add(pair)
    return {where: len(pairs) for where, pairs in seen.items()
            if len(pairs) >= ANCHOR_REPEATS}


def modes(count: int, top: int) -> list[dict]:
    """The two failure modes, per contended file, most contended first."""
    shas = recent_merges(count)
    if not shas:
        raise SystemExit("no history to rank; fetch before measuring")
    out = []
    for path, hits, pct in ranking(count)[:top]:
        row = replay(path, shas)
        found = anchors(row["regions"])
        row.update({
            "path": path, "merges": hits, "pct": pct,
            "anchored": sum(1 for append, where, _ in row["regions"]
                            if append and where in found),
            "at": collections.Counter(found),
            "elsewhere": collections.Counter(
                where for append, where, _ in row["regions"]
                if not (append and where in found)),
        })
        out.append(row)
    return out


def verdict(row: dict) -> str:
    """ANCHOR, SPREAD, BOTH — or UNJUDGED below `MIN_COLLISIONS`."""
    total = len(row["regions"])
    if total < MIN_COLLISIONS:
        return "unjudged"
    share = row["anchored"] / total
    if share >= ANCHOR_SHARE:
        return "ANCHOR"
    if share <= 1 - ANCHOR_SHARE:
        return "SPREAD"
    return "BOTH"


REMEDY = {
    # Two problems, two remedies — `docs/ROADMAP.md` objective 5.
    "ANCHOR": "move the list out of source; splitting the file changes nothing",
    "SPREAD": "split along a seam; the data is not the problem",
    "BOTH": "move the lists out AND split; one remedy leaves the other tax",
    "unjudged": "too few replayed collisions to call",
}


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
    parser.add_argument("--modes", action="store_true",
                        help="replay each file's merges pairwise and split its "
                             "collisions into appends-to-one-list and "
                             "edits-spread-through-the-file")
    args = parser.parse_args(argv)

    shas = recent_merges(args.merges)
    if args.check and len(shas) < MIN_HISTORY:
        print(f"only {len(shas)} merges are reachable (need {MIN_HISTORY}); a "
              f"shallow clone reads every file at 0% and would pass this check "
              f"by measuring nothing. Check out with fetch-depth: 0.",
              file=sys.stderr)
        return 1
    if args.modes:
        # The window, so a reading quoted anywhere can be reproduced. Every
        # number here moves as merges land; `src/main.rs` went 16% -> 4% in
        # five days and took a CI job down with it.
        newest = subprocess.run(
            ["git", "-C", str(REPO), "show", "-s", "--format=%h %cs", shas[0]],
            capture_output=True, text=True, check=False).stdout.strip()
        print(f"of the {len(shas)} merges to main ending at {newest}, every "
              f"consecutive pair\nof merges touching one file replayed as if "
              f"the two had been written\nconcurrently:\n")
        for row in modes(args.merges, args.top):
            call = verdict(row)
            total = len(row["regions"])
            print(f"{row['path']}  —  {row['pct']}% of merges touch it")
            print(f"    {row['collisions']} of {row['pairs']} consecutive "
                  f"pairs would have collided, over {total} conflicted "
                  f"region(s)")
            print(f"    {row['anchored']} of {total} are two appends to one "
                  f"shared list  ->  {call}")
            print(f"    {REMEDY[call]}")
            for where, pairs in row["at"].most_common(4):
                print(f"        anchor  {pairs:3} pairs  {where}")
            for where, hits in row["elsewhere"].most_common(3):
                print(f"        spread  {hits:3}       {where}")
            print()
        return 0
    rows = ranking(args.merges)
    if not args.check:
        print(f"of the last {len(shas)} merges to main:")
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
