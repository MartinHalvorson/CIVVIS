#!/bin/zsh
# civvis-games — the on/off switch for the two unattended game lanes.
#
#   civvis-games on  [reason]   run both lanes indefinitely
#   civvis-games retire [reason] request the active game's native Retire action
#   civvis-games off [reason]   stop both lanes NOW and keep them stopped
#   civvis-games status         what is running, and why
#   civvis-games wins [n]       the last n live-game wins from the ladder ledger
#   civvis-games ensure         make the world match the operator's standing intent
#
# This is the TRACKED copy. `civvis-install-host-automation.sh` makes
# ~/bin/civvis-games a symlink to it, so every host runs the same switch and a
# fix landed here reaches all of them. Its own paths are derived from where it
# lives (`OPS=${0:A:h}` resolves the symlink), never from a named checkout.
#
# The two lanes:
#   ladder     Firaxis Civ VI, driven through the GUI. com.civvis.ladder-watchdog
#              restarts it whenever it stops, so "indefinitely" is its default.
#   spectator  the headless CIVVIS exhibition league on :8766 (com.civvis.spectator,
#              KeepAlive).
#
# ⚠⚠ WHY `off` IS MORE THAN `gamelock.py --halt`. The halt marker is honoured at
# every *boundary* — the watchdog won't start a loop, the interactive host exits,
# gamelock.acquire() refuses a new attempt, the spectator stands down. It cannot
# stop a batch that is already playing: civvis-game-supervisor.sh sets
# `trap 'exit 0' ... TERM`, and zsh defers a trap until the foreground command
# returns — that command being civ6_civvis_climb.py, which runs for hours.
# Measured 2026-08-20T17:23Z: the halt landed, the host exited, and the supervisor,
# the climb and a live Civ VI game all kept going as orphans. So `off` halts AND
# tears the live chain down explicitly, youngest process first.
#
# ⚠ TERM, never KILL, on the Civ VI core — a hard kill wedges it and the remedy
# becomes the fault. And every pgrep here is bracket-escaped, because
# civ6_civvis_climb.busy() greps for these same strings: an unescaped pattern in
# this script's own argv makes the next climb refuse to start.
set -u

OPS=${0:A:h}
REPO=${CIVVIS_REPO:-${OPS:h:h}}
GAMELOCK="$REPO/tools/civ6_control/gamelock.py"
RUN_ROOT=${CIVVIS_RUNS_ROOT:-$HOME/civvis-civ6-runs/control}
STOCK_LAUNCHER="$OPS/civvis-ladder-terminal-launcher.sh"
# The operator wrapper the ladder keeper also honours (civvis_collab.py's
# LADDER_OPERATOR_WRAPPER): only its EXISTENCE is a decision here, exactly as
# it is there, so `on`/`ensure` and a keeper recovery start the same loop.
WRAPPER=${CIVVIS_LADDER_WRAPPER:-$HOME/civvis-verification-launch.command}
UID_N=$(id -u)
LADDER_JOB=com.civvis.ladder-watchdog
SPECTATOR_JOB=com.civvis.spectator

say() { print -r -- "$*" }

launcher() {
  if [[ -x "$WRAPPER" ]]; then print -r -- "$WRAPPER"; else print -r -- "$STOCK_LAUNCHER"; fi
}

pids_for() { pgrep -f "$1" 2>/dev/null | tr '\n' ' ' }

term_wait() {   # term_wait <label> <pattern> <seconds>
  local label=$1 pattern=$2 budget=$3 found
  found=$(pids_for "$pattern")
  [[ -z ${found// } ]] && return 0
  say "  stopping ${label}: ${found}"
  kill -TERM ${=found} 2>/dev/null
  local waited=0
  while (( waited < budget )); do
    [[ -z $(pids_for "$pattern") ]] && return 0
    sleep 2; (( waited += 2 ))
  done
  found=$(pids_for "$pattern")
  [[ -z ${found// } ]] && return 0
  say "  ⚠ ${label} still alive after ${budget}s: ${found}"
  return 1
}

job_state() {   # job_state <label>
  local label=$1 loaded disabled
  launchctl print "gui/${UID_N}/${label}" >/dev/null 2>&1 && loaded=loaded || loaded="NOT loaded"
  launchctl print-disabled "gui/${UID_N}" 2>/dev/null \
    | grep -q "\"${label}\" => disabled" && disabled=" (DISABLED)" || disabled=""
  print -r -- "${loaded}${disabled}"
}

ensure_job() {  # ensure_job <label>
  local label=$1
  local plist="$HOME/Library/LaunchAgents/${label}.plist"
  [[ -f $plist ]] || { say "  ⚠ missing ${plist}"; return 1 }
  launchctl enable "gui/${UID_N}/${label}" 2>/dev/null
  launchctl print "gui/${UID_N}/${label}" >/dev/null 2>&1 \
    || launchctl bootstrap "gui/${UID_N}" "$plist" 2>/dev/null
  say "  ${label}: $(job_state $label)"
}

# Recent wins from the ladder ledger. ⚠ The live ledger beside the runs is the
# current one; docs/civ6_ladder.json only catches up on `civ6_ladder.py sync`
# and was a day and two wins stale when this was written.
wins_report() {   # wins_report <how-many>
  local limit=${1:-5}
  python3 - "$limit" "$REPO" <<'PYEOF'
import json, sys
from pathlib import Path

LADDER = ["DIFFICULTY_SETTLER", "DIFFICULTY_CHIEFTAIN", "DIFFICULTY_WARLORD",
          "DIFFICULTY_PRINCE", "DIFFICULTY_KING", "DIFFICULTY_EMPEROR",
          "DIFFICULTY_IMMORTAL", "DIFFICULTY_DEITY"]
limit = int(sys.argv[1])
repo = Path(sys.argv[2])
home = Path.home()
for candidate in (home / "civvis-civ6-runs/control/ladder.json",
                  repo / "docs/civ6_ladder.json"):
    if candidate.is_file():
        ledger = candidate
        break
else:
    print("  (no ladder ledger found)"); raise SystemExit

attempts = json.loads(ledger.read_text()).get("attempts", [])
# ⚠ The ledger is NOT stored in chronological order — two 2026-08-16 Settler
# wins sit newest-first inside the same day — so sort on `utc` rather than
# trusting list order, or the oldest rows print out of sequence.
wins = sorted((a for a in attempts if a.get("won")), key=lambda a: a.get("utc") or "")
print(f"  {len(wins)} wins in {len(attempts)} recorded attempts"
      f"  ({ledger.parent.name}/{ledger.name})")
by_rung = {}
for w in wins:
    by_rung[w.get("difficulty")] = by_rung.get(w.get("difficulty"), 0) + 1
tally = "  ".join(
    f"lv{LADDER.index(d) + 1} {d.split('_')[-1].title()}: {n}"
    for d, n in sorted(by_rung.items(),
                       key=lambda kv: LADDER.index(kv[0]) if kv[0] in LADDER else 99)
    if d)
if tally:
    print(f"  by rung — {tally}")
if not wins:
    raise SystemExit
print()
for w in wins[-limit:][::-1]:
    diff = w.get("difficulty") or ""
    lvl = f"lv{LADDER.index(diff) + 1} {diff.split('_')[-1].title()}" if diff in LADDER else "lv? "
    # `victory_type` is the string; guard on None, never truthiness — the
    # sibling `victory` field is a count whose legitimate 0 is falsy.
    vt = w.get("victory_type")
    vt = vt if vt is not None else "(type unrecorded)"
    when = (w.get("utc") or "?").replace("T", " ").replace(":00Z", "Z")[:16] + "Z"
    score, rival = w.get("score"), w.get("rival_best")
    margin = f"{score} vs {rival}" if score is not None and rival is not None else str(score)
    print(f"  {when}  {lvl:<14} {vt:<18} {margin:>13}  t{w.get('turns')}")
PYEOF
}

# Which CIVVIS revision the games actually run. ⚠ `~/.civvis-play-pin` is read
# by civvis-game-supervisor.sh at the START OF EVERY CYCLE, so changing it needs
# no restart — that is the whole point of the file. Contents are either "head"
# (fetch + `checkout --detach origin/main`, and REFUSE the batch unless the
# checkout reads back as exactly origin/main) or an absolute path to a tree,
# which pins the games to that tree and stops tracking GitHub. A pin naming a
# path that does not exist makes the supervisor log "no tree at ..." every 60s
# and play nothing — that is how the lane was found stalled on 2026-08-21.
PINFILE=${CIVVIS_PINFILE:-$HOME/.civvis-play-pin}

play_tree() {   # the tree the supervisor would build, resolved as it resolves it
  local pin supcmd tree
  pin=$(cat "$PINFILE" 2>/dev/null || print -r -- head)
  if [[ -n $pin && $pin != head ]]; then
    print -r -- "$pin"; return
  fi
  # pin==head → HEAD_REPO, which the supervisor derives as three levels up from
  # its own script, so read it off the live process rather than assuming a tree.
  supcmd=$(ps -o command= -p "${${$(pids_for 'civvis-game-superviso[r]\.sh')%% *}:-0}" 2>/dev/null)
  tree=${${supcmd##* }:h:h:h}
  [[ -n $tree && -f $tree/Cargo.toml ]] && print -r -- "$tree" || print -r -- "$REPO"
}

version_report() {
  local pin tree head main behind batch
  pin=$(cat "$PINFILE" 2>/dev/null || print -r -- head)
  tree=$(play_tree)
  if [[ -z $pin || $pin == head ]]; then
    say "  pin        head — every batch fetches origin/main and refuses to run if it is not exact"
  elif [[ -f $pin/Cargo.toml ]]; then
    say "  pin        $pin"
    say "             ⚠ PINNED TO A TREE — batches do NOT track GitHub"
  else
    say "  pin        $pin"
    say "             ⚠⚠ PIN NAMES NO TREE — the supervisor refuses every cycle ('no tree at ...')"
    say "             fix: printf 'head\n' > $PINFILE   (takes effect next cycle, no restart)"
  fi
  head=$(git -C "$tree" rev-parse --short HEAD 2>/dev/null)
  main=$(git -C "$tree" rev-parse --short origin/main 2>/dev/null)
  behind=$(git -C "$tree" rev-list --count HEAD..origin/main 2>/dev/null)
  say "  tree       ${tree:t}"
  if [[ -n $head && -n $main ]]; then
    if [[ $head == $main ]]; then
      say "  revision   $head == origin/main — current"
    else
      say "  revision   $head, origin/main $main — ${behind:-?} commit(s) behind"
    fi
  fi
  batch=$(grep -h "batch pinned to" $(ls -t "$HOME"/civvis-climb-logs/climb-*.log 2>/dev/null | head -1) 2>/dev/null | tail -1)
  [[ -n $batch ]] && say "  live batch ${batch##*to }"
  # ⚠ Forced genome arms change WHAT THE SEAT IS PLAYING and therefore what a
  # ladder row means, but they live in a file no other report mentions — a
  # sibling session set barbarian-hunt on 2026-08-21 and two batches ran with it
  # before anyone looked. Surface it beside the revision, where it belongs.
  local forced="$HOME/.civvis-live-force-on"
  if [[ -s $forced ]]; then
    say "  ⚠ forced    $(tr -d '\n' < $forced)  (arm forced ON via ${forced/#$HOME/~}, set $(stat -f %Sm $forced))"
  fi
  local withheld="${CIVVIS_WITHOUT:-}"
  [[ -n $withheld ]] && say "  ⚠ withheld  $withheld"
}

# ⚠⚠ THE OPERATOR'S STANDING INTENT, which is NOT the same thing as the halt
# marker. The halt is a lock any session (or any script) can take; on
# 2026-08-21 a sibling session halted the lane to "stage latest origin/main",
# never resumed, and the games stayed down for over an hour with nothing
# reporting it — the ladder watchdog does exactly what it is designed to do
# against a halt, which is stand down. The operator's instruction is that games
# run continuously unless THEY stop them, so that intent has to outlive any one
# session and be checked by something. This file is that intent; `ensure` is the
# something (com.civvis.keepplaying runs it every five minutes).
INTENTFILE=${CIVVIS_INTENTFILE:-$HOME/.civvis-operator-intent}
# A halt younger than this is left alone, so a legitimate short staging halt by
# another session still works. Older than this, with intent=running, is a
# forgotten halt and gets cleared.
HALT_GRACE_S=${CIVVIS_HALT_GRACE_S:-600}

set_intent() { print -r -- "$1" > "$INTENTFILE" }

halt_age_s() {   # prints the halt's age in seconds, or nothing when not halted
  python3 - <<'PYEOF'
import json, sys
from datetime import datetime, timezone
from pathlib import Path
marker = Path.home() / ".civvis-operator-halt.json"
try:
    since = json.loads(marker.read_text()).get("since")
    stamp = datetime.strptime(since, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
    print(int((datetime.now(timezone.utc) - stamp).total_seconds()))
except Exception:
    pass
PYEOF
}

relaunch_ladder() {
  if [[ -n $(pids_for 'civvis-interactive-hos[t]\.sh') ]]; then
    say "  ladder: an interactive host is already up"
  else
    # The ladder must start THROUGH Terminal: installing the control mod writes
    # inside Civ6.app and macOS grants that to Terminal, not to launchd. `-g -j`
    # keeps the window out of the way of the game being recorded.
    open -g -j -a Terminal "$(launcher)" && say "  ladder: asked Terminal to start the loop via ${$(launcher):t}"
  fi
}

case ${1:-status} in

on)
  reason=${2:-"operator: run both lanes indefinitely"}
  say "== turning the game lanes ON =="
  python3 "$GAMELOCK" --resume >/dev/null 2>&1
  set_intent running
  say "  operator halt cleared (${reason}); standing intent recorded: running"
  say "  services:"
  ensure_job "$LADDER_JOB"
  ensure_job "$SPECTATOR_JOB"
  # Turning the lane on must never silently resume a tree-pinned or dead-pin
  # batch: the operator asked for games on the latest GitHub CIVVIS, always.
  # Do this BEFORE opening Terminal.  The verified-head wrapper refuses an
  # absent or non-head pin, and starting it first would leave `on` reporting
  # success while its just-opened window immediately exits.
  pin_now=$(cat "$PINFILE" 2>/dev/null || true)
  if [[ $pin_now == head ]]; then
    say "  version:   pin already tracks origin/main"
  else
    printf 'head\n' > "$PINFILE"
    if [[ -n $pin_now ]]; then
      say "  version:   pin was '$pin_now' — reset to head so batches track origin/main"
    else
      say "  version:   pin was absent — wrote head so batches track origin/main"
    fi
  fi
  relaunch_ladder
  say "  spectator: KeepAlive brings it back within ~60s"
  say ""
  say "watch it:  tail -f ~/Library/Logs/civvis-ladder.log"
  say "retire one game: civvis-games retire"
  say "turn off:  civvis-games off"
  ;;

retire)
  reason=${2:-"operator: retire current verification game"}
  # A retirement replaces one live game; it is not an alternate spelling of
  # `off`.  Refuse an explicitly halted lane so the command cannot claim that a
  # successor will arrive when the operator has told all supervisors to stop.
  if python3 "$GAMELOCK" --halt-status >/dev/null 2>&1; then
    say "== retiring the active game =="
    say "  refused: the game lanes are OFF; use civvis-games on before requesting a replacement"
    exit 2
  fi
  say "== retiring the active game =="
  python3 "$REPO/tools/civ6_control/operator_retire.py" request \
    --runs-root "$RUN_ROOT" --reason "$reason"
  retire_status=$?
  if (( retire_status != 0 )); then
    exit "$retire_status"
  fi
  # A live game can outlast an accidentally stale intent file.  Only set it
  # after the request has safely bound to one real harness, so a typo does not
  # cause an idle host to start playing by itself.
  set_intent running
  say "  requested Civilization VI's native Retire action; no process was stopped"
  say "  lane remains ON; after operator_retired is recorded, the supervisor starts the next game"
  ;;

off)
  reason=${2:-"operator: stop the game lanes"}
  say "== turning the game lanes OFF =="
  python3 "$GAMELOCK" --halt --reason "$reason" >/dev/null 2>&1
  set_intent stopped
  say "  operator halt set — no boundary will start new work; standing intent: stopped"
  # Youngest first: the climb TERMs what it can, then anything it orphaned.
  term_wait "climb"           'civ6_civvis_clim[b]\.py'        30
  term_wait "play harness"    'civ6_pla[y]\.py'                80
  term_wait "brain"           'civ6_brai[n]\.py'               20
  term_wait "mirror follower" 'tools/follo[w]\.py'             15
  term_wait "popup clearer"   'popup_clea[r]\.py'              15
  term_wait "supervisor"      'civvis-game-superviso[r]\.sh'   20
  term_wait "interactive host" 'civvis-interactive-hos[t]\.sh' 15
  term_wait "keepers"         'civvis-(mirror|popup)-keepe[r]\.sh' 15
  term_wait "mirror server"   'civvis pla[y] --mirror'         15
  # The spectator stands itself down on the halt; give it a moment to notice.
  sleep 3
  term_wait "spectator game"  'civvis pla[y] .*--leagu[e]'     20
  if [[ -n $(pids_for 'Civ6_Ex[e]') ]]; then
    say "  ⚠ Civilization VI is still up: $(pids_for 'Civ6_Ex[e]')"
    say "    quit it from the app — never hard-kill the core, it wedges."
  else
    say "  Civilization VI is closed"
  fi
  say ""
  say "the launchd services stay loaded and honour the halt, so nothing restarts."
  say "to end one active game but keep playing: civvis-games retire"
  say "turn on:  civvis-games on"
  ;;

ensure)
  # Idempotent keeper: make the world match the operator's standing intent.
  intent=$(cat "$INTENTFILE" 2>/dev/null || print -r -- running)
  if [[ $intent != running ]]; then
    say "intent=stopped — leaving the lanes down"
    exit 0
  fi
  age=$(halt_age_s)
  if [[ -n $age ]]; then
    if (( age >= HALT_GRACE_S )); then
      say "intent=running but a halt has stood ${age}s (>= ${HALT_GRACE_S}s grace) — clearing it"
      python3 "$GAMELOCK" --resume >/dev/null 2>&1
      relaunch_ladder
    else
      say "intent=running; halt is only ${age}s old — inside the grace, leaving it"
      exit 0
    fi
  fi
  if [[ -z $(pids_for 'civ6_pla[y]\.py') && -z $(pids_for 'civvis-interactive-hos[t]\.sh') ]]; then
    say "intent=running but no ladder chain is up — starting it"
    relaunch_ladder
  fi
  ;;

status)
  say "== civvis game lanes =="
  if halt=$(python3 "$GAMELOCK" --halt-status 2>&1); then
    say "switch:    OFF — ${halt}"
  else
    say "switch:    ON — no operator halt"
  fi
  say "services:  ${LADDER_JOB}: $(job_state $LADDER_JOB)"
  say "           ${SPECTATOR_JOB}: $(job_state $SPECTATOR_JOB)"
  if [[ -x "$WRAPPER" ]]; then
    say "entry:     ${WRAPPER/#$HOME/~} -> $(readlink "$WRAPPER" 2>/dev/null || print -r -- '(a file of its own)')"
  else
    say "entry:     stock launcher ${STOCK_LAUNCHER:t} (no ${WRAPPER/#$HOME/~}; run civvis-install-host-automation.sh)"
  fi
  say ""
  say "version (what the next batch will build):"
  version_report
  say ""
  say "ladder lane (Firaxis Civ VI):"
  for label pattern in \
      "watchdog-started host" 'civvis-interactive-hos[t]\.sh' \
      "supervisor"            'civvis-game-superviso[r]\.sh' \
      "climb"                 'civ6_civvis_clim[b]\.py' \
      "play harness"          'civ6_pla[y]\.py' \
      "Civ VI"                'Civ6_Ex[e]' \
      "mirror follower"       'tools/follo[w]\.py'; do
    found=$(pids_for "$pattern")
    printf '  %-22s %s\n' "$label" "${found:-—}"
  done
  run=$(ls -td "$RUN_ROOT"/*/ 2>/dev/null | head -1)
  if [[ -n $run && -f "$run/events.jsonl" ]]; then
    turn=$(grep -o '"turn": [0-9]*' "$run/events.jsonl" 2>/dev/null | tail -1 | tr -dc 0-9)
    say "  newest run           $(basename $run) (turn ${turn:-?}, $(stat -f %Sm "$run/events.jsonl"))"
    if [[ -f "$run/operator-retire-request.json" && ! -f "$run/operator-retire.json" ]]; then
      say "  retirement           requested — awaiting native in-game retirement acknowledgement"
    elif [[ -f "$run/operator-retire.json" ]]; then
      say "  retirement           recorded — operator_retired"
    fi
  fi
  say ""
  say "recent live-game wins (Firaxis Civ VI):"
  wins_report 5
  say ""
  say "spectator lane (headless CIVVIS league):"
  printf '  %-22s %s\n' "supervisor" "${$(pids_for 'spectator_superviso[r]\.py'):-—}"
  printf '  %-22s %s\n' "game" "${$(pids_for 'civvis pla[y] .*--leagu[e]'):-—}"
  ;;

wins)
  say "== live-game wins =="
  wins_report "${2:-20}"
  ;;

*)
  say "usage: civvis-games {on|retire|off|status|wins [n]|ensure} [reason]"
  exit 64
  ;;
esac
