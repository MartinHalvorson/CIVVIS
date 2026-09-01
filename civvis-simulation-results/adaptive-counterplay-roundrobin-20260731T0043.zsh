#!/bin/zsh
# Resource-adaptive, checkpointed counterplay batch with breadth-first ordering.
#
# The scenario/seed matrix is unchanged from adaptive-counterplay-20260731T0015.
# At each round it runs at most one six-map shard from every profile, so a
# constrained overnight window samples every 4-, 6-, and 8-player fight early.
# Completed shard names are shared with the prior queue and are skipped.

set -u

RESULT_DIR=/Users/martbot-mbp-m5-max-128/civvis-simulation-results
MASTER_LOG="$RESULT_DIR/adaptive-counterplay-20260731T0015.log"
CORES="$(sysctl -n hw.ncpu 2>/dev/null || print 8)"

mkdir -p "$RESULT_DIR"
exec >> "$MASTER_LOG" 2>&1

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
  local workers
  workers="$(awk -v cores="$CORES" -v idle="$idle" 'BEGIN {
    jobs = int(cores * idle / 100) - 1
    if (jobs < 1) jobs = 1
    if (jobs > 8) jobs = 8
    print jobs
  }')"
  print -r -- "$idle $workers"
}

wait_for_capacity() {
  local probe idle workers waits=0
  while true; do
    probe="$(capacity)"
    idle="${probe%% *}"
    workers="${probe##* }"
    if (( idle >= 4.0 || waits >= 3 )); then
      print -r -- "$idle $workers"
      return
    fi
    printf 'resource hold: CPU idle %s%%; retrying in 60s (%d/3)\n' "$idle" "$(( waits + 1 ))" >&2
    sleep 60
    (( waits += 1 ))
  done
}

run() {
  local name="$1"
  local workers="$2"
  local idle="$3"
  shift 3
  printf '\n===== %s | workers %s | idle %s%% | %s =====\n' \
    "$name" "$workers" "$idle" "$(date '+%Y-%m-%dT%H:%M:%S%z')"
  # Stay subordinate to already-running collaborator batches even if their
  # workers wake between this boundary probe and the evaluator launch.
  nice -n 19 target/ci/ai_eval "$@"
  local exit_code=$?
  printf '===== %s | exit %s | %s =====\n' \
    "$name" "$exit_code" "$(date '+%Y-%m-%dT%H:%M:%S%z')"
  return 0
}

run_part() {
  local prefix="$1"
  local total_pairs="$2"
  local shard="$3"
  local seed="$4"
  local part_index="$5"
  shift 5
  local offset=$(( (part_index - 1) * shard ))
  (( offset >= total_pairs )) && return
  local count="$shard"
  local remaining=$(( total_pairs - offset ))
  (( remaining < count )) && count="$remaining"
  local name="${prefix}_part${part_index}_seed$(( seed + offset ))"
  if /usr/bin/grep -Fq "===== ${name} | exit 0" "$MASTER_LOG"; then
    printf '===== %s | already complete; skipped =====\n' "$name"
    return
  fi
  local capacity_line idle workers
  capacity_line="$(wait_for_capacity)"
  idle="${capacity_line%% *}"
  workers="${capacity_line##* }"
  run "$name" "$workers" "$idle" "$@" \
    --pairs "$count" --jobs "$workers" --seed "$(( seed + offset ))"
}

cd /Users/martbot-mbp-m5-max-128/civvis-win-estimation-c240 || exit 1
printf '===== adaptive counterplay round-robin queue starts | cores %s | %s =====\n' \
  "$CORES" "$(date '+%Y-%m-%dT%H:%M:%S%z')"

for part_index in {1..15}; do
  run_part 4p_lane_race_science_domination 90 6 31000000 "$part_index" \
    advanced_counter_in_lane advanced \
    --players 4 --width 32 --height 22 --city-states 4 \
    --turns 500 --speed standard --map continents --shape planet --poles poles --randomize-civs \
    --victories science,domination --difficulty prince

  run_part 4p_connected_rush_science_domination 90 6 31100000 "$part_index" \
    advanced_rush_connected advanced \
    --players 4 --width 32 --height 22 --city-states 4 \
    --turns 500 --speed standard --map pangaea --shape planet --poles poles --randomize-civs \
    --victories science,domination --difficulty prince

  run_part 6p_lane_race_deployment 72 6 31200000 "$part_index" \
    advanced_counter_in_lane advanced \
    --players 6 --width 74 --height 46 --city-states 9 \
    --turns 250 --speed online --map continents --shape planet --poles poles --randomize-civs \
    --victories science,culture,domination --difficulty prince

  run_part 6p_congress_targeting 72 6 31300000 "$part_index" \
    advanced_congress_counter advanced \
    --players 6 --width 74 --height 46 --city-states 9 \
    --turns 250 --speed online --map small_continents --shape planet --poles poles --randomize-civs \
    --victories science,culture,domination --difficulty prince

  run_part 6p_early_alarm_build 72 6 31400000 "$part_index" \
    advanced_early_score_build advanced \
    --players 6 --width 74 --height 46 --city-states 9 \
    --turns 250 --speed online --map pangaea --shape planet --poles poles --randomize-civs \
    --victories science,culture,domination --difficulty prince

  run_part 8p_connected_rush_deployment 60 6 31500000 "$part_index" \
    advanced_rush_connected advanced \
    --players 8 --width 84 --height 54 --city-states 12 \
    --turns 250 --speed online --map continents --shape planet --poles poles --randomize-civs \
    --victories science,culture,domination --difficulty prince

  run_part 8p_evolved_denial_ablation 60 6 31600000 "$part_index" \
    advanced_evolved_blind advanced_evolved \
    --players 8 --width 84 --height 54 --city-states 12 \
    --turns 250 --speed online --map islands --shape planet --poles poles --randomize-civs \
    --victories science,culture,domination --difficulty prince

  run_part 8p_congress_hard_vote 60 6 31700000 "$part_index" \
    advanced_congress_counter_hard advanced_congress_counter \
    --players 8 --width 84 --height 54 --city-states 12 \
    --turns 250 --speed online --map small_continents --shape planet --poles poles --randomize-civs \
    --victories science,culture,domination --difficulty prince
done

printf '\n===== adaptive counterplay round-robin queue complete | %s =====\n' \
  "$(date '+%Y-%m-%dT%H:%M:%S%z')"
