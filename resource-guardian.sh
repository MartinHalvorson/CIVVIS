#!/bin/zsh
# resource-guardian.sh — keeps this Mac from being monopolised by any one entity.
#
# Runs indefinitely. Every INTERVAL seconds it takes a census and applies the
# smallest action that fixes a real problem:
#
#   renice  a long-running CPU hog that is starving shorter work
#   kill    a process whose owning Claude session is gone, or that is eating
#           the machine's memory
#   log     everything else, for a human to look at
#
# Design rule: a process is only ever killed on evidence that it is abandoned or
# genuinely runaway. "Busy" is not a crime — this box is supposed to be busy.
# Ownership is decided by the owning session's transcript mtime, NOT by ppid==1:
# the Bash tool detaches children, so ppid==1 says nothing about abandonment.

set -u

LOG=/Users/martin/resource-guardian.log
STATE=/Users/martin/.resource-guardian.state
STOP=/Users/martin/.resource-guardian.stop
INTERVAL=${INTERVAL:-120}

CORES=$(sysctl -n hw.ncpu)
MEMBYTES=$(sysctl -n hw.memsize)
MEMGB=$((MEMBYTES / 1073741824))

# --- thresholds ---------------------------------------------------------------
RSS_KILL_GB=${RSS_KILL_GB:-40}      # single-process RSS that is a runaway on a 128G box
RSS_WARN_GB=${RSS_WARN_GB:-20}
HOG_CPU=${HOG_CPU:-80}              # %CPU to count as a sustained hog
HOG_MINUTES=${HOG_MINUTES:-45}      # ...for this long, below RENICE_TO, before we renice it
RENICE_TO=${RENICE_TO:-10}          # the nice value a hog is held down to
# A single process wider than this many cores is starving the rest of the fleet
# even if it has not yet run for HOG_MINUTES. On a shared box the damage from one
# job taking half the machine is immediate, so this pass acts on width, not age.
WIDE_CPU=${WIDE_CPU:-500}
WIDE_MINUTES=${WIDE_MINUTES:-10}
ORPHAN_STALE_MIN=${ORPHAN_STALE_MIN:-90}  # owner session silent this long => abandoned
ORPHAN_CPU=${ORPHAN_CPU:-25}        # ...and burning at least this much CPU => kill
DISK_FREE_GB=${DISK_FREE_GB:-100}
MEM_CRIT_PCT=${MEM_CRIT_PCT:-8}     # free memory % that counts as critical
MEM_CRIT_CHECKS=${MEM_CRIT_CHECKS:-3}   # ...for this many consecutive passes
MEM_VICTIM_MIN_GB=${MEM_VICTIM_MIN_GB:-5}  # never kill something too small to help
MEMFILE=/Users/martin/.resource-guardian.memcrit
SUSTAIN_MULT=${SUSTAIN_MULT:-3}     # load > cores*this ...
SUSTAIN_CHECKS=${SUSTAIN_CHECKS:-10}  # ... for this many consecutive passes = sustained
HIFILE=/Users/martin/.resource-guardian.hiload

# Never touch these, whatever they do. Long-lived jobs the operator wants alive.
# --spectate/--supervised and target/spectator are the viewer streams a human is
# actually watching (the frame-per-turn requirement); they must never be reniced
# into stutter, and they are a few % of the box, not a monopoliser.
# NOTE on the claude patterns: a live session's argv is `claude --dangerously-skip-permissions`,
# which matches neither `claude$` (not at end of line) nor `/claude ` (no leading slash).
# The bare-word alternative below is what actually protects the agent sessions; without it
# they were fair game, and they sit near the top of the RSS list at ~1GB each.
#
# ⚠⚠⚠ THE LIVE GAME WAS NOT ON THIS LIST AND GOT DEMOTED INTO THE BATCH BAND.
# Measured 2026-08-11 01:54:19, minutes after this watchdog was restarted:
#   RENICE ->10 sustained pid=59964 cpu=198.1 elapsed=50:11 :: .../MacOS/Civ6_Exe_Child
# That is the exact inversion of what this exists for. The spectator patterns
# above protect the VIEWER; nothing protected the GAME. The sustained rule is
# `HOG_CPU=80` for `HOG_MINUTES=45`, and a live ladder game sits at ~200% for 40
# to 100 minutes — so every run past the 45-minute mark was demoted to the same
# band as the `ai_eval` batch it is supposed to outrank.
#
# The live ladder is the scarce instrument on this box: one game is one data
# point for one revision, it cannot be parallelised, and Civ 6's turn loop runs
# off frames — so holding it down does not just slow it, it changes what it
# measures. `Civ6_Exe` is the game, `civ6_play.py` is the harness driving it and
# `civvis_orders` is the decision worker on the critical path of every turn.
# None of them is a monopoliser: the game peaks near 250% of an 1800% box.
PROTECT_RE='Civ6_Exe|civ6_play\.py|civvis_orders|civvis-keeper|spectator_supervisor|target/spectator|--spectate|--supervised|Terminal|WindowServer|loginwindow|launchd|kernel_task|ollama|Finder|SystemUIServer|(^| )claude( |$)|claude$|/claude |sshd|resource-guardian'

# Killing is a much blunter act than renicing, so the memory pass gets a stricter
# veto list on top of PROTECT_RE. Under real pressure the largest unprotected
# process can easily be an OS daemon (mediaanalysisd, Spotlight, a wallpaper
# extension) — shedding those neither frees fleet memory nor sticks, since launchd
# just restarts them, and it destabilises the desktop. Only fleet work is shed.
KILL_NEVER_RE='^/System/|^/usr/(libexec|sbin|bin)/|^/Library/Apple|^/Applications/Utilities/|MediaAnalysis|Spotlight|mds_|WindowServer|logind|distnoted|Wallpaper'

DRYRUN=${DRYRUN:-0}

log() { print -r -- "$(date '+%Y-%m-%d %H:%M:%S')  $*" | tee -a $LOG }

# act <verb> <pid> — do it, unless we are only rehearsing
do_kill()   { (( DRYRUN )) && return 0; kill -TERM $1 2>/dev/null; sleep 2; kill -KILL $1 2>/dev/null }
# Absolute, not an increment: macOS `renice -n N` adds N to the current value, so
# repeated passes would ratchet a job down to 20. `renice N -p pid` sets it flat.
do_renice() { (( DRYRUN )) && return 0; renice $RENICE_TO -p $1 >/dev/null 2>&1 }

# Minutes since the owning Claude session last wrote its transcript.
# Prints "" when the process is not attributable to a session at all.
owner_stale_min() {
  local pid=$1 args=$2 sid f mt cwd
  # argv first (cheap). Most jobs here are launched as ./target/release/foo from a
  # scratchpad cwd, so argv carries no session id — fall back to the cwd via lsof,
  # which is where the id actually lives. Without the fallback this whole pass is
  # a silent no-op that never fires and therefore always looks healthy.
  sid=$(print -r -- "$args" | grep -o '/-Users-martin/[0-9a-f-]\{36\}' | head -1 | sed 's|.*/||')
  if [[ -z $sid ]]; then
    cwd=$(lsof -a -p $pid -d cwd -Fn 2>/dev/null | grep '^n' | head -1)
    sid=$(print -r -- "$cwd" | grep -o '/-Users-martin/[0-9a-f-]\{36\}' | head -1 | sed 's|.*/||')
  fi
  [[ -z $sid ]] && { print -r -- ""; return }
  f=$(ls -1 /Users/martin/.claude/projects/-Users-martin/${sid}*.jsonl 2>/dev/null | head -1)
  # No transcript => we cannot prove the owner is gone. Don't kill what you can't
  # confirm; skip rather than assume abandonment.
  [[ -z $f ]] && { print -r -- ""; return }
  mt=$(stat -f %m "$f" 2>/dev/null) || { print -r -- ""; return }
  print -r -- $(( ( $(date +%s) - mt ) / 60 ))
}

# elapsed "dd-hh:mm:ss" / "hh:mm:ss" / "mm:ss" -> minutes
etime_min() {
  local e=$1 d=0 h=0 m=0
  if [[ $e == *-* ]]; then d=${e%%-*}; e=${e#*-}; fi
  local -a p; p=(${(s.:.)e})
  case ${#p} in
    3) h=${p[1]}; m=${p[2]} ;;
    2) m=${p[1]} ;;
    1) m=0 ;;
  esac
  print -r -- $(( 10#$d*1440 + 10#$h*60 + 10#$m ))
}

log "=== guardian start: ${CORES} cores, ${MEMGB}GB, interval ${INTERVAL}s, pid $$ ==="

while :; do
  [[ -f $STOP ]] && { log "stop file present, exiting"; rm -f $STOP; exit 0 }

  load1=$(sysctl -n vm.loadavg | awk '{print $2}')
  freepct=$(memory_pressure 2>/dev/null | awk -F': ' '/free percentage/{gsub(/%/,"",$2); print $2}')
  [[ -z ${freepct:-} ]] && freepct=100
  diskfree=$(df -g /System/Volumes/Data 2>/dev/null | awk 'NR==2{print $4}')
  # macOS pgrep has no -c; `pgrep -c` fails usage and would silently report 0.
  nrustc=$(pgrep -x rustc 2>/dev/null | wc -l | tr -d ' ')
  nclaude=$(pgrep -x claude 2>/dev/null | wc -l | tr -d ' ')

  acted=0

  # ---- pass 1: memory runaways --------------------------------------------
  while read -r pid rssk pcpu et args; do
    [[ -z ${pid:-} ]] && continue
    print -r -- "$args" | grep -qE "$PROTECT_RE" && continue
    local_gb=$(( rssk / 1048576 ))
    if (( local_gb >= RSS_KILL_GB )); then
      log "KILL runaway-memory pid=$pid rss=${local_gb}GB cpu=$pcpu elapsed=$et :: ${args[1,120]}"
      do_kill $pid
      acted=1
    elif (( local_gb >= RSS_WARN_GB )); then
      log "WARN  high-memory   pid=$pid rss=${local_gb}GB cpu=$pcpu elapsed=$et :: ${args[1,120]}"
    fi
  done < <(ps -Ao pid,rss,pcpu,etime,args | tail -n +2 | awk '$2>10485760')

  # ---- pass 1b: aggregate memory pressure ----------------------------------
  # Pass 1 only fires when a SINGLE process balloons past RSS_KILL_GB. The way
  # this box actually runs out of memory is the fleet in aggregate: five agents
  # at 25GB each exhaust 128GB while no one of them is anywhere near the
  # per-process limit. macOS will not wait politely for that — jetsam picks a
  # victim on its own, and its pick is arbitrary: it could take the spectator
  # stream a human is watching, or a claude session with an hour of work in it.
  # Making a deliberate choice on sustained evidence beats letting the kernel
  # make a random one. Sustained, because a momentary dip is not an emergency.
  if (( freepct <= MEM_CRIT_PCT )); then
    mcrit=$(( $(cat $MEMFILE 2>/dev/null || print 0) + 1 ))
  else
    mcrit=0
  fi
  print -r -- $mcrit > $MEMFILE
  if (( mcrit >= MEM_CRIT_CHECKS )); then
    vpid=""; vrssk=0; vargs=""
    while read -r p rk rest; do
      print -r -- "$rest" | grep -qE "$PROTECT_RE" && continue
      print -r -- "$rest" | grep -qE "$KILL_NEVER_RE" && continue
      (( rk / 1048576 < MEM_VICTIM_MIN_GB )) && continue
      vpid=$p; vrssk=$rk; vargs=$rest
      break
    done < <(ps -Ao pid,rss,args -m | tail -n +2 | head -25)
    if [[ -n $vpid ]]; then
      log "KILL memory-pressure free=${freepct}% for $(( mcrit * INTERVAL / 60 ))min :: largest unprotected pid=$vpid rss=$(( vrssk / 1048576 ))GB :: ${vargs[1,110]}"
      do_kill $vpid
      acted=1
      print -r -- 0 > $MEMFILE
    else
      log "WARN  memory free ${freepct}% sustained, but no unprotected consumer >= ${MEM_VICTIM_MIN_GB}GB to shed"
    fi
  fi

  # ---- pass 2: abandoned work (owner session long silent, still burning CPU)
  while read -r pid ppid pcpu et args; do
    [[ -z ${pid:-} ]] && continue
    print -r -- "$args" | grep -qE "$PROTECT_RE" && continue
    stale=$(owner_stale_min $pid "$args")
    [[ -z $stale ]] && continue                 # not attributable => leave alone
    (( stale < ORPHAN_STALE_MIN )) && continue  # owner still active => legitimate
    cpu_int=${pcpu%%.*}
    (( cpu_int < ORPHAN_CPU )) && continue      # idle => harmless, leave it
    log "KILL abandoned pid=$pid owner-silent=${stale}min cpu=$pcpu elapsed=$et :: ${args[1,120]}"
    do_kill $pid
    acted=1
    # NOTE: the CPU floor is applied in awk, not in the loop body. Without it this
    # pass walks ~600 ppid==1 system daemons and forks 4 processes for each — the
    # monitor becomes the hog. With it, typically one row survives to the fork.
  done < <(ps -Ao pid,ppid,pcpu,etime,args | tail -n +2 | awk -v c=$ORPHAN_CPU '($1==1||$2==1) && $3>=c')

  # ---- pass 3: sustained hogs at top priority get told to yield -------------
  while read -r pid ni pcpu et args; do
    [[ -z ${pid:-} ]] && continue
    # Skip only what is ALREADY yielding as much as we would ask for. The guard
    # used to be `ni != 0`, which meant a job launched at nice 5 was exempt from
    # this pass forever, however much CPU it took — a little politeness at launch
    # bought total immunity from fair-share. Anything below RENICE_TO is fair game.
    (( ni >= RENICE_TO )) && continue
    print -r -- "$args" | grep -qE "$PROTECT_RE" && continue
    cpu_int=${pcpu%%.*}
    (( cpu_int < HOG_CPU )) && continue
    mins=$(etime_min "$et")
    # Two ways to qualify. A merely-busy job gets the long grace period; a job
    # already spread across WIDE_CPU/100 cores is starving the other agents right
    # now, and waiting HOG_MINUTES to notice just means 45min of unfair sharing.
    if (( cpu_int >= WIDE_CPU )); then
      (( mins < WIDE_MINUTES )) && continue
      why="wide=$(( cpu_int / 100 ))cores"
    else
      (( mins < HOG_MINUTES )) && continue
      why="sustained"
    fi
    do_renice $pid \
      && { log "RENICE ->$RENICE_TO $why pid=$pid cpu=$pcpu elapsed=$et :: ${args[1,120]}"; acted=1 }
  done < <(ps -Ao pid,nice,pcpu,etime,args -r | tail -n +2 | awk -v c=$HOG_CPU -v n=$RENICE_TO '$2<n && $3>=c' | head -20)

  # ---- pass 4: sustained pressure ------------------------------------------
  # An instantaneous spike is normal on this box and not worth a line in the log.
  # What matters is pressure that persists: "too many resources for too long".
  # Only when load has been above cores*SUSTAIN_MULT for SUSTAIN_CHECKS
  # consecutive passes do we record it — together with who was responsible, so
  # there is evidence after the fact rather than just a number.
  load_int=${load1%%.*}
  if (( load_int > CORES * SUSTAIN_MULT )); then
    hi=$(( $(cat $HIFILE 2>/dev/null || print 0) + 1 ))
  else
    hi=0
  fi
  print -r -- $hi > $HIFILE
  if (( hi == SUSTAIN_CHECKS )); then
    mins=$(( SUSTAIN_CHECKS * INTERVAL / 60 ))
    log "WARN  load ${load1} on ${CORES} cores sustained ${mins}min (rustc=$nrustc claude=$nclaude). Top consumers:"
    ps -Ao pcpu,pid,etime,comm -r | tail -n +2 | head -6 | while read c p e cm; do
      log "        ${c}% pid=$p elapsed=$e $cm"
    done
  elif (( hi > SUSTAIN_CHECKS && hi % (SUSTAIN_CHECKS * 5) == 0 )); then
    log "WARN  load still elevated (${load1}) after $(( hi * INTERVAL / 60 ))min"
  fi
  # ---- the inversion this watchdog could not see -----------------------------
  # ⚠⚠⚠ PROTECTED IS NOT THE SAME AS PRIORITISED, and for two weeks nobody noticed.
  # PROTECT_RE only stops this script from demoting something; it cannot promote
  # anything, and macOS lets an unprivileged process RAISE its nice but never
  # lower it. So a live game that started life demoted stays demoted, silently,
  # underneath every ordinary nice-0 job on the box.
  #
  # It does start life demoted. `civvis-interactive-host.sh` launches the game
  # supervisor with `&`, and macOS puts a shell-backgrounded process in a lower
  # band -- measured 2026-08-11: the identical command is nice 0 in the
  # foreground and nice 5 backgrounded, with the whole subtree inheriting it, so
  # Civ6_Exe_Child runs at 5 while a 199% rustc sits at 0 above it. The fix is at
  # the launch site (launchd, ProcessType Interactive + Nice 0, as
  # com.civvis.keeper.plist already does); all this pass can do is stop it being
  # invisible. Reported once per pass, not acted on -- there is no safe action.
  #
  # ⚠⚠ AND IT IS NOT ONLY THE GAME. The first version of this check looked for
  # `Civ6_Exe` alone, which repeats in miniature the mistake it was written to
  # catch: PROTECT_RE is a DENYLIST BY OMISSION, so anything new and important
  # is unwatched until somebody remembers it. The game itself went unlisted for
  # two weeks. Every protected process is protected because it must not be held
  # down, so every one of them deserves the same check.
  #
  # The best nice among unprotected jobs actually burning CPU is the bar. A
  # protected process doing real work (>20%) below that bar is inverted. Idle
  # protected processes are excluded deliberately: something waiting on a queue
  # at nice 10 is not being starved, and reporting it would be the noise that
  # teaches everyone to skip these lines.
  bar=$(ps -Ao nice,pcpu,args | tail -n +2 | grep -vE "$PROTECT_RE" \
    | awk '$2>20 {if (n=="" || $1<n) n=$1} END{print (n=="" ? "" : n)}')
  #
  # ⚠ NAME IT FROM `comm`, NOT FROM THE ARGS. The first cut printed the third
  # whitespace field of the command line, which for the live game is
  # `/Users/martin/Library/Application` — so every report read "protected
  # Application", a label naming nothing. The match still has to run against the
  # full argv, because that is what PROTECT_RE is written for; only the LABEL
  # comes from `comm`, looked up per hit. Hits are rare by construction, so the
  # extra fork costs nothing on a healthy pass.
  if [[ -n ${bar:-} ]]; then
    ps -Ao nice,pcpu,pid,args | tail -n +2 | grep -E "$PROTECT_RE" \
      | awk -v b="$bar" '$2>20 && $1>b {print $1, $2, $3}' \
      | while read -r ni cpu hitpid; do
          what=$(ps -o comm= -p $hitpid 2>/dev/null)
          [[ -z ${what:-} ]] && what="pid $hitpid"
          log "WARN  PRIORITY INVERSION: protected ${what:t} (pid $hitpid) is at nice ${ni} on ${cpu}% CPU while unprotected work runs at nice ${bar}. macOS forbids lowering nice unprivileged, so this cannot be repaired here -- fix the launch site."
        done
  fi

  (( freepct < 15 )) && log "WARN  memory free ${freepct}%"
  [[ -n ${diskfree:-} ]] && (( diskfree < DISK_FREE_GB )) && log "WARN  disk free ${diskfree}GB"

  # fg/bg is the fairness split the operator actually wants at a glance: how many
  # cores foreground work is getting versus long-running batch held at RENICE_TO.
  share=$(ps -Ao nice,pcpu | tail -n +2 | awk -v n=$RENICE_TO \
    '{if($2<1)next; if($1>=n) b+=$2; else f+=$2} END{printf "fg=%.1f bg=%.1f", f/100, b/100}')
  print -r -- "$(date '+%H:%M:%S') load=$load1 memfree=${freepct}% disk=${diskfree}G rustc=$nrustc claude=$nclaude $share cores acted=$acted" > $STATE

  sleep $INTERVAL
done
