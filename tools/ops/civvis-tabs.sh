#!/bin/zsh
# Keep exactly one CIVVIS tab, in exactly one window: the live exhibition.
#
# Every restarted game and every other agent's test server leaves a tab behind,
# and each one renders a full map canvas -- so stale tabs cost real CPU/GPU that
# should go to the game actually being watched. CIVVIS servers live on
# 127.0.0.1:87xx; only that range is touched, so ordinary browsing and
# non-CIVVIS localhost pages are left alone.
#
# The survivor is chosen globally, not per window: keeping one per window would
# spread games across windows, which is exactly what we do not want. The tab in
# the lowest-numbered window wins, so the exhibition stays put in one window
# instead of hopping as games restart.
PORT=${1:-8766}
KEEPLOG=$HOME/civvis-keeper.log

closed=$(osascript <<OSA 2>/dev/null
set thePort to "$PORT"
set closedCount to 0
set keptExhibition to false
tell application "Google Chrome"
  -- Forward over windows so the earliest window keeps the tab; backward over
  -- tabs because closing one renumbers those after it.
  repeat with wi from 1 to (count windows)
    set w to window wi
    repeat with i from (count of tabs of w) to 1 by -1
      set u to URL of tab i of w
      if (u contains "127.0.0.1:87") or (u contains "localhost:87") then
        if (u contains (":" & thePort)) and (keptExhibition is false) then
          set keptExhibition to true
        else
          close tab i of w
          set closedCount to closedCount + 1
        end if
      end if
    end repeat
  end repeat
end tell
return closedCount
OSA
)

if [[ -n "$closed" && "$closed" != "0" ]]; then
  echo "[keeper] $(date -u +%FT%TZ) closed $closed stale CIVVIS tab(s)" >> $KEEPLOG
fi
echo "${closed:-0}"
