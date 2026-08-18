#!/bin/zsh
# Keep CIVVIS's live mirror follower available for an unattended Firaxis run.
#
# The game supervisor starts tools/follow.py at batch boundaries, but the
# follower is deliberately detached and can otherwise die during a long game
# without anything noticing until the next batch.  This keeper runs from the
# same interactive Terminal context as the game supervisor, adopts an existing
# follower, and only restarts it after a bounded, evidence-based failure.
#
# Do not run this from launchd.  follow.py may need to restore the Chrome mirror
# through AppleScript, and this Mac's launchd context lacks the relevant GUI
# permissions.
set -u

RUNS=${CIVVIS_RUNS:-$HOME/civvis-civ6-runs/control}
PINFILE=${CIVVIS_PINFILE:-$HOME/.civvis-play-pin}
MIRROR_HOME=${CIVVIS_MIRROR_HOME:-$HOME/civvis-civ6-mirror}
FOLLOW_LOG=${CIVVIS_FOLLOW_LOG:-$MIRROR_HOME/follow-nohup.log}
KEEPER_LOG=${CIVVIS_MIRROR_KEEPER_LOG:-$MIRROR_HOME/mirror-keeper.log}
LOCK=${CIVVIS_MIRROR_KEEPER_LOCK:-$HOME/.civvis-mirror-keeper.lock}
PID_FILE=$LOCK/pid

# A follower normally needs only a few seconds to release/reopen the visible
# server.  The grace avoids racing the game supervisor's intentional 4-second
# follower replacement between build revisions.
MISSING_GRACE_S=${CIVVIS_MIRROR_MISSING_GRACE_S:-15}
PORT_GRACE_S=${CIVVIS_MIRROR_PORT_GRACE_S:-60}
FOLLOW_STALE_S=${CIVVIS_MIRROR_FOLLOW_STALE_S:-180}
RUN_FRESH_S=${CIVVIS_MIRROR_RUN_FRESH_S:-120}

follower_pid=""
missing_since=0
port_missing_since=0

mkdir -p "$MIRROR_HOME"

say() { print -r -- "[mirror-keeper] $(date -u +%FT%TZ) $*" >> "$KEEPER_LOG"; }

release_lock() {
  local holder=""
  [[ -f "$PID_FILE" ]] && holder=$(<"$PID_FILE")
  [[ "$holder" == "$$" ]] && rm -rf -- "$LOCK"
}

acquire_lock() {
  if mkdir "$LOCK" 2>/dev/null; then
    print -r -- "$$" > "$PID_FILE"
    return 0
  fi

  local holder=""
  [[ -f "$PID_FILE" ]] && holder=$(<"$PID_FILE")
  if [[ -n "$holder" ]] && kill -0 "$holder" 2>/dev/null; then
    say "another mirror keeper is already alive (pid $holder); exiting"
    return 1
  fi

  # This exact lock belongs only to this helper.  A previous shell may have
  # been interrupted before its EXIT trap, so reclaim a proven-dead holder.
  say "reclaiming stale mirror keeper lock (holder=${holder:-unknown})"
  rm -rf -- "$LOCK"
  mkdir "$LOCK" 2>/dev/null || return 2
  print -r -- "$$" > "$PID_FILE"
}

expected_repo() {
  local pin="head"
  [[ -f "$PINFILE" ]] && pin=$(<"$PINFILE")
  if [[ -z "$pin" || "$pin" == "head" ]]; then
    print -r -- $HOME/CIVVIS
  else
    print -r -- "$pin"
  fi
}

find_follower() {
  # [] keeps pgrep from matching its own command line.  The follower is the
  # only process that ends in tools/follow.py; the game and mirror server have
  # distinct commands.
  pgrep -f '[t]ools/follow.py' 2>/dev/null | head -n 1 || true
}

newest_events() {
  # `N` makes the no-run-yet state an empty list instead of zsh's fatal
  # NOMATCH error.  That state is ordinary between cleanup and the next
  # bootstrap, and a keeper must wait through it rather than exit.
  local -a candidates
  candidates=("$RUNS"/*/events.jsonl(N))
  (( ${#candidates} > 0 )) || return 0
  ls -t "${candidates[@]}" 2>/dev/null | head -n 1 || true
}

live_tiles_exported() {
  local events now age
  events=$(newest_events)
  [[ -n "$events" && -f "$events" ]] || return 1
  now=$(date +%s)
  age=$(( now - $(stat -f %m "$events") ))
  (( age <= RUN_FRESH_S )) || return 1
  # The mirror cannot legitimately serve until a tile export exists.  Do not
  # call a missing port an error while a fresh game is generating its map.
  /usr/bin/grep -q '"kind": "tiles"' "$events" 2>/dev/null
}

server_alive() {
  /usr/bin/curl -fsS --max-time 4 http://127.0.0.1:8610/status >/dev/null 2>&1
}

follower_log_path() {
  local pid=$1 log_path=""
  # A batch-started follower is intentionally detached with follow.log, while
  # this keeper's own fallback writes follow-nohup.log.  Inspect its open stderr
  # descriptor rather than assuming either filename; otherwise a healthy new
  # batch is killed after FOLLOW_STALE_S solely because it uses the former.
  if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
    log_path=$(lsof -a -p "$pid" -d 1 -Fn 2>/dev/null | sed -n 's/^n//p' | head -n 1)
  fi
  if [[ "$log_path" == "$MIRROR_HOME"/* && -f "$log_path" ]]; then
    print -r -- "$log_path"
  else
    print -r -- "$FOLLOW_LOG"
  fi
}

follow_log_age() {
  local pid=$1 log_path="" now
  log_path=$(follower_log_path "$pid")
  [[ -f "$log_path" ]] || { print -r -- 999999; return; }
  now=$(date +%s)
  print -r -- $(( now - $(stat -f %m "$log_path") ))
}

start_follower() {
  local repo
  repo=$(expected_repo)
  if [[ ! -d "$repo" || ! -f "$repo/tools/follow.py" || ! -x "$repo/target/release/civvis" ]]; then
    say "cannot start follower: incomplete runtime tree '$repo'"
    return 1
  fi
  (
    cd "$repo" || exit 1
    exec /usr/bin/env PYTHONUNBUFFERED=1 python3 -u tools/follow.py
  ) >>"$FOLLOW_LOG" 2>&1 &
  follower_pid=$!
  missing_since=0
  port_missing_since=0
  say "started follower pid $follower_pid from $repo"
}

restart_follower() {
  local reason=$1 pid=$2
  if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
    say "$reason; requesting clean follower stop (pid $pid)"
    kill -TERM "$pid" 2>/dev/null || true
  fi
  follower_pid=""
  missing_since=$(date +%s)
  port_missing_since=0
}

acquire_lock
case $? in
  0) ;;
  1) exit 0 ;;
  *) exit 70 ;;
esac
trap release_lock EXIT
trap 'exit 0' HUP INT TERM

say "keeper up (pid $$)"

while true; do
  now=$(date +%s)
  observed=$(find_follower)
  if [[ -z "$observed" ]]; then
    if (( missing_since == 0 )); then
      missing_since=$now
      say "follower absent; allowing ${MISSING_GRACE_S}s for a planned replacement"
    elif (( now - missing_since >= MISSING_GRACE_S )); then
      start_follower || missing_since=$now
    fi
    sleep 5
    continue
  fi

  follower_pid=$observed
  missing_since=0

  if live_tiles_exported; then
    if server_alive; then
      port_missing_since=0
    elif (( port_missing_since == 0 )); then
      port_missing_since=$now
      say "fresh tile stream but :8610 is absent; allowing ${PORT_GRACE_S}s for follower recovery"
    elif (( now - port_missing_since >= PORT_GRACE_S )); then
      restart_follower ":8610 remained absent for $(( now - port_missing_since ))s after a tile export" "$follower_pid"
      sleep 5
      continue
    fi

    log_age=$(follow_log_age "$follower_pid")
    if (( log_age >= FOLLOW_STALE_S )); then
      restart_follower "follower log was silent ${log_age}s while Civ VI events were fresh" "$follower_pid"
      sleep 5
      continue
    fi
  else
    port_missing_since=0
  fi

  sleep 5
done
