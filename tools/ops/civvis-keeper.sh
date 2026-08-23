#!/bin/zsh
# Keeps the CIVVIS exhibition alive AND on screen.
#
# The supervisor handles game cycling, rebuilds and checkpoint recovery. This
# covers what it cannot:
#   1. the supervisor process dying;
#   2. the exhibition vanishing from the screen. Other agents share this Chrome.
#      Two distinct failures have happened: the tab was navigated away, and
#      Chrome was left running with ZERO WINDOWS -- in which case `open -a` does
#      nothing at all and a "make new tab" script errors with "Can't get
#      window 1". Both cases are handled below, and osascript errors are logged
#      rather than discarded, because swallowing them hid a reopen loop that had
#      been failing every 25s.
export PATH="$HOME/.cargo/bin:$PATH"
# zsh sets BG_NICE by default: every `&` job is started at nice +5. The
# supervisor is launched in the background, so the exhibition -- the one CIVVIS
# workload with a human watching -- silently ran at nice 5, tied with a
# six-worker background fleet league burning 490% CPU. Nothing in the Python
# does this and no launchd key fixes it; it is the shell. Turn it off.
unsetopt BG_NICE
# ⚠⚠ THE TRACKED HELPER, NOT THE ONE IN $HOME. The three helpers this loop calls
# below were invoked as `$HOME/<name>.sh`, which is the hand-edited home copy —
# "a home copy is a dead ladder", the failure this fleet keeps rediscovering
# (civvis-the-supervisor-must-run-from-its-tracked-path). It was not theoretical:
# on 2026-08-18 `tools/ops/` had `/Users/martin` mechanically replaced by `$HOME`
# so the scripts would run on a host that is not their author's
# (tools/test_ops_portability.py, whose LEGACY_DEBT table is empty because of
# it). Not one home copy was re-synced, so for five days every one of those
# fixes sat in a file this loop did not call, and `civvis-sync.sh` reported
# SCRIPT DRIFT on eleven of them every fifteen minutes.
#
# The tracked helper is the sibling of this script, so it needs no repository
# root to be guessed: `${0:A:h}` is `tools/ops/` in whatever worktree this copy
# came from. $HOME stays as the fallback, because a host that installed only the
# home copies must keep working.
OPS=${0:A:h}
ops() {
  local name=$1
  shift
  if [[ -x $OPS/$name ]]; then
    $OPS/$name "$@"
  elif [[ -x $HOME/$name ]]; then
    $HOME/$name "$@"
  fi
}
REPO=$HOME/CIVVIS
# Start the supervisor from the CANONICAL source worktree, never from $REPO.
# $REPO is a shared checkout that other agents edit live: on 2026-07-25 its
# tools/spectator_supervisor.py was 293 lines lighter than origin/main, a
# mid-refactor nobody had finished. The supervisor re-execs itself out of the
# canonical worktree anyway, so launching there just skips running a stranger's
# WIP for the first few seconds. CIVVIS_DEPLOY_ROOT keeps the deploy root
# ($REPO/target/spectator: binary, checkpoints, results) exactly where it was.
SRC=$HOME/civvis-spectator-src
LOG=$REPO/spectator.log
KEEPLOG=$HOME/civvis-keeper.log
PORT=8766
URL="http://127.0.0.1:$PORT/"

log() { echo "[keeper] $(date -u +%FT%TZ) $1" >> $KEEPLOG }

while true; do
  # Count only top-level supervisors; the one forked for --prepare-once
  # prebuilds shares the script name and must not be mistaken for a duplicate.
  alive=0
  for p in $(pgrep -f "spectator_supervisor.py" 2>/dev/null); do
    [[ "$(ps -o ppid= -p $p 2>/dev/null | tr -d ' ')" == "1" ]] && alive=$((alive+1))
  done
  # A dying supervisor leaves its civvis server behind, still holding the port.
  # The replacement then spawns servers that panic "cannot bind port ... Address
  # already in use" forever, because nothing owns the orphan. Adopt-or-clear it
  # before starting a supervisor: if no supervisor is alive but something is
  # still listening, that listener is unmanaged and must go.
  if [[ $alive -eq 0 ]]; then
    holder=$(lsof -nP -iTCP:$PORT -sTCP:LISTEN 2>/dev/null | awk 'NR>1{print $2}' | sort -u | head -1)
    if [[ -n "$holder" ]]; then
      log "orphaned server $holder holds port $PORT with no supervisor; clearing it"
      kill "$holder" 2>/dev/null
      sleep 5
    fi
  fi

  if [[ $alive -eq 0 ]]; then
    log "supervisor not running; starting it"
    # --source-check-interval 10: look for new upstream code every 10s during
    # play (default is 30) so a commit is compiled and promoted well before the
    # current game ends.
    #
    # No --cooldown. A finished result is held for the ten seconds the result
    # screen counts down, and that is fixed in the code on purpose: while this
    # script could set it, the number on screen was whatever this line said,
    # and it said 5 while the game promised 10. The flag is still accepted and
    # ignored, so passing it here would only earn a log line saying so.
    #
    # No `nice` here, deliberately. The exhibition is the one CIVVIS workload a
    # human actually watches, and it needs about one core; the fleet league runs
    # six jobs at nice 5. A previous session hand-started this supervisor under
    # `nice 10`, which left the visible game the LOWEST-priority civvis process
    # on the box and starved its render behind background evolution.
    #
    # --busy-timeout 600. The default is 0, which the help spells out as "0
    # never kills active compute": a server that binds the port and then spins
    # at 100% CPU without ever serving a request is waited on FOREVER, because
    # the supervisor only extends its recovery window while it sees CPU
    # activity. Observed 2026-07-29: an 8-player `islands` game burned 12:25 of
    # CPU in 12:32 elapsed, served nothing, and would have held the exhibition
    # for the rest of the run. Ten minutes is far longer than any legitimate
    # turn (a whole game averages ~1.5 min here) while still ending a hang.
    #
    # The map/victory flags are not decoration: the exhibition that was running
    # before 2026-07-25 used a 74x46 map with 9 city-states, and a keeper restart
    # that omitted them silently downgraded the show to the 60x38 defaults.
    # --no-open matters most -- without it the supervisor opens a browser tab per
    # game, which is precisely the tab churn the pruning below exists to undo.
    (cd $SRC && CIVVIS_DEPLOY_ROOT=$REPO nohup python3 tools/spectator_supervisor.py \
        --players 6 --width 74 --height 46 --city-states 9 --turns 250 \
        --map pangaea --speed online \
        --victories science,culture,religious,diplomatic,domination,score \
        --no-open --source-check-interval 10 --busy-timeout 600 >> $LOG 2>&1 &)
    sleep 25
    continue
  fi

  if curl -s --max-time 5 -o /dev/null "http://127.0.0.1:$PORT/status" 2>/dev/null; then
    if pgrep -x "Google Chrome" > /dev/null 2>&1; then
      # An empty AppleScript result is NOT an empty browser. The enumeration
      # fails intermittently under load (and aborts outright if a window closes
      # mid-loop), 2>/dev/null hides the error, and `grep -c` on nothing is 0 --
      # which reads exactly like "the tab is gone". On 2026-07-28 that had the
      # keeper "restoring" the exhibition every ~2 minutes, each restore adding
      # a window, so the page reloaded before it ever settled and painted zero
      # frames for an hour. Absent means Chrome answered and had no such tab.
      count_shown() {
        local raw
        raw=$(osascript -e 'tell application "Google Chrome" to get URL of tabs of every window' 2>/dev/null)
        if [[ -z "$raw" ]]; then
          echo "unknown"
        else
          echo "$raw" | tr ',' '\n' | grep -c "$PORT"
        fi
      }
      shown=$(count_shown)
      # Confirm before restoring. At every game boundary the page does a
      # location.replace to stamp the new instance/seed into its URL, and for a
      # moment the tab matches nothing -- a single miss is normal, not a lost
      # exhibition. Restoring on that miss spawned a redundant tab (62 of them
      # in one log) which then had to be pruned again: pure churn on the one
      # canvas we want rendering steadily. Two consecutive misses ~3s apart
      # mean the tab is really gone.
      # Three misses over ten seconds, not two over three. A reload of this page
      # is ~3 MB plus /rules plus a full boot, and a tab that is mid-navigation
      # can report a URL that matches nothing; at 2/3s that read as a lost
      # exhibition and the keeper opened a fresh WINDOW for it every couple of
      # minutes on 2026-07-28, so the page never survived long enough to paint.
      # A genuinely closed tab is still restored inside half a minute.
      if [[ "$shown" == "0" ]]; then
        sleep 5
        shown=$(count_shown)
      fi
      if [[ "$shown" == "0" ]]; then
        sleep 5
        shown=$(count_shown)
      fi
      if [[ "$shown" == "0" ]]; then
        wins=$(osascript -e 'tell application "Google Chrome" to count windows' 2>/dev/null)
        [[ -z "$wins" ]] && wins=0
        log "exhibition not on screen (windows=$wins); restoring"
        # A window must exist before a tab can be added to it.
        out=$(osascript -e "tell application \"Google Chrome\"
          if (count windows) is 0 then
            make new window
            set URL of active tab of window 1 to \"$URL\"
          else
            tell window 1 to make new tab with properties {URL:\"$URL\"}
          end if
        end tell" 2>&1)
        [[ -n "$out" ]] && log "restore said: $out"
        sleep 12
      fi
    else
      log "Chrome is not running; launching it on the exhibition"
      open -a "Google Chrome" "$URL" >> $KEEPLOG 2>&1
      sleep 15
    fi

    # Prune stale CIVVIS tabs so the machine renders one map, not eight. Every
    # restarted game and every other agent's test server leaves a tab behind,
    # and each one drives a full canvas. Only 127.0.0.1:87xx is touched.
    ops civvis-tabs.sh "$PORT" > /dev/null 2>&1

    # A tab that points at the exhibition is not necessarily a tab that is
    # SHOWING it -- see civvis-refresh.sh. Everything above this line was green
    # on 2026-07-25 while the visible page was frozen a full game behind.
    ops civvis-refresh.sh "$PORT" > /dev/null 2>&1

    # Keep provisional challengers seatable. A new entrant competes with one
    # flat global rating against the third-best per-civ rating of a 15-way
    # maximum, so it is frozen out and never accumulates the games that would
    # rate it -- see civvis-challenger-guard.sh. Self-limiting: it touches only
    # named challengers and stops at 12 games each, after which it is a no-op
    # forever. Throttles itself; safe to call every pass.
    ops civvis-challenger-guard.sh >> $KEEPLOG 2>&1
  fi

  sleep 15
done
