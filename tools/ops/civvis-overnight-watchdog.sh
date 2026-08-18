#!/bin/zsh
# Lightweight, GUI-capable watchdog for the unattended Civ VI/CIVVIS session.
#
# The audit itself is deterministic.  Keeping this loop in Terminal removes an
# unnecessary dependency on a model invocation while preserving the same
# bounded recovery rules and macOS Accessibility context.

set -u

AUDIT=$HOME/civvis-overnight-audit.sh
WATCH_LOG=$HOME/civvis-civ6-runs/overnight_watchdog.log
LOCK=$HOME/.civvis-overnight-watchdog.lock
PID_FILE=$LOCK/pid
STOP_AT=${CIVVIS_OVERNIGHT_STOP_AT:-2026-08-12T05:12:01Z}
INTERVAL_S=${CIVVIS_OVERNIGHT_AUDIT_INTERVAL_S:-300}

say() {
  print -r -- "[overnight-watchdog] $(date -u +%FT%TZ) $*" >> "$WATCH_LOG"
}

run_audit() {
  local result rc
  result=$(/bin/zsh "$AUDIT" 2>&1)
  rc=$?
  say "audit exit=$rc $result"
  print -r -- "$result"
  return $rc
}

if [[ "${1:-}" == "--once" ]]; then
  run_audit
  exit $?
fi

stop_epoch=$(/bin/date -j -u -f "%Y-%m-%dT%H:%M:%SZ" "$STOP_AT" +%s 2>/dev/null || true)
if [[ -z "$stop_epoch" || "$stop_epoch" != <-> ]]; then
  say "invalid stop deadline '$STOP_AT'; refusing to run"
  exit 64
fi

if ! mkdir "$LOCK" 2>/dev/null; then
  holder=""
  [[ -f "$PID_FILE" ]] && holder=$(<"$PID_FILE")
  if [[ -n "$holder" ]] && kill -0 "$holder" 2>/dev/null; then
    say "another watchdog is already alive (pid $holder); exiting"
    exit 0
  fi
  # This exact lock belongs only to this watchdog and its recorded owner is
  # proven dead.  Reclaim it so an interrupted prior session cannot suppress
  # the requested overnight protection.
  rm -rf -- "$LOCK"
  if ! mkdir "$LOCK" 2>/dev/null; then
    say "could not acquire watchdog lock"
    exit 70
  fi
fi
print -r -- "$$" > "$PID_FILE"

release_lock() {
  local holder=""
  [[ -f "$PID_FILE" ]] && holder=$(<"$PID_FILE")
  [[ "$holder" == "$$" ]] && rm -rf -- "$LOCK"
}
trap release_lock EXIT
trap 'exit 0' HUP INT TERM

say "watchdog up (pid $$), every ${INTERVAL_S}s, through $STOP_AT"
while true; do
  now=$(date +%s)
  if (( now >= stop_epoch )); then
    say "requested deadline reached; stopping"
    exit 0
  fi

  run_audit || true

  now=$(date +%s)
  remaining=$(( stop_epoch - now ))
  (( remaining > 0 )) || continue
  sleep_for=$INTERVAL_S
  (( sleep_for > remaining )) && sleep_for=$remaining
  sleep "$sleep_for"
done

