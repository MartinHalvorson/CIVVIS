# Pool completed `ai_eval` shards from the adaptive counterplay log by scenario.
#
# This reads only terminal shard records (the master log remains authoritative),
# so an in-flight or failed shard is never counted.  It uses only POSIX awk
# features because the shared macOS host provides the BSD implementation.

BEGIN {
  OFS = "\t"
  print "scenario", "shards", "maps", "games", "players", "game_wins_treatment/control", "paired_score", "win_direction_treatment/neutral/control", "terminal_score", "terminal_direction_treatment/neutral/control", "rush_seat_games_treatment", "rush_player_turns_treatment"
}

function clear_active() {
  active = base = ""
  maps = games = players = 0
  treatment_wins = control_wins = 0
  paired_score = terminal_score = ""
  favorable = neutral = adverse = 0
  terminal_favorable = terminal_neutral = terminal_adverse = 0
  collecting_rush = rush_lines = 0
  rush_seat_games = rush_seat_total = 0
  rush_turns = rush_turn_total = 0
}

function numeric_percent(value) {
  sub(/%$/, "", value)
  return value + 0
}

function record_active(  b) {
  if (active == "" || seen[active]++) {
    clear_active()
    return
  }

  b = base
  if (!(b in order)) {
    order[b] = ++scenario_count
    scenarios[scenario_count] = b
  }
  shard_total[b]++
  map_total[b] += maps
  game_total[b] += games
  player_value[b] = players
  treatment_win_total[b] += treatment_wins
  control_win_total[b] += control_wins
  paired_score_total[b] += paired_score * maps
  terminal_score_total[b] += terminal_score * maps
  favorable_total[b] += favorable
  neutral_total[b] += neutral
  adverse_total[b] += adverse
  terminal_favorable_total[b] += terminal_favorable
  terminal_neutral_total[b] += terminal_neutral
  terminal_adverse_total[b] += terminal_adverse
  rush_seat_games_total[b] += rush_seat_games
  rush_seat_total_total[b] += rush_seat_total
  rush_turns_total[b] += rush_turns
  rush_turn_total_total[b] += rush_turn_total
  clear_active()
}

/^===== .* \| workers / {
  clear_active()
  active = $2
  base = active
  sub(/_part[0-9]+_seed[0-9]+$/, "", base)
  next
}

/^mirrored head-to-head:/ && active != "" {
  maps = $3 + 0
  games = $5 + 0
  players = $7 + 0
  next
}

/^game-win share:/ && active != "" {
  found = 0
  for (i = 1; i <= NF; i++) {
    if ($i ~ /^[0-9]+\/[0-9]+$/) {
      split($i, wins, "/")
      if (found == 0) treatment_wins = wins[1] + 0
      else if (found == 1) control_wins = wins[1] + 0
      found++
    }
  }
  next
}

/^paired-map score for / && active != "" {
  paired_score = numeric_percent($5)
  next
}

/^paired direction:/ && active != "" {
  line = $0
  gsub(/[,;]/, "", line)
  split(line, fields, " ")
  favorable = fields[4] + 0
  neutral = fields[6] + 0
  adverse = fields[8] + 0
  next
}

/^paired terminal-score diagnostic/ && active != "" {
  terminal_score = numeric_percent($6)
  next
}

/^terminal-score direction:/ && active != "" {
  line = $0
  gsub(/[,;]/, "", line)
  split(line, fields, " ")
  terminal_favorable = fields[4] + 0
  terminal_neutral = fields[6] + 0
  terminal_adverse = fields[8] + 0
  next
}

/^Ancient-rush treatment exposure:/ && active != "" {
  collecting_rush = 1
  rush_lines = 0
  next
}

collecting_rush && /^  [[:alnum:]_]+ [0-9]+\/[0-9]+ seat-games ever rushed/ {
  rush_lines++
  if (rush_lines == 1) {
    split($2, seat_parts, "/")
    split($7, turn_parts, "/")
    rush_seat_games = seat_parts[1] + 0
    rush_seat_total = seat_parts[2] + 0
    rush_turns = turn_parts[1] + 0
    rush_turn_total = turn_parts[2] + 0
  }
  if (rush_lines >= 2) collecting_rush = 0
  next
}

/^===== .* \| exit 0 / && active != "" {
  record_active()
}

END {
  for (i = 1; i <= scenario_count; i++) {
    b = scenarios[i]
    maps = map_total[b]
    printf "%s\t%d\t%d\t%d\t%d\t%d/%d\t%.1f%%\t%d/%d/%d\t%.1f%%\t%d/%d/%d\t%d/%d\t%d/%d\n", \
      b, shard_total[b], maps, game_total[b], player_value[b], \
      treatment_win_total[b], control_win_total[b], \
      paired_score_total[b] / maps, favorable_total[b], neutral_total[b], adverse_total[b], \
      terminal_score_total[b] / maps, terminal_favorable_total[b], terminal_neutral_total[b], terminal_adverse_total[b], \
      rush_seat_games_total[b], rush_seat_total_total[b], rush_turns_total[b], rush_turn_total_total[b]
  }
}
