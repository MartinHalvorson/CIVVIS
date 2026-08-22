#!/usr/bin/env bash
#
# One turn of the CIVVIS build/simulate loop.
#
# The engine ships as two builds — the native release binary and the wasm32
# module behind civvis.ai/beta — and this alternates them, rebuilding fresh and
# playing a full headless game on each. A rust/wasm pair shares one seed, so
# the two builds are asked for the same game and their answers can be put side
# by side.
#
# It runs from its own detached worktree. $HOME/civvis (== CIVVIS on
# this case-insensitive volume) is where live Civ 6 games take their
# `civvis_orders` binary from; rebuilding there swaps the program out from
# under a running measurement.
#
#   ./iterate.sh            play the arm whose turn it is
#   ./iterate.sh rust|wasm  force an arm (does not disturb the alternation)

set -uo pipefail

TREE=${CIVVIS_SIMLOOP_TREE:-$HOME/civvis-simloop}
LOGS=${CIVVIS_SIMLOOP_LOGS:-$HOME/civvis-simloop-logs}
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"

# One at a time. `mkdir` is the atomic primitive macOS has without flock(1);
# a stale directory from a killed run is cleared by hand, deliberately, so a
# crash is visible rather than silently overwritten by the next fire.
LOCK="$LOGS/.running"
if ! mkdir "$LOCK" 2>/dev/null; then
  echo "an iteration is already running (holder: $(cat "$LOCK/pid" 2>/dev/null || echo unknown))" >&2
  exit 75
fi
echo $$ > "$LOCK/pid"
trap 'rm -rf "$LOCK"' EXIT

STATE="$LOGS/state.env"
ITER=0; NEXT_ARM=rust; SEED_BASE=1000; FRESH_SEED=1100
# shellcheck source=/dev/null
[ -f "$STATE" ] && . "$STATE"
ITER=$((ITER + 1))
ARM="${1:-$NEXT_ARM}"

# One configuration per pair, rotating. Seventeen iterations of six-on-
# continents exercised one slice of the engine over and over; the sphere's whole
# geometry had never been built a world on. The pair index selects it, so both
# arms play the same board and the parity check still means something.
PAIR=$(( (ITER - 1) / 2 ))
eval "$(python3 - "$LOGS/configs.json" "$PAIR" <<'PY'
import json, sys, shlex
configs = json.load(open(sys.argv[1]))
c = configs[int(sys.argv[2]) % len(configs)]
for key in ("name", "players", "speed", "difficulty", "map_script", "map_topology",
            "map_poles", "start_era", "benchmark_seed"):
    print(f"CONFIG_{key.upper()}={shlex.quote(str(c[key]))}")
print(f"CONFIG_COUNT={len(configs)}")
PY
)"
PLAYERS="$CONFIG_PLAYERS"
SPEED="$CONFIG_SPEED"
DIFFICULTY="$CONFIG_DIFFICULTY"

# Every other lap of the config rotation replays that board's benchmark seed
# instead of a fresh one.
#
# The throughput check that actually means something is "same seed, same board,
# newer revision" — anything else is comparing two different games. But the seed
# used to advance with every pair, so no seed was ever played twice and that
# check could never fire; only the noisy fallback ever ran. A recurring seed per
# board gives it something to compare against, on whatever revision `main` is on
# by then.
#
# The globe's benchmark is deliberately 1037, a seed already known to diverge
# (#1061), so a fix to it announces itself here without anyone remembering to
# look.
# `CONFIG_COUNT`, not a literal: this alternates benchmark and fresh laps a
# whole rotation at a time, so it has to advance in step with the roster. Left
# hardcoded at 6, adding two boards would have desynchronised the two cycles and
# some board would have drawn a benchmark seed every single lap while another
# never drew one at all.
if [ $(( (PAIR / CONFIG_COUNT) % 2 )) -eq 0 ]; then
  SEED="$CONFIG_BENCHMARK_SEED"
  SEED_KIND=benchmark
else
  # A pair shares one seed, so it is claimed by the rust arm and read back by
  # the wasm one rather than advancing twice.
  if [ "$ARM" = rust ]; then
    SEED="$FRESH_SEED"
    FRESH_SEED=$(( FRESH_SEED + 1 ))
  else
    SEED=$(( FRESH_SEED - 1 ))
  fi
  SEED_KIND=fresh
fi

# Track `main`, but only ever at a pair boundary. The rust arm opens a seed and
# the wasm arm closes it, and the parity check is only meaningful when both
# played the same program — a merge landing between the two halves would be
# reported as the builds disagreeing. `rust` is the pair's first arm, so that is
# where the tree is allowed to move.
if [ "$ARM" = rust ]; then
  git -C "$TREE" fetch -q origin main 2>/dev/null
  target="$(git -C "$TREE" rev-parse origin/main 2>/dev/null)"
  if [ -n "$target" ] && [ "$target" != "$(git -C "$TREE" rev-parse HEAD)" ]; then
    echo "   tree: $(git -C "$TREE" rev-parse --short HEAD) -> $(git -C "$TREE" rev-parse --short origin/main)"
    git -C "$TREE" checkout -q --detach "$target" 2>/dev/null \
      || echo "   ⚠ could not advance the runner tree; staying where it is"
  fi
fi

SHA="$(git -C "$TREE" rev-parse --short HEAD)"
STAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
TAG="$(printf '%03d' "$ITER")-$ARM"
mkdir -p "$LOGS/runs"
BUILD_LOG="$LOGS/runs/$TAG.build.log"
SIM_LOG="$LOGS/runs/$TAG.sim.log"
RESULT="$LOGS/runs/$TAG.json"

echo "== iteration $ITER · arm=$ARM · seed=$SEED ($SEED_KIND) · cfg=$CONFIG_NAME · sha=$SHA · $STAMP"

# ------------------------------------------------------------------ the build
build_started=$(date +%s)
WASM_RAW="$TREE/target/wasm32-unknown-unknown/release/civvis.wasm"
WASM_SHIPPED="$LOGS/runs/$TAG.wasm"
if [ "$ARM" = rust ]; then
  ( cd "$TREE" && nice -n 5 cargo build --release --bin civvis ) > "$BUILD_LOG" 2>&1
  build_status=$?
else
  # Stamped the way `beta/publish.sh` stamps it, minus one. On wasm the
  # revision is an `option_env!` read at compile time, so a module built
  # without these can never say which program it is — and an unidentifiable
  # module is no use in a ledger that exists to compare builds.
  #
  # `CIVVIS_BUILT_AT` is deliberately *not* passed. `option_env!` is a build
  # input, so a value that changes every iteration invalidates the crate every
  # iteration: measured at 2m10s for a rebuild that, with these two stable
  # stamps, cargo finishes in 0.01s. A publication wants the wall-clock; a loop
  # rebuilding one revision over and over wants to be told there is nothing to
  # do. The commit and its date still move whenever the revision does, which is
  # exactly when the rebuild is real.
  ( cd "$TREE" && CIVVIS_COMMIT="$(git -C "$TREE" rev-parse HEAD)" \
      CIVVIS_COMMIT_TIME="$(git -C "$TREE" show -s --format=%cI HEAD)" \
      nice -n 5 cargo rustc --lib --target wasm32-unknown-unknown \
      --release --crate-type cdylib ) > "$BUILD_LOG" 2>&1
  build_status=$?
  # The published module is the shrunk one, so that is what gets played. A raw
  # module measures a build nobody is served.
  if [ "$build_status" -eq 0 ] && command -v wasm-opt >/dev/null; then
    nice -n 5 wasm-opt -O3 "$WASM_RAW" -o "$WASM_SHIPPED" >> "$BUILD_LOG" 2>&1 \
      || { echo "wasm-opt failed; playing the unshrunk module" >> "$BUILD_LOG"; \
           cp "$WASM_RAW" "$WASM_SHIPPED"; }
  elif [ "$build_status" -eq 0 ]; then
    cp "$WASM_RAW" "$WASM_SHIPPED"
  fi
fi
build_seconds=$(( $(date +%s) - build_started ))
# Cargo's own "generated N warnings" tally starts with `warning:` too, so a
# naive count reports one more warning than the compiler found.
warnings=$(grep '^warning' "$BUILD_LOG" 2>/dev/null | grep -cv 'generated .* warning' || true)
echo "   build: status=$build_status ${build_seconds}s warnings=$warnings"

if [ "$build_status" -ne 0 ]; then
  python3 "$LOGS/record.py" --result "$RESULT" --ledger "$LOGS/ledger.jsonl" \
    --iteration "$ITER" --arm "$ARM" --seed "$SEED" --sha "$SHA" --stamp "$STAMP" \
    --build-seconds "$build_seconds" --build-warnings "$warnings" \
    --failed "the build did not complete" --build-log "$BUILD_LOG"
  # The arm still advances: a broken build is this iteration's result, and the
  # next fire should try the other build rather than re-running the same
  # failure until somebody looks.
  { echo "ITER=$ITER"; echo "NEXT_ARM=$([ "$ARM" = rust ] && echo wasm || echo rust)";
    echo "SEED_BASE=$SEED_BASE"; echo "FRESH_SEED=$FRESH_SEED"; } > "$STATE"
  exit 1
fi

# ------------------------------------------------------------------- the game
sim_started=$(date +%s)
if [ "$ARM" = rust ]; then
  # The native arm is driven through the same routes as the wasm one rather
  # than through `simulate`. `simulate` builds its world from `GameOptions` and
  # the module builds its from `Params`; two different constructors from one
  # seed are two different games, and comparing them reports a divergence every
  # single time. Through `/new` and `/step` both arms reach the same
  # `Session`, so a difference is the build.
  #
  # A port of its own: 8765 is where a spectator someone is watching lives.
  # Since #1094 the module seats its AI from the league snapshot compiled into
  # it, and a bare `civvis play` seats `advanced` — so the two builds played the
  # same world with different agents and the pair stopped being comparable.
  # `--league <dir>` gives the native side the same roster *and* sets
  # `seat_from_roster`, which is exactly what `browser_session` does. Verified:
  # both then seat `g60-51` on seed 1034.
  #
  # From a copy, not from the tree: a server pointed at a league directory can
  # write lock and marker files into it, and the runner tree is a checkout that
  # must stay clean. Refreshed every iteration because the roster is versioned
  # with the code, and the module has its copy baked in at compile time — they
  # have to be the same revision's roster or the seating diverges again.
  # A stale roster is worse than none: it would seat a different agent than the
  # module compiled from this revision, which is the very divergence this is
  # here to remove. So the copy is all-or-nothing, and a revision without the
  # file falls back to no league at all and says so.
  LEAGUE_DIR="$LOGS/.league"
  LEAGUE_ARGS=()
  rm -rf "$LEAGUE_DIR"
  if [ -f "$TREE/data/league/league.json" ]; then
    mkdir -p "$LEAGUE_DIR"
    if cp "$TREE/data/league/league.json" "$LEAGUE_DIR/league.json" 2>/dev/null; then
      LEAGUE_ARGS=(--league "$LEAGUE_DIR")
    fi
  fi
  [ ${#LEAGUE_ARGS[@]} -eq 0 ] && echo "   ⚠ no league roster at this revision; the arms will seat differently"

  PORT=$(( 18700 + ITER % 200 ))
  # Native reads its revision from the launch environment, wasm bakes it in at
  # compile time. Both arms should be able to say which program they are.
  CIVVIS_COMMIT="$(git -C "$TREE" rev-parse HEAD)" \
  CIVVIS_COMMIT_TIME="$(git -C "$TREE" show -s --format=%cI HEAD)" \
  nice -n 5 "$TREE/target/release/civvis" play --no-open --port "$PORT" \
    --players "$PLAYERS" --seed "$SEED" --speed "$SPEED" \
    --difficulty "$DIFFICULTY" --map "$CONFIG_MAP_SCRIPT" \
    --shape "$CONFIG_MAP_TOPOLOGY" --poles "$CONFIG_MAP_POLES" \
    "${LEAGUE_ARGS[@]}" \
    --spectate --paused > "$SIM_LOG.server" 2>&1 &
  server_pid=$!
  trap 'kill '"$server_pid"' 2>/dev/null; rm -rf "$LOCK"' EXIT
  for _ in $(seq 1 60); do
    curl -sf -m 2 "http://127.0.0.1:$PORT/status" >/dev/null 2>&1 && break
    /bin/sleep 0.5
  done
  nice -n 5 node "$LOGS/sim.mjs" --arm rust --base "http://127.0.0.1:$PORT" \
    --seed "$SEED" --players "$PLAYERS" --speed "$SPEED" \
    --difficulty "$DIFFICULTY" --map-script "$CONFIG_MAP_SCRIPT" \
    --map-topology "$CONFIG_MAP_TOPOLOGY" --map-poles "$CONFIG_MAP_POLES" --start-era "$CONFIG_START_ERA" \
    > "$SIM_LOG" 2>&1
  sim_status=$?
  # Before the kill, while the process still exists: what the engine actually
  # spent, as opposed to how long the wall clock took on a shared box.
  server_cpu="$(ps -o time= -p "$server_pid" 2>/dev/null | tr -d ' ')"
  cpu_seconds="$(python3 -c "
import sys
raw = sys.argv[1] if len(sys.argv) > 1 else ''
try:
    days, _, rest = raw.rpartition('-')
    parts = [float(x) for x in rest.split(':')]
    total = 0.0
    for part in parts:
        total = total * 60 + part
    print(round(total + (float(days) * 86400 if days else 0), 3))
except Exception:
    print(0)
" "$server_cpu")"
  kill "$server_pid" 2>/dev/null
  wait "$server_pid" 2>/dev/null
  trap 'rm -rf "$LOCK"' EXIT
else
  nice -n 5 node "$LOGS/sim.mjs" --arm wasm --wasm "$WASM_SHIPPED" \
    --seed "$SEED" --players "$PLAYERS" --speed "$SPEED" \
    --difficulty "$DIFFICULTY" --map-script "$CONFIG_MAP_SCRIPT" \
    --map-topology "$CONFIG_MAP_TOPOLOGY" --map-poles "$CONFIG_MAP_POLES" --start-era "$CONFIG_START_ERA" \
    > "$SIM_LOG" 2>&1
  sim_status=$?
fi
sim_seconds=$(( $(date +%s) - sim_started ))
load_now="$(uptime | sed -E 's/.*averages?: ([0-9.]+).*/\1/')"
echo "   sim:   status=$sim_status ${sim_seconds}s"

# ------------------------------------------------- is one build even repeatable
#
# Everything this loop concludes rests on each build being deterministic: if a
# build did not reproduce itself, a "divergence" could be that rather than a
# difference between the builds, and an identical pair could be luck. That
# assumption had never been tested, so once per lap of the rotation the same
# arm replays the same seed and the two runs are compared.
#
# One board, on benchmark laps only — about one iteration in twelve. A check
# nobody can afford to run does not get run.
REPEAT_LOG=""
if [ "$sim_status" -eq 0 ] && [ "$CONFIG_NAME" = baseline ] && [ "$SEED_KIND" = benchmark ]; then
  REPEAT_LOG="$SIM_LOG.repeat"
  echo "   repeat: replaying seed $SEED on the same build"
  if [ "$ARM" = rust ]; then
    RPORT=$(( PORT + 400 ))
    nice -n 5 "$TREE/target/release/civvis" play --no-open --port "$RPORT" \
      --players "$PLAYERS" --seed "$SEED" --speed "$SPEED" \
      --difficulty "$DIFFICULTY" --map "$CONFIG_MAP_SCRIPT" \
      --shape "$CONFIG_MAP_TOPOLOGY" --poles "$CONFIG_MAP_POLES" \
      "${LEAGUE_ARGS[@]}" \
      --spectate --paused > "$REPEAT_LOG.server" 2>&1 &
    repeat_pid=$!
    trap 'kill '"$repeat_pid"' 2>/dev/null; rm -rf "$LOCK"' EXIT
    for _ in $(seq 1 60); do
      curl -sf -m 2 "http://127.0.0.1:$RPORT/status" >/dev/null 2>&1 && break
      /bin/sleep 0.5
    done
    nice -n 5 node "$LOGS/sim.mjs" --arm rust --base "http://127.0.0.1:$RPORT" \
      --seed "$SEED" --players "$PLAYERS" --speed "$SPEED" \
      --difficulty "$DIFFICULTY" --map-script "$CONFIG_MAP_SCRIPT" \
      --map-topology "$CONFIG_MAP_TOPOLOGY" --map-poles "$CONFIG_MAP_POLES" --start-era "$CONFIG_START_ERA" \
      > "$REPEAT_LOG" 2>&1
    kill "$repeat_pid" 2>/dev/null; wait "$repeat_pid" 2>/dev/null
    trap 'rm -rf "$LOCK"' EXIT
  else
    nice -n 5 node "$LOGS/sim.mjs" --arm wasm --wasm "$WASM_SHIPPED" \
      --seed "$SEED" --players "$PLAYERS" --speed "$SPEED" \
      --difficulty "$DIFFICULTY" --map-script "$CONFIG_MAP_SCRIPT" \
      --map-topology "$CONFIG_MAP_TOPOLOGY" --map-poles "$CONFIG_MAP_POLES" --start-era "$CONFIG_START_ERA" \
      > "$REPEAT_LOG" 2>&1
  fi
fi

raw_bytes=0
[ "$ARM" = wasm ] && [ -f "$WASM_RAW" ] && raw_bytes=$(wc -c < "$WASM_RAW" | tr -d ' ')

# ------------------------------------------------- can the site still be built
#
# `beta/publish.sh` is the only thing that assembles what civvis.ai actually
# serves, and **nothing gates it**. The `published-build` CI job runs
# `cargo check --lib --target wasm32` and stops there; the workflow that runs
# the real script is `workflow_dispatch` only, deliberately, because publishing
# is a judgement call. So the assembly half — the viewer copy, its asset
# rewrites, the "every atlas the page asks for must ship" check and the 25 MiB
# bundle budget — is exercised for the first time when somebody publishes.
#
# That half asserts on the exact shape of `web/index.html`, and `web/` is
# touched by almost every commit that lands. Once per full lap of the config
# rotation is enough to catch a break within the hour instead of at a publish.
# ------------------------------------------------ does a saved world come back
#
# `GET /save` and `POST /load` exist in both builds and nothing had ever
# exercised either.
#
# Every benchmark lap, on whatever board it lands — four seconds, so there is no
# reason to ration it to one board the way the three-minute publish check is.
# Spreading it across the rotation is also the point: a globe save has to carry
# the sphere's own geometry, and a ten-major `crowded` save carries far more
# world than `baseline` does. Restricting it to one board would have tested the
# easiest case only.
SAVELOAD=""
if [ "$sim_status" -eq 0 ] && [ "$SEED_KIND" = benchmark ]; then
  SL_LOG="$LOGS/runs/$TAG.saveload.log"
  if [ "$ARM" = rust ]; then
    SPORT=$(( PORT + 700 ))
    nice -n 5 "$TREE/target/release/civvis" play --no-open --port "$SPORT" \
      --players "$PLAYERS" --seed "$SEED" --speed "$SPEED" \
      --difficulty "$DIFFICULTY" --map "$CONFIG_MAP_SCRIPT" \
      --shape "$CONFIG_MAP_TOPOLOGY" --poles "$CONFIG_MAP_POLES" \
      "${LEAGUE_ARGS[@]}" \
      --spectate --paused > /dev/null 2>&1 &
    sl_pid=$!
    trap 'kill '"$sl_pid"' 2>/dev/null; rm -rf "$LOCK"' EXIT
    for _ in $(seq 1 60); do
      curl -sf -m 2 "http://127.0.0.1:$SPORT/status" >/dev/null 2>&1 && break
      /bin/sleep 0.5
    done
    nice -n 5 node "$LOGS/saveload.mjs" --arm rust --base "http://127.0.0.1:$SPORT" \
      --seed "$SEED" --players "$PLAYERS" --speed "$SPEED" --difficulty "$DIFFICULTY" \
      --map-script "$CONFIG_MAP_SCRIPT" --map-topology "$CONFIG_MAP_TOPOLOGY" \
      --map-poles "$CONFIG_MAP_POLES" --start-era "$CONFIG_START_ERA" --emit-save "$LOGS/.cross-save.json" > "$SL_LOG" 2>&1
    SAVELOAD=$([ $? -eq 0 ] && echo ok || echo failed)
    kill "$sl_pid" 2>/dev/null; wait "$sl_pid" 2>/dev/null
    trap 'rm -rf "$LOCK"' EXIT
  else
    nice -n 5 node "$LOGS/saveload.mjs" --arm wasm --wasm "$WASM_SHIPPED" \
      --seed "$SEED" --players "$PLAYERS" --speed "$SPEED" --difficulty "$DIFFICULTY" \
      --map-script "$CONFIG_MAP_SCRIPT" --map-topology "$CONFIG_MAP_TOPOLOGY" \
      --map-poles "$CONFIG_MAP_POLES" --start-era "$CONFIG_START_ERA" --load-save "$LOGS/.cross-save.json" > "$SL_LOG" 2>&1
    SAVELOAD=$([ $? -eq 0 ] && echo ok || echo failed)
  fi
  echo "   save/load: $SAVELOAD"
fi

PUBLISH_STATUS=""
PUBLISH_BYTES=0
if [ "$sim_status" -eq 0 ] && [ "$ARM" = wasm ] && [ $(( PAIR % 12 )) -eq 11 ]; then
  echo "   publish: assembling the site"
  PUB_OUT="$LOGS/.publish-check"
  rm -rf "$PUB_OUT"
  if ( cd "$TREE" && nice -n 5 ./beta/publish.sh --out "$PUB_OUT" ) \
      > "$LOGS/runs/$TAG.publish.log" 2>&1; then
    PUBLISH_STATUS=ok
    PUBLISH_BYTES=$(find "$PUB_OUT" -type f -exec wc -c {} + 2>/dev/null | tail -1 | awk '{print $1}')
  else
    PUBLISH_STATUS=failed
  fi
  echo "   publish: $PUBLISH_STATUS ${PUBLISH_BYTES} bytes"
  # The bundle is 12 MB and only its size and verdict are wanted.
  rm -rf "$PUB_OUT"
fi

python3 "$LOGS/record.py" --result "$RESULT" --ledger "$LOGS/ledger.jsonl" \
  --iteration "$ITER" --arm "$ARM" --seed "$SEED" --sha "$SHA" --stamp "$STAMP" \
  --build-seconds "$build_seconds" --build-warnings "$warnings" \
  --sim-seconds "$sim_seconds" --sim-status "$sim_status" \
  --raw-wasm-bytes "$raw_bytes" --config "$CONFIG_NAME" \
  --cpu-seconds "${cpu_seconds:-0}" --load "${load_now:-0}" \
  ${REPEAT_LOG:+--repeat-log "$REPEAT_LOG"} \
  ${PUBLISH_STATUS:+--publish "$PUBLISH_STATUS" --publish-bytes "$PUBLISH_BYTES"} \
  ${SAVELOAD:+--saveload "$SAVELOAD" --saveload-log "$SL_LOG"} \
  --sim-log "$SIM_LOG" --build-log "$BUILD_LOG"

# One module per iteration is enough to reproduce a failure; a directory of
# them is 10 MB a turn and fills the disk by morning.
find "$LOGS/runs" -name '*.wasm' -not -name "$TAG.wasm" -delete 2>/dev/null

# The logs of old iterations, likewise. This loop is meant to run until the tab
# closes, and at roughly 57 KB an iteration nothing here would ever be reclaimed
# — about 40 MB a day, for logs nobody reads past the next few iterations.
#
# The `.json` rows stay: they are small, and together with `ledger.jsonl` they
# are the record. Only the bulky per-run logs age out, and only well behind the
# window `summary.py` reports on, so anything currently being talked about is
# still on disk.
KEEP_FROM=$(( ITER - 40 ))
if [ "$KEEP_FROM" -gt 0 ]; then
  for old in "$LOGS"/runs/*.log; do
    [ -e "$old" ] || continue
    n=$(basename "$old" | cut -c1-3 | sed 's/^0*//')
    case "$n" in (''|*[!0-9]*) continue ;; esac
    [ "$n" -lt "$KEEP_FROM" ] && rm -f "$old"
  done
fi

{ echo "ITER=$ITER"; echo "NEXT_ARM=$([ "$ARM" = rust ] && echo wasm || echo rust)";
  echo "SEED_BASE=$SEED_BASE"; echo "FRESH_SEED=$FRESH_SEED"; } > "$STATE"

exit "$sim_status"
