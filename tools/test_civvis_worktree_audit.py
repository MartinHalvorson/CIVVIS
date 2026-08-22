#!/usr/bin/env python3
"""The reaper removes finished task worktrees and nothing else, and the audit
looks outside them.

On 2026-08-18 it removed `civvis-spectator-src` — the tree the live civvis.ai
exhibition runs its supervisor from — and took the exhibition down. That tree
passed every check the reaper had: clean, idle, HEAD plainly on GitHub. It was
never a task worktree at all. `docs/SPECTATOR_DEPLOY.md` prescribes creating it
with `--detach`, so the shape that distinguishes it was already written down and
simply not consulted.

The second half is the blind spot found on 2026-08-22: the audit only ever
looked at REGISTERED WORKTREES, and every byte actually at risk that day was
outside one — 2,253 lines of operator scripts and the build-parity loop, in `~`
and in the loose non-git `~/civvis-*` directories, on no git object at all.
`LooseFilesAreScanned` is that hole, and each test below is one way the scan can
go quiet while work is lost.
"""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civvis_worktree_audit as audit  # noqa: E402


def run(*args, cwd=None):
    return subprocess.run(args, cwd=cwd, capture_output=True, text=True, check=True)


def a_repo_with(tmp: Path, worktrees: dict[str, str]) -> Path:
    """A real repo with a real remote and real worktrees, keyed name -> branch.

    A branch of `None` means detached, which is the case that mattered.
    """
    origin = tmp / "origin.git"
    run("git", "init", "--bare", "-q", str(origin))
    repo = tmp / "repo"
    run("git", "clone", "-q", str(origin), str(repo))
    run("git", "-C", str(repo), "config", "user.email", "t@example.com")
    run("git", "-C", str(repo), "config", "user.name", "t")
    (repo / "a.txt").write_text("one\n")
    run("git", "-C", str(repo), "add", "a.txt")
    run("git", "-C", str(repo), "commit", "-qm", "one")
    run("git", "-C", str(repo), "branch", "-M", "main")
    run("git", "-C", str(repo), "push", "-q", "-u", "origin", "main")
    for name, branch in worktrees.items():
        target = tmp / name
        if branch is None:
            run("git", "-C", str(repo), "worktree", "add", "-q", "--detach",
                str(target), "origin/main")
        else:
            run("git", "-C", str(repo), "worktree", "add", "-q", "-b", branch,
                str(target), "origin/main")
    return repo


class OnlyTaskWorktreesAreRemoved(unittest.TestCase):
    def reaped(self, repo: Path):
        with mock.patch.object(audit, "process_is_running_from", lambda p: False), \
             mock.patch.object(audit, "on_github", lambda repo, head: True):
            return audit.reap(str(repo), [], idle_minutes=0.0, apply=False)

    def test_a_detached_deploy_checkout_is_never_a_candidate(self):
        """The exact shape of `civvis-spectator-src`, and the exact outage."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo = a_repo_with(tmp, {"deploy": None})
            rows = self.reaped(repo)
            self.assertEqual(
                [r["path"] for r in rows], [],
                "a detached worktree is a deploy checkout, not finished task "
                "work; removing one took civvis.ai down",
            )

    def test_a_task_worktree_is_still_a_candidate(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo = a_repo_with(tmp, {"task": "agent/m/a/finished-thing"})
            rows = self.reaped(repo)
            self.assertEqual(len(rows), 1, rows)
            self.assertTrue(rows[0]["branch"].startswith("agent/"))

    def test_a_branch_that_is_not_agent_shaped_is_spared(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo = a_repo_with(tmp, {"other": "somebodys-deploy-branch"})
            self.assertEqual(self.reaped(repo), [])


class ARunningServiceIsNeverReaped(unittest.TestCase):
    def test_a_tree_a_process_runs_from_is_spared_whatever_its_branch(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo = a_repo_with(tmp, {"task": "agent/m/a/looks-finished"})
            with mock.patch.object(audit, "process_is_running_from", lambda p: True), \
                 mock.patch.object(audit, "on_github", lambda repo, head: True):
                rows = audit.reap(str(repo), [], idle_minutes=0.0, apply=False)
            self.assertEqual(rows, [], "a live service is not finished work")

    def test_being_unable_to_tell_means_not_removing(self):
        """`ps` or `lsof` failing must not read as 'nothing is running'."""
        def explode(*a, **k):
            raise OSError("no ps here")

        with mock.patch.object(audit.subprocess, "run", explode):
            self.assertTrue(audit.process_is_running_from("/tmp/anything"))

    def test_a_quiet_directory_is_reported_as_not_running(self):
        with TemporaryDirectory() as raw:
            self.assertFalse(audit.process_is_running_from(raw))


def a_loose_home(tmp: Path) -> Path:
    """A scan root shaped like this machine's: loose files and output dirs."""
    home = tmp / "home"
    (home / "civvis-logs").mkdir(parents=True)
    (home / "civvis-logs" / "runs").mkdir()
    (home / "civvis-tool.sh").write_text("#!/bin/zsh\necho hi\n")
    # ⚠ THE REGRESSION. An allowlist of .py/.sh/.rs/.lua/.js/.html/.md reported
    # the build-parity harness clean while sim.mjs, saveload.mjs and mapcheck.mjs
    # were on no git object anywhere.
    (home / "civvis-logs" / "sim.mjs").write_text("export const x = 1;\n")
    (home / "civvis-logs" / "run.log").write_text("noise\n")
    (home / "civvis-logs" / "ledger.jsonl.bak3").write_text("rotated\n")
    (home / "civvis-logs" / "runs" / "iter-1.json").write_text("{}\n")
    return home


class LooseFilesAreScanned(unittest.TestCase):
    """Work outside a worktree is the work this fleet actually loses."""

    def scan(self, repo: Path, home: Path, depth=0, rescue=False):
        return audit.loose_audit(str(repo), str(home), ["civvis-*"], depth,
                                 rescue, set())

    def stranded(self, rows):
        return {Path(r["path"]).name: r for r in rows
                if r["kind"] == "STRANDED-ON-DISK"}

    def test_an_mjs_beside_tracked_python_is_found(self):
        """The allowlist bug: .mjs is not a suffix anybody thought to list."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo, home = a_repo_with(tmp, {}), a_loose_home(tmp)
            found = self.stranded(self.scan(repo, home))
            self.assertIn("civvis-logs", found,
                          "a loose directory holding an untracked .mjs must be "
                          f"reported, got {found}")
            self.assertIn(".mjs", found["civvis-logs"]["detail"],
                          "the per-suffix breakdown is what makes an unexpected "
                          "file class visible without an allowlist")

    def test_a_log_file_is_not_a_finding(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo, home = a_repo_with(tmp, {}), a_loose_home(tmp)
            files = [f for r in self.scan(repo, home) for f in r.get("files", [])]
            self.assertFalse([f for f in files if f.endswith(".log")],
                             "run output is denied by suffix, or the signal "
                             "drowns: 5,150 of 5,313 findings were machine output")
            self.assertFalse([f for f in files if ".bak" in f],
                             "ledger.jsonl.bak3 is one rotation, not a new suffix")

    def test_the_depth_bound_is_reported_never_silent(self):
        """A bound nobody is told about is how a scan reports a clean lie."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo, home = a_repo_with(tmp, {}), a_loose_home(tmp)
            rows = self.scan(repo, home, depth=0)
            bounds = [r for r in rows if r["kind"] == "SCAN-BOUNDS"]
            self.assertTrue(bounds, f"the pruned depth must be printed, got {rows}")
            self.assertIn("director", bounds[0]["detail"])
            self.assertFalse(
                [f for r in rows for f in r.get("files", []) if "runs" in f],
                "depth 0 must not descend into runs/ — that is the bound",
            )

    def test_a_registered_worktree_is_left_to_the_other_half(self):
        """Its files are covered by reachability, a better question than this."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo = a_repo_with(tmp, {"civvis-task": "agent/m/a/thing"})
            (tmp / "civvis-task" / "untracked.py").write_text("x = 1\n")
            rows = audit.loose_audit(str(repo), str(tmp), ["civvis-*"], 0, False,
                                     {str(Path(tmp / "civvis-task").resolve())})
            self.assertFalse(self.stranded(rows),
                             f"a worktree must not be reported twice, got {rows}")

    def test_a_batch_check_of_the_wrong_length_raises(self):
        """The second alignment guard: a short batch-check clears live files.

        `zip` stops at the shorter side, so a truncated read would silently drop
        the tail of the file list from the answer and report it as tracked.
        """
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo = a_repo_with(tmp, {})
            for name in ("a", "b", "c"):
                (tmp / name).write_text(f"{name}\n")
            paths = [str(tmp / n) for n in ("a", "b", "c")]

            class ShortRead:
                stdout = "cafebabe missing\n"          # one line for three paths
                returncode = 0

            # hash-object answers correctly; only the batch-check is short, so
            # this pins the second guard and nothing else.
            three = "\n".join(["cafebabe"] * 3)
            with mock.patch.object(audit, "git", lambda *a, **k: three), \
                 mock.patch.object(audit.subprocess, "run",
                                   lambda *a, **k: ShortRead()):
                with self.assertRaisesRegex(RuntimeError, "batch-check returned"):
                    audit.stranded_blobs(str(repo), paths)

    def test_a_symlink_is_not_followed(self):
        """A link into a 179 GB run tree, or a loop, is not this scan's work."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo = a_repo_with(tmp, {})
            loose = tmp / "civvis-links"
            loose.mkdir()
            (tmp / "elsewhere.rs").write_text("fn main() {}\n")
            (loose / "link.rs").symlink_to(tmp / "elsewhere.rs")
            rows = self.scan(repo, tmp, depth=1)
            self.assertFalse(
                [f for r in rows for f in r.get("files", []) if "link.rs" in f],
                "a symlink is a second name for a file, not a second file",
            )

    def test_a_nested_clone_is_not_this_repos_problem(self):
        """~/civvis-fleet/src is a second CIVVIS clone: 2,890 false alarms."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo = a_repo_with(tmp, {})
            nested = tmp / "civvis-fleet"
            nested.mkdir()
            run("git", "init", "-q", str(nested / "src"))
            (nested / "src" / "only-here.rs").write_text("fn main() {}\n")
            # ⚠ depth=1, deliberately. At depth 0 the walk never reaches
            # `src/` at all and this test passes with the nested-repo check
            # deleted — it proved the bound, not the exclusion.
            rows = self.scan(repo, tmp, depth=1)
            self.assertFalse(
                [f for r in rows for f in r.get("files", []) if "only-here" in f],
                "a directory with its own .git is on GitHub through its own "
                "remote, not this one",
            )
            bounds = [r for r in rows if r["kind"] == "SCAN-BOUNDS"]
            self.assertTrue(bounds and "nested git repo" in bounds[0]["detail"],
                            f"the skipped clone must be counted, got {rows}")

    def test_a_nested_clone_at_the_scan_root_is_also_skipped(self):
        """The top-level loop has its own copy of the check; both must hold."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo = a_repo_with(tmp, {})
            clone = tmp / "civvis-clone"
            run("git", "init", "-q", str(clone))
            (clone / "only-here.rs").write_text("fn main() {}\n")
            rows = self.scan(repo, tmp, depth=1)
            self.assertFalse(
                [f for r in rows for f in r.get("files", []) if "only-here" in f],
                "a scanned entry that is itself a clone belongs to its own remote",
            )

    def test_rescue_saves_the_bytes_outside_refs_heads_and_converges(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo, home = a_repo_with(tmp, {}), a_loose_home(tmp)
            origin = tmp / "origin.git"
            rows = self.scan(repo, home, rescue=True)
            self.assertTrue(all(r.get("saved") for r in self.stranded(rows).values()),
                            f"a rescue must report what it saved, got {rows}")
            refs = run("git", "-C", str(origin), "for-each-ref",
                       "--format=%(refname)").stdout.split()
            # ⚠ The production pre-push hook refuses any refs/heads/ name that is
            # not an agent branch, so a snapshot written there is silently
            # refused while the audit reports the files as saved.
            self.assertFalse([r for r in refs if r.startswith("refs/heads/wip")],
                             f"snapshots must not land under refs/heads: {refs}")
            ref = [r for r in refs if "stranded" in r]
            self.assertTrue(ref, f"expected a stranded ref, got {refs}")
            saved = run("git", "-C", str(origin), "show",
                        f"{ref[0]}:civvis-logs/sim.mjs").stdout
            self.assertIn("export const x = 1;", saved)
            # Hole 3 of the original sweep was "it could only report". If the
            # scan cannot go quiet, the 15-minute log is the thing nobody reads.
            self.assertFalse(self.stranded(self.scan(repo, home)),
                             "a rescued file must stop being reported")

    def test_a_failed_push_reads_as_not_rescued(self):
        """`hash-object -w` has already written the blobs locally by then."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo, home = a_repo_with(tmp, {}), a_loose_home(tmp)
            with mock.patch.object(audit, "push_wip", lambda *a, **k: False):
                rows = self.scan(repo, home, rescue=True)
            for row in self.stranded(rows).values():
                self.assertIsNone(row["saved"])
                self.assertIn("NOT RESCUED", row["detail"])

    def test_a_short_hash_object_read_raises_rather_than_mispairing(self):
        """Alignment is the contract: a silent slip clears the wrong file."""
        with TemporaryDirectory() as raw:
            repo = a_repo_with(Path(raw), {})
            with mock.patch.object(audit, "git", lambda *a, **k: "deadbeef"):
                # ⚠ Match the MESSAGE. Both guards raise RuntimeError, so a bare
                # assertRaises passes with either one deleted and neither is
                # tested.
                with self.assertRaisesRegex(RuntimeError, "hash-object returned"):
                    audit.stranded_blobs(str(repo), ["/a", "/b", "/c"])


if __name__ == "__main__":
    unittest.main()
