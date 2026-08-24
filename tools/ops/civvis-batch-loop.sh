#!/bin/zsh
# Keep the tree that PLAYS Civilization VI at the GitHub origin/main tip, rebuild the
# decider from it, and run a pinned batch on it with the league's top strategy —
# for ever, without a person in the loop.
#
# Written 2026-08-02 at operator request: "make sure we are always syncing to the head
# of github civvis and running that code in our tests. we should be employing the top
# strategies. we should have automation around this so this all happens automatically."
#
# ## The gap this closes
#
# `~/civvis-sync.sh` (com.civvis.sync, every 15 min) already fast-forwards ~/CIVVIS and
# ~/civvis-spectator-src to origin/main. It deliberately does NOT touch
# ~/civvis-batch-runner, and it must not: that is the tree batches execute from, and
# swapping source under a live measurement is what the climb's pin guard exists to
# stop (civvis-run-batches-from-a-dedicated-tree — a pull mid-batch cost 7 attempts).
#
# So the chain had a broken link. The code checkout tracked GitHub; the tree that
# actually plays the game was advanced BY HAND "between batches" and drifted. On
# 2026-08-02 it sat at df475b9 while origin/main was f114601 — five commits and seven
# merged PRs of fixes that no game was played on.
#
# This script is the between-batches step, automated. It is the ONLY thing that moves
# the runner tree, and it only ever does so when no run is live, which is precisely
# the condition the manual rule stated and could not enforce.
#
# ## The chain it maintains, end to end
#
#   GitHub origin/main
#     -> git fetch into ~/CIVVIS (shared object store; worktrees read the same refs)
#     -> git checkout --detach <tip> in ~/civvis-batch-runner   [BETWEEN batches only]
#     -> cargo build --release --bin civvis_orders               [from that tree]
#     -> the batch, PINNED to that sha, with --strategy auto
#
# The last link is why the top strategy rides along for free and must not be pinned
# separately. `civvis_orders --strategy auto` ranks on `league::strategy_strength` —
# the outright-win LOWER BOUND, not the placement Elo — and `league_dirs()` resolves
# the league relative to THE BINARY'S OWN PATH (target/release/civvis_orders ->
# <repo>/data/league). So the strategy table a batch plays is whichever
# data/league/league.json came with the sha the binary was built from. Advancing the
# tree and rebuilding is what refreshes the strategies; there is no second thing to
# remember, and a stale tree silently means stale strategies as well as stale code.
#
# ⚠ Naming the pick, every batch, is not decoration. This project shipped a learned
# evaluator that never once loaded while its docs called it good
# (civvis-valuenet-never-loaded), and the league champion measured +48 in the compact
# evaluation and -53 DEPLOYED (civvis-champion-does-not-transfer-to-deployment). So
# the provenance file records the strategy name and its strength bound as the binary
# itself reports them, from the `{"kind":"genome"}` line it prints on startup. A batch
# whose league failed to resolve is then distinguishable from one that chose the
# stock genome on purpose.
#
# ## Why a broken tip cannot wedge this
#
# "Always run head" and "always produce rows" are in tension: a tip that cannot start
# a game plays no games. Resolution — the no-game penalty is recorded PER SHA:
#
#   * a sha is only distrusted after NOGAME_LIMIT consecutive no-game batches, and
#     host recovery (Problem Reporter, wedged core) is attempted between them, because
#     "NO GAME" is far more often the host than the code;
#   * a distrusted sha falls back to the last sha that demonstrably played a game;
#   * a NEW tip is ALWAYS tried immediately, whatever the old one did. The fallback
#     can never outlive the commit that earned it, so this never settles onto an old
#     build the way the hand-run protocol did.
#
# Exit codes from tools/civ6_civvis_climb.py, which is what the classification reads:
#   0 = a game was WON            1 = games played, none won      2 = no decider binary
#   3 = no game could be started  4 = preflight failed, or the code changed mid-batch
#
# This script is a supervisor and never exits on its own; com.civvis.batchloop keeps
# it alive. To stop it, bootout the agent (see the bottom of this file).

set -u

# launchd starts a job with a near-empty PATH, and `cargo` here is a rustup shim in
# ~/.cargo/bin that is NOT on the login PATH at all (civvis-ship-needs-cargo-on-path).
# A build that fails for that reason looks exactly like a build that fails on the code.
export PATH=$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin

REPO=$HOME/CIVVIS
RUNNER=$HOME/civvis-batch-runner
RUNS=$HOME/civvis-civ6-runs
LOG=$RUNS/batch-loop.log
PROV=$RUNS/BATCH-PROVENANCE-runner.txt
STATE=$HOME/.civvis-batch-loop.state
BUILDLOG=$RUNS/batch-loop-build.log
# One driver, named. There is exactly ONE Civilization VI on this machine and a batch
# takes it whole, so two drivers is not "slower", it is corrupt rows.
LOCK=$HOME/.civvis-batch-loop.pid

# Attempts per batch. A batch is a COMPARISON pinned to one program, so this is also
# how long the loop will decline to pick up a newer tip. Eight attempts is what the
# hand-run batches used.
ATTEMPTS=${CIVVIS_BATCH_ATTEMPTS:-8}
# Consecutive no-game batches on ONE sha before it is distrusted. 2, not 1: a single
# no-game batch is usually the host (a Problem Reporter modal eating the Create Game
# click has caused this exact symptom), and blaming the code for it would park the
# loop on an old build for a host fault.
NOGAME_LIMIT=${CIVVIS_NOGAME_LIMIT:-2}
# The victory objective this loop plays for. It used to be the literal `civvis`
# written into all three sites below, while the installed supervisor service
# passed nothing at all and inherited `science` — two production loops running
# two different experiments into one ladder. Neither value survived measurement:
# `science` completes 0/16 at this profile and `civvis` is the untargeted agent.
# Empty means "whatever the tree we are about to play declares", asked of that
# tree rather than restated here — the same reason the genome probe below asks
# the binary which strategy `auto` resolves to instead of ranking them again.
# `victory_lane` resolves it once per pass, after the checkout, so a batch always
# reports the lane the code it pinned would use.
VICTORY=${CIVVIS_VICTORY:-}
# The rung this loop plays for. It expressed NOTHING here and inherited
# `civ6_civvis_climb.py`'s `DIFFICULTY_SETTLER`, silently — the same shape as
# the victory lane before #1960, and now the same divergence: since #1969 the
# installed supervisor selects its rung from the live ledger through
# `civ6_ladder_policy.py`, so the two production loops would be climbing on two
# different rules and only one of them would ever reach Chieftain.
#
# Empty means "whatever the policy says", asked of the tree about to be played,
# exactly as the lane is. `CIVVIS_DIFFICULTY` pins one instead.
DIFFICULTY_OVERRIDE=${CIVVIS_DIFFICULTY:-}
ladder_rung() {
  [[ -n $DIFFICULTY_OVERRIDE ]] && { print -r -- "$DIFFICULTY_OVERRIDE"; return 0 }
  [[ -f $RUNNER/tools/civ6_ladder_policy.py ]] || return 0
  ( cd $RUNNER && python3 tools/civ6_ladder_policy.py \
      --runs $RUNS/control target ) 2>/dev/null
}
victory_lane() {
  [[ -n $VICTORY ]] && { print -r -- "$VICTORY"; return 0 }
  ( cd $RUNNER && python3 -c \
      'import sys; sys.path.insert(0, "tools"); import civ6_play; print(civ6_play.DEFAULT_CIVVIS_VICTORY)' \
  ) 2>/dev/null
}
# How long to wait before looking again. This is the width of the window in which
# another driver can take the game between one batch ending and this loop noticing —
# so it is a contention parameter, not a politeness one. Observed 2026-08-02: other
# agent sessions were hand-starting batches within a minute of the previous one
# ending ([[civvis-civ6-the-ladder-has-no-owner]] records three agents taking it from
# each other in an hour), and at 120 s this loop lost every gap. A fetch and a `ps`
# are cheap; the batch itself is hours. Do not raise this to "be nice".
IDLE_S=${CIVVIS_BATCH_IDLE_S:-45}
# How long to keep playing the known-good sha before spending batches on a new tip
# again, once a tip has been caught starting no games.
#
# ⚠ Without this the per-sha rule below is pathological in exactly the situation it
# was written for. `main` takes several commits an hour here, and each new sha buys a
# fresh set of NOGAME_LIMIT batches — so while head is broken the loop would spend
# almost all of its wall clock failing to start games on successive tips and almost
# none of it playing. Measured cause, 2026-08-02: #731 (4e0adbd) rewrote the setup
# vision pass in civ6_play.py onto a new input/OCR layer; shas that include it read
# Prince/Standard off the Create Game screen instead of Settler/Online and start
# nothing. 1000a13 (before it) is 6/6; df475b9 (after it) is 0/4.
#
# So: still track head, still retry it unprompted, but at a rate that leaves most of
# the day for games that actually happen. Clear ~/.civvis-batch-loop.state to retry
# immediately once head is fixed.
TIP_RETRY_S=${CIVVIS_TIP_RETRY_S:-3600}

mkdir -p $RUNS

say() {
  print -r -- "[loop] $(date -u +%FT%TZ) $*" | tee -a $LOG
}

# ---------------------------------------------------------------- state

# Flat `key value` lines. Keys used:
#   last_good <sha>     the newest sha this loop has watched play an actual game
#   nogame <sha> <n>    consecutive no-game batches on <sha>
state_get() {
  local key=$1
  [[ -f $STATE ]] || return 0
  awk -v k="$key" '$1 == k { $1 = ""; sub(/^ /, ""); print; exit }' $STATE
}

state_set() {
  local key=$1 value=$2 tmp=$STATE.tmp.$$
  : > $tmp
  [[ -f $STATE ]] && awk -v k="$key" '$1 != k' $STATE >> $tmp
  print -r -- "$key $value" >> $tmp
  mv $tmp $STATE
}

nogame_count() {
  local sha=$1 rec
  rec=$(state_get "nogame")
  # `nogame <sha> <n>` — a count only ever describes the sha it was recorded against,
  # so a different sha reads 0 rather than inheriting the old one's suspicion.
  [[ ${rec%% *} == $sha ]] && print -r -- "${rec##* }" || print -r -- 0
}

# ---------------------------------------------------------------- guards

# A run in flight OWNS the tree it executes from, and this script must never move that
# tree underneath it — that is the whole reason the runner worktree exists.
#
# ⚠ The `ps` output is SNAPSHOTTED into a variable before anything greps it, and that
# is load-bearing. Written as a pipeline (`ps | grep -E '…civ6_play…'`) the grep's OWN
# command line contains every pattern it is searching for, so `ps` sees it and the
# guard reports a live run for ever — the loop would then idle permanently and look
# exactly like a loop that was correctly staying out of the way. In a command
# substitution the only process alive is `ps` itself, and the grep runs afterwards
# against a string.
# ⚠ `/civvis_orders`, WITH THE SLASH, and that is not a stylistic choice. A bare
# `civvis_orders` also matches this loop's OWN build step -- `cargo build --release
# --bin civvis_orders` -- and anyone else's. The decider is always invoked by path
# (`.../target/release/civvis_orders --mirror ...`), so requiring the slash keeps the
# thing being RUN and drops the thing being COMPILED. Without it, a build anywhere on
# the machine reads as a live game and this loop defers until someone notices.
run_is_live() {
  local snapshot
  snapshot=$(ps -axo command= 2>/dev/null)
  print -r -- "$snapshot" \
    | grep -qE 'civ6_civvis_climb\.py|civ6_play\.py|civ6_brain\.py|/civvis_orders'
}

# "NO GAME" is usually something on screen, not something in the code. Four Civ 6
# segfaults on 2026-08-02 left a modal titled "Problem Report for Civilization VI"
# up, and every later attempt reported NO GAME because that modal was taking the
# click the Create Game vision pass was about to make. ⚠ The process is named
# `Problem Reporter`, NOT `ReportCrash` — the daemon has no windows, and that one
# word cost a batch.
host_recovery() {
  say "host recovery: clearing modals and restarting the game core"
  pkill -f "Problem Reporter" 2>/dev/null && say "  killed a Problem Reporter modal"
  # pgrep lies about whether Civ 6 is really playable (civvis-civ6-the-launchpad-is-the-gate);
  # civ6_launch.py --restart is the supported way to get back to a usable main menu.
  ( cd $RUNNER && python3 -u tools/civ6_launch.py --restart --timeout 360 ) \
    >> $LOG 2>&1 || say "  civ6_launch.py --restart did not succeed"
}

# ---------------------------------------------------------------- the loop

# `source ~/civvis-batch-loop.sh` with this set defines the helpers and runs nothing,
# so the guards above can be exercised against the REAL definitions rather than a
# copy of them pasted into a test. A guard that has quietly stopped working looks
# exactly like a guard with nothing to report — see ~/civvis-sync.sh, which learned
# this the same way. Self-test after any edit:
#
#   zsh ~/civvis-batch-loop-selftest.zsh
#
# It plants a fault in BOTH directions (a process that must be seen, a process that
# must not be) and checks the guard against pgrep rather than against an assumption
# that the machine is idle — it is not supposed to be.
[[ ${CIVVIS_BATCH_LOOP_SELFTEST:-0} == 1 ]] && return 0

# ⚠ ONE DRIVER. Observed 2026-08-02 09:13, minutes before this script existed: two
# batches ran at once and the second recorded
#   "NO GAME — another run holds the game: a game is running under run tag
#    'civvis-20260802T131301Z'"
# then died. Both were started by hand, both pinned df475b9, and the surviving one's
# ledger rows cannot be told apart from rows played on an uncontended machine. The
# `run_is_live` guard below stops this loop starting on top of ANY batch; this lock
# stops a second copy of the loop itself, which the guard alone cannot (two copies
# would both see the same idle machine and both start).
if [[ -f $LOCK ]]; then
  held=$(<$LOCK)
  if [[ -n $held ]] && kill -0 $held 2>/dev/null; then
    # ⚠ Sleep BEFORE exiting. com.civvis.batchloop sets KeepAlive, so an immediate
    # exit is immediately restarted, and a copy that keeps losing the lock would
    # respawn every ~10 s for ever — a spin that writes a log line each time and
    # buries everything else in it.
    say "another batch loop is already running as pid $held; this copy is standing down"
    sleep $IDLE_S
    exit 0
  fi
  say "clearing a stale lock from pid ${held:-?}"
fi
print -r -- $$ > $LOCK
# Release it however this exits, so a crash does not lock the machine out of its own
# automation until a person notices.
trap 'rm -f $LOCK' EXIT INT TERM

say "=== batch loop starting: attempts=$ATTEMPTS nogame_limit=$NOGAME_LIMIT pid=$$ ==="

# ⚠⚠ macOS WILL NOT LET A launchd JOB EDIT ANOTHER APP'S BUNDLE, AND THE HARNESS HAS
# TO. The mod lives inside Civ6.app and `civ6_civvis_climb.py` re-installs it at the
# start of every attempt, on purpose, so a fix landed mid-batch takes effect. Under
# launchd that write returns `[Errno 1] Operation not permitted` — NOT a Unix
# permission (the file is martin:staff rw-) but TCC App Management, which is granted
# per executable and which an interactive shell here already has.
#
# Measured 2026-08-02 on this loop's first batch: preflight still passed, the batch
# still ran, and the only symptom was one line — `[teardown] could not clear the run
# tag` — followed by `installed source differs; harness syncs at attempt start`, a
# WARNING that quietly could not come true. That is the failure mode this project
# keeps paying for: the run continues and silently measures the previous mod.
#
# Checked once, at startup, and said loudly. It cannot be fixed from in here: TCC
# grants need a person.
APP_BUNDLE_PROBE="$HOME/Library/Application Support/Steam/steamapps/common/Sid Meier's Civilization VI/Civ6.app/Contents/Assets/DLC/CivvisControl/.civvis-tcc-probe"
if touch "$APP_BUNDLE_PROBE" 2>/dev/null; then
  rm -f "$APP_BUNDLE_PROBE"
  say "app-bundle write: OK — the mod can be synced at attempt start"
else
  say "⚠⚠ CANNOT WRITE INTO Civ6.app — EVERY ATTEMPT WILL DIE with"
  say "   'PermissionError: cannot install .../CivvisControl'. Preflight passes first,"
  say "   so this looks healthy right up until the first attempt fails."
  say "   This is macOS TCC, not file permissions, and it needs a person."
  say "   ⚠ The grant is APP MANAGEMENT, not Full Disk Access — measured on the context"
  say "   that works: it writes app bundles but cannot read ~/Library/Safari."
  say "     System Settings > Privacy & Security > App Management   (or Full Disk"
  say "     Access > + > Cmd-Shift-G > /bin/zsh, whose pane does have a + button)"
  say "   Verify with: zsh ~/civvis-tcc-probe.sh   — it tests a REAL launchd job,"
  say "   which a terminal cannot do (your prompt already holds the grant)."
  say "   Meanwhile run this loop from a terminal instead:"
  say "     unsetopt BG_NICE; nohup /bin/zsh ~/civvis-batch-loop.sh >> ~/civvis-civ6-runs/batch-loop.nohup.log 2>&1 &"
fi

while true; do
  if run_is_live; then
    say "a run is live; leaving the runner tree alone"
    sleep $IDLE_S
    continue
  fi

  # One fetch, into the shared object store. `-c gc.auto=0` because Homebrew git
  # segfaults on auto-maintenance here (git-automaintenance-segfaults), and
  # `maintenance.auto false` is already set globally for the same reason.
  if ! git -C $REPO -c gc.auto=0 fetch --quiet origin main 2>>$LOG; then
    say "fetch failed; retrying after $IDLE_S s"
    sleep $IDLE_S
    continue
  fi

  tip=$(git -C $REPO rev-parse origin/main)
  last_good=$(state_get last_good)
  target=$tip
  strikes=$(nogame_count $tip)

  # A tip that is not itself distrusted still has to wait out the cooling period if a
  # RECENT tip failed, because "new sha" is not evidence that the fault was fixed —
  # 23 of the 24 commits since the last working sha never touched the setup path at all.
  tip_failed_at=$(state_get tip_failed_at)
  if (( strikes < NOGAME_LIMIT )) && [[ -n $tip_failed_at && -n $last_good && $last_good != $tip ]]; then
    age=$(( $(date +%s) - tip_failed_at ))
    if (( age < TIP_RETRY_S )); then
      target=$last_good
      say "head ${tip:0:7} is untried, but a tip failed $((age / 60)) min ago; playing known-good ${last_good:0:7} for another $(( (TIP_RETRY_S - age) / 60 )) min"
    else
      say "cooling period over — giving head ${tip:0:7} another chance"
      state_set tip_failed_at ""
    fi
  fi

  if (( strikes >= NOGAME_LIMIT )); then
    if [[ -n $last_good && $last_good != $tip ]]; then
      target=$last_good
      say "⚠ tip $tip produced no game in $strikes batches; running last-good $last_good instead."
      say "  This is per-sha: the next commit to land on origin/main is tried immediately."
    else
      # Nothing better to fall back to. Running the tip again beats running nothing,
      # and the counter is cleared so the host gets a fresh set of chances.
      say "⚠ tip $tip produced no game in $strikes batches and there is no known-good fallback; trying it again."
      state_set nogame "$tip 0"
    fi
  fi

  if [[ -n $(git -C $RUNNER status --porcelain 2>/dev/null) ]]; then
    say "⚠ $RUNNER is DIRTY — refusing to move it. Uncommitted work there is not mine to discard."
    sleep $IDLE_S
    continue
  fi

  current=$(git -C $RUNNER rev-parse HEAD)
  if [[ $current != $target ]]; then
    if git -C $RUNNER checkout --detach --quiet $target 2>>$LOG; then
      say "runner advanced ${current:0:7} -> ${target:0:7}"
    else
      say "⚠ could not check out $target in the runner tree"
      sleep $IDLE_S
      continue
    fi
  else
    say "runner already at ${target:0:7}"
  fi

  # Rebuild the decider FROM THE RUNNER TREE. This is also what refreshes the league
  # snapshot the `auto` strategy ranks over, because league_dirs() resolves relative
  # to the binary's own path.
  say "building civvis_orders at ${target:0:7}"
  if ! ( cd $RUNNER && cargo build --release --bin civvis_orders ) >> $BUILDLOG 2>&1; then
    say "⚠ BUILD FAILED at ${target:0:7} — see $BUILDLOG"
    if [[ -n $last_good && $last_good != $target ]]; then
      say "  falling back to last-good $last_good for the next pass"
      state_set nogame "$target $NOGAME_LIMIT"
    fi
    sleep $IDLE_S
    continue
  fi

  # Ask the binary itself which strategy `auto` resolves to, rather than reimplementing
  # the ranking here and risking a provenance file that disagrees with the run.
  probe_dir=$(mktemp -d /tmp/civvis-genome-probe.XXXXXX)
  lane=$(victory_lane)
  # An unresolved lane must not become a bare `--victory` with nothing after it.
  # Skip the pass and say why: a checkout mid-swap fixes itself next time round,
  # and a permanent breakage names itself in the log instead of running a batch
  # under an objective nobody chose.
  if [[ -z $lane ]]; then
    say "cannot resolve the victory lane from $RUNNER; skipping this pass"
    sleep $IDLE_S
    continue
  fi
  rung=$(ladder_rung)
  # Same rule as the lane, and the same reason: an unresolved rung must not
  # become a bare `--difficulty`, and a batch run at a rung nobody chose is a
  # row that cannot be compared with the supervisor's.
  if [[ ! $rung =~ ^DIFFICULTY_(SETTLER|CHIEFTAIN|WARLORD|PRINCE|KING|EMPEROR|IMMORTAL|DEITY)$ ]]; then
    say "ladder policy returned invalid difficulty '${rung:-<empty>}'; skipping this pass"
    sleep $IDLE_S
    continue
  fi
  genome=$( ( cd $RUNNER && ./target/release/civvis_orders --mirror $probe_dir --turn 0 \
              --victory $lane --strategy auto ) 2>&1 >/dev/null \
            | grep -m1 '"kind":"genome"' )
  rm -rf $probe_dir
  [[ -z $genome ]] && genome='{"kind":"genome","strategy":"UNKNOWN — the probe printed nothing"}'
  say "strategy auto -> $genome"

  stamp=$(date -u +%Y%m%dT%H%M%SZ)
  batchlog=$RUNS/batch-loop-$stamp.log

  {
    print -r -- ""
    print -r -- "AUTO-ADVANCED $(date -u +%FT%TZ) by ~/civvis-batch-loop.sh (com.civvis.batchloop)."
    print -r -- "  tree      $RUNNER (DETACHED)"
    print -r -- "  sha       $target$([[ $target == $tip ]] && print -- '  = origin/main tip' || print -- "  (FALLBACK; origin/main tip is $tip)")"
    print -r -- "  subject   $(git -C $REPO log --format=%s -1 $target)"
    print -r -- "  strategy  $genome"
    # ⚠ This line is the only description of the batch most readers will see, and
    # it is written by hand beside the command it claims to describe. It said
    # `--war-from-plan` for hours after the flag was removed below. If you change
    # the invocation, change this in the same edit.
    print -r -- "  batch     --attempts $ATTEMPTS --victory $lane --difficulty $rung --strategy auto"
    print -r -- "  log       $batchlog"
  } >> $PROV

  # ⚠ RE-CHECK, don't trust the check at the top of the pass. The fetch, the checkout
  # and above all the release build are minutes of wall clock, and this machine has
  # had other things start batches during exactly that window. Checking once per pass
  # would leave a minutes-wide hole; checking here leaves a sub-second one.
  if run_is_live; then
    say "a run started while this pass was building; standing down rather than contending for the game"
    sleep $IDLE_S
    continue
  fi

  say "starting batch: $ATTEMPTS attempts pinned to ${target:0:7} -> $batchlog"
  # ⚠⚠⚠ --war-from-plan WAS REMOVED 2026-08-03 (later the same day). IT WAS
  # BYPASSING A GUARD, NOT EXERCISING AN OPTION. Read this before restoring it.
  #
  # `civ6_play.main` refuses `--civvis-war-from-plan` outright and says why: the
  # override declares on the plan's preferred rival even when the planner DECLINED
  # war, and live run `live-loop-rome-20260802-0800` forced one under a Religion
  # plan on turn 37, spent the remaining 213 turns in Recovery asking for peace,
  # and finished 400-1081. "A production launcher must not be able to bypass the
  # decider whose behavior it claims to measure."
  #
  # The flag reached the game anyway because `civ6_civvis_climb.py` started its OWN
  # `civ6_brain.py` beside the one `civ6_play` already runs — two deciders on one
  # orders.sqlite, differing on exactly this flag. PR #1041 leaves one decider, so
  # the climb now REFUSES this flag (exit 4) instead of routing around the guard.
  # Passing it here would abort every batch immediately.
  #
  # ⚠ The reasoning below is still true and still unaddressed — the declaration was
  # never attempted, not refused. It needs a fix inside CIVVIS's own diplomacy on a
  # rebuilt board, not a launcher override. Kept in full so the problem is not lost
  # with the workaround.
  #
  # ⚠ Rows either side of 2026-08-03 differ in BOTH directions: they gained this
  # flag in the morning and lost it in the evening. Do not read a before/after
  # difference across that day as a code effect.
  #
  # 96 of 123 corpus runs reaching turn 50 NEVER DECLARED WAR, and the corpus
  # holds only 58 `cannot_declare` refusals — all of them inside two runs. So
  # the declaration was not being refused, it was not being attempted. CIVVIS's
  # own diplomacy wants a casus belli or a denouncement matured over five turns,
  # and NOTHING matures on a board that --fresh-board rebuilds every turn, so
  # the decline is an artefact of the reconstruction rather than a judgement
  # about the war (see the `Decider` docstring in tools/civ6_brain.py). This
  # flag declares when the PLAN names a target.
  #
  # ⚠ This changes what the ladder measures. Rows are separated by `code_rev`,
  # but a code_rev boundary now also carries this configuration change — do not
  # read a before/after difference as a code effect.
  ( cd $RUNNER && python3 -u tools/civ6_civvis_climb.py \
      --attempts $ATTEMPTS \
      --victory $lane \
      --difficulty $rung \
      --strategy auto \
      --logs $RUNS/control ) >> $batchlog 2>&1
  rc=$?
  say "batch exit $rc"

  case $rc in
    0)
      say "*** a game was WON on ${target:0:7} ***"
      state_set last_good $target
      state_set nogame "$target 0"
      ;;
    1)
      # Games were played and none was won. That is a result about the AI, not about
      # the build — the build demonstrably runs Civilization VI, which is all
      # `last_good` claims.
      say "games played, none won on ${target:0:7} — recording it as a build that plays"
      state_set last_good $target
      state_set nogame "$target 0"
      ;;
    2)
      say "⚠ no decider binary — the build step and the batch disagree about where it lives"
      ;;
    3)
      strikes=$(( $(nogame_count $target) + 1 ))
      state_set nogame "$target $strikes"
      say "⚠ NO GAME on ${target:0:7} (strike $strikes/$NOGAME_LIMIT)"
      # Only the TIP failing starts the cooling clock. A no-game batch on the
      # known-good sha is the host misbehaving, not head, and must not buy head
      # extra credit — nor cost it any.
      if [[ $target == $tip ]] && (( strikes >= NOGAME_LIMIT )); then
        state_set tip_failed_at $(date +%s)
        say "  head is now distrusted; the known-good sha plays for the next $((TIP_RETRY_S / 60)) min"
      fi
      host_recovery
      ;;
    4)
      # Preflight refuses on a broken bridge, and the pin guard fires if the code
      # changed underneath. Neither is evidence against the sha, so neither counts a
      # strike; the next pass re-reads the tip and tries again.
      say "⚠ preflight failed or the code changed mid-batch — not blaming ${target:0:7}"
      ;;
    *)
      say "⚠ unexpected exit $rc from the climb tool"
      ;;
  esac

  sleep $IDLE_S
done

# Stop it with:
#   launchctl bootout gui/501/com.civvis.batchloop
# Start it again with:
#   launchctl enable gui/501/com.civvis.batchloop
#   launchctl bootstrap gui/501 ~/Library/LaunchAgents/com.civvis.batchloop.plist
# `enable` first is required — `bootstrap` silently no-ops on a disabled label
# (civvis-automation-stopped).
