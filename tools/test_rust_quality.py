import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


MODULE_PATH = Path(__file__).with_name("rust_quality.py")
SPEC = importlib.util.spec_from_file_location("rust_quality", MODULE_PATH)
assert SPEC and SPEC.loader
quality = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(quality)


class QualityHelpersTests(unittest.TestCase):
    def test_changed_rust_files_ignores_deleted_and_non_rust_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            (repo / "src").mkdir()
            (repo / "src/main.rs").write_text("fn main() {}\n")
            (repo / "README.md").write_text("docs\n")
            completed = quality.subprocess.CompletedProcess(
                [], 0, "src/main.rs\nREADME.md\nold.rs\n", ""
            )
            with patch.object(quality, "run", return_value=completed):
                self.assertEqual(
                    quality.changed_rust_files(repo, "base", "head"),
                    [Path("src/main.rs")],
                )

    def test_span_paths_normalizes_relative_and_absolute_diagnostics(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            message = {
                "spans": [
                    {"file_name": "src/lib.rs"},
                    {"file_name": str(repo / "src/main.rs")},
                ]
            }
            self.assertEqual(
                quality._span_paths(message, repo),
                {(repo / "src/lib.rs").resolve(), (repo / "src/main.rs").resolve()},
            )

    def test_clippy_only_reports_warnings_on_changed_lines(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            on_changed_lines = {
                "reason": "compiler-message",
                "message": {
                    "level": "warning",
                    "code": {"code": "clippy::needless_return"},
                    "message": "remove the return",
                    "rendered": "warning: remove the return",
                    "spans": [
                        {"file_name": "src/lib.rs", "line_start": 5, "line_end": 6}
                    ],
                },
            }
            standing_debt_same_file = {
                **on_changed_lines,
                "message": {
                    **on_changed_lines["message"],
                    "code": {"code": "clippy::type_complexity"},
                    "spans": [
                        {"file_name": "src/lib.rs", "line_start": 400, "line_end": 401}
                    ],
                },
            }
            unrelated_file = {
                **on_changed_lines,
                "message": {
                    **on_changed_lines["message"],
                    "spans": [
                        {"file_name": "src/other.rs", "line_start": 5, "line_end": 6}
                    ],
                },
            }
            completed = quality.subprocess.CompletedProcess(
                [],
                0,
                "\n".join(
                    json.dumps(payload)
                    for payload in [
                        on_changed_lines,
                        standing_debt_same_file,
                        unrelated_file,
                    ]
                ),
                "",
            )
            ranges = {(repo / "src/lib.rs").resolve(): [(4, 8)]}
            with patch.object(quality, "run", return_value=completed):
                diagnostics, raw = quality.clippy(repo, ranges)
            self.assertEqual(len(diagnostics), 1)
            self.assertIn("needless_return", diagnostics[0])
            self.assertEqual(raw, "")

    def test_changed_line_ranges_parses_unified_zero_hunks(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            diff = (
                "diff --git a/src/lib.rs b/src/lib.rs\n"
                "--- a/src/lib.rs\n"
                "+++ b/src/lib.rs\n"
                "@@ -10,2 +12,3 @@ fn context()\n"
                "+a\n+b\n+c\n"
                "@@ -40 +44,0 @@ fn gone()\n"
                "-d\n"
                "@@ -50 +53 @@ fn one()\n"
                "-e\n+f\n"
            )
            completed = quality.subprocess.CompletedProcess([], 0, diff, "")
            with patch.object(quality, "run", return_value=completed):
                ranges = quality.changed_line_ranges(repo, "base", "head")
            key = (repo / "src/lib.rs").resolve()
            # The pure deletion at old line 40 leaves no head-side range.
            self.assertEqual(ranges, {key: [(12, 14), (53, 53)]})

    def test_merge_base_replaces_a_moving_branch_tip(self):
        ok = lambda text: quality.subprocess.CompletedProcess([], 0, text, "")
        fail = quality.subprocess.CompletedProcess([], 128, "", "fatal")
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)

            # A pull_request head is GitHub's test merge commit; its first
            # parent is the main the merge was built against.
            def merge_head(repo, args, **kwargs):
                if args[-1] == "head^2":
                    return ok("branchhead\n")
                if args[-1] == "head^1":
                    return ok("mainparent\n")
                return fail

            with patch.object(quality, "run", side_effect=merge_head):
                self.assertEqual(quality.merge_base(repo, "tip", "head"), "mainparent")

            # A push head has one parent: fall back to the true fork point.
            def push_head(repo, args, **kwargs):
                if args[-1] == "head^2":
                    return fail
                if args[:2] == ["git", "merge-base"]:
                    return ok("fork\n")
                return fail

            with patch.object(quality, "run", side_effect=push_head):
                self.assertEqual(quality.merge_base(repo, "tip", "head"), "fork")

            # A clone that can answer neither keeps the supplied base.
            with patch.object(quality, "run", return_value=fail):
                self.assertEqual(quality.merge_base(repo, "tip", "head"), "tip")

    def test_merge_base_finds_main_whichever_parent_it_is(self):
        """⚠⚠ The parent order flips between the CI and the local merge shape.

        GitHub's pull_request head merges the branch INTO main, so `^1` is main.
        A developer clearing a conflict with `git merge origin/main` on their own
        branch produces the reverse, and reading `^1` there compares the merge
        against the branch's own pre-merge tip — the diff holds only the conflict
        resolution and the gate passes having checked nothing. That is a false
        GREEN, so this is built on a real repository rather than mocks: the bug
        it guards lived under mocks that asserted the CI shape alone.
        """
        import subprocess

        def git(repo, *args):
            subprocess.run(
                ["git", "-c", "user.email=t@t", "-c", "user.name=t", *args],
                cwd=repo, check=True, capture_output=True, text=True,
            )

        def sha(repo, rev):
            return subprocess.run(
                ["git", "rev-parse", rev], cwd=repo, check=True,
                capture_output=True, text=True,
            ).stdout.strip()

        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            git(repo, "init", "-q", "-b", "main")
            (repo / "a.txt").write_text("base\n")
            git(repo, "add", "a.txt")
            git(repo, "commit", "-qm", "base")

            git(repo, "checkout", "-q", "-b", "feature")
            (repo / "b.txt").write_text("branch work\n")
            git(repo, "add", "b.txt")
            git(repo, "commit", "-qm", "branch work")
            branch_tip = sha(repo, "HEAD")

            git(repo, "checkout", "-q", "main")
            (repo / "c.txt").write_text("someone else\n")
            git(repo, "add", "c.txt")
            git(repo, "commit", "-qm", "someone else")
            main_tip = sha(repo, "HEAD")
            git(repo, "update-ref", "refs/remotes/origin/main", main_tip)

            # The LOCAL shape: main merged into the branch, so `^1` is the
            # branch. This is the one that used to pass having checked nothing.
            git(repo, "checkout", "-q", "feature")
            git(repo, "merge", "-q", "--no-ff", "-m", "merge main", "main")
            local_merge = sha(repo, "HEAD")
            self.assertEqual(sha(repo, "HEAD^1"), branch_tip, "^1 is the branch")
            self.assertEqual(sha(repo, "HEAD^2"), main_tip, "^2 is main")
            self.assertEqual(
                quality.merge_base(repo, branch_tip, local_merge), main_tip,
                "a local `git merge origin/main` must still be read against main",
            )

            # The CI shape: the branch merged into main, so `^1` is main. The
            # answer is the same commit, reached through the other parent.
            git(repo, "checkout", "-q", "main")
            git(repo, "merge", "-q", "--no-ff", "-m", "test merge", branch_tip)
            ci_merge = sha(repo, "HEAD")
            self.assertEqual(sha(repo, "HEAD^1"), main_tip, "^1 is main")
            self.assertEqual(
                quality.merge_base(repo, main_tip, ci_merge), main_tip,
                "GitHub's test merge is still read against the main it used",
            )

    def test_rustfmt_abort_is_a_skip_not_a_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            killed = quality.subprocess.CompletedProcess(
                [], -9, "", "memory allocation of 23788898244 bytes failed"
            )
            with patch.object(quality, "run", return_value=killed):
                failures, skipped = quality.rustfmt(repo, [Path("src/game.rs")], {})
            self.assertEqual(failures, [])
            self.assertEqual(len(skipped), 1)
            self.assertIn("too large", skipped[0])

    def test_rustfmt_scopes_an_out_of_line_module_to_its_own_file(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            (repo / "src/ai").mkdir(parents=True)
            completed = quality.subprocess.CompletedProcess([], 0, "", "")
            with patch.object(quality, "run", return_value=completed) as run:
                failures, skipped = quality.rustfmt(repo, [Path("src/ai.rs")], {})
            self.assertEqual((failures, skipped), ([], []))
            self.assertEqual(
                run.call_args.args[1],
                [
                    "rustfmt",
                    "--check",
                    "--edition",
                    "2021",
                    "--config",
                    "skip_children=true",
                    "src/ai.rs",
                ],
            )

    def test_rustfmt_reports_only_chunks_overlapping_the_change(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            resolved = (repo / "src/lib.rs").resolve()
            output = (
                f"Diff in {resolved}:5:\n"
                " context\n-old\n+new\n"
                f"Diff in {resolved}:200:\n"
                " context\n-stale\n+debt\n"
            )
            completed = quality.subprocess.CompletedProcess([], 1, output, "")
            ranges = {resolved: [(6, 6)]}
            with patch.object(quality, "run", return_value=completed):
                failures, skipped = quality.rustfmt(repo, [Path("src/lib.rs")], ranges)
            self.assertEqual(skipped, [])
            self.assertEqual(len(failures), 1)
            self.assertIn(":5:", failures[0])
            self.assertNotIn(":200:", failures[0])

    def test_rustfmt_debt_beside_the_change_is_not_dragged_in(self):
        # The chunk's context lines brush the changed range, but the lines
        # rustfmt wants to rewrite sit above it — standing debt, not new debt.
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            resolved = (repo / "src/lib.rs").resolve()
            output = (
                f"Diff in {resolved}:5:\n"
                " untouched context\n untouched context\n-stale\n+debt\n"
            )
            completed = quality.subprocess.CompletedProcess([], 1, output, "")
            ranges = {resolved: [(9, 12)]}
            with patch.object(quality, "run", return_value=completed):
                failures, skipped = quality.rustfmt(repo, [Path("src/lib.rs")], ranges)
            self.assertEqual((failures, skipped), ([], []))

    def test_rustfmt_standing_debt_alone_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            resolved = (repo / "src/lib.rs").resolve()
            output = f"Diff in {resolved}:200:\n context\n-stale\n+debt\n"
            completed = quality.subprocess.CompletedProcess([], 1, output, "")
            ranges = {resolved: [(6, 6)]}
            with patch.object(quality, "run", return_value=completed):
                failures, skipped = quality.rustfmt(repo, [Path("src/lib.rs")], ranges)
            self.assertEqual((failures, skipped), ([], []))


if __name__ == "__main__":
    unittest.main()
