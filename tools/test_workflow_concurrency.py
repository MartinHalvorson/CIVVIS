#!/usr/bin/env python3
"""A commit on `main` is verified, not superseded by the commit after it.

GitHub Actions allows at most one run executing and at most one run *pending*
per concurrency group; a third arrival cancels the pending one, whatever
`cancel-in-progress` says. So a workflow that runs on pushes to `main` and keys
its group on `github.ref` — a constant for every one of those pushes — has one
queue slot for the whole trunk.

At this repository's velocity that is not a theoretical loss. Merges land about
five minutes apart and `cargo-test` takes seven to thirteen; on 2026-08-18 the
fourteen most recent `main` runs were **11 cancelled, 3 finished**. The
post-merge run is the entire safety argument for `strict = false` — a pull
request may merge without being up to date because `main` re-runs the full gate
on the real squash result — so the backstop against a semantic conflict between
two independently green PRs was running on about a fifth of commits.

This gate is structural and discovered rather than listed: it reads the
workflows, finds the ones that push to `main`, and fails any whose group cannot
distinguish one commit from another.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

WORKFLOWS = Path(__file__).resolve().parent.parent / ".github" / "workflows"

#: Anything that varies per commit. `github.sha` is the ordinary answer;
#: `github.run_id` would also be distinct, and is allowed rather than argued
#: with — the property under test is "not one group for the whole branch".
PER_COMMIT = ("github.sha", "github.run_id", "github.event.after")


def _block(source: str, header: str) -> str:
    """The indented block under a top-level `header:` key, or ''."""
    out, inside = [], False
    for line in source.splitlines():
        if line.startswith(f"{header}:"):
            inside = True
            continue
        if inside and line and not line[0].isspace():
            break
        if inside:
            out.append(line)
    return "\n".join(out)


def pushes_to_main(source: str) -> bool:
    trigger = _block(source, "on")
    if "push:" not in trigger:
        return False
    after = trigger.split("push:", 1)[1]
    return "main" in after.split("\n\n", 1)[0][:200]


def concurrency_group(source: str) -> str | None:
    block = _block(source, "concurrency")
    match = re.search(r"^\s*group:\s*(.+?)\s*$", block, re.MULTILINE)
    return match.group(1) if match else None


class EveryMainPushGetsItsOwnRun(unittest.TestCase):
    def test_the_workflows_are_discovered(self):
        """A hand-written list is complete the day it is written."""
        self.assertTrue(list(WORKFLOWS.glob("*.yml")),
                        f"no workflows found under {WORKFLOWS}")

    def test_a_workflow_that_gates_main_can_tell_two_commits_apart(self):
        offenders = []
        checked = []
        for path in sorted(WORKFLOWS.glob("*.yml")):
            source = path.read_text(encoding="utf-8")
            if not pushes_to_main(source):
                continue
            group = concurrency_group(source)
            if group is None:
                continue
            checked.append(path.name)
            if not any(token in group for token in PER_COMMIT):
                offenders.append(f"{path.name}: group: {group}")
        self.assertTrue(
            checked,
            "no workflow with a concurrency group runs on pushes to main; if "
            "that is now true this gate is obsolete, and if it is not the "
            "parser above stopped matching",
        )
        self.assertEqual(offenders, [], "\n".join(
            ["a workflow gates `main` but shares one concurrency group across "
             "every commit on it, so merges cancel each other's verification. "
             "Key the group on `github.sha` for pushes:"] + offenders))

    def test_pull_request_runs_are_still_keyed_on_the_pull_request(self):
        """The other half must not regress into per-sha PR groups: on a PR a
        newer push genuinely supersedes an older one and cancelling is savings.
        """
        for name in ("tests.yml", "quality.yml", "collaboration-policy.yml"):
            with self.subTest(workflow=name):
                group = concurrency_group((WORKFLOWS / name).read_text())
                self.assertIsNotNone(group, f"{name} lost its concurrency group")
                self.assertIn("github.event.pull_request.number", group)


if __name__ == "__main__":
    unittest.main()
