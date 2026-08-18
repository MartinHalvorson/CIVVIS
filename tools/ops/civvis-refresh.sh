#!/bin/zsh
# Reload the exhibition tab when it has stopped following the live server.
#
# civvis-keeper.sh checks that a tab pointing at the exhibition EXISTS. That is
# not the same as the tab SHOWING the game, and the difference is what "civvis
# isn't running" actually looked like on 2026-07-25: the server was happily on
# turn 199 while the tab sat frozen on a page that had loaded a minute before
# that server process existed.
#
# The supervisor hot-swaps the game server mid-match (build promoted, checkpoint
# resumed). The page is supposed to notice and location.replace itself onto the
# successor, but its reconnect poll only arms after it observes a game FINISH --
# so a page whose fetches were in flight during a mid-game swap can be orphaned
# with no path back. The server's /state reports the live server_instance, and a
# connected page carries that instance in its own URL, so the two disagreeing is
# a precise, cheap staleness signal.
#
# Reloads at most once per server instance: after the tab is pointed at
# instance=N it matches until the supervisor swaps in a new process.
PORT=${1:-8766}
KEEPLOG=$HOME/civvis-keeper.log
# Deliberately outside any session scratchpad -- those are deleted when a Claude
# session ends, and this file has to outlive them.
STAMP=$HOME/.civvis-stale-instance
DARK=$HOME/.civvis-dark-since
PULLED=$HOME/.civvis-last-pull

read -r inst seed pace <<< "$(curl -s --max-time 6 "http://127.0.0.1:$PORT/state" 2>/dev/null \
  | python3 -c 'import json,sys
try:
    d = json.load(sys.stdin)
    print(d.get("server_instance", ""), d.get("seed", ""), d.get("pace", ""))
except Exception:
    pass' 2>/dev/null)"

# No instance means the server is mid-restart; the keeper handles that path.
[[ -z "$inst" ]] && exit 0

# Every game is a new server process starting at the default second per turn.
# The page is supposed to hand its remembered watch pace to whichever server it
# finds itself talking to -- but it cannot do that while it is not rendering,
# and this tab spends much of its life on a macOS Space that is not the active
# one, where Chrome suspends the whole render loop. So the exhibition kept
# coming up at 1000ms and crawling. Assert the pace server-side, once per
# instance, and leave it alone afterwards so a person who deliberately slows it
# down to watch a turn is not fought every fifteen seconds.
PACESTAMP=$HOME/.civvis-pace-instance
if [[ -n "$pace" && "$pace" != "0" && "$(cat $PACESTAMP 2>/dev/null)" != "$inst" ]]; then
  echo "$inst" > $PACESTAMP
  curl -s --max-time 4 -X POST "http://127.0.0.1:$PORT/pace" -d '{"ms":0,"paused":false}' >/dev/null 2>&1
  echo "[keeper] $(date -u +%FT%TZ) instance $inst came up at ${pace}ms; pushed Lightning" >> $KEEPLOG
fi

tabs=$(osascript -e 'tell application "Google Chrome" to get URL of tabs of every window' 2>/dev/null | tr ',' '\n')
[[ -z "$tabs" ]] && exit 0

if [[ $(echo "$tabs" | grep -c "instance=$inst") != "0" ]]; then
  rm -f $STAMP
  # The tab follows the live server. That is still not the same as the tab
  # DRAWING it: the viewer's render loop is a requestAnimationFrame chain, and
  # Chrome suspends rAF completely in a tab that is not the ACTIVE tab of its
  # window -- so a correctly-pointed background tab paints exactly zero frames
  # while every check above stays green. Other agents on this box open their
  # own 127.0.0.1:87xx tabs and steal the foreground constantly; civvis-tabs.sh
  # prunes those tabs but never gives the focus back.
  #
  # /status says so directly: `viewers` counts pages that asked inside the
  # 6s VIEWER_ACTIVE window, `frames_painted` is the last turn one of them drew.
  # (`frames_missed` is NOT the signal -- misses are only counted against a
  # viewer that never left, so an unwatched exhibition holds it at zero.)
  painting=$(curl -s --max-time 4 "http://127.0.0.1:$PORT/status" 2>/dev/null \
    | python3 -c 'import json,sys
try:
    d = json.load(sys.stdin)
    print("yes" if d.get("viewers") or d.get("frames_painted") is not None else "no")
except Exception:
    print("unknown")' 2>/dev/null)
  [[ "$painting" != "no" ]] && { rm -f $DARK; exit 0; }

  now=$(date +%s)
  [[ -f $DARK ]] || { echo "$now" > $DARK; exit 0; }
  dark=$(( now - $(cat $DARK 2>/dev/null || echo "$now") ))
  since_pull=$(( now - $(cat $PULLED 2>/dev/null || echo 0) ))
  # Thirty minutes, not five. Measured 2026-07-29: when the window sits on a
  # macOS Space that is not displayed, setting the active tab reports "ok" and
  # painting still never resumes -- nothing inside Chrome can fix an undisplayed
  # Space. So this retry is usually futile, and its only real value is that the
  # exhibition is the ACTIVE tab whenever the operator does come back to that
  # Space, which one attempt every half hour achieves as well as six.
  #
  # A dark tab is also not purely a loss: the server waits for every attached
  # viewer to paint before stepping the next turn, so an unwatched exhibition
  # runs several times faster. Do not chase this harder than this.
  (( dark < 1800 || since_pull < 1800 )) && exit 0
  echo "$now" > $PULLED
  rm -f $DARK
  out=$(osascript -e "tell application \"Google Chrome\"
    repeat with wi from 1 to (count of windows)
      repeat with ti from 1 to (count of tabs of window wi)
        if (URL of tab ti of window wi) contains \":$PORT\" then
          set active tab index of window wi to ti
          return \"ok\"
        end if
      end repeat
    end repeat
    return \"no tab\"
  end tell" 2>&1)
  echo "[keeper] $(date -u +%FT%TZ) exhibition tab drew nothing for $((dark / 60))m (backgrounded); pulled forward -> $out" >> $KEEPLOG
  exit 0
fi

# Two consecutive misses before acting. A tab the keeper just opened sits on the
# bare URL for a moment while the page connects and stamps itself; reloading it
# inside that window would fight the page instead of fixing it.
if [[ ! -f $STAMP || "$(cat $STAMP 2>/dev/null)" != "$inst" ]]; then
  echo "$inst" > $STAMP
  exit 0
fi
rm -f $STAMP

url="http://127.0.0.1:$PORT/?instance=$inst&game=$seed"
out=$(osascript -e "tell application \"Google Chrome\"
  repeat with w in windows
    repeat with t in tabs of w
      if (URL of t contains \":$PORT\") then
        set URL of t to \"$url\"
        return \"ok\"
      end if
    end repeat
  end repeat
  return \"no tab\"
end tell" 2>&1)

echo "[keeper] $(date -u +%FT%TZ) exhibition tab stale (live instance $inst); reloaded -> $out" >> $KEEPLOG
