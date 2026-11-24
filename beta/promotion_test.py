#!/usr/bin/env python3
"""Test to prod on a clock: the promotion workflows' own steps, run for real.

Since 2026-11-23 (operator: "auto run the test to prod every 6 hrs for now")
`to-prod-auto-6h` moves the `site-stable` tag every six hours to the commit
`/test` is serving, and `to-test-auto-30`'s gate lets a half-hourly tick
publish a promotion whose own deploy never ran. Nobody watches either, and
neither runs in a pull request's checks: a workflow with a schedule runs on
main only, so a slip in one of these steps would first show as a front page
that moved when it should not have, or as a `/test` that stopped following
main.

So this takes the steps out of the workflow files as they ship and runs them
against a small repository: an origin holding a few commits and the tag, and
a clone standing for the runner's checkout. What is stood in for is what the
steps reach outside the repository: the two lanes' `build.json` (a `curl` on
the path that answers from two files), GitHub's API (a `gh` that reads and
moves the origin's tag), and, for the gate, the month's count of deployments.

    python3 beta/promotion_test.py
"""

from __future__ import annotations

import calendar
import datetime
import os
import pathlib
import re
import subprocess
import sys
import tempfile
import textwrap
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"
AUTO = WORKFLOWS / "to-prod-auto-6h.yml"
MANUAL = WORKFLOWS / "to-prod-manual-only.yml"
PUBLISH = WORKFLOWS / "to-test-auto-30.yml"

#: The lanes' `build.json`, answered from two files. A lane with no file is a
#: lane that cannot be reached, which is what `curl -f` says with exit 22.
CURL = textwrap.dedent(
    """\
    #!/bin/bash
    case "${@: -1}" in
      https://civvis.pages.dev/test/build.json) lane="$LANES/test.json" ;;
      https://civvis.pages.dev/build.json) lane="$LANES/root.json" ;;
      *) echo "curl: not a lane: ${@: -1}" >&2; exit 2 ;;
    esac
    [ -f "$lane" ] || exit 22
    cat "$lane"
    """
)

#: The two calls the steps make of GitHub's API, answered by the origin: which
#: commit the tag names, and moving it.
GH = textwrap.dedent(
    """\
    #!/bin/bash
    method=GET; path=""; sha=""
    while [ "$#" -gt 0 ]; do
      case "$1" in
        api) ;;
        -H|-F|--jq) shift ;;
        -X) shift; method="$1" ;;
        -f) shift; sha="${1#sha=}" ;;
        *) path="$1" ;;
      esac
      shift
    done
    case "$method $path" in
      "GET repos/"*"/commits/tags/site-stable")
        git -C "$ORIGIN" rev-parse --verify --quiet 'refs/tags/site-stable^{commit}' ;;
      "PATCH repos/"*"/git/refs/tags/site-stable")
        git -C "$ORIGIN" update-ref refs/tags/site-stable "$sha" ;;
      *) echo "gh: not a call these steps make: $method $path" >&2; exit 2 ;;
    esac
    """
)

#: The gate asks GitHub twice in Python read from standard input: how many
#: deployments the month has seen, and whether a change to the site is
#: waiting. Those two are answered from the environment (no count at all when
#: `USED` is empty); every other use of Python is the real one.
PYTHON = textwrap.dedent(
    """\
    #!/bin/bash
    if [ "$#" -eq 1 ] && [ "$1" = "-" ]; then
      script="$(cat)"
      case "$script" in
        *workflow_runs*) [ -n "${USED:-}" ] || exit 1; echo "$USED"; exit 0 ;;
        *"commits?sha="*) echo "${SITE_CHANGE:-}"; exit 0 ;;
      esac
      echo "python3: not a question the gate asks of GitHub" >&2; exit 2
    fi
    exec "$REAL_PYTHON" "$@"
    """
)


def step(workflow: pathlib.Path, name: str) -> str:
    """The script of the step called `name`, as the runner would be handed it."""
    lines = workflow.read_text(encoding="utf-8").splitlines()
    at = next(i for i, line in enumerate(lines) if line.strip() == f"- name: {name}")
    run = next(i for i in range(at + 1, len(lines)) if lines[i].strip() == "run: |")
    depth = len(lines[run]) - len(lines[run].lstrip())
    body = []
    for line in lines[run + 1:]:
        if line.strip() and len(line) - len(line.lstrip()) <= depth:
            break
        body.append(line)
    return textwrap.dedent("\n".join(body)) + "\n"


class Site:
    """An origin with commits a, b, c on main and s beside it, and a runner's clone."""

    def __init__(self, scratch: str):
        self.scratch = pathlib.Path(scratch)
        self.origin = self.scratch / "origin.git"
        self.clone = self.scratch / "clone"
        self.lanes = self.scratch / "lanes"
        self.fakes = self.scratch / "bin"
        self.lanes.mkdir()
        self.fakes.mkdir()
        for name, text in (("curl", CURL), ("gh", GH), ("python3", PYTHON)):
            (self.fakes / name).write_text(text, encoding="utf-8")
            (self.fakes / name).chmod(0o755)
        self.env = {
            **os.environ,
            "PATH": f"{self.fakes}{os.pathsep}{os.environ['PATH']}",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.invalid",
            "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@example.invalid",
            "LANES": str(self.lanes),
            "ORIGIN": str(self.origin),
            "REAL_PYTHON": sys.executable,
            "GITHUB_REPOSITORY": "someone/civvis",
            "GITHUB_OUTPUT": str(self.scratch / "output"),
            "GITHUB_STEP_SUMMARY": str(self.scratch / "summary"),
            "GH_TOKEN": "none",
        }
        work = self.scratch / "work"
        self.git("init", "-q", "--bare", str(self.origin), at=self.scratch)
        self.git("symbolic-ref", "HEAD", "refs/heads/main", at=self.origin)
        self.git("init", "-q", str(work), at=self.scratch)
        self.git("checkout", "-q", "-b", "main", at=work)
        self.commits = {}
        for name in "abc":
            self.commits[name] = self.commit(work, name)
        self.git("checkout", "-q", "-b", "beside", self.commits["a"], at=work)
        self.commits["s"] = self.commit(work, "s")
        self.git("push", "-q", str(self.origin), "main", "beside", at=work)
        self.git("clone", "-q", str(self.origin), str(self.clone), at=self.scratch)

    def git(self, *args: str, at: pathlib.Path) -> str:
        return subprocess.run(
            ["git", *args], cwd=at, env=self.env, check=True, capture_output=True, text=True
        ).stdout.strip()

    def commit(self, work: pathlib.Path, name: str) -> str:
        self.git("commit", "-q", "--allow-empty", "-m", name, at=work)
        return self.git("rev-parse", "HEAD", at=work)

    def state(self, *, tag: str | None, root: str | None, test: str | None) -> None:
        """Where the tag is and what each lane says it serves; None is absent."""
        if tag is None:
            self.git("update-ref", "-d", "refs/tags/site-stable", at=self.origin)
        else:
            self.git("update-ref", "refs/tags/site-stable", self.commits.get(tag, tag), at=self.origin)
        self.git("fetch", "-q", "--prune", "--prune-tags", "--tags", "--force", "origin", at=self.clone)
        for lane, commit in (("root", root), ("test", test)):
            path = self.lanes / f"{lane}.json"
            path.unlink(missing_ok=True)
            if commit is not None:
                path.write_text('{"commit": "%s"}' % self.commits.get(commit, commit), encoding="utf-8")

    def tag(self) -> str | None:
        found = subprocess.run(
            ["git", "rev-parse", "--verify", "--quiet", "refs/tags/site-stable^{commit}"],
            cwd=self.origin, env=self.env, capture_output=True, text=True,
        ).stdout.strip()
        return next((name for name, sha in self.commits.items() if sha == found), found or None)

    def run(self, script: str, **env: str) -> tuple[int, dict[str, str], str]:
        """Run a step as the runner does; its exit, its outputs and what it said."""
        output = pathlib.Path(self.env["GITHUB_OUTPUT"])
        output.write_text("", encoding="utf-8")
        path = self.scratch / "step.sh"
        path.write_text(script, encoding="utf-8")
        done = subprocess.run(
            ["bash", "-e", str(path)], cwd=self.clone, env={**self.env, **env},
            capture_output=True, text=True,
        )
        outputs = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
        return done.returncode, outputs, done.stdout + done.stderr


class TheTagStep(unittest.TestCase):
    """`to-prod-auto-6h` promotes what /test serves, forward, or nothing."""

    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.site = Site(scratch.name)
        self.script = step(AUTO, "Point site-stable at what /test serves")
        # Nothing for the runner to fill in: what runs here is what runs there.
        self.assertNotIn("${{", self.script)

    def promote(self, **state):
        self.site.state(**state)
        code, outputs, said = self.site.run(self.script)
        self.assertEqual(code, 0, said)
        return outputs, said

    def test_it_promotes_the_commit_test_serves_not_the_head_of_main(self):
        outputs, said = self.promote(tag="a", root="a", test="b")
        self.assertEqual(outputs, {"moved": "true", "sha": self.site.commits["b"], "back": self.site.commits["a"]})
        self.assertEqual(self.site.tag(), "b", said)

    def test_a_front_page_already_on_that_commit_deploys_nothing(self):
        outputs, said = self.promote(tag="b", root="b", test="b")
        self.assertEqual(outputs, {"moved": "false"})
        self.assertEqual(self.site.tag(), "b")
        self.assertIn("already serves", said)

    def test_a_promotion_that_never_deployed_is_deployed_again(self):
        # The tag is on b and `/` still serves a: the last deploy did not run.
        outputs, _ = self.promote(tag="b", root="a", test="b")
        self.assertEqual(outputs, {"moved": "true", "sha": self.site.commits["b"], "back": self.site.commits["a"]})
        self.assertEqual(self.site.tag(), "b")

    def test_it_only_moves_forward(self):
        outputs, said = self.promote(tag="b", root="b", test="a")
        self.assertEqual(outputs, {"moved": "false"})
        self.assertEqual(self.site.tag(), "b")
        self.assertIn("only moves forward", said)

    def test_a_tag_somebody_put_past_test_stays_where_it_was_put(self):
        outputs, said = self.promote(tag="c", root="a", test="b")
        self.assertEqual(outputs, {"moved": "false"})
        self.assertEqual(self.site.tag(), "c")
        self.assertIn("stays where it was put", said)

    def test_a_test_lane_that_is_not_on_main_is_not_promoted(self):
        outputs, said = self.promote(tag="a", root="a", test="s")
        self.assertEqual(outputs, {"moved": "false"})
        self.assertEqual(self.site.tag(), "a")
        self.assertIn("not on main", said)

    def test_not_knowing_what_a_lane_serves_moves_nothing(self):
        unknown = "0123456789abcdef0123456789abcdef01234567"
        for root, test in (("a", None), (None, "b"), ("a", "unknown"), ("a", unknown), (unknown, "b"), ("a", "main")):
            with self.subTest(root=root, test=test):
                outputs, said = self.promote(tag="a", root=root, test=test)
                self.assertEqual(outputs, {"moved": "false"})
                self.assertEqual(self.site.tag(), "a")
                self.assertIn("could not read", said)

    def test_the_first_promotion_needs_no_tag(self):
        outputs, _ = self.promote(tag=None, root="a", test="b")
        self.assertEqual(outputs["moved"], "true")
        self.assertEqual(self.site.tag(), "b")


class TheTagGoesBack(unittest.TestCase):
    """A failed deploy returns the tag to what `/` serves, and only its own move."""

    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.site = Site(scratch.name)
        self.script = step(AUTO, "Point site-stable back at what / serves")
        self.assertNotIn("${{", self.script)

    def test_the_tag_returns_to_the_commit_the_front_page_serves(self):
        self.site.state(tag="b", root="a", test="b")
        code, _, said = self.site.run(self.script, SHA=self.site.commits["b"], BACK=self.site.commits["a"])
        self.assertEqual(code, 0, said)
        self.assertEqual(self.site.tag(), "a")

    def test_a_tag_moved_since_is_somebodys_choice(self):
        self.site.state(tag="c", root="a", test="b")
        code, _, said = self.site.run(self.script, SHA=self.site.commits["b"], BACK=self.site.commits["a"])
        self.assertEqual(code, 0, said)
        self.assertEqual(self.site.tag(), "c")
        self.assertIn("left alone", said)

    def test_only_a_failure_puts_it_back(self):
        # A cancelled deploy leaves the tag for the next scheduled run, which
        # the gate below lets through; putting it back would undo a promotion
        # every time a tick took its place in the queue.
        text = AUTO.read_text(encoding="utf-8")
        self.assertIn("needs.deploy.result == 'failure'", text)
        self.assertNotIn("needs.deploy.result != 'success'", text)


class TheGate(unittest.TestCase):
    """A half-hourly tick publishes a promotion that is waiting, inside the month."""

    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.site = Site(scratch.name)
        self.script = step(PUBLISH, "Is a scheduled deploy worth it?")

    def gate(self, *, event="schedule", promotion="", used="0", **state):
        script = self.script.replace("${{ github.event_name }}", event).replace("${{ inputs.promotion }}", promotion)
        self.assertNotIn("${{", script)
        self.site.state(**state)
        code, outputs, said = self.site.run(script, GITHUB_SHA=self.site.commits["c"], USED=used)
        self.assertEqual(code, 0, said)
        return outputs["go"], said

    def lines(self) -> tuple[int, int]:
        """The pace and the second line as the gate draws them at this moment."""
        now = datetime.datetime.now(datetime.timezone.utc)
        month = calendar.monthrange(now.year, now.month)[1] * 86400
        elapsed = (now.day - 1) * 86400 + now.hour * 3600 + now.minute * 60 + now.second
        return 1 + 320 * elapsed // month, 20 + 470 * elapsed // month

    def test_nothing_new_and_nothing_promoted_is_nothing_to_do(self):
        go, said = self.gate(tag="a", root="a", test="c")
        self.assertEqual(go, "false")
        self.assertIn("nothing to do", said)

    def test_a_waiting_promotion_is_something_to_do(self):
        # /test already serves the head of main, so before this the tick
        # stopped here and the promotion waited for the next merge.
        go, said = self.gate(tag="b", root="a", test="c")
        self.assertEqual(go, "true")
        self.assertIn("a promotion is waiting", said)

    def test_a_waiting_promotion_goes_out_past_the_pace(self):
        pace, second = self.lines()
        between = str((pace + second) // 2)
        go, said = self.gate(tag="b", root="a", test="c", used=between)
        self.assertEqual(go, "true", said)
        # The same count with no promotion waiting, and main ahead of /test,
        # waits: it is the promotion that let the run through.
        go, said = self.gate(tag="a", root="a", test="b", used=between)
        self.assertEqual(go, "false", said)

    def test_a_waiting_promotion_still_answers_to_the_month(self):
        go, said = self.gate(tag="b", root="a", test="c", used="100000")
        self.assertEqual(go, "false")
        self.assertIn("ahead of the monthly budget", said)

    def test_no_count_of_the_month_is_still_no_licence(self):
        go, _ = self.gate(tag="b", root="a", test="c", used="")
        self.assertEqual(go, "false")

    def test_a_tag_or_a_front_page_that_cannot_be_read_is_nothing_waiting(self):
        go, _ = self.gate(tag=None, root="a", test="c")
        self.assertEqual(go, "false")
        go, _ = self.gate(tag="b", root=None, test="c")
        self.assertEqual(go, "false")

    def test_a_promotion_calling_in_is_not_gated(self):
        # to-prod-auto-6h's runs are scheduled ones. Without its say-so the
        # gate would take one for a tick and could stop it at the budget.
        go, said = self.gate(promotion="true", tag="b", root="a", test="c", used="100000")
        self.assertEqual(go, "true")
        self.assertIn("no gate", said)
        go, _ = self.gate(event="workflow_dispatch", tag="a", root="a", test="c", used="100000")
        self.assertEqual(go, "true")


class TheWiring(unittest.TestCase):
    def test_every_workflow_that_deploys_is_counted_against_the_month(self):
        # Discovered, not listed: a workflow that calls the publish job in
        # spends from Cloudflare's 500, and the gate's sum has to know it.
        text = PUBLISH.read_text(encoding="utf-8")
        counted = set(re.findall(r'"([\w.-]+\.yml)"', text[text.index("for workflow in ("):].split("):", 1)[0]))
        callers = {
            path.name for path in WORKFLOWS.glob("*.yml")
            if "uses: ./.github/workflows/to-test-auto-30.yml" in path.read_text(encoding="utf-8")
        }
        self.assertEqual(callers, {AUTO.name, MANUAL.name})
        self.assertLessEqual(callers | {PUBLISH.name}, counted)

    def test_the_two_promotions_take_turns(self):
        groups = [
            re.search(r"^concurrency:\n(?:\s*#.*\n)*\s+group: (\S+)$", path.read_text(encoding="utf-8"), re.M).group(1)
            for path in (AUTO, MANUAL)
        ]
        self.assertEqual(groups[0], groups[1])

    def test_it_runs_every_six_hours_and_says_it_has_decided(self):
        text = AUTO.read_text(encoding="utf-8")
        self.assertEqual(re.findall(r'^\s+- cron: "(.+)"$', text, re.M), ["12 */6 * * *"])
        self.assertRegex(text, r"uses: \./\.github/workflows/to-test-auto-30\.yml\n(?:.*\n)*?\s+promotion: true")
        self.assertIn("promotion:\n        type: boolean\n        default: false", PUBLISH.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
