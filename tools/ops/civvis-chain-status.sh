#!/bin/zsh
# Is CIVVIS actually playing Civilization VI with the head of GitHub and the top
# strategy — right now, on this machine?
#
# Written 2026-08-02 alongside ~/civvis-batch-loop.sh, because "make sure we are
# always syncing" is only worth anything if it is CHECKABLE. Every link in the chain
# below has silently broken at least once here, and each break looked like normal
# operation from the outside:
#
#   * the runner tree sat 5 commits behind origin/main for a day while batches ran
#     from it, because advancing it was a rule in a note rather than a job;
#   * a batch pinned df475b9 played FOUR games and started NONE of them, while the
#     ledger filled with rows that a casual reader would count as attempts;
#   * this project has shipped a learned evaluator that never once loaded while its
#     documentation called it good (civvis-valuenet-never-loaded).
#
# Exit 0 = the chain is whole. Exit 1 = a link needs a person.
#
#   zsh ~/civvis-chain-status.sh

set -u
export PATH=$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin

REPO=$HOME/CIVVIS
RUNNER=$HOME/civvis-batch-runner
SPECTATOR=$HOME/civvis-spectator-src
STATE=$HOME/.civvis-batch-loop.state
LADDER=$HOME/civvis-civ6-runs/civvis_ladder.jsonl

bad=0
ok()   { print -r -- "  PASS  $*" }
warn() { print -r -- "  WARN  $*" }
err()  { print -r -- "  FAIL  $*"; bad=1 }

print "GitHub"
if git -C $REPO -c gc.auto=0 fetch --quiet origin main 2>/dev/null; then
  tip=$(git -C $REPO rev-parse origin/main)
  ok "origin/main ${tip:0:7} — $(git -C $REPO log --format=%s -1 $tip)"
else
  err "could not fetch origin/main; every check below is against a stale ref"
  tip=$(git -C $REPO rev-parse origin/main 2>/dev/null)
fi

print "checkouts"
# ⚠ "Behind" is not automatically a fault, and calling it one would make this script
# cry wolf every quarter of an hour. com.civvis.sync ticks every 900 s, so a commit
# that landed since its last tick SHOULD still be missing. What is a fault is being
# behind a commit the sync agent has already had a chance at — that means the agent
# is dead, wedged, or skipping the checkout, and only then does a person need to look.
last_sync=$(date -r $HOME/civvis-sync.log +%s 2>/dev/null || print 0)
tip_time=$(git -C $REPO log --format=%ct -1 $tip 2>/dev/null || print 0)
for dir in $REPO $SPECTATOR; do
  [[ -d $dir ]] || { err "$dir is missing"; continue }
  head=$(git -C $dir rev-parse HEAD)
  behind=$(git -C $dir rev-list --count HEAD..$tip 2>/dev/null)
  if [[ $head == $tip ]]; then
    ok "${dir:t} at the tip"
  elif (( tip_time > last_sync )); then
    warn "${dir:t} is $behind commit(s) behind, but the tip landed after com.civvis.sync last ran — it ticks every 15 min"
  else
    err "${dir:t} is $behind commit(s) behind and com.civvis.sync has already run since — the agent is not doing its job"
  fi
done

print "the tree that PLAYS the game"
# ⚠ This is the link that has no business being behind and repeatedly was. It is also
# the only one that is ALLOWED to be behind for a good reason: a batch pins one sha
# so its rows stay comparable, so "behind, with a batch running" is correct and
# "behind, with nothing running" is drift.
rhead=$(git -C $RUNNER rev-parse HEAD)
rbehind=$(git -C $RUNNER rev-list --count HEAD..$tip 2>/dev/null)
live_pin=$(pgrep -f 'civ6_civvis_climb\.py' >/dev/null 2>&1 && print yes || print no)
last_good=$(awk '$1=="last_good"{print $2}' $STATE 2>/dev/null)
if [[ $rhead == $tip ]]; then
  ok "civvis-batch-runner at the tip"
elif [[ $live_pin == yes ]]; then
  ok "civvis-batch-runner at ${rhead:0:7}, $rbehind behind — a batch is PINNED to it (correct: rows stay comparable)"
elif [[ -n $last_good && $rhead == $last_good ]]; then
  warn "civvis-batch-runner held at known-good ${rhead:0:7}, $rbehind behind — the tip could not start a game"
  warn "  the next commit to land on origin/main is tried immediately; this is not a permanent pin"
else
  err "civvis-batch-runner is $rbehind behind the tip with NO batch running — this is drift"
  err "  games played from here measure code GitHub moved past"
fi

print "the decider binary"
bin=$RUNNER/target/release/civvis_orders
if [[ ! -x $bin ]]; then
  err "no civvis_orders at $bin"
else
  # cargo does not stamp the sha into the binary (CIVVIS_COMMIT is read from the
  # environment at RUNTIME, per #892), so freshness is judged the only way that is
  # actually true: is any source newer than the artefact? `git checkout` rewrites the
  # mtime of every file it changes, so a tree that moved and was not rebuilt fails here.
  newest=$(find $RUNNER/src $RUNNER/data -type f -newer $bin -print -quit 2>/dev/null)
  if [[ -n $newest ]]; then
    err "binary is STALE — $newest is newer than it. The batch would play the previous build."
  else
    ok "binary is newer than every source and data file in the tree"
  fi
fi

print "the strategy it will play"
# Ask the binary, do not reimplement the ranking. `auto` ranks on the outright-win
# LOWER BOUND (league::strategy_strength), not the placement Elo — the README explains
# at length why the placement ordering names a different strategy in 23 of 50 pairs.
if [[ -x $bin ]]; then
  probe=$(mktemp -d /tmp/civvis-chain-probe.XXXXXX)
  # The lane is irrelevant to what this probe reads back — it asks which strategy
  # `auto` resolves to — but a lane written by hand here is a fourth copy of a
  # fact that already drifted across the launchers, so it is asked for instead.
  lane=$( ( cd $RUNNER && python3 -c \
      'import sys; sys.path.insert(0, "tools"); import civ6_play; print(civ6_play.DEFAULT_CIVVIS_VICTORY)' \
  ) 2>/dev/null )
  genome=""
  [[ -n $lane ]] && genome=$( ( cd $RUNNER && ./target/release/civvis_orders --mirror $probe --turn 0 \
              --victory $lane --strategy auto ) 2>&1 >/dev/null | grep -m1 '"kind":"genome"' )
  rm -rf $probe
  if [[ -z $lane ]]; then
    err "cannot resolve the victory lane from $RUNNER — the probe was not run"
  elif [[ -z $genome ]]; then
    err "the binary printed no genome line — cannot tell which strategy would play"
  elif [[ $genome == *'"strategy":"stock"'* ]]; then
    err "resolves to STOCK — the league did not load, so --strategy auto is doing nothing"
  else
    ok "$genome"
    round=$(python3 -c "import json;print(json.load(open('$RUNNER/data/league/league.json'))['round'])" 2>/dev/null)
    [[ -n $round ]] && ok "league snapshot round $round (ships with the sha; rebuilt = refreshed)"
  fi
fi

print "automation"
if launchctl list 2>/dev/null | grep -q "	com.civvis.sync\$"; then
  ok "com.civvis.sync loaded"
else
  err "com.civvis.sync NOT loaded — the checkouts will stop tracking GitHub"
fi
# ⚠ The batch loop is NOT necessarily a LaunchAgent, and checking launchctl alone
# would report a working machine as broken. macOS TCC forbids a launchd job from
# writing into Civ6.app, and the harness must (the mod lives in the bundle and is
# re-installed every attempt), so the loop normally runs nohup'd from an interactive
# context that holds the App Management grant. What matters is that SOMETHING holds
# the lock and is alive — how it was started is an implementation detail.
loop_pid=$(cat $HOME/.civvis-batch-loop.pid 2>/dev/null)
if [[ -n $loop_pid ]] && kill -0 $loop_pid 2>/dev/null; then
  if launchctl list 2>/dev/null | grep -q "	com.civvis.batchloop\$"; then
    ok "batch loop alive as pid $loop_pid (com.civvis.batchloop)"
  else
    ok "batch loop alive as pid $loop_pid (interactive; the LaunchAgent is deliberately off)"
    warn "  it will NOT survive a reboot until /bin/zsh has Full Disk Access and the agent is re-enabled"
  fi
else
  err "no batch loop is running — nothing advances the runner tree or starts batches"
  err "  start it: unsetopt BG_NICE; nohup /bin/zsh ~/civvis-batch-loop.sh >> ~/civvis-civ6-runs/batch-loop.nohup.log 2>&1 &"
fi

print "is it actually playing"
if [[ $live_pin == yes ]]; then
  ok "a batch is running: $(pgrep -lf 'civ6_civvis_climb\.py' | head -1 | cut -c1-100)"
else
  warn "no batch running right now (the loop starts one within its idle interval)"
fi
# ⚠ THE DENOMINATOR. Rows exist whether or not a game ever started, and a batch that
# starts nothing looks busy. Count both.
if [[ -f $LADDER ]]; then
  # ⚠ The verdict has to come back through the EXIT STATUS. This block printed a
  # tidy "FAIL  df475b9: 4 rows and NOT ONE started a game" under a final banner
  # reading CHAIN WHOLE, because a subprocess printing the word FAIL sets nothing.
  # A status script whose summary line disagrees with its own body is worse than no
  # status script — the summary is the part anyone actually reads.
  python3 - "$LADDER" "$rhead" <<'PY'
import json, sys
path, head = sys.argv[1], sys.argv[2][:7]
rows = played = 0
for line in open(path):
    line = line.strip()
    if not line:
        continue
    try:
        r = json.loads(line)
    except ValueError:
        continue
    if str(r.get("code_rev"))[:7] != head:
        continue
    rows += 1
    if r.get("last_turn") is not None:
        played += 1
if rows == 0:
    print(f"  WARN  no ladder rows yet for {head}")
elif played == 0:
    print(f"  FAIL  {head}: {rows} row(s), and NOT ONE started a game")
    sys.exit(1)
else:
    print(f"  PASS  {head}: {played}/{rows} row(s) actually started a game")
PY
  (( $? )) && bad=1
fi

print ""
(( bad )) && print "CHAIN BROKEN — see FAIL above" || print "CHAIN WHOLE"
exit $bad
