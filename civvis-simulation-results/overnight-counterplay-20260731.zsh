#!/bin/zsh
# Low-priority, deterministic counterplay batch. Managed by launchd so it can
# continue overnight without depending on an interactive terminal. Each cell
# is deliberately sharded: a busy host cannot erase hours of unreported work.

run() {
  local name="$1"
  shift
  printf "\n===== %s | %s =====\n" "$name" "$(date "+%Y-%m-%dT%H:%M:%S%z")"
  "$@"
  local status=$?
  printf "===== %s | exit %s | %s =====\n" "$name" "$status" "$(date "+%Y-%m-%dT%H:%M:%S%z")"
  return 0
}

run_shards() {
  local prefix="$1"
  local pairs="$2"
  local shard="$3"
  local seed="$4"
  shift 4
  local offset=0
  local part=1
  while (( offset < pairs )); do
    local count="$shard"
    local remaining=$(( pairs - offset ))
    (( remaining < count )) && count="$remaining"
    run "${prefix}_part${part}_seed$(( seed + offset ))" \
      nice -n 15 target/ci/ai_eval "$@" \
      --pairs "$count" --jobs 3 --seed "$(( seed + offset ))"
    (( offset += count ))
    (( part += 1 ))
  done
}

cd /Users/martbot-mbp-m5-max-128/civvis-win-estimation-c240 || exit 1

# 4-player, direct science/domination stress tests: 180 maps / 360 games.
run_shards 4p_lane_race_science_domination 90 15 31000000 \
  advanced_counter_in_lane advanced \
  --players 4 --width 32 --height 22 --city-states 4 \
  --turns 500 --speed standard --map continents --shape planet --poles poles --randomize-civs \
  --victories science,domination --difficulty prince

run_shards 4p_connected_rush_science_domination 90 15 31100000 \
  advanced_rush_connected advanced \
  --players 4 --width 32 --height 22 --city-states 4 \
  --turns 500 --speed standard --map pangaea --shape planet --poles poles --randomize-civs \
  --victories science,domination --difficulty prince

# Deployment-scale six-player counter-leader treatments: 216 maps / 432 games.
run_shards 6p_lane_race_deployment 72 12 31200000 \
  advanced_counter_in_lane advanced \
  --players 6 --width 74 --height 46 --city-states 9 \
  --turns 250 --speed online --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --difficulty prince

run_shards 6p_congress_targeting 72 12 31300000 \
  advanced_congress_counter advanced \
  --players 6 --width 74 --height 46 --city-states 9 \
  --turns 250 --speed online --map small_continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --difficulty prince

run_shards 6p_early_alarm_build 72 12 31400000 \
  advanced_early_score_build advanced \
  --players 6 --width 74 --height 46 --city-states 9 \
  --turns 250 --speed online --map pangaea --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --difficulty prince

# Eight-player full-table tests: 180 maps / 360 games.
run_shards 8p_connected_rush_deployment 60 10 31500000 \
  advanced_rush_connected advanced \
  --players 8 --width 84 --height 54 --city-states 12 \
  --turns 250 --speed online --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --difficulty prince

run_shards 8p_evolved_denial_ablation 60 10 31600000 \
  advanced_evolved_blind advanced_evolved \
  --players 8 --width 84 --height 54 --city-states 12 \
  --turns 250 --speed online --map islands --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --difficulty prince

run_shards 8p_congress_hard_vote 60 10 31700000 \
  advanced_congress_counter_hard advanced_congress_counter \
  --players 8 --width 84 --height 54 --city-states 12 \
  --turns 250 --speed online --map small_continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --difficulty prince

printf "\n===== overnight counterplay queue complete | %s =====\n" "$(date "+%Y-%m-%dT%H:%M:%S%z")"
