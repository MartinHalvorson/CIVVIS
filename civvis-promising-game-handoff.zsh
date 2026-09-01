#!/bin/zsh
# Preserve one identified promising CIVVIS game through a real outcome.
#
# The active controller was launched before the operator policy changed from
# the turn-150 score cutoff to full-outcome games.  Its argv cannot safely be
# changed in flight.  This helper therefore waits until the late checkpoint,
# freezes only its verified climb parent (not Civ VI), and either lets the game
# finish normally or reloads its last autosave with a zero cutoff.  Afterwards
# it replaces the managed Terminal host so subsequent fresh-head games inherit
# the new policy.
set -euo pipefail
unsetopt BG_NICE

ROOT=/Users/martbot-mbp-m5-max-128
REPO="$ROOT/CIVVIS"
TAG=${1:-civvis-20260828T020050Z}
RUN="$ROOT/civvis-civ6-runs/control/$TAG"
WRAPPER="$ROOT/civvis-verified-head-launcher.zsh"
CONTINUATION="$ROOT/civvis-promising-game-continuation.zsh"
LOG="$ROOT/civvis-climb-logs/${TAG}-promising-handoff.log"
HOLDER="$ROOT/.civvis-civ6-game.lock/holder.json"
SUPERVISOR_LOCK="$ROOT/.civvis-game-supervisor.lock/pid"
HOST_LOCK="$ROOT/.civvis-interactive-host.lock/pid"
INSTALL_CONFIG="$ROOT/Library/Application Support/Steam/steamapps/common/Sid Meier's Civilization VI/Civ6.app/Contents/Assets/DLC/CivvisControl/config.json"

say() {
  print -r -- "[promising-handoff] $(date -u +%FT%TZ) $*" >> "$LOG"
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

# Print the exact player and its climb parent only when both identities and the
# current installed tag agree.  A matching PID alone must never authorize a
# signal: another manual run may be using the same shared Civ VI install.
owned_player() {
  local pid='' parent='' command='' parent_command='' installed=''
  [[ -r "$HOLDER" ]] || return 1
  pid=$(sed -nE 's/^[[:space:]]*"pid"[[:space:]]*:[[:space:]]*([0-9]+).*/\1/p' "$HOLDER" | head -n 1)
  [[ "$pid" == <-> ]] || return 1
  kill -0 "$pid" 2>/dev/null || return 1
  command=$(ps -p "$pid" -o command= 2>/dev/null) || return 1
  [[ "$command" == *"civ6_play.py"* && "$command" == *"--tag $TAG"* ]] || return 1
  parent=$(ps -p "$pid" -o ppid= 2>/dev/null | tr -d ' ')
  [[ "$parent" == <-> ]] || return 1
  parent_command=$(ps -p "$parent" -o command= 2>/dev/null) || return 1
  [[ "$parent_command" == *"civ6_civvis_climb.py"* ]] || return 1
  installed=$(python3 - "$INSTALL_CONFIG" 2>/dev/null <<'PY'
import json
import sys
from pathlib import Path
path = Path(sys.argv[1])
try:
    print(json.loads(path.read_text()).get("RunTag", ""))
except Exception:
    pass
PY
)
  [[ "$installed" == "$TAG" ]] || return 1
  print -r -- "$pid $parent"
}

# Return tab-separated late-game evidence, or nothing if a readable state has
# not arrived.  This run already established a six-city expansion and was
# explicitly selected for completion.  Late border loyalty or combat pressure
# must not revoke that instruction: neither changes the identity of the owned
# parent that this helper freezes.
late_state() {
  [[ -r "$RUN/events.jsonl" ]] || return 1
  jq -r '
    select(.kind == "state") |
    [
      .turn,
      (.cities | length),
      ([.cities[] | select((.damage // 0) > 0)] | length),
      ([.cities[] | select((.loyalty // 100) < 70)] | length)
    ] | @tsv
  ' "$RUN/events.jsonl" 2>/dev/null | tail -n 1
}

has_live_player() {
  pgrep -f '[c]iv6_play\.py' >/dev/null 2>&1
}

has_live_climb() {
  pgrep -f '[c]iv6_civvis_climb\.py' >/dev/null 2>&1
}

# Swap the old environment-bearing Terminal host only after freezing the
# supervisor and proving that neither a player nor a build/climb is active.
# The host's EXIT trap stops its own supervisor; CONT delivers that TERM when
# the supervisor was intentionally frozen for this handoff.
activate_new_host() {
  local supervisor='' host='' waited=0
  supervisor=$(pid_from_lock "$SUPERVISOR_LOCK" 'civvis-game-supervisor.sh') || {
    say 'no verified supervisor is live; opening the updated wrapper directly'
    open -g -j -a Terminal "$WRAPPER"
    return 0
  }
  host=$(pid_from_lock "$HOST_LOCK" 'civvis-interactive-host.sh') || {
    say "supervisor $supervisor has no verified interactive host; refusing host replacement"
    return 1
  }
  if has_live_player || has_live_climb; then
    return 1
  fi
  kill -STOP "$supervisor" 2>/dev/null || return 1
  sleep 1
  if has_live_player || has_live_climb; then
    kill -CONT "$supervisor" 2>/dev/null || true
    return 1
  fi
  say "verified no-game gap; retiring old host $host and supervisor $supervisor"
  kill -TERM "$host" 2>/dev/null || {
    kill -CONT "$supervisor" 2>/dev/null || true
    return 1
  }
  # The host has sent TERM to its stopped child.  Let it run that trap and
  # release its lock; this does not target Civ VI, which is already absent.
  kill -CONT "$supervisor" 2>/dev/null || true
  while kill -0 "$host" 2>/dev/null && (( waited < 30 )); do
    sleep 1
    (( waited += 1 ))
  done
  if kill -0 "$host" 2>/dev/null; then
    say "host $host did not exit cleanly; leaving it intact"
    return 1
  fi
  say 'opening a fresh managed Terminal host with the full-outcome policy'
  open -g -j -a Terminal "$WRAPPER"
}

resume_after_cutoff() {
  local cont="${TAG}-promising-cont" waited=0
  [[ -x "$CONTINUATION" ]] || {
    say "continuation launcher is not executable: $CONTINUATION"
    return 1
  }
  say "legacy score cutoff fired; asking Terminal to reload the autosave as $cont with no score cutoff"
  open -g -j -a Terminal "$CONTINUATION"
  # A direct controller must originate in Terminal on this host so that the
  # protected Civ VI bundle modification retains its App Management grant.
  # Wait for the continuation's own terminal summary rather than treating an
  # `open` return as proof that the game actually ran.
  while [[ ! -f "$ROOT/civvis-civ6-runs/control/$cont/summary.json" ]]; do
    if (( waited >= 180 )) && ! pgrep -f "[c]iv6_play\\.py.*--tag $cont" >/dev/null 2>&1; then
      say 'Terminal continuation did not start a controller within three minutes'
      return 1
    fi
    sleep 5
    (( waited += 5 ))
  done
  say 'Terminal continuation wrote its terminal summary'
  return 0
}

[[ -d "$RUN" ]] || {
  say "run directory is absent: $RUN"
  exit 64
}
[[ -x "$WRAPPER" ]] || {
  say "updated wrapper is not executable: $WRAPPER"
  exit 64
}

say "watching $TAG; a score-only cutoff will be resumed after the late-game city-safety check"
parent_frozen=0
while true; do
  pair=$(owned_player || true)
  if [[ -n "$pair" ]]; then
    state=$(late_state || true)
    if [[ -n "$state" ]]; then
      IFS=$'\t' read -r turn cities damaged low_loyalty <<< "$state"
      if (( turn >= 145 && cities >= 4 && ! parent_frozen )); then
        player=${pair%% *}
        climb=${pair##* }
        # Re-read immediately before the signal, so a PID reuse or a just-ended
        # player cannot turn an observational decision into an unsafe signal.
        again=$(owned_player || true)
        if [[ "$again" == "$player $climb" ]]; then
          kill -STOP "$climb" 2>/dev/null || {
            say "could not freeze verified climb parent $climb"
            sleep 2
            continue
          }
          parent_frozen=1
          say "turn $turn has $cities cities ($damaged damaged, $low_loyalty low loyalty); froze only climb parent $climb to preserve this game"
        fi
      fi
    fi
    sleep 2
    continue
  fi

  # The active player has gone.  A terminal summary is the authoritative
  # distinction between an ordinary completion and the inherited cutoff.
  if [[ -f "$RUN/summary.json" ]]; then
    reason=$(jq -r '.reason // ""' "$RUN/summary.json" 2>/dev/null || true)
    if [[ "$reason" == 'abandoned' && "$parent_frozen" == 1 ]]; then
      resume_after_cutoff || true
    else
      say "original game ended with reason '${reason:-unknown}'; no reload is needed"
    fi
    while ! activate_new_host; do
      sleep 2
    done
    say 'handoff complete'
    exit 0
  fi

  # No owned player and no summary can only be a startup/teardown race.  Do
  # not signal anything; a later loop will see either the player or the record.
  sleep 2
done
