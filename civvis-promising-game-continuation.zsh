#!/bin/zsh
# Terminal-owned continuation for the one protected promising game.
#
# This file is deliberately opened by Terminal rather than launched by a
# background watchdog: this host grants the Terminal-responsible process the
# permission needed to install the Civ VI control DLC.
set -euo pipefail
unsetopt BG_NICE

ROOT=/Users/martbot-mbp-m5-max-128
REPO="$ROOT/CIVVIS"
TAG=${1:-civvis-20260828T020050Z}
CONT="${TAG}-promising-resume142c"
RUN="$ROOT/civvis-civ6-runs/control/$TAG"
CONT_RUN="$ROOT/civvis-civ6-runs/control/$CONT"
AUTOSAVES="$ROOT/Library/Application Support/Sid Meier's Civilization VI/Sid Meier's Civilization VI/Saves/Single/auto"
SINGLE_SAVES="$ROOT/Library/Application Support/Sid Meier's Civilization VI/Sid Meier's Civilization VI/Saves/Single"
LOG="$ROOT/civvis-climb-logs/${CONT}-play.log"
POPUP_GUARD="$ROOT/civvis-promising-popup-guard.py"
popup_guard_pid=''

say() {
  print -r -- "[promising-continuation] $(date -u +%FT%TZ) $*" >> "$LOG"
}

stop_popup_guard() {
  [[ "$popup_guard_pid" == <-> ]] || return 0
  kill -TERM "$popup_guard_pid" 2>/dev/null || true
  wait "$popup_guard_pid" 2>/dev/null || true
  popup_guard_pid=''
}

# Usually the predecessor ended on the inherited score cutoff and produced a
# summary.  This one was instead interrupted while its Civ VI core was alive:
# its verified t143 autosave and event stream are still stronger evidence than
# inventing a terminal reason.  Permit that narrow recovery only after a real
# late-game state; never turn an empty startup directory into a saved-game run.
if [[ -f "$RUN/summary.json" ]]; then
  [[ "$(jq -r '.reason // ""' "$RUN/summary.json")" == 'abandoned' ]] || {
    say 'refusing continuation: original game did not end on the score cutoff'
    exit 64
  }
else
  recovered_turn=$(jq -r 'select(.kind == "state") | .turn' "$RUN/events.jsonl" \
    2>/dev/null | tail -n 1)
  [[ "$recovered_turn" == <-> && "$recovered_turn" -ge 100 ]] || {
    say 'refusing continuation: no terminal summary and no late-game state'
    exit 64
  }
  say "recovering interrupted original from verified turn $recovered_turn"
fi
[[ ! -f "$CONT_RUN/summary.json" ]] || {
  say 'continuation is already complete'
  exit 0
}

save=''
candidate=''
if [[ ! -f "$RUN/summary.json" ]]; then
  # The t143 save contains the modal that wedged the interrupted controller.
  # Load the preserved preceding turn from the ordinary Single-player list;
  # it is a different deterministic simulation boundary, not a new game.
  recovery_source="$RUN/recovery-saves/AutoSave_0142.Civ6Save"
  recovery_manual="$SINGLE_SAVES/civvis-promising-t142.Civ6Save"
  [[ -f "$recovery_source" ]] || {
    say "refusing continuation: missing preserved recovery save $recovery_source"
    exit 66
  }
  cp -p "$recovery_source" "$recovery_manual"
  save="$recovery_manual"
else
  for candidate in "$AUTOSAVES"/AutoSave_*.Civ6Save(N); do
    [[ -n "$save" && "$save" -nt "$candidate" ]] && continue
    save="$candidate"
  done
fi
[[ -n "$save" && -f "$save" ]] || {
  say 'refusing continuation: no readable autosave exists'
  exit 66
}
[[ -x "$REPO/target/release/civvis_orders" ]] || {
  say 'refusing continuation: the original game binary is absent'
  exit 66
}
[[ -r "$POPUP_GUARD" ]] || {
  say "refusing continuation: missing protected popup guard $POPUP_GUARD"
  exit 66
}

say "loading ${save:t} as $CONT with restart floor disabled"
python3 -u "$POPUP_GUARD" --tag "$CONT" --run-dir "$CONT_RUN" --log "$LOG" \
  >> "$LOG" 2>&1 &
popup_guard_pid=$!
trap 'stop_popup_guard; exit 0' HUP INT TERM
set +e
python3 -u "$REPO/tools/civ6_play.py" \
  --tag "$CONT" \
  --orders-db "$CONT_RUN/orders.sqlite" \
  --difficulty DIFFICULTY_KING \
  --map-size MAPSIZE_SMALL \
  --speed GAMESPEED_ONLINE \
  --leader LEADER_TRAJAN \
  --load-save "$save" \
  --max-turns 250 \
  --timeout 10800 \
  --timeout-ceiling 14400 \
  --lock-wait 30 \
  --report-every 10 \
  --export-state \
  --announcement-seconds 0.05 \
  --era-announcement-seconds 0.05 \
  --dialogue-seconds 0.25 \
  --civvis-decides \
  --civvis-bin "$REPO/target/release/civvis_orders" \
  --civvis-victory diplomatic \
  --civvis-strategy "" \
  --civvis-refresh-seconds 0 \
  --restart-below-leader-ratio 0 \
  --stall-seconds 120 \
  --frozen-seconds 180 \
  --settler-escort-cap-sync \
  --tile-export-every 4 \
  --combat-frames 0 \
  --replan-frames 2 \
  --window-side right \
  --window-frac 0.5 \
  --window-vfrac 0.5 \
  >> "$LOG" 2>&1
rc=$?
set -e
stop_popup_guard
trap - HUP INT TERM
say "controller exited $rc; summary present=$([[ -f "$CONT_RUN/summary.json" ]] && print yes || print no)"
exit "$rc"
