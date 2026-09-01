#!/bin/zsh
# Retire the intentionally frozen legacy host only after the protected King
# recovery reaches a real Civ VI ending, then start the normal fresh-head loop.
#
# This runs outside the game controller.  It never signals a live Civ VI game,
# and it refuses to treat a harness failure as a completed promising game.
set -euo pipefail
unsetopt BG_NICE

ROOT=/Users/martbot-mbp-m5-max-128
TAG=civvis-20260828T020050Z-promising-resume142c
RUN="$ROOT/civvis-civ6-runs/control/$TAG"
WRAPPER="$ROOT/civvis-verified-head-launcher.zsh"
LOG="$ROOT/civvis-climb-logs/${TAG}-post-handoff.log"
SUPERVISOR_LOCK="$ROOT/.civvis-game-supervisor.lock/pid"
HOST_LOCK="$ROOT/.civvis-interactive-host.lock/pid"

say() {
  print -r -- "[post-promising-handoff] $(date -u +%FT%TZ) $*" >> "$LOG"
}

pid_from_lock() {
  local file=$1 expected=$2 pid='' command=''
  [[ -r "$file" ]] || return 1
  pid=$(<"$file")
  [[ "$pid" == <-> ]] || return 1
  kill -0 "$pid" 2>/dev/null || return 1
  command=$(ps -p "$pid" -o command= 2>/dev/null) || return 1
  [[ "$command" == *"$expected"* ]] || return 1
  print -r -- "$pid"
}

is_live_python_player() {
  local pid=$1 command='' executable=''
  [[ "$pid" == <-> ]] || return 1
  kill -0 "$pid" 2>/dev/null || return 1
  command=$(ps -p "$pid" -o command= 2>/dev/null) || return 1
  [[ "$command" == *"civ6_play.py"* ]] || return 1
  executable=$(lsof -a -p "$pid" -d txt -Fn 2>/dev/null | sed -n 's/^n//p' | head -n 1)
  case "$executable" in
    */Python|*/python|*/python[0-9]*) return 0 ;;
  esac
  return 1
}

any_live_player() {
  local pid=''
  for pid in ${(f)"$(pgrep -f '[c]iv6_play\.py' 2>/dev/null)"}; do
    is_live_python_player "$pid" && return 0
  done
  return 1
}

legacy_climb_below() {
  local supervisor=$1 pid='' ppid='' command=''
  for pid in ${(f)"$(pgrep -f '[c]iv6_civvis_climb\.py' 2>/dev/null)"}; do
    kill -0 "$pid" 2>/dev/null || continue
    ppid=$(ps -p "$pid" -o ppid= 2>/dev/null | tr -d '[:space:]')
    [[ "$ppid" == "$supervisor" ]] || continue
    command=$(ps -p "$pid" -o command= 2>/dev/null) || continue
    [[ "$command" == *"civ6_civvis_climb.py"* && \
       "$command" == *"--restart-below-leader-ratio 0.40"* ]] || continue
    print -r -- "$pid"
    return 0
  done
  return 1
}

natural_result() {
  local summary=$1 kind='' turn='' max_turns=''
  kind=$(jq -r '.outcome.kind // ""' "$summary" 2>/dev/null || true)
  [[ "$kind" == victory || "$kind" == defeat ]] && return 0
  turn=$(jq -r '.last_turn // 0' "$summary" 2>/dev/null || true)
  max_turns=$(jq -r '.max_turns // 0' "$summary" 2>/dev/null || true)
  [[ "$turn" == <-> && "$max_turns" == <-> && "$max_turns" -gt 0 && "$turn" -ge "$max_turns" ]]
}

cleanup_tag_scoped_capture_guard() {
  local guard="$ROOT/bin/screencapture"
  if [[ -f "$guard" ]] && grep -q 'Temporary, tag-scoped CIVVIS guard' "$guard"; then
    rm -f -- "$guard"
    say 'removed the completed recovery capture guard'
  fi
}

retire_legacy_host() {
  local supervisor='' host='' climb='' waited=0
  supervisor=$(pid_from_lock "$SUPERVISOR_LOCK" 'civvis-game-supervisor.sh') || {
    say 'no verified legacy supervisor; opening the current wrapper directly'
    cleanup_tag_scoped_capture_guard
    open -g -j -a Terminal "$WRAPPER"
    return 0
  }
  host=$(pid_from_lock "$HOST_LOCK" 'civvis-interactive-host.sh') || {
    say "supervisor $supervisor has no verified host; refusing replacement"
    return 1
  }
  if any_live_player || pgrep -x Civ6_Exe_Child >/dev/null 2>&1; then
    say 'a Civ VI player/core is still live; waiting for the no-game boundary'
    return 1
  fi

  # The former score-cutoff climb was deliberately STOPped. Freeze its parent
  # first, then deliver TERM+CONT to that exact old child so it cannot schedule
  # another 0.40-cutoff game while the host tears down.  The child has no Civ
  # VI player at this verified boundary; SIGTERM uses Python's default action.
  kill -STOP "$supervisor" 2>/dev/null || return 1
  sleep 1
  if any_live_player || pgrep -x Civ6_Exe_Child >/dev/null 2>&1; then
    kill -CONT "$supervisor" 2>/dev/null || true
    say 'a player/core appeared during handoff verification; left host intact'
    return 1
  fi
  climb=$(legacy_climb_below "$supervisor" || true)
  if [[ -n "$climb" ]]; then
    say "retiring frozen legacy climb $climb beneath supervisor $supervisor"
    kill -TERM "$climb" 2>/dev/null || true
    kill -CONT "$climb" 2>/dev/null || true
    while kill -0 "$climb" 2>/dev/null && (( waited < 20 )); do
      sleep 1
      (( waited += 1 ))
    done
    if kill -0 "$climb" 2>/dev/null; then
      say "legacy climb $climb did not exit after verified TERM+CONT; keeping host frozen"
      return 1
    fi
  fi

  # The parent cannot start a replacement while stopped.  The host owns it and
  # its helper processes; its TERM trap is the clean, scoped shutdown path.
  say "verified no-game gap; retiring legacy host $host and supervisor $supervisor"
  kill -TERM "$host" 2>/dev/null || {
    kill -CONT "$supervisor" 2>/dev/null || true
    return 1
  }
  kill -CONT "$supervisor" 2>/dev/null || true
  waited=0
  while kill -0 "$host" 2>/dev/null && (( waited < 45 )); do
    sleep 1
    (( waited += 1 ))
  done
  if kill -0 "$host" 2>/dev/null; then
    say "host $host did not exit cleanly; not opening a competing host"
    return 1
  fi

  cleanup_tag_scoped_capture_guard
  say 'opening fresh Terminal host: origin/main will be fetched and built before its next King game'
  open -g -j -a Terminal "$WRAPPER"
  return 0
}

[[ -d "$RUN" ]] || {
  say "missing recovery run: $RUN"
  exit 64
}
[[ -x "$WRAPPER" ]] || {
  say "missing current wrapper: $WRAPPER"
  exit 64
}

say "watching protected recovery $TAG for a real outcome"
while [[ ! -f "$RUN/summary.json" ]]; do
  sleep 10
done

reason=$(jq -r '.reason // ""' "$RUN/summary.json" 2>/dev/null || true)
if ! natural_result "$RUN/summary.json"; then
  say "recovery wrote non-natural terminal reason '${reason:-unknown}'; retaining the frozen host for explicit recovery"
  exit 1
fi
say "recovery reached a real outcome (reason='${reason:-unknown}'); waiting for controller teardown"
while any_live_player || pgrep -x Civ6_Exe_Child >/dev/null 2>&1; do
  sleep 5
done
while ! retire_legacy_host; do
  sleep 5
done
say 'fresh-head full-outcome handoff complete'
