#!/usr/bin/env python3
"""Refuse a PR that silently deletes freshly merged work.

On 2026-08-05 the operator asked what had been overwriting what across the
fleet. Blaming every line deleted on `main` over the preceding week answered
it: sequential rewrites of the same hot regions — machines redesigning the
map-lens panel and the volcano tiles hours apart (#1109 → #1138 → #1191 →
#1199, #1153 → #1185 → #1226), and wide purges sweeping along fresh unrelated
lines (#969, #1194, #1155). Every one of those PRs merged green: nothing in CI
asked "who wrote the lines you are deleting, and how recently?" The damage was
only visible afterwards, in `Restore ...` commits (#1124, #1146).

This check asks that question before the merge. It blames every line the PR
deletes, at the PR's merge base. Deleting *old* lines is ordinary maintenance
and passes silently. Deleting lines that landed on `main` within the last
KEEP_DAYS is either deliberate supersession — in which case saying so costs
one line of the PR body — or exactly the accident this repository kept having.

A deletion of young work is acknowledged by naming its source anywhere in the
PR body: the victim PR's `#number` (a `Supersedes: #N` or `Coordinated with:
#N` line both qualify) or the victim commit's short sha. An acknowledged
deletion always passes; the check never judges whether replacing the work is
wise, only whether it is stated.

A genuinely bulk rewrite waives the whole check with a line of its own:

    overwrite-guard: allow <why this rewrite replaces the work it deletes>

⚠ That line is matched **anchored to the start of a line, and the reason is
mandatory**. It used to be neither: `WAIVER in body`, a bare substring, meant
any pull request whose body merely *mentioned* the marker switched the gate off
— prose about the guard, a quotation of this docstring, an indented block
showing the shape. #2328 found it on itself while writing `speed_ab.py`'s
`paired-cost: allow <reason>` hatch as an analogy to this one: its body
discussed the marker, and the guard printed "waived" where the author expected
victims. Nothing had in fact been waived silently — every pull request merged
that day was checked — but the hole was open from #1262 until now.

The two halves are one idea. Line-anchoring means writing *about* a switch
cannot flip it. A mandatory reason means the hatch costs a sentence, and that
sentence is exactly what #2059 never had to write. `speed_ab.py` spells its
hatch the same way on purpose, so the fleet learns one idiom rather than two.

Thresholds, tuned on the week that motivated this: a victim is reported when
the PR deletes >= 25 of its young lines, or when the PR deletes >= 50 young
lines across all victims combined (that aggregate is what caught #1155, which
shaved 7-14 lines from each of four fresh PRs). Adjacent-hunk churn of a few
lines never trips it. `AI_PLAYER_ELO_RANKINGS.md` is exempt: league refreshes
replace that whole snapshot several times a day by design.

    ./tools/overwrite_guard.py --base <base-ref> --head <sha> [--body-file F]

`--base` is a *ref*, not one end of a two-dot range: the tool takes
`merge-base(base, head)` itself. A local `--base origin/main` therefore judges
the branch's own deletions instead of every line `main` has gained since the
branch was cut — run two-dot, this told #2328's author their pull request was
"deleting 2036 lines from #2335", which it had never touched. `overwrite-
guard.yml` already passes the merge base, and the merge base of a merge base
and the head is itself, so CI is unchanged.

Exit 0 = clean or acknowledged. Exit 1 = unacknowledged young deletions, with
one `::error` line per victim naming the exact acknowledgment to add.
"""

from __future__ import annotations

import argparse
import collections
import os
import re
import subprocess
import sys
import time

KEEP_DAYS = 7
PER_VICTIM = 25
AGGREGATE = 50
MAX_RANGES_PER_FILE = 400
SNAPSHOT_PATHS = {"AI_PLAYER_ELO_RANKINGS.md"}

#: The escape hatch, spelled the way `tools/speed_ab.py` spells `paired-cost:
#: allow <reason>`: anchored to a line, case-insensitive, reason mandatory.
#: An optional list bullet is allowed because the ownership block this line
#: normally joins is a bulleted list. Arbitrary indentation is NOT allowed —
#: four leading spaces is a Markdown code block, and showing the marker is not
#: using it. (`speed_ab.py`'s copy permits leading whitespace; that is the one
#: deliberate difference, and the reason is the indented-block fixture below.)
WAIVER_MARKER = "overwrite-guard: allow"
WAIVER = re.compile(
    r"^(?:[-*+][ \t]+)?overwrite-guard:[ \t]*allow\b[ \t]*(?P<reason>.*)$",
    re.MULTILINE | re.IGNORECASE,
)

#: Fenced code blocks, blanked before the waiver is looked for. Documenting
#: this guard means showing the waiver line, and the clearest way to show it is
#: a fence — including in the pull request that adds this very check.
FENCE = re.compile(r"^ {0,3}(```+|~~~+)")


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], capture_output=True, text=True, encoding="utf-8",
        errors="replace", check=False,
    ).stdout


def prose(body: str) -> str:
    """The body with fenced code blocks blanked out, line numbering preserved.

    An unterminated fence swallows the rest of the body, which fails towards
    *fewer* waivers — the safe direction for a gate whose whole defect was
    waiving too easily.
    """
    kept: list[str] = []
    fence: str | None = None
    for line in body.splitlines():
        mark = FENCE.match(line)
        if fence is None:
            if mark:
                fence = mark.group(1)[:3]
                kept.append("")
            else:
                kept.append(line)
            continue
        if mark and mark.group(1).startswith(fence):
            fence = None
        kept.append("")
    return "\n".join(kept)


def waiver_reason(body: str) -> str | None:
    """The reason the body gives for waiving the whole check, or None.

    A bare `overwrite-guard: allow` is not a waiver. The marker is the part a
    reader greps for; the sentence after it is the part that makes the waiver a
    decision somebody can be answerable for rather than four words pasted in.
    """
    found = WAIVER.search(prose(body))
    if not found:
        return None
    return found.group("reason").strip() or None


def waiver_note(body: str) -> str | None:
    """Why a body that carries the marker did not waive the check.

    Silence is how the substring hole survived: the old matcher said nothing
    about the marker unless it had already waived. A body that mentions it and
    does not waive now hears which of the two reasons applies, so nobody has to
    discover the rule by watching a gate behave unexpectedly.
    """
    if WAIVER_MARKER.lower() not in body.lower():
        return None
    if WAIVER.search(prose(body)):
        return (f"the '{WAIVER_MARKER}' line carries no reason and so does not "
                f"waive; write '{WAIVER_MARKER} <why>'")
    return (f"the body mentions '{WAIVER_MARKER}' but not as a line of its own, "
            f"so it does not waive: indented, fenced, quoted or mid-sentence "
            f"text is discussion of the hatch, not a use of it")


def merge_base(base: str, head: str) -> str:
    """The fork point of `head` from `base` — what the branch itself changed.

    A two-dot `git diff origin/main HEAD` charges the branch with every line
    `main` has gained since the branch was cut. On this trunk that is not a
    corner case but the normal state after an hour: run that way, this tool
    told #2328 it was "deleting 2036 lines from #2335", a pull request it had
    never touched. A gate that cries wolf locally is a gate people stop running
    locally, and not running it locally is how the waiver hole lasted.

    `overwrite-guard.yml` already passes `merge-base(base.sha, head.sha)`, and
    the merge base of that commit and the head is itself, so CI is unaffected.
    """
    found = git("merge-base", base, head).strip()
    if re.fullmatch(r"[0-9a-f]{40}", found):
        return found
    print(f"warning: no merge base for '{base}' and '{head}'; judging against "
          f"'{base}' exactly as given", file=sys.stderr)
    return base


def deleted_ranges(base: str, head: str) -> dict[str, list[tuple[int, int]]]:
    """Old-side line ranges the PR deletes, per file, renames followed.

    Rename detection stays ON: with it off, a renamed file reads as a total
    deletion of young lines and a plain workflow rename (482215ef5) would be
    the loudest overwrite of the week.
    """
    out = git("diff", "-U0", "-M", "--no-color", base, head)
    ranges: dict[str, list[tuple[int, int]]] = collections.defaultdict(list)
    current = None
    for line in out.splitlines():
        if line.startswith("--- "):
            name = line[4:]
            current = None if name == "/dev/null" else name[2:] if name.startswith("a/") else name
        elif line.startswith("@@ ") and current:
            m = re.match(r"@@ -(\d+)(?:,(\d+))? \+", line)
            if not m:
                continue
            start = int(m.group(1))
            count = int(m.group(2)) if m.group(2) is not None else 1
            if count > 0:
                ranges[current].append((start, start + count - 1))
    return ranges


def young_victims(base: str, head: str, now: float) -> dict[str, int]:
    """Deleted-young-line counts keyed by the commit that wrote those lines."""
    victims: collections.Counter[str] = collections.Counter()
    cutoff = now - KEEP_DAYS * 86400
    for path, ranges in deleted_ranges(base, head).items():
        if path in SNAPSHOT_PATHS:
            continue
        args = ["blame", "--line-porcelain", base]
        for start, end in ranges[:MAX_RANGES_PER_FILE]:
            args += ["-L", f"{start},{end}"]
        args += ["--", path]
        blame = git(*args)
        if not blame:
            continue  # binary, unreadable, or path absent at base
        origin = None
        for row in blame.splitlines():
            m = re.match(r"^([0-9a-f]{40}) \d+ \d+", row)
            if m:
                origin = m.group(1)
            elif row.startswith("committer-time ") and origin:
                if int(row.split()[1]) >= cutoff:
                    victims[origin] += 1
    return dict(victims)


def acknowledged(body: str, sha: str, subject: str) -> bool:
    """A victim is acknowledged by its PR number or its commit sha prefix.

    Deliberately matched anywhere in the body, unlike the waiver: naming the
    work you are replacing is a positive act, and `Supersedes:`, `Coordinated
    with:` and a sentence of prose are all honest ways to do it. The number
    does need a boundary, though — without one, victim #1 counted any mention
    of #1109 as its own acknowledgement.
    """
    if sha[:7] in body:
        return True
    m = re.search(r"\(#(\d+)\)\s*$", subject)
    return bool(m) and bool(re.search(rf"#{m.group(1)}\b", body))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="origin/main",
                        help="base ref; the tool takes merge-base(base, head), "
                             "so a moving branch tip is safe here")
    parser.add_argument("--head", default="HEAD", help="PR head sha")
    parser.add_argument("--body-file", help="file holding the PR body")
    parser.add_argument("--now", type=float, default=None,
                        help="override the clock (tests)")
    args = parser.parse_args(argv)

    body = ""
    if args.body_file and os.path.exists(args.body_file):
        with open(args.body_file, encoding="utf-8", errors="replace") as handle:
            body = handle.read()
    reason = waiver_reason(body)
    if reason:
        print(f"waived: '{WAIVER_MARKER}' — {reason}")
        return 0
    note = waiver_note(body)
    if note:
        print(f"note: {note}", file=sys.stderr)

    base = merge_base(args.base, args.head)
    if base != args.base:
        print(f"base: merge-base({args.base}, {args.head}) = {base[:9]}")
    victims = young_victims(base, args.head, args.now or time.time())
    if not victims:
        print("no young deletions; nothing to acknowledge")
        return 0

    unacknowledged = []
    for sha, count in sorted(victims.items(), key=lambda kv: -kv[1]):
        subject = git("log", "-1", "--format=%s", sha).strip()
        if acknowledged(body, sha, subject):
            print(f"    ok   -{count:<5} {sha[:9]} {subject[:70]} (acknowledged)")
        else:
            unacknowledged.append((sha, count, subject))

    flagged = [v for v in unacknowledged if v[1] >= PER_VICTIM]
    total = sum(count for _, count, _ in unacknowledged)
    if not flagged and total < AGGREGATE:
        for sha, count, subject in unacknowledged:
            print(f"    ok   -{count:<5} {sha[:9]} {subject[:70]} (below threshold)")
        return 0

    reportable = flagged or unacknowledged
    print(
        f"\nThis PR deletes {total} lines merged to main within the last "
        f"{KEEP_DAYS} days without saying so.",
        file=sys.stderr,
    )
    for sha, count, subject in reportable:
        m = re.search(r"\(#(\d+)\)\s*$", subject)
        name = f"#{m.group(1)}" if m else sha[:9]
        print(
            f"::error title=overwrite-guard::deletes {count} recent lines from "
            f"{name} ({subject[:80]}). If replacing that work is intended, add "
            f"'Supersedes: {name}' to the PR body; otherwise rebuild on top of it.",
        )
    return 1


if __name__ == "__main__":
    sys.exit(main())
