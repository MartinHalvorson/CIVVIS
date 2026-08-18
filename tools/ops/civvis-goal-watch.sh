#!/bin/zsh
# 24h exhibition watch. Emits one line per event on stdout; silence means healthy.
# Ends when the Chrome tab showing the exhibition is gone, or at the deadline.
#
# Lives outside the session scratchpad on purpose: zsh reads a script
# incrementally, so a file that vanishes mid-run breaks a running watchdog.
#
# The signal that matters is PAINTING, not tab presence. The viewer's render
# loop is a requestAnimationFrame chain, and Chrome suspends rAF entirely in a
# tab that is not the active one -- so a tab sitting at the right URL behind
# another tab shows a frozen map while every other check reads green. The
# server reports this directly: /status `viewers` counts pages that asked
# within the last 6s and `frames_painted` is the last turn one of them drew.
# Other agents on this box open their own 127.0.0.1:87xx tabs, which steal the
# foreground, so this has to be repaired rather than merely reported.

PORT=8766
DEADLINE=${1:?deadline epoch required}
SUMMARY_EVERY=7200
DARK_GRACE=3600     # seconds of no painting before the tab is brought to the front
REACTIVATE_EVERY=3600

misses=0
last_turn=-1
last_inst=""
last_move=$(date +%s)
last_summary=$(date +%s)
last_paint=$(date +%s)
last_pull=0
games=0
pulls=0
down=0
slow=0

status_json() { curl -s --max-time 20 "http://127.0.0.1:$PORT/status" 2>/dev/null }

# /state is ~3 MB and puts `pace` and `server_instance` well past any prefix
# cut, so read the whole body and stream it through grep -- one pass, ~0.05s,
# and it cannot silently return empty the way `head -c 400` did.
state_fields() {
  curl -s --max-time 15 "http://127.0.0.1:$PORT/state" 2>/dev/null \
    | grep -oE '"(pace|server_instance)":[0-9]+' | sort -u
}

tab_count() {
  osascript -e 'tell application "Google Chrome"
    set n to 0
    repeat with w in windows
      repeat with t in tabs of w
        if (URL of t) contains "127.0.0.1:8766" then set n to n + 1
      end repeat
    end repeat
    return n
  end tell' 2>/dev/null
}

pull_forward() {
  osascript -e 'tell application "Google Chrome"
    repeat with wi from 1 to (count of windows)
      repeat with ti from 1 to (count of tabs of window wi)
        if (URL of tab ti of window wi) contains "127.0.0.1:8766" then
          set active tab index of window wi to ti
          return "ok"
        end if
      end repeat
    end repeat
    return "missing"
  end tell' 2>/dev/null
}

while true; do
  now=$(date +%s)
  if [ "$now" -ge "$DEADLINE" ]; then
    echo "WATCH-COMPLETE 24h elapsed; $games games seen, $pulls tab pull-forwards"
    exit 0
  fi

  # --- the stop condition: the tab the operator is watching ---------------
  n=$(tab_count)
  if [ -z "$n" ] || [ "$n" = "0" ]; then
    misses=$((misses + 1))
    # A game boundary does a location.replace, so one miss is normal. Three
    # consecutive misses ~3 minutes apart is a closed tab.
    if [ "$misses" -ge 3 ]; then
      echo "TAB-CLOSED no Chrome tab on 127.0.0.1:$PORT for 3 consecutive checks; ending the watch"
      exit 0
    fi
  else
    misses=0
  fi

  # --- the game, and whether anyone is actually seeing it -----------------
  st=$(status_json)
  if [ -z "$st" ]; then
    # Every game boundary and every mid-match build hot-swap replaces the
    # server process, so a single unanswered /status is the normal shape of a
    # healthy exhibition, not an outage. Only a miss that survives the next
    # check is worth waking anyone for.
    # Three, not two, and a 20s timeout. This box routinely sits at load 50
    # (other agents run ~22 concurrent rustc builds), and /status was measured
    # taking 10s+ under that while the server was perfectly healthy. Two misses
    # at an 8s timeout was reporting load, not death.
    down=$((down + 1))
    [ "$down" -ge 3 ] && { echo "SERVER-DOWN /status has not answered on port $PORT for $down consecutive checks"; down=0; }
  else
    down=0
    turn=$(echo "$st" | sed -n 's/.*"turn":\([0-9]*\).*/\1/p')
    viewers=$(echo "$st" | sed -n 's/.*"viewers":\([0-9]*\).*/\1/p')
    painted=$(echo "$st" | sed -n 's/.*"frames_painted":\([0-9]*\).*/\1/p')
    missed=$(echo "$st" | sed -n 's/.*"frames_missed":\([0-9]*\).*/\1/p')
    fields=$(state_fields)
    inst=$(echo "$fields" | sed -n 's/^"server_instance":\([0-9]*\)$/\1/p' | head -1)
    pace_now=$(echo "$fields" | sed -n 's/^"pace":\([0-9]*\)$/\1/p' | head -1)

    [ -n "$painted" ] && last_paint=$now
    [ -n "$viewers" ] && [ "$viewers" != "0" ] && last_paint=$now

    if [ -n "$inst" ] && [ "$inst" != "$last_inst" ]; then
      [ -n "$last_inst" ] && games=$((games + 1))
      last_inst=$inst
      last_move=$now
      # No pace push here. civvis-refresh.sh asserts Lightning once per
      # instance from the keeper's 15s loop -- faster than this 60s one and,
      # unlike this, it survives the session ending. Confirmed handling every
      # instance this watch reported. The `slow` counter below stays as the
      # backstop for the case that actually matters: the durable path failing.
    elif [ -n "$turn" ] && [ "$turn" != "$last_turn" ]; then
      last_turn=$turn
      last_move=$now
    elif [ $((now - last_move)) -ge 420 ]; then
      echo "STALLED turn $turn on instance $inst has not moved in $(((now - last_move) / 60))m"
      last_move=$now
    fi

    dark=$((now - last_paint))
    if [ "$dark" -ge "$DARK_GRACE" ] && [ $((now - last_pull)) -ge "$REACTIVATE_EVERY" ]; then
      last_pull=$now
      if [ "$(pull_forward)" = "ok" ]; then
        pulls=$((pulls + 1))
        echo "TAB-DARK nothing painted for $((dark / 60))m -- the tab is backgrounded or its window is on another Space, so Chrome suspends rendering; made it the active tab (the sim itself keeps running either way)"
        last_paint=$now
      fi
    fi

    if [ -n "$missed" ] && [ "$missed" != "0" ]; then
      echo "FRAMES-MISSED $missed turns reached no viewer on instance $inst"
    fi

    # The page is supposed to hand its remembered Lightning to whatever server
    # it finds itself talking to, and usually does -- but a tab the keeper has
    # just re-created can come up before that assertion lands, leaving the
    # exhibition at the server's default second per turn. Three checks of a
    # non-zero pace is a stuck one, not a viewer mid-handshake. The goal for
    # this run is Lightning, so this enforces it and says so each time.
    if [ -n "$pace_now" ] && [ "$pace_now" != "0" ]; then
      slow=$((slow + 1))
      if [ "$slow" -ge 3 ]; then
        curl -s --max-time 8 -X POST "http://127.0.0.1:$PORT/pace" \
             -d '{"ms":0,"paused":false}' >/dev/null 2>&1
        echo "PACE-REPAIRED exhibition had been sitting at ${pace_now}ms; pushed Lightning"
        slow=0
      fi
    else
      slow=0
    fi
  fi

  if [ $((now - last_summary)) -ge $SUMMARY_EVERY ]; then
    last_summary=$now
    rated=$(/usr/bin/python3 -c "import json;print(json.load(open(''"$HOME"'/civvis-spectator-src/league/league.json'))['round'])" 2>/dev/null)
    echo "SUMMARY turn $turn/$last_inst | viewers $viewers | $games games this watch | league round $rated | $pulls pull-forwards | $(( (DEADLINE - now) / 3600 ))h left"
  fi

  sleep 60
done
