#!/bin/zsh
# The single managed entry point for this host's continuous CIVVIS verification
# games.  Keep policy here rather than inheriting arbitrary Terminal variables:
# every game must begin from a fresh origin/main build, on King, with the
# deployment genome (all and only default-on genes).
set -euo pipefail
unsetopt BG_NICE

GAME_REPO=/Users/martbot-mbp-m5-max-128/CIVVIS
GAME_PIN=/Users/martbot-mbp-m5-max-128/.civvis-play-pin
GAME_LAUNCHER="$GAME_REPO/tools/ops/civvis-ladder-terminal-launcher.sh"

[[ -x "$GAME_LAUNCHER" ]] || {
  print -u2 "missing tracked CIVVIS launcher: $GAME_LAUNCHER"
  exit 66
}
[[ -r "$GAME_PIN" && "$(<"$GAME_PIN")" == head ]] || {
  print -u2 "refusing verification launch: $GAME_PIN must contain exactly 'head'"
  exit 64
}
[[ "$(git -C "$GAME_REPO" remote get-url origin)" == \
   "https://github.com/MartinHalvorson/CIVVIS.git" ]] || {
  print -u2 "refusing verification launch: CIVVIS origin is not the required GitHub remote"
  exit 64
}

# Never inherit an old labelled experiment, retired strategy, alternate host,
# or former restart policy from a long-lived Terminal window.
unset CIVVIS_WITH CIVVIS_WITHOUT CIVVIS_WITH_FILE CIVVIS_STRATEGY CIVVIS_VICTORY
unset CIVVIS_DIFFICULTY CIVVIS_PLAY_ATTEMPTS CIVVIS_RESTART_BELOW_LEADER_RATIO
unset CIVVIS_HEAD_REPO CIVVIS_PINFILE CIVVIS_LADDER_HOST CIVVIS_LADDER_SUPERVISOR
unset CIVVIS_SUPERVISOR CIVVIS_INTERACTIVE_HOST_LOG CIVVIS_INTERACTIVE_HOST_LOCK

export CIVVIS_HEAD_REPO="$GAME_REPO"
export CIVVIS_PINFILE="$GAME_PIN"
export CIVVIS_DIFFICULTY=DIFFICULTY_KING
# A source refresh/build belongs to every new game, not merely every batch.
export CIVVIS_PLAY_ATTEMPTS=1
# Finish games to a real outcome.  A score snapshot at turn 150 cannot tell a
# six-city expansion that is about to mature from a genuinely lost position;
# using zero disables that single early-abandon rule without changing the
# fresh-head, King, or deployment-genome requirements above.
export CIVVIS_RESTART_BELOW_LEADER_RATIO=0

exec /bin/zsh "$GAME_LAUNCHER"
