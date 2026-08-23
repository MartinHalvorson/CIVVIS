#!/usr/bin/env python3
"""Prove overwrite_guard's verdicts on a purpose-built history.

The fixture repo re-creates the failure this guard exists for, in miniature:
an old file, a young feature landed on main, and a branch that deletes the
young lines. Committer dates are pinned so the test controls "young" exactly
and never depends on the wall clock.

`WaiverMatchingTests` covers the second failure, the one in the guard itself:
`WAIVER in body` matched the marker as a bare substring, so a pull request that
merely *wrote about* the hatch switched the gate off. Every body in that class
contains the marker, so the old matcher waived every one of them; only three
are decisions. `MergeBaseTests` covers the third: `--base origin/main` used to
mean a two-dot diff, which charges a branch with everything `main` gained after
it was cut.
"""

from __future__ import annotations

import contextlib
import io
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import overwrite_guard

NOW = 1_800_000_000
OLD = NOW - 30 * 86400
YOUNG = NOW - 2 * 86400


def run(repo: pathlib.Path, *args: str, date: int | None = None) -> str:
    env = dict(os.environ)
    if date is not None:
        stamp = f"{date} +0000"
        env["GIT_AUTHOR_DATE"] = stamp
        env["GIT_COMMITTER_DATE"] = stamp
    result = subprocess.run(
        ["git", "-C", str(repo), *args], capture_output=True, text=True,
        env=env, check=True,
    )
    return result.stdout.strip()


class OverwriteGuardTests(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory(prefix="civvis-guard-")
        self.repo = pathlib.Path(self.dir.name)
        run(self.repo, "init", "-q", "-b", "main")
        run(self.repo, "config", "user.email", "guard@test")
        run(self.repo, "config", "user.name", "guard")
        (self.repo / "engine.txt").write_text(
            "\n".join(f"engine line {i}" for i in range(40)) + "\n")
        run(self.repo, "add", "engine.txt")
        run(self.repo, "commit", "-q", "-m", "Ancient foundation", date=OLD)
        feature = "\n".join(f"lens panel line {i}" for i in range(30)) + "\n"
        (self.repo / "panel.txt").write_text(feature)
        run(self.repo, "add", "panel.txt")
        run(self.repo, "commit", "-q", "-m", "Add the lens panel (#1109)",
            date=YOUNG)
        self.base = run(self.repo, "rev-parse", "HEAD")
        self.addCleanup(self.dir.cleanup)

    def branch_deleting(self, path: str, keep: int, message: str) -> str:
        run(self.repo, "checkout", "-q", "-b", f"topic-{keep}-{path}", self.base)
        lines = (self.repo / path).read_text().splitlines(keepends=True)
        (self.repo / path).write_text("".join(lines[:keep]))
        run(self.repo, "add", path)
        run(self.repo, "commit", "-q", "-m", message, date=NOW)
        return run(self.repo, "rev-parse", "HEAD")

    def verdict(self, head: str, body: str = "") -> int:
        body_file = self.repo / "body.txt"
        body_file.write_text(body)
        cwd = os.getcwd()
        os.chdir(self.repo)
        try:
            return overwrite_guard.main([
                "--base", self.base, "--head", head,
                "--body-file", str(body_file), "--now", str(NOW),
            ])
        finally:
            os.chdir(cwd)

    def test_deleting_young_work_unacknowledged_fails(self):
        head = self.branch_deleting("panel.txt", 0, "Redesign the panel")
        self.assertEqual(self.verdict(head), 1)

    def test_naming_the_victim_pr_passes(self):
        head = self.branch_deleting("panel.txt", 0, "Redesign the panel")
        self.assertEqual(self.verdict(head, "Supersedes: #1109"), 0)

    def test_the_waiver_with_a_reason_passes(self):
        head = self.branch_deleting("panel.txt", 0, "Bulk rewrite")
        self.assertEqual(self.verdict(
            head, "overwrite-guard: allow the panel is regenerated wholesale"), 0)

    def test_a_bare_waiver_is_not_a_waiver(self):
        """The reason is the half that makes the hatch a decision.

        This is the one behaviour change a legitimate user can feel: before,
        the four words alone passed. #2059 is the standing example of a change
        that shipped because nobody had to write the sentence.
        """
        head = self.branch_deleting("panel.txt", 0, "Bulk rewrite")
        self.assertEqual(self.verdict(head, "overwrite-guard: allow"), 1)

    def test_a_body_that_only_discusses_the_waiver_still_reports_its_victims(self):
        """End to end, on a real deletion: the hole this change closes.

        The body below is what a pull request *about* the guard looks like. The
        old matcher read it as a waiver and printed "waived"; the victim is
        supposed to be reported.
        """
        head = self.branch_deleting("panel.txt", 0, "Redesign the panel")
        body = ("This PR documents the hatch: a body carrying "
                "`overwrite-guard: allow` waives the whole check.\n")
        self.assertEqual(self.verdict(head, body), 1)

    def test_deleting_old_lines_is_ordinary_maintenance(self):
        head = self.branch_deleting("engine.txt", 0, "Retire the foundation")
        self.assertEqual(self.verdict(head), 0)

    def test_small_adjacent_churn_stays_quiet(self):
        head = self.branch_deleting("panel.txt", 24, "Tweak six panel lines")
        self.assertEqual(self.verdict(head), 0)

    def test_a_rename_is_not_a_deletion(self):
        run(self.repo, "checkout", "-q", "-b", "rename", self.base)
        run(self.repo, "mv", "panel.txt", "panel_moved.txt")
        run(self.repo, "commit", "-q", "-m", "Move the panel", date=NOW)
        head = run(self.repo, "rev-parse", "HEAD")
        self.assertEqual(self.verdict(head), 0)


#: The paragraph from #2328's pull request body that found this defect, quoted
#: as published except for one restoration: the author had written the marker
#: out in full — `overwrite-guard: allow` — where the published text says
#: "`overwrite_guard.py` offers" and "`overwrite_guard.py` matches". They ran
#: the guard locally, saw it report "waived" where they expected victims,
#: worked out why, and reworded the sentence before the body was ever pushed.
#: Restoring the phrase restores the exact text that defeated the old matcher,
#: which is the text this fixture exists to refuse.
PR_2328_PARAGRAPH = (
    "3. **An intended cost can be accepted.** A promoted feature is a "
    "performance event by definition, so this gate *will* fire on honest "
    "changes. A `paired-cost: allow <reason>` line in the PR body passes the "
    "run, the same shape of hatch `overwrite-guard: allow` offers. "
    "\u26a0 Deliberately **line-anchored and reason-bearing**, unlike that "
    "one: `overwrite-guard: allow` matches its waiver as a bare substring, so "
    "a PR body that merely *discusses* the marker waives the gate \u2014 this "
    "body did, on the first local run, until it was reworded.\n"
)


class WaiverMatchingTests(unittest.TestCase):
    """Writing about the switch must not flip it.

    Every fixture here contains the exact marker, so `WAIVER in body` waived
    all of them. `assert_mentions_but_does_not_waive` asserts the marker really
    is present before asserting the refusal, because a fixture that quietly
    stopped containing it would pass while proving nothing.
    """

    def assert_mentions_but_does_not_waive(self, body: str):
        self.assertIn(overwrite_guard.WAIVER_MARKER.lower(), body.lower(),
                      "fixture no longer carries the marker, so it proves "
                      "nothing about the matcher")
        self.assertIsNone(overwrite_guard.waiver_reason(body))

    def test_the_paragraph_that_found_the_defect_does_not_waive(self):
        self.assert_mentions_but_does_not_waive(PR_2328_PARAGRAPH)

    def test_mid_sentence_prose_does_not_waive(self):
        self.assert_mentions_but_does_not_waive(
            "The escape hatch is overwrite-guard: allow, and it is grep-able.")

    def test_inline_backticks_do_not_waive(self):
        self.assert_mentions_but_does_not_waive(
            "`overwrite-guard: allow` in the body waives the whole check.\n")

    def test_an_indented_code_block_does_not_waive(self):
        self.assert_mentions_but_does_not_waive(
            "Add a line reading:\n\n    overwrite-guard: allow <reason>\n")

    def test_a_blockquote_does_not_waive(self):
        self.assert_mentions_but_does_not_waive(
            "The docstring says:\n\n> overwrite-guard: allow bulk rewrite\n")

    def test_a_fenced_code_block_does_not_waive(self):
        self.assert_mentions_but_does_not_waive(
            "Add:\n\n```\noverwrite-guard: allow bulk rewrite\n```\n")

    def test_a_tilde_fence_does_not_waive(self):
        self.assert_mentions_but_does_not_waive(
            "Add:\n\n~~~text\noverwrite-guard: allow bulk rewrite\n~~~\n")

    def test_an_html_comment_does_not_waive(self):
        self.assert_mentions_but_does_not_waive(
            "<!-- overwrite-guard: allow left in the template -->\n")

    def test_the_tools_own_docstring_does_not_waive(self):
        """The guard documents its hatch; quoting the guard is not using it.

        Discovered rather than written out: whatever shape the docstring shows
        the waiver in, pasting the docstring into a body must not waive.
        """
        self.assert_mentions_but_does_not_waive(overwrite_guard.__doc__)

    def test_a_bare_marker_is_not_a_waiver(self):
        self.assert_mentions_but_does_not_waive("overwrite-guard: allow\n")

    def test_whitespace_is_not_a_reason(self):
        self.assert_mentions_but_does_not_waive("overwrite-guard: allow   \t\n")

    def test_a_line_of_its_own_with_a_reason_waives(self):
        self.assertEqual(
            overwrite_guard.waiver_reason(
                "## What changed\n\noverwrite-guard: allow the lens panel is "
                "regenerated wholesale from data/lens.json\n"),
            "the lens panel is regenerated wholesale from data/lens.json")

    def test_a_list_bullet_waives_because_the_ownership_block_is_a_list(self):
        self.assertEqual(
            overwrite_guard.waiver_reason(
                "- Supersedes: #1109\n- overwrite-guard: allow whole-file "
                "regeneration\n"),
            "whole-file regeneration")

    def test_the_marker_is_case_insensitive(self):
        self.assertEqual(
            overwrite_guard.waiver_reason("Overwrite-Guard: ALLOW generated\n"),
            "generated")

    def test_carriage_returns_do_not_defeat_the_anchor(self):
        self.assertEqual(
            overwrite_guard.waiver_reason("intro\r\noverwrite-guard: allow "
                                          "generated output\r\n"),
            "generated output")

    def test_a_bare_marker_is_told_why_it_did_not_waive(self):
        note = overwrite_guard.waiver_note("overwrite-guard: allow\n")
        self.assertIn("carries no reason", note)

    def test_a_discussion_is_told_why_it_did_not_waive(self):
        note = overwrite_guard.waiver_note(PR_2328_PARAGRAPH)
        self.assertIn("not as a line of its own", note)

    def test_a_body_without_the_marker_is_not_lectured(self):
        self.assertIsNone(overwrite_guard.waiver_note("Supersedes: #1109\n"))


class AcknowledgementBoundaryTests(unittest.TestCase):
    """A victim's number is matched anywhere, but as a number.

    Naming the work you replace stays deliberately loose — `Supersedes:`,
    `Coordinated with:` and a sentence of prose are all honest. The number
    itself needs an edge: without one, victim #1 read any mention of #1109 as
    its own acknowledgement.
    """

    SHA = "0" * 40

    def test_a_longer_number_does_not_acknowledge_a_shorter_one(self):
        self.assertFalse(overwrite_guard.acknowledged(
            "Coordinated with: #1109", self.SHA, "Land the thing (#1)"))

    def test_the_victims_own_number_acknowledges_it(self):
        self.assertTrue(overwrite_guard.acknowledged(
            "Supersedes: #1109", self.SHA, "Add the lens panel (#1109)"))

    def test_punctuation_after_the_number_still_acknowledges(self):
        self.assertTrue(overwrite_guard.acknowledged(
            "This replaces #1109, deliberately.", self.SHA,
            "Add the lens panel (#1109)"))


class MergeBaseTests(unittest.TestCase):
    """`--base origin/main` must not charge the branch with main's own commits.

    Two-dot `git diff main HEAD` calls every line `main` gained after the fork
    a deletion by the branch. That is what told #2328's author their pull
    request was "deleting 2036 lines from #2335": a phantom, produced entirely
    by the trunk moving. CI never saw it because the workflow passes a merge
    base already — the local path was the broken one, and the local path is the
    one that would have shown the waiver hole a day earlier.
    """

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory(prefix="civvis-guard-mb-")
        self.repo = pathlib.Path(self.dir.name)
        run(self.repo, "init", "-q", "-b", "main")
        run(self.repo, "config", "user.email", "guard@test")
        run(self.repo, "config", "user.name", "guard")
        (self.repo / "engine.txt").write_text(
            "\n".join(f"engine line {i}" for i in range(40)) + "\n")
        run(self.repo, "add", "engine.txt")
        run(self.repo, "commit", "-q", "-m", "Ancient foundation", date=OLD)
        self.fork = run(self.repo, "rev-parse", "HEAD")

        # The branch: it adds a file and deletes nothing at all.
        run(self.repo, "checkout", "-q", "-b", "topic", self.fork)
        (self.repo / "topic.txt").write_text("topic line\n")
        run(self.repo, "add", "topic.txt")
        run(self.repo, "commit", "-q", "-m", "Add a topic file", date=NOW)
        self.head = run(self.repo, "rev-parse", "HEAD")

        # main moves on afterwards, with young work the branch never saw.
        run(self.repo, "checkout", "-q", "main")
        (self.repo / "wing.txt").write_text(
            "\n".join(f"wing line {i}" for i in range(40)) + "\n")
        run(self.repo, "add", "wing.txt")
        run(self.repo, "commit", "-q", "-m", "Land the wing (#2335)", date=YOUNG)

        # A history the head shares nothing with, for the refusal below.
        run(self.repo, "checkout", "-q", "--orphan", "alien")
        run(self.repo, "rm", "-q", "-rf", ".")
        (self.repo / "alien.txt").write_text("alien\n")
        run(self.repo, "add", "alien.txt")
        run(self.repo, "commit", "-q", "-m", "Unrelated history", date=OLD)
        run(self.repo, "checkout", "-q", "main")
        self.addCleanup(self.dir.cleanup)

    def in_repo(self, call):
        cwd = os.getcwd()
        os.chdir(self.repo)
        try:
            return call()
        finally:
            os.chdir(cwd)

    def test_the_two_dot_diff_really_does_invent_the_deletion(self):
        """The phantom is real, so the fix is not guarding against nothing."""
        phantom = self.in_repo(
            lambda: overwrite_guard.deleted_ranges("main", self.head))
        self.assertIn("wing.txt", phantom)
        self.assertEqual(sum(end - start + 1 for start, end in phantom["wing.txt"]),
                         40)

    def test_the_merge_base_sees_no_deletion(self):
        real = self.in_repo(
            lambda: overwrite_guard.deleted_ranges(self.fork, self.head))
        self.assertEqual(real, {})

    def test_a_moving_branch_tip_is_safe_as_base(self):
        body = self.repo / "body.txt"
        body.write_text("")
        verdict = self.in_repo(lambda: overwrite_guard.main([
            "--base", "main", "--head", self.head,
            "--body-file", str(body), "--now", str(NOW),
        ]))
        self.assertEqual(verdict, 0)

    def test_a_merge_base_passed_directly_is_left_alone(self):
        """What CI passes. The merge base of a merge base and the head is it."""
        self.assertEqual(
            self.in_repo(lambda: overwrite_guard.merge_base(self.fork, self.head)),
            self.fork)

    def test_unrelated_histories_fall_back_to_the_base_as_given(self):
        self.assertEqual(
            self.in_repo(lambda: overwrite_guard.merge_base("main", "nope")),
            "main")

    def test_the_base_it_judged_against_is_printed_every_run(self):
        """#2328's docstring already said `--base <merge-base>` and was right.

        Its author passed `origin/main` anyway and was told they had deleted
        2036 lines from a pull request they had never touched. A tool that
        names the commit it judged against makes that visible in the first
        line of output instead of nowhere.
        """
        body = self.repo / "body.txt"
        body.write_text("")
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.in_repo(lambda: overwrite_guard.main([
                "--base", "main", "--head", self.head,
                "--body-file", str(body), "--now", str(NOW),
            ]))
        self.assertIn(self.fork[:9], out.getvalue())
        self.assertIn("merge base", out.getvalue())

    def test_a_base_the_head_does_not_descend_from_is_refused(self):
        """Neither 0 nor 1: there is no honest verdict, so it says so.

        Without this the fallback silently blames every line of an unrelated
        history on the branch — the phantom again, just with no merge base to
        rescue it.
        """
        body = self.repo / "body.txt"
        body.write_text("")
        verdict = self.in_repo(lambda: overwrite_guard.main([
            "--base", "alien", "--head", self.head,
            "--body-file", str(body), "--now", str(NOW),
        ]))
        self.assertEqual(verdict, 2)


class OneIdiomTests(unittest.TestCase):
    """The repository gets one waiver idiom, and that is a check, not a claim.

    `tools/speed_ab.py` spells the same hatch as `paired-cost: allow <reason>`.
    Two hand-maintained copies of a security-shaped pattern drift, and the way
    they drift is one of them getting looser — which is the whole defect this
    file exists for. The duplication itself is forced: `overwrite-guard.yml`
    copies `overwrite_guard.py` alone to `/tmp` and runs it there, so it cannot
    import from the repository. Comparing them here is the cheap alternative.
    """

    def setUp(self):
        import speed_ab
        self.speed_ab = speed_ab

    def test_the_two_gates_spell_their_hatch_the_same_way(self):
        self.assertEqual(
            overwrite_guard.WAIVER.pattern.replace("overwrite-guard", "MARKER"),
            self.speed_ab.ACKNOWLEDGEMENT.pattern.replace("paired-cost", "MARKER"))

    def test_the_two_gates_match_under_the_same_flags(self):
        self.assertEqual(overwrite_guard.WAIVER.flags,
                         self.speed_ab.ACKNOWLEDGEMENT.flags)

    def test_both_blank_fenced_blocks_the_same_way(self):
        body = "intro\n```\nhidden\n```\ntail\n"
        self.assertEqual(overwrite_guard.prose(body), self.speed_ab.prose(body))


if __name__ == "__main__":
    unittest.main()
