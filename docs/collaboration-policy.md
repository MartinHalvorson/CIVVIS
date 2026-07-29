# Collaboration policy

This is the contract the `collaboration-policy` CI check enforces on every pull
request. `tools/civvis_collab.py` is the only implementation; this document
explains it. If the two disagree, the code is right and this file is a bug.

## The short version

1. Start every task with the launcher. Do not hand-build a branch.
2. Change only the paths you claimed.
3. Before you mark a PR ready, merge current `origin/main` and tick every box.
4. You may edit the same file as another PR. You may not edit the same **lines**
   without saying so.

## Starting a task

```bash
python3 tools/civvis_collab.py start <task-slug> \
  --agent <agent-id> --path <path-or-glob>
```

This creates the worktree, the uniquely named branch, and the draft PR with a
correctly formatted ownership block. Everything the check wants is already
filled in. You do not need to write the PR body by hand.

If another open PR already claims one of your paths, the launcher prints a note
and records that PR under `Coordinated with:` automatically. It does **not**
refuse to start. Shared files are normal in this repository.

## What the check actually enforces

| Rule | When it blocks | How to fix |
| --- | --- | --- |
| Branch name matches `agent/<machine>/<agent>/<task>-<stamp>-<nonce>` | always | start a new task with the launcher; never rename a branch |
| Ownership block fields are filled | always | the launcher fills them; do not delete lines from the PR body |
| Machine and agent IDs match the branch | always | do not edit those two lines |
| Every changed file is covered by `Claimed paths:` | always | revert the stray file, or add it to `Claimed paths:` |
| No `autosync:` commits | always | those commits are machine backups; never merge them into a task |
| Your edits do not collide with another open PR | **ready PRs only** | see below |
| Every validation checkbox is ticked | ready PRs only | run each check, then change `- [ ]` to `- [x]` |

Being behind `main` is **not** a policy failure. It is reported as a notice.
GitHub's branch protection runs with `strict = true`, so it refuses the merge
itself until you update, and `ship` re-merges `main` on its own whenever it sees
the base move. Failing the check as well only painted a red X on a PR whose
author had done nothing wrong and could not durably fix, because `main` moves
again while CI runs.

Merging something that *was* behind `main` is still a hard error — the
post-merge auditor catches it. The rule is enforced where it can be satisfied,
not continuously against a moving target.

## Overlap: files versus lines

Two PRs **share a file** whenever they both change it. That is expected and is
never an error. `web/index.html`, `src/game.rs`, and `src/ai.rs` are edited by
most tasks.

Two PRs **collide** when they rewrite the same lines, within three lines of
context — the amount Git needs to merge cleanly. Only a collision is a problem.

The check reports the difference explicitly:

- `::notice::PR #123 edits the same file(s) in different places, no action needed`
  Nothing to do. Git will merge both.
- `::error::edits collide with PR #123 on the same lines of web/index.html`
  Real conflict. Resolve it.

While your PR is a **draft**, a collision is reported as a notice and does not
fail the run. It only blocks once you mark the PR ready.

### Resolving a collision

Coordinate in the **older** PR — its author started first. Then either:

- change your approach so you no longer touch those lines, or
- add the other PR's number to the `Coordinated with:` line of your PR body,
  which records that both authors know and have agreed.

```
- Coordinated with: #123
```

`Coordinated with: none` is valid and is the normal state.

You do not need to list every PR that touches your files. You only need to list
the ones you genuinely collide with. That list stays short and does not go stale
when unrelated PRs open or merge.

If a file's diff cannot be read — binary content, or a diff GitHub truncated —
the check falls back to treating the whole file as a collision. That is
deliberate: the policy is strict exactly where it cannot see detail.

## Finishing

```bash
python3 tools/civvis_collab.py ship
```

Stay with it until it reports the squash merge. If it stops on a conflict, a
failed check, or a permission boundary, fix that specific blocker or report it.
Do not leave finished work sitting in a draft PR.

## Why it is built this way

An earlier version required each PR to name *every* other open PR that touched
any of its files. That list was a snapshot of a set that changed every time any
PR opened or merged, so a compliant PR became non-compliant without its author
doing anything, and older PRs were penalised hardest. With twelve open PRs on
one file it demanded 132 declarations that could never all be correct at once.

Line-level collision detection replaced it because it asks a question an author
can actually answer from their own diff, and the answer stays true until that
author changes something.
