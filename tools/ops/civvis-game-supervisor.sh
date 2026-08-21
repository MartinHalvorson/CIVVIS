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
# `CIVVIS_STRATEGY` is the explicit genome gate for this host. Pin the stable
# internal identity, not a leaderboard display label: Rome's selected
# `g56-48` bound is 0.510 versus the generic fallback's 0.393, and every row
# records both this request and the resolved genome. The resolver also accepts
# a unique display label for compatibility with older launchers, but a `g*-*`
# pin keeps this batch comparable while the candidate is measured in Civ VI.
# Set `CIVVIS_STRATEGY=auto` to opt into per-civilization re-selection, or
# `CIVVIS_STRATEGY=stock` to run the untuned control.
STRATEGY=${CIVVIS_STRATEGY:-g56-48}
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
FORCED=${CIVVIS_WITH:-}
WITH_ARGS=()
if [[ -n "$FORCED" ]]; then
  for treatment in ${(s:,:)FORCED}; do
    [[ -n "$treatment" ]] || continue
    WITH_ARGS+=(--with "$treatment")
  done
fi
# Attempts per cycle. One game per source revision cannot establish
# repeatability; the policy below advances only after a comparable trailing
# batch. Three is the smallest useful default and can be raised or lowered for
# an operator's host with CIVVIS_PLAY_ATTEMPTS.
ATTEMPTS=${CIVVIS_PLAY_ATTEMPTS:-3}
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
# The floor under a game's expected win rate, below which the harness stops
# playing it out (`civ6_play.ABANDON_CELLS`: the ladder's own measured table;
# 0/34 of the live games that were under three quarters of the best rival's
# score for five turns past turn 120 ever won). Unset means every game is
# played to its end. Operator request 2026-08-19: "ok to abandon games early
# if expected win rate <5%" — set `CIVVIS_ABANDON_BELOW_WIN_RATE=0.05` in the
# login shell the launcher inherits (~/.zprofile on this host, beside
# CIVVIS_DIFFICULTY), and an abandoned game is filed as `abandoned`, never as
# a stall or a defeat.
ABANDON_BELOW=${CIVVIS_ABANDON_BELOW_WIN_RATE:-}
SUP=$LOGS/supervisor.log
MIRROR_HOME=$HOME/civvis-civ6-mirror
FOLLOW_LOG=$MIRROR_HOME/follow-nohup.log
MIRROR_PORT=${CIVVIS_MIRROR_PORT:-8610}
FOLLOW_REVISION_FILE=$MIRROR_HOME/follower-runtime-revision

say() { print -r -- "[$(date -u +%FT%TZ)] $*" >> "$SUP" }

mkdir -p "$LOGS"
say "supervisor up (strategy=$STRATEGY, withheld=${WITHHELD:-none}, forced=${FORCED:-none}, pinfile=$PINFILE)"

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
    # ⚠ --stop, NOT --restart. The climb's `busy()` refuses to touch a running
    # Civ 6 it cannot prove it owns ("refusing to stop an unowned run", exit 3)
    # — and --restart hands it exactly that: a relaunched, unowned game parked
    # at the menu. Measured 2026-08-15 12:30–12:35Z: restart → climb refuses →
    # failure → restart, four times in five minutes into the ten-minute nap,
    # with each "attempt" lasting under a second. The remedy must leave the
    # game DOWN; the climb launches Civ 6 itself and then owns what it
    # launched. A wedged core is equally cleared by a stop.
    say "previous attempt started no game; stopping Civ 6 so the next climb owns its launch"
    python3 tools/civ6_launch.py --stop >>"$SUP" 2>&1
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
      say "could not fetch origin/main; refusing to run a stale head batch; retrying in 120s"
      sleep 120
      continue
    fi
    if ! git checkout --quiet --detach origin/main >>"$SUP" 2>&1; then
      say "could not checkout fetched origin/main; refusing to run a stale head batch; retrying in 120s"
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
  if [[ "$PIN" == "head" ]]; then
    ORIGIN_MAIN_SHA=$(git rev-parse origin/main 2>/dev/null || true)
    if [[ "$HEAD_SHA" != "$ORIGIN_MAIN_SHA" ]]; then
      say "checkout $HEAD_SHA does not equal fetched origin/main ${ORIGIN_MAIN_SHA:-<unresolved>}; refusing to run a stale head batch; retrying in 120s"
      sleep 120
      continue
    fi
  fi
  HEAD_SHA=${HEAD_SHA:0:7}
  if ! cargo build --release --bin civvis_orders --bin civvis >>"$SUP" 2>&1; then
    say "build FAILED at $HEAD_SHA; retrying in 120s"
    sleep 120
    continue
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
    ( cd "$REPO" && nohup python3 -u tools/follow.py \
        > "$FOLLOW_LOG" 2>&1 & )
    print -r -- "$HEAD_SHA" > "$FOLLOW_REVISION_FILE.$$.tmp"
    mv -f "$FOLLOW_REVISION_FILE.$$.tmp" "$FOLLOW_REVISION_FILE"
    sleep 5
  fi

  TAG=$(date -u +%Y%m%dT%H%M%SZ)
  say "starting $ATTEMPTS attempt(s) on $HEAD_SHA at $DIFFICULTY (log climb-$TAG.log)"
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
  python3 -u tools/civ6_civvis_climb.py --attempts "$ATTEMPTS" \
      --difficulty "$DIFFICULTY" --strategy "$STRATEGY" \
      "${WITHOUT_ARGS[@]}" \
      "${WITH_ARGS[@]}" \
      ${VICTORY:+--victory} ${VICTORY:+"$VICTORY"} \
      ${ABANDON_BELOW:+--abandon-below-win-rate} ${ABANDON_BELOW:+"$ABANDON_BELOW"} \
      --logs "$LOGS" > "$LOGS/climb-$TAG.log" 2>&1

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
