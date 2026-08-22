#!/bin/zsh
# Operator launch for the CIVVIS verification-games loop (2026-08-18).
#
# Opened with `open -g -j -a Terminal` so the whole tree is Terminal-descended
# and inherits Terminal's TCC grants (App Management to write inside Civ6.app,
# Accessibility to post clicks, Screen Recording to read the screen). A launchd
# job or a non-Terminal shell has none of those and every attempt dies at
# "NO GAME" or "cannot install .../DLC/CivvisControl".
set -u

# Settler, per the operator. The ladder policy already targets it; pinning
# makes it explicit rather than policy-dependent.
export CIVVIS_DIFFICULTY=DIFFICULTY_SETTLER

# One game per supervisor cycle, so the lane rotation below advances per game.
export CIVVIS_PLAY_ATTEMPTS=1

# Stock live configuration. Not changed alongside the rest of this launch.
export CIVVIS_STRATEGY=WildCard9

# CIVVIS's mirror gets the UPPER-LEFT quadrant, matching the real game's
# upper-right one (`civ6_civvis_climb` passes --window-side right
# --window-frac 0.5 --window-vfrac 0.5, i.e. x 864..1728, y 33..575 on this
# 1728x1117 logical display). Chrome bounds are {left, top, right, bottom}.
export CIVVIS_MIRROR_BOUNDS="{0, 33, 864, 575}"

# ⚠⚠ A supervisor copy outside tools/ops/ derives HEAD_REPO=${0:A:h:h:h} as
# $HOME and then builds in `/`. The supervisor's own guard names this override;
# it is REQUIRED with CIVVIS_SUPERVISOR below.
export CIVVIS_HEAD_REPO=/Users/martin/CIVVIS

# A different victory condition each game, per the operator. The tracked
# supervisor reads its lane once at startup, so the rotation lives in a
# generated copy -- regenerate with ~/civvis-make-rotating-supervisor.sh
# whenever the tracked supervisor moves.
export CIVVIS_SUPERVISOR=$HOME/civvis-supervisor-rotating.sh
# ⚠ PINNED, NOT ROTATING — operator asked to "hardcore focus on winning"
# (2026-08-19 ~01:30Z). Rotation deliberately trades win rate for lane variety;
# a single-entry list makes the generated rotation block pick the same lane
# every cycle. Live ladder evidence at this profile (finished games only):
#   diplomatic n=14  1 win (7.1%)  median score 924  median gap -228
#   science    n= 9  0 wins        median score 892  median gap -192
#   religious  n= 1  0 wins        median score 554  median gap -921
#   unset      n=229 8 wins (3.5%) median score 394  median gap -204
# Diplomatic is the only targeted lane with a recorded live win and the highest
# median score, and victory_eval put it 14/16 at this exact profile (science
# 0/16), which is why it is `civ6_play.DEFAULT_CIVVIS_VICTORY`.
# Restore variety by listing several lanes again.
export CIVVIS_VICTORY_LANES="science"   # fallback only; ~/.civvis-victory-lanes wins

# ⚠⚠⚠ The tracked host EXITS AT EVERY GAME BOUNDARY: a game ending leaves the
# gamelock briefly held with no harness behind it (a STANDING hold), and
# `gamelock.py --hold-status` exits 0 for that exactly as it does for a real
# operator halt. Measured 22:56:46Z and 00:33:13Z on 2026-08-18/19 with no halt
# marker on disk; `com.civvis.ladder-watchdog` then relaunched a STOCK tree
# through Terminal, losing every setting here. Use the generated tolerant host,
# which stops on an EXPLICIT halt only. Regenerate with
# ~/civvis-make-rotating-host.sh whenever the tracked host moves.
export CIVVIS_LADDER_HOST=$HOME/civvis-host-tolerant.sh

# ⚠ The host derives its helpers from its OWN directory (${0:A:h}), so a copy in
# $HOME would look for them in $HOME. Pin every one to the tracked tree.
export CIVVIS_POPUP_KEEPER=/Users/martin/CIVVIS/tools/ops/civvis-popup-keeper.sh
export CIVVIS_MIRROR_KEEPER=/Users/martin/CIVVIS/tools/ops/civvis-mirror-keeper.sh
export CIVVIS_GAMELOCK=/Users/martin/CIVVIS/tools/civ6_control/gamelock.py

exec /bin/zsh /Users/martin/CIVVIS/tools/ops/civvis-ladder-terminal-launcher.sh
