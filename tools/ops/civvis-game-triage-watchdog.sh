#!/bin/zsh
# Abandon a live game that cannot be won, and one whose agent has died.
#
# Operator asked (2026-08-19) to focus on winning science victories and to give
# up early where there is no hope. Both conditions end the same way — restart —
# so one watchdog owns both.
#
# ── 1. HOPELESS ────────────────────────────────────────────────────────────
# `events.jsonl` carries per-turn `score` and `rival_best` on kind=turn/ctx=agent
# rows (the mirror's honest comparison; the outcome event's score is our OWN
# seat and reading it as a margin once turned a 469-point gap into a "two-point
# near miss"). Lead = score - rival_best.
#
# Thresholds derived from THIS SESSION'S 18 games, not guessed. Lead by turn:
#
#   final lead   t120   t160   t200        verdict
#     -36         -70    -20    +14        best game — must survive
#    -191         -68   -144   -106        strong  — must survive
#    -250         -62    -54   -195        strong  — must survive
#    -302        -135   -312   -295        recovered to 971 — must survive
#    -560        -143   -165   -442        dead by t200
#    -737        -180   -272   -340        dead by t200
#    -416        -120   -404   -416        dead by t160
#    -846        -280   -427   -687        dead by t120
#    -921        -328   -480   -611        dead by t120
#    -970        -249   -437   -658        dead by t120
#
# Gates: t>=120 lead<=-280; t>=160 lead<=-350; t>=200 lead<=-300, each needing
# two consecutive polls. Replayed over all 18 games: every survivor finished
# better than -260 and every abandoned game finished -302 or worse, so the split
# matches "could still contend" against "no hope of winning". 561 turns of dead
# game would have been skipped.
#
# ⚠ Judge these against WINNING, not against a score floor. 102855Z scored 971 —
# a fine number — while sitting 289 behind at t151 and finishing 302 behind. It
# is abandoned on purpose: a good-looking score that never threatens the leader
# is exactly the game the operator asked not to keep playing.
#
# ── 2. DEAD AGENT ──────────────────────────────────────────────────────────
# max(turn) in orders.sqlite far behind the mirror's turn: the agent stopped
# issuing orders while Civ 6 played on. Seen 2026-08-19 — a DiplomacyActionView
# defeated the autoclose AND popup_clear disqualified itself ("no turn recorded
# yet; this is setup"), so 44 turns were played by nobody.
#
# Restart order in both cases: TERM the climb (owns the gamelock, releases it),
# then SIGINT civ6_play — SIGTERM skips its atexit teardown and strands a
# RunTag that wedges the next attempt.
set -u

RUNS=${CIVVIS_RUNS:-$HOME/civvis-civ6-runs/control}
PORT=${CIVVIS_MIRROR_PORT:-8610}
LOG=${CIVVIS_TRIAGE_LOG:-$HOME/civvis-civ6-runs/game_triage.log}
POLL_S=${CIVVIS_TRIAGE_POLL_S:-60}
WEDGE_GAP=${CIVVIS_WEDGE_GAP:-12}
CONFIRM=${CIVVIS_TRIAGE_CONFIRM:-2}
LOCK=${CIVVIS_TRIAGE_LOCK:-$HOME/.civvis-game-triage.lock}

say() { print -r -- "[triage] $(date -u +%FT%TZ) $*" >> "$LOG" }

if ! mkdir "$LOCK" 2>/dev/null; then
  holder=$(cat "$LOCK/pid" 2>/dev/null || print -r -- "")
  if [[ -n "$holder" ]] && kill -0 "$holder" 2>/dev/null; then
    say "another triage watchdog is alive (pid $holder); exiting"; exit 0
  fi
  rm -rf -- "$LOCK"; mkdir "$LOCK" 2>/dev/null || exit 70
fi
print -r -- "$$" > "$LOCK/pid"
trap 'rm -rf -- "$LOCK"' EXIT
trap 'exit 0' HUP INT TERM

abandon() {
  local why="$1"
  say "ABANDONING: $why"
  local climb=$(pgrep -f "[c]iv6_civvis_climb\.py" 2>/dev/null | head -1)
  [[ -n "$climb" ]] && kill -TERM "$climb" 2>/dev/null && say "  TERM climb $climb"
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    [[ -n "$climb" ]] && kill -0 "$climb" 2>/dev/null || break
    sleep 3
  done
  local play=$(pgrep -f "[c]iv6_play\.py" 2>/dev/null | head -1)
  [[ -n "$play" ]] && kill -INT "$play" 2>/dev/null && say "  INT civ6_play $play"
}

say "triage up (pid $$; hopeless t120<=-280 t160<=-350 t200<=-300; wedge gap>${WEDGE_GAP}; confirm ${CONFIRM}x)"
hopeless=0; wedged=0; last_tag=""
while true; do
  sleep "$POLL_S"
  pgrep -f "[c]iv6_play\.py" >/dev/null 2>&1 || { hopeless=0; wedged=0; continue }
  tag=$(ls -t "$RUNS" 2>/dev/null | grep '^civvis-' | head -1)
  [[ -z "$tag" ]] && continue
  [[ "$tag" != "$last_tag" ]] && { hopeless=0; wedged=0; last_tag=$tag; say "watching $tag" }

  read -r turn score rival <<<"$(python3 - "$RUNS/$tag/events.jsonl" <<'PY' 2>/dev/null
import json, sys, pathlib
p = pathlib.Path(sys.argv[1]); last = None
if p.is_file():
    with p.open() as fh:
        for line in fh:
            try: e = json.loads(line)
            except Exception: continue
            if (e.get("kind") == "turn" and e.get("ctx") == "agent"
                    and e.get("rival_best") is not None and e.get("score") is not None):
                last = (e.get("turn", 0), e["score"], e["rival_best"])
print(f"{last[0]} {last[1]} {last[2]}" if last else "")
PY
)"
  if [[ -n "${turn:-}" && "$turn" =~ '^[0-9]+$' ]]; then
    lead=$(( score - rival ))
    limit=""
    (( turn >= 120 )) && limit=-280
    (( turn >= 160 )) && limit=-350
    (( turn >= 200 )) && limit=-300
    if [[ -n "$limit" ]] && (( lead <= limit )); then
      hopeless=$(( hopeless + 1 ))
      say "$tag t${turn} score ${score} vs ${rival} (lead ${lead} <= ${limit}) strike ${hopeless}/${CONFIRM}"
      (( hopeless >= CONFIRM )) && { abandon "$tag hopeless at t${turn}: lead ${lead}"; hopeless=0 }
    else
      hopeless=0
    fi
  fi

  mirror=$(curl -s --max-time 5 "http://127.0.0.1:${PORT}/status" 2>/dev/null \
    | python3 -c 'import sys,json;print(json.load(sys.stdin).get("turn") or 0)' 2>/dev/null)
  agent=$(python3 - "$RUNS/$tag/orders.sqlite" <<'PY' 2>/dev/null
import sqlite3, sys
try:
    c = sqlite3.connect(f"file:{sys.argv[1]}?mode=ro", uri=True)
    print(c.execute("select coalesce(max(turn),0) from orders").fetchone()[0])
except Exception:
    print(-1)
PY
)
  if [[ "$mirror" =~ '^[0-9]+$' && "$agent" =~ '^[0-9]+$' ]] && (( mirror - agent >= WEDGE_GAP )); then
    wedged=$(( wedged + 1 ))
    say "$tag agent t${agent} vs game t${mirror} (gap $(( mirror - agent ))) strike ${wedged}/${CONFIRM}"
    (( wedged >= CONFIRM )) && { abandon "$tag dead agent: t${agent} vs t${mirror}"; wedged=0 }
  else
    wedged=0
  fi
done
