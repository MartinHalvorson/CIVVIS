#!/usr/bin/env python3
"""Prove that no CIVVIS work exists only on this disk — and make it stop mattering.

⚠⚠ THIS EXISTS BECAUSE THE FLEET LOSES FINISHED WORK. On 2026-08-03 four PRs read
`+0/-0` on GitHub while complete, compiling implementations sat UNSTAGED in local
worktrees; two of those PRs were closed as abandoned because of it. On 2026-08-05
an audit of 110 worktrees found `civvis_orders --without <treatment>` — the
control arm the live bridge had never had — as 146 unstaged lines present on NO
GitHub ref at all, while a memory note recorded the feature as shipped.

`civvis-sync.sh` already ran every fifteen minutes and did not catch either one.
It had three specific holes, and this file is each of them closed:

  1. **It deliberately never looked inside an agent worktree.** Being *behind*
     main is not a fault there — that part was right — so the tree was skipped
     entirely, and being *dirty* was never asked. Every byte ever stranded here
     was stranded in exactly the place the check refused to look.
  2. **It asked about commits, not content.** `git rev-list origin/main..<branch>`
     counts commits; unstaged work has none, so an abandoned tree scored zero and
     passed. Worse, that test cannot answer the real question at all: squash-merge
     rewrites every commit, so commit identity says "unlanded" for work that has
     been on main for days (98 of 110 worktrees, measured).
  3. **It could only report.** A log line nobody reads on a machine running
     eleven agents is not a safeguard.

The question that actually matters is *is this content on GitHub*, and the check
that answers it is reachability from a remote ref — including `refs/pull/N/head`,
which persists forever even for a closed PR:

    git for-each-ref --contains <sha> --count=1 refs/remotes

`--rescue` closes the third hole by making detection almost beside the point: it
pushes a snapshot commit of every dirty worktree to `refs/civvis/wip/<branch>`
WITHOUT touching that worktree's HEAD, index or files (see `snapshot`). An agent
mid-edit is never disturbed, and its bytes are on GitHub within fifteen minutes
whether or not it ever gets to commit them.

⚠ THE NAMESPACE IS NOT NEGOTIABLE. The `pre-push` hook rejects any `refs/heads/`
name that is not `agent/<machine>/<agent>/<task>-<UTC>-<nonce>`, so a snapshot
pushed to `refs/heads/wip/...` is refused and the rescue silently does nothing —
which is worse than no rescue, because the audit would report the tree as saved.
`refs/civvis/` is outside the hook's check and is the namespace
`docs/VERSION_CONTROL.md` already uses for preserved commits.

Exit 0 = nothing exists only on this disk. Exit 1 = something does.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
import time

# Directories whose contents are build output or git internals. Walking a target/
# tree costs more than the whole rest of the audit and can never hold source.
SKIP_DIRS = {".git", "target", "node_modules", "__pycache__", ".venv"}


def git(*args: str, repo: str | None = None, env: dict | None = None,
        check: bool = False) -> str:
    """Run git and return stdout stripped. Empty string on failure unless check."""
    cmd = ["git"]
    if repo:
        cmd += ["-C", repo]
    # gc.auto=0 on every call: Homebrew git segfaults when it forks the automatic
    # maintenance process on this machine, and a fetch is exactly what triggers it.
    cmd += ["-c", "gc.auto=0", *args]
    full_env = {**os.environ, **(env or {})}
    proc = subprocess.run(cmd, capture_output=True, text=True, env=full_env)
    if check and proc.returncode != 0:
        raise RuntimeError(f"{' '.join(cmd)} failed: {proc.stderr.strip()}")
    return proc.stdout.strip() if proc.returncode == 0 else ""


def fetch(repo: str) -> bool:
    """Refresh every remote ref this audit reasons about.

    ⚠ A stale `origin/main` makes the whole run meaningless — every worktree
    reports "nothing outstanding" against a ref from before the work landed. The
    pull refs matter just as much: a CLOSED PR keeps its content at
    `refs/pull/N/head` forever, so a worktree whose PR was closed is NOT stranded
    and must not be reported as such.
    """
    ok = subprocess.run(
        ["git", "-C", repo, "-c", "gc.auto=0", "fetch", "-q", "--prune", "origin"],
        capture_output=True, text=True).returncode == 0
    subprocess.run(
        ["git", "-C", repo, "-c", "gc.auto=0", "fetch", "-q", "origin",
         "+refs/pull/*/head:refs/remotes/pr/*"],
        capture_output=True, text=True)
    return ok


def worktrees(repo: str) -> list[str]:
    out = git("worktree", "list", "--porcelain", repo=repo)
    return [l.split(" ", 1)[1] for l in out.splitlines() if l.startswith("worktree ")]


def newest_edit(path: str) -> float:
    """mtime of the most recently touched source file, or 0.0 if none."""
    newest = 0.0
    for root, dirs, files in os.walk(path):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for f in files:
            try:
                newest = max(newest, os.stat(os.path.join(root, f)).st_mtime)
            except OSError:
                pass
    return newest


def on_github(repo: str, sha: str) -> bool:
    """Is this commit reachable from any ref GitHub holds?

    ⚠ NOT `git branch --merged` and NOT `git cherry`. Squash-merge gives the
    landed content a brand new commit with a brand new patch-id, so both of those
    report long-landed branches as unlanded — measured at 98 false alarms out of
    110 worktrees, which is the same as having no check.

    ⚠ `refs/civvis/` counts. Nothing under it is fetched by the default
    refspec, so a commit this tool has already pushed to `refs/civvis/wip/`
    would otherwise be reported as disk-only forever — the local mirror ref
    `push_wip` writes after a successful push is the only local evidence the
    remote has it, and searching `refs/remotes` alone misses it.
    """
    return bool(git("for-each-ref", "--contains", sha, "--count=1",
                    "--format=%(refname)", "refs/remotes", "refs/civvis",
                    repo=repo))


def snapshot(repo: str, tree_path: str, branch: str) -> str | None:
    """Push the worktree's contents to `refs/civvis/wip/<branch>`, return the sha.

    ⚠⚠ THIS MUST NOT DISTURB A LIVE AGENT, so it never runs `git add`, never
    writes the worktree's index and never moves its HEAD. It stages into a
    THROWAWAY index file, writes a tree, and hangs one commit off the current
    HEAD with `commit-tree`. The agent's `git status` is byte-for-byte unchanged
    afterwards; the only trace is a ref on the remote.
    """
    head = git("rev-parse", "HEAD", repo=tree_path)
    if not head:
        return None
    with tempfile.TemporaryDirectory() as tmp:
        # ⚠ The identity is passed explicitly, never inherited. This runs from
        # launchd, which has almost no environment: with no user.email git
        # refuses to write a commit at all, `commit-tree` returns nothing, and
        # the rescue silently saves nothing while reporting success. CI has the
        # same empty identity and is what caught it.
        env = {
            "GIT_INDEX_FILE": os.path.join(tmp, "index"),
            "GIT_AUTHOR_NAME": "civvis-worktree-audit",
            "GIT_AUTHOR_EMAIL": "civvis-worktree-audit@localhost",
            "GIT_COMMITTER_NAME": "civvis-worktree-audit",
            "GIT_COMMITTER_EMAIL": "civvis-worktree-audit@localhost",
        }
        # Seed the scratch index from HEAD so unchanged files are already staged,
        # then add everything the tree currently holds.
        if not git("read-tree", head, repo=tree_path, env=env):
            pass  # read-tree is silent on success; failure shows up in write-tree
        git("add", "-A", repo=tree_path, env=env)
        tree = git("write-tree", repo=tree_path, env=env)
        if not tree:
            return None
        stamp = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        msg = (f"wip snapshot of {os.path.basename(tree_path)} at {stamp}\n\n"
               "Written by tools/civvis_worktree_audit.py --rescue so that work in\n"
               "progress cannot exist only on one disk. NOT for review or merge:\n"
               "the owning agent's HEAD, index and files were not touched.\n")
        commit = git("commit-tree", tree, "-p", head, "-m", msg,
                     repo=tree_path, env=env)
        if not commit:
            return None
    # See the module docstring: refs/heads/wip/* is rejected by the pre-push hook.
    ref = f"refs/civvis/wip/{branch}"
    # ⚠ `--force`, not `--force-with-lease`. The lease needs a remote-tracking
    # ref to compare against, and nothing under refs/civvis/ is ever fetched by
    # the default refspec — so the FIRST rescue succeeds and every later one is
    # rejected for a lease it cannot know. This ref is a rolling snapshot this
    # tool alone writes; overwriting our own previous snapshot is the point.
    return commit if push_wip(tree_path, commit, branch) else None


def push_wip(tree_path: str, sha: str, branch: str) -> bool:
    """Force `sha` onto `refs/civvis/wip/<branch>` on the remote.

    ⚠ `--force`, not `--force-with-lease`. The lease needs a remote-tracking
    ref to compare against, and nothing under refs/civvis/ is ever fetched by
    the default refspec — so the FIRST rescue succeeds and every later one is
    rejected for a lease it cannot know. This ref is a rolling snapshot this
    tool alone writes; overwriting our own previous snapshot is the point.
    """
    ref = f"refs/civvis/wip/{branch}"
    pushed = subprocess.run(
        ["git", "-C", tree_path, "-c", "gc.auto=0", "push", "--force",
         "origin", f"{sha}:{ref}"],
        capture_output=True, text=True)
    if pushed.returncode != 0:
        return False
    # Mirror the ref locally ONLY after the push succeeded, so `on_github` can
    # see that this commit is preserved without a network round trip. Writing it
    # before the push would let a failed push read as a rescue.
    git("update-ref", ref, sha, repo=tree_path)
    return True


def preserve_head(tree_path: str, head: str, branch: str) -> str | None:
    """Put a clean worktree's local-only HEAD on the remote, as-is.

    `snapshot` only helps a DIRTY worktree: it exists to capture uncommitted
    bytes. A CLEAN worktree whose HEAD is on no remote ref is exactly as lossy —
    the commits exist on one disk and nothing but this ref is holding them — and
    for a long-lived branch that is far more work than a single dirty tree. This
    pushes the commit itself rather than a snapshot of it, so the history is
    preserved verbatim rather than as one squashed tree.
    """
    if branch in ("HEAD", "(detached)") or not branch:
        # A detached HEAD has no name to file the ref under. Use the sha, which
        # is at least stable and greppable.
        branch = f"detached/{head[:12]}"
    return head if push_wip(tree_path, head, branch) else None


def audit(repo: str, idle_minutes: int, rescue: bool) -> list[dict]:
    findings = []
    now = time.time()
    for path in worktrees(repo):
        if not os.path.isdir(path):
            findings.append({"path": path, "kind": "MISSING",
                             "detail": "registered worktree directory is gone"})
            continue
        branch = git("rev-parse", "--abbrev-ref", "HEAD", repo=path) or "HEAD"
        porcelain = git("status", "--porcelain", repo=path)
        dirty = [l for l in porcelain.splitlines() if l.strip()]
        head = git("rev-parse", "HEAD", repo=path)

        if dirty:
            idle_for = (now - newest_edit(path)) / 60.0
            active = idle_for < idle_minutes
            saved = snapshot(repo, path, branch) if rescue else None
            findings.append({
                "path": path, "branch": branch,
                "kind": "DIRTY-ACTIVE" if active else "DIRTY-ABANDONED",
                "detail": f"{len(dirty)} uncommitted file(s), "
                          f"last edited {idle_for:.0f} min ago"
                          + (f"; snapshot {saved[:8]} -> refs/civvis/wip/{branch}" if saved
                             else "; NOT SNAPSHOTTED" if rescue else ""),
                "active": active, "saved": saved,
            })

        if head and not on_github(repo, head):
            # A clean worktree is rescued too. `snapshot` above only fires for a
            # dirty one, and for years that left the worse case unhandled: a
            # clean worktree carrying commits that exist on no remote at all.
            # Measured 2026-08-08 — a branch with 17 such commits reported here
            # every cycle while `--rescue` pushed nothing, and deleting the
            # worktree would have destroyed all of them.
            saved = preserve_head(path, head, branch) if rescue else None
            findings.append({
                "path": path, "branch": branch, "kind": "COMMIT-NOT-ON-GITHUB",
                "detail": f"HEAD {head[:8]} is reachable from no remote ref"
                          + (f"; preserved -> refs/civvis/wip/{branch}" if saved
                             else "; NOT PRESERVED" if rescue else ""),
                "saved": saved,
            })
    return findings


# --- self-test ---------------------------------------------------------------
# ⚠ There is no CIVVIS fleet in CI, so this builds a throwaway repo with a real
# remote, a real worktree and a real unstaged edit, and asserts the audit sees it.
# It is the only way to keep the hole from silently reopening: the failure this
# file exists to prevent is invisible to every other test in the repo.
def selftest() -> int:
    import shutil
    tmp = tempfile.mkdtemp(prefix="civvis-audit-selftest-")
    try:
        remote, work = os.path.join(tmp, "remote.git"), os.path.join(tmp, "work")
        subprocess.run(["git", "init", "-q", "--bare", remote], check=True)
        subprocess.run(["git", "init", "-q", "-b", "main", work], check=True)
        cfg = ["-c", "user.email=t@t", "-c", "user.name=t"]
        open(os.path.join(work, "a.txt"), "w").write("one\n")
        subprocess.run(["git", "-C", work, "add", "-A"], check=True)
        subprocess.run(["git", "-C", work, *cfg, "commit", "-qm", "one"], check=True)
        subprocess.run(["git", "-C", work, "remote", "add", "origin", remote], check=True)
        subprocess.run(["git", "-C", work, "push", "-q", "-u", "origin", "main"], check=True)

        wt = os.path.join(tmp, "wt")
        subprocess.run(["git", "-C", work, "worktree", "add", "-q", "-b", "feat", wt],
                       check=True)
        subprocess.run(["git", "-C", wt, "push", "-q", "-u", "origin", "feat"], check=True)

        clean = audit(work, idle_minutes=30, rescue=False)
        assert not clean, f"a clean fleet must report nothing, got {clean}"

        # The exact failure mode: finished work left unstaged, no commit at all.
        open(os.path.join(wt, "a.txt"), "w").write("one\ntwo\n")
        found = audit(work, idle_minutes=30, rescue=False)
        kinds = {f["kind"] for f in found}
        assert "DIRTY-ACTIVE" in kinds, f"unstaged work must be reported, got {found}"

        # And that rescuing it puts the bytes on the remote without moving HEAD.
        before = git("rev-parse", "HEAD", repo=wt)
        before_status = git("status", "--porcelain", repo=wt)
        audit(work, idle_minutes=30, rescue=True)
        assert git("rev-parse", "HEAD", repo=wt) == before, "HEAD must not move"
        assert git("status", "--porcelain", repo=wt) == before_status, \
            "the agent's working tree must be untouched"
        saved = subprocess.run(
            ["git", "-C", remote, "show", "refs/civvis/wip/feat:a.txt"],
            capture_output=True, text=True).stdout
        assert "two" in saved, f"the snapshot must carry the unstaged bytes, got {saved!r}"
        # ⚠ The production remote's pre-push hook refuses any refs/heads/ name
        # that is not an agent branch, so a snapshot written there is silently
        # refused. Assert the namespace, not just that some ref exists.
        heads = git("for-each-ref", "--format=%(refname)", "refs/heads", repo=remote)
        assert "wip" not in heads, f"snapshots must not land under refs/heads: {heads}"

        # ⚠ A rescue must survive being run again — it runs every fifteen
        # minutes. The first push creates the ref; only the second exercises the
        # overwrite, which is where --force-with-lease fails for want of a
        # remote-tracking ref that refs/civvis/ never gets.
        open(os.path.join(wt, "a.txt"), "w").write("one\ntwo\nthree\n")
        audit(work, idle_minutes=30, rescue=True)
        again = subprocess.run(
            ["git", "-C", remote, "show", "refs/civvis/wip/feat:a.txt"],
            capture_output=True, text=True).stdout
        assert "three" in again, f"a repeat rescue must overwrite, got {again!r}"

        # A commit that never reached the remote must be named.
        subprocess.run(["git", "-C", wt, "add", "-A"], check=True)
        subprocess.run(["git", "-C", wt, *cfg, "commit", "-qm", "local only"], check=True)
        found = audit(work, idle_minutes=30, rescue=False)
        assert any(f["kind"] == "COMMIT-NOT-ON-GITHUB" for f in found), \
            f"a local-only commit must be reported, got {found}"

        # ⚠ AND RESCUING IT MUST ACTUALLY PRESERVE IT. Reporting is not saving.
        # The tree is CLEAN at this point — everything was just committed — so
        # `snapshot` does not fire, and before 2026-08-08 `--rescue` pushed
        # nothing here while still exiting as though it had done its job.
        local_only = git("rev-parse", "HEAD", repo=wt)
        audit(work, idle_minutes=30, rescue=True)
        preserved = subprocess.run(
            ["git", "-C", remote, "rev-parse", "refs/civvis/wip/feat"],
            capture_output=True, text=True).stdout.strip()
        assert preserved == local_only, (
            "a clean worktree's local-only HEAD must be pushed verbatim, "
            f"remote has {preserved!r}, worktree has {local_only!r}")

        # And once preserved it must stop being reported, or the audit never
        # converges and a real finding drowns in a permanent false positive.
        after = audit(work, idle_minutes=30, rescue=False)
        assert not any(f["kind"] == "COMMIT-NOT-ON-GITHUB" for f in after), \
            f"a preserved commit must no longer be reported, got {after}"
        print("selftest: ok")
        return 0
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--repo", default=os.path.expanduser("~/CIVVIS"))
    ap.add_argument("--idle-minutes", type=int, default=30,
                    help="a dirty tree edited more recently than this is an agent "
                         "mid-change, not abandoned work")
    ap.add_argument("--rescue", action="store_true",
                    help="push a wip/<branch> snapshot of every dirty worktree "
                         "without touching it")
    ap.add_argument("--no-fetch", action="store_true",
                    help="skip the fetch; only for tests, a stale ref voids the run")
    ap.add_argument("--quiet", action="store_true", help="print only problems")
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args()

    if args.selftest:
        return selftest()

    if not args.no_fetch and not fetch(args.repo):
        print("FETCH FAILED — refusing to report on stale refs", file=sys.stderr)
        return 1

    findings = audit(args.repo, args.idle_minutes, args.rescue)
    # Abandoned work first: that is the class that gets lost.
    order = {"DIRTY-ABANDONED": 0, "COMMIT-NOT-ON-GITHUB": 1, "MISSING": 2,
             "DIRTY-ACTIVE": 3}
    for f in sorted(findings, key=lambda f: order.get(f["kind"], 9)):
        print(f"{f['kind']}: {os.path.basename(f['path'])} — {f['detail']}")

    # An ACTIVE dirty tree that was snapshotted is not a problem: an agent is
    # working and its bytes are already on GitHub. Everything else needs a person.
    unresolved = [f for f in findings
                  if not (f["kind"] == "DIRTY-ACTIVE" and f.get("saved"))]
    if not unresolved:
        if not args.quiet:
            print(f"nothing exists only on this disk "
                  f"({len(worktrees(args.repo))} worktree(s) checked)")
        return 0
    # Under --quiet every printed line is a finding: civvis_sync.sh step 4
    # echoes and counts them, so a summary here would be double-reported and
    # counted as an extra problem ("2 item(s)" followed by "3 item(s)" in the
    # sweep log). The exit code already carries the verdict.
    if not args.quiet:
        print(f"{len(unresolved)} item(s) need a person")
    return 1


if __name__ == "__main__":
    sys.exit(main())
