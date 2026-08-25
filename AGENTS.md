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

After cloning on every computer, bootstrap the clone once:

```bash
python3 tools/civvis_collab.py bootstrap
```

This installs the repository guard and every managed background service —
the five-minute Git synchronization service, the memory guard, and on a
Civilization VI seat the ladder keeper. The synchronization service
force-updates the dedicated, clean `main` management worktree to GitHub
`origin/main` while leaving task worktrees untouched. The task launcher
repairs **all** of them on every task, which is what makes a service added
after a machine was bootstrapped reach that machine at all: `start` used to
repair only the two safeguards that existed when that line was written, and
the ladder keeper was consequently absent from a Civilization VI seat.
Never bypass the guard with `--no-verify`.

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
- Name the computer in every commit. End the message with a `Computer:` trailer
  carrying this device's own name (`scutil --get ComputerName` on macOS,
  `hostname` elsewhere), so `git log` alone says which machine in the fleet
  produced a commit. The device name is deliberately the whole identifier for
  now; a richer computer ID can replace the value later without moving it. Put
  the same `- Computer:` line in the PR ownership block too — squash merge
  writes the PR body onto `main` and throws the branch's own messages away.
- Do not force-push. Do not rebase a branch after it has been pushed. To update
  a task branch, fetch and merge `origin/main` into it once, resolve carefully,
  rerun validation, and push normally.
- Never run a repository-wide daemon that stages, commits, rebases, or pushes
  development work. Automated builds may fetch and use a private detached
  worktree based on `origin/main`; they must not mutate a development checkout.
- The managed Git synchronization service fetches GitHub and force-aligns only
  the dedicated `main` management worktree to exact `origin/main`. It never
  changes task branches or their files. It refuses to overwrite a dirty `main`;
  before repairing a clean divergent `main`, it preserves the old commit under
  `refs/civvis/recovery/main/`. `start` refuses to create a task unless this
  synchronization has succeeded.
- Do not perform broad formatting, generated-file rewrites, or unrelated
  cleanup in a feature PR. CIVVIS has several large conflict hotspots, notably
  `src/game.rs`, `src/ai.rs`, and `web/index.html`.

## A claim is not a check

The defects that survive longest here are not wrong code. Both suites stay
green; something simply asserts a fact that nothing verifies.

- **A guard you add runs in the same change that adds it.** `civvis_inert.py`
  documented `--max 0  # CI ratchet` in its own usage from the day it was
  written and no workflow called it. What it would have reported for that whole
  time: Poland's Winged Hussar carried `force_retreat` in `data/units.json`, the
  engine read the key nowhere, and the unit shipped as a plain Cuirassier with
  no unique ability (#1941, #1943). `tools/test_ci_wiring.py` now fails when a
  tool's own header claims CI use and no workflow runs it — wire it, or record
  in `CANNOT_RUN_IN_CI` why a runner cannot.
- **A sentence that states a fact is enforced by a test, or it is a guess with
  good posture.** `civ6_fidelity.py` printed "(Gathering Storm load order)"
  unconditionally and reported 210 divergences against a Vanilla database;
  acting on that report would have rewritten correct expansion values to vanilla
  ones (#1946). `FLOAT_DETERMINISM.md` recorded `mapgen.rs` as fully converted
  while ten platform-trig calls sat in the *default* map script (#1950).
- **A setting you send is read back from wherever it lands.** `civ6_play.py`
  verified difficulty, size, speed, map, leader and modes from inside the
  running game and took `--ruleset` on trust — the one axis with no reading that
  could be wrong (#1947).
- **A gate covers the default configuration.** The determinism test pinned
  `MapScript::Continents` while the CLI defaults to `tennis_ball`, so the check
  that exists to keep platform trig out of mapgen could not see the map most
  games are played on (#1950).
- **A live-game interaction is modelled on a quoted line, not on a memory of
  the game.** A PR that models or drives something the real game does — it
  touches `tools/civ6_control/mod/`, or the deal, diplomacy, barbarian or
  religion code in `src/game.rs` — quotes in its body the shipped script line
  or database row it is modelled on: `Base/Assets/UI/DiplomacyActionView.lua:2545`
  (`MakeDeal_ApplyStatement`), `GameplayDB.BarbarianAttackForces`. That is
  how a working deal was found to be evaluated only inside a `MAKE_DEAL`
  session, a gift to be legal and buy nothing, and the barbarian bands to
  break at Chieftain/Warlord and Emperor/Immortal — none of which the
  green rows in `docs/MECHANICS.md` knew. `tools/civ6_scripts.py grep
  <regex>` finds the line on a fleet Mac; `check-pr` posts a notice, never a
  block, when such a PR carries no citation.
- **Discover, never list.** A hand-written list of files to check is complete
  the day it is written and silently shrinks afterwards. It has cost this
  repository three times: twenty-five ungated `tools/` suites, one of `beta/`'s
  six scripts broken for two separate reasons (#1905), and the mod tests
  (#1941). Glob them, and fail when the glob comes up empty.

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
- **While a PR's checks are in flight, leave its head alone**: do not push to
  it, merge `main` into it, or edit its body. Every refreshed head cancels the
  in-flight CI run and restarts the ~7-minute gate; chasing a busy trunk this
  way once killed 44% of a day's CI runs and kept PRs from converging at all.
  `main` advancing while you wait is normal and safe: `strict` is `false`, a
  green run auto-merges even a few commits behind, and the push to `main`
  reruns the full gate on the actual squash result. Merge `main` once before
  marking ready, and again only for a real conflict or when `ship` reports the
  branch past the staleness limit.
- Merge only through a green PR using squash merge. Delete the remote task
  branch after merge and remove the local worktree.
- **Never leave work only in a worktree.** Uncommitted changes in a worktree
  exist on exactly one disk, and this fleet has lost them: four PRs once read
  `+0/-0` on GitHub while their finished implementations sat unstaged locally,
  and two were closed as abandoned before anyone looked. Commit and push before
  you stop, even mid-task — a WIP commit on your own branch is always cheaper
  than the work disappearing. `tools/civvis_worktree_audit.py --rescue` is the
  backstop, not the plan; it snapshots dirty worktrees to `wip/<branch>` so
  nothing is lost, but a `wip/` ref is a rescue, not a contribution.
- **Before deleting a worktree or branch, ask whether GitHub has the content**,
  not whether it was merged. Squash merge rewrites commits, so `git branch
  --merged` and `git cherry` both call long-landed work unlanded; a closed PR
  keeps its content at `refs/pull/N/head` forever, so "closed" does not mean
  lost. The check that answers it, after fetching `+refs/pull/*/head`:

  ```bash
  git for-each-ref --contains <sha> --count=1 refs/remotes   # empty => only copy
  ```
- If a conflict is semantic or ownership is unclear, stop and coordinate. Do
  not resolve a whole file with `--ours` or `--theirs` merely to make Git pass.
