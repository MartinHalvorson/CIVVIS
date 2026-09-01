#!/bin/zsh
# Capacity-gated continuation of the isolated 6-player rush generalization.
# Parts 3 and 4 use distinct maps, run sequentially, and append only to the
# supplemental log so the primary round-robin remains independent.

set -u

RESULT_DIR=/Users/martbot-mbp-m5-max-128/civvis-simulation-results
LOG="$RESULT_DIR/supplemental-6p-rush-generalization-20260731T0140.log"

mkdir -p "$RESULT_DIR"
exec >> "$LOG" 2>&1

capacity() {
  local idle
  idle="$(top -l 1 -n 0 -o cpu 2>/dev/null | awk '
    /CPU usage:/ {
      for (i = 1; i <= NF; i++) {
        if ($i == "idle") {
          value = $(i - 1)
          gsub("%", "", value)
          print value
          exit
        }
      }
    }
  ')"
  [[ -n "$idle" ]] || idle=0
  print -r -- "$idle"
}

run_part() {
  local part="$1"
  local seed="$2"
  local name="6p_pangaea_rush_science_domination_supplemental_part${part}_seed${seed}"
  if /usr/bin/grep -Fq "===== ${name} | exit 0" "$LOG"; then
    printf '===== %s | already complete; skipped =====\n' "$name"
    return
  fi

  local idle=0
  local attempt
  for attempt in {1..4}; do
    idle="$(capacity)"
    if (( idle >= 45.0 )); then
      break
    fi
    printf 'capacity deferral: CPU idle %s%%; retrying in 60s (%d/4)\n' "$idle" "$attempt"
    sleep 60
  done

  if (( idle < 45.0 )); then
    printf '===== %s | deferred: CPU idle %s%% | %s =====\n' \
      "$name" "$idle" "$(date '+%Y-%m-%dT%H:%M:%S%z')"
    return
  fi

  printf '\n===== %s | workers 4 | idle %s%% | %s =====\n' \
    "$name" "$idle" "$(date '+%Y-%m-%dT%H:%M:%S%z')"
  nice -n 20 target/ci/ai_eval advanced_rush_connected advanced \
    --players 6 --width 74 --height 46 --city-states 9 \
    --turns 250 --speed online --map pangaea --shape planet --poles poles --randomize-civs \
    --victories science,domination --difficulty prince \
    --pairs 6 --jobs 4 --seed "$seed"
  local exit_code=$?
  printf '===== %s | exit %s | %s =====\n' \
    "$name" "$exit_code" "$(date '+%Y-%m-%dT%H:%M:%S%z')"
}

cd /Users/martbot-mbp-m5-max-128/civvis-win-estimation-c240 || exit 1
run_part 3 31800012
run_part 4 31800018
