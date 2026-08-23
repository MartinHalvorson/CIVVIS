#!/usr/bin/env python3
"""Create, validate, and monitor isolated CIVVIS agent tasks.

This tool is intentionally dependency-free so the same workflow runs on macOS,
Linux, and Windows. Git owns local isolation, GitHub draft PRs own fleet-visible
claims, and this script checks the contract at both boundaries.
"""

from __future__ import annotations

import argparse
import datetime as dt
import fnmatch
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import secrets
import shutil
import stat
import subprocess
import sys
import time
from typing import Any, Dict, Iterable, List, Optional, Sequence, Set, Tuple
import urllib.error
import urllib.request


REPOSITORY = "MartinHalvorson/CIVVIS"
DEFAULT_BRANCH = "main"
#: Checks `ship` will not merge without. ⚠ THERE IS NO BRANCH PROTECTION AND NO
#: RULESET ON THIS REPOSITORY — both API reads come back empty — so this tuple
#: is the only thing that makes a check binding. A gate absent from here is
#: advisory no matter how red it goes.
#:
#: `rust-quality` was absent, and it was not a theoretical gap: five commits on
#: `main` in the forty most recent runs are red on it, and #1954 merged with its
#: final `rust-quality` run FAILING while every other check was green. The
#: format-and-clippy ratchet is scoped to the lines a change touches precisely
#: so it is always satisfiable; a ratchet nobody has to pass ratchets nothing,
#: and it trains a fleet at a hundred merges a day to read red as normal.
#:
#: `paired-cost` joined on 2026-08-23. Advisory it could not do the one job it
#: exists for: `ship` waits on this tuple and merges without reading anything
#: else, so #2059 — six times slower, four PRs and four days to pay back —
#: would have merged again with a red advisory X beside it. What made it safe
#: to require is not confidence, it is three properties the check now has:
#: it always reports (no `paths:` filter and no job-level `if:`, because an
#: absent required check reads as pending here and a skipped one reads as a
#: failure); its verdict is the MEDIAN of five paired blocks and it re-measures
#: on a disjoint block of seeds before failing anything, so one contended pair
#: cannot fail a merge; and an intended cost is accepted by a
#: `paired-cost: allow <reason>` line in the pull request body, the same escape
#: hatch `overwrite-guard: allow` is. A false failure is cleared by re-running
#: the job — `gh run rerun --failed <run-id>`, which `ship` already does for
#: itself on a cancelled or timed-out run.
def _eval_manifest_outputs() -> Tuple[str, ...]:
    """`eval_manifest.GENERATED_OUTPUTS`, without importing the module eagerly.

    `civvis_collab.py` is copied to machines as a standalone freshness worker,
    so a hard import would make the launcher depend on a file the worker never
    ships with. Missing, it simply resolves nothing automatically.
    """
    try:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        from eval_manifest import GENERATED_OUTPUTS  # type: ignore[import-not-found]
    except Exception:
        return ()
    return tuple(GENERATED_OUTPUTS)


#: Kept in sorted order: `required_check_state` reports missing checks in
#: this tuple's order and `test_civvis_collab.py` compares that report
#: with `sorted(REQUIRED_CHECKS)`.
REQUIRED_CHECKS = (
    "cargo-test",
    "collaboration-policy",
    "paired-cost",
    "rust-quality",
)

#: Generated artifacts a merge conflict may be resolved in by regenerating,
#: mapped to the command that rebuilds them (argv relative to the repo root,
#: run with this interpreter).
#:
#: A path belongs here only if all three hold: its content is a deterministic
#: function of tracked source, rebuilding it is cheap, and nothing about it is
#: measured. `docs/TACTICS_BASELINE.md` looks similar and fails the third —
#: `tools/tactics_bench.py --write-baseline` runs a benchmark, so its output
#: depends on the machine that ran it and must never be regenerated to settle
#: somebody else's merge.
#:
#: The eval pair is discovered from the generator rather than spelled again
#: here, so a third artifact cannot be added to `eval_manifest.py` and quietly
#: left out of this map.
REGENERATED_ON_MERGE: Dict[str, Tuple[str, ...]] = {
    name: ("tools/eval_manifest.py", "--write")
    for name in _eval_manifest_outputs()
}

#: Every other check a pull request gets, with the reason it is NOT required.
#: `test_civvis_collab.py` discovers the checks the workflows actually produce
#: and fails when one is in neither place — so a new gate is a deliberate
#: decision rather than an omission nobody can tell from an oversight.
ADVISORY_CHECKS = {
    "overwrite-guard":
        "Blocking-worthy, and deliberately queued behind the cancelled-check "
        "retry: it is cancelled on 15 of its 50 most recent runs (body edits "
        "and ready-for-review transitions retrigger it), and GitHub does not "
        "restart a cancelled run on its own. Requiring it before `ship` retries "
        "a cancellation would strand pull requests on a non-verdict.",
    "published-build":
        "A job inside `cargo-test`'s workflow, and skipped by design on a diff "
        "of zero bytes. Never observed red on a merged pull request; promote it "
        "when there is evidence it can be, not on the strength of its name.",
    "control-mod":
        "Same workflow and same reasoning as `published-build`.",
}
# Terminal check conclusions that say nothing about the code. A run that was
# superseded by its own concurrency group, killed by the runner clock, or
# marked stale never reached a verdict — and, critically, GitHub will not start
# another one on its own. Auto-merge waits for a required check to go green, so
# a check that ends in one of these and is never re-run leaves the PR open
# forever with nothing watching it. `ship` re-runs them instead of reporting a
# failure the code did not earn. `failure` is deliberately absent: that IS a
# verdict, and re-running it would just burn CI on a broken tree.
RETRYABLE_CHECK_CONCLUSIONS = frozenset({"cancelled", "timed_out", "stale"})
# How many times `ship` re-runs one required check before it stops and says so.
# A check that is cancelled twice is not being superseded; something is wrong
# with it, and quietly re-running forever would hide that.
CHECK_RERUN_LIMIT = 2
# How long `ship` waits for the production spectator to be LISTENING at all
# before deciding it is not running. Distinct from `--live-timeout-seconds`,
# which is how long a spectator that IS up may take to reach the merged
# revision. `ship` runs when a spectator may be restarting onto that revision,
# so this is a grace for it to reappear, not a single probe.
LIVE_PRESENCE_GRACE_S = 30.0
# A supervised spectator finishes its current game before rebuilding from main.
# A four-seat, 250-turn exhibition can exceed the old ten-minute allowance, so
# ``ship`` must wait through that safe handoff instead of reporting an
# unverified merge while the healthy spectator is still serving the old build.
LIVE_BUILD_HANDOFF_TIMEOUT_S = 30 * 60.0
BRANCH_RE = re.compile(
    r"^agent/(?P<machine>[a-z0-9][a-z0-9-]{0,31})/"
    r"(?P<agent>[a-z0-9][a-z0-9-]{0,31})/"
    r"(?P<task>[a-z0-9][a-z0-9-]{0,47})-"
    r"(?P<stamp>\d{8}T\d{6}Z)-(?P<nonce>[a-f0-9]{4,12})$"
)
ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,31}$")
TASK_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,47}$")
PUSH_GUARD_MARKER = "CIVVIS managed pre-push guard v1"
FRESHNESS_MARKER = "CIVVIS managed Git freshness service v1"
FRESHNESS_SCHEMA = 2
FRESHNESS_INTERVAL_SECONDS = 300
FRESHNESS_STALE_SECONDS = 15 * 60
FRESHNESS_LOCK_STALE_SECONDS = 30 * 60
FIELD_LABELS = {
    "machine": "Machine ID",
    "agent": "Agent/session ID",
    "task": "Task",
    "paths": "Claimed paths",
    "coordinated": "Coordinated with",
}
PLACEHOLDERS = {"", "todo", "tbd", "fill me", "n/a"}

# --- R3: an effect size must carry its evidence into the record -------------
#
# Every headline number in this repository is the point estimate of the run that
# promoted it, so every one is conditioned on having passed a gate and is biased
# upward. They replicate in direction and fail downward in size: +207 -> +86,
# +92 -> +61, and `strategic_deep`'s +45 -> -8, which #482 *excluded* rather
# than merely failed to reproduce. See `docs/EVAL_INTEGRITY.md` §4.
#
# The instance that motivates a mechanical check is the last one. The +45 lived
# in `docs/GENOME.md` as a bare promoted figure; the refutation reached a PR body
# and never reached the document, so the discovery estimate stayed in the record
# and the replication did not. A bare number in a document outlives the run that
# produced it, and nothing here could tell the two apart.
#
# The rule is therefore about *evidence adjacency*, not about the number: an Elo
# figure added to a document must sit near something that says where it came
# from. That admits well-sourced prose — including a refutation citing the number
# it refutes, which necessarily carries a CI, a map count or a PR reference — and
# rejects the bare promoted claim, which by construction carries none.
EVIDENCE_DOC_RE = re.compile(r"^(docs/.*\.md|README\.md)$")
EFFECT_SIZE_RE = re.compile(
    r"""(?ix)
    (?: elo[-\s]?equivalent \s* [+−-]? \d+     # "Elo-equivalent +207"
      | [+−-] \d{2,4} \s* elo                  # "+45 Elo"
      | \b elo \s* (?:of|at) \s* [+−-]? \d+    # "Elo of +61"
    )
    """
)
EVIDENCE_RE = re.compile(
    r"""(?ix)
    (?: \b seeds? \b
      | \b CI \b | 95\s*% | ±                  # an interval
      | \b\d+\s*(?:maps|pairs|games)\b              # the evidence base
      | \bPR\s*\#?\d+ | \#\d{2,}                    # where it was measured
      | discovery\s+estimate | confirmed\s+on | disjoint
      | p\s*[=<>]                                   # a reported p-value
    )
    """
)


#: How much added prose around a figure counts as "beside it". Wide enough to
#: reach the sentence that sources the number, narrow enough that an unrelated
#: measurement elsewhere in the same hunk cannot launder a bare claim.
EVIDENCE_WINDOW_CHARS = 320


def unevidenced_effect_sizes(added: Dict[str, Sequence[str]]) -> List[str]:
    """Added doc prose that states an effect size without saying where it came from.

    Matched against the added lines *joined*, not line by line. These documents
    are wrapped at 80 columns, so the real instance this exists to catch —
    ``strategic_deep`` at +45 / Elo in `docs/GENOME.md` — puts the figure at the
    end of one line and the unit at the start of the next, and a per-line scan
    sees neither.
    """
    problems: List[str] = []
    for path in sorted(added):
        if not EVIDENCE_DOC_RE.match(path):
            continue
        joined = " ".join(line.strip() for line in added[path])
        for match in EFFECT_SIZE_RE.finditer(joined):
            start = max(0, match.start() - EVIDENCE_WINDOW_CHARS)
            window = joined[start : match.end() + EVIDENCE_WINDOW_CHARS]
            if EVIDENCE_RE.search(window):
                continue
            quoted = joined[max(0, match.start() - 40) : match.end() + 40].strip()
            problems.append(
                f"{path} adds an effect size with no evidence beside it: "
                f"...{quoted}... A promoted number is selected on having passed a "
                "gate, so it is biased upward and is not quotable alone "
                "(docs/EVAL_INTEGRITY.md §4). Cite the seed, the interval, the map "
                "count or the PR it was measured in, or mark it a DISCOVERY ESTIMATE."
            )
    return problems


class CommandError(RuntimeError):
    pass


def run(
    args: Sequence[str],
    *,
    cwd: Optional[Path] = None,
    check: bool = True,
    capture: bool = True,
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        list(args),
        cwd=str(cwd) if cwd else None,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.PIPE if capture else None,
        check=False,
    )
    if check and result.returncode:
        rendered = " ".join(args)
        detail = (result.stderr or result.stdout or "").strip()
        raise CommandError(f"{rendered} failed ({result.returncode}): {detail}")
    return result


def git(repo: Path, *args: str, check: bool = True) -> str:
    return run(("git", "-C", str(repo), *args), check=check).stdout.strip()


def repo_root(path: Optional[Path] = None) -> Path:
    start = path or Path.cwd()
    result = run(("git", "-C", str(start), "rev-parse", "--show-toplevel"))
    return Path(result.stdout.strip()).resolve()


def clean_token(value: str) -> str:
    return value.strip().strip("`").strip()


def parse_claims(body: str) -> Dict[str, str]:
    claims: Dict[str, str] = {}
    wanted = {label.lower(): key for key, label in FIELD_LABELS.items()}
    for raw in body.splitlines():
        match = re.match(r"^\s*-\s*([^:]+):\s*(.*?)\s*$", raw)
        if not match:
            continue
        key = wanted.get(match.group(1).strip().lower())
        if key:
            claims[key] = clean_token(match.group(2))
    return claims


MACHINE_REGISTRY = Path("docs/MACHINES.md")


def machine_registry(path: Path = MACHINE_REGISTRY) -> Optional[Set[str]]:
    """Every machine ID `docs/MACHINES.md` knows, canonical or alias.

    The registry exists because one physical laptop introduced itself under
    four different branch IDs, which made the 2026-08-05 overwrite forensics
    needlessly hard. The parse is deliberately permissive — any backticked
    token that would be a valid branch machine ID counts, whether it sits in
    the canonical table, an alias column, or the unresolved list — because
    the registry is a ratchet toward stable names, not a gate. Returns None
    when the file is absent so callers can stay silent rather than nag every
    checkout that predates it.
    """
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return None
    return {
        token
        for token in re.findall(r"`([a-z0-9][a-z0-9-]{0,31})`", text)
        if ID_RE.fullmatch(token)
    }


def split_paths(raw: str) -> List[str]:
    return [clean_token(item) for item in raw.split(",") if clean_token(item)]


def split_coordination(raw: str) -> Set[int]:
    return {int(value) for value in re.findall(r"#(\d+)", raw)}


def valid_claim_pattern(pattern: str) -> bool:
    if not pattern or pattern in {"*", "**", ".", "./"}:
        return False
    if pattern.startswith(("/", "\\")):
        return False
    parts = pattern.replace("\\", "/").split("/")
    return ".." not in parts and all(parts)


def path_is_claimed(path: str, patterns: Iterable[str]) -> bool:
    normalized = path.replace("\\", "/")
    return any(fnmatch.fnmatchcase(normalized, pattern) for pattern in patterns)


def claim_patterns_overlap(left: str, right: str) -> bool:
    if left == right:
        return True
    left_prefix = left[:-3] if left.endswith("/**") else None
    right_prefix = right[:-3] if right.endswith("/**") else None
    if left_prefix and (right == left_prefix or right.startswith(left_prefix + "/")):
        return True
    if right_prefix and (left == right_prefix or left.startswith(right_prefix + "/")):
        return True
    return fnmatch.fnmatchcase(left, right) or fnmatch.fnmatchcase(right, left)


def claims_overlap(left: Iterable[str], right: Iterable[str]) -> bool:
    return any(claim_patterns_overlap(a, b) for a in left for b in right)


HUNK_RE = re.compile(r"^@@ -(?P<start>\d+)(?:,(?P<count>\d+))? \+")
# Git needs three lines of unchanged context to merge two edits cleanly, so two
# hunks that stop within this many lines of each other are treated as touching.
MERGE_CONTEXT_LINES = 3


def patch_base_ranges(patch: str) -> List[Tuple[int, int]]:
    """Return inclusive base-side line ranges touched by a unified diff.

    Only the pre-image (``-``) side matters: two branches conflict when they
    rewrite the same original lines. A pure insertion has count 0; it is
    recorded as a zero-width range at the insertion point so that two branches
    inserting at the same seam still register as touching.
    """
    ranges: List[Tuple[int, int]] = []
    for line in (patch or "").splitlines():
        match = HUNK_RE.match(line)
        if not match:
            continue
        start = int(match.group("start"))
        count = int(match.group("count") or 1)
        if count == 0:
            ranges.append((start, start))
        else:
            ranges.append((start, start + count - 1))
    return ranges


def ranges_touch(
    left: Sequence[Tuple[int, int]],
    right: Sequence[Tuple[int, int]],
    *,
    context: int = MERGE_CONTEXT_LINES,
) -> bool:
    """Return whether any two line ranges overlap or sit within ``context``."""
    for left_start, left_end in left:
        for right_start, right_end in right:
            if left_start - context <= right_end and right_start - context <= left_end:
                return True
    return False


def file_edits_collide(
    mine: Optional[Sequence[Tuple[int, int]]],
    theirs: Optional[Sequence[Tuple[int, int]]],
) -> bool:
    """Return whether two PRs' edits to one file can actually conflict.

    ``None`` means the ranges could not be determined (binary file, or a diff
    GitHub truncated). Those fall back to whole-file collision so the policy
    stays conservative exactly where it cannot see detail.
    """
    if mine is None or theirs is None:
        return True
    if not mine or not theirs:
        return True
    return ranges_touch(mine, theirs)


def colliding_paths(
    mine: Dict[str, Optional[List[Tuple[int, int]]]],
    theirs: Dict[str, Optional[List[Tuple[int, int]]]],
) -> List[str]:
    """Return the shared paths whose edits actually touch the same lines."""
    return sorted(
        path
        for path in set(mine) & set(theirs)
        if file_edits_collide(mine[path], theirs[path])
    )


#: The anchor pin, exempted from collision reporting when a PR edits nothing
#: else in its file.
#:
#: ⚠ THIS EXEMPTION IS A WORKAROUND FOR A PROBLEM THAT NO LONGER EXISTS, KEPT
#: NARROW ON PURPOSE. It was built for `ADVANCED_V1_SOURCE_CONTRACT_FNV`, a
#: hash over every byte of `src/ai.rs` and `src/ai/advanced.rs`: any two PRs
#: touching either file collided on that one line by construction, every merge
#: invalidated the value on every other open PR, and the resulting "coordinate
#: with #N" bookkeeping never converged because the colliding set changed on
#: every merge. On 2026-08-03, 18 of the 41 commits that reached `main` touched
#: it; over the thirty days to 2026-08-17, 248 of ~1,669.
#:
#: #1841 removed the cause. `ANCHOR_BEHAVIOUR_FNV` pins what `advanced_v1`
#: DOES — its decision stream over five profiles — so an ordinary change to
#: either AI file moves nothing, and this line is expected to change only when
#: the Elo protocol does. The exemption stays because that rare edit still
#: collides for the same meaningless reason, and because a gate that has to be
#: re-added under pressure is a gate that is not there. It should now almost
#: never fire; if it starts firing often again, something has reintroduced a
#: pin that moves on every edit.
SOURCE_CONTRACT_PIN = "ANCHOR_BEHAVIOUR_FNV"
SOURCE_CONTRACT_FILE = "src/main.rs"


def is_pin_only_edit(added: Sequence[str]) -> bool:
    """True when every added line is the contract pin or its doc paragraph."""
    meaningful = [line for line in added if line.strip()]
    if not meaningful:
        return False
    saw_pin = False
    for line in meaningful:
        stripped = line.strip()
        if SOURCE_CONTRACT_PIN in stripped:
            saw_pin = True
            continue
        if stripped.startswith("///") or stripped.startswith("#[cfg(test)]"):
            continue
        return False
    return saw_pin


def drop_pin_only_collisions(
    collisions: Sequence[str],
    added_lines: Optional[Dict[str, Sequence[str]]],
) -> List[str]:
    """Remove the source-contract pin from a collision list. See SOURCE_CONTRACT_PIN."""
    if not added_lines:
        return list(collisions)
    return [
        path
        for path in collisions
        if not (
            path == SOURCE_CONTRACT_FILE
            and is_pin_only_edit(added_lines.get(path) or [])
        )
    ]


def as_range_map(
    files: Iterable[str],
    ranges: Optional[Dict[str, Optional[List[Tuple[int, int]]]]],
) -> Dict[str, Optional[List[Tuple[int, int]]]]:
    """Pair every changed path with its line ranges, or ``None`` when unknown."""
    known = ranges or {}
    return {path: known.get(path) for path in files}


def validate_pr(
    pr: Dict[str, Any],
    *,
    files: Sequence[str],
    commit_subjects: Sequence[str],
    other_files: Optional[Dict[int, Set[str]]] = None,
    ranges: Optional[Dict[str, Optional[List[Tuple[int, int]]]]] = None,
    other_ranges: Optional[Dict[int, Dict[str, Optional[List[Tuple[int, int]]]]]] = None,
    other_coordination: Optional[Dict[int, Set[int]]] = None,
    advisories: Optional[List[str]] = None,
    added_lines: Optional[Dict[str, Sequence[str]]] = None,
) -> List[str]:
    number = int(pr.get("number", 0))
    branch = str(pr.get("headRefName") or pr.get("head", {}).get("ref") or "")
    body = str(pr.get("body") or "")
    draft = bool(pr.get("isDraft", pr.get("draft", False)))
    errors: List[str] = []

    branch_match = BRANCH_RE.fullmatch(branch)
    if not branch_match:
        errors.append(
            "head branch must match "
            "agent/<machine>/<agent>/<task>-<YYYYMMDDTHHMMSSZ>-<nonce>; "
            "do not rename this branch, start a new task with: "
            "python3 tools/civvis_collab.py start <task-slug> --path <path>"
        )

    claims = parse_claims(body)
    for key, label in FIELD_LABELS.items():
        value = claims.get(key, "").strip().lower()
        if value in PLACEHOLDERS:
            errors.append(f"PR body field '{label}' must be filled")

    if branch_match:
        if claims.get("machine") != branch_match.group("machine"):
            errors.append("Machine ID must match the branch machine component")
        if claims.get("agent") != branch_match.group("agent"):
            errors.append("Agent/session ID must match the branch agent component")

    patterns = split_paths(claims.get("paths", ""))
    invalid = [pattern for pattern in patterns if not valid_claim_pattern(pattern)]
    if invalid:
        errors.append("invalid claimed path patterns: " + ", ".join(invalid))
    for changed in files:
        if patterns and not path_is_claimed(changed, patterns):
            errors.append(
                f"changed path is not claimed: {changed}; either revert it or add "
                f"`{changed}` to the 'Claimed paths:' line of this PR body"
            )

    for subject in commit_subjects:
        if subject.lower().startswith("autosync:"):
            errors.append(f"mutating autosync commit is forbidden: {subject}")

    errors.extend(unevidenced_effect_sizes(added_lines or {}))

    coordinated = split_coordination(claims.get("coordinated", ""))
    mine = as_range_map(files, ranges)
    notes = advisories if advisories is not None else []
    for other_number in sorted({*(other_files or {}), *(other_ranges or {})}):
        if other_ranges and other_number in other_ranges:
            theirs = other_ranges[other_number]
        else:
            theirs = {path: None for path in (other_files or {}).get(other_number, ())}
        shared = sorted(set(mine) & set(theirs))
        if not shared:
            continue
        collisions = drop_pin_only_collisions(
            colliding_paths(mine, theirs), added_lines
        )
        if not collisions:
            # Same file, disjoint regions. Git merges this cleanly, so it is
            # information for the author, never a gate.
            notes.append(
                f"PR #{other_number} edits the same file(s) in different places, "
                f"no action needed: {', '.join(shared[:5])}"
            )
            continue
        preview = ", ".join(collisions[:5])
        detail = (
            f"edits collide with PR #{other_number} on the same lines of {preview}"
        )
        if other_number in coordinated:
            continue
        # A newly started task records its existing neighbours automatically.
        # Its older neighbour cannot have known that future PR number when it
        # first became ready, so accept that explicit reverse declaration too.
        # This keeps a harmless CI re-run or main refresh from blocking the
        # older PR solely because its body predates the newer task.
        if number in (other_coordination or {}).get(other_number, set()):
            notes.append(
                f"{detail} — PR #{other_number} already declares coordination "
                f"with PR #{number}"
            )
            continue
        if draft:
            notes.append(
                f"{detail} — resolve before marking ready, or add '#{other_number}' "
                "to 'Coordinated with:' in this PR body"
            )
        else:
            errors.append(
                f"{detail}; coordinate in the older PR, then add '#{other_number}' "
                "to the 'Coordinated with:' line of this PR body"
            )

    if not draft and re.search(r"^\s*- \[ \]", body, re.MULTILINE):
        errors.append(
            "ready PRs must complete every validation checkbox; run each listed "
            "check, then change its '- [ ]' to '- [x]' in this PR body"
        )

    return errors


def compare_status_is_current(status: str) -> bool:
    """Return whether a PR head contains the current base branch tip."""
    return status in {"ahead", "identical"}


# How many commits behind main a ready PR may be before staleness stops being
# advice and becomes a refusal. At this repository's velocity 150 commits is
# under a day; at any velocity, 150 unseen changes is 150 chances for the
# merge to be a combination no CI run has tested.
STALE_BASE_LIMIT = 150


def base_staleness(status: str, behind_by: int) -> Optional[Tuple[str, str]]:
    """Classify how far a ready PR trails its base: None, advisory, or error.

    An honest account of what enforces freshness here, because a previous
    comment got it wrong and the wrongness propagated into design decisions:
    branch protection runs with `strict=false` — deliberately, since at
    hundreds of merges a day requiring every branch to contain the current
    tip would re-queue every open PR after every merge — so GitHub refuses
    nothing on staleness, and the real mitigations are `ship` re-merging main
    before ready and this check. Mild staleness therefore stays an advisory:
    main moves again during CI and a red X the author cannot durably clear
    teaches people to ignore red. Severe staleness is different. A branch
    hundreds of commits behind merges as a tree nobody's CI has seen, which
    is one of the doors the 2026-08-05 cross-machine overwrites walked
    through. The refusal is durably fixable, unlike strict mode: one
    `git merge origin/main` resets the distance to zero, and main advancing
    during CI cannot re-cross a STALE_BASE_LIMIT-commit threshold.
    """
    if compare_status_is_current(status):
        return None
    if behind_by >= STALE_BASE_LIMIT:
        return (
            "error",
            f"this branch is {behind_by} commits behind main — far past the "
            f"{STALE_BASE_LIMIT}-commit limit at which a merge becomes a "
            "combination no CI run has tested. Run: git fetch origin main && "
            "git merge origin/main, resolve, revalidate, push",
        )
    return (
        "advisory",
        f"main advanced while this PR was open ({behind_by} commits). "
        "Nothing enforces freshness below the "
        f"{STALE_BASE_LIMIT}-commit limit — merging main before ship is on "
        "the author. Run: git fetch origin main && git merge origin/main",
    )


def comparison_or_reason(fetch) -> Tuple[Optional[Dict[str, Any]], str]:
    """Run one compare request. ``(comparison, "")`` or ``(None, why not)``.

    ⚠⚠⚠ AN ADVISORY THAT CANNOT RUN MUST NOT BLOCK A MERGE, AND FOR ONE
    AFTERNOON THIS ONE BLOCKED EVERY MERGE IN THE REPOSITORY.
    `/repos/.../compare/A...B` began returning 404 for this repository on
    2026-08-17 — for a PR head one commit ahead of `main`, and for two
    adjacent commits *on* `main`, while `/commits/<sha>` resolved both SHAs
    fine. Whatever the cause, it is not something a branch can fix. The call
    was unguarded in all three of its call sites, so the exception escaped
    `check_pr_action`, `collaboration-policy` exited 1, and a required check
    nobody could turn green stopped the fleet.

    The staleness check this feeds is deliberately advisory below
    `STALE_BASE_LIMIT` (see `base_staleness`), which makes the failure mode
    doubly wrong: an *unmeasurable* distance was reported as a *violation*,
    and the one distance that is a real refusal — 150+ commits behind — is
    precisely the one this function cannot confirm when it returns `None`.
    So the callers degrade to the advisory, never to the error: "could not
    measure" is not "measured as bad", and the remedy is the same one
    ordinary staleness asks for.

    A body without ``status`` is treated as no answer for the same reason —
    ``base_staleness("")`` would silently read as "not current" and invent a
    verdict out of a malformed response.
    """
    try:
        comparison = fetch()
    except CommandError as exc:
        return None, str(exc)
    if not isinstance(comparison, dict) or not comparison.get("status"):
        return None, "GitHub returned a comparison with no status"
    return comparison, ""


UNMEASURED_BASE_ADVISORY = (
    "could not measure how far this branch trails main ({reason}). Treating "
    "it as ordinary staleness rather than a violation: a check that cannot "
    "run must not block a merge. Merging main before ship is on the author — "
    "run: git fetch origin main && git merge origin/main"
)


def github_json(path: str, token: str) -> Any:
    url = f"https://api.github.com{path}"
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "User-Agent": "civvis-collaboration-policy",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", "replace")
        raise CommandError(f"GitHub API {path} failed ({exc.code}): {detail}") from exc


def pr_files(repository: str, number: int, token: str) -> List[str]:
    rows = github_json(f"/repos/{repository}/pulls/{number}/files?per_page=100", token)
    return [str(row["filename"]) for row in rows]


def pr_file_ranges(
    repository: str, number: int, token: str
) -> Dict[str, Optional[List[Tuple[int, int]]]]:
    """Map every changed path to the base-side line ranges the PR rewrites.

    A path maps to ``None`` when GitHub does not return a patch (binary content
    or a diff too large to inline); callers must treat that as whole-file.
    """
    rows = github_json(f"/repos/{repository}/pulls/{number}/files?per_page=100", token)
    ranges: Dict[str, Optional[List[Tuple[int, int]]]] = {}
    for row in rows:
        patch = row.get("patch")
        name = str(row["filename"])
        ranges[name] = patch_base_ranges(str(patch)) if patch else None
    return ranges


def patch_added_lines(patch: str) -> List[str]:
    """The added-side content of a unified diff, without the '+' marker.

    `+++ b/path` is a header, not an addition; it is the only '+'-prefixed line
    that must not be treated as content.
    """
    return [
        line[1:]
        for line in patch.splitlines()
        if line.startswith("+") and not line.startswith("+++")
    ]


def pr_added_lines(repository: str, number: int, token: str) -> Dict[str, List[str]]:
    """Map every changed path to the lines this PR adds to it.

    A path with no inlined patch (binary, or a diff too large) maps to an empty
    list: the effect-size gate can only judge text it can see, and silently
    passing an unreadable diff is the correct failure direction for a lint.
    """
    rows = github_json(f"/repos/{repository}/pulls/{number}/files?per_page=100", token)
    added: Dict[str, List[str]] = {}
    for row in rows:
        patch = row.get("patch")
        added[str(row["filename"])] = patch_added_lines(str(patch)) if patch else []
    return added


def pr_commit_subjects(repository: str, number: int, token: str) -> List[str]:
    rows = github_json(f"/repos/{repository}/pulls/{number}/commits?per_page=100", token)
    return [str(row["commit"]["message"]).splitlines()[0] for row in rows]


def check_pr_action(event_path: Path, token: str, repository: str) -> int:
    event = json.loads(event_path.read_text(encoding="utf-8"))
    if "pull_request" not in event:
        print("collaboration policy: non-PR event, nothing to validate")
        return 0

    current = dict(event["pull_request"])
    current["headRefName"] = current.get("head", {}).get("ref", "")
    current["isDraft"] = current.get("draft", False)
    number = int(current["number"])
    ranges = pr_file_ranges(repository, number, token)
    files = sorted(ranges)
    subjects = pr_commit_subjects(repository, number, token)
    open_prs = github_json(f"/repos/{repository}/pulls?state=open&per_page=100", token)
    # Only PRs that share at least one path can collide, so fetch patches for
    # those alone instead of every open PR on the repository.
    other_ranges: Dict[int, Dict[str, Optional[List[Tuple[int, int]]]]] = {}
    other_coordination: Dict[int, Set[int]] = {}
    for other in open_prs:
        other_number = int(other["number"])
        if other_number == number:
            continue
        shared = set(pr_files(repository, other_number, token)) & set(files)
        if shared:
            other_ranges[other_number] = pr_file_ranges(repository, other_number, token)
            other_claims = parse_claims(str(other.get("body") or ""))
            other_coordination[other_number] = split_coordination(
                other_claims.get("coordinated", "")
            )
    advisories: List[str] = []
    errors = validate_pr(
        current,
        files=files,
        commit_subjects=subjects,
        ranges=ranges,
        other_ranges=other_ranges,
        other_coordination=other_coordination,
        advisories=advisories,
        added_lines=pr_added_lines(repository, number, token),
    )
    if not current["isDraft"]:
        base_sha = str(current.get("base", {}).get("sha") or "")
        head_sha = str(current.get("head", {}).get("sha") or "")
        if base_sha and head_sha:
            comparison, unmeasured = comparison_or_reason(
                lambda: github_json(
                    f"/repos/{repository}/compare/{base_sha}...{head_sha}", token
                )
            )
            if comparison is None:
                advisories.append(
                    UNMEASURED_BASE_ADVISORY.format(reason=unmeasured))
            else:
                verdict = base_staleness(
                    str(comparison.get("status") or ""),
                    int(comparison.get("behind_by") or 0),
                )
                if verdict is not None:
                    kind, message = verdict
                    (errors if kind == "error" else advisories).append(message)
    branch_match = BRANCH_RE.fullmatch(str(current["headRefName"]))
    registry = machine_registry()
    if branch_match and registry is not None:
        machine = branch_match.group("machine")
        if machine not in registry:
            advisories.append(
                f"machine ID '{machine}' is not in docs/MACHINES.md. If this "
                "is a new computer, add it to the canonical table; if it is "
                "an existing one under a new name, add the name as an alias. "
                "One physical machine, one ID — that is what makes ownership "
                "traceable across the fleet."
            )
    for advisory in advisories:
        print(f"::notice::{advisory}")
    if errors:
        for error in errors:
            print(f"::error::{error}")
        print(f"collaboration policy: {len(errors)} violation(s)")
        return 1
    print(
        f"collaboration policy: PR #{number} owns {len(files)} changed path(s) "
        "on a valid single-writer branch"
    )
    return 0


def gh_json(args: Sequence[str], *, cwd: Optional[Path] = None) -> Any:
    result = run(("gh", *args), cwd=cwd)
    return json.loads(result.stdout or "null")


def gh_pr_file_ranges(
    number: int, *, cwd: Optional[Path] = None
) -> Dict[str, Optional[List[Tuple[int, int]]]]:
    """``pr_file_ranges`` over the GitHub CLI, for commands that run locally."""
    rows = gh_json(
        (
            "api",
            "--paginate",
            f"/repos/{REPOSITORY}/pulls/{number}/files?per_page=100",
        ),
        cwd=cwd,
    )
    ranges: Dict[str, Optional[List[Tuple[int, int]]]] = {}
    for row in rows or []:
        patch = row.get("patch")
        ranges[str(row["filename"])] = patch_base_ranges(str(patch)) if patch else None
    return ranges


def required_check_state(
    rows: Sequence[Dict[str, Any]],
    *,
    required: Iterable[str] = REQUIRED_CHECKS,
    minimum_started: Optional[Dict[str, str]] = None,
) -> Tuple[str, List[str]]:
    """Summarize the newest check run for every required workflow.

    GitHub can retain several runs with the same name on one PR head after a
    body edit or draft/ready transition. Only the newest eligible run is the
    current gate. ``minimum_started`` lets ``ship`` wait for the policy run
    caused by its own ready-for-review transition instead of accepting an
    older green draft check in the few seconds before the new run appears.
    """
    missing_or_pending: List[str] = []
    failed: List[str] = []
    retryable: List[str] = []
    thresholds = minimum_started or {}
    for name in required:
        candidates = [
            row
            for row in rows
            if str(row.get("name") or row.get("context") or "") == name
            and str(row.get("startedAt") or row.get("started_at") or "")
            >= thresholds.get(name, "")
        ]
        if not candidates:
            missing_or_pending.append(name)
            continue
        latest = max(
            candidates,
            key=lambda row: str(
                row.get("startedAt")
                or row.get("started_at")
                or row.get("completedAt")
                or row.get("completed_at")
                or ""
            ),
        )
        status = str(latest.get("status") or "").lower()
        conclusion = str(
            latest.get("conclusion") or latest.get("state") or ""
        ).lower()
        if status and status != "completed":
            missing_or_pending.append(name)
        elif conclusion in RETRYABLE_CHECK_CONCLUSIONS:
            retryable.append(name)
        elif conclusion not in {"success", "successful"}:
            failed.append(name)
    # A real failure outranks a cancellation: report the verdict the tree
    # earned rather than the infrastructure noise beside it. A cancellation
    # outranks a pending sibling so the re-run is dispatched now instead of
    # after everything else finishes.
    if failed:
        return "failed", failed
    if retryable:
        return "retryable", retryable
    if missing_or_pending:
        return "pending", missing_or_pending
    return "success", []


def check_run_id(row: Dict[str, Any]) -> Optional[str]:
    """The Actions run id behind a check rollup row, if it has one.

    The rollup carries the job URL, not the run id, and the run is what can be
    re-dispatched: `.../actions/runs/<run>/job/<job>`. Status contexts from
    outside Actions have no such URL and return ``None``.
    """
    match = re.search(
        r"/actions/runs/(\d+)", str(row.get("detailsUrl") or row.get("details_url") or "")
    )
    return match.group(1) if match else None


def rerun_required_check(
    rows: Sequence[Dict[str, Any]], name: str, *, required: Iterable[str] = REQUIRED_CHECKS
) -> bool:
    """Re-dispatch the newest run behind required check ``name``.

    Returns whether a re-run was actually requested, so the caller can report
    an un-retryable check rather than silently looping on it.
    """
    if name not in tuple(required):
        return False
    candidates = [
        row
        for row in rows
        if str(row.get("name") or row.get("context") or "") == name and check_run_id(row)
    ]
    if not candidates:
        return False
    latest = max(
        candidates,
        key=lambda row: str(row.get("startedAt") or row.get("started_at") or ""),
    )
    run_id = check_run_id(latest)
    return (
        gh_api_write(
            "POST",
            f"repos/{REPOSITORY}/actions/runs/{run_id}/rerun",
            {},
            check=False,
        )
        is not None
    )


def ship_pr_errors(pr: Dict[str, Any], branch: str) -> List[str]:
    """Return reasons a task PR is not yet an honest finished feature."""
    errors: List[str] = []
    if str(pr.get("state") or "").upper() != "OPEN":
        errors.append("the current branch PR is not open")
    if str(pr.get("headRefName") or "") != branch:
        errors.append("the current branch does not own the discovered PR")
    body = str(pr.get("body") or "")
    if re.search(r"^\s*- \[ \]", body, re.MULTILINE):
        errors.append("complete every PR validation checkbox before shipping")
    summary = re.search(
        r"^## What changed\s*(.*?)(?=^## |\Z)",
        body,
        re.MULTILINE | re.DOTALL,
    )
    summary_text = summary.group(1).strip() if summary else ""
    if not summary_text or "implementation is in progress" in summary_text.lower():
        errors.append("replace the draft 'What changed' text with the finished summary")
    return errors


def ref_contains(repo: Path, ancestor: str, descendant: str = "HEAD") -> bool:
    return run(
        ("git", "-C", str(repo), "merge-base", "--is-ancestor", ancestor, descendant),
        check=False,
    ).returncode == 0


def current_pr(repo: Path) -> Dict[str, Any]:
    branch = git(repo, "symbolic-ref", "--quiet", "--short", "HEAD")
    return dict(
        gh_json(
            (
                "pr",
                "view",
                branch,
                "--repo",
                REPOSITORY,
                "--json",
                "number,url,state,isDraft,body,headRefName,headRefOid,baseRefOid,"
                "mergeCommit,mergeStateStatus,mergedAt,statusCheckRollup,title",
            ),
            cwd=repo,
        )
    )


def pr_merge_sha(pr: Dict[str, Any]) -> str:
    """The squash commit of a PR GitHub has already auto-merged, if any."""
    if str(pr.get("state") or "").upper() != "MERGED":
        return ""
    commit = pr.get("mergeCommit") or {}
    if isinstance(commit, dict):
        return str(commit.get("oid") or "")
    return str(commit)


def wait_for_pr_head(
    repo: Path,
    branch: str,
    local_head: str,
    *,
    deadline: float,
    poll_seconds: float,
) -> Dict[str, Any]:
    """Wait through GitHub's brief branch-ref to PR-view consistency gap."""
    while True:
        pr = current_pr(repo)
        if str(pr.get("headRefOid") or "") == local_head:
            return pr
        # Armed auto-merge can close the PR between the push and this read.
        # A closed PR's head is immutable and its branch may already be
        # deleted, so waiting for it to observe a later local ref can only
        # time out after a successful shipment.
        if pr_merge_sha(pr):
            return pr
        remote_head = remote_heads(repo).get(branch, "")
        if remote_head != local_head:
            raise CommandError(
                "the PR head changed outside this task's one-writer worktree"
            )
        if time.monotonic() >= deadline:
            raise CommandError("timed out waiting for GitHub to observe the pushed PR head")
        time.sleep(max(0.1, poll_seconds))


def regenerable_conflicts(repo: Path) -> List[str]:
    """The conflicted paths this tool is allowed to resolve by regenerating.

    Empty when nothing is conflicted, and empty when *anything* conflicted is
    not on the list — a partial automatic resolution is worse than none,
    because it hands the author a half-merged tree that looks resolved.
    """
    unmerged = [
        line.strip()
        for line in git(repo, "diff", "--name-only", "--diff-filter=U").splitlines()
        if line.strip()
    ]
    if not unmerged or any(name not in REGENERATED_ON_MERGE for name in unmerged):
        return []
    return sorted(unmerged)


def resolve_by_regenerating(repo: Path, conflicted: List[str]) -> None:
    """Rebuild the generated artifacts from the merged sources and stage them.

    ★★★★★ THE ONLY HOT CONFLICT CLASS THAT NEEDS NO JUDGEMENT. On 2026-08-19,
    35 of the 138 commits main took in a day touched `docs/eval_manifest.json`
    and 32 touched `docs/EVAL_STATUS.md` — the fourth and fifth most-edited
    paths in the repository, behind only `advanced.rs`, its tests and
    `elo.rs`. Both are pure functions of tracked source, both are appended to
    by every agent registering an evaluator arm, and every conflict in them
    has exactly one correct resolution: run the generator again. One pull
    request hit the same conflict on four consecutive ship attempts and
    resolved it four identical times by hand.

    ⚠ This is sound *because the source merged cleanly*. `regenerable_conflicts`
    returns nothing unless every conflicted path is generated, so the tree this
    regenerates from is the real merge of both branches' sources. Regenerating
    over a conflicted source file would silently publish one side's arms.

    ⚠ And it regenerates rather than choosing a side. `--write` is run, then
    `--check`, so a resolution that does not match what the merged source
    implies fails loudly instead of being committed.
    """
    commands = []
    for name in conflicted:
        command = REGENERATED_ON_MERGE[name]
        if command not in commands:
            commands.append(command)
    for command in commands:
        run((sys.executable, str(repo / command[0]), *command[1:]), cwd=repo, capture=False)
        # The generator reporting success is not the same as every artifact it
        # owns being current; ask it.
        run(
            (sys.executable, str(repo / command[0]), "--check"),
            cwd=repo,
            capture=False,
        )
    git(repo, "add", "--", *conflicted)
    still = [line for line in git(repo, "diff", "--name-only", "--diff-filter=U").splitlines() if line.strip()]
    if still:
        raise CommandError(
            "regenerating the generated documents left conflicts behind: " + ", ".join(still)
        )


def settle_merge_conflict(repo: Path, detail: str) -> None:
    """Finish a conflicted merge, or hand it back to the author.

    A conflict confined to generated documents is arithmetic, not a decision:
    rebuild them from the merged source and commit. Anything else raises, with
    git's own message, exactly as it did before. See `resolve_by_regenerating`.
    """
    conflicted = regenerable_conflicts(repo)
    if not conflicted:
        raise CommandError(
            "latest main did not merge cleanly; resolve this task worktree, "
            f"revalidate it, and run ship again: {detail or 'merge conflict'}"
        )
    print(
        "main advanced into a generated-document conflict; regenerating "
        + ", ".join(conflicted)
    )
    resolve_by_regenerating(repo, conflicted)
    git(repo, "commit", "--no-edit")


def merge_current_main(repo: Path) -> bool:
    """Integrate a newly advanced main and type-check the merged result.

    This used to rerun the entire `cargo test --profile ci --locked` suite —
    several local minutes per ship whenever the trunk had moved, duplicating
    the full CI gate that runs on the pushed result minutes later anyway. The
    author already validated the branch before shipping (AGENTS.md), and the
    exact merged tree is what CI tests and auto-merge gates on. What a clean
    auto-merge can still break locally is names and types drifting under the
    diff, and `cargo check` catches that whole class in a fraction of the
    time. A conflicted merge never reaches this path — it raises above, and
    resolving plus revalidating it is on the author.
    """
    fetch_main(repo)
    if ref_contains(repo, "origin/main"):
        return False
    print("main advanced; merging it and type-checking the result")
    merged = run(
        ("git", "-C", str(repo), "merge", "--no-edit", "origin/main"),
        check=False,
    )
    if merged.returncode:
        settle_merge_conflict(repo, (merged.stderr or merged.stdout or "").strip())
    git(repo, "diff", "--check", "origin/main...")
    run(
        ("cargo", "check", "--locked"),
        cwd=repo,
        capture=False,
    )
    return True


def local_deploy_root(repo: Path) -> Optional[Path]:
    common = common_git_dir(repo)
    root = common.parent
    return root if (root / "target" / "spectator" / "build.json").is_file() else None


def live_status_commit(url: str, timeout: float = 5.0) -> str:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as response:
            payload = json.load(response)
    except (OSError, ValueError, urllib.error.URLError):
        return ""
    return str(payload.get("commit") or "") if isinstance(payload, dict) else ""


def live_status_answers(url: str, timeout: float = 5.0) -> bool:
    """Whether anything is LISTENING at ``url`` — a different question from what
    revision it reports.

    ``live_status_commit`` returns ``""`` for a refused connection and for a
    server that answered without a commit, and the wait loop could not tell
    those apart. They are opposite situations: a server that is up and stale is
    still building and waiting is exactly right, while a port with nothing
    behind it will never answer however long anyone waits.

    An HTTP error status still counts as answering — something is there.
    ``HTTPError`` is a subclass of both ``URLError`` and ``OSError``, so it has
    to be caught first or a live server returning 503 reads as absent.
    """
    try:
        with urllib.request.urlopen(url, timeout=timeout):
            return True
    except urllib.error.HTTPError:
        return True
    except (OSError, urllib.error.URLError):
        return False


def deployed_commit_covers(repo: Path, deployed: str, merged_sha: str) -> bool:
    if not deployed:
        return False
    if merged_sha.startswith(deployed) or deployed.startswith(merged_sha):
        return True
    resolved = git(repo, "rev-parse", "--verify", deployed, check=False)
    return bool(resolved) and ref_contains(repo, merged_sha, resolved)


def wait_for_local_live_build(
    repo: Path,
    merged_sha: str,
    *,
    url: str,
    timeout_seconds: float,
    poll_seconds: float,
) -> bool:
    if local_deploy_root(repo) is None or timeout_seconds <= 0:
        print("no local production spectator detected; merge is complete")
        return False
    # ⚠⚠ THE GUARD ABOVE READS A DIRECTORY, NOT A SERVER. `local_deploy_root`
    # only says this clone is a production host, so on a host where the
    # exhibition is deliberately stopped -- all CIVVIS automation was disabled
    # at operator request on 2026-07-31 and the keeper LaunchAgent is not loaded
    # -- every merge polled a port with nothing behind it for the full
    # `--live-timeout-seconds`, ten minutes by default, and then printed a
    # warning about a service that was switched off on purpose. A warning that
    # fires on every merge is one nobody reads.
    #
    # Nothing listening and answering-but-stale are opposite situations: the
    # second is a spectator still building, which is what the timeout is FOR.
    # So probe reachability first, and only there give up early.
    #
    # ⚠ Not on a single probe. `ship` runs at exactly the moment a spectator may
    # be restarting onto the new revision, so a refused connection in that
    # instant is expected. Give it a short grace to appear before concluding it
    # is not running at all -- ten minutes becomes half a minute, not zero.
    grace = min(LIVE_PRESENCE_GRACE_S, timeout_seconds)
    present_by = time.monotonic() + grace
    while not live_status_answers(url):
        if time.monotonic() >= present_by:
            print(
                f"nothing is listening at {url} after {grace:.0f}s; the "
                "production spectator is not running, so there is no live "
                "revision to confirm. Merge is complete."
            )
            return False
        time.sleep(max(0.1, min(poll_seconds, 2.0)))
    print(f"waiting for the production spectator at {url} to run {merged_sha[:7]}")
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        deployed = live_status_commit(url)
        if deployed_commit_covers(repo, deployed, merged_sha):
            print(f"production spectator is live on {deployed}")
            return True
        time.sleep(max(0.1, poll_seconds))
        fetch_main(repo)
    print(
        "production spectator did not confirm the merged revision before the "
        "live-build timeout"
    )
    return False


def finish_ship(
    repo: Path,
    *,
    number: int,
    branch: str,
    merged_sha: str,
    live_url: str,
    live_timeout_seconds: float,
    poll_seconds: float,
) -> int:
    """Clean up and verify a PR merged manually or by armed auto-merge."""
    if not merged_sha:
        raise CommandError(f"merged PR #{number} did not report its merge commit")
    print(f"PR #{number} squash-merged as {merged_sha[:7]}")
    deletion = run(
        ("git", "-C", str(repo), "push", "origin", "--delete", branch),
        check=False,
    )
    if deletion.returncode:
        print("remote branch was already deleted or could not be deleted")
    fetch_main(repo)
    wait_for_local_live_build(
        repo,
        merged_sha,
        url=live_url,
        timeout_seconds=live_timeout_seconds,
        poll_seconds=poll_seconds,
    )
    return 0


def merge_pr_or_observe_auto_merge(
    repo: Path, *, number: int, local_head: str
) -> Optional[str]:
    """Squash-merge a green PR, accepting auto-merge that wins the same race.

    Auto-merge is deliberately armed before the required checks finish.  Once
    they do, GitHub can land the PR between our successful-check poll and this
    explicit merge request.  A 405 in that interval is either a successful
    shipment or a brief propagation gap; re-read the PR, then let the caller
    poll again rather than reporting a false failure.
    """
    merge_result = gh_api_write(
        "PUT",
        f"repos/{REPOSITORY}/pulls/{number}/merge",
        {"merge_method": "squash", "sha": local_head},
        check=False,
    )
    if isinstance(merge_result, dict) and merge_result.get("merged"):
        merged_sha = str(merge_result.get("sha") or "")
        if merged_sha:
            return merged_sha

    # The API's non-success response is ambiguous: an actual rejection and an
    # auto-merge that completed a moment earlier both return here.  The PR is
    # authoritative for which happened.
    merged_sha = pr_merge_sha(current_pr(repo))
    if merged_sha:
        return merged_sha

    if merge_result is None:
        return None

    message = (
        str(merge_result.get("message") or "unknown reason")
        if isinstance(merge_result, dict)
        else "the merge request did not complete"
    )
    raise CommandError("GitHub refused the green squash merge: " + message)


def ship_task(args: argparse.Namespace) -> int:
    """Push a finished task, wait for green CI, squash-merge, and verify live."""
    root = repo_root()
    if not shutil.which("gh"):
        raise CommandError("GitHub CLI 'gh' is required to ship a task")
    run(("gh", "auth", "status"), cwd=root)
    branch = git(root, "symbolic-ref", "--quiet", "--short", "HEAD", check=False)
    if not BRANCH_RE.fullmatch(branch):
        raise CommandError("ship must run from this task's conforming agent branch")
    if git(root, "status", "--porcelain"):
        raise CommandError("commit the finished feature and leave the worktree clean first")

    install_push_guard(root)
    fetch_main(root)
    if run(
        ("git", "-C", str(root), "diff", "--quiet", "origin/main...HEAD"),
        check=False,
    ).returncode == 0:
        raise CommandError("the task has no file changes relative to main")

    pr = current_pr(root)
    errors = ship_pr_errors(pr, branch)
    if errors:
        raise CommandError("; ".join(errors))

    deadline = time.monotonic() + max(1.0, args.timeout_seconds)
    ready_thresholds: Dict[str, str] = {}
    auto_merge_armed = False
    rerun_attempts: Dict[str, int] = {name: 0 for name in REQUIRED_CHECKS}

    def finish_merged(pr: Dict[str, Any]) -> Optional[int]:
        merged_sha = pr_merge_sha(pr)
        if not merged_sha:
            return None
        return finish_ship(
            root,
            number=int(pr["number"]),
            branch=branch,
            merged_sha=merged_sha,
            live_url=args.live_url,
            live_timeout_seconds=args.live_timeout_seconds,
            poll_seconds=args.poll_seconds,
        )

    while True:
        if time.monotonic() >= deadline:
            raise CommandError("timed out waiting for the task to reach main")

        # Auto-merge may have fired after the preceding poll. Detect that
        # before comparing commit ancestry: a squash commit deliberately does
        # not contain the task branch's commits, so ancestry cannot identify
        # this successful terminal state.
        pr = current_pr(root)
        if (finished := finish_merged(pr)) is not None:
            return finished

        merged_main = merge_current_main(root)
        if merged_main and git(root, "status", "--porcelain"):
            raise CommandError("main integration left unexpected worktree changes")
        git(root, "diff", "--check", "origin/main...")
        git(root, "push", "origin", f"HEAD:{branch}")
        local_head = git(root, "rev-parse", "HEAD")

        pr = wait_for_pr_head(
            root,
            branch,
            local_head,
            deadline=deadline,
            poll_seconds=args.poll_seconds,
        )
        if (finished := finish_merged(pr)) is not None:
            return finished
        errors = ship_pr_errors(pr, branch)
        if errors:
            raise CommandError("; ".join(errors))
        if pr.get("isDraft"):
            # Permit a small clock skew while still excluding earlier draft
            # policy runs from the ready-for-review gate.
            threshold = dt.datetime.now(dt.timezone.utc) - dt.timedelta(seconds=5)
            ready_thresholds["collaboration-policy"] = threshold.isoformat().replace(
                "+00:00", "Z"
            )
            run(
                ("gh", "pr", "ready", str(pr["number"]), "--repo", REPOSITORY),
                cwd=root,
            )
            print(f"PR #{pr['number']} is ready; waiting for required checks")

        while True:
            if time.monotonic() >= deadline:
                # ⚠ A timeout is not a verdict, and the agent that reads it has
                # to know which of two very different situations it is in. With
                # auto-merge armed and every required check still running, the
                # PR finishes without us and waiting was the whole plan. With a
                # check that has stopped without a verdict, nothing is coming —
                # so sweep once more on the way out rather than leaving a PR
                # that no one will look at again.
                pr = current_pr(root)
                if (finished := finish_merged(pr)) is not None:
                    return finished
                rollup = pr.get("statusCheckRollup") or []
                state, names = required_check_state(
                    rollup, minimum_started=ready_thresholds
                )
                if state == "retryable":
                    for name in names:
                        rerun_required_check(rollup, name)
                    raise CommandError(
                        "timed out, and "
                        + ", ".join(names)
                        + " had stopped without a verdict; re-ran them, so watch "
                        f"PR #{pr['number']} rather than assuming it merges"
                    )
                if state == "failed":
                    raise CommandError("required checks failed: " + ", ".join(names))
                raise CommandError(
                    "timed out waiting for required checks ("
                    + (", ".join(names) or "none outstanding")
                    + ")"
                    + (
                        "; auto-merge is armed and merges this PR when they pass"
                        if auto_merge_armed
                        else "; auto-merge is NOT armed, so this PR needs a look"
                    )
                )
            pr = current_pr(root)
            if (finished := finish_merged(pr)) is not None:
                return finished
            if str(pr.get("state") or "").upper() != "OPEN":
                raise CommandError("the PR closed before ship completed")
            if str(pr.get("headRefOid") or "") != local_head:
                raise CommandError(
                    "the PR head changed outside this task's one-writer worktree"
                )

            if not auto_merge_armed:
                # Armed auto-merge fires the instant the required checks go
                # green, instead of only when this poll happens to observe it.
                # With `strict: false` it fires even when `main` has advanced
                # past this head — deliberate: waiting for a run that finishes
                # inside a no-new-merges window is what kept busy PRs from
                # ever converging.
                armed = run(
                    (
                        "gh", "pr", "merge", str(pr["number"]),
                        "--repo", REPOSITORY, "--squash", "--auto",
                        "--delete-branch",
                    ),
                    cwd=root,
                    check=False,
                )
                if armed.returncode == 0:
                    auto_merge_armed = True
                    print(f"auto-merge armed on PR #{pr['number']}")

            fetch_main(root)
            # Auto-merge can fire during the fetch itself. Re-read the PR
            # before trying to update a branch GitHub may just have deleted.
            pr = current_pr(root)
            if (finished := finish_merged(pr)) is not None:
                return finished
            if str(pr.get("state") or "").upper() != "OPEN":
                raise CommandError("the PR closed before ship completed")
            if str(pr.get("headRefOid") or "") != local_head:
                raise CommandError("the PR head changed outside this task's one-writer worktree")
            if not ref_contains(root, "origin/main"):
                # `main` advancing while the gate runs is the NORMAL state of a
                # busy trunk, not a problem this loop must fix. Refreshing the
                # head here — server-side or locally — pushes a new commit,
                # which trips `cancel-in-progress` and restarts the ~7-minute
                # gate from zero; while `main` takes a merge every few minutes
                # the PR can then never converge. On 2026-08-06, 58 of 133
                # cargo-test runs (44%) were cancelled exactly this way.
                # Doing nothing is safe by design: branch protection runs with
                # `strict: false` (see `base_staleness`), armed auto-merge
                # lands a green run even when the head trails the trunk, and
                # the push to `main` reruns the full gate on the actual squash
                # result. Only two cases still need a fresh head:
                merge_state = str(pr.get("mergeStateStatus") or "").upper()
                if merge_state == "DIRTY":
                    # A textual conflict. GitHub can neither auto-merge nor
                    # update-branch this; only a real merge in the worktree
                    # resolves it, which is what the outer loop does.
                    print("main advanced into a conflict; merging locally")
                    ready_thresholds.clear()
                    break
                behind = int(
                    git(root, "rev-list", "--count", "HEAD..origin/main") or "0"
                )
                if behind >= STALE_BASE_LIMIT:
                    # Past the same limit that makes `base_staleness` a
                    # refusal: a merge from here would be a tree no CI run has
                    # even approximated. Refresh through GitHub so the reset
                    # gate runs on the exact merge result.
                    print(
                        f"branch is {behind} commits behind main; "
                        "updating the PR branch through GitHub"
                    )
                    updated = gh_api_write(
                        "PUT",
                        f"repos/{REPOSITORY}/pulls/{pr['number']}/update-branch",
                        {},
                        check=False,
                    )
                    if updated is None:
                        print("GitHub could not update the branch; merging locally")
                        ready_thresholds.clear()
                        break
                    git(root, "fetch", "origin", branch)
                    git(root, "merge", "--ff-only", f"origin/{branch}")
                    local_head = git(root, "rev-parse", "HEAD")
                    ready_thresholds.clear()
                    time.sleep(args.poll_seconds)
                    continue

            rollup = pr.get("statusCheckRollup") or []
            state, names = required_check_state(
                rollup, minimum_started=ready_thresholds
            )
            if state == "failed":
                raise CommandError("required checks failed: " + ", ".join(names))
            if state == "retryable":
                # Nothing else will start these. Auto-merge is armed and waiting
                # for a green required check that can no longer arrive, so the
                # PR is stuck until someone re-dispatches the run.
                stuck: List[str] = []
                for name in names:
                    if rerun_attempts[name] >= CHECK_RERUN_LIMIT:
                        stuck.append(f"{name} (re-run {rerun_attempts[name]}x already)")
                    elif rerun_required_check(rollup, name):
                        rerun_attempts[name] += 1
                        print(
                            f"required check {name} ended without a verdict and "
                            f"nothing re-starts it; re-running "
                            f"({rerun_attempts[name]}/{CHECK_RERUN_LIMIT})"
                        )
                    else:
                        stuck.append(f"{name} (no Actions run to re-dispatch)")
                if stuck:
                    raise CommandError(
                        "required checks cannot reach a verdict: "
                        + ", ".join(stuck)
                        + " — re-running did not help, so this needs a look"
                    )
                time.sleep(max(0.1, args.poll_seconds))
                continue
            if state == "success":
                merged_sha = merge_pr_or_observe_auto_merge(
                    root, number=int(pr["number"]), local_head=local_head
                )
                if not merged_sha:
                    print("GitHub is still publishing the auto-merge result; rechecking")
                    time.sleep(max(0.1, args.poll_seconds))
                    continue
                return finish_ship(
                    root,
                    number=int(pr["number"]),
                    branch=branch,
                    merged_sha=merged_sha,
                    live_url=args.live_url,
                    live_timeout_seconds=args.live_timeout_seconds,
                    poll_seconds=args.poll_seconds,
                )

            print("waiting on: " + ", ".join(names))
            time.sleep(max(0.1, args.poll_seconds))


def existing_pr_claims(repo: Path) -> List[Dict[str, Any]]:
    rows = gh_json(
        (
            "pr",
            "list",
            "--repo",
            REPOSITORY,
            "--state",
            "open",
            "--limit",
            "100",
            "--json",
            "number,body,headRefName,title",
        ),
        cwd=repo,
    )
    return list(rows)


def parse_remote_heads(raw: str) -> Dict[str, str]:
    heads: Dict[str, str] = {}
    prefix = "refs/heads/"
    for line in raw.splitlines():
        sha, separator, ref = line.partition("\t")
        if separator and ref.startswith(prefix):
            heads[ref[len(prefix) :]] = sha
    return heads


def remote_heads(repo: Path) -> Dict[str, str]:
    return parse_remote_heads(git(repo, "ls-remote", "--heads", "origin"))


def commit_is_pr_backed(rows: Sequence[Dict[str, Any]], sha: str) -> Optional[int]:
    for row in rows:
        if row.get("merged_at") and row.get("merge_commit_sha") == sha:
            return int(row["number"])
    return None


def associated_pr_number(sha: str) -> Optional[int]:
    rows = gh_json(("api", f"repos/{REPOSITORY}/commits/{sha}/pulls"))
    return commit_is_pr_backed(rows or [], sha)


def required_check_gate_errors(
    check_runs: Sequence[Dict[str, Any]],
    merged_at: str,
    required: Iterable[str] = REQUIRED_CHECKS,
) -> List[str]:
    """Report required checks that were not successful before a PR merged."""
    merge_time = dt.datetime.fromisoformat(merged_at.replace("Z", "+00:00"))
    errors: List[str] = []
    for name in required:
        eligible: List[Dict[str, Any]] = []
        for row in check_runs:
            if row.get("name") != name:
                continue
            started_at = str(row.get("started_at") or "")
            if not started_at:
                continue
            started = dt.datetime.fromisoformat(started_at.replace("Z", "+00:00"))
            if started <= merge_time:
                eligible.append(row)
        if not eligible:
            errors.append(f"required check {name} had not started before merge")
            continue
        latest = max(eligible, key=lambda row: str(row.get("started_at") or ""))
        completed_at = str(latest.get("completed_at") or "")
        completed = (
            dt.datetime.fromisoformat(completed_at.replace("Z", "+00:00"))
            if completed_at
            else None
        )
        if latest.get("conclusion") != "success" or not completed or completed > merge_time:
            errors.append(f"required check {name} was not green before merge")
    return errors


def merged_pr_gate_errors(number: int, base_sha: str = "") -> List[str]:
    view = gh_json(
        (
            "pr",
            "view",
            str(number),
            "--repo",
            REPOSITORY,
            "--json",
            "headRefOid,mergedAt",
        )
    )
    head_sha = str(view.get("headRefOid") or "")
    merged_at = str(view.get("mergedAt") or "")
    if not head_sha or not merged_at:
        return ["merged PR metadata is incomplete"]
    errors: List[str] = []
    if base_sha:
        comparison, unmeasured = comparison_or_reason(
            lambda: gh_json(
                (
                    "api",
                    f"repos/{REPOSITORY}/compare/{base_sha}...{head_sha}",
                )
            )
        )
        if comparison is None:
            # A post-merge audit finding, not a gate, and phrased as what is
            # actually known: the distance is UNKNOWN, not over the limit. See
            # `comparison_or_reason`.
            errors.append(
                f"could not measure how far PR #{number} trailed main at "
                f"merge ({unmeasured}); staleness unverified")
        else:
            # Merging a few commits behind main is the designed outcome, not a
            # violation: `strict: false` admits it, `ship` deliberately stops
            # refreshing the head while the gate runs (every refresh cancelled
            # the in-flight run — 44% of all runs on 2026-08-06), and the push
            # to `main` reruns the full gate on the actual squash result. The
            # hard line is the same one `base_staleness` draws pre-merge: past
            # STALE_BASE_LIMIT the merged tree is a combination no CI run has
            # even approximated.
            behind_by = int(comparison.get("behind_by") or 0)
            if (
                not compare_status_is_current(str(comparison.get("status") or ""))
                and behind_by >= STALE_BASE_LIMIT
            ):
                errors.append(
                    f"PR head was {behind_by} commits behind main at merge, past "
                    f"the {STALE_BASE_LIMIT}-commit staleness limit"
                )
    payload = gh_json(
        (
            "api",
            f"repos/{REPOSITORY}/commits/{head_sha}/check-runs?per_page=100",
        )
    )
    errors.extend(
        required_check_gate_errors(payload.get("check_runs") or [], merged_at)
    )
    return errors


def format_claim_body(
    *,
    machine: str,
    agent: str,
    task: str,
    paths: Sequence[str],
    coordinated: Sequence[int],
) -> str:
    coordination = ", ".join(f"#{number}" for number in coordinated) or "none"
    claimed = ", ".join(f"`{path}`" for path in paths)
    return f"""## Ownership claim

- Machine ID: `{machine}`
- Agent/session ID: `{agent}`
- Task: {task.replace('-', ' ')}
- Claimed paths: {claimed}
- Coordinated with: {coordination}
- Related issue/request: operator request

## What changed

Draft claim; implementation is in progress.

## Validation

- [ ] Branch started from current `origin/main`
- [ ] Ownership/overlap is coordinated above
- [ ] Latest `origin/main` merged before ready
- [ ] `git diff --check origin/main...`
- [ ] `cargo test --profile ci --locked`
- [ ] Relevant focused tests
- [ ] Soak run for engine changes, or reason it is not applicable
- [ ] No unrelated formatting, generated output, or runtime artifacts

## Notes for integration

Squash merge only. Delete the branch after merge.
"""


def validate_identifier(label: str, value: str, pattern: re.Pattern[str]) -> None:
    if not pattern.fullmatch(value):
        raise CommandError(
            f"{label} '{value}' must be lowercase letters, numbers, and hyphens "
            f"and fit the fleet naming limit"
        )


def fetch_main(repo: Path) -> None:
    last_error: Optional[Exception] = None
    for delay in (0, 1, 2):
        if delay:
            time.sleep(delay)
        try:
            git(repo, "fetch", "--prune", "origin", DEFAULT_BRANCH)
            return
        except CommandError as exc:
            last_error = exc
    assert last_error is not None
    raise last_error


def common_git_dir(repo: Path) -> Path:
    raw = Path(git(repo, "rev-parse", "--git-common-dir"))
    return raw.resolve() if raw.is_absolute() else (repo / raw).resolve()


def push_guard_paths(repo: Path) -> Tuple[Path, Path]:
    source = repo / "tools" / "civvis_push_guard.py"
    target = common_git_dir(repo) / "hooks" / "pre-push"
    return source, target


def install_push_guard(repo: Path) -> Path:
    source, target = push_guard_paths(repo)
    if not source.is_file():
        raise CommandError(f"versioned push guard is missing: {source}")
    source_bytes = source.read_bytes()
    if target.is_symlink():
        raise CommandError(f"refusing to replace symlinked pre-push hook: {target}")
    if target.exists():
        existing = target.read_bytes()
        if existing != source_bytes and PUSH_GUARD_MARKER.encode() not in existing:
            raise CommandError(
                f"refusing to overwrite unmanaged pre-push hook: {target}; "
                "preserve and resolve that hook explicitly before retrying"
            )
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary = target.with_name(
        f".{target.name}.civvis-{os.getpid()}-{secrets.token_hex(4)}"
    )
    try:
        with temporary.open("xb") as handle:
            handle.write(source_bytes)
            handle.flush()
            os.fsync(handle.fileno())
        if os.name != "nt":
            temporary.chmod(
                temporary.stat().st_mode
                | stat.S_IXUSR
                | stat.S_IXGRP
                | stat.S_IXOTH
            )
        os.replace(temporary, target)
    finally:
        temporary.unlink(missing_ok=True)
    return target


def push_guard_error(repo: Path) -> Optional[str]:
    source, target = push_guard_paths(repo)
    if not source.is_file():
        return f"versioned push guard is missing: {source}"
    if target.is_symlink():
        return f"local pre-push guard must not be a symlink: {target}"
    if not target.is_file():
        return (
            "local pre-push guard is not installed; run "
            "python3 tools/civvis_collab.py install-hooks"
        )
    if target.read_bytes() != source.read_bytes():
        return (
            "local pre-push guard is outdated or unmanaged; run "
            "python3 tools/civvis_collab.py install-hooks"
        )
    if os.name != "nt" and not os.access(target, os.X_OK):
        return f"local pre-push guard is not executable: {target}"
    return None


def install_hooks_command(args: argparse.Namespace) -> int:
    del args
    target = install_push_guard(repo_root())
    print(f"installed CIVVIS pre-push guard: {target}")
    return 0


def configure_clone(root: Path) -> None:
    for key, value in (
        ("fetch.prune", "true"),
        ("pull.ff", "only"),
        ("push.default", "simple"),
        ("merge.conflictStyle", "zdiff3"),
        ("rerere.enabled", "true"),
        ("rerere.autoupdate", "false"),
    ):
        git(root, "config", key, value)


def bootstrap_command(args: argparse.Namespace) -> int:
    del args
    root = repo_root()
    configure_clone(root)
    hook = install_push_guard(root)
    report = refresh_repository(root, wait_seconds=30.0)
    failure = (
        report.get("fetch_error")
        or report.get("main_update_error")
        or report.get("skipped")
    )
    if failure:
        raise CommandError(str(failure))
    services = install_managed_services(root)
    print(f"installed CIVVIS pre-push guard: {hook}")
    for name, absent, paths in services:
        for path in paths:
            print(f"installed CIVVIS {name}: {path}")
        if not paths and absent:
            print(f"{name}: {absent}")
    print_refresh_report(report)
    return 0


def start_task(args: argparse.Namespace) -> int:
    root = repo_root()
    if not shutil.which("gh"):
        raise CommandError("GitHub CLI 'gh' is required to publish the draft claim")
    run(("gh", "auth", "status"), cwd=root)

    configured_machine = git(root, "config", "--get", "civvis.machine", check=False)
    machine = args.machine or configured_machine
    agent = args.agent or os.environ.get("CIVVIS_AGENT_ID", "")
    if not machine:
        raise CommandError("pass --machine once or set: git config civvis.machine <stable-id>")
    if not agent:
        raise CommandError("pass --agent or set CIVVIS_AGENT_ID")
    validate_identifier("machine", machine, ID_RE)
    validate_identifier("agent", agent, ID_RE)
    validate_identifier("task", args.task, TASK_RE)

    paths = [path.replace("\\", "/") for path in args.path]
    if not paths or any(not valid_claim_pattern(path) for path in paths):
        raise CommandError("provide one or more safe repo-relative --path claims")
    coordinated = sorted(set(args.coordinate))

    conflicts: List[Tuple[int, List[str]]] = []
    for pr in existing_pr_claims(root):
        other = split_paths(parse_claims(str(pr.get("body") or "")).get("paths", ""))
        if other and claims_overlap(paths, other):
            conflicts.append((int(pr["number"]), other))
    undeclared = [(number, claim) for number, claim in conflicts if number not in coordinated]
    if undeclared:
        # Claim overlap is normal on hotspot files such as web/index.html and
        # src/game.rs. Record the neighbours automatically instead of refusing
        # to start; CI gates on real line-level collisions, not on shared files.
        for number, claim in undeclared:
            print(
                f"note: PR #{number} already claims {', '.join(claim)}; "
                "recording it under 'Coordinated with'"
            )
        coordinated = sorted({*coordinated, *(number for number, _ in undeclared)})

    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    nonce = secrets.token_hex(2)
    branch = f"agent/{machine}/{agent}/{args.task}-{stamp}-{nonce}"
    parent = Path(args.parent).expanduser().resolve() if args.parent else root.parent
    worktree = parent / f"civvis-{args.task}-{nonce}"
    if worktree.exists():
        raise CommandError(f"worktree path already exists: {worktree}")

    if args.dry_run:
        print(json.dumps({"branch": branch, "worktree": str(worktree), "paths": paths}, indent=2))
        return 0

    if args.machine and machine != configured_machine:
        git(root, "config", "civvis.machine", machine)
    configure_clone(root)
    install_push_guard(root)
    report = refresh_repository(root, wait_seconds=30.0)
    failure = (
        report.get("fetch_error")
        or report.get("main_update_error")
        or report.get("skipped")
    )
    if failure:
        raise CommandError(
            "cannot start from a synchronized origin/main: " + str(failure)
        )
    # Every managed service, not the two that existed when this line was
    # written: a host bootstrapped before a service was added never got it, and
    # the ladder keeper was missing from a Civilization VI seat for that reason.
    # Repairs are quiet — this runs on every task, and a launcher that reports
    # three green services on every start is a launcher nobody reads.
    install_managed_services(root)
    git(root, "worktree", "add", "-b", branch, str(worktree), "origin/main")
    git(worktree, "commit", "--allow-empty", "-m", f"claim: {args.task.replace('-', ' ')}")
    git(worktree, "push", "-u", "origin", branch)

    body = format_claim_body(
        machine=machine,
        agent=agent,
        task=args.task,
        paths=paths,
        coordinated=coordinated,
    )
    title = args.title or args.task.replace("-", " ").capitalize()
    result = run(
        (
            "gh",
            "pr",
            "create",
            "--repo",
            REPOSITORY,
            "--draft",
            "--base",
            DEFAULT_BRANCH,
            "--head",
            branch,
            "--title",
            title,
            "--body",
            body,
        ),
        cwd=worktree,
    )
    print(f"worktree: {worktree}")
    print(f"branch:   {branch}")
    print(f"draft PR: {result.stdout.strip()}")
    return 0


def parse_worktrees(raw: str) -> List[Dict[str, str]]:
    rows: List[Dict[str, str]] = []
    current: Dict[str, str] = {}
    for line in raw.splitlines() + [""]:
        if not line:
            if current:
                rows.append(current)
                current = {}
            continue
        key, _, value = line.partition(" ")
        current[key] = value
    return rows


def freshness_dir(repo: Path) -> Path:
    return common_git_dir(repo) / "civvis-freshness"


def freshness_key(repo: Path) -> str:
    value = str(common_git_dir(repo)).encode("utf-8")
    return hashlib.sha256(value).hexdigest()[:12]


def freshness_state_path(repo: Path) -> Path:
    return freshness_dir(repo) / "state.json"


def freshness_worker_path(repo: Path) -> Path:
    return freshness_dir(repo) / "civvis_collab.py"


def atomic_write(path: Path, data: bytes, *, executable: bool = False) -> None:
    if path.is_symlink():
        raise CommandError(f"refusing to replace symlink: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(
        f".{path.name}.civvis-{os.getpid()}-{secrets.token_hex(4)}"
    )
    try:
        with temporary.open("xb") as handle:
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        if executable and os.name != "nt":
            temporary.chmod(
                temporary.stat().st_mode
                | stat.S_IXUSR
                | stat.S_IXGRP
                | stat.S_IXOTH
            )
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


class FreshnessLock:
    """A tiny cross-platform exclusion lock for overlapping timer runs."""

    def __init__(self, repo: Path, *, wait_seconds: float = 0.0):
        self.path = freshness_dir(repo) / "refresh.lock"
        self.acquired = False
        self.deadline = time.monotonic() + max(0.0, wait_seconds)

    def __enter__(self) -> bool:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        while True:
            try:
                descriptor = os.open(
                    self.path,
                    os.O_CREAT | os.O_EXCL | os.O_WRONLY,
                    0o600,
                )
            except FileExistsError:
                try:
                    age = time.time() - self.path.stat().st_mtime
                except FileNotFoundError:
                    continue
                if age > FRESHNESS_LOCK_STALE_SECONDS:
                    self.path.unlink(missing_ok=True)
                    continue
                if time.monotonic() >= self.deadline:
                    return False
                time.sleep(0.1)
                continue
            with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
                json.dump({"pid": os.getpid(), "started_at": utc_now()}, handle)
            self.acquired = True
            return True

    def __exit__(self, exc_type: Any, exc: Any, traceback: Any) -> None:
        del exc_type, exc, traceback
        if self.acquired:
            self.path.unlink(missing_ok=True)


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def read_freshness_state(repo: Path) -> Optional[Dict[str, Any]]:
    try:
        value = json.loads(freshness_state_path(repo).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return value if isinstance(value, dict) else None


def write_freshness_state(repo: Path, state: Dict[str, Any]) -> None:
    encoded = (json.dumps(state, indent=2, sort_keys=True) + "\n").encode("utf-8")
    atomic_write(freshness_state_path(repo), encoded)


def main_worktree(repo: Path) -> Path:
    rows = parse_worktrees(git(repo, "worktree", "list", "--porcelain"))
    for row in rows:
        if row.get("branch") == f"refs/heads/{DEFAULT_BRANCH}":
            path = Path(row.get("worktree", "")).resolve()
            if path.is_dir():
                return path
    raise CommandError(
        "this clone needs a stable main management worktree before its "
        "freshness service can be installed"
    )


def force_update_main_worktree(repo: Path, origin_main: str) -> Dict[str, str]:
    """Align the clean management worktree to the exact fetched main revision.

    Task worktrees are never candidates for this update. A dirty management
    worktree is preserved verbatim and reported as an error. If a clean local
    main has commits that are not ancestors of GitHub main, keep an immutable
    recovery ref before resetting it so the scheduled repair loses no history.
    """
    path = main_worktree(repo)
    dirty = git(
        path,
        "status",
        "--porcelain=v1",
        "--untracked-files=all",
    )
    if dirty:
        raise CommandError(
            f"refusing to force-update dirty main management worktree: {path}"
        )

    before = git(path, "rev-parse", "HEAD")
    result = {
        "path": str(path),
        "before": before,
        "after": origin_main,
        "mode": "current",
    }
    if before == origin_main:
        return result

    ancestry = run(
        ("git", "-C", str(path), "merge-base", "--is-ancestor", before, origin_main),
        check=False,
    )
    if ancestry.returncode == 0:
        git(path, "merge", "--ff-only", origin_main)
        result["mode"] = "fast-forward"
    elif ancestry.returncode == 1:
        stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
        recovery_ref = (
            f"refs/civvis/recovery/main/{stamp}-{secrets.token_hex(2)}-{before[:12]}"
        )
        git(path, "update-ref", recovery_ref, before)
        git(path, "reset", "--hard", origin_main)
        result["mode"] = "forced"
        result["recovery_ref"] = recovery_ref
    else:
        raise CommandError(
            f"cannot determine whether local main {before[:12]} precedes "
            f"origin/main {origin_main[:12]}"
        )

    actual = git(path, "rev-parse", "HEAD")
    if actual != origin_main:
        raise CommandError(
            f"main update stopped at {actual[:12]}, expected {origin_main[:12]}"
        )
    if git(path, "status", "--porcelain=v1", "--untracked-files=all"):
        raise CommandError(f"main update left unexpected worktree changes: {path}")
    return result


def worktree_freshness(repo: Path, origin_main: str) -> List[Dict[str, Any]]:
    rows: List[Dict[str, Any]] = []
    registrations = parse_worktrees(git(repo, "worktree", "list", "--porcelain"))
    for registration in registrations:
        path_text = registration.get("worktree", "")
        branch = registration.get("branch", "").removeprefix("refs/heads/")
        row: Dict[str, Any] = {"path": path_text, "branch": branch or "detached"}
        if "prunable" in registration or not Path(path_text).exists():
            row["prunable"] = True
            rows.append(row)
            continue
        path = Path(path_text)
        row["head"] = git(path, "rev-parse", "HEAD")
        row["dirty"] = bool(git(path, "status", "--porcelain", check=False))
        counts = git(
            path,
            "rev-list",
            "--left-right",
            "--count",
            f"HEAD...{origin_main}",
        ).split()
        row["ahead"] = int(counts[0])
        row["behind"] = int(counts[1])
        rows.append(row)
    return rows


def refresh_managed_worker(repo: Path) -> None:
    target = freshness_worker_path(repo)
    if not target.exists():
        return
    source = run(
        (
            "git",
            "-C",
            str(repo),
            "show",
            f"origin/{DEFAULT_BRANCH}:tools/civvis_collab.py",
        )
    ).stdout.encode("utf-8")
    if FRESHNESS_MARKER.encode("utf-8") not in source:
        # During this feature's own rollout the installed worker is newer than
        # main. Keep it until the protected trunk contains the managed version;
        # after merge, every subsequent fetch can update the worker atomically.
        return
    if target.read_bytes() != source:
        atomic_write(target, source, executable=True)


def refresh_repository(repo: Path, *, wait_seconds: float = 0.0) -> Dict[str, Any]:
    root = repo_root(repo)
    with FreshnessLock(root, wait_seconds=wait_seconds) as acquired:
        if not acquired:
            previous = read_freshness_state(root) or {}
            return {**previous, "skipped": "another refresh is already running"}

        report: Dict[str, Any] = {
            "schema": FRESHNESS_SCHEMA,
            "machine": git(root, "config", "--get", "civvis.machine", check=False),
            "attempted_at": utc_now(),
        }
        try:
            git(root, "fetch", "--prune", "origin")
            origin_main = git(root, "rev-parse", f"origin/{DEFAULT_BRANCH}")
            report["fetched_at"] = utc_now()
            report["origin_main"] = origin_main
        except CommandError as error:
            report["fetch_error"] = str(error)
        else:
            try:
                report["main_update"] = force_update_main_worktree(root, origin_main)
            except CommandError as error:
                report["main_update_error"] = str(error)
            report["worktrees"] = worktree_freshness(root, origin_main)
            refresh_managed_worker(root)
        write_freshness_state(root, report)
        return report


def print_refresh_report(report: Dict[str, Any]) -> None:
    if report.get("skipped"):
        print(f"freshness: {report['skipped']}")
        return
    if report.get("fetch_error"):
        print(f"freshness ERROR: {report['fetch_error']}", file=sys.stderr)
        return
    print(
        f"origin/{DEFAULT_BRANCH} fetched at {report.get('fetched_at', '?')} "
        f"({str(report.get('origin_main', ''))[:12]})"
    )
    update = report.get("main_update") or {}
    if update:
        detail = (
            f"main {update.get('mode', '?')}: "
            f"{str(update.get('before', ''))[:12]} -> "
            f"{str(update.get('after', ''))[:12]} ({update.get('path', '')})"
        )
        if update.get("recovery_ref"):
            detail += f"; recovery {update['recovery_ref']}"
        print(detail)
    if report.get("main_update_error"):
        print(f"main sync ERROR: {report['main_update_error']}", file=sys.stderr)
    for row in report.get("worktrees", []):
        if row.get("prunable"):
            detail = "prunable"
        else:
            detail = (
                f"ahead {row.get('ahead', 0)}, behind {row.get('behind', 0)}, "
                f"{'dirty' if row.get('dirty') else 'clean'}"
            )
        print(f"  {row.get('branch', 'detached')}: {detail} ({row.get('path', '')})")


def refresh_command(args: argparse.Namespace) -> int:
    root = repo_root(Path(args.repo).expanduser()) if args.repo else repo_root()
    report = refresh_repository(root, wait_seconds=0.0 if args.scheduled else 30.0)
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    elif (
        not args.scheduled
        or report.get("fetch_error")
        or report.get("main_update_error")
    ):
        print_refresh_report(report)
    return 1 if report.get("fetch_error") or report.get("main_update_error") else 0


def install_managed_freshness_worker(repo: Path) -> Path:
    source = Path(__file__).resolve().read_bytes()
    if FRESHNESS_MARKER.encode("utf-8") not in source:
        raise CommandError("refusing to install an unmarked freshness worker")
    target = freshness_worker_path(repo)
    if target.exists() and FRESHNESS_MARKER.encode("utf-8") not in target.read_bytes():
        raise CommandError(f"refusing to replace unmanaged freshness worker: {target}")
    atomic_write(target, source, executable=True)
    return target


def freshness_service_label(repo: Path) -> str:
    return f"com.civvis.freshness.{freshness_key(repo)}"


def macos_freshness_plist(repo: Path, worker: Path) -> bytes:
    label = freshness_service_label(repo)
    log = freshness_dir(repo) / "service.log"
    payload = {
        "EnvironmentVariables": {"CIVVIS_FRESHNESS_MARKER": FRESHNESS_MARKER},
        "Label": label,
        "ProcessType": "Background",
        "ProgramArguments": [
            sys.executable,
            str(worker),
            "refresh",
            "--scheduled",
            "--repo",
            str(main_worktree(repo)),
        ],
        "RunAtLoad": True,
        "StandardErrorPath": str(log),
        "StandardOutPath": str(log),
        "StartInterval": FRESHNESS_INTERVAL_SECONDS,
        "ThrottleInterval": 60,
        "WorkingDirectory": str(main_worktree(repo)),
    }
    return plistlib.dumps(payload, sort_keys=True)


def systemd_quote(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"').replace("%", "%%") + '"'


def systemd_freshness_units(repo: Path, worker: Path) -> Tuple[bytes, bytes]:
    command = " ".join(
        systemd_quote(value)
        for value in (
            sys.executable,
            str(worker),
            "refresh",
            "--scheduled",
            "--repo",
            str(main_worktree(repo)),
        )
    )
    service = f"""# {FRESHNESS_MARKER}
[Unit]
Description=Refresh CIVVIS Git remote state

[Service]
Type=oneshot
WorkingDirectory={systemd_quote(str(main_worktree(repo)))}
ExecStart={command}
""".encode("utf-8")
    timer = f"""# {FRESHNESS_MARKER}
[Unit]
Description=Keep CIVVIS Git remote state fresh

[Timer]
OnBootSec=60
OnUnitActiveSec={FRESHNESS_INTERVAL_SECONDS}
Persistent=true

[Install]
WantedBy=timers.target
""".encode("utf-8")
    return service, timer


def write_managed_service(path: Path, data: bytes) -> bool:
    existing = path.read_bytes() if path.exists() else b""
    marker_encodings = ("utf-8", "utf-16-le", "utf-16-be")
    if existing and not any(
        FRESHNESS_MARKER.encode(encoding) in existing for encoding in marker_encodings
    ):
        raise CommandError(f"refusing to replace unmanaged scheduler definition: {path}")
    if existing == data:
        return False
    atomic_write(path, data)
    return True


def vbscript_string(value: str) -> str:
    return '"' + value.replace('"', '""') + '"'


def windows_freshness_launcher_data(repo: Path, worker: Path) -> bytes:
    command = subprocess.list2cmdline(
        (
            sys.executable,
            str(worker),
            "refresh",
            "--scheduled",
            "--repo",
            str(main_worktree(repo)),
        )
    )
    script = f"""' {FRESHNESS_MARKER}
Option Explicit

Dim shell
Dim command
Dim exitCode

Set shell = CreateObject("WScript.Shell")
command = {vbscript_string(command)}
exitCode = shell.Run(command, 0, True)
WScript.Quit exitCode
"""
    return script.encode("utf-16")


def windows_freshness_task_command(launcher: Path) -> str:
    wscript = shutil.which("wscript.exe")
    if not wscript:
        raise CommandError(
            "cannot install a windowless Windows freshness task: wscript.exe is missing"
        )
    return subprocess.list2cmdline((wscript, "//B", str(launcher)))


#: The memory guard's launchd label and thresholds.
#:
#: ⚠⚠⚠ macOS ENFORCES NO MEMORY CEILING, AND THIS FLEET HAS PROVED IT. On
#: 2026-08-10 two `civvis` benchmark processes reached a **206 GB and a 205 GB
#: physical footprint each on a 128 GB machine**, and the kernel answered with a
#: system-wide jetsam that terminated **14,818 processes**. Neither `ulimit -v`
#: nor `ulimit -d` is honoured on macOS, so nothing in the operating system was
#: ever going to stop it.
#:
#: The guard written that day lived in `~/.local/bin` on the one laptop that had
#: been hurt, installed by hand, tracked by nothing. Every other machine in the
#: fleet ran the same benchmarks with no ceiling at all. It ships here now for
#: the same reason the push guard and the freshness service do: a protection
#: that exists on one disk protects one disk.
#:
#: 32 GB hard on a 128 GB host is roughly a quarter of RAM — far above any
#: legitimate run measured here, far below the point where the kernel starts
#: killing things that were not asked. `--soft-gb` only matters once the system
#: is already short, and the guard reads PHYSICAL FOOTPRINT rather than RSS
#: because at the moment of that jetsam the offenders held 20 GB resident with
#: 186 GB parked in the compressor: an RSS limit would never have fired.
MEMGUARD_LABEL = "com.civvis.memguard"
MEMGUARD_HARD_GB = "32"
MEMGUARD_SOFT_GB = "16"
MEMGUARD_PRESSURE_PCT = "12"


def memguard_source(repo: Path) -> Path:
    return repo_root(repo) / "tools" / "ops" / "memguard.py"


def macos_memguard_plist(guard: Path) -> bytes:
    """launchd job for the memory guard. Every ten seconds, low-priority I/O."""
    logs = Path.home() / "Library" / "Logs" / "memguard-agent.log"
    arguments = "".join(
        f"\n\t\t<string>{value}</string>"
        for value in (
            sys.executable,
            str(guard),
            "--hard-gb",
            MEMGUARD_HARD_GB,
            "--soft-gb",
            MEMGUARD_SOFT_GB,
            "--pressure-pct",
            MEMGUARD_PRESSURE_PCT,
        )
    )
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" '
        '"http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n'
        '<plist version="1.0">\n<dict>\n'
        f"\t<key>Label</key>\n\t<string>{MEMGUARD_LABEL}</string>\n"
        f"\t<key>ProgramArguments</key>\n\t<array>{arguments}\n\t</array>\n"
        "\t<key>EnvironmentVariables</key>\n\t<dict>\n"
        f"\t\t<key>HOME</key>\n\t\t<string>{Path.home()}</string>\n"
        "\t\t<key>PATH</key>\n\t\t<string>/usr/bin:/bin:/usr/sbin:/sbin</string>\n"
        "\t</dict>\n"
        "\t<key>StartInterval</key>\n\t<integer>10</integer>\n"
        "\t<key>RunAtLoad</key>\n\t<true/>\n"
        "\t<key>LowPriorityIO</key>\n\t<true/>\n"
        f"\t<key>StandardOutPath</key>\n\t<string>{logs}</string>\n"
        f"\t<key>StandardErrorPath</key>\n\t<string>{logs}</string>\n"
        f"{MANAGED_KEY}"
        "</dict>\n</plist>\n"
    ).encode("utf-8")


# ⚠⚠ EVERY MANAGED PLIST MUST CARRY THIS KEY. `write_managed_service` refuses to
# overwrite a definition that does not contain FRESHNESS_MARKER, and it makes
# that check BEFORE the "identical, nothing to do" check — so a managed plist
# written without the marker can be created once and then never updated. The
# memory guard shipped that way on 2026-08-17 (#1867): the first `bootstrap`
# installed it and every later `bootstrap` raised "refusing to replace unmanaged
# scheduler definition" the moment the content had to change, e.g. because the
# tree moved or a threshold was tuned. Emit this in the plist body, not a
# comment, so the marker survives plist round-tripping.
MANAGED_KEY = (f"\t<key>CivvisManagedBy</key>\n\t<string>{FRESHNESS_MARKER}"
               "</string>\n")

SPECTATOR_LABEL = "com.civvis.spectator"

LADDER_LABEL = "com.civvis.ladder"
LADDER_WATCHDOG_LABEL = "com.civvis.ladder-watchdog"
LADDER_STALE_HOURS = "3"
LADDER_WATCHDOG_INTERVAL_SECONDS = 600


def ladder_supervisor_source(repo: Path) -> Path:
    return repo_root(repo) / "tools" / "ops" / "civvis-game-supervisor.sh"


def ladder_watchdog_source(repo: Path) -> Path:
    return repo_root(repo) / "tools" / "ops" / "ladder_watchdog.py"


def host_plays_civ6(runs: Optional[Path] = None) -> bool:
    """Whether this host is a Civilization VI seat, on evidence rather than hope.

    The supervisor drives a real Civ 6 install through the macOS GUI: it needs
    the game, Steam, and Accessibility permission. Installing a game-playing
    launchd job on a fleet machine that has none of that would be a job that can
    only fail, so the gate is a runs directory this host has actually written.
    """
    runs = runs if runs is not None else Path.home() / "civvis-civ6-runs"
    return runs.is_dir()


def macos_ladder_watchdog_plist(watchdog: Path) -> bytes:
    """The ladder keeper: one interval job, its own process, its own clock.

    ⚠⚠⚠ THERE IS DELIBERATELY NO `KeepAlive` JOB RUNNING THE SUPERVISOR. #1888
    shipped one and it cannot work on macOS: installing the control mod writes
    inside `Civ6.app`, that permission is attributed to the responsible process,
    and a LaunchAgent's children inherit launchd's empty grant set — so every
    attempt died at "cannot install .../DLC/CivvisControl" having played no
    turns, while launchd faithfully restarted a loop that could never play.
    `install.py`'s Finder fallback does not rescue it either, because driving
    Finder is an Apple Event and launchd has no Automation grant.

    So the keeper starts the loop through Terminal instead, which does hold the
    grant, and this job exists to run the keeper on an interval. It covers both
    failure modes — a loop that is gone, and a loop that is alive but finishing
    no games — because they are the same question asked in two directions. What
    matters is that it runs in its own process, so it outlives what it watches.
    """
    logs = Path.home() / "Library" / "Logs" / "civvis-ladder-watchdog.log"
    arguments = "".join(
        f"\n\t\t<string>{value}</string>"
        for value in (sys.executable, str(watchdog),
                      "--stale-hours", LADDER_STALE_HOURS)
    )
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" '
        '"http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n'
        '<plist version="1.0">\n<dict>\n'
        f"\t<key>Label</key>\n\t<string>{LADDER_WATCHDOG_LABEL}</string>\n"
        f"\t<key>ProgramArguments</key>\n\t<array>{arguments}\n\t</array>\n"
        "\t<key>EnvironmentVariables</key>\n\t<dict>\n"
        f"\t\t<key>HOME</key>\n\t\t<string>{Path.home()}</string>\n"
        "\t\t<key>PATH</key>\n\t\t<string>/usr/bin:/bin:/usr/sbin:/sbin</string>\n"
        "\t</dict>\n"
        "\t<key>StartInterval</key>\n\t<integer>"
        f"{LADDER_WATCHDOG_INTERVAL_SECONDS}</integer>\n"
        "\t<key>RunAtLoad</key>\n\t<true/>\n"
        "\t<key>LowPriorityIO</key>\n\t<true/>\n"
        f"\t<key>StandardOutPath</key>\n\t<string>{logs}</string>\n"
        f"\t<key>StandardErrorPath</key>\n\t<string>{logs}</string>\n"
        f"{MANAGED_KEY}"
        "</dict>\n</plist>\n"
    ).encode("utf-8")


def managed_job_is_loaded(label: str) -> bool:
    domain = f"gui/{os.getuid()}"
    return not run(("launchctl", "print", f"{domain}/{label}"),
                   check=False).returncode


def load_managed_job(label: str, path: Path, attempts: int = 10) -> None:
    """Write-then-(re)load a managed LaunchAgent, and confirm it is there.

    ⚠⚠ `launchctl bootout` IS ASYNCHRONOUS, AND `bootstrap` RIGHT BEHIND IT
    FAILS. The service is still tearing down when the next command arrives, so
    the bootstrap is refused — and because every call here passes `check=False`,
    nothing said a word. The plist was written, the job was gone, and the only
    symptom was a service that had simply stopped existing.

    Measured on this host 2026-08-18: `bootstrap` reported "installed CIVVIS
    spectator service" while `launchctl print` could not find the job at all.
    Running the identical `bootstrap` by hand a moment later loaded it first
    try, which is the signature of a race rather than a bad plist.

    So: wait for the bootout to actually take effect, retry the bootstrap while
    launchd is still busy, and RAISE if the job is not there at the end. A
    service reported as installed and absent is the failure mode this whole
    registry exists to prevent.
    """
    domain = f"gui/{os.getuid()}"
    if managed_job_is_loaded(label):
        run(("launchctl", "bootout", f"{domain}/{label}"), check=False)
        for _ in range(attempts):
            if not managed_job_is_loaded(label):
                break
            time.sleep(0.5)

    for attempt in range(attempts):
        run(("launchctl", "bootstrap", domain, str(path)), check=False)
        if managed_job_is_loaded(label):
            break
        time.sleep(0.5)
    else:
        raise CommandError(
            f"launchd would not load {label} from {path}. The plist is written "
            f"but the service is absent, which is the one outcome this must "
            f"never report as success; try `launchctl bootstrap {domain} {path}`"
        )
    run(("launchctl", "kickstart", f"{domain}/{label}"), check=False)


def retire_ladder_keepalive_job() -> bool:
    """Remove the KeepAlive supervisor job #1888 installed. True if one was there.

    Leaving it would be worse than never having shipped it. It restarts a loop
    that cannot install the control mod from launchd's grant set, so it plays no
    turns, burns a full head build every minute of ThrottleInterval, and holds
    the supervisor lock the keeper needs to start a loop that WOULD work.
    """
    path = Path.home() / "Library" / "LaunchAgents" / f"{LADDER_LABEL}.plist"
    domain = f"gui/{os.getuid()}"
    loaded = not run(("launchctl", "print", f"{domain}/{LADDER_LABEL}"),
                     check=False).returncode
    if loaded:
        run(("launchctl", "bootout", f"{domain}/{LADDER_LABEL}"), check=False)
    if path.exists():
        path.unlink()
        return True
    return loaded


def install_ladder_supervisor(repo: Path) -> List[Path]:
    """Install the ladder keeper. Empty where it does not apply.

    One interval job, covering both ways the loop stops producing attempts.
    There is no second job running the supervisor directly — see
    `macos_ladder_watchdog_plist` for why a LaunchAgent cannot play Civ 6.
    """
    if sys.platform != "darwin" or not host_plays_civ6():
        return []
    supervisor = ladder_supervisor_source(repo)
    watchdog = ladder_watchdog_source(repo)
    for source in (supervisor, watchdog):
        if not source.is_file():
            raise CommandError(f"versioned ladder service is missing: {source}")
    retire_ladder_keepalive_job()

    agents = Path.home() / "Library" / "LaunchAgents"
    path = agents / f"{LADDER_WATCHDOG_LABEL}.plist"
    changed = write_managed_service(path, macos_ladder_watchdog_plist(watchdog))
    domain = f"gui/{os.getuid()}"
    loaded = not run(("launchctl", "print", f"{domain}/{LADDER_WATCHDOG_LABEL}"),
                     check=False).returncode
    if changed or not loaded:
        load_managed_job(LADDER_WATCHDOG_LABEL, path)
    return [path]


def spectator_runner_source(repo: Path) -> Path:
    return repo_root(repo) / "tools" / "ops" / "civvis-spectator-runner.sh"


def host_serves_the_exhibition(source: Optional[Path] = None) -> bool:
    """Whether this host holds the spectator's canonical source worktree.

    `docs/SPECTATOR_DEPLOY.md` has the operator create it explicitly, so its
    presence is the host saying "I serve the exhibition". Installing the job
    without it would give the machine a service that can only log a missing
    prerequisite forever.
    """
    source = source if source is not None else Path.home() / "civvis-spectator-src"
    return (source / "tools" / "spectator_supervisor.py").is_file()


def macos_spectator_plist(runner: Path, repo: Path) -> bytes:
    """launchd job for the exhibition supervisor: keep it alive.

    ⚠ UNLIKE THE LADDER, THIS ONE CAN RUN DIRECTLY UNDER launchd. The ladder's
    supervisor has to be started through Terminal because installing the
    Civilization VI control mod writes inside `Civ6.app` and macOS attributes
    that permission to the responsible process. The exhibition drives no GUI —
    `--no-open`, build, serve HTTP, play headless games — so it needs no such
    grant, and it is already running under launchd today.

    `KeepAlive` is the point. On 2026-08-18 the supervisor exited and the
    exhibition stayed down until somebody looked; its own restart loop could not
    help, because the worktree it execs its supervisor from had been deleted.
    `ThrottleInterval` bounds the other direction: a runner that refuses for a
    missing prerequisite (exit 78) retries once a minute instead of spinning.
    """
    logs = Path.home() / "Library" / "Logs" / "civvis-spectator.log"
    arguments = "".join(
        f"\n\t\t<string>{value}</string>" for value in ("/bin/zsh", str(runner))
    )
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" '
        '"http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n'
        '<plist version="1.0">\n<dict>\n'
        f"\t<key>Label</key>\n\t<string>{SPECTATOR_LABEL}</string>\n"
        f"\t<key>ProgramArguments</key>\n\t<array>{arguments}\n\t</array>\n"
        f"\t<key>WorkingDirectory</key>\n\t<string>{repo_root(repo)}</string>\n"
        "\t<key>EnvironmentVariables</key>\n\t<dict>\n"
        f"\t\t<key>HOME</key>\n\t\t<string>{Path.home()}</string>\n"
        f"\t\t<key>CIVVIS_DEPLOY_ROOT</key>\n\t\t<string>{repo_root(repo)}</string>\n"
        "\t\t<key>PATH</key>\n\t\t<string>"
        f"{Path.home() / '.cargo' / 'bin'}:/usr/local/bin:/opt/homebrew/bin"
        ":/usr/bin:/bin:/usr/sbin:/sbin</string>\n"
        "\t</dict>\n"
        "\t<key>KeepAlive</key>\n\t<true/>\n"
        "\t<key>RunAtLoad</key>\n\t<true/>\n"
        "\t<key>ThrottleInterval</key>\n\t<integer>60</integer>\n"
        f"\t<key>StandardOutPath</key>\n\t<string>{logs}</string>\n"
        f"\t<key>StandardErrorPath</key>\n\t<string>{logs}</string>\n"
        f"{MANAGED_KEY}"
        "</dict>\n</plist>\n"
    ).encode("utf-8")


def install_spectator_service(repo: Path) -> Optional[Path]:
    """Keep the exhibition supervisor alive. `None` where it does not apply."""
    if sys.platform != "darwin" or not host_serves_the_exhibition():
        return None
    runner = spectator_runner_source(repo)
    if not runner.is_file():
        raise CommandError(f"versioned spectator runner is missing: {runner}")
    path = Path.home() / "Library" / "LaunchAgents" / f"{SPECTATOR_LABEL}.plist"
    changed = write_managed_service(path, macos_spectator_plist(runner, repo))
    domain = f"gui/{os.getuid()}"
    loaded = not run(("launchctl", "print", f"{domain}/{SPECTATOR_LABEL}"),
                     check=False).returncode
    if changed or not loaded:
        load_managed_job(SPECTATOR_LABEL, path)
    return path


def install_memory_guard(repo: Path) -> Optional[Path]:
    """Install the memory guard as a launchd job. `None` where it does not apply.

    macOS only, deliberately: the guard reads `vm_stat` and `ps` footprints,
    which are Darwin-specific, and the incident it exists to prevent is a
    Darwin jetsam. On other platforms this returns `None` rather than
    pretending — a guard reported as installed when it is not is worse than an
    honest absence.
    """
    guard = memguard_source(repo)
    if not guard.is_file():
        raise CommandError(f"versioned memory guard is missing: {guard}")
    if sys.platform != "darwin":
        return None
    path = Path.home() / "Library" / "LaunchAgents" / f"{MEMGUARD_LABEL}.plist"
    domain = f"gui/{os.getuid()}"
    changed = write_managed_service(path, macos_memguard_plist(guard))
    loaded = not run(
        ("launchctl", "print", f"{domain}/{MEMGUARD_LABEL}"), check=False
    ).returncode
    if loaded and not changed:
        return path
    if loaded:
        run(("launchctl", "bootout", f"{domain}/{MEMGUARD_LABEL}"), check=False)
    run(("launchctl", "bootstrap", domain, str(path)), check=False)
    run(("launchctl", "kickstart", f"{domain}/{MEMGUARD_LABEL}"), check=False)
    return path


#: Every managed background service, with the sentence to print when a host
#: does not get one. `bootstrap` installs them; `start` REPAIRS them, which is
#: the half that was missing.
#:
#: ⚠⚠ A SERVICE ADDED AFTER A MACHINE WAS BOOTSTRAPPED NEVER REACHED IT.
#: `bootstrap` is documented as a once-per-clone step and behaves like one, so
#: for a year the repair path on every task installed exactly the two
#: safeguards that existed when it was written — the push guard and the
#: freshness service. The memory guard and the ladder keeper were added later
#: and were bootstrap-only. Measured on `mbp-m5-pro-64` 2026-08-18: a host that
#: `host_plays_civ6()` calls a Civilization VI seat, with the freshness services
#: loaded and `com.civvis.ladder-watchdog` **absent** — so the keeper built on
#: 2026-08-17 to end a 14.3-hour silent outage was not running on the machine it
#: was built for. AGENTS.md said "the task launcher repairs both safeguards",
#: and "both" was the whole bug.
#:
#: ⚠ DISCOVERED, NOT LISTED. `test_civvis_collab.py` finds every installer that
#: writes into `~/Library/LaunchAgents` and fails if it is missing here, because
#: a hand-written list of things to repair is complete on the day it is written
#: and silently shrinks afterwards.
MANAGED_SERVICES: Tuple[Tuple[str, str, str], ...] = (
    ("freshness service", "install_freshness_service", ""),
    ("memory guard", "install_memory_guard",
     "macOS only, not installed on this platform"),
    ("ladder service", "install_ladder_supervisor",
     "this host is not a Civilization VI seat, not installed"),
    ("spectator service", "install_spectator_service",
     "this host does not hold the exhibition's source worktree, not installed"),
)


def install_managed_services(repo: Path) -> List[Tuple[str, str, List[Path]]]:
    """Install or repair every managed service. Idempotent, and cheap when green.

    Each installer already returns without touching launchd when its plist is
    unchanged and its job is loaded, which is what makes this safe to run on
    every `start` — the freshness service has been called that way all along.
    """
    results: List[Tuple[str, str, List[Path]]] = []
    for name, function, absent in MANAGED_SERVICES:
        produced = globals()[function](repo)
        if produced is None:
            paths: List[Path] = []
        elif isinstance(produced, Path):
            paths = [produced]
        else:
            paths = list(produced)
        results.append((name, absent, paths))
    return results


def install_freshness_service(repo: Path) -> List[Path]:
    root = repo_root(repo)
    worker = install_managed_freshness_worker(root)
    key = freshness_key(root)
    if sys.platform == "darwin":
        label = freshness_service_label(root)
        path = Path.home() / "Library" / "LaunchAgents" / f"{label}.plist"
        domain = f"gui/{os.getuid()}"
        changed = write_managed_service(path, macos_freshness_plist(root, worker))
        loaded = not run(
            ("launchctl", "print", f"{domain}/{label}"), check=False
        ).returncode
        if loaded and not changed:
            return [path]
        if loaded:
            run(("launchctl", "bootout", f"{domain}/{label}"))
        run(("launchctl", "bootstrap", domain, str(path)))
        run(("launchctl", "kickstart", f"{domain}/{label}"))
        return [path]
    if os.name == "nt":
        name = f"CIVVIS Git Freshness {key}"
        launcher = freshness_dir(root) / "run-hidden.vbs"
        write_managed_service(
            launcher,
            windows_freshness_launcher_data(root, worker),
        )
        command = windows_freshness_task_command(launcher)
        run(
            (
                "schtasks",
                "/Create",
                "/SC",
                "MINUTE",
                "/MO",
                str(max(1, FRESHNESS_INTERVAL_SECONDS // 60)),
                "/TN",
                name,
                "/TR",
                command,
                "/F",
            ),
            cwd=main_worktree(root),
        )
        return [worker, launcher]
    if shutil.which("systemctl"):
        directory = Path.home() / ".config" / "systemd" / "user"
        service = directory / f"civvis-freshness-{key}.service"
        timer = directory / f"civvis-freshness-{key}.timer"
        service_data, timer_data = systemd_freshness_units(root, worker)
        changed = write_managed_service(service, service_data)
        changed = write_managed_service(timer, timer_data) or changed
        enabled = not run(
            ("systemctl", "--user", "is-enabled", timer.name), check=False
        ).returncode
        if changed:
            run(("systemctl", "--user", "daemon-reload"))
        if changed or not enabled:
            run(("systemctl", "--user", "enable", "--now", timer.name))
        return [service, timer]
    raise CommandError("no supported per-user scheduler is available on this computer")


def freshness_service_error(repo: Path) -> Optional[str]:
    root = repo_root(repo)
    worker = freshness_worker_path(root)
    if not worker.is_file() or FRESHNESS_MARKER.encode("utf-8") not in worker.read_bytes():
        return (
            "managed Git freshness worker is missing; run "
            "python3 tools/civvis_collab.py bootstrap"
        )
    key = freshness_key(root)
    if sys.platform == "darwin":
        label = freshness_service_label(root)
        path = Path.home() / "Library" / "LaunchAgents" / f"{label}.plist"
        if not path.is_file() or FRESHNESS_MARKER.encode("utf-8") not in path.read_bytes():
            return "managed Git freshness LaunchAgent is missing or stale; run bootstrap"
        try:
            payload = plistlib.loads(path.read_bytes())
        except (TypeError, ValueError):
            return "managed Git freshness LaunchAgent is unreadable; run bootstrap"
        expected_tail = [
            str(worker),
            "refresh",
            "--scheduled",
            "--repo",
            str(main_worktree(root)),
        ]
        if (
            list(payload.get("ProgramArguments") or [])[1:] != expected_tail
            or payload.get("StartInterval") != FRESHNESS_INTERVAL_SECONDS
            or payload.get("WorkingDirectory") != str(main_worktree(root))
        ):
            return "managed Git freshness LaunchAgent is outdated; run bootstrap"
        domain = f"gui/{os.getuid()}/{label}"
        if run(("launchctl", "print", domain), check=False).returncode:
            return "managed Git freshness LaunchAgent is not loaded; run bootstrap"
        return None
    if os.name == "nt":
        name = f"CIVVIS Git Freshness {key}"
        if run(("schtasks", "/Query", "/TN", name), check=False).returncode:
            return "managed Git freshness scheduled task is missing; run bootstrap"
        return None
    if shutil.which("systemctl"):
        directory = Path.home() / ".config" / "systemd" / "user"
        timer = f"civvis-freshness-{key}.timer"
        service_path = directory / f"civvis-freshness-{key}.service"
        timer_path = directory / timer
        try:
            service_text = service_path.read_text(encoding="utf-8")
            timer_text = timer_path.read_text(encoding="utf-8")
        except OSError:
            return "managed Git freshness systemd units are missing; run bootstrap"
        if (
            FRESHNESS_MARKER not in service_text
            or str(worker) not in service_text
            or '"refresh" "--scheduled" "--repo"' not in service_text
            or FRESHNESS_MARKER not in timer_text
            or f"OnUnitActiveSec={FRESHNESS_INTERVAL_SECONDS}" not in timer_text
        ):
            return "managed Git freshness systemd units are outdated; run bootstrap"
        if run(("systemctl", "--user", "is-enabled", timer), check=False).returncode:
            return "managed Git freshness timer is not enabled; run bootstrap"
        return None
    return "no supported per-user scheduler is available for Git freshness"


def freshness_state_error(repo: Path, remote_main: str = "") -> Optional[str]:
    state = read_freshness_state(repo)
    if not state:
        return "Git freshness heartbeat is missing; run python3 tools/civvis_collab.py bootstrap"
    if state.get("schema") != FRESHNESS_SCHEMA:
        return "Git freshness heartbeat schema is outdated; run bootstrap"
    configured_machine = git(repo, "config", "--get", "civvis.machine", check=False)
    if configured_machine and state.get("machine") != configured_machine:
        return "Git freshness heartbeat belongs to a different machine identity"
    if state.get("fetch_error"):
        return "last Git freshness fetch failed: " + str(state["fetch_error"])
    if state.get("main_update_error"):
        return "last automatic main update failed: " + str(state["main_update_error"])
    try:
        observed = dt.datetime.fromisoformat(str(state.get("fetched_at", "")))
        if observed.tzinfo is None:
            raise ValueError
        age = (dt.datetime.now(dt.timezone.utc) - observed).total_seconds()
    except (TypeError, ValueError):
        return "Git freshness heartbeat timestamp is invalid; run bootstrap"
    if age > FRESHNESS_STALE_SECONDS:
        return f"Git freshness heartbeat is stale ({int(age)} seconds old)"
    if remote_main and state.get("origin_main") != remote_main:
        return "this computer has not fetched the current GitHub main revision"
    return None


def gh_api_optional(path: str) -> Tuple[int, Any]:
    result = run(("gh", "api", path), check=False)
    if result.returncode:
        return result.returncode, None
    return 0, json.loads(result.stdout)


def gh_api_write(
    method: str, path: str, payload: Dict[str, Any], *, check: bool = True
) -> Any:
    """Call a writing GitHub endpoint.

    With ``check=False`` a rejected call returns ``None`` instead of raising, so
    a caller can fall back to a different strategy.
    """
    result = subprocess.run(
        ("gh", "api", "--method", method, path, "--input", "-"),
        input=json.dumps(payload),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode and not check:
        return None
    if result.returncode:
        detail = "\n".join(
            value.strip() for value in (result.stdout, result.stderr) if value.strip()
        )
        raise CommandError(
            f"GitHub {method} {path} failed ({result.returncode}): {detail}"
        )
    return json.loads(result.stdout or "null")


def personal_repository_protection_payload() -> Dict[str, Any]:
    """Return branch protection accepted for a user-owned GitHub repository."""
    return {
        "required_status_checks": {
            # Not strict. Requiring every branch to be rebased onto the newest
            # main serialises the whole fleet: with N open PRs and one merge per
            # CI round, every other branch is invalidated the moment one lands,
            # and the queue spends its time re-running green checks. The checks
            # themselves still gate; only the up-to-date requirement is dropped.
            "strict": False,
            "contexts": ["cargo-test", "collaboration-policy"],
        },
        # Admins are not held to the required checks, so an outage outside the
        # repository can never hard-block main. On 2026-07-25 GitHub-hosted
        # Actions stopped starting jobs at all (a billing failure), every run
        # died in three seconds with zero steps, and with enforce_admins on there
        # was no way to land verified work. Self-hosted runners fixed the cause;
        # this keeps the next such outage from being unrecoverable.
        "enforce_admins": False,
        "required_pull_request_reviews": {
            "dismiss_stale_reviews": False,
            "require_code_owner_reviews": False,
            "required_approving_review_count": 0,
            "require_last_push_approval": False,
        },
        "restrictions": None,
        "required_conversation_resolution": True,
        "required_linear_history": True,
        "allow_force_pushes": False,
        "allow_deletions": False,
        "block_creations": False,
        "lock_branch": False,
    }


def enforce_github_command(args: argparse.Namespace) -> int:
    root = repo_root()
    permission = gh_json(("api", f"repos/{REPOSITORY}", "--jq", ".permissions.admin"), cwd=root)
    if permission is not True:
        raise CommandError(
            "the active GitHub account is not a repository administrator; "
            "authenticate the MartinHalvorson owner account, then rerun"
        )

    gh_api_write(
        "PATCH",
        f"repos/{REPOSITORY}",
        {
            "allow_squash_merge": True,
            "allow_merge_commit": False,
            "allow_rebase_merge": False,
            "allow_auto_merge": True,
            "delete_branch_on_merge": True,
            "squash_merge_commit_title": "PR_TITLE",
            "squash_merge_commit_message": "PR_BODY",
        },
    )
    gh_api_write(
        "PUT",
        f"repos/{REPOSITORY}/branches/{DEFAULT_BRANCH}/protection",
        personal_repository_protection_payload(),
    )
    print("GitHub enforcement applied: PR-only current green main, squash-only, no force/delete")
    return audit_command(argparse.Namespace(json=False))


def audit_repo(root: Path) -> Dict[str, List[str]]:
    findings: Dict[str, List[str]] = {"errors": [], "warnings": [], "ok": []}
    errors, warnings, ok = findings["errors"], findings["warnings"], findings["ok"]
    remote_main = ""

    if shutil.which("gh"):
        repo = gh_json(("api", f"repos/{REPOSITORY}"), cwd=root)
        heads = remote_heads(root)
        main_sha = heads.get(DEFAULT_BRANCH, "")
        remote_main = main_sha
        if repo.get("allow_merge_commit") or repo.get("allow_rebase_merge"):
            errors.append("repository permits non-squash merge methods")
        else:
            ok.append("squash is the only enabled merge method")
        if not repo.get("delete_branch_on_merge"):
            errors.append("merged branches are not deleted automatically")
        else:
            ok.append("merged branches are deleted automatically")

        _, rulesets = gh_api_optional(f"repos/{REPOSITORY}/rulesets")
        active = [row for row in (rulesets or []) if row.get("enforcement") == "active"]
        protection_code, protection = gh_api_optional(
            f"repos/{REPOSITORY}/branches/{DEFAULT_BRANCH}/protection"
        )
        is_admin = bool((repo.get("permissions") or {}).get("admin"))
        if not active and protection_code and is_admin:
            errors.append("no active GitHub ruleset or branch protection protects main")
        elif not active and protection_code:
            warnings.append(
                "non-admin GitHub account cannot inspect main branch protection; "
                "rerun the audit with owner credentials for authoritative verification"
            )
        else:
            ok.append("GitHub protects main")
        if protection:
            checks = set((protection.get("required_status_checks") or {}).get("contexts") or [])
            required = {"cargo-test", "collaboration-policy"}
            if not required.issubset(checks):
                errors.append("main does not require both cargo-test and collaboration-policy")
            else:
                ok.append("main requires cargo-test and collaboration-policy checks")
            # `strict` and `enforce_admins` are deliberately off; see
            # personal_repository_protection_payload(). Requiring up-to-date heads
            # serialises the queue behind one merge per CI round, and holding
            # admins to the checks left main unmergeable for hours on 2026-07-25
            # when Actions could not start a single job. Flag the old settings as
            # warnings so the audit agrees with what enforce-github writes.
            if (protection.get("required_status_checks") or {}).get("strict"):
                warnings.append(
                    "main requires up-to-date PR heads; this serialises the merge queue"
                )
            if (protection.get("enforce_admins") or {}).get("enabled"):
                warnings.append(
                    "main holds admins to the required checks; a CI outage can hard-block main"
                )
            if not (protection.get("required_conversation_resolution") or {}).get("enabled"):
                errors.append("main does not require conversation resolution")
            if (protection.get("allow_force_pushes") or {}).get("enabled"):
                errors.append("main permits force pushes")
            if (protection.get("allow_deletions") or {}).get("enabled"):
                errors.append("main permits deletion")

        workflows = gh_json(
            ("workflow", "list", "--repo", REPOSITORY, "--json", "name,path,state"),
            cwd=root,
        )
        active_paths = {
            row["path"] for row in (workflows or []) if row.get("state") == "active"
        }
        required_workflows = {
            ".github/workflows/tests.yml",
            ".github/workflows/collaboration-policy.yml",
        }
        missing = sorted(required_workflows - active_paths)
        if missing:
            errors.append("required workflows are not active: " + ", ".join(missing))
        else:
            ok.append("test and collaboration-policy workflows are active")

        prs = existing_pr_claims(root)
        open_heads = {str(pr.get("headRefName") or "") for pr in prs}
        pr_views: Dict[int, Dict[str, Any]] = {}
        pr_changed: Dict[int, Set[str]] = {}
        for pr in prs:
            number = int(pr["number"])
            view = gh_json(
                (
                    "pr",
                    "view",
                    str(number),
                    "--repo",
                    REPOSITORY,
                    "--json",
                    "files,commits,isDraft,body,headRefName,headRefOid,number",
                ),
                cwd=root,
            )
            pr_views[number] = view
            pr_changed[number] = {row["path"] for row in view.get("files", [])}
        # `gh pr view` reports paths without patches, which would force the
        # whole-file fallback and report collisions CI does not. Fetch the same
        # hunk ranges check-pr uses so the audit and the gate always agree.
        pr_ranges: Dict[int, Dict[str, Optional[List[Tuple[int, int]]]]] = {
            number: gh_pr_file_ranges(number, cwd=root) for number in pr_views
        }
        for number, view in pr_views.items():
            files = sorted(pr_changed[number])
            subjects = [row["messageHeadline"] for row in view.get("commits", [])]
            others = {
                key: value
                for key, value in pr_ranges.items()
                if key != number and set(value) & set(pr_ranges[number])
            }
            other_coordination = {
                key: split_coordination(
                    parse_claims(str(pr_views[key].get("body") or "")).get(
                        "coordinated", ""
                    )
                )
                for key in others
            }
            violations = validate_pr(
                view,
                files=files,
                commit_subjects=subjects,
                ranges=pr_ranges[number],
                other_ranges=others,
                other_coordination=other_coordination,
            )
            for violation in violations:
                errors.append(f"PR #{number}: {violation}")
            if not view.get("isDraft") and main_sha:
                head_sha = str(view.get("headRefOid") or "")
                if head_sha:
                    comparison, unmeasured = comparison_or_reason(
                        lambda: gh_json(
                            (
                                "api",
                                f"repos/{REPOSITORY}/compare/"
                                f"{main_sha}...{head_sha}",
                            ),
                            cwd=root,
                        )
                    )
                    if comparison is None:
                        # ⚠ THIS IS THE CALL THAT KILLED THE WHOLE AUDIT. One
                        # PR GitHub would not compare raised out of the loop
                        # and `audit` printed no findings at all — not the
                        # other PRs' violations, not the branch checks below,
                        # nothing. A report that reports nothing when one row
                        # is awkward is worse than the row.
                        warnings.append(
                            f"PR #{number}: could not compare against main "
                            f"({unmeasured}); freshness unverified"
                        )
                    elif not compare_status_is_current(
                        str(comparison.get("status") or "")
                    ):
                        # Transient, and enforced by GitHub at merge time. See
                        # the note in check_pr_action.
                        warnings.append(
                            f"PR #{number}: behind current main; GitHub will "
                            "require an update before it can merge"
                        )
        if prs and not any(item.startswith("PR #") for item in errors):
            ok.append(f"all {len(prs)} open PR claim(s) satisfy policy")

        for branch in sorted(heads):
            if branch == DEFAULT_BRANCH:
                continue
            if not BRANCH_RE.fullmatch(branch):
                errors.append(f"nonconforming remote development branch: {branch}")
            elif branch not in open_heads:
                errors.append(f"remote task branch has no open PR claim: {branch}")
    else:
        errors.append("GitHub CLI is unavailable; remote enforcement cannot be audited")

    worktrees = parse_worktrees(git(root, "worktree", "list", "--porcelain"))
    for row in worktrees:
        path_text = row.get("worktree", "")
        branch = row.get("branch", "").removeprefix("refs/heads/")
        if "prunable" in row:
            warnings.append(f"prunable worktree registration: {path_text}")
            continue
        path = Path(path_text)
        status = git(path, "status", "--porcelain", check=False) if path.exists() else ""
        if branch == DEFAULT_BRANCH and status:
            errors.append(f"main worktree is dirty: {path}")
        elif branch and branch != DEFAULT_BRANCH and not BRANCH_RE.fullmatch(branch):
            label = "dirty" if status else "clean"
            warnings.append(f"legacy/nonconforming {label} worktree branch {branch}: {path}")
    ok.append(f"inspected {len(worktrees)} local worktree registration(s)")

    hook_error = push_guard_error(root)
    if hook_error:
        errors.append(hook_error)
    else:
        ok.append("shared local pre-push guard is installed and current")

    service_error = freshness_service_error(root)
    if service_error:
        errors.append(service_error)
    else:
        ok.append("automatic Git main synchronization service is installed and enabled")
    state_error = freshness_state_error(root, remote_main)
    if state_error:
        errors.append(state_error)
    else:
        ok.append("this computer synchronized its management checkout to GitHub main recently")
        state = read_freshness_state(root) or {}
        for row in state.get("worktrees", []):
            behind = int(row.get("behind", 0))
            if behind:
                message = (
                    f"local worktree {row.get('branch', 'detached')} is "
                    f"{behind} commit(s) behind main: {row.get('path', '')}"
                )
                if row.get("branch") == DEFAULT_BRANCH:
                    errors.append(message)
                else:
                    warnings.append(message)

    if sys.platform == "darwin":
        agents = Path.home() / "Library" / "LaunchAgents"
        active = sorted(agents.glob("*civvis*autosync*.plist")) if agents.exists() else []
        if active:
            errors.append(
                "unmanaged mutating CIVVIS launch agents remain installed: "
                + ", ".join(map(str, active))
            )
        else:
            ok.append("no unmanaged mutating CIVVIS launch agent is installed")
    elif os.name == "nt" and shutil.which("schtasks"):
        result = run(("schtasks", "/Query", "/TN", "CIVVIS Git Autosync"), check=False)
        if result.returncode == 0:
            errors.append("unmanaged mutating CIVVIS scheduled task remains installed")
    elif shutil.which("systemctl"):
        result = run(("systemctl", "--user", "is-enabled", "civvis-autosync.timer"), check=False)
        if result.returncode == 0:
            errors.append("unmanaged mutating CIVVIS systemd timer remains enabled")

    return findings


def print_findings(findings: Dict[str, List[str]], *, as_json: bool = False) -> None:
    if as_json:
        print(json.dumps(findings, indent=2, sort_keys=True))
        return
    labels = {"errors": "ERROR", "warnings": "WARNING", "ok": "OK"}
    for level in ("errors", "warnings", "ok"):
        for message in findings[level]:
            print(f"{labels[level]:7} {message}")
    print(
        f"SUMMARY errors={len(findings['errors'])} "
        f"warnings={len(findings['warnings'])} ok={len(findings['ok'])}"
    )


def audit_command(args: argparse.Namespace) -> int:
    findings = audit_repo(repo_root())
    print_findings(findings, as_json=args.json)
    return 1 if findings["errors"] else 0


def monitor_command(args: argparse.Namespace) -> int:
    root = repo_root()
    duration = max(1, args.duration_minutes) * 60
    interval = max(60, args.interval_seconds)
    deadline = time.monotonic() + duration
    log_path = Path(args.log).expanduser().resolve()
    log_path.parent.mkdir(parents=True, exist_ok=True)
    rounds = 0
    ever_failed = False
    previous_heads = remote_heads(root)
    while True:
        rounds += 1
        observed = dt.datetime.now(dt.timezone.utc).isoformat()
        findings = audit_repo(root)
        current_heads = remote_heads(root)
        main_before = previous_heads.get(DEFAULT_BRANCH)
        main_after = current_heads.get(DEFAULT_BRANCH)
        if main_before and main_after and main_before != main_after:
            git(root, "fetch", "--prune", "origin", DEFAULT_BRANCH)
            ancestry = run(
                (
                    "git",
                    "-C",
                    str(root),
                    "merge-base",
                    "--is-ancestor",
                    main_before,
                    main_after,
                ),
                check=False,
            )
            if ancestry.returncode:
                findings["errors"].append(
                    f"main was rewritten or force-pushed: {main_before[:7]} -> {main_after[:7]}"
                )
                commits = [main_after]
            else:
                commits = git(
                    root, "rev-list", "--reverse", f"{main_before}..{main_after}"
                ).splitlines()
            for sha in commits:
                pr_number = associated_pr_number(sha)
                if pr_number is None:
                    subject = git(root, "show", "-s", "--format=%s", sha)
                    findings["errors"].append(
                        f"direct main commit detected: {sha[:7]} {subject}"
                    )
                else:
                    findings["ok"].append(
                        f"main commit {sha[:7]} is backed by merged PR #{pr_number}"
                    )
                    parent_sha = git(root, "rev-parse", f"{sha}^")
                    gate_errors = merged_pr_gate_errors(pr_number, parent_sha)
                    for error in gate_errors:
                        findings["errors"].append(
                            f"PR #{pr_number} merged without a green gate: {error}"
                        )
                    if not gate_errors:
                        findings["ok"].append(
                            f"PR #{pr_number} had both required checks green before merge"
                        )
        for branch, sha in current_heads.items():
            if branch == DEFAULT_BRANCH or previous_heads.get(branch) == sha:
                continue
            if not BRANCH_RE.fullmatch(branch):
                findings["errors"].append(
                    f"new or updated nonconforming remote branch: {branch} at {sha[:7]}"
                )
        previous_heads = current_heads
        ever_failed = ever_failed or bool(findings["errors"])
        record = {"observed_at": observed, "round": rounds, **findings}
        with log_path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(record, sort_keys=True) + "\n")
        print(
            f"MONITOR {observed} round={rounds} errors={len(findings['errors'])} "
            f"warnings={len(findings['warnings'])}",
            flush=True,
        )
        if time.monotonic() >= deadline:
            break
        next_audit = min(deadline, time.monotonic() + interval)
        while time.monotonic() < next_audit:
            remaining = next_audit - time.monotonic()
            time.sleep(min(60, max(0, remaining)))
            if time.monotonic() < next_audit:
                print("MONITOR heartbeat: waiting for next fleet audit", flush=True)
    print(f"MONITOR complete rounds={rounds} log={log_path}")
    return 1 if ever_failed else 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    bootstrap = sub.add_parser(
        "bootstrap",
        help="install clone guards and automatic main synchronization",
    )
    bootstrap.set_defaults(func=bootstrap_command)

    refresh = sub.add_parser(
        "refresh",
        help="force-update the main checkout and report task-worktree freshness",
    )
    refresh.add_argument("--json", action="store_true")
    refresh.add_argument("--scheduled", action="store_true", help=argparse.SUPPRESS)
    refresh.add_argument("--repo", default="", help=argparse.SUPPRESS)
    refresh.set_defaults(func=refresh_command)

    start = sub.add_parser("start", help="create a task worktree, branch, and draft PR claim")
    start.add_argument("task", help="lowercase hyphenated task slug")
    start.add_argument("--machine", help="stable fleet-unique machine ID")
    start.add_argument("--agent", help="active agent/session ID")
    start.add_argument("--path", action="append", required=True, help="claimed path or glob")
    start.add_argument("--coordinate", type=int, action="append", default=[], help="coordinated PR number")
    start.add_argument("--title", help="draft PR title")
    start.add_argument("--parent", help="directory in which to create the worktree")
    start.add_argument("--dry-run", action="store_true")
    start.set_defaults(func=start_task)

    ship = sub.add_parser(
        "ship",
        help="push a finished task, wait for green CI, merge it, and verify live",
    )
    ship.add_argument("--timeout-seconds", type=float, default=1200.0)
    ship.add_argument("--poll-seconds", type=float, default=10.0)
    ship.add_argument(
        "--live-timeout-seconds",
        type=float,
        default=LIVE_BUILD_HANDOFF_TIMEOUT_S,
    )
    ship.add_argument(
        "--live-url",
        default=os.environ.get(
            "CIVVIS_LIVE_STATUS_URL", "http://127.0.0.1:8766/status"
        ),
    )
    ship.set_defaults(func=ship_task)

    check_pr = sub.add_parser("check-pr", help="validate the current GitHub pull request event")
    check_pr.add_argument("--event", default=os.environ.get("GITHUB_EVENT_PATH", ""))
    check_pr.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY", REPOSITORY))
    check_pr.add_argument("--token", default=os.environ.get("GITHUB_TOKEN", ""))

    audit = sub.add_parser("audit", help="audit local and GitHub fleet enforcement")
    audit.add_argument("--json", action="store_true")
    audit.set_defaults(func=audit_command)

    enforce = sub.add_parser("enforce-github", help="apply repository and main protection settings")
    enforce.set_defaults(func=enforce_github_command)

    install_hooks = sub.add_parser(
        "install-hooks", help="install the shared local pre-push guard for this clone"
    )
    install_hooks.set_defaults(func=install_hooks_command)

    monitor = sub.add_parser("monitor", help="run recurring fleet audits")
    monitor.add_argument("--duration-minutes", type=int, default=180)
    monitor.add_argument("--interval-seconds", type=int, default=300)
    monitor.add_argument(
        "--log",
        default=str(Path.home() / ".local/state/civvis-collab/monitor.jsonl"),
    )
    monitor.set_defaults(func=monitor_command)
    return parser


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.command == "check-pr":
        if not args.event:
            parser.error("check-pr requires --event or GITHUB_EVENT_PATH")
        if not args.token:
            parser.error("check-pr requires --token or GITHUB_TOKEN")
        return check_pr_action(Path(args.event), args.token, args.repository)
    try:
        return int(args.func(args))
    except CommandError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
