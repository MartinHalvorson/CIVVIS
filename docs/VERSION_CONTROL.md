# Version control for the CIVVIS agent fleet

CIVVIS uses protected, PR-based trunk development. The workflow is designed
for any number of computers and concurrent agents. GitHub is the coordination
boundary: local disks hold isolated worktrees, remote task branches hold
checkpoints, draft PRs advertise ownership, and `main` contains only integrated
work.

## The invariants

1. `origin/main` is the only integration trunk and is always expected to build.
2. One task has one new branch, one worktree, one draft PR, and one active
   writer.
3. No computer automatically commits or integrates a development checkout.
4. Every merge reaches `main` through a current, green PR and is squash-merged.
5. Ownership is visible before substantial editing, not discovered at merge
   time.

These rules solve different problems. Worktrees prevent agents on one computer
from sharing uncommitted state. Unique branches prevent computers from pushing
over each other. Draft PRs reveal likely file overlap. Required CI and a single
trunk serialize integration.

## Bootstrap every clone

Immediately after cloning CIVVIS on any current or future computer, bootstrap
the clone:

```bash
python3 tools/civvis_collab.py bootstrap
```

Bootstrap installs two clone-wide safeguards:

1. The repository-managed pre-push guard is installed in Git's common hooks
   directory, so it covers every linked worktree in that clone.
2. A per-user background service runs `civvis_collab.py refresh --scheduled`
   every five minutes through launchd, a systemd user timer, or Windows Task
   Scheduler. It fetches and prunes GitHub refs, force-updates the dedicated
   `main` management worktree to exact `origin/main`, and writes an atomic
   heartbeat under Git's common directory.

The task launcher refreshes both safeguards automatically before its first
push, and `audit` reports a missing service, a stale heartbeat, or a heartbeat
that has not observed GitHub's current `main` as an error. Never use
`git push --no-verify`.

The background service automatically keeps the stable `main` management
worktree synchronized with GitHub. It updates a clean, behind `main` by
fast-forward. If clean `main` has local-only or divergent commits, it first
preserves the old commit under `refs/civvis/recovery/main/`, then resets the
management worktree to the exact fetched `origin/main`. It refuses to overwrite
a dirty `main` and records that refusal as a synchronization error.

The service never changes an isolated task worktree, stages or commits work, or
pushes any ref. Its heartbeat records the update receipt plus every registered
worktree's exact head, dirty state, and ahead/behind count relative to
`origin/main`. After each fetch the installed worker also replaces itself
atomically from `origin/main`, so already-bootstrapped machines adopt changes to
the synchronization policy without a manual reinstall. Inspect it at any time
with:

```bash
python3 tools/civvis_collab.py refresh
python3 tools/civvis_collab.py refresh --json
```

An offline computer cannot fetch, so its heartbeat becomes stale and the next
`start` fails closed until GitHub is reachable. An online, idle computer keeps
its canonical checkout at GitHub HEAD automatically. A computer actively
starting work has the same guarantee synchronously: `start` fetches, updates
the management worktree, and creates the new task worktree from that exact
`origin/main` SHA.

The guard rejects all pushes or deletions of `main`, new development branches
that do not use the fleet naming convention, and non-fast-forward updates to a
task branch. It permits tags and non-`main` branch deletion so completed and
legacy branches can be cleaned up. This is a local safety boundary, not a
replacement for GitHub branch protection: a user can bypass a hook, and a new
clone has no hook until it is installed. The repository-owner ruleset remains
the authoritative enforcement boundary.

## Identities and names

Give each computer a stable, short machine ID such as `martin-mbp` or
`render-win-02`. It must remain unique as the fleet grows. Give each active
agent/session a short ID such as `codex-47` or `claude-ui-3`.

Branches use:

```text
agent/<machine-id>/<agent-id>/<task>-<YYYYMMDDTHHMMSSZ>-<nonce>
```

Example:

```text
agent/martin-mbp/codex-47/mobile-cinema-20260723T210500Z-a31f
```

The nonce can be four or more random hexadecimal characters. A branch name is
never reused, including after its PR is merged. Avoid persistent branches named
only `fix/foo`, `session-x`, or `agent/name`; their ownership and lifetime are
ambiguous.

Every commit also names the computer that produced it, in a `Computer:` trailer
on the last line:

```text
Computer: mbp-m5-max-128
```

For now that value is simply the device's own name — `scutil --get ComputerName`
on macOS, `hostname` elsewhere — not a curated identifier. It may therefore
differ from the short `<machine-id>` in the branch name, and that is fine: the
branch component is chosen once and stays stable for claims and ownership
checks, while the trailer says which physical machine actually ran the work.
When the fleet outgrows device names, replace the value with a stronger computer
ID and keep the trailer where it is.

The trailer does **not** reach `main` on its own. This repository squash-merges
with `squash_merge_commit_message = PR_BODY`, so the commit that lands on `main`
is titled by the PR title and bodied by the PR description; the branch's own
messages and its `<machine-id>` are discarded with the branch. Repeat the same
line in the PR ownership block so it survives the squash:

```text
- Computer: `mbp-m5-max-128`
```

`Machine ID` in that block stays as it is — `check-pr` compares it to the branch
name literally and fails on anything else. `Computer` is an extra line the
checker ignores.

## Start a task

Start from a stable base checkout used only to manage worktrees. Do not edit in
that checkout. The supported cross-platform launcher verifies the local
freshness service, performs a synchronous fetch, then handles identity
validation, worktree/branch creation, empty claim commit, push, and draft PR
creation:

```bash
python3 tools/civvis_collab.py start mobile-cinema \
  --machine martin-mbp --agent codex-47 \
  --path web/index.html --title "Improve the mobile cinema layout"
```

The launcher records the stable machine ID in repository-local Git config. On
later tasks from that computer, `--machine` may be omitted. If a claim overlaps
an open PR, the launcher refuses to start until coordination is recorded; after
coordinating in the older PR, rerun with `--coordinate <PR-number>`.

The manual equivalent is documented below for recovery and inspection.

```bash
git fetch --prune origin
git worktree add -b \
  agent/martin-mbp/codex-47/mobile-cinema-20260723T210500Z-a31f \
  ../civvis-mobile-cinema-a31f origin/main
cd ../civvis-mobile-cinema-a31f
git status --short --branch
```

Use equivalent PowerShell commands on Windows; the branch and worktree model is
the same on every operating system. Before editing, inspect active ownership:

```bash
gh pr list --repo MartinHalvorson/CIVVIS --state open \
  --json number,isDraft,headRefName,title,url
```

Create a visible claim immediately. An empty claim commit is acceptable because
the PR will eventually be squash-merged:

```bash
git commit --allow-empty -m "claim: mobile cinema layout"
git push -u origin HEAD
gh pr create --draft --base main \
  --title "Improve the mobile cinema layout" \
  --body-file .github/pull_request_template.md
```

Edit the PR body immediately. Fill in machine ID, agent ID, task, exact claimed
paths/globs, dependencies, and any overlap with another PR. The template
is a starting form, not a completed claim.

If another open PR owns an overlapping file or subsystem, use one of these
explicit outcomes before working:

- split the tasks so their paths and responsibilities no longer overlap;
- record which agent owns each hunk or interface in both PRs;
- make the later task wait for the earlier PR, then create a fresh branch from
  the new `origin/main`;
- explicitly hand the entire task to one writer.

Silence is not coordination. Starting anyway merely postpones the collision.

## Work and checkpoint

At the beginning and end of each work period:

```bash
git status --short --branch
git diff --check
git add -- path/to/file another/path
git commit -m "Describe one coherent change" \
  -m "Computer: $(scutil --get ComputerName 2>/dev/null || hostname)"
git push
```

Rules during development:

- Stage only the files belonging to the task. Never sweep the worktree with
  `git add -A` or `git add .`.
- Push useful checkpoints before a context switch, shutdown, or handoff. WIP
  commits are fine on a task branch; squash merge keeps `main` concise.
- Do not use a periodic autosync service as a backup. It cannot tell which
  agent owns a change, whether the change is complete, or what commit message
  describes it.
- The managed synchronization timer is not an autosync backup: it updates only
  the canonical `main` management worktree and records task drift. A task
  branch that is behind `main` stays untouched until its owner performs the
  normal reviewed merge from `origin/main`.
- Do not rebase or force-push a published branch. Stable history lets another
  computer resume it safely and makes review comments durable.
- Keep the PR narrow. Unrelated changes get their own branch and claim.
- Avoid whole-repository formatting. Run formatters in check mode unless the
  task explicitly owns the resulting files.

Recommended repository-local Git settings are:

```bash
git config fetch.prune true
git config pull.ff only
git config push.default simple
git config merge.conflictStyle zdiff3
git config rerere.enabled true
git config rerere.autoupdate false
```

`rerere` may offer a previous resolution, but `autoupdate=false` keeps it from
staging that resolution without review.

## Move work between computers or agents

A branch still has one writer even when it moves. The current writer must:

1. commit and push every intended change;
2. post a PR comment containing the last commit SHA and the new machine/agent;
3. stop editing and state that the handoff is complete.

Only then may the new writer create its own worktree:

```bash
git fetch origin
git worktree add --track \
  -b agent/martin-mbp/codex-47/mobile-cinema-20260723T210500Z-a31f \
  ../civvis-mobile-cinema-handoff \
  origin/agent/martin-mbp/codex-47/mobile-cinema-20260723T210500Z-a31f
```

On another clone, retaining the exact remote branch name keeps ordinary pushes
safe. For a handoff inside the same clone, remove the old worktree first, then
attach that existing local branch to the new worktree. Do not leave both writers
active.

## Update and integrate

Do not continually merge `main` into every task. Update once when upstream is
needed or just before the PR becomes ready:

```bash
git fetch origin main
git merge --no-edit origin/main
```

Resolve conflicts by intent. Review all three sides with the `zdiff3` context,
run focused tests, and inspect the final diff. Never accept an entire large file
as `ours` or `theirs` merely to clear the index.

### Generated documents resolve themselves

`docs/eval_manifest.json` and `docs/EVAL_STATUS.md` are written by
`tools/eval_manifest.py`, and every agent registering an evaluator arm appends
to both. On 2026-08-19 they were the **fourth and fifth most-edited paths in
the repository** — 35 and 32 of the 138 commits main took that day — behind
only `advanced.rs`, its tests and `elo.rs`. One pull request hit the same
conflict on four consecutive ship attempts.

Those conflicts have exactly one correct resolution, so `civvis_collab.py ship`
now performs it: when **every** conflicted path is a generated artifact, it
reruns the generator over the merged sources, re-checks it, commits, and
carries on. You do not need to resolve them, and you should not resolve them by
hand — regenerating is what produces the union of both branches' registrations,
where picking a side silently drops one.

The automation is deliberately all-or-nothing. If anything else is conflicted —
including the generator itself — `ship` stops exactly as before and the whole
merge is yours, because regenerating on top of a conflicted source would
publish one side's arms and call it a merge.

⚠ `docs/closed/TACTICS_BASELINE.md` looks like it belongs here and does not.
`tools/tactics_bench.py --write-baseline` runs a benchmark, so its content
depends on the machine that measured it; it must never be regenerated to settle
somebody else's merge. The rule for adding a path to `REGENERATED_ON_MERGE` is
all three of: deterministic in tracked source, cheap to rebuild, and nothing
about it measured.

Before marking the PR ready:

```bash
git diff --check origin/main...
cargo test --profile ci --locked
```

Also run the soak validation required by `CONTRIBUTING.md` for engine changes.
Record exact commands and results in the PR. CI is a required independent gate,
not a substitute for focused local tests.

### Ship completed work immediately

After the feature works and the PR body accurately records its summary and
completed validation, do not leave it in draft for a person or another agent to
notice later. Run this from the task worktree:

```bash
python3 tools/civvis_collab.py ship
```

`ship` is the completion boundary. It merges the latest `main` once (with a
`cargo check` of the merged result), pushes the finished commit, marks the PR
ready, arms squash auto-merge, and waits for `cargo-test` and
`collaboration-policy`. While the gate runs it deliberately **leaves the head
alone**. `main` advancing in the meantime is the normal state of this trunk,
not something to chase: protection runs with `strict: false`, so a green run
merges even when the head trails `main` by a few commits, and the push to
`main` reruns the full gate on the actual squash result. Chasing the trunk is
what used to prevent convergence entirely — every refreshed head cancels the
in-flight CI run and restarts the gate from zero, and on 2026-08-06 that
killed 58 of 133 cargo-test runs (44%). `ship` refreshes the head in exactly
two cases: GitHub reports a real conflict (resolved by a local merge, then
revalidation), or the branch has fallen `STALE_BASE_LIMIT` (150) commits
behind, where a merge would be a tree no CI run has approximated. It never
invents a summary, checks validation boxes, resolves conflicts, or accepts a
failed gate.

The same rule binds every agent and person: **while a PR's checks are in
flight, do not push to it, merge `main` into it, or edit its body.** Each of
those restarts the gate and, at fleet velocity, everyone else's merges too.

On a host running the production spectator, `ship` then watches `/status` until
the merged revision is actually serving. Other hosts stop after confirming the
merge. The normal target is therefore roughly one CI run (currently about five
minutes) from the final push to `main`, followed by the spectator's background
build and checkpointed live cutover.

Use squash merge only. The squash commit is the atomic unit integrated into
`main`; intermediate checkpoints and merge-from-main commits remain out of the
trunk history. Delete the remote branch after merge. Then remove the worktree
from the base checkout:

```bash
git worktree remove ../civvis-mobile-cinema-a31f
git fetch --prune origin
```

Because squash merge does not make the task commit an ancestor of `main`, Git
may refuse safe local deletion with `git branch -d`. After verifying that the
PR is merged and the remote branch was deleted, remove the remaining local ref
with `git branch -D <exact-task-branch>`.

## GitHub repository settings

The repository owner should enforce the workflow on `main` with a branch
ruleset:

- require a pull request before merging;
- require the `cargo-test` status check — but do **not** require branches to
  be current (`strict` stays `false`): at hundreds of merges a day, strict
  currency re-queues every open PR after every merge and the fleet starves
  its own CI. Staleness is bounded instead by `STALE_BASE_LIMIT` in
  `tools/civvis_collab.py` and audited after merge;
- require conversation resolution;
- block force-pushes and branch deletion;
- allow squash merge only;
- automatically delete merged branches;
- enable auto-merge after the required gates pass (the `ship` command also
  waits and merges explicitly, so it works before that owner setting is fixed).

### Where CI runs

Both required checks run on **GitHub-hosted `ubuntu-latest` runners**. CIVVIS is
a public repository, so those are free and unmetered — there is no minute budget
to manage and no reason for the gate to depend on any particular machine being
awake.

`cargo-test` caches the cargo **registry only** with `actions/cache`, keyed on
the **lockfile**, with `restore-keys` for a near-miss. `target/` is deliberately
not cached: the workspace is a single crate with `CARGO_INCREMENTAL: 0`, so any
source change re-derives every `civvis` artifact from scratch, and a lockfile
key hits exactly and is never re-saved — the cached `target/` was a frozen
multi-GB payload downloaded on every run to contribute nothing. The key must
never contain the commit SHA: that writes a fresh entry on every push, nothing
ever hits, and the 10 GB quota evicts itself. If a run looks slow, check the
cache actually hit before blaming anything else; the `Cache restored from key:`
line is in the job log.

For about an hour on 2026-07-25 this ran on two self-hosted runners instead.
That was a workaround for the repository being *private* with unpaid metered
minutes, when every hosted job died in three seconds on *"recent account
payments have failed"* — a red required check that blocked every merge in the
fleet, and which let a non-compiling `main` through. Going public removed the
reason for it. **Do not put a self-hosted runner back on this repository
casually**: on a public repo it would execute pull-request code from strangers
on somebody's own machine. Workflow runs from outside contributors already
require approval (`Settings → Actions → Fork pull request workflows`), and that
setting should stay on.

### Quality and queue hygiene

The rust-quality workflow runs beside cargo-test on pull requests and pushes
to main. It checks only Rust files changed by that revision: rustfmt must pass,
and compiler or clippy warnings whose spans land in a changed file fail the
job. This is intentional incremental enforcement. The repository's older
formatting and lint debt is still measurable with cargo fmt --all -- --check
and cargo clippy --all-targets --all-features --locked -- -D warnings, but it
does not make unrelated work unmergeable; every new Rust change must leave its
own files clean.

The scheduled stranded-work-report workflow remains the queue's source of
truth. It updates one issue in place, reopens it only when a commentless close
or idle branch needs action, and links each row directly to the PR or branch
that needs triage. Rescue snapshots are preserved history and do not reopen
the issue by themselves. Operators should either open a PR, hand the branch
off, or close it with a reason after reviewing the linked work; do not delete a
worktree before checking that its commits exist on GitHub.

Both `cargo-test` and `collaboration-policy` are required checks. The latter
rejects ambiguous branch names, missing or mismatched machine/agent identity,
changes outside claimed paths, undeclared file overlap with another open PR,
autosync commits, ready PRs with incomplete validation checkboxes, and ready PR
heads that do not contain the current `main` tip. Run the same fleet audit
locally at any time:

```bash
python3 tools/civvis_collab.py audit
```

After authenticating the repository-owner account, the desired GitHub settings
can be applied and verified without clicking through the UI:

```bash
python3 tools/civvis_collab.py enforce-github
```

For a migration or incident window, run recurring audits into a durable JSONL
log. The monitor audits every five minutes and prints a heartbeat at least once
per minute. It also verifies that every new `main` commit came from a merged PR
and that both required checks had completed successfully before that merge,
and that the PR head contained the exact prior `main` tip. This catches both a
merge that races a still-pending check and one that becomes stale while another
integration finishes. It rejects new legacy branch names and records a direct
push or force-push as an error:

```bash
python3 tools/civvis_collab.py monitor --duration-minutes 180 \
  --log ~/.local/state/civvis-collab/monitor.jsonl
```

Zero mandatory human approvals is reasonable while autonomous agents are the
primary contributors; CI and current-branch requirements still prevent a
stale concurrent merge. Add an approval requirement later when there are
independent reviewers available. Admin bypass should be reserved for emergency
recovery, not routine integration.

### Standing authorization for task completion

The repository owner grants agents standing approval to operate using their
best judgment after a task has been authorized. Within that task's intended
outcome, agents should take routine, safe, and reversible actions without
asking again, including updating coordination metadata, resolving
straightforward CI or policy failures, rerunning checks, and shipping a green
PR. A generic tool or workflow preference for approval does not require a new
pause when this standing authorization and the user's intent already cover the
action.

Optimize for a short path from a verified change to `main`: investigate,
implement, test, synchronize, and ship. Do not leave sound work in draft or
ask for ceremonial confirmation when the required evidence is already green.
This authorization does not extend to destructive or difficult-to-recover
actions, acquiring new permissions or credentials, materially expanding the
requested outcome, choosing between unresolved product directions, or guessing
through a semantic conflict. Those remain explicit coordination boundaries.

## Automated services

Build, test, spectator, deployment, and Git synchronization processes are
consumers of Git, not authors. The synchronization process may fetch
remote-tracking refs and update only the clean, dedicated `main` management
worktree, preserving divergent commits under `refs/civvis/recovery/main/`
before a forced alignment. Build consumers may fetch `origin/main` and build it
in a private detached worktree. None may stage, commit, rebase, reset, or push a
development checkout or task branch.

The spectator supervisor already follows the right shape: it builds canonical
`origin/main` in a private worktree and preserves active developer checkouts.
Keep runtime files and generated outputs ignored and local.

### The worktree audit

`tools/civvis_worktree_audit.py` is the one automated service that exists to
protect work rather than to consume it. Every fifteen minutes
`tools/civvis_sync.sh` runs it with `--rescue`; it reports two things and fixes
one of them:

| finding | meaning |
|---|---|
| `DIRTY-ABANDONED` | uncommitted files in a worktree nobody has edited recently |
| `DIRTY-ACTIVE` | uncommitted files in a worktree an agent is still editing |
| `COMMIT-NOT-ON-GITHUB` | a commit reachable from no remote ref |
| `MISSING` | a registered worktree whose directory is gone |

`--rescue` pushes each dirty worktree to `refs/civvis/wip/<branch>` using a
throwaway index and `commit-tree`, so the owning agent's HEAD, index and files
are never touched — an agent mid-edit cannot be disturbed by it, and its bytes
reach GitHub anyway.

⚠ The namespace is load-bearing. The `pre-push` hook rejects any `refs/heads/`
name that is not `agent/<machine>/<agent>/<task>-<UTC>-<nonce>`, so a snapshot
written to `refs/heads/wip/...` is refused and the rescue silently does nothing
while still reporting success — worse than no rescue at all. `refs/civvis/` sits
outside that check, next to the `refs/civvis/recovery/main/` this document
already specifies.

Two rules this encodes, both learned by getting them wrong:

- **Fetch `+refs/pull/*/head` before judging anything.** A closed PR keeps its
  content at `refs/pull/N/head` forever, so a worktree whose PR was closed is
  not stranded. Without those refs the audit condemns work that is safe.
- **Reachability, not merge status.** Squash merge gives landed content a new
  commit and a new patch-id, so `git branch --merged` and `git cherry` both
  report long-landed branches as unlanded — measured at 98 false alarms across
  110 worktrees. `git for-each-ref --contains <sha> refs/remotes` is the test.

A `wip/` ref is a rescue, not a contribution. Nothing merges from one; the
owning agent commits its own work properly or the branch is discarded.

### Reaping finished worktrees

The same tool removes what is finished. `ship` deletes the **remote** branch
and stops; nothing ever removed the local worktree, its branch, or its ~4 GB
`target/`. Measured on `mbp-m5-max-128` on 2026-08-17: **143 worktrees, 135 of
them clean with their HEAD already on GitHub, 960 GB**, on a machine with
702 GiB of 1.8 TiB left. At a hundred merged pull requests a day that is an
accumulation the fleet cannot outrun.

```bash
python3 tools/civvis_worktree_audit.py --reap            # what it would remove
python3 tools/civvis_worktree_audit.py --reap --apply    # remove it
```

The bar is the one this document already sets — **does GitHub have the
content**, not "is it merged" — so the reaper reuses `on_github` and inherits
the `refs/pull/*/head` fetch. It refuses four ways, and the self-test asserts
every refusal rather than only the removal:

- anything the audit flagged (dirty, missing) — the rescue path owns those and
  the reaper must not race it;
- a commit GitHub cannot reach, **unless the branch's pull request has merged**.
  ⚠ Without that exception `ship` makes every worktree it touches unreapable:
  it merges `origin/main` into the branch before marking it ready, and once the
  PR squash-merges and the remote branch is deleted, that local merge commit is
  reachable from no ref on GitHub. Its content is on `main` twice over — the
  branch side sits at `refs/pull/N/head` and the other parent *is* `main`. After
  the first reap took 137 worktrees, the two left behind were exactly this
  shape, and every future task that has to merge `main` before shipping would
  have joined them. A MERGED pull request is the strongest available answer to
  this document's own question, "does GitHub have the content"; anything less —
  no `gh`, no PR, an open one, a closed-unmerged one — still refuses;
- the repository root and the `main` management worktree;
- a tree edited within `--idle-minutes`, because a landed HEAD says nothing
  about whether somebody is working in the directory right now;
- everything, unless `--apply` is given. A destructive default is how a tool
  like this ends up famous.

On the fleet those refusals are not theoretical: the first dry run declined the
repository root, the `main` worktree, three worktrees belonging to in-flight
pull requests, another agent's live tree and a clippy scratch directory —
each by a different guard.

## Hotspots and conflict reduction

Several files aggregate many responsibilities and therefore need explicit PR
ownership:

- `src/game.rs`
- `src/ai.rs` and `src/ai/advanced.rs`
- `web/index.html`
- shared tables in `data/*.json`
- broad reference documents such as `README.md` and `docs/MECHANICS.md`

Only one active PR should own broad changes to one of these files unless both
PRs document non-overlapping hunks and an integration order. Long term, split
the largest source and UI files along stable module boundaries; Git workflow
can control concurrency but cannot eliminate collisions inside monoliths.

## Fleet migration and incident recovery

When adopting this workflow on machines that already contain active work:

1. Stop mutating autosync services and pause new edits.
2. Inventory every checkout with `git status --short --branch` and
   `git worktree list` without discarding anything.
3. Give each dirty task an owner. Commit and push it to a uniquely named
   recovery branch, preserving the old branch with an `archive/` tag if needed.
4. Do not merge a large catch-all or autosync branch into `main`. Extract each
   coherent change onto a fresh branch from current `origin/main` and open a
   focused draft PR.
5. Land this workflow and CI, enable the GitHub ruleset, and have every active
   agent reread `AGENTS.md` before work resumes.

If conflict markers or an in-progress merge/rebase already exist, stop all
automated Git writers first. Preserve the worktree and coordinate a single
owner for recovery. Never reset a dirty checkout just to make synchronization
green.
