# Summarize completed `ai_eval` shards from the adaptive counterplay log.
# The log remains the source of truth; this is a read-only report view.

BEGIN {
  OFS = "\t"
  print "scenario", "maps", "games", "players", "paired_score", "paired_direction", "terminal_score", "victories"
}

/^===== .* \| workers / {
  active = $2
  maps = games = players = paired = direction = terminal = victories = ""
  collecting_victories = 0
  next
}

/^mirrored head-to-head:/ && active != "" {
  maps = $3
  games = $5
  players = $7
  next
}

/^paired-map score for / && active != "" {
  paired = $5
  next
}

/^paired direction:/ && active != "" {
  direction = $0
  sub(/^paired direction: /, "", direction)
  next
}

/^paired terminal-score diagnostic/ && active != "" {
  terminal = $6
  next
}

/^Victory types:/ && active != "" {
  collecting_victories = 1
  next
}

collecting_victories && /^  [[:alnum:]_]+/ {
  line = $0
  sub(/^  /, "", line)
  victories = victories (victories == "" ? "" : "; ") line
  next
}

collecting_victories && /^$/ {
  collecting_victories = 0
  next
}

/^===== .* \| exit 0 / && active != "" {
  print active, maps, games, players, paired, direction, terminal, victories
  active = ""
  collecting_victories = 0
}
