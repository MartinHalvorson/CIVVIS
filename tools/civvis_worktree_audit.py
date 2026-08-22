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
  4. **It only ever looked at REGISTERED WORKTREES** — and so did this file, for
     its first three weeks. On 2026-08-22 the worktree half found three dirty
     trees, all three already byte-identical on their rescue refs, and reported
     the fleet clean. It was clean. Meanwhile 2,253 lines of working
     infrastructure sat in `~` and in the loose non-git `~/civvis-*` directories
     on no git object at all: six operator scripts launchd runs every day, and
     the whole rust/wasm build-parity loop that had found #1061, #1093 and
     #1076. `git worktree list` cannot name one byte of that. `--scan-root`
     (default: the repo's parent) is hole 4 closed; `--no-scan` reopens it.

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
import json
import re
import shutil
import subprocess
import sys
import tempfile
import time

# Directories whose contents are build output or git internals. Walking a target/
# tree costs more than the whole rest of the audit and can never hold source.
SKIP_DIRS = {".git", "target", "node_modules", "__pycache__", ".venv"}

# ── the loose scan: the half of this disk a worktree list cannot see ──────────
#
# ⚠⚠ ON 2026-08-22 EVERY BYTE ACTUALLY AT RISK WAS OUTSIDE A WORKTREE. The
# worktree half of this audit reported three dirty trees and all three were
# already byte-identical on their rescue refs — it was clean, and it was clean
# because it had nothing to find. Meanwhile 2,253 lines of working
# infrastructure sat in ~ and in the loose non-git ~/civvis-* directories on no
# git object at all: six operator scripts launchd runs every day, and the whole
# rust/wasm build-parity loop that found #1061, #1093 and #1076. `git worktree
# list` cannot name any of it, so nothing ever asked.

#: Suffixes whose content is data or a build artifact.
#:
#: ⚠⚠ THIS IS A DENYLIST AND IT MUST STAY ONE. The first pass of that sweep used
#: an ALLOWLIST — .py .sh .rs .lua .js .html .md — and reported the build-parity
#: harness clean while three of its seven code files (`sim.mjs`, `saveload.mjs`,
#: `mapcheck.mjs`) and its `configs.json` were on no git object anywhere. An
#: allowlist fails SILENT on the extension nobody thought of; a denylist fails
#: loud, with one noisy finding you can then deny. Adding a suffix here is a
#: deliberate act that should come with a measurement; guessing a new allowlist
#: entry is not possible, which is the point.
#:
#: The real stranded set on this machine carried `.tgz`, `.store`, `.f32`,
#: `.patch`, `.list` and `.bak-20260731` — six suffixes no one would have
#: allowlisted in advance.
SCAN_SKIP_SUFFIXES = frozenset({
    # logs and run output
    ".log", ".out", ".jsonl", ".ndjson", ".csv.gz", ".bak", ".tmp", ".swp", ".part",
    # compiled or packaged
    ".pyc", ".pyo", ".o", ".a", ".so", ".dylib", ".rlib", ".rmeta", ".wasm", ".d",
    ".zip", ".gz", ".tgz", ".tar", ".bz2", ".xz", ".zst", ".dmg", ".pkg",
    # media and binary blobs
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".icns", ".svg.gz",
    ".mp4", ".mov", ".webm", ".wav", ".mp3", ".pdf", ".dds", ".bin",
    ".woff", ".woff2", ".ttf", ".otf",
    # databases and game saves
    ".db", ".sqlite", ".sqlite3", ".sqlite-shm", ".sqlite-wal", ".civ6save",
})

#: Names that are never work regardless of suffix.
SCAN_SKIP_NAMES = frozenset({".DS_Store", ".stop", ".running", "state.env"})

#: A file bigger than this is data. The largest source file in CIVVIS is
#: `src/game.rs` at well under a megabyte; the loose directories hold 2 MB+
#: JSON run dumps by the thousand.
SCAN_MAX_BYTES = 2_000_000

#: How far below each scanned entry to walk.
#:
#: 0 — the top of each loose `~/civvis-*` directory and no further — is where
#: hand-written work actually sits on this machine, with machine output one
#: level down in `runs/`. Measured on the real disk: depth 0 scans 180 files and
#: reports 78 stranded across 14 directories, every one of them plausible work;
#: depth 1 scans 5,463 and reports 5,313, of which 5,150 are one loop's
#: per-iteration JSON and a Chrome profile's cache.
#:
#: This is a BOUND, not a claim that nothing lives deeper, which is why every
#: run prints how many directories the limit declined to open. Raise it with
#: `--scan-depth`; a deeper sweep by hand now and then is worth the noise.
SCAN_DEPTH = 0


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
            # ⚠ In a LINKED worktree `.git` is a file, not a directory, so
            # `SKIP_DIRS` never excluded it and `os.walk` handed it over as
            # source. Its mtime is set when the worktree is created, which
            # makes a tree that has never been edited look freshly worked on.
            # Old trees are unaffected — measured on this fleet, `.git` there
            # is as old as everything else — so this is not the reason a
            # worktree went unreported; it is the reason a NEW one reads as
            # active for its first `--idle-minutes`, which is exactly the
            # window `--reap` has to get right.
            if f == ".git":
                continue
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


def branch_pr_is_merged(branch: str) -> bool:
    """Has this branch's pull request already merged? False when unknown.

    ⚠⚠⚠ WITHOUT THIS, `ship` MAKES EVERY WORKTREE IT TOUCHES UNREAPABLE.
    `ship` merges `origin/main` into the branch before marking it ready. That
    merge commit is created locally and, once the pull request squash-merges
    and `ship` deletes the remote branch, it is reachable from no ref on
    GitHub — so `on_github` says no and `--reap` refuses, correctly but
    uselessly, because the CONTENT is on `main` twice over: the branch side is
    at `refs/pull/N/head` and the other parent IS `main`.

    Measured on 2026-08-17: after the first reap took 137 worktrees, the two
    that were left behind were exactly this shape. Every future task that has
    to merge `main` before shipping would have joined them, which turns the
    reaper into something that only tidies the tasks that were never busy.

    `AGENTS.md` already draws the line here — "ask whether GitHub has the
    content, not whether it was merged... a closed PR keeps its content at
    `refs/pull/N/head` forever" — and a MERGED pull request is the strongest
    possible answer to that question about a branch.

    ⚠ False on any doubt: no `gh`, no network, no pull request, an open one, a
    closed-unmerged one, or an unparseable answer all mean "do not reap".
    """
    if not branch or branch in ("HEAD", "main"):
        return False
    if shutil.which("gh") is None:
        return False
    try:
        out = subprocess.run(
            ["gh", "pr", "view", branch, "--json", "state,mergedAt"],
            capture_output=True, text=True, timeout=30)
    except (subprocess.SubprocessError, OSError):
        return False
    if out.returncode != 0:
        return False
    try:
        view = json.loads(out.stdout or "{}")
    except ValueError:
        return False
    return view.get("state") == "MERGED" and bool(view.get("mergedAt"))


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


def scan_candidates(scan_root: str, patterns: list[str], known: set[str],
                    depth: int = SCAN_DEPTH) -> tuple[list[str], dict]:
    """Files under `scan_root` that could hold work, plus what was left out.

    Everything a registered worktree already covers is excluded — that half of
    the audit asks a better question about it (reachability, not blob presence)
    and would otherwise report every uncommitted file twice.

    ⚠ A directory holding its own `.git` is excluded too, and NOT because it is
    uninteresting: `~/civvis-fleet/src` is a second clone of CIVVIS, so its
    files are on GitHub through that clone while being absent from THIS repo's
    object database. Hashing them against this repo reports 2,890 files as
    disk-only when every one of them is safe.

    The returned `skipped` counts are printed, never swallowed: a bound nobody
    is told about is how a scan reports "clean" for a directory it declined to
    open.
    """
    import fnmatch

    paths: list[str] = []
    skipped = {"depth_pruned": 0, "oversize": 0, "denied_suffix": 0, "nested_repo": 0}

    def collect(entry: str) -> None:
        base = entry.count(os.sep)
        for dirpath, dirs, files in os.walk(entry):
            if dirpath.count(os.sep) - base >= depth:
                skipped["depth_pruned"] += len(dirs)
                dirs[:] = []
            keep = []
            for d in dirs:
                full = os.path.join(dirpath, d)
                if d in SKIP_DIRS:
                    continue
                if os.path.realpath(full) in known:
                    continue
                if os.path.exists(os.path.join(full, ".git")):
                    skipped["nested_repo"] += 1
                    continue
                keep.append(d)
            dirs[:] = keep
            for name in files:
                consider(os.path.join(dirpath, name))

    def consider(full: str) -> None:
        name = os.path.basename(full)
        if name in SCAN_SKIP_NAMES:
            return
        suffix = os.path.splitext(name)[1].lower()
        # `ledger.jsonl.bak2` through `.bak6` are one rotation, not five new
        # suffixes to chase into the denylist by hand.
        if suffix in SCAN_SKIP_SUFFIXES or re.fullmatch(r"\.bak\d+", suffix):
            skipped["denied_suffix"] += 1
            return
        try:
            if os.path.islink(full) or not os.path.isfile(full):
                return
            if os.stat(full).st_size > SCAN_MAX_BYTES:
                skipped["oversize"] += 1
                return
        except OSError:
            return
        paths.append(os.path.abspath(full))

    try:
        entries = sorted(os.listdir(scan_root))
    except OSError:
        return [], skipped
    for entry in entries:
        if not any(fnmatch.fnmatch(entry, pat) for pat in patterns):
            continue
        full = os.path.join(scan_root, entry)
        if os.path.isfile(full):
            consider(full)
        elif os.path.isdir(full) and not os.path.islink(full):
            if os.path.realpath(full) in known:
                continue
            if os.path.exists(os.path.join(full, ".git")):
                skipped["nested_repo"] += 1
                continue
            collect(full)
    return paths, skipped


def stranded_blobs(repo: str, paths: list[str], chunk: int = 400) -> list[str]:
    """The subset of `paths` whose exact bytes are in no git object.

    This is [[stranded-code-audit]]'s `git cat-file -e $(git hash-object FILE)`
    made batch: one `hash-object` per `chunk` paths and ONE `cat-file
    --batch-check` for the lot, because per-file subprocesses cost more than
    every other part of this tool put together. Measured: 7,156 files in 3.1 s.

    ⚠ `hash-object` WITHOUT `-w`. Detection must not write the very objects it
    is about to ask about — that would make the second run of a rescue whose
    push failed report the files as saved.
    """
    if not paths:
        return []
    hashes: list[str] = []
    for i in range(0, len(paths), chunk):
        out = git("hash-object", "--", *paths[i:i + chunk], repo=repo)
        hashes.extend(out.split())
    if len(hashes) != len(paths):
        # Alignment is the whole contract: a short read would silently pair a
        # hash with the wrong file and clear a stranded one.
        raise RuntimeError(
            f"hash-object returned {len(hashes)} hashes for {len(paths)} paths")
    proc = subprocess.run(
        ["git", "-C", repo, "-c", "gc.auto=0", "cat-file", "--batch-check"],
        input="\n".join(hashes) + "\n", capture_output=True, text=True)
    lines = proc.stdout.splitlines()
    if len(lines) != len(paths):
        raise RuntimeError(
            f"batch-check returned {len(lines)} lines for {len(paths)} paths")
    return [p for p, line in zip(paths, lines) if line.endswith(" missing")]


def rescue_loose(repo: str, scan_root: str, paths: list[str], label: str) -> str | None:
    """Push every loose stranded file to `refs/civvis/wip/stranded/<label>`.

    One commit for the lot, chained onto the previous snapshot so a file that
    later disappears from the disk stays reachable in the ref's history.

    ⚠ This is the step that makes the loose scan converge. Without it the audit
    would report the same thousands of files every fifteen minutes, and hole 3
    of the original `civvis-sync.sh` — "it could only report" — would be back
    for the half of the disk this commit was written to cover.
    """
    if not paths:
        return None
    with tempfile.TemporaryDirectory() as tmp:
        env = {
            "GIT_INDEX_FILE": os.path.join(tmp, "index"),
            "GIT_AUTHOR_NAME": "civvis-worktree-audit",
            "GIT_AUTHOR_EMAIL": "civvis-worktree-audit@localhost",
            "GIT_COMMITTER_NAME": "civvis-worktree-audit",
            "GIT_COMMITTER_EMAIL": "civvis-worktree-audit@localhost",
        }
        entries = []
        for i in range(0, len(paths), 400):
            batch = paths[i:i + 400]
            blobs = git("hash-object", "-w", "--", *batch, repo=repo).split()
            if len(blobs) != len(batch):
                return None
            for blob, path in zip(blobs, batch):
                rel = os.path.relpath(path, scan_root)
                if rel.startswith(".."):
                    continue
                mode = "100755" if os.access(path, os.X_OK) else "100644"
                entries.append(f"{mode} {blob}\t{rel}")
        if not entries:
            return None
        proc = subprocess.run(
            ["git", "-C", repo, "-c", "gc.auto=0", "update-index", "--add",
             "--index-info"],
            input="\n".join(entries) + "\n", capture_output=True, text=True,
            env={**os.environ, **env})
        if proc.returncode != 0:
            return None
        tree = git("write-tree", repo=repo, env=env)
        if not tree:
            return None
        ref = f"refs/civvis/wip/stranded/{label}"
        parent = git("rev-parse", "--verify", "--quiet", ref, repo=repo)
        stamp = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        msg = (f"stranded loose files on {label} at {stamp}\n\n"
               f"{len(entries)} file(s) under {scan_root} whose content was in no\n"
               "git object. Written by tools/civvis_worktree_audit.py --rescue so\n"
               "that nothing outside a worktree exists only on one disk. NOT for\n"
               "review or merge; the files on disk were not touched.\n")
        args = ["commit-tree", tree, "-m", msg]
        if parent:
            args += ["-p", parent]
        commit = git(*args, repo=repo, env=env)
        if not commit:
            return None
    # Same namespace and the same plain --force as every other rescue here: see
    # push_wip. A failed push must read as NOT RESCUED, never as saved, because
    # `hash-object -w` above has already put the blobs in the LOCAL object
    # database and the next run's detection would otherwise find them and go
    # quiet.
    return commit if push_wip(repo, commit, f"stranded/{label}") else None


def machine_label(repo: str) -> str:
    """The fleet-unique id for this machine, for the stranded ref's name."""
    return (git("config", "--local", "civvis.machine", repo=repo)
            or os.uname().nodename.split(".")[0] or "unknown")


def audit(repo: str, idle_minutes: int, rescue: bool, scan_root: str | None = None,
          scan_patterns: list[str] | None = None, scan_depth: int = SCAN_DEPTH,
          ) -> list[dict]:
    findings = []
    now = time.time()
    trees = worktrees(repo)
    for path in trees:
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

    if scan_root:
        findings += loose_audit(repo, scan_root,
                                scan_patterns or ["civvis-*"], scan_depth, rescue,
                                {os.path.realpath(t) for t in trees})
    return findings


def loose_audit(repo: str, scan_root: str, patterns: list[str], depth: int,
                rescue: bool, known: set[str]) -> list[dict]:
    """Report — and with `--rescue`, save — work that is on no git object.

    One finding per directory, not per file. The stranded set on this machine is
    3,417 files and 3,253 of them are one loop's per-iteration JSON output; a
    finding per file would bury the nine hand-written ones and make the sweep's
    log the thing nobody reads. The per-suffix breakdown is what makes the nine
    visible: `civvis-simloop-logs: 9 (.mjs 3, .py 2, .sh 2, .json 1, .env 1)`
    names `.mjs` without any allowlist having had to guess it.
    """
    paths, skipped = scan_candidates(scan_root, patterns, known, depth)
    # ⚠ NOT `if not paths: return []`. A scan that excluded everything it found
    # has the most to explain, and returning early there suppressed the bounds
    # line in exactly that case — a scan reporting nothing because it opened
    # nothing, which is the failure this whole commit exists to remove.
    missing = stranded_blobs(repo, paths) if paths else []
    findings: list[dict] = []
    if skipped["depth_pruned"] or skipped["oversize"] or skipped["nested_repo"]:
        findings.append({
            "path": scan_root, "kind": "SCAN-BOUNDS", "branch": "",
            "detail": f"scanned {len(paths)} file(s) at depth {depth}; not "
                      f"opened: {skipped['depth_pruned']} deeper director(ies), "
                      f"{skipped['oversize']} file(s) over "
                      f"{SCAN_MAX_BYTES // 1000}kB, "
                      f"{skipped['nested_repo']} nested git repo(s)",
            "bounds": True, "saved": None,
        })
    if not missing:
        return findings
    saved = rescue_loose(repo, scan_root, missing, machine_label(repo)) if rescue else None
    by_dir: dict[str, list[str]] = {}
    for m in missing:
        by_dir.setdefault(os.path.dirname(m), []).append(m)
    for directory, files in sorted(by_dir.items()):
        counts: dict[str, int] = {}
        for f in files:
            counts[os.path.splitext(f)[1].lower() or "<none>"] = \
                counts.get(os.path.splitext(f)[1].lower() or "<none>", 0) + 1
        breakdown = ", ".join(f"{ext} {n}" for ext, n in
                              sorted(counts.items(), key=lambda kv: -kv[1])[:6])
        findings.append({
            "path": directory, "branch": "", "kind": "STRANDED-ON-DISK",
            "detail": f"{len(files)} file(s) on no git object ({breakdown})"
                      + (f"; saved {saved[:8]} -> refs/civvis/wip/stranded/"
                         f"{machine_label(repo)}" if saved
                         else "; NOT RESCUED" if rescue else ""),
            "saved": saved, "files": files,
        })
    return findings



def process_is_running_from(path: str) -> bool:
    """Whether any live process has this tree as its cwd or names it in argv.

    The branch rule above is the principled refusal; this is the one that holds
    when somebody starts a service by hand from a tree that does look like a
    task. Both are cheap and neither is sufficient alone: a service that is
    momentarily DOWN has no process to find, which is precisely the state
    `civvis-spectator-src` was in when it was deleted.
    """
    real = os.path.realpath(path)
    try:
        listing = subprocess.run(
            ["ps", "-axo", "command="], capture_output=True, text=True, timeout=30
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return True  # cannot tell -> do not remove
    if any(real in line for line in listing.splitlines()):
        return True
    try:
        cwds = subprocess.run(
            ["lsof", "-a", "-d", "cwd", "-Fn", "--", real],
            capture_output=True, text=True, timeout=60,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return True
    return bool(cwds.strip())


def reap(repo: str, findings: list[dict], idle_minutes: int,
         apply: bool) -> list[dict]:
    """Remove worktrees whose work is already safe on GitHub.

    ⚠⚠⚠ THE FLEET NEVER REMOVED A FINISHED WORKTREE, AND IT ADDS ONE PER TASK.
    `ship` deletes the REMOTE branch and stops; nothing has ever removed the
    local worktree, its branch, or its ~4 GB `target/`. Measured on
    `mbp-m5-max-128` on 2026-08-17: **142 worktrees, 120 of them on branches
    whose pull request had already merged, none of them dirty, 960 GB** — and
    the machine had 702 GiB of 1.8 TiB left. At ~100 merged PRs a day this is
    not drift, it is an accumulation the fleet cannot outrun.

    ⚠ THE SAFETY BAR IS THE ONE AGENTS.md ALREADY SETS, and it is not
    "is it merged". Squash merge rewrites commits, so `git branch --merged`
    and `git cherry` both call long-landed work unlanded — measured at 98 false
    alarms out of 110 here. The question is whether GITHUB HAS THE CONTENT,
    which `on_github` answers over `refs/remotes` and `refs/civvis` after a
    fetch that includes `refs/pull/*/head`. A closed pull request keeps its
    content at `refs/pull/N/head` forever, so "closed" does not mean lost
    either.

    Six refusals, each because it has cost something somewhere:

      * anything the audit flagged — DIRTY, COMMIT-NOT-ON-GITHUB or MISSING —
        is never a candidate. The rescue path exists for those and this must
        not race it;
      * the `main` management worktree and the repository root are never
        touched;
      * ⚠⚠ A WORKTREE THAT IS NOT ON AN `agent/*` BRANCH IS NOT TASK
        SCAFFOLDING AND IS NOT THIS TOOL'S BUSINESS. This tool's whole premise
        is finished task work whose content GitHub already has, and a task
        worktree is one `civvis_collab.py start` created, on an
        `agent/<machine>/<agent>/<task>` branch. A DETACHED worktree pinned to
        `origin/main` looks perfect to every other test here — clean, idle,
        HEAD plainly on GitHub — and on 2026-08-18 that is exactly how this
        reaper deleted `civvis-spectator-src`, the tree the live civvis.ai
        exhibition runs its supervisor from, and took the exhibition down. It
        was never a task. `docs/SPECTATOR_DEPLOY.md` prescribes creating it
        `--detach`, so the shape that saved it was already written down;
      * a worktree some live process is running from is never removed, whatever
        its branch says. That is the belt to the rule above's braces: a service
        started by hand from a task-shaped tree is still a running service;
      * a worktree edited within `idle_minutes` is left alone, because an agent
        may be mid-task in a tree whose HEAD happens to be landed;
      * `--reap` reports; `--reap --apply` removes. A destructive default is
        how a tool like this ends up famous.
    """
    # ⚠ A tree flagged ONLY with COMMIT-NOT-ON-GITHUB, whose branch's pull
    # request has MERGED, is `ship` scaffolding rather than lost work: the
    # merge commit `ship` made before marking the PR ready is local, but both
    # its parents are on GitHub and its content reached `main` as the squash.
    # See `branch_pr_is_merged`. Every other flag still refuses, so a dirty
    # tree or a missing one is untouched no matter what its PR says.
    blocking = {}
    for finding in findings:
        blocking.setdefault(finding.get("path"), set()).add(finding.get("kind"))
    flagged = set()
    for path, kinds in blocking.items():
        if kinds == {"COMMIT-NOT-ON-GITHUB"} and branch_pr_is_merged(
            git("rev-parse", "--abbrev-ref", "HEAD", repo=path) if os.path.isdir(path) else ""
        ):
            continue
        flagged.add(path)
    root = os.path.realpath(repo)
    main_tree = None
    for path in worktrees(repo):
        if git("rev-parse", "--abbrev-ref", "HEAD", repo=path) == "main":
            main_tree = os.path.realpath(path)

    now = time.time()
    reaped = []
    for path in worktrees(repo):
        real = os.path.realpath(path)
        if real == root or real == main_tree or path in flagged:
            continue
        if not os.path.isdir(path):
            continue
        branch = git("rev-parse", "--abbrev-ref", "HEAD", repo=path) or "HEAD"
        # Only trees `civvis_collab.py start` created. See the docstring: a
        # detached or otherwise-named worktree is somebody's deploy checkout,
        # and this tool has no business inferring that it is finished.
        if not branch.startswith("agent/"):
            continue
        if process_is_running_from(path):
            continue
        head = git("rev-parse", "HEAD", repo=path)
        if not head:
            continue
        reachable = on_github(repo, head)
        if not reachable and not branch_pr_is_merged(branch):
            continue
        if git("status", "--porcelain", repo=path).strip():
            continue
        idle_for = (now - newest_edit(path)) / 60.0
        if idle_for < idle_minutes:
            continue
        row = {"path": path, "branch": branch, "head": head,
               "idle_minutes": idle_for, "removed": False,
               # Two different claims, and a log a person reads should not
               # blur them: "GitHub can reach this commit" is not the same
               # statement as "this branch's pull request merged".
               "why": "HEAD is on GitHub" if reachable
                      else "its pull request merged"}
        if apply:
            # `--force` because a task worktree legitimately holds build output
            # git does not track; the clean check above is the real gate.
            git("worktree", "remove", "--force", path, repo=repo)
            if branch not in ("HEAD", "main"):
                git("branch", "-D", branch, repo=repo)
            row["removed"] = True
        reaped.append(row)
    return reaped


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
        subprocess.run(["git", "-C", work, "worktree", "add", "-q", "-b", "agent/selftest/one/feat", wt],
                       check=True)
        subprocess.run(["git", "-C", wt, "push", "-q", "-u", "origin", "agent/selftest/one/feat"], check=True)

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
            ["git", "-C", remote, "show", "refs/civvis/wip/agent/selftest/one/feat:a.txt"],
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
            ["git", "-C", remote, "show", "refs/civvis/wip/agent/selftest/one/feat:a.txt"],
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
            ["git", "-C", remote, "rev-parse", "refs/civvis/wip/agent/selftest/one/feat"],
            capture_output=True, text=True).stdout.strip()
        assert preserved == local_only, (
            "a clean worktree's local-only HEAD must be pushed verbatim, "
            f"remote has {preserved!r}, worktree has {local_only!r}")

        # And once preserved it must stop being reported, or the audit never
        # converges and a real finding drowns in a permanent false positive.
        after = audit(work, idle_minutes=30, rescue=False)
        assert not any(f["kind"] == "COMMIT-NOT-ON-GITHUB" for f in after), \
            f"a preserved commit must no longer be reported, got {after}"
        # --- the reaper ------------------------------------------------------
        # ⚠ EVERY ASSERTION BELOW IS A REFUSAL. The removal itself is one line
        # of git; what makes this safe to run unattended is the four cases it
        # declines, and a test that only proved it can delete would be a test
        # of the dangerous half.
        #
        # Start from the state the loop above leaves: `wt` is clean and its
        # HEAD is preserved on the remote, so it IS reapable.
        os.utime(os.path.join(wt, "a.txt"), (0, 0))   # make it look idle

        # 1. It refuses without --apply, and says what it would do.
        planned = reap(work, audit(work, 30, False), idle_minutes=30, apply=False)
        assert [os.path.basename(r["path"]) for r in planned] == ["wt"], planned
        assert not planned[0]["removed"], "a dry run must not remove"
        assert os.path.isdir(wt), "a dry run must leave the worktree on disk"

        # 2. It refuses a worktree the audit flagged. Dirty it and it drops out
        #    of the candidate list entirely — the rescue path owns that case and
        #    this must not race it.
        open(os.path.join(wt, "a.txt"), "w").write("one\ntwo\nthree\nfour\n")
        os.utime(os.path.join(wt, "a.txt"), (0, 0))
        assert reap(work, audit(work, 30, False), idle_minutes=30, apply=True) == [], \
            "a dirty worktree must never be reaped"
        assert os.path.isdir(wt), "the dirty worktree must still be there"

        # 3. It refuses a commit GitHub does not have, even when clean.
        subprocess.run(["git", "-C", wt, "add", "-A"], check=True)
        subprocess.run(["git", "-C", wt, *cfg, "commit", "-qm", "unpushed"], check=True)
        os.utime(os.path.join(wt, "a.txt"), (0, 0))
        assert reap(work, audit(work, 30, False), idle_minutes=30, apply=True) == [], \
            "a worktree holding a commit no remote has must never be reaped"
        assert os.path.isdir(wt), "the unpushed worktree must still be there"

        # 4. It refuses a tree an agent touched recently, even when everything
        #    else is safe: HEAD being landed says nothing about whether somebody
        #    is working in the directory right now.
        subprocess.run(["git", "-C", wt, "push", "-q", "origin", "agent/selftest/one/feat"], check=True)
        subprocess.run(["git", "-C", work, "fetch", "-q", "origin"], check=True)
        os.utime(os.path.join(wt, "a.txt"), None)     # touched just now
        assert reap(work, audit(work, 30, False), idle_minutes=30, apply=True) == [], \
            "a worktree edited inside the idle window must never be reaped"
        assert os.path.isdir(wt), "the active worktree must still be there"

        # 5. And with all four satisfied it removes the worktree AND its branch.
        os.utime(os.path.join(wt, "a.txt"), (0, 0))
        done = reap(work, audit(work, 30, False), idle_minutes=30, apply=True)
        assert [r["removed"] for r in done] == [True], done
        assert not os.path.isdir(wt), "the reaped worktree must be gone from disk"
        heads_left = git("for-each-ref", "--format=%(refname:short)", "refs/heads",
                         repo=work)
        assert "agent/selftest/one/feat" not in heads_left.split(), \
            f"the reaped branch must be gone too, heads are {heads_left!r}"
        # ⚠ And the work must still be reachable — that is the whole premise.
        assert subprocess.run(["git", "-C", remote, "rev-parse", "refs/heads/agent/selftest/one/feat"],
                              capture_output=True).returncode == 0, \
            "reaping must never be the last copy: the remote still has the branch"

        # --- the merged-PR escape ------------------------------------------
        # ⚠ Every case here is a REFUSAL except the last. The escape exists so
        # `ship` scaffolding does not pin a worktree forever; it must not
        # become a way for a merged PR to justify deleting anything else.
        import unittest.mock as _mock

        wt2 = os.path.join(tmp, "wt2")
        subprocess.run(["git", "-C", work, "worktree", "add", "-q", "-b", "agent/selftest/two/feat2", wt2],
                       check=True)
        subprocess.run(["git", "-C", wt2, "push", "-q", "-u", "origin", "agent/selftest/two/feat2"], check=True)
        # A commit that exists only here — exactly what `ship`'s merge leaves.
        open(os.path.join(wt2, "a.txt"), "w").write("scaffold\n")
        subprocess.run(["git", "-C", wt2, "add", "-A"], check=True)
        subprocess.run(["git", "-C", wt2, *cfg, "commit", "-qm", "ship merge"], check=True)
        os.utime(os.path.join(wt2, "a.txt"), (0, 0))

        # 1. With no merged PR it is refused, which is today's behaviour.
        with _mock.patch(__name__ + ".branch_pr_is_merged", return_value=False):
            assert reap(work, audit(work, 30, False), idle_minutes=30, apply=True) == [], \
                "a disk-only commit with no merged PR must never be reaped"
        assert os.path.isdir(wt2), "it must still be there"

        # 2. A merged PR does NOT excuse a dirty tree.
        open(os.path.join(wt2, "b.txt"), "w").write("uncommitted\n")
        os.utime(os.path.join(wt2, "b.txt"), (0, 0))
        with _mock.patch(__name__ + ".branch_pr_is_merged", return_value=True):
            assert reap(work, audit(work, 30, False), idle_minutes=30, apply=True) == [], \
                "a merged PR must not excuse uncommitted work"
        assert os.path.isdir(wt2), "the dirty tree must still be there"
        os.remove(os.path.join(wt2, "b.txt"))

        # 3. A merged PR does NOT excuse a tree an agent is still editing.
        os.utime(os.path.join(wt2, "a.txt"), None)
        with _mock.patch(__name__ + ".branch_pr_is_merged", return_value=True):
            assert reap(work, audit(work, 30, False), idle_minutes=30, apply=True) == [], \
                "a merged PR must not excuse an active worktree"
        assert os.path.isdir(wt2), "the active tree must still be there"

        # 4. Clean, idle, and its PR merged: the scaffolding goes.
        os.utime(os.path.join(wt2, "a.txt"), (0, 0))
        with _mock.patch(__name__ + ".branch_pr_is_merged", return_value=True):
            done2 = reap(work, audit(work, 30, False), idle_minutes=30, apply=True)
        assert [r["removed"] for r in done2] == [True], done2
        assert not os.path.isdir(wt2), "the scaffolding worktree must be gone"

        # 5. And the real predicate is conservative about anything it cannot
        #    establish: no branch, no `gh`, no answer all mean "do not reap".
        assert not branch_pr_is_merged(""), "an empty branch is never merged"
        assert not branch_pr_is_merged("main"), "main is never a task branch"
        assert not branch_pr_is_merged("no-such-branch-anywhere"), \
            "an unknown branch must not read as merged"

        # ⚠ AND THE REFUSAL THAT COST AN OUTAGE. A detached worktree pinned
        # to a remote branch is a deploy checkout, not task scaffolding, and it
        # passes every other check here: clean, idle, HEAD plainly on the
        # remote. Removing one took civvis.ai's exhibition down on 2026-08-18.
        deploy = os.path.join(tmp, "deploy")
        subprocess.run(["git", "-C", work, "worktree", "add", "-q", "--detach",
                        deploy, "origin/main"], check=True)
        planned_deploy = reap(work, audit(work, 30, False),
                              idle_minutes=0, apply=False)
        assert all(os.path.basename(r["path"]) != "deploy" for r in planned_deploy), \
            f"a detached deploy checkout must never be a reap candidate: {planned_deploy}"

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
    ap.add_argument("--reap", action="store_true",
                    help="list worktrees whose work GitHub already has and which "
                         "are therefore safe to remove; add --apply to remove them")
    ap.add_argument("--apply", action="store_true",
                    help="with --reap, actually remove. Off by default: a "
                         "destructive default is how a tool like this ends up famous")
    ap.add_argument("--no-fetch", action="store_true",
                    help="skip the fetch; only for tests, a stale ref voids the run")
    ap.add_argument("--scan-root", default=None,
                    help="directory holding the loose CIVVIS files and non-git "
                         "directories to check for content on no git object "
                         "(default: the repo's parent)")
    ap.add_argument("--scan-glob", action="append", default=None, metavar="GLOB",
                    help="which entries of --scan-root to scan; repeatable "
                         "(default: civvis-*)")
    ap.add_argument("--scan-depth", type=int, default=SCAN_DEPTH,
                    help="how far below each scanned entry to walk; the count of "
                         "directories the limit pruned is always reported")
    ap.add_argument("--no-scan", action="store_true",
                    help="worktrees only. ⚠ That is the check that was blind on "
                         "2026-08-22: every byte at risk was outside a worktree")
    ap.add_argument("--quiet", action="store_true", help="print only problems")
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args()

    if args.selftest:
        return selftest()

    if not args.no_fetch and not fetch(args.repo):
        print("FETCH FAILED — refusing to report on stale refs", file=sys.stderr)
        return 1

    scan_root = None if args.no_scan else (
        args.scan_root or os.path.dirname(os.path.abspath(args.repo)))
    findings = audit(args.repo, args.idle_minutes, args.rescue,
                     scan_root=scan_root, scan_patterns=args.scan_glob,
                     scan_depth=args.scan_depth)
    # Abandoned work first: that is the class that gets lost.
    order = {"DIRTY-ABANDONED": 0, "STRANDED-ON-DISK": 1, "COMMIT-NOT-ON-GITHUB": 2,
             "MISSING": 3, "DIRTY-ACTIVE": 4, "SCAN-BOUNDS": 5}
    for f in sorted(findings, key=lambda f: order.get(f["kind"], 9)):
        # A worktree is known by its directory name, but a scanned directory
        # needs its path: basename alone prints "runs", "Default" and "martin",
        # which name nothing a person can act on.
        if f["kind"] in ("STRANDED-ON-DISK", "SCAN-BOUNDS") and scan_root:
            label = os.path.relpath(f["path"], scan_root)
            label = "." if label == "." else label
        else:
            label = os.path.basename(f["path"])
        print(f"{f['kind']}: {label} — {f['detail']}")

    # An ACTIVE dirty tree that was snapshotted is not a problem: an agent is
    # working and its bytes are already on GitHub. Everything else needs a person.
    unresolved = [f for f in findings
                  if not (f["kind"] == "DIRTY-ACTIVE" and f.get("saved"))
                  and not (f["kind"] == "STRANDED-ON-DISK" and f.get("saved"))
                  and f["kind"] != "SCAN-BOUNDS"]
    if args.reap:
        reaped = reap(args.repo, findings, args.idle_minutes, args.apply)
        verb = "removed" if args.apply else "would remove"
        for row in reaped:
            print(f"REAP: {verb} {os.path.basename(row['path'])} "
                  f"({row['branch'][:60]}, idle {row['idle_minutes']:.0f} min, "
                  f"{row['head'][:8]}: {row['why']})")
        if reaped:
            print(f"{verb} {len(reaped)} worktree(s) whose work GitHub already has"
                  + ("" if args.apply else "; re-run with --apply"))
        else:
            print("nothing to reap")

    if not unresolved:
        if not args.quiet:
            where = f"{len(worktrees(args.repo))} worktree(s)"
            if scan_root:
                where += f" and the loose files under {scan_root}"
            print(f"nothing exists only on this disk ({where} checked)")
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
