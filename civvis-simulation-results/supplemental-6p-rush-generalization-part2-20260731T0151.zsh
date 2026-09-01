#!/bin/zsh
# Second independent shard for the isolated 6-player Pangaea rush generalization.
# It appends only after part 1 completed, remains separate from the primary
# round-robin matrix, and uses the lowest practical CPU priority.

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

idle=0
for attempt in {1..4}; do
  idle="$(capacity)"
  if (( idle >= 45.0 )); then
    break
  fi
  printf 'capacity deferral: CPU idle %s%%; retrying in 60s (%d/4)\n' "$idle" "$attempt"
  sleep 60
done

if (( idle < 45.0 )); then
  printf '===== 6p_pangaea_rush_science_domination_supplemental_part2_seed31800006 | deferred: CPU idle %s%% | %s =====\n' \
    "$idle" "$(date '+%Y-%m-%dT%H:%M:%S%z')"
  exit 0
fi

printf '\n===== 6p_pangaea_rush_science_domination_supplemental_part2_seed31800006 | workers 4 | idle %s%% | %s =====\n' \
  "$idle" "$(date '+%Y-%m-%dT%H:%M:%S%z')"

cd /Users/martbot-mbp-m5-max-128/civvis-win-estimation-c240 || exit 1
nice -n 20 target/ci/ai_eval advanced_rush_connected advanced \
  --players 6 --width 74 --height 46 --city-states 9 \
  --turns 250 --speed online --map pangaea --shape planet --poles poles --randomize-civs \
  --victories science,domination --difficulty prince \
  --pairs 6 --jobs 4 --seed 31800006
exit_code=$?
printf '===== 6p_pangaea_rush_science_domination_supplemental_part2_seed31800006 | exit %s | %s =====\n' \
  "$exit_code" "$(date '+%Y-%m-%dT%H:%M:%S%z')"
exit "$exit_code"
