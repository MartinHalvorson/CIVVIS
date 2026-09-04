#!/bin/zsh
# Keep Civ 6 games playing, one per fresh build of CIVVIS head.
#
# The climb pins a revision per batch and ENDS the batch when main moves
# ("THE BUILD CHANGED MID-BATCH"). That is correct for measurement and it is
# also the operator's requirement -- each game starts from a fresh build of
# head -- but nothing restarted it, so every merged PR silently stopped the
# games. This supervises that cycle: pull, build, play ONE game, repeat.
#
# ⚠⚠⚠ NEVER `pkill -9` CIVILIZATION VI HERE. Repeated hard kills wedge the game
# core: it relaunches, renders the menu, loads a map -- and the InGame gameplay
# context never starts, so the agent emits NOTHING. The harness then reads "no
# game started", runs `return_to_main_menu`, and that walks into "are you sure
# you wish to quit". Measured 2026-08-10: runs civvis-20260810T125137Z and
# ...T153127Z both ended with exactly 22 `autoclose_armed` events and ZERO
# `agent` events, after this script's own teardown had -9'd the game.
# `civ6_launch.py --stop/--restart` is the documented clean path and it cleared
# it in ten seconds. See civvis-civ6-the-game-core-wedges-after-hard-kills.
set -u
export PATH="$HOME/.cargo/bin:$PATH"
# ⚠ PINNED to a known-good revision while `main`'s mod is broken. HEAD reaches
# the in-game map and the agent context NEVER loads (22 autoclose events, zero
# agent events), so no turn is ever played. 597157c3 seats the agent and plays.
# Bisected 2026-08-10; the break is in bcb75f97 (#1474) or 8c9de1bf (#1478).
# Set CIVVIS_PIN=head to go back to tracking main once that is fixed.
# Re-read EVERY cycle from a one-line file, so the tree can be switched without
# killing a game in progress. Contents: an absolute path to the tree to play
# from, or "head" to track origin/main in $HEAD_REPO below.
PINFILE=${CIVVIS_PINFILE:-$HOME/.civvis-play-pin}
INTENTFILE=${CIVVIS_OPERATOR_INTENT_FILE:-${CIVVIS_INTENTFILE:-$HOME/.civvis-operator-intent}}
# How old a fetched `origin/main` may be and still be worth playing when GitHub
# is unreachable. See the refusal this guards, below. Six hours keeps an
# unattended overnight host producing games through a network blip without ever
# letting it report a night of results for a program main has moved past; `0`
# restores the old behaviour of refusing on any fetch failure.
HEAD_FETCH_GRACE_S=${CIVVIS_HEAD_FETCH_GRACE_S:-21600}
# ⚠⚠ THE TREE THIS SUPERVISOR PLAYS FROM IS DERIVED, NOT TYPED. It used to read
# `REPO=/Users/martin/CIVVIS`, which is a path that exists on exactly one
# machine in the fleet. Everywhere else the supervisor reached `cd "$REPO"`,
# logged "no tree at ...", slept 60s and did that forever — so on this host it
# was never installable and the ladder loop was hand-started from a terminal
# instead. That is how the loop came to be a process nobody supervised, and how
# 2026-08-17 lost 14.3 hours of attempts to a session that simply ended.
# `$0` is this script inside `<tree>/tools/ops/`, so the tree is three levels
# up. Resolved ONCE here, before the loop's `cd`, and overridable for a test.
HEAD_REPO=${CIVVIS_HEAD_REPO:-${0:A:h:h:h}}
LOGS=$HOME/civvis-climb-logs
# Named league genomes were retired in #2357.  The live decider now selects
# `AdvancedAi::new()` with the gene ledger applied when --strategy is ABSENT;
# that deployment genome has all and only its default-on genes.  civvis-orders
# deliberately rejects every --strategy value rather than silently running a
# different agent, so an inherited pre-retirement CIVVIS_STRATEGY must never be
# forwarded into a verification game.  Keep it only long enough to make the
# override visible in the supervisor log below.
REQUESTED_RETIRED_STRATEGY=${CIVVIS_STRATEGY:-}
# `CIVVIS_WITHOUT` is the explicit LIVE A/B gate for already registered
# treatments.  It is deliberately empty by default: a comma-separated value
# (for example `war-economy`) changes this batch's controller, and
# `civ6_play.py` writes the resolved withheld list into every summary.  Do not
# turn a native screen result into a deployment change here; name the arm,
# run its comparable control, and let the ladder read both.
WITHHELD=${CIVVIS_WITHOUT:-}
WITHOUT_ARGS=()
if [[ -n "$WITHHELD" ]]; then
  for treatment in ${(s:,:)WITHHELD}; do
    [[ -n "$treatment" ]] || continue
    WITHOUT_ARGS+=(--without "$treatment")
  done
fi
# `CIVVIS_WITH` is the matching explicit verification gate for a ledger-held
# live treatment.  Unlike `CIVVIS_WITHOUT`, it cannot change deployment: the
# decider accepts only a named treatment that the gene ledger has withheld,
# and records the forced list in every run's genome event and ladder row.
# Keep it empty in ordinary operation; a comma-separated value (for example
# `amenity-project-preemption`) is a labeled force-on arm, not a promotion.
FORCED_ENV=${CIVVIS_WITH:-}
# A GUI-capable interactive host cannot acquire a new environment variable
# after it has started, so an authorized operator otherwise has to restart the
# host merely to schedule a labeled arm.  Read this file only between batches:
# its absence (or an empty file) is exactly the ordinary deployment genome;
# its one comma-separated line is the same explicit `CIVVIS_WITH` value.  The
# per-batch read is deliberate: editing the file can never alter a game that is
# already running, and one three-attempt batch retains one recorded identity.
FORCE_FILE=${CIVVIS_WITH_FILE:-$HOME/.civvis-live-force-on}
FORCED=""
FORCE_SOURCE="none"
WITH_ARGS=()

# Resolve the force-on selection once at the no-game batch boundary.  A bad or
# conflicting operator request must stop before build/launch rather than fall
# through to an unlabelled control, because that would file the wrong arm under
# a plausible-looking ladder row.  Whitespace also rejects accidental multi-line
# files: treatment names are hyphenated tokens and the decider receives every
# comma member as its own quoted `--with` word below.
resolve_forced_arm() {
  local from_file=""
  FORCED="$FORCED_ENV"
  FORCE_SOURCE="environment"
  [[ -n "$FORCED" ]] || FORCE_SOURCE="none"

  if [[ -e "$FORCE_FILE" ]]; then
    if [[ ! -r "$FORCE_FILE" ]]; then
      say "force-on file exists but is unreadable ($FORCE_FILE); refusing batch"
      return 1
    fi
    from_file=$(<"$FORCE_FILE")
    if [[ "$from_file" == *[[:space:]]* ]]; then
      say "force-on file contains whitespace ($FORCE_FILE); refusing batch"
      return 1
    fi
    if [[ -n "$from_file" ]]; then
      if [[ -n "$FORCED" && "$FORCED" != "$from_file" ]]; then
        say "force-on file conflicts with CIVVIS_WITH; refusing batch"
        return 1
      fi
      FORCED="$from_file"
      FORCE_SOURCE="file:$FORCE_FILE"
    fi
  fi

  WITH_ARGS=()
  if [[ -n "$FORCED" ]]; then
    for treatment in ${(s:,:)FORCED}; do
      [[ -n "$treatment" ]] || continue
      WITH_ARGS+=(--with "$treatment")
    done
  fi
}
# `CIVVIS_SCREEN_GENE` / `~/.civvis-live-screen-gene`: one gene tag whose
# on/off arm every game of the batch is DEALT from its own run tag
# (`civ6_civvis_climb.py --screen-gene`, docs/LIVE_SCREEN.md).  Unlike the
# force-on file, a bad screen loses the SCREEN and the batch plays unarmed:
# the force file refuses a batch because a mislabelled arm would be filed as
# deployment, whereas an undealt screen is exactly deployment, and a typo in
# this file must not stop an unattended ladder.  Same strict one-line,
# no-whitespace format, read at the same no-game boundary.
SCREEN_FILE=${CIVVIS_SCREEN_GENE_FILE:-$HOME/.civvis-live-screen-gene}
SCREEN_ENV=${CIVVIS_SCREEN_GENE:-}
SCREEN_GENE=""
SCREEN_SOURCE="none"
SCREEN_ARGS=()

resolve_screen_gene() {
  local from_file=""
  SCREEN_GENE="$SCREEN_ENV"
  SCREEN_SOURCE="environment"
  [[ -n "$SCREEN_GENE" ]] || SCREEN_SOURCE="none"
  SCREEN_ARGS=()

  if [[ -e "$SCREEN_FILE" ]]; then
    if [[ ! -r "$SCREEN_FILE" ]]; then
      say "screen-gene file exists but is unreadable ($SCREEN_FILE); no screen this batch"
      SCREEN_GENE=""; SCREEN_SOURCE="none"
      return 0
    fi
    from_file=$(<"$SCREEN_FILE")
    if [[ "$from_file" == *[[:space:]]* ]]; then
      say "screen-gene file contains whitespace ($SCREEN_FILE); no screen this batch"
      SCREEN_GENE=""; SCREEN_SOURCE="none"
      return 0
    fi
    if [[ -n "$from_file" ]]; then
      if [[ -n "$SCREEN_GENE" && "$SCREEN_GENE" != "$from_file" ]]; then
        say "screen-gene file conflicts with CIVVIS_SCREEN_GENE; no screen this batch"
        SCREEN_GENE=""; SCREEN_SOURCE="none"
        return 0
      fi
      SCREEN_GENE="$from_file"
      SCREEN_SOURCE="file:$SCREEN_FILE"
    fi
  fi
  # One gene, one arm per game. A comma would name a bundle, which is a
  # force-on, not a screen.
  if [[ "$SCREEN_GENE" == *,* ]]; then
    say "screen-gene names several tags ($SCREEN_GENE); a screen deals ONE gene; no screen this batch"
    SCREEN_GENE=""; SCREEN_SOURCE="none"
    return 0
  fi
  # The batch's own arm wins: a gene both forced (or withheld) and screened
  # would have its coin overruled on every game.
  if [[ -n "$SCREEN_GENE" ]]; then
    local named
    for named in ${(s:,:)FORCED} ${(s:,:)WITHHELD}; do
      if [[ "$named" == "$SCREEN_GENE" ]]; then
        say "screen-gene $SCREEN_GENE is also this batch's forced/withheld arm; no screen this batch"
        SCREEN_GENE=""; SCREEN_SOURCE="none"
        return 0
      fi
    done
    SCREEN_ARGS=(--screen-gene "$SCREEN_GENE")
  fi
  return 0
}
# Attempts per cycle. One game per source revision cannot establish
# repeatability; the policy below advances only after a comparable trailing
# batch. Three is the smallest useful default and can be raised or lowered for
# an operator's host with CIVVIS_PLAY_ATTEMPTS.
ATTEMPTS=${CIVVIS_PLAY_ATTEMPTS:-3}
# A recorded desktop cannot tolerate the ordinary launcher's screenshots.  This
# opt-in profile is deliberately fixed rather than pretending every visual
# configuration has a capture-free equivalent: it is Rome / Emperor / Online /
# Continents / Small, and the in-game seat record remains the proof.
CAPTURE_FREE=${CIVVIS_CAPTURE_FREE:-0}
if [[ "$CAPTURE_FREE" != 0 && "$CAPTURE_FREE" != 1 ]]; then
  say "invalid CIVVIS_CAPTURE_FREE='${CAPTURE_FREE}'; refusing to choose a launcher"
  exit 64
fi
# The live ladder policy is read-only and chooses the lowest rung that still
# needs a first win or a repeatable trailing batch. CIVVIS_DIFFICULTY remains an
# explicit emergency/operator override; absent that override, the selected rung
# is always passed to civ6_civvis_climb rather than inherited from its default.
RUNS_DIR=$HOME/civvis-civ6-runs/control
EXPLICIT_DIFFICULTY=${CIVVIS_DIFFICULTY:-}
# The victory objective. This service passed NOTHING here, and inheriting a
# launcher default silently is how it spent 307 attempts aiming at Science —
# the one lane `victory_eval` completes 0/16 at this exact profile, while the
# hand-run batch loop was inheriting nothing and hard-coding `civvis`. The two
# production loops were running two different experiments into one ledger.
#
# Empty still means "the default the tree declares", which is now stated once in
# `civ6_play.DEFAULT_CIVVIS_VICTORY` with its evidence; the flag is passed only
# when this knob is set, so the default has exactly one home and this file does
# not become another copy of it. Set `CIVVIS_VICTORY` to pin a different lane.
VICTORY=${CIVVIS_VICTORY:-}
# ⚠⚠ ONE EXPANSION PER WORD IN THE INVOCATION BELOW. zsh does not word-split an
# unquoted `${VAR:+--flag "$VAR"}`: with the knob set it reaches the climb as
# ONE argument, `--victory science`, which argparse rejects as "unrecognized
# arguments" and the cycle plays no turns. The victory form had never been
# exercised (no host had set the knob); the abandon floor was, on 2026-08-19
# at 17:00Z, and four starts in a row played nothing. So the flag and its
# value are two expansions, `${VAR:+--flag} ${VAR:+"$VAR"}`, each one word or
# none; `tools/test_ops_ladder_objective.py` runs these lines under zsh.
# ★★★ Score is telemetry, not an automatic loss call.  This supervisor pins
# the legacy argument to zero even if an old host policy still exports a former
# threshold.  `civ6_play.py` independently enforces the same rule, so a direct
# or stale launcher cannot cut a verification game off early either.  Native
# operator `retire` remains the way to deliberately end a game.
REQUESTED_LEGACY_SCORE_RATIO=${CIVVIS_RESTART_BELOW_LEADER_RATIO:-}
RESTART_BELOW_LEADER_RATIO=0
# Optional live-host wall-clock budget. The climb's defaults remain the source
# of truth when these are absent; the operator can raise them for a GUI host
# whose healthy 250-turn games take longer. Run civvis-20260822T020434Z was
# still advancing and leading at turn 246 when the default 8,100-second ceiling
# stopped it, four turns before its autosave continuation won 1458-990. Keep
# each flag/value as argv words so a decimal setting reaches argparse intact.
PLAY_TIMEOUT=${CIVVIS_PLAY_TIMEOUT:-}
PLAY_TIMEOUT_CEILING=${CIVVIS_PLAY_TIMEOUT_CEILING:-}
TIMEOUT_ARGS=()
[[ -n "$PLAY_TIMEOUT" ]] && TIMEOUT_ARGS+=(--timeout "$PLAY_TIMEOUT")
[[ -n "$PLAY_TIMEOUT_CEILING" ]] \
    && TIMEOUT_ARGS+=(--timeout-ceiling "$PLAY_TIMEOUT_CEILING")
SUP=$LOGS/supervisor.log
MIRROR_HOME=$HOME/civvis-civ6-mirror
FOLLOW_LOG=$MIRROR_HOME/follow-nohup.log
MIRROR_PORT=${CIVVIS_MIRROR_PORT:-8610}
FOLLOW_REVISION_FILE=$MIRROR_HOME/follower-runtime-revision

say() { print -r -- "[$(date -u +%FT%TZ)] $*" >> "$SUP" }

if [[ -n "$REQUESTED_LEGACY_SCORE_RATIO" && "$REQUESTED_LEGACY_SCORE_RATIO" != 0 ]]; then
  say "ignoring legacy CIVVIS_RESTART_BELOW_LEADER_RATIO=$REQUESTED_LEGACY_SCORE_RATIO; verification games play to their in-game outcome"
fi

verification_intent_running() {
  [[ -r "$INTENTFILE" ]] && [[ "$(<"$INTENTFILE")" == running ]]
}

intent_reason() {
  if [[ -r "$INTENTFILE" ]]; then
    print -r -- "intent=$(<"$INTENTFILE")"
  else
    print -r -- "intent=missing"
  fi
}

mkdir -p "$LOGS"
if ! verification_intent_running; then
  say "verification intent is not running; exiting before startup ($(intent_reason))"
  exit 0
fi
if [[ -n "$REQUESTED_RETIRED_STRATEGY" ]]; then
  say "ignoring retired CIVVIS_STRATEGY=$REQUESTED_RETIRED_STRATEGY; live seat uses deployment genome (no --strategy)"
fi
say "supervisor up (genome=deployment, withheld=${WITHHELD:-none}, force_file=$FORCE_FILE, pinfile=$PINFILE)"
# The game inherits this shell's priority and macOS will not lower a nice once
# set, so a supervisor that starts demoted plays every game demoted. Say the
# number on every start; a non-zero one names the launch site to fix.
OWN_NICE=$(ps -o ni= -p $$ 2>/dev/null | tr -d " ")
if [[ "${OWN_NICE:-0}" != 0 ]]; then
  say "WARN supervisor is at nice ${OWN_NICE}: every game it starts will run below ordinary work. The launcher backgrounded it from a zsh with BG_NICE set; put 'unsetopt BG_NICE' before the '&' (see civvis-interactive-host.sh)"
else
  say "supervisor priority nice 0"
fi

# This runner can be started manually or by an interactive host wrapper. Two copies
# are not harmless: each believes it owns the one Civ VI installation and may
# tear down the other's attempt between turns.  Keep the ownership boundary
# outside the game lock (which belongs to the active harness), so a supervisor
# crash cannot leave the next launch unable to recover a stale game lock.
SUPERVISOR_LOCK=${CIVVIS_SUPERVISOR_LOCK:-$HOME/.civvis-game-supervisor.lock}
SUPERVISOR_PID_FILE=$SUPERVISOR_LOCK/pid

release_supervisor_lock() {
  local holder=""
  [[ -f "$SUPERVISOR_PID_FILE" ]] && holder=$(<"$SUPERVISOR_PID_FILE")
  if [[ "$holder" == "$$" ]]; then
    rm -rf -- "$SUPERVISOR_LOCK"
  fi
}

acquire_supervisor_lock() {
  if mkdir "$SUPERVISOR_LOCK" 2>/dev/null; then
    print -r -- "$$" > "$SUPERVISOR_PID_FILE"
    return 0
  fi

  local holder=""
  [[ -f "$SUPERVISOR_PID_FILE" ]] && holder=$(<"$SUPERVISOR_PID_FILE")
  if [[ -n "$holder" ]] && kill -0 "$holder" 2>/dev/null; then
    say "another game supervisor is already alive (pid $holder); exiting"
    return 1
  fi

  # A killed shell cannot run its EXIT trap.  The stale directory is ours alone
  # and its PID no longer exists, so reclaiming this exact path is safe.
  say "reclaiming stale supervisor lock (holder=${holder:-unknown})"
  rm -rf -- "$SUPERVISOR_LOCK"
  if ! mkdir "$SUPERVISOR_LOCK" 2>/dev/null; then
    say "could not acquire supervisor lock; retry through launchd"
    return 2
  fi
  print -r -- "$$" > "$SUPERVISOR_PID_FILE"
}

acquire_supervisor_lock
lock_status=$?
if (( lock_status != 0 )); then
  case $lock_status in
    1) exit 0 ;; # an intentional second invocation should not churn launchd
    *) exit 70 ;;
  esac
fi
trap release_supervisor_lock EXIT
trap 'exit 0' HUP INT TERM

consecutive_failures=0

# The installation lock establishes exclusive access to Civilization VI, but it
# does not identify which supervisor owns that access. A newly started
# supervisor can find a healthy harness inherited from an older supervisor (or
# an operator's recovery run) in the lock and mistake it for its own stale
# child. Its play log belongs to the older owner, so the orphan check would
# otherwise terminate a game that is still advancing.
#
# Treat a harness as ours only when it is actually below this supervisor in the
# process tree. Anything else is handled by `unowned_harness_pid` and left
# alone until its own harness exits.
supervisor_owns_process() {
  local cursor="$1" parent="" hops=0
  while [[ "$cursor" =~ '^[0-9]+$' ]] && (( hops < 64 )); do
    [[ "$cursor" == "$$" ]] && return 0
    parent=$(ps -p "$cursor" -o ppid= 2>/dev/null | tr -d '[:space:]')
    case "$parent" in
      ''|*[!0-9]*|0|1) return 1 ;;
    esac
    [[ "$parent" == "$cursor" ]] && return 1
    cursor="$parent"
    hops=$(( hops + 1 ))
  done
  return 1
}

# The harness publishes its exact PID before it starts interacting with Civ VI.
# Never accept an untyped global `pgrep` result as ownership: other CIVVIS
# worktrees (or an operator's separate run) can legitimately contain a
# civ6_play.py process.  An invalid or stale holder is deliberately *not* a
# reason to signal another process; the outer loop waits for an unowned run to
# finish instead.
owned_harness_pid() {
  local holder="$HOME/.civvis-civ6-game.lock/holder.json"
  local pid="" command=""
  [[ -r "$holder" ]] || return 1

  pid=$(sed -nE 's/^[[:space:]]*"pid"[[:space:]]*:[[:space:]]*([0-9]+).*/\1/p' "$holder" | head -n 1)
  case "$pid" in
    ''|*[!0-9]*) return 1 ;;
  esac
  kill -0 "$pid" 2>/dev/null || return 1
  command=$(ps -p "$pid" -o command= 2>/dev/null)
  [[ "$command" == *"civ6_play.py"* ]] || return 1
  supervisor_owns_process "$pid" || return 1
  print -r -- "$pid"
}

# A `pgrep` candidate alone is not a harness.  In particular, a diagnostic
# shell whose command text happens to mention civ6_play.py used to make the
# supervisor wait an extra minute at a clean batch boundary.  `ps -o comm` is
# truncated on macOS, so inspect lsof's first txt mapping instead: it is the
# actual executable path and lets us distinguish a Python harness from a shell
# that merely carries the filename as an argument.  We still leave a verified
# unowned harness completely alone.
unowned_harness_pid() {
  local pid="" command="" executable=""
  for pid in ${(f)"$(pgrep -f '[c]iv6_play.py' 2>/dev/null)"}; do
    kill -0 "$pid" 2>/dev/null || continue
    command=$(ps -p "$pid" -o command= 2>/dev/null)
    [[ "$command" == *"civ6_play.py"* ]] || continue
    executable=$(lsof -a -p "$pid" -d txt -Fn 2>/dev/null | sed -n 's/^n//p' | head -n 1)
    case "$executable" in
      */Python|*/python|*/python[0-9]*) ;;
      *) continue ;;
    esac
    print -r -- "$pid"
    return 0
  done
  return 1
}

# The live display is a shared machine-level resource, so its owner must be
# identified by the log it has open rather than by every `tools/follow.py` on
# the machine.  This keeps one supervisor from tearing down an unrelated
# worktree's follower during a batch handoff.
display_follower_pid() {
  local pid="" stderr_path=""
  for pid in ${(f)"$(pgrep -f '[t]ools/follow.py' 2>/dev/null)"}; do
    stderr_path=$(lsof -a -p "$pid" -d 1 -Fn 2>/dev/null | sed -n 's/^n//p' | head -n 1)
    if [[ "$stderr_path" == "$FOLLOW_LOG" ]]; then
      print -r -- "$pid"
      return 0
    fi
  done
  return 1
}

# A server can outlive its follower because follow.py deliberately gives the
# mirror a new session.  Port 8610 and the command shape together identify the
# visible CIVVIS server without sweeping up other civvis processes.
display_mirror_server_pid() {
  local pid="" command=""
  for pid in ${(f)"$(lsof -tiTCP:"$MIRROR_PORT" -sTCP:LISTEN 2>/dev/null)"}; do
    command=$(ps -p "$pid" -o command= 2>/dev/null)
    if [[ "$command" == *"civvis play --mirror"* ]]; then
      print -r -- "$pid"
      return 0
    fi
  done
  return 1
}

while true; do
  if ! verification_intent_running; then
    say "verification intent is not running; exiting before the next game ($(intent_reason))"
    exit 0
  fi
  # ⚠ NEVER kill a HEALTHY game here. A global `pkill -f civ6_play.py` used to
  # terminate unrelated live runs. Wait for this supervisor's lock holder to
  # go quiet for two minutes, then ask only that exact harness to stop cleanly.
  while true; do
    OWNED_PID=$(owned_harness_pid || true)
    if [[ -z "$OWNED_PID" ]]; then
      UNOWNED_PID=$(unowned_harness_pid || true)
      if [[ -n "$UNOWNED_PID" ]]; then
        say "an unowned Civ VI harness is present (pid $UNOWNED_PID); leaving it alone and retrying in 60s"
        sleep 60
        continue
      fi
      break
    fi
    NEWEST=$(ls -t "$LOGS"/civvis-*-play.log 2>/dev/null | head -1)
    if [[ -n "$NEWEST" ]]; then
      AGE=$(( $(date +%s) - $(stat -f %m "$NEWEST") ))
    else
      AGE=999
    fi
    if (( AGE < 120 )); then
      say "a live game is still playing (log touched ${AGE}s ago); waiting for it"
      sleep 60
    else
      say "owned harness pid $OWNED_PID is orphaned (log silent ${AGE}s); requesting clean stop"
      kill -TERM "$OWNED_PID" 2>/dev/null || true
      for _ in {1..15}; do
        kill -0 "$OWNED_PID" 2>/dev/null || break
        sleep 1
      done
      if kill -0 "$OWNED_PID" 2>/dev/null; then
        say "owned harness pid $OWNED_PID is still alive after TERM; leaving it alone and retrying in 60s"
        sleep 60
        continue
      fi
      break
    fi
  done
  rm -rf "$HOME/.civvis-civ6-game.lock"

  if ! pgrep -x steam_osx >/dev/null; then
    say "steam is down; starting it"
    open -a Steam
    sleep 45
  fi

  # A wedged core is invisible until a game fails to start, so recycle the game
  # cleanly whenever the previous attempt did not play a turn.
  if (( consecutive_failures > 0 )); then
    # The next climb must own its launch: an unowned game parked at the menu
    # makes `busy()` refuse to start it. But a failed graceful quit is not
    # authority to SIGKILL Civ VI — that can leave its saves half-written. The
    # environment helper sends one ordinary TERM and reports failure; preserve
    # a game that does not exit and retry the verified boundary later.
    say "previous attempt started no game; requesting a graceful Civ 6 stop before the next owned launch"
    if ! PYTHONPATH="$HEAD_REPO/tools${PYTHONPATH:+:$PYTHONPATH}" \
        python3 -c 'import civ6_env, sys; sys.exit(0 if civ6_env.quit_game(timeout_s=45.0) else 1)' \
        >>"$SUP" 2>&1; then
      say "Civ 6 did not exit after the graceful stop; preserving it and retrying in 60s"
      sleep 60
      continue
    fi
  fi

  PIN=$(cat "$PINFILE" 2>/dev/null || print -r -- head)
  if [[ "$PIN" == "head" ]]; then
    REPO=$HEAD_REPO
  else
    REPO=$PIN
  fi
  cd "$REPO" || { say "no tree at $REPO"; sleep 60; continue }
  # `cd /` SUCCEEDS, so a bad HEAD_REPO derivation passes the guard above and
  # surfaces downstream as `could not find Cargo.toml in '/'` plus
  # "build FAILED at <empty sha>" every 120s with no game. Measured
  # 2026-08-17 22:33Z: these exact bytes, synced to the legacy home copy
  # (~/civvis-game-supervisor.sh) and run from there, derive three dirnames
  # up as `/`. Refuse the cycle with the derivation spelled out instead of
  # looping a cryptic build failure.
  if [[ ! -f "$REPO/Cargo.toml" ]]; then
    say "REFUSING cycle: no Cargo.toml at '$REPO' (pin='$PIN', HEAD_REPO='$HEAD_REPO', script ${0:A}); set CIVVIS_HEAD_REPO or run the tracked copy under tools/ops/; retrying in 300s"
    sleep 300
    continue
  fi
  rm -f status.json                     # tools/follow.py dirties the tree
  if [[ "$PIN" == "head" ]]; then
    # This tree normally has a detached HEAD.  `git pull` can leave that HEAD
    # unchanged while its failure is hidden, so a batch appears healthy but
    # keeps replaying an old decider.  Fetch and detach-checkout explicitly,
    # then refuse the cycle unless the checkout reads back as exact main.
    if ! git -c gc.auto=0 fetch --quiet origin main >>"$SUP" 2>&1; then
      # ⚠⚠ A REFUSAL HERE USED TO BE UNCONDITIONAL, AND ON 2026-08-28 THAT COST
      # THREE HOURS OF ZERO GAMES. github.com became unreachable from this host
      # (ping and DNS fine, example.com 200, github.com:443 timing out after
      # 75 s) and every 120 s cycle logged "could not fetch origin/main" and
      # played nothing -- while a checkout of origin/main fetched minutes
      # earlier sat right there, buildable.
      #
      # "Stale" is about the AGE of what we hold, not about whether the network
      # answered. A fetch we made within the grace window below is still the
      # program the operator is verifying; refusing to play it trades a small,
      # bounded staleness for a total outage, which is the worse of the two.
      # Past the window, refuse exactly as before: an unattended host must not
      # spend a night reporting results for a program main has moved past.
      ORIGIN_MAIN_REF=$(git rev-parse --git-path refs/remotes/origin/main 2>/dev/null || true)
      FETCH_AGE_S=""
      if [[ -n "$ORIGIN_MAIN_REF" && -f "$ORIGIN_MAIN_REF" ]]; then
        FETCH_AGE_S=$(( $(date +%s) - $(stat -f %m "$ORIGIN_MAIN_REF") ))
      fi
      if [[ -n "$FETCH_AGE_S" ]] && (( FETCH_AGE_S <= HEAD_FETCH_GRACE_S )); then
        say "could not fetch origin/main; playing the origin/main this tree fetched ${FETCH_AGE_S}s ago (grace ${HEAD_FETCH_GRACE_S}s)"
      else
        say "could not fetch origin/main and the last fetch is ${FETCH_AGE_S:-unknown}s old (grace ${HEAD_FETCH_GRACE_S}s); refusing to run a stale head batch; retrying in 120s"
        sleep 120
        continue
      fi
    fi
    if ! git checkout --quiet --detach origin/main >>"$SUP" 2>&1; then
      # ⚠⚠ NAME THE CAUSE. A dirty head tree is the ordinary way this fails and
      # the generic message hid it completely: on 2026-08-28 another agent was
      # testing an uncommitted harness fix inside the head tree, git refused
      # ("Please commit your changes or stash them before you switch branches"),
      # and the ladder logged "could not checkout fetched origin/main" every
      # 120s with no hint of which file or what to do about it. The raw git
      # output does go to this log, but it lands ABOVE the timestamped line an
      # operator reads, so the diagnosis was several minutes of `git status`
      # that the loop could have printed itself.
      #
      # Still a refusal: a head batch that cannot be proved exact origin/main
      # must not be filed as one. What changes is that the message says why and
      # names the remedy — including the pin, which is the whole point of
      # `~/.civvis-play-pin` and lets a tree be played AS-IS, dirt included,
      # while somebody is deliberately testing in it.
      DIRTY_FILES=$(git status --porcelain 2>/dev/null | awk '{print $NF}' | head -3 | tr '\n' ' ')
      if [[ -n "$DIRTY_FILES" ]]; then
        DIRTY_COUNT=$(git status --porcelain 2>/dev/null | wc -l | tr -d ' ')
        say "could not checkout fetched origin/main: THE HEAD TREE $REPO IS DIRTY (${DIRTY_COUNT} uncommitted path(s): ${DIRTY_FILES}), so a head batch cannot be proved exact. Commit or stash them, or put an absolute tree path in $PINFILE to play that tree as-is. Retrying in 120s"
      else
        say "could not checkout fetched origin/main; refusing to run a stale head batch; retrying in 120s"
      fi
      sleep 120
      continue
    fi
  fi
  HEAD_SHA=$(git rev-parse HEAD 2>/dev/null || true)
  if [[ ! "$HEAD_SHA" =~ ^[0-9a-f]{40}$ ]]; then
    say "could not resolve the batch checkout HEAD; refusing to run an unverified batch; retrying in 120s"
    sleep 120
    continue
  fi
  # Logs and the follower-runtime marker use the readable short revision, but
  # the native mirror's /status reports its launch stamp. Keep the exact
  # fetched SHA before shortening it so `ship` can prove the visible mirror is
  # the same build that the supervisor just selected.
  HEAD_REVISION=$HEAD_SHA
  HEAD_COMMIT_TIME=$(git show -s --format=%cI HEAD 2>/dev/null || true)
  if [[ "$PIN" == "head" ]]; then
    ORIGIN_MAIN_SHA=$(git rev-parse origin/main 2>/dev/null || true)
    if [[ "$HEAD_SHA" != "$ORIGIN_MAIN_SHA" ]]; then
      say "checkout $HEAD_SHA does not equal fetched origin/main ${ORIGIN_MAIN_SHA:-<unresolved>}; refusing to run a stale head batch; retrying in 120s"
      sleep 120
      continue
    fi
  fi
  if ! resolve_forced_arm; then
    sleep 60
    continue
  fi
  # After the arm: a screen defers to a forced/withheld gene of the same name.
  resolve_screen_gene
  HEAD_SHA=${HEAD_REVISION:0:7}
  if ! cargo build --release --bin civvis_orders --bin civvis >>"$SUP" 2>&1; then
    say "build FAILED at $HEAD_SHA; retrying in 120s"
    sleep 120
    continue
  fi
  # A clean release build can take minutes.  Main may advance while it runs,
  # so the checkout that was fresh before the build is not necessarily fresh
  # when the UI begins a new game.  Re-fetch at this boundary: when it moved,
  # leave this built-but-stale binary unused and rebuild the new exact head.
  if [[ "$PIN" == "head" ]]; then
    if ! git -c gc.auto=0 fetch --quiet origin main >>"$SUP" 2>&1; then
      # The build we are about to launch was cut from a checkout this cycle
      # already accepted above, so a network that dropped DURING the build is
      # not a reason to throw that build away and try again into the same dead
      # network. Launch it and say the recheck did not happen.
      say "could not recheck origin/main after fresh build; launching the head this cycle already verified (${HEAD_REVISION:0:7})"
      ORIGIN_MAIN_AFTER_BUILD=$HEAD_REVISION
    else
      ORIGIN_MAIN_AFTER_BUILD=$(git rev-parse origin/main 2>/dev/null || true)
    fi
    if [[ ! "$ORIGIN_MAIN_AFTER_BUILD" =~ ^[0-9a-f]{40}$ ]]; then
      say "could not resolve origin/main after fresh build; refusing to launch an unverified batch; retrying in 120s"
      sleep 120
      continue
    fi
    if [[ "$HEAD_REVISION" != "$ORIGIN_MAIN_AFTER_BUILD" ]]; then
      say "origin/main advanced from ${HEAD_REVISION:0:7} to ${ORIGIN_MAIN_AFTER_BUILD:0:7} during fresh build; rebuilding exact head before launch"
      continue
    fi
  fi

  DIFFICULTY=$EXPLICIT_DIFFICULTY
  if [[ -z "$DIFFICULTY" ]]; then
    if [[ ! -f "$REPO/tools/civ6_ladder_policy.py" ]]; then
      say "ladder policy missing at $REPO/tools/civ6_ladder_policy.py; refusing an un-gated run"
      sleep 300
      continue
    fi
    DIFFICULTY=$(python3 "$REPO/tools/civ6_ladder_policy.py" \
      --runs "$RUNS_DIR" target 2>>"$SUP") || DIFFICULTY=""
  fi
  if [[ ! "$DIFFICULTY" =~ ^DIFFICULTY_(SETTLER|CHIEFTAIN|WARLORD|PRINCE|KING|EMPEROR|IMMORTAL|DEITY)$ ]]; then
    say "ladder policy returned invalid difficulty '${DIFFICULTY:-<empty>}'; refusing an ungated run"
    sleep 300
    continue
  fi

  # ⚠⚠⚠ THE MIRROR SERVER SERVES `/assets/app.js` FROM ITS CWD, while the page's
  # `index.html` is EMBEDDED IN THE BINARY. Run it from a tree whose app.js does
  # not match that binary and one bad top-level lookup blanks the whole map --
  # the sidebar, buttons and title still paint, so it reads as "CIVVIS is up but
  # the game is not showing". Cost most of an afternoon on 2026-08-10.
  # So: whenever the tree *or its built revision* changes, restart the follower
  # FROM $REPO so binary and assets are the same revision by construction.  A
  # cwd check alone is insufficient: `git pull` changes the same checkout path
  # in place, while a long-lived mirror server still has the previous binary.
  FOLLOW_PID=$(display_follower_pid || true)
  FOLLOW_CWD=""
  if [[ -n "$FOLLOW_PID" ]]; then
    FOLLOW_CWD=$(lsof -a -d cwd -p "$FOLLOW_PID" -Fn 2>/dev/null \
        | sed -n 's/^n//p' | head -1)
  fi
  FOLLOW_REVISION=""
  [[ -f "$FOLLOW_REVISION_FILE" ]] && FOLLOW_REVISION=$(<"$FOLLOW_REVISION_FILE")
  if [[ "$FOLLOW_CWD" != "$REPO" || "$FOLLOW_REVISION" != "$HEAD_SHA" ]]; then
    say "display mirror is cwd='${FOLLOW_CWD:-absent}' revision='${FOLLOW_REVISION:-unknown}', want cwd='$REPO' revision='$HEAD_SHA'; refreshing exact mirror owners"
    if [[ -n "$FOLLOW_PID" ]] && kill -0 "$FOLLOW_PID" 2>/dev/null; then
      kill -TERM "$FOLLOW_PID" 2>/dev/null || true
      sleep 2
    fi
    MIRROR_PID=$(display_mirror_server_pid || true)
    if [[ -n "$MIRROR_PID" ]] && kill -0 "$MIRROR_PID" 2>/dev/null; then
      kill -TERM "$MIRROR_PID" 2>/dev/null || true
      sleep 2
    fi
    mkdir -p "$MIRROR_HOME"
    # `civvis play --mirror` inherits these launch stamps through follow.py.
    # It cannot infer the selected revision from a shared checkout, because
    # that checkout advances in place while the mirror keeps serving.
    ( cd "$REPO" && CIVVIS_COMMIT="$HEAD_REVISION" \
        CIVVIS_COMMIT_TIME="$HEAD_COMMIT_TIME" \
        nohup python3 -u tools/follow.py \
        > "$FOLLOW_LOG" 2>&1 & )
    print -r -- "$HEAD_SHA" > "$FOLLOW_REVISION_FILE.$$.tmp"
    mv -f "$FOLLOW_REVISION_FILE.$$.tmp" "$FOLLOW_REVISION_FILE"
    sleep 5
  fi

  # The ownership scan above runs before the fetch/build/mirror-preflight
  # work.  That work is long enough for an independent harness to start and
  # claim Civ VI after the scan; launching into it produces an instant
  # "something already holds the game" failure and falsely burns a ladder
  # cycle.  Recheck at the launch boundary and leave the other owner alone.
  if ! verification_intent_running; then
    say "verification intent is not running; exiting before launch ($(intent_reason))"
    exit 0
  fi
  LAUNCH_UNOWNED_PID=$(unowned_harness_pid || true)
  if [[ -n "$LAUNCH_UNOWNED_PID" ]]; then
    say "an unowned Civ VI harness appeared during preflight (pid $LAUNCH_UNOWNED_PID); leaving it alone and retrying in 60s"
    sleep 60
    continue
  fi

  TAG=$(date -u +%Y%m%dT%H%M%SZ)
  say "starting $ATTEMPTS attempt(s) on $HEAD_SHA at $DIFFICULTY (capture_free=$CAPTURE_FREE, forced=${FORCED:-none}, source=$FORCE_SOURCE, screen=${SCREEN_GENE:-none}, screen_source=$SCREEN_SOURCE, log climb-$TAG.log)"
  # The success check below must not read a PREVIOUS cycle's play log. A climb
  # that exits before creating one — 2026-08-15T11:07:31Z: "something already
  # holds the game; refusing to stop an unowned run", gone in under a second —
  # left `ls -t` pointing at the finished game's log, which scored the failed
  # cycle as "played turns: 293" and reset consecutive_failures. Under a
  # persistent instant failure that misread would spin fast forever and the
  # recovery/backoff arms below would never fire. Mark the cycle start; only a
  # play log written after the mark can vouch for this cycle.
  CYCLE_MARK=$LOGS/.cycle-start
  : > "$CYCLE_MARK"
  # The climb retires the follower above and starts its own fresh one before
  # the attempt.  Carry the exact selected revision under mirror-only names;
  # the climb promotes them only into that replacement follower, so the game
  # controller never inherits the display server's provenance stamp.
  # The preflight above selected and built this exact head.  A decision worker
  # that re-execs onto a newer GitHub revision mid-game turns one fresh-build
  # game into an unrepeatable mixture of programs; fetch and build again only
  # at the next game boundary.
  # Every production ladder row is Rome.  Do not inherit the climb's default:
  # an upstream default change must not silently change the civilization that
  # the live ledger compares.
  if (( CAPTURE_FREE )); then
    # `civ6_capture_free_loop.py` never captures the desktop.  It owns the
    # direct Create Game/attach lifecycle and writes the same per-game play-log
    # marker the success check below already understands.  A live screen gene
    # is visual instrumentation, so this fixed no-capture profile cannot deal
    # it; log that fact rather than silently claiming an armed screen.
    if [[ -n "$SCREEN_GENE" ]]; then
      say "capture-free batch skips screen gene '$SCREEN_GENE' (no desktop capture)"
    fi
    CIVVIS_MIRROR_COMMIT="$HEAD_REVISION" \
    CIVVIS_MIRROR_COMMIT_TIME="$HEAD_COMMIT_TIME" \
    python3 -u tools/civ6_capture_free_loop.py --attempts "$ATTEMPTS" \
        --refresh-seconds 0 \
        --difficulty "$DIFFICULTY" \
        --leader LEADER_TRAJAN \
        --ruleset RULESET_EXPANSION_2 \
        --map Continents.lua \
        --map-size MAPSIZE_SMALL \
        --speed GAMESPEED_ONLINE \
        --max-turns 650 \
        "${WITHOUT_ARGS[@]}" \
        "${WITH_ARGS[@]}" \
        "${TIMEOUT_ARGS[@]}" \
        ${VICTORY:+--victory} ${VICTORY:+"$VICTORY"} \
        --logs "$LOGS" > "$LOGS/climb-$TAG.log" 2>&1
  else
    CIVVIS_MIRROR_COMMIT="$HEAD_REVISION" \
    CIVVIS_MIRROR_COMMIT_TIME="$HEAD_COMMIT_TIME" \
    python3 -u tools/civ6_civvis_climb.py --attempts "$ATTEMPTS" \
        --refresh-seconds 0 \
        --difficulty "$DIFFICULTY" \
        --leader LEADER_TRAJAN \
        "${WITHOUT_ARGS[@]}" \
        "${WITH_ARGS[@]}" \
        "${SCREEN_ARGS[@]}" \
        "${TIMEOUT_ARGS[@]}" \
        ${VICTORY:+--victory} ${VICTORY:+"$VICTORY"} \
        ${RESTART_BELOW_LEADER_RATIO:+--restart-below-leader-ratio} ${RESTART_BELOW_LEADER_RATIO:+"$RESTART_BELOW_LEADER_RATIO"} \
        --logs "$LOGS" > "$LOGS/climb-$TAG.log" 2>&1
  fi

  # "Played a turn" is the only honest success test: a run can reach the map,
  # emit autoclose events and still never seat the agent. Count every play log
  # this cycle wrote, not the newest one: in a batch the last attempt can fail
  # while earlier ones played, and the newest-only read would score the whole
  # batch as a failure.
  played_games=0
  for PLAY in "$LOGS"/civvis-*-play.log(N); do
    [[ "$PLAY" -nt "$CYCLE_MARK" ]] || continue
    grep -qE "^\[turn [0-9]+\]" "$PLAY" 2>/dev/null && played_games=$((played_games + 1))
  done
  if (( played_games > 0 )); then
    consecutive_failures=0
    say "cycle on $HEAD_SHA played $played_games game(s) with turns"
  else
    consecutive_failures=$((consecutive_failures + 1))
    say "game on $HEAD_SHA PLAYED NO TURNS (failure $consecutive_failures)"
  fi

  if (( consecutive_failures >= 4 )); then
    say "four starts in a row played nothing; sleeping 10m before trying again"
    sleep 600
    consecutive_failures=1
  fi

  sleep 10
done
