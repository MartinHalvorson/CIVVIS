#!/bin/zsh
# Run the civvis.ai exhibition supervisor, and keep running it.
#
# This is the versioned twin of the hand-written `run-civvis-spectator.sh` that
# lived in one operator's home directory. Two things made that a liability, and
# both cost an outage on 2026-08-18:
#
#   * it named `/Users/<somebody>` throughout, so it existed on exactly one
#     machine and could not be installed as a service anywhere;
#   * nothing supervised it. When the supervisor exited, the exhibition stayed
#     down until a person noticed — and what actually happened was worse: the
#     worktree it execs its supervisor from was deleted by
#     `civvis_worktree_audit.py --reap`, so its own restart loop could not
#     recover either.
#
# ⚠ THIS DOES NOT NEED TERMINAL, AND THAT IS WORTH SAYING BECAUSE ITS SIBLING
# DOES. `civvis-game-supervisor.sh` must be started through Terminal, because
# installing the Civilization VI control mod writes inside `Civ6.app` and macOS
# grants that permission to the responsible process. This one drives no GUI —
# it passes `--no-open` and only builds, serves HTTP and plays headless games —
# so a plain LaunchAgent is enough, and running under launchd is exactly how it
# is running today.
#
# Paths derive rather than being typed:
#   CIVVIS_DEPLOY_ROOT    the checkout that owns binaries, checkpoints and logs
#                         (default: the repository this script lives in)
#   CIVVIS_SPECTATOR_SRC  the detached worktree holding the canonical supervisor,
#                         which self-updates by `os.execv` when it changes on
#                         origin/main (default: $HOME/civvis-spectator-src, the
#                         layout docs/SPECTATOR_DEPLOY.md prescribes)

set -u

deploy_root=${CIVVIS_DEPLOY_ROOT:-${0:A:h:h:h}}
spectator_src=${CIVVIS_SPECTATOR_SRC:-$HOME/civvis-spectator-src}
supervisor="$spectator_src/tools/spectator_supervisor.py"
supervisor_pid=""
stopping=0

export CIVVIS_DEPLOY_ROOT="$deploy_root"

if [[ ! -r "$supervisor" ]]; then
  print -r -- "no spectator supervisor at $supervisor" >&2
  print -r -- "create the source worktree as docs/SPECTATOR_DEPLOY.md describes:" >&2
  print -r -- "  git -C $deploy_root worktree add --detach $spectator_src origin/main" >&2
  exit 78   # EX_CONFIG: a missing prerequisite, not a crash to restart into
fi

stop_spectator() {
  stopping=1
  trap - HUP INT TERM EXIT
  if [[ -n "$supervisor_pid" ]] && kill -0 "$supervisor_pid" 2>/dev/null; then
    kill -INT "$supervisor_pid" 2>/dev/null || true
    for _ in {1..70}; do
      kill -0 "$supervisor_pid" 2>/dev/null || break
      sleep 0.1
    done
    kill -0 "$supervisor_pid" 2>/dev/null && kill -TERM "$supervisor_pid" 2>/dev/null
    wait "$supervisor_pid" 2>/dev/null || true
  fi
  exit 0
}

trap stop_spectator HUP INT TERM EXIT
cd "$deploy_root" || exit 1

while (( ! stopping )); do
  # HUP is ignored only in the child, so this wrapper can translate a hangup
  # into SIGINT and exercise the supervisor's clean shutdown, which also stops
  # the game server it is running.
  (
    trap - EXIT INT TERM
    trap '' HUP
    exec /usr/bin/python3 "$supervisor" \
      --port "${CIVVIS_SPECTATOR_PORT:-8766}" \
      --players 8 \
      --width 84 \
      --height 54 \
      --city-states 12 \
      --turns 250 \
      --map lakes \
      --poles randomized \
      --speed online \
      --victories science,culture,domination \
      --no-open
  ) &
  supervisor_pid=$!
  wait "$supervisor_pid"
  rc=$?
  supervisor_pid=""
  (( stopping )) && break
  print -r -- "spectator supervisor exited with status ${rc}; restarting in 2s"
  sleep 2
done
