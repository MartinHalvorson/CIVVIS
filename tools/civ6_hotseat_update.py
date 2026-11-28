#!/usr/bin/env python3
"""Bring the hotseat play copy up to main without losing a fix, and say whether
the decider needs building.

    civ6_hotseat_update.py --checkout ~/civvis-hotseat-main            # exit 10: build the decider
    civ6_hotseat_update.py --checkout ~/civvis-hotseat-main --built    # after the build: stamp it
    civ6_hotseat_update.py --checkout ~/civvis-hotseat-main --dry-run  # say what would happen

The person's games run from a private checkout (`~/civvis-hotseat-main`) that
the wrapper's `update` moved with `git checkout --detach origin/main`, and then
always rebuilt the decider (~7 minutes, and a build cannot share the Mac with
Civ VI). On 2026-11-27 a fix applied there by hand, so the person could play at
once (the #5998 seating fix), was reverted with `git checkout -- <file>` before
the play copy was moved to a main that had it; the next launch
(hotseat-20261128T031456Z) seated the lobby without it and the person waited
ten minutes for turn 1.

What this does, in order:

1. Fetches origin's main.
2. Saves every local change to a tracked file as a patch (`--patches`, default
   `~/civvis-hotseat/patches/<utc>-<revision>.patch`) before anything moves, so
   no change is ever lost, and asks git to merge it onto main in memory
   (`git merge-tree`): already in main (main made the same change): dropped,
   and said; merges cleanly: kept, put back on top of main after the move, and
   said; conflicts with main: nothing moves, exit 2, the files and the patch
   are named.
3. Moves the checkout to main (detached), and says what came with it: the
   commits that touch the hotseat first, then how many others.
4. Says whether the decider needs building: only when something it is compiled
   from changed since it was last built. That is `src/`, `Cargo.toml`,
   `Cargo.lock`, `build.rs`, and every file `src/` names in an `include_str!`
   or `include_bytes!` (`docs/gene_ledger.json`, `deploy/live-force-on.txt`,
   ...), compared from the revision stamped beside the binary by `--built`
   (`civvis_orders.revision`), local changes included. No stamp, no binary: a
   build. Exit 10 means build it, then run this again with `--built`.

Exit codes: 0 done and nothing to build; 10 build the decider; 2 refused
(a local change main conflicts with, or git failed); nothing moved then.
Standard library only, so the wrapper can run the newest copy of this file
straight from origin/main (`git show origin/main:tools/civ6_hotseat_update.py`).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import posixpath
import re
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

#: Exit codes.
DONE, BUILD, REFUSED = 0, 10, 2
#: What the decider is compiled from, besides what `src/` includes.
COMPILED = ("Cargo.toml", "Cargo.lock", "build.rs", "rust-toolchain", "rust-toolchain.toml")
#: A file `src/` compiles in: `include_str!("../../docs/x.json")`.
INCLUDE = re.compile(r'include_(?:str|bytes)!\(\s*"([^"]+)"\s*\)')
#: The commits a person's game feels, listed first in "what changed".
HOTSEAT_PATHS = ("tools/civ6_hotseat.py", "tools/civ6_hotseat_update.py", "tools/civ6_control",
                 "tools/civ6_brain.py", "tools/civ6_play.py", "tools/people_elo.py", "deploy")
#: How many commits are named before the rest are counted.
SHOWN = 15
BINARY = Path("target") / "release" / "civvis_orders"


class GitError(RuntimeError):
    pass


def git(repo: Path, *args: str, env: dict | None = None, ok: tuple[int, ...] = (0,),
        input_text: str | None = None) -> tuple[int, str]:
    done = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True,
                          env={**os.environ, **(env or {})}, input=input_text, check=False)
    if done.returncode not in ok:
        raise GitError(f"git {' '.join(args)}: {(done.stderr or done.stdout).strip()}")
    return done.returncode, done.stdout


def out(repo: Path, *args: str) -> str:
    return git(repo, *args)[1].strip()


def changed_files(repo: Path) -> list[str]:
    """Tracked files that differ from HEAD (staged or not)."""
    return [line for line in out(repo, "diff", "--name-only", "HEAD").splitlines() if line]


def save_patch(repo: Path, patches: Path, head: str) -> Path:
    patches.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    path = patches / f"{stamp}-{head[:8]}.patch"
    path.write_text(git(repo, "diff", "--binary", "HEAD")[1])
    return path


def merge_local(repo: Path, target: str) -> tuple[str, str | None]:
    """The local changes against `target`, merged in memory: ("none", None)
    with no change; ("in-main", None) when the merge is `target` itself;
    ("keep", tree) when it merges cleanly into something more; ("conflict",
    None) otherwise."""
    if not changed_files(repo):
        return "none", None
    local = out(repo, "stash", "create")
    if not local:
        return "none", None
    code, said = git(repo, "merge-tree", "--write-tree", target, local, ok=(0, 1))
    tree = said.split("\n", 1)[0].strip()
    if code != 0:
        return "conflict", None
    if tree == out(repo, "rev-parse", f"{target}^{{tree}}"):
        return "in-main", None
    return "keep", tree


def compiled_inputs(repo: Path, revision: str) -> set[str]:
    """Every non-`src/` file `src/` compiles in at `revision`, repository-relative."""
    code, said = git(repo, "grep", "-I", "-E", r"include_(str|bytes)!\(", revision, "--", "src",
                     ok=(0, 1))
    found: set[str] = set()
    for line in said.splitlines() if code == 0 else []:
        # `<revision>:<path>:<text>`
        try:
            _, path, text = line.split(":", 2)
        except ValueError:
            continue
        for named in INCLUDE.findall(text):
            resolved = posixpath.normpath(posixpath.join(posixpath.dirname(path), named))
            if not resolved.startswith("../"):
                found.add(resolved)
    return found


def is_compiled(path: str, includes: set[str]) -> bool:
    return path.startswith("src/") or path in COMPILED or path in includes


def local_hash(repo: Path, includes: set[str]) -> str | None:
    """The local changes to what the decider is compiled from, as a hash
    (None when there are none): part of what a build is stamped with."""
    files = [path for path in changed_files(repo) if is_compiled(path, includes)]
    if not files:
        return None
    diff = git(repo, "diff", "--binary", "HEAD", "--", *files)[1]
    return hashlib.sha256(diff.encode()).hexdigest()[:16]


def stamp_path(repo: Path, binary: Path) -> Path:
    binary = binary if binary.is_absolute() else repo / binary
    return binary.with_name(binary.name + ".revision")


def build_needed(repo: Path, binary: Path, head: str) -> tuple[bool, str]:
    """Whether the decider must be built for the checkout as it stands, and why."""
    full = binary if binary.is_absolute() else repo / binary
    if not full.is_file():
        return True, f"there is no decider at {full}"
    try:
        stamp = json.loads(stamp_path(repo, binary).read_text())
        built = str(stamp["revision"])
    except (OSError, ValueError, KeyError, TypeError):
        return True, "the decider's revision is not recorded (no stamp from `--built`)"
    includes = compiled_inputs(repo, head)
    code, said = git(repo, "diff", "--name-only", built, head, ok=(0, 128))
    if code != 0:
        return True, f"the revision the decider was built from ({built[:8]}) is not in this checkout"
    changed = [path for path in said.splitlines() if is_compiled(path, includes)]
    if changed:
        shown = ", ".join(changed[:6]) + (f" and {len(changed) - 6} more" if len(changed) > 6 else "")
        return True, f"{len(changed)} file(s) it is compiled from changed since {built[:8]}: {shown}"
    if local_hash(repo, includes) != stamp.get("local"):
        return True, "local changes to what it is compiled from differ from the build's"
    return False, f"nothing it is compiled from changed since it was built at {built[:8]}"


def what_changed(repo: Path, old: str, new: str) -> list[str]:
    if old == new:
        return []
    lines = []
    hotseat = out(repo, "log", "--no-merges", "--format=%h %s", f"{old}..{new}", "--",
                  *HOTSEAT_PATHS).splitlines()
    every = out(repo, "log", "--no-merges", "--format=%h", f"{old}..{new}").splitlines()
    if hotseat:
        lines.append(f"for the hotseat ({len(hotseat)}):")
        lines += [f"  {line[:100]}" for line in hotseat[:SHOWN]]
        if len(hotseat) > SHOWN:
            lines.append(f"  ... and {len(hotseat) - SHOWN} more")
    others = len(every) - len(hotseat)
    if others > 0:
        lines.append(f"and {others} other commit(s) (`git log {old[:8]}..{new[:8]}`)")
    return lines


def update(repo: Path, patches: Path, binary: Path, *, remote: str = "origin", branch: str = "main",
           dry_run: bool = False, say=print) -> int:
    repo = repo.expanduser()
    git(repo, "fetch", "-q", remote, branch)
    target = out(repo, "rev-parse", f"{remote}/{branch}")
    old = out(repo, "rev-parse", "HEAD")
    files = changed_files(repo)
    state, tree = merge_local(repo, target)
    patch = None
    if files:
        patch = save_patch(repo, patches, old)
        say(f"local changes to {', '.join(files)}: saved as {patch}")
    if state == "conflict":
        say(f"⚠ those changes conflict with {remote}/{branch} ({target[:8]}), so the checkout "
            f"stays at {old[:8]} with them in place. Bring them into a pull request, or drop "
            f"them (`git -C {repo} checkout -- <file>`) once main has the fix; then update again.")
        return REFUSED
    if state == "in-main":
        say(f"{remote}/{branch} already has them: they are dropped as the checkout moves")
    elif state == "keep":
        say(f"{remote}/{branch} does not have them: they are kept, on top of {target[:8]}")
    if dry_run:
        needed, why = build_needed(repo, binary, target) if state != "keep" else (True, "local changes")
        say(f"would move {old[:8]} -> {target[:8]}; decider build: "
            f"{'yes' if needed else 'no'} ({why})")
        for line in what_changed(repo, old, target):
            say(line)
        return BUILD if needed else DONE
    if old != target or state != "none":
        git(repo, "checkout", "-q", "-f", "--detach", target)
        if state == "keep":
            diff = git(repo, "diff", "--binary", target, tree)[1]
            git(repo, "apply", "--whitespace=nowarn", "-", input_text=diff)
    head = out(repo, "rev-parse", "HEAD")
    say(f"checkout at {out(repo, 'log', '-1', '--format=%h %s')[:100]}"
        + (" (was " + old[:8] + ")" if old != head else " (already there)"))
    for line in what_changed(repo, old, head):
        say(line)
    needed, why = build_needed(repo, binary, head)
    say(f"decider build: {'needed' if needed else 'not needed'}: {why}")
    return BUILD if needed else DONE


def built(repo: Path, binary: Path, say=print) -> int:
    """Stamp the decider just built with what it was built from."""
    repo = repo.expanduser()
    head = out(repo, "rev-parse", "HEAD")
    full = binary if binary.is_absolute() else repo / binary
    if not full.is_file():
        say(f"no decider at {full}: nothing stamped")
        return REFUSED
    stamp = {"revision": head, "local": local_hash(repo, compiled_inputs(repo, head)),
             "utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")}
    stamp_path(repo, binary).write_text(json.dumps(stamp))
    say(f"decider stamped as built from {head[:8]}"
        + (" with local changes" if stamp["local"] else ""))
    return DONE


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0],
                                 formatter_class=argparse.RawDescriptionHelpFormatter,
                                 epilog="\n\n".join(__doc__.split("\n\n")[1:]))
    ap.add_argument("--checkout", type=Path, required=True, help="the play copy")
    ap.add_argument("--patches", type=Path,
                    default=Path.home() / "civvis-hotseat" / "patches",
                    help="where local changes are saved (default %(default)s)")
    ap.add_argument("--binary", type=Path, default=BINARY,
                    help="the decider, relative to the checkout (default %(default)s)")
    ap.add_argument("--remote", default="origin")
    ap.add_argument("--branch", default="main")
    ap.add_argument("--built", action="store_true",
                    help="stamp the decider just built; run after a successful build")
    ap.add_argument("--dry-run", action="store_true", help="say what would happen; move nothing")
    args = ap.parse_args(argv)
    try:
        if args.built:
            return built(args.checkout, args.binary)
        return update(args.checkout, args.patches, args.binary, remote=args.remote,
                      branch=args.branch, dry_run=args.dry_run)
    except GitError as error:
        print(f"⚠ {error}; the checkout is left as it was", file=sys.stderr)
        return REFUSED


if __name__ == "__main__":
    raise SystemExit(main())
