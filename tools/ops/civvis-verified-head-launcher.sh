#!/bin/zsh
# civvis-verified-head-launcher.sh — the managed entry point for a host's
# continuous CIVVIS verification games.
#
# Every verification game begins from a fresh origin/main build of the GitHub
# CIVVIS, plays the deployment genome (all and only default-on genes), and runs
# under the policy the operator wrote down for THIS host — never under whatever
# a long-lived Terminal window happened to have exported. This file is that
# contract. It applies the policy and hands over to the tracked launcher,
# civvis-ladder-terminal-launcher.sh, which hosts the loop and keeps its log.
#
# Who opens it (always through Terminal, which holds the grants a game needs —
# see ladder_watchdog.py for why launchd cannot):
#   * the ladder keeper, when `civvis_collab.py bootstrap` found the operator
#     wrapper `~/civvis-verification-launch.command` on the host. The installer
#     (civvis-install-host-automation.sh) makes that name a SYMLINK to this
#     file, so a keeper-recovered loop is the operator's loop and not the
#     tree's stock defaults;
#   * `civvis-games on` and `civvis-games ensure`, the same way;
#   * an operator, by hand:  open -g -j -a Terminal ~/civvis-verification-launch.command
#
# ⚠ THE POLICY LIVES IN ONE FILE, ~/.civvis-verification-policy, NOT IN A SHELL
# PROFILE. Before this file existed, the rung, the attempts per cycle and the
# game timeouts were `export`s in one machine's ~/.zprofile — which a Terminal
# login shell happened to source and nothing else could see, share, or
# install. KEY=VALUE lines, `#` comments, and ONLY these keys are honoured:
#
#   CIVVIS_HEAD_REPO                  the tree the games fetch, detach-checkout
#                                     and build in every cycle. Default: the
#                                     tree this file lives in. ⚠ It must not be
#                                     the worktree your `main` branch lives on:
#                                     the supervisor detaches it every cycle,
#                                     and the freshness service needs a `main`.
#   CIVVIS_DIFFICULTY                 DIFFICULTY_SETTLER … DIFFICULTY_DEITY.
#                                     Absent: the read-only ladder policy picks
#                                     the rung, as the stock launcher would.
#   CIVVIS_VICTORY                    the victory lane forwarded to the supervisor.
#   CIVVIS_PLAY_ATTEMPTS              games per cycle. Default 1, so EVERY game
#                                     fetches and builds origin/main afresh
#                                     (operator, 2026-08-21: "use the latest
#                                     version of github civvis code for each
#                                     game"). 3 pins one revision per batch.
#   CIVVIS_RESTART_BELOW_LEADER_RATIO the turn-150 abandon line. Absent: the
#                                     harness's own default (0.60 — operator,
#                                     2026-08-27). 0 plays every game out.
#   CIVVIS_PLAY_TIMEOUT               seconds per game. Default 10800: a
#                                     250-turn game needs the room (a Warlord
#                                     win reached t247 and hit the old 8100 s).
#   CIVVIS_PLAY_TIMEOUT_CEILING       the hard ceiling. Default 14400.
#
# Anything else in the environment that would change WHAT the seat plays — a
# labelled experiment, a retired strategy, an alternate host, a former restart
# policy — is unset here, on purpose, before the launcher runs. That is why a
# stale `export CIVVIS_STRATEGY=g40-37` in a window from last week cannot leave
# the next game stuck before its first city, as it once did.
set -u
# zsh's BG_NICE starts every `&` child at nice +5 and macOS never lowers a nice
# once set, so a demoted Civ VI stays demoted for its whole run. See
# civvis-verification-relaunch.sh and tools/test_ops_background_priority.py.
unsetopt BG_NICE

OPS=${0:A:h}
LOG=${CIVVIS_LADDER_LOG:-$HOME/Library/Logs/civvis-ladder.log}
PIN=${CIVVIS_PINFILE:-$HOME/.civvis-play-pin}
POLICY=${CIVVIS_VERIFICATION_POLICY:-$HOME/.civvis-verification-policy}
# Tests point this at a stub. A host never needs to; the sibling is the launcher.
LAUNCHER=${CIVVIS_LADDER_LAUNCHER:-$OPS/civvis-ladder-terminal-launcher.sh}
mkdir -p "${LOG:h}"

say() { print -r -- "[verified-head] $(date -u +%FT%TZ) $*" >> "$LOG" }

# This wrapper can refuse before it execs the ladder launcher (notably when an
# automatic caller lacks the required verification intent). Terminal keeps
# that rejected document window unless the outermost entry point reaps it.
# Schedule a delayed, title-scoped cleanup rather than closing the current tab:
# a manually typed command returns to a busy normal shell and is left alone.
WINDOW_REAPER_SCHEDULED=0
schedule_idle_window_reap() {
  (( WINDOW_REAPER_SCHEDULED )) && return 0
  WINDOW_REAPER_SCHEDULED=1
  [[ -z ${CIVVIS_LADDER_KEEP_WINDOW:-} ]] || return 0
  local own_tty=${TTY:-}
  [[ "$own_tty" == /dev/tty* ]] || return 0
  (
    /usr/bin/nohup /usr/bin/osascript >>"$LOG" 2>&1 <<'APPLESCRIPT'
delay 1
set reaped to 0
tell application "Terminal"
  repeat with i from (count of windows) to 1 by -1
    try
      set w to item i of windows
      if (busy of w) is false and (((name of w) contains "civvis-ladder-terminal-launcher") or ((name of w) contains "civvis-verified-head-launcher")) then
        close w
        set reaped to reaped + 1
      end if
    end try
  end repeat
end tell
return "window cleanup: reaped " & reaped & " idle managed window(s)"
APPLESCRIPT
  ) &
  say "window cleanup: scheduled idle managed-window reaper"
}
trap 'schedule_idle_window_reap || true' EXIT

# ⚠ A refusal is LOGGED, not merely printed. The window this runs in is
# minimised by the launcher and gone once the shell exits — that is the whole
# point of civvis-ladder-terminal-launcher.sh — so stderr alone would vanish
# and "the keeper keeps opening Terminal and nothing plays" would have no
# explanation on disk.
refuse() {
  say "REFUSING launch: $*"
  print -u2 -r -- "civvis-verified-head-launcher: REFUSING launch: $*"
  exit "${REFUSE_STATUS:-64}"
}

typeset -A policy
policy=(
  CIVVIS_PLAY_ATTEMPTS        1
  CIVVIS_PLAY_TIMEOUT         10800
  CIVVIS_PLAY_TIMEOUT_CEILING 14400
)

if [[ -f "$POLICY" ]]; then
  lineno=0
  while IFS= read -r line || [[ -n "$line" ]]; do
    (( lineno += 1 ))
    line=${line%%'#'*}
    line=${line//[[:space:]]/}
    [[ -z "$line" ]] && continue
    [[ "$line" == *=* ]] || refuse "$POLICY:$lineno is not KEY=VALUE: '$line'"
    key=${line%%=*}
    value=${line#*=}
    case "$key" in
      CIVVIS_HEAD_REPO)
        [[ -f "$value/Cargo.toml" ]] \
          || refuse "$POLICY:$lineno CIVVIS_HEAD_REPO='$value' is not a buildable tree" ;;
      CIVVIS_DIFFICULTY)
        [[ "$value" =~ '^DIFFICULTY_(SETTLER|CHIEFTAIN|WARLORD|PRINCE|KING|EMPEROR|IMMORTAL|DEITY)$' ]] \
          || refuse "$POLICY:$lineno CIVVIS_DIFFICULTY='$value' is not a Civ VI difficulty" ;;
      CIVVIS_VICTORY)
        [[ "$value" =~ '^[a-z][a-z,]*$' ]] \
          || refuse "$POLICY:$lineno CIVVIS_VICTORY='$value' is not a victory lane" ;;
      CIVVIS_PLAY_ATTEMPTS|CIVVIS_PLAY_TIMEOUT|CIVVIS_PLAY_TIMEOUT_CEILING)
        [[ "$value" =~ '^[1-9][0-9]*$' ]] \
          || refuse "$POLICY:$lineno $key='$value' must be a positive integer" ;;
      CIVVIS_RESTART_BELOW_LEADER_RATIO)
        [[ "$value" =~ '^(0|1|0?\.[0-9]+|1\.0+)$' ]] \
          || refuse "$POLICY:$lineno $key='$value' must be a ratio from 0 to 1" ;;
      *)
        say "ignoring unknown policy key '$key' at $POLICY:$lineno (honoured: CIVVIS_HEAD_REPO CIVVIS_DIFFICULTY CIVVIS_VICTORY CIVVIS_PLAY_ATTEMPTS CIVVIS_RESTART_BELOW_LEADER_RATIO CIVVIS_PLAY_TIMEOUT CIVVIS_PLAY_TIMEOUT_CEILING)"
        continue ;;
    esac
    policy[$key]=$value
  done < "$POLICY"
fi

# The game tree is a policy choice, not an environment one: a stale
# `CIVVIS_HEAD_REPO` export naming a reaped worktree is exactly the kind of
# inheritance this file exists to refuse.
HEAD_REPO=${policy[CIVVIS_HEAD_REPO]:-${OPS:h:h}}
unset 'policy[CIVVIS_HEAD_REPO]'

[[ -x "$LAUNCHER" ]] || REFUSE_STATUS=66 refuse "missing tracked launcher $LAUNCHER"
[[ -f "$HEAD_REPO/Cargo.toml" ]] \
  || refuse "no buildable tree at '$HEAD_REPO'; set CIVVIS_HEAD_REPO in $POLICY"
# ⚠ With the pin at `head` the supervisor runs `git checkout --detach
# origin/main` IN the game tree every cycle. Done to the worktree that holds
# the `main` branch, that breaks civvis_collab's freshness service, whose
# main_worktree() finds no `refs/heads/main` and refuses to sync anything.
branch=$(git -C "$HEAD_REPO" symbolic-ref --quiet --short HEAD 2>/dev/null || true)
[[ "$branch" != main ]] \
  || refuse "'$HEAD_REPO' is attached to branch main; the supervisor detach-checkouts origin/main there every cycle and the freshness service needs a main worktree — set CIVVIS_HEAD_REPO in $POLICY to a tree that may sit detached"
pin=$(cat "$PIN" 2>/dev/null || true)
[[ "$pin" == head ]] \
  || refuse "$PIN must contain exactly 'head' (found '${pin:-<absent>}'): verification games track origin/main; \`civvis-games on\` resets the pin"
origin=$(git -C "$HEAD_REPO" remote get-url origin 2>/dev/null || true)
[[ "$origin" =~ '^(https://github\.com/|git@github\.com:|ssh://git@github\.com/)MartinHalvorson/CIVVIS(\.git)?/?$' ]] \
  || refuse "origin of '$HEAD_REPO' is '${origin:-<none>}', not the GitHub CIVVIS; verification games must build what GitHub main holds"

# Never inherit a labelled experiment, a retired strategy, an alternate host,
# or a former restart policy from the window that opened this.
unset CIVVIS_WITH CIVVIS_WITHOUT CIVVIS_WITH_FILE CIVVIS_STRATEGY CIVVIS_VICTORY \
      CIVVIS_DIFFICULTY CIVVIS_PLAY_ATTEMPTS CIVVIS_RESTART_BELOW_LEADER_RATIO \
      CIVVIS_ABANDON_BELOW_WIN_RATE CIVVIS_PLAY_TIMEOUT CIVVIS_PLAY_TIMEOUT_CEILING \
      CIVVIS_HEAD_REPO CIVVIS_LADDER_HOST CIVVIS_LADDER_SUPERVISOR CIVVIS_SUPERVISOR \
      CIVVIS_INTERACTIVE_HOST_LOG CIVVIS_INTERACTIVE_HOST_LOCK

export CIVVIS_HEAD_REPO="$HEAD_REPO"
export CIVVIS_PINFILE="$PIN"
summary=''
for key in ${(ok)policy}; do
  export "$key=${policy[$key]}"
  summary+="$key=${policy[$key]} "
done
if [[ -f "$POLICY" ]]; then
  policy_note="policy $POLICY"
else
  policy_note="no $POLICY — defaults; the rung comes from the ladder policy"
fi
say "launching from $HEAD_REPO (origin/main, pin=head) with ${summary}(${policy_note})"

# The foreground guard rides the same Terminal-descended tree, which is the
# only place it can drive Notification Center and System Settings (see its
# header). Detached, one instance, exits on its own when the lane is gone.
# CIVVIS_FOREGROUND_GUARD=0 skips it; ~/.civvis-foreground-guard-off stops it.
GUARD=${CIVVIS_FOREGROUND_GUARD_SCRIPT:-$OPS/civvis-foreground-guard.sh}
if [[ "${CIVVIS_FOREGROUND_GUARD:-1}" != 0 && -x "$GUARD" ]]; then
  ( /bin/zsh "$GUARD" >/dev/null 2>&1 & )
  say "foreground guard started (${GUARD:t})"
fi

exec /bin/zsh "$LAUNCHER"
