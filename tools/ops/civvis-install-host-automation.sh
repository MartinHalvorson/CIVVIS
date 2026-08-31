#!/bin/zsh
# civvis-install-host-automation.sh — wire a Mac's verification-game automation
# to THIS tracked tree. Idempotent; run it again after pulling.
#
# What it installs:
#   ~/bin/civvis-games                    -> tools/ops/civvis-games.sh
#   ~/civvis-verification-launch.command  -> tools/ops/civvis-verified-head-launcher.sh
#        the name civvis_collab.py's keeper install looks for; with it present
#        the ladder keeper is re-rendered with `--supervisor <that wrapper>`, so
#        a recovered loop is the operator's loop and not the tree's defaults
#   ~/.civvis-verification-policy         written once if absent, seeded from
#        any CIVVIS_* policy exports in the calling shell (a ~/.zprofile setup
#        migrates itself); see the wrapper for the keys
#   ~/Library/LaunchAgents/com.civvis.keepplaying.plist  `civvis-games ensure` every 5 min
#   ~/Library/LaunchAgents/com.civvis.run-prune.plist    civvis-run-prune.sh hourly
# and retires the pre-repo labels com.martbot.civvis-keepplaying and
# com.martbot.civvis-run-prune (their plists are kept as *.retired-<stamp>).
#
# Symlinks, never copies. A home COPY of a tracked script is the thing
# tools/test_ops_portability.py exists to stop: "a home copy is a dead ladder".
#
#   civvis-install-host-automation.sh [--dry-run] [--no-launchctl]
#                                     [--head-repo PATH] [--replace-wrapper]
#   --head-repo PATH    the tree the games fetch, detach-checkout and build in.
#                       REQUIRED when this tree is attached to branch `main`
#                       (the supervisor detaches the game tree every cycle and
#                       the freshness service needs a `main` worktree).
#   --replace-wrapper   replace an operator's own ~/civvis-verification-launch.command
#                       (a regular file) with the symlink; kept as *.retired-<stamp>
#   --no-launchctl      write files only; no launchctl, no keeper re-render (tests)
#   --dry-run           print what would change, change nothing
set -u
OPS=${0:A:h}
REPO=${OPS:h:h}

DRY_RUN=0; NO_LAUNCHCTL=0; REPLACE_WRAPPER=0; HEAD_REPO_OPT=''
while (( $# )); do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --no-launchctl) NO_LAUNCHCTL=1 ;;
    --replace-wrapper) REPLACE_WRAPPER=1 ;;
    --head-repo) shift; HEAD_REPO_OPT=${1:-} ;;
    -h|--help) sed -n '2,/^set -u$/p' "$0" | sed '$d' | sed -E 's/^# ?//'; exit 0 ;;
    *) print -u2 -r -- "unknown argument: $1 (try --help)"; exit 64 ;;
  esac
  shift
done

AGENTS=${CIVVIS_LAUNCH_AGENTS_DIR:-$HOME/Library/LaunchAgents}
HOME_BIN=${CIVVIS_HOME_BIN:-$HOME/bin}
WRAPPER_LINK=${CIVVIS_LADDER_WRAPPER:-$HOME/civvis-verification-launch.command}
POLICY=${CIVVIS_VERIFICATION_POLICY:-$HOME/.civvis-verification-policy}
RUNS=${CIVVIS_RUNS_DIR:-$HOME/civvis-civ6-runs}
UID_N=$(id -u)
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
NEW_LABELS=(com.civvis.keepplaying com.civvis.run-prune)
OLD_LABELS=(com.martbot.civvis-keepplaying com.martbot.civvis-run-prune)
POLICY_KEYS=(CIVVIS_DIFFICULTY CIVVIS_VICTORY CIVVIS_PLAY_ATTEMPTS
             CIVVIS_RESTART_BELOW_LEADER_RATIO CIVVIS_PLAY_TIMEOUT
             CIVVIS_PLAY_TIMEOUT_CEILING)

say() { print -r -- "$*" }
warn() { print -r -- "  ⚠ $*" }
refuse() { print -u2 -r -- "REFUSING: $*"; exit 64 }
# Print the step, and do it unless this is a dry run.
run() { say "  + $*"; (( DRY_RUN )) || "$@" }

[[ "$(uname -s)" == Darwin ]] || (( NO_LAUNCHCTL )) \
  || refuse "launchd is macOS; pass --no-launchctl to only write the files"
for part in ${(s:/:)REPO}; do
  # A `.civvis-*` root is a state directory some other job rotates and deletes
  # (civvis_collab.ephemeral_service_source). A service installed from one is a
  # service pointed at a directory that is about to vanish.
  [[ "$part" == .civvis-* ]] && refuse "$REPO is an ephemeral tree; install from a durable checkout"
done
for need in civvis-games.sh civvis-verified-head-launcher.sh civvis-run-prune.sh; do
  [[ -x "$OPS/$need" ]] || refuse "$OPS/$need is missing or not executable; is this a CIVVIS tree?"
done
[[ -f "$REPO/Cargo.toml" ]] || refuse "$REPO has no Cargo.toml; is this a CIVVIS tree?"
for label in $NEW_LABELS; do
  [[ -f "$REPO/deploy/$label.plist" ]] || refuse "deploy/$label.plist is missing from $REPO"
done

# --- the game tree ----------------------------------------------------------
branch=$(git -C "$REPO" symbolic-ref --quiet --short HEAD 2>/dev/null || true)
existing_head=''
if [[ -f "$POLICY" ]]; then
  existing_head=$(sed -nE 's/^[[:space:]]*CIVVIS_HEAD_REPO[[:space:]]*=[[:space:]]*([^#[:space:]]+).*/\1/p' "$POLICY" | tail -n 1)
fi
head_repo=${HEAD_REPO_OPT:-$existing_head}
if [[ -n "$head_repo" ]]; then
  [[ -f "$head_repo/Cargo.toml" ]] || refuse "--head-repo $head_repo is not a buildable tree"
  [[ "$(git -C "$head_repo" symbolic-ref --quiet --short HEAD 2>/dev/null || true)" != main ]] \
    || refuse "--head-repo $head_repo is attached to branch main; the supervisor detaches the game tree every cycle — pick a clone or worktree that may sit detached at origin/main"
elif [[ "$branch" == main ]]; then
  refuse "$REPO is attached to branch main. The supervisor detach-checkouts origin/main in the game tree every cycle and civvis_collab's freshness service needs a main worktree, so the games must run in another tree: pass --head-repo <tree>"
fi
if [[ -n "$existing_head" && -n "$HEAD_REPO_OPT" && "$existing_head" != "$HEAD_REPO_OPT" ]]; then
  refuse "$POLICY already sets CIVVIS_HEAD_REPO=$existing_head; edit the policy yourself rather than have an installer change it"
fi

say "== CIVVIS host automation from $REPO =="
(( DRY_RUN )) && say "  (dry run — nothing below is applied)"
[[ -n "$branch" ]] && say "  tree branch: $branch" || say "  tree: detached HEAD $(git -C "$REPO" rev-parse --short HEAD 2>/dev/null || print -r -- '?')"
[[ -n "$head_repo" ]] && say "  game tree:   $head_repo" || say "  game tree:   this tree (the wrapper's default)"

# --- symlinks ----------------------------------------------------------------
retire_regular_file() {   # retire_regular_file <file>  — keep an operator's own copy
  # ⚠ Not `path`: lowercase `path` is zsh's array view of $PATH, and a `local
  # path=...` here silently empties PATH for the rest of the function — `mv`
  # was "command not found" the first time this ran.
  local target=$1
  if [[ -f "$target" && ! -L "$target" ]]; then
    run mv -- "$target" "$target.retired-$STAMP" || refuse "could not retire $target"
    say "    kept your copy as ${target:t}.retired-$STAMP"
  fi
}

say "symlinks:"
run mkdir -p "$HOME_BIN"
retire_regular_file "$HOME_BIN/civvis-games"
run ln -sfn "$OPS/civvis-games.sh" "$HOME_BIN/civvis-games"
[[ ":$PATH:" == *":$HOME_BIN:"* ]] \
  || warn "$HOME_BIN is not on PATH; add \`export PATH=\"$HOME_BIN:\$PATH\"\` to your shell profile"
if [[ -f "$WRAPPER_LINK" && ! -L "$WRAPPER_LINK" && ! $REPLACE_WRAPPER -eq 1 ]]; then
  warn "$WRAPPER_LINK is a file of its own; keeping it (pass --replace-wrapper to link the tracked wrapper instead)"
else
  retire_regular_file "$WRAPPER_LINK"
  run ln -sfn "$OPS/civvis-verified-head-launcher.sh" "$WRAPPER_LINK"
fi

# --- the policy file ---------------------------------------------------------
write_policy() {
  local key seeded=0
  {
    print -r -- "# CIVVIS verification-game policy for this host."
    print -r -- "# Read by tools/ops/civvis-verified-head-launcher.sh at every launch; see its"
    print -r -- "# header for the keys and their defaults. KEY=VALUE, '#' comments."
    print -r -- "# Written by civvis-install-host-automation.sh on $STAMP."
    [[ -n "$head_repo" ]] && print -r -- "CIVVIS_HEAD_REPO=$head_repo"
    for key in $POLICY_KEYS; do
      if [[ -n "${(P)key:-}" ]]; then
        print -r -- "$key=${(P)key}"
        seeded=1
      else
        print -r -- "#$key="
      fi
    done
  } > "$POLICY"
  (( seeded )) && say "    seeded from the CIVVIS_* exports of this shell — check them: $POLICY"
}
say "policy:"
if [[ -f "$POLICY" ]]; then
  if [[ -n "$head_repo" && -z "$existing_head" ]]; then
    say "  + append CIVVIS_HEAD_REPO=$head_repo to $POLICY"
    (( DRY_RUN )) || print -r -- "CIVVIS_HEAD_REPO=$head_repo" >> "$POLICY"
  else
    say "  keeping $POLICY"
  fi
else
  say "  + write $POLICY"
  (( DRY_RUN )) || write_policy
fi

# --- launchd jobs ------------------------------------------------------------
render() { sed -e "s|__HOME__|$HOME|g" -e "s|__OPS__|$OPS|g" "$REPO/deploy/$1.plist" }
loaded() { launchctl print "gui/$UID_N/$1" >/dev/null 2>&1 }

say "launchd:"
run mkdir -p "$AGENTS"
for label in $NEW_LABELS; do
  plist="$AGENTS/$label.plist"
  rendered=$(render "$label")
  if [[ -f "$plist" ]] && [[ "$(<"$plist")" == "$rendered" ]]; then
    say "  $label: plist unchanged"
    changed=0
  else
    say "  + write $plist"
    (( DRY_RUN )) || print -r -- "$rendered" > "$plist"
    changed=1
  fi
  (( NO_LAUNCHCTL )) && continue
  if (( changed )) || ! loaded "$label"; then
    loaded "$label" && run launchctl bootout "gui/$UID_N/$label"
    run launchctl enable "gui/$UID_N/$label"
    run launchctl bootstrap "gui/$UID_N" "$plist"
  fi
  (( DRY_RUN )) || { loaded "$label" && say "  $label: loaded" || warn "$label did not load; try: launchctl bootstrap gui/$UID_N $plist" }
done
for label in $OLD_LABELS; do
  plist="$AGENTS/$label.plist"
  [[ -f "$plist" ]] || continue
  say "  retiring pre-repo job $label"
  (( NO_LAUNCHCTL )) || { loaded "$label" && run launchctl bootout "gui/$UID_N/$label" }
  run mv -- "$plist" "$plist.retired-$STAMP"
done

# --- the ladder keeper -------------------------------------------------------
# civvis_collab.install_ladder_supervisor renders com.civvis.ladder-watchdog and,
# because the wrapper name now exists, hands the keeper `--supervisor <wrapper>`.
say "keeper:"
if (( NO_LAUNCHCTL || DRY_RUN )); then
  say '  skipped (dry run / --no-launchctl); `civvis_collab.py bootstrap` or a rerun of this installer renders it'
elif [[ ! -d "$RUNS" ]]; then
  say "  skipped: $RUNS does not exist yet, so civvis_collab does not consider this host a Civ VI seat (it will after the first game)"
else
  python3 - "$REPO" <<'PY' || warn "keeper re-render failed; run: python3 tools/civvis_collab.py bootstrap"
import sys
from pathlib import Path
repo = Path(sys.argv[1])
sys.path.insert(0, str(repo / "tools"))
import civvis_collab  # noqa: E402
paths = civvis_collab.install_ladder_supervisor(repo)
for path in paths:
    text = path.read_text()
    tag = "names the wrapper" if "--supervisor" in text else "⚠ has no --supervisor"
    print(f"  {path.name}: {tag}")
PY
fi

say ""
say "done. next:"
say "  printf 'head\\n' > ~/.civvis-play-pin      # verification games track origin/main"
say "  civvis-games status                        # what will play, and from where"
say "  civvis-games on                            # start both lanes; watch ~/Library/Logs/civvis-ladder.log"
