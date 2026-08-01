#!/bin/sh
# Keep the foreground Civilization VI run fed while the overnight climb runs.
#
# The machine's standing policy is the resource guardian's: correct by YIELDING,
# never by killing. This does the same thing on a shorter cadence and only for the
# duration of the climb, because the guardian waits ten minutes on its width rule
# and a Civ 6 turn is frame-tied -- a sweep at sixteen cores takes the game from
# ~9s a turn to ~45s while it ramps.
#
# It only ever RAISES nice on batch sweeps it can name. It never signals anything,
# never touches claude sessions, the spectator, or the guardian itself.
while pgrep -f civ6_civvis_climb.py >/dev/null 2>&1; do
    for p in $(pgrep -f 'ai_eval|civvis league|battle_bench' 2>/dev/null); do
        renice 15 -p "$p" >/dev/null 2>&1
    done
    sleep 60
done
