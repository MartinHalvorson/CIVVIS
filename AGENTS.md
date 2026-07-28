# CIVVIS agent instructions

These rules apply to every human and automated coding agent in this repository.
Read [docs/VERSION_CONTROL.md](docs/VERSION_CONTROL.md) before changing files.

## Operating mandate

- Operate using your best judgment. Once a user authorizes a task, standing
  approval covers the routine, safe, and reversible work needed to finish it:
  investigation, implementation, validation, overlap coordination, PR metadata,
  straightforward conflict resolution, CI repair or retry, and shipping.
- Do not pause for confirmation merely because a generic workflow recommends
  approval. When the user's intent is clear and the action stays within task
  scope, make the best supported decision, verify it, and continue.
- Favor prompt integration. As soon as the change is sound, the branch is
  current, validations are accurate, and required checks are green, run `ship`
  and stay with it through merge. Fix routine CI or policy failures and retry
  without waiting for the user.
- Best judgment does not override genuine stop conditions: destructive or
  difficult-to-recover actions, new credentials or permissions, a material
  expansion beyond the requested outcome, an unresolved product choice, or a
  semantic conflict whose intended resolution is unclear.

After cloning on every computer, install the repository guard once:

```bash
python3 tools/civvis_collab.py install-hooks
```

The launcher refreshes it on every task. Never bypass it with `--no-verify`.

Start every task with the repository launcher; it creates the isolated
worktree, globally unique branch, checkpoint, and draft ownership PR:

```bash
python3 tools/civvis_collab.py start <task-slug> --machine <machine-id> \
  --agent <agent-id> --path <owned-path-or-glob>
```

## Mandatory Git isolation

- Treat `main` as read-only. Never develop on it or push to it directly.
- Every task gets a new branch and a separate Git worktree created from the
  latest `origin/main`.
- A branch and its worktree have exactly one writer. Do not let two agents,
  processes, or computers edit the same branch or worktree concurrently.
- Use a globally unique branch name:
  `agent/<machine-id>/<agent-id>/<task>-<UTC timestamp>-<nonce>`.
  Never reuse a branch for another task or PR.
- Open a draft PR before substantial editing. Its ownership block must name
  the machine, agent, task, and exact claimed paths/globs. The launcher writes
  this block for you.
- Sharing a file with another PR is normal and is not a violation. You must
  coordinate only when you rewrite the same *lines* as another open PR: agree in
  the older PR, then add its number to `Coordinated with:`. Collisions are
  reported as notices on drafts and only block once a PR is marked ready. See
  [docs/collaboration-policy.md](docs/collaboration-policy.md).
- If work moves to another computer or agent, record an explicit handoff in the
  draft PR, push the current commit, and stop the old writer before the new one
  starts.

## Safe Git behavior

- Before editing, run `git status --short --branch` and confirm that the branch
  is neither `main` nor another task's branch.
- Preserve dirty work you did not create. Never reset, discard, stash, stage,
  commit, or move another writer's changes.
- Stage exact paths with `git add -- <paths>`. Do not use `git add -A` in a
  shared repository.
- Make descriptive checkpoint commits and push them to the task branch. A push
  is the cross-machine backup and handoff mechanism; `git stash` is not.
- Do not force-push. Do not rebase a branch after it has been pushed. To update
  a task branch, fetch and merge `origin/main` into it once, resolve carefully,
  rerun validation, and push normally.
- Never run a repository-wide daemon that stages, commits, pulls, rebases,
  merges, or pushes development work. Automated builds may fetch and use a
  private detached worktree based on `origin/main`; they must not mutate a
  development checkout.
- Do not perform broad formatting, generated-file rewrites, or unrelated
  cleanup in a feature PR. CIVVIS has several large conflict hotspots, notably
  `src/game.rs`, `src/ai.rs`, and `web/index.html`.

## Integration

- Keep tasks small and single-purpose. Split independent work into independent
  branches and PRs.
- Before marking a PR ready, merge the latest `origin/main`, run the validation
  required by `CONTRIBUTING.md`, and record the results in the PR.
- A finished feature is not finished at a local commit or draft PR. Once the
  implementation works, the worktree is clean, the PR summary is final, and
  every validation checkbox is accurate, run:

  ```bash
  python3 tools/civvis_collab.py ship
  ```

  Stay with the command until it reports the squash merge and, on a production
  spectator host, the live revision. If it stops on a conflict, failed check,
  or permission boundary, resolve or report that concrete blocker; never leave
  completed work waiting silently in a draft PR.
- Merge only through a green PR using squash merge. Delete the remote task
  branch after merge and remove the local worktree.
- If a conflict is semantic or ownership is unclear, stop and coordinate. Do
  not resolve a whole file with `--ours` or `--theirs` merely to make Git pass.
