#!/bin/zsh
# Keep the local CIVVIS checkouts in sync with GitHub, and say so when they are not.
#
# Written 2026-08-01 at operator request ("sync local civvis with github, these two
# should always be in sync"). Two jobs, and the second is the one that matters:
#
#   1. Fast-forward the checkouts that are SUPPOSED to track origin/main.
#   2. Report anything local that GitHub does not have.
#
# (2) exists because of what this was written after: `follow.py` -- 460 lines of the
# tooling that drives the mirror window -- had been running for days out of
# /Users/martin/civvis-civ6-mirror, a directory with NO .git. No `git status`
# anywhere on this machine could see it, because git had never heard of the
# directory at all. `git hash-object` + `git cat-file -e` can see it: if the object
# store has never held a file's contents, that file exists nowhere but this disk.
# See civvis-civ6-the-harness-is-an-untracked-copy.
#
# ⚠⚠ 2026-08-05: THIS SCRIPT USED TO LIVE ONLY AT ~/civvis-sync.sh, AND ITS OWN
# `git cat-file -e` TEST SAID THIS DISK WAS THE ONLY COPY. The guard against
# stranded code was itself stranded. It is now tracked here and ~/civvis-sync.sh
# is a one-line shim that execs this file, so the scheduled job always runs the
# version on main.
#
# The same audit found the hole that mattered: this script deliberately skipped
# agent worktrees, and an agent worktree is the only place work has ever been
# stranded. 146 unstaged lines of `civvis_orders --without` — the live bridge's
# only control arm — sat in one for a day, on no GitHub ref, while this ran every
# fifteen minutes and reported "in sync". Step 4 closes that.
#
# What it will NOT do, deliberately:
#   * fast-forward an agent worktree. Those live on their own branches by design
#     (civvis-version-control-policy); "behind origin/main" is not a fault there.
#     ⚠ Being DIRTY is a fault everywhere, and step 4 does check that.
#   * fast-forward a DIRTY tree, or one a live Civilization VI run is executing
#     from. Swapping source under a running run is a documented way to lose one.
#   * commit, push, or delete anything. It reports; a person decides.
#   * run `git gc` (Homebrew git segfaults on auto-maintenance here --
#     git-automaintenance-segfaults).
#
# Exit 0 = in sync. Exit 1 = something needs a person.

set -u

REPO=/Users/martin/CIVVIS
LOG=/Users/martin/civvis-sync.log
# Checkouts that are meant to sit at the origin/main tip. Agent worktrees are not
# in this list and must never be.
TIP_CHECKOUTS=(/Users/martin/CIVVIS /Users/martin/civvis-spectator-src)
# Directories that hold copies of repo files but are not checkouts at all.
UNTRACKED_COPIES=(/Users/martin/civvis-civ6-mirror /Users/martin/civvis-settler-harness)

problems=0

say() {
  print -r -- "[sync] $(date -u +%FT%TZ) $*" | tee -a $LOG
}

# A run in flight owns the tree it is executing from. `pgrep -f` matches the full
# command line, which is where civ6_play/civ6_brain/civvis_orders spell out the
# checkout they were launched from.
run_is_live_in() {
  local dir=$1
  # One `ps` and two greps, on purpose: a `while read` over pgrep output runs in a
  # subshell, so a `return` inside it cannot set this function's exit status --
  # the guard would silently never fire, which is worse than not having it.
  ps -axo command= 2>/dev/null \
    | grep -E 'civ6_play\.py|civ6_brain\.py|civvis_orders' \
    | grep -q -- "$dir"
}

say "--- checking ---"

# One fetch for the shared object store; every worktree reads the same refs.
# gc.auto=0 so a fetch can never fork the maintenance process that segfaults here.
if ! git -C $REPO -c gc.auto=0 fetch -q --prune origin 2>>$LOG; then
  say "FETCH FAILED -- cannot say anything about sync state"
  exit 1
fi
tip=$(git -C $REPO rev-parse --short origin/main)

# --- 1. the checkouts that should track main ---------------------------------
for wt in $TIP_CHECKOUTS; do
  name=${wt:t}
  [[ -d $wt ]] || { say "$name MISSING at $wt"; problems=$((problems+1)); continue; }

  behind=$(git -C $wt rev-list --count HEAD..origin/main 2>/dev/null)
  ahead=$(git -C $wt rev-list --count origin/main..HEAD 2>/dev/null)
  dirty=$(git -C $wt status --porcelain 2>/dev/null | wc -l | tr -d ' ')

  if [[ $ahead != 0 ]]; then
    say "$name is $ahead COMMIT(S) AHEAD of origin/main -- not fast-forwarding, a person must look"
    problems=$((problems+1)); continue
  fi
  if [[ $behind == 0 ]]; then
    [[ $dirty != 0 ]] && { say "$name at tip but $dirty uncommitted file(s)"; problems=$((problems+1)); }
    continue
  fi
  if [[ $dirty != 0 ]]; then
    say "$name is $behind behind but has $dirty uncommitted file(s) -- left alone"
    problems=$((problems+1)); continue
  fi
  if run_is_live_in $wt; then
    say "$name is $behind behind but a live Civ 6 run is executing from it -- left alone"
    continue
  fi

  # The root checkout sits on `main`; spectator-src is kept detached at the tip.
  if [[ $(git -C $wt rev-parse --abbrev-ref HEAD) == "main" ]]; then
    git -C $wt merge --ff-only origin/main >/dev/null 2>>$LOG
  else
    git -C $wt checkout -q --detach origin/main 2>>$LOG
  fi
  if [[ $(git -C $wt rev-parse --short HEAD) == $tip ]]; then
    say "$name fast-forwarded $behind commit(s) -> $tip"
  else
    say "$name FAILED to fast-forward to $tip"
    problems=$((problems+1))
  fi
done

# --- 2. local content GitHub does not have -----------------------------------
# The expensive question asked cheaply: does the object store have this file's
# contents at all? A miss means the only copy is on this disk.
#
# But a miss alone is not an alarm, and getting that wrong would make this script
# useless -- it would shout about the same two files every fifteen minutes until
# nobody read it. A copy whose bytes git has never seen is one of two things:
#
#   BEHIND  an older hand-edited copy of a file main has since moved past. Its
#           unique lines are all superseded comments and re-indents. GitHub has
#           strictly more. Nothing is at risk; logged, not counted.
#   AHEAD   it carries a line main does not have. That line exists nowhere else
#           in the world. This is the alarm, and it is the one that caught
#           follow.py.
#
# Telling those apart is harder than diffing lines, and this got it wrong once.
# `CivvisControlAgent.lua` in the harness has six code lines main does not have:
#
#     local function buildParams(row)          <- harness
#     local function buildParams(row, city)    <- main, after PR #698
#
# A line diff calls that six lines of unique work. It is the opposite -- the
# harness holds the SUPERSEDED one-argument version, and main moved past it.
#
# What separates the two cases is NAMES. Genuinely new work introduces an
# identifier the tracked file has never heard of; an older revision of a function
# only ever re-uses names that are already there. So: pull the identifiers out of
# the copy's unique lines, and report only those the tracked file lacks entirely.
unique_names() {
  local copy=$1 tracked=$2
  diff "$tracked" "$copy" 2>/dev/null | grep '^>' | sed 's/^> //' \
    | grep -vE '^[[:space:]]*(#|--|//)' | grep -vE '^[[:space:]]*$' \
    | grep -oE '[A-Za-z_][A-Za-z0-9_]{2,}' | sort -u \
    | while read -r name; do
        grep -qF -- "$name" "$tracked" || print -r -- "$name"
      done
}

for dir in $UNTRACKED_COPIES; do
  [[ -d $dir ]] || continue
  find $dir -type f \( -name '*.py' -o -name '*.lua' -o -name '*.rs' -o -name '*.js' -o -name '*.html' \) \
    -not -path '*/__pycache__/*' -not -path '*/target/*' -not -path '*/node_modules/*' 2>/dev/null \
  | while read -r f; do
      h=$(git -C $REPO hash-object "$f" 2>/dev/null) || continue
      git -C $REPO cat-file -e "$h" 2>/dev/null || print -r -- "$f"
    done > /tmp/civvis-sync-stranded.$$

  while read -r f; do
    # The tracked file of the same name, if the repo has one at all.
    rel=$(git -C $REPO ls-files | grep -E "(^|/)${f:t}\$" | head -1)
    if [[ -z $rel ]]; then
      say "STRANDED: $f exists in no git object and the repo has no file of that name -- this disk is the only copy"
      problems=$((problems+1))
      continue
    fi
    names=$(unique_names "$f" "$REPO/$rel")
    if [[ -z $names ]]; then
      say "stale copy (behind $rel, introduces no name it lacks): $f"
    else
      say "DRIFTED AHEAD: $f introduces $(print -r -- $names | wc -w | tr -d ' ') name(s) absent from $rel -- $(print -r -- ${names//$'\n'/ }) -- and its contents are in no git object"
      problems=$((problems+1))
    fi
  done < /tmp/civvis-sync-stranded.$$
  rm -f /tmp/civvis-sync-stranded.$$
done

# --- 3. commits on a local branch that never reached GitHub -------------------
# A branch whose remote is gone is normal here: the PR squash-merged and GitHub
# deleted it. What is NOT normal is a branch with commits and no PR at all.

# Every PR ever opened, fetched ONCE. Two reasons this is not a per-branch
# `gh pr list --head`: it is ~25 fewer API calls, and `gh` infers the repository
# from the working directory -- which is wherever launchd happened to start us,
# usually not a git repo at all. It then returns an empty answer that looks
# exactly like "this branch has no PR", and every branch on the machine gets
# reported as unpushed work. `-R` removes the guess.
SLUG=$(git -C $REPO remote get-url origin 2>/dev/null \
       | sed -E 's#^.*github\.com[:/]##; s#\.git$##')
if ! gh pr list -R "$SLUG" --state all --limit 500 \
       --json headRefName,number,state --jq '.[]|"\(.headRefName)\t\(.number)\t\(.state)"' \
       > /tmp/civvis-sync-prs.$$ 2>/dev/null; then
  say "could not list PRs for $SLUG -- skipping the branch check rather than guessing"
  : > /tmp/civvis-sync-prs.$$
  prs_known=0
else
  prs_known=1
fi

git -C $REPO for-each-ref --format='%(refname:short)' refs/heads \
| while read -r b; do
    [[ $b == "main" ]] && continue
    if git -C $REPO rev-parse --verify -q "refs/remotes/origin/$b" >/dev/null 2>&1; then
      n=$(git -C $REPO rev-list --count "origin/$b..$b" 2>/dev/null)
      [[ $n != 0 ]] && print -r -- "UNPUSHED $n $b"
    else
      n=$(git -C $REPO rev-list --count "origin/main..$b" 2>/dev/null)
      [[ $n != 0 ]] && print -r -- "NOREMOTE $n $b"
    fi
  done > /tmp/civvis-sync-branches.$$
while read -r kind n b; do
  if [[ $kind == UNPUSHED ]]; then
    say "UNPUSHED: $b has $n commit(s) not on GitHub"
    problems=$((problems+1))
  else
    # No remote branch. Merged-and-deleted is the normal end of a task here, so
    # that is not a fault; a branch GitHub has never seen at all is.
    #
    # ⚠ ASK REACHABILITY BEFORE ASKING THE PR LIST. Matching the branch NAME
    # against PR head names is wrong twice over: a branch can be renamed or
    # copied locally (`tmp/m977b` held PR #977's exact commits and was reported
    # as "never opened as a PR"), and a PR head branch that GitHub deleted after
    # merge keeps its content at refs/pull/N/head forever. What decides whether
    # anything is at risk is whether the COMMIT is on GitHub, not what it is
    # called. The pull refs are fetched here because the default refspec does
    # not bring them down.
    git -C $REPO -c gc.auto=0 fetch -q origin '+refs/pull/*/head:refs/remotes/pr/*' 2>>$LOG
    if [[ -n $(git -C $REPO for-each-ref --contains "$b" --count=1 \
                 --format='%(refname)' refs/remotes 2>/dev/null) ]]; then
      continue          # the commits are on GitHub under some ref; nothing at risk
    fi
    [[ $prs_known == 1 ]] || continue
    if ! grep -qF "$b	" /tmp/civvis-sync-prs.$$; then
      say "NO PR: $b has $n commit(s) reachable from no GitHub ref at all"
      problems=$((problems+1))
    fi
  fi
done < /tmp/civvis-sync-branches.$$
rm -f /tmp/civvis-sync-branches.$$ /tmp/civvis-sync-prs.$$

# --- 3b. the operational scripts that drive the fleet ------------------------
# ⚠⚠ ELEVEN OF THESE EXISTED ON EXACTLY ONE DISK — 1,794 lines including
# `civvis-batch-loop.sh`, the 492-line supervisor that runs the whole
# measurement fleet. `git cat-file -e $(git hash-object …)` missed on every one.
# They are tracked under tools/ops/ now, and this step is what keeps the two
# copies from drifting apart again: the home copy is what launchd actually runs,
# so a silent edit there is a silent fork.
#
# Reported, never overwritten. These are live operational scripts and a sweep
# that rewrites one mid-run is a much worse failure than a stale copy.
for home in /Users/martin/civvis-*.sh; do
  [[ -f $home ]] || continue
  name=${home:t}
  [[ $name == civvis-sync.sh ]] && continue        # a shim by design; see the header
  tracked=$REPO/tools/ops/$name
  if [[ ! -f $tracked ]]; then
    say "UNTRACKED SCRIPT: $home has no tools/ops/$name -- it exists only on this disk"
    problems=$((problems+1))
  elif ! cmp -s "$home" "$tracked"; then
    say "SCRIPT DRIFT: $home differs from tools/ops/$name -- launchd runs the home copy"
    problems=$((problems+1))
  fi
done

# --- 4. work that exists only in a worktree ----------------------------------
# The class every other step here is blind to. Steps 1 and 3 reason about
# COMMITS; unstaged work has none, so an abandoned tree scores zero on both and
# passes. This asks the only question that matters — is the content on GitHub —
# and `--rescue` makes the answer yes for anything dirty, without touching the
# tree an agent may still be editing.
audit=$REPO/tools/civvis_worktree_audit.py
if [[ -x $audit ]]; then
  # --no-fetch: the fetch above already refreshed origin, and re-fetching the
  # ~1200 pull refs every fifteen minutes is the one part of this that is slow.
  # ⚠ The audit needs them for its reachability test, so it fetches them itself
  # when they are missing; passing --no-fetch only skips the redundant second
  # fetch of refs/heads.
  while IFS= read -r line; do
    [[ -z $line ]] && continue
    say "$line"
    [[ $line == DIRTY-ACTIVE:* ]] || problems=$((problems+1))
  done < <(python3 "$audit" --repo "$REPO" --rescue --quiet 2>>$LOG)
else
  say "worktree audit missing at $audit -- cannot say whether work is stranded"
  problems=$((problems+1))
fi

# The live verification game must provably track origin/main. The brain's
# updater hands the running game a fresh decider at turn boundaries and writes
# a heartbeat every refresh cycle; `check --heartbeat-minutes` fails when that
# heartbeat is stale, unreadable, or reporting a refresh error — and passes
# untouched on machines that have never run the live loop (no runtime cache).
# Ten minutes of slack over the 30s refresh means only real silence alarms.
ladder="$REPO/tools/civ6_ladder.py"
if [[ -f $ladder ]]; then
  while IFS= read -r line; do
    [[ -z $line || $line != LADDER:* ]] && continue
    say "$line"
    problems=$((problems+1))
  done < <(python3 "$ladder" watch --minutes 10 2>>$LOG)
fi

if [[ $problems == 0 ]]; then
  say "in sync with GitHub at $tip"
  exit 0
fi
say "$problems item(s) need a person"
exit 1
