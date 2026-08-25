#!/usr/bin/env python3
"""Publish a daily inventory of work that exists but is going nowhere.

Three shapes of stranded work have each cost this fleet real progress, and
none of them is visible from any single machine:

- **Commentless closes.** A PR closed unmerged with a written rationale is a
  decision; one closed silently is indistinguishable from an accident. On
  2026-08-05 two of them (#1250, #1252) triggered a full cross-machine
  forensic sweep that a one-line closing comment would have made unnecessary.
- **Idle task branches.** A branch whose last real commit is more than a day
  old, with no open PR, is work only one disk plus GitHub remembers — and
  nothing routinely asks whether it was meant to land.
- **Rescue snapshots.** `civvis_worktree_audit.py --rescue` preserves dirty
  worktrees under `refs/civvis/wip/*` precisely so bytes survive a dead
  session. Preservation without review is a graveyard, not a rescue.
- **Workflow runs that never end.** A run stuck non-terminal is work GitHub
  believes is still happening. `release.yml` run 31116714949 — the re-run of
  the `v0.6.1` tag build — held `queued` with **zero jobs ever allocated** from
  2026-08-06 until it was cancelled 390 hours later. No `timeout-minutes` can
  reach that: the timeout clock starts when a runner picks a job up, and this
  run had no jobs. Nothing in the repository asked whether a run was still
  alive, so the only bound on it was somebody noticing.

This report upserts a single issue (title below) rather than filing new ones:
one place to look, no notification pile, and the issue's edit history is the
fleet's stranded-work timeline for free. Rows carry enough to act on —
branch, age, subject, line counts — so triage happens from the issue without
an archaeology session. An empty report writes "nothing stranded" rather than
skipping the update: silence must be distinguishable from breakage.

Closing the issue is the operator's "cycle remediated" signal, not the end of
the report: the upsert finds the issue in either state and keeps writing the
timeline into it, and reopens it only when an actionable row — a commentless
close or an idle branch — appears again. Rescue snapshots alone never reopen
it; they are preserved history and would otherwise pin the issue open
forever. Before 2026-08-07 the search covered open issues only, so the first
close would have silently forked the timeline into a fresh issue.

    GH_TOKEN=... ./tools/stranded_work_report.py [--dry-run]
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import re
import subprocess
import sys
from urllib.parse import quote
import urllib.request

REPOSITORY = "MartinHalvorson/CIVVIS"
ISSUE_TITLE = "Stranded work report"
ISSUE_LABEL = "stranded-work"
CLOSE_WINDOW_DAYS = 7
IDLE_HOURS = 24
# GitHub cancels a job that has waited 24 hours for a hosted runner, and the
# longest `timeout-minutes` in this repository is 90. A run still non-terminal
# past a day is therefore one that cannot finish on its own.
STUCK_RUN_HOURS = 24


def github_url(kind: str, value: str) -> str:
    """Return an encoded GitHub link suitable for a report row."""
    return f"https://github.com/{REPOSITORY}/{kind}/{quote(value, safe='/')}"


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], capture_output=True, text=True, encoding="utf-8",
        errors="replace", check=False,
    ).stdout


def api(path: str, method: str = "GET", data: dict | None = None):
    request = urllib.request.Request(
        f"https://api.github.com{path}",
        method=method,
        data=json.dumps(data).encode() if data is not None else None,
        headers={
            "Authorization": f"Bearer {os.environ['GH_TOKEN']}",
            "Accept": "application/vnd.github+json",
        },
    )
    with urllib.request.urlopen(request) as response:
        return json.load(response)


def commentless_closes(now: datetime.datetime) -> list[str]:
    """Closed-unmerged PRs from the window whose thread holds no comment."""
    since = now - datetime.timedelta(days=CLOSE_WINDOW_DAYS)
    rows = []
    pulls = api(f"/repos/{REPOSITORY}/pulls?state=closed&sort=updated"
                f"&direction=desc&per_page=100")
    for pull in pulls:
        if pull.get("merged_at") or not pull.get("closed_at"):
            continue
        closed = datetime.datetime.fromisoformat(pull["closed_at"].rstrip("Z"))
        if closed < since:
            continue
        if api(f"/repos/{REPOSITORY}/issues/{pull['number']}/comments?per_page=1"):
            continue
        rows.append(
            f"- [#{pull['number']}]({github_url('pull', str(pull['number']))}) "
            f"{pull['head']['ref']} closed "
            f"{pull['closed_at'][:16]} with no stated reason — "
            f"{pull['title'][:80]}"
        )
    return rows


def idle_branches(now: datetime.datetime) -> list[str]:
    """agent/* branches whose tip is real work, aging, and not in an open PR."""
    open_refs = {
        pull["head"]["ref"]
        for pull in api(f"/repos/{REPOSITORY}/pulls?state=open&per_page=100")
    }
    rows = []
    git("fetch", "--prune", "origin")
    for line in git(
        "for-each-ref", "refs/remotes/origin/agent/",
        "--format=%(refname:short)\t%(committerdate:unix)\t%(contents:subject)",
    ).splitlines():
        name, stamp, subject = line.split("\t", 2)
        branch = name.removeprefix("origin/")
        if branch in open_refs or subject.startswith("claim:"):
            continue
        age_hours = (now.timestamp() - int(stamp)) / 3600
        if age_hours < IDLE_HOURS:
            continue
        ahead = git("rev-list", "--count", f"origin/main..{name}").strip()
        if ahead in ("", "0"):
            continue
        stat = git("diff", "--shortstat", f"origin/main...{name}").strip()
        rows.append(
            f"- [{branch}]({github_url('tree', branch)}) idle "
            f"{age_hours / 24:.1f}d, {ahead} commits ahead "
            f"({stat or 'no diff'}) — {subject[:70]}"
        )
    return rows


def branch_of(ref: str) -> str:
    """The agent branch a snapshot was taken from."""
    return ref.removeprefix("refs/civvis/wip/")


def merged_branches() -> set[str]:
    """Branches whose pull request merged, so their snapshot is history.

    One paged listing rather than a call per snapshot: at 151 snapshots the
    per-branch form is 151 requests, which is both slow and the kind of thing
    that quietly eats a rate limit on a scheduled job.
    """
    merged: set[str] = set()
    page = 1
    while page <= 10:
        pulls = api(f"/repos/{REPOSITORY}/pulls?state=closed&sort=updated"
                    f"&direction=desc&per_page=100&page={page}")
        if not pulls:
            break
        for pull in pulls:
            if pull.get("merged_at"):
                merged.add((pull.get("head") or {}).get("ref") or "")
        page += 1
    merged.discard("")
    return merged


def rescue_refs() -> list[str]:
    """Snapshots whose work never landed, and a count of the ones that did.

    ⚠ A LIST OF 151 IS NOT A TO-DO LIST. This reported every snapshot the audit
    had ever preserved, in one undifferentiated wall, and the wall is why the
    section was never drained: the overwhelming majority are the preserved
    history of work that DID land, and nothing distinguished them from the few
    that hold something nobody ever merged.

    A snapshot whose branch merged is finished business. What is left after
    removing those is the actual queue, and it is short enough to read.

    Note a snapshot is NOT judged by diffing it against `main`. The fleet
    squash-merges, so a snapshot of work that landed still shows its whole diff
    against main forever — measured 2026-08-18, 150 of 151 did. That test would
    call almost everything stranded, which is exactly as useless as calling
    nothing stranded.
    """
    git("fetch", "origin", "+refs/civvis/wip/*:refs/civvis/wip/*")
    try:
        merged = merged_branches()
    except Exception:  # noqa: BLE001 - a report that cannot reach GitHub still reports
        merged = set()

    rows, landed = [], 0
    for line in git(
        "for-each-ref", "refs/civvis/wip/",
        "--format=%(refname)\t%(committerdate:iso-strict)\t%(contents:subject)",
    ).splitlines():
        ref, date, subject = line.split("\t", 2)
        branch = branch_of(ref)
        if branch in merged:
            landed += 1
            continue
        stat = git("diff", "--shortstat", f"{ref}^", ref).strip()
        rows.append(f"- `{branch}` ({date[:16]}, {stat or 'empty'}) — {subject[:70]}")
    if landed:
        rows.append(
            f"\n_{landed} further snapshot(s) belong to branches whose pull "
            f"request merged. That work is in `main`; the snapshots are "
            f"preserved history and are not listed._")
    return rows


def stuck_runs(now: datetime.datetime) -> list[str]:
    """Workflow runs GitHub still calls queued or in-progress after a day.

    Asked per status rather than by listing every run and filtering: the
    repository takes hundreds of runs a day and the two non-terminal states
    are a handful at any moment, so this is two small pages instead of a
    paged crawl of the whole history.

    ⚠ Report the run, never cancel it. A long `in_progress` run can be a real
    build somebody is waiting on; a report that acts on its own findings is
    how a queue tool becomes the outage.
    """
    rows = []
    for status in ("queued", "in_progress"):
        try:
            payload = api(f"/repos/{REPOSITORY}/actions/runs"
                          f"?status={status}&per_page=100")
        except Exception:  # noqa: BLE001 - a report that loses one API still reports
            continue
        for run in payload.get("workflow_runs", []):
            started = run.get("run_started_at") or run.get("created_at")
            if not started:
                continue
            age_hours = (now - datetime.datetime.fromisoformat(
                started.rstrip("Z"))).total_seconds() / 3600
            if age_hours < STUCK_RUN_HOURS:
                continue
            rows.append(
                f"- [{run.get('name') or 'run'} #{run['id']}]({run['html_url']}) "
                f"{status} {age_hours / 24:.1f}d on `{run.get('head_branch')}` "
                f"({run.get('event')}) — {(run.get('display_title') or '')[:70]}")
    return rows


def compose(now: datetime.datetime) -> tuple[str, bool]:
    """The report body, and whether any row actually demands a remedy."""
    sections = [
        ("Closed without a word", commentless_closes(now), True,
         "Add a closing comment stating why, or reopen and land it."),
        ("Idle branches holding real commits", idle_branches(now), True,
         "Open a PR, hand the branch off, or delete it with the reason in a "
         "closing comment on its claim PR."),
        ("Rescue snapshots (`refs/civvis/wip/*`)", rescue_refs(), False,
         "Land what was meant to land; the rest is preserved history and can "
         "stay."),
        ("Workflow runs that never ended", stuck_runs(now), True,
         "Check nothing is waiting on it, then `gh run cancel <id>`. Past "
         "GitHub's own 24-hour queue limit a run will not start on its own, "
         "and `timeout-minutes` never covered a run with no jobs."),
    ]
    parts = [
        f"_Generated {now.isoformat(timespec='minutes')}Z by "
        f"`tools/stranded_work_report.py`. One issue, updated in place; "
        f"its edit history is the timeline._",
    ]
    actionable = False
    for title, rows, demands_remedy, remedy in sections:
        actionable = actionable or (demands_remedy and bool(rows))
        parts.append(f"\n## {title}\n")
        parts.append("\n".join(rows) if rows else "Nothing stranded here today.")
        if rows:
            parts.append(f"\n_Remedy: {remedy}_")
    return "\n".join(parts), actionable


def upsert(body: str, actionable: bool) -> None:
    issues = api(f"/repos/{REPOSITORY}/issues?state=all"
                 f"&labels={ISSUE_LABEL}&per_page=10")
    existing = next((i for i in issues if i["title"] == ISSUE_TITLE), None)
    if existing is None:
        created = api(f"/repos/{REPOSITORY}/issues", "POST",
                      {"title": ISSUE_TITLE, "body": body,
                       "labels": [ISSUE_LABEL]})
        print(f"created issue #{created['number']}")
        return
    update: dict[str, str] = {"body": body}
    if actionable and existing["state"] == "closed":
        update["state"] = "open"
    api(f"/repos/{REPOSITORY}/issues/{existing['number']}", "PATCH", update)
    reopened = " and reopened it" if "state" in update else ""
    print(f"updated issue #{existing['number']}{reopened}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true",
                        help="print the report instead of writing the issue")
    args = parser.parse_args(argv)
    now = datetime.datetime.now(datetime.timezone.utc).replace(
        microsecond=0, tzinfo=None)
    body, actionable = compose(now)
    if args.dry_run:
        print(body)
        return 0
    upsert(body, actionable)
    return 0


if __name__ == "__main__":
    sys.exit(main())
