#!/bin/zsh
# Operator launch for the CIVVIS verification-games loop.
#
# Rewritten 2026-08-27. Every override this file used to carry pointed at a
# hand-generated copy in $HOME, and all of them had drifted behind the tracked
# tree by nine days:
#
#   * CIVVIS_SUPERVISOR -> ~/civvis-supervisor-rotating.sh (generated 08-18)
#     still read `STRATEGY=${CIVVIS_STRATEGY:-WildCard9}` and forwarded
#     `--strategy WildCard9`. Named league genomes were RETIRED in #2357 and
#     `civvis-orders` now rejects every --strategy value rather than silently
#     running a different agent, so that copy could not have played a game.
#     It also predates `--restart-below-leader-ratio`, CIVVIS_WITH/WITHOUT and
#     every supervisor fix merged since.
#   * CIVVIS_LADDER_HOST -> ~/civvis-host-tolerant.sh existed only because the
#     tracked host asked gamelock `--hold-status` and so stopped itself at
#     every game boundary on a STANDING hold. The tracked host asks
#     `--halt-status` now; the workaround is obsolete.
#   * CIVVIS_POPUP_KEEPER / CIVVIS_MIRROR_KEEPER / CIVVIS_GAMELOCK pinned the
#     tracked helpers by absolute path only because the host was a $HOME copy
#     that would otherwise look for them beside itself. The tracked host
#     derives them from its own directory correctly.
#
# So this now launches the TRACKED chain and overrides nothing structural. The
# only knobs left are the ones that describe THIS host and the operator's aim.
# The rule the generator warnings kept restating is the whole lesson: a
# hand-maintained copy in $HOME is the drift that stops the ladder.
#
# Opened with `open -g -j -a Terminal` (or started from a Terminal-descended
# shell) so the tree inherits Terminal's TCC grants: App Management to write
# inside Civ6.app, Accessibility to post clicks, Screen Recording to read the
# screen. A launchd job has none of those and every attempt dies at "NO GAME"
# or "cannot install .../DLC/CivvisControl".
set -u

# The tracked tree this plays from. `~/.civvis-play-pin` holds "head" so the
# supervisor fetches and detach-checks-out origin/main before every attempt;
# an absolute path there pins a worktree instead. It was pinned to
# ~/civvis-science-expansion (#2107) until 2026-08-27 — 570 commits behind, so
# every "verification" game validated code nobody was shipping.
export CIVVIS_HEAD_REPO=/Users/martin/CIVVIS

# ⚠⚠ NOT PINNED ANY MORE — THIS PIN WAS GRINDING A SOLVED RUNG.
#
# It read `DIFFICULTY_SETTLER` from 2026-08-18 until 2026-08-28, on the note
# that "the ladder policy already targets it". The policy had long since moved:
#
#   Settler    396 attempts  14 wins  EARNED
#   Chieftain  102 attempts   7 wins  EARNED
#   Warlord     28 attempts   1 win
#   Prince      35 attempts   1 win     <- `civ6_ladder_policy.py target`
#   King        69 attempts   0 wins
#
# A pin cannot notice that it has been overtaken, and this one kept the live
# seat on the easiest rung in the game for ten days after that rung was earned
# three times over. Operator, 2026-08-28: "we've beat level 4 in the past.
# please get us operating at higher levels than this" — level 4 is Prince, and
# Prince is exactly what the policy selects.
#
# Leave this UNSET so `civvis-game-supervisor.sh` asks the policy every batch
# and climbs on its own. Set it only to force one rung for a deliberate arm.

# One game per supervisor cycle, so each game starts from a fresh build of head
# and a merge reaches the next game immediately.
export CIVVIS_PLAY_ATTEMPTS=1

# ⚠ Targeting is a NEGATIVE gate in advanced.rs: an agent aimed at one lane
# prices the others' machinery at -10_000 and abstains from every non-emergency
# World Congress ballot. So the lane is a real choice, and this host had it on
# `science` — the one lane measured never to land. At this exact profile
# (6 players, 250 turns, Online, docs/EVAL.md): victory_eval completion
# diplomatic 14/16, culture 12/16, religious 8/16, domination 2/16, science
# 0/16; the host's own census of 199 real terminal events ranks diplomatic 41 >
# culture 24 > religious 5; and the live ladder at this profile has diplomatic
# n=14 with the only recorded live win and median score 924 against science
# n=9, no wins, median 892. `civ6_play.DEFAULT_CIVVIS_VICTORY` is already
# "diplomatic" for exactly these numbers; the launcher was overriding it.
export CIVVIS_VICTORY=diplomatic

# CIVVIS's mirror gets the UPPER-LEFT quadrant, matching the real game's
# upper-right one (`civ6_civvis_climb` passes --window-side right
# --window-frac 0.5 --window-vfrac 0.5, i.e. x 864..1728, y 33..575 on this
# 1728x1117 logical display). Chrome bounds are {left, top, right, bottom}.
export CIVVIS_MIRROR_BOUNDS="{0, 33, 864, 575}"

# ⚠ NOT SET, deliberately: CIVVIS_STRATEGY (retired in #2357 — the decider
# selects `AdvancedAi::new()` with the gene ledger applied when --strategy is
# absent), CIVVIS_WITH / CIVVIS_WITHOUT (labelled A/B arms, not deployment),
# and CIVVIS_RESTART_BELOW_LEADER_RATIO (the harness's own 0.60 default is the
# operator's rule: at or after turn 150, under 60 % of the leader's score).

exec /bin/zsh /Users/martin/CIVVIS/tools/ops/civvis-ladder-terminal-launcher.sh
