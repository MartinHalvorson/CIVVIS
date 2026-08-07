# Simultaneous turns

`--turn-structure simultaneous` is a rules variant of the turn cycle, chosen
at setup like a map script and recorded on the save. The stock regime —
`sequential`, the default everywhere — lets each civilization act on the
world exactly as the previous one left it. The simultaneous regime freezes
the world at the top of each game turn, lets every seat plan its complete
turn against that same snapshot, and then commits the plans in seat order
under the ordinary rules.

It exists for throughput. Roughly two thirds of simulator runtime is the
AIs' own deliberation (`docs/SIMULATOR_PERFORMANCE.md`), and under the
sequential regime that deliberation cannot be parallelized across seats,
because each seat's information set includes the previous seat's whole
turn. Freezing the information set at the top of the turn removes exactly
that dependency and nothing else: seat deliberation becomes independent
work, and the planning phase fans it out across up to `--jobs` threads
(`run_structured_jobs`). Only the deliberation is concurrent — the prepare
and the commit stay strictly serial, every planning world and RNG stream is
fixed before the first worker starts, and results are consumed in seat
order, so the fan-out is an execution detail: `jobs = 1` and `jobs = N`
produce byte-for-byte the same game and census (test:
`parallel_planning_is_an_execution_detail`). A scoped seat-planner fleet is
started once for the whole headless game and reuses its workers on every
cycle; it owns only the AI borrows while each request carries an owned private
world. This avoids creating fresh worker threads on every cycle. Under
`simulate`, a simultaneous game therefore takes `--jobs` at the full batch
default (one per core) and skips the AIs' inner WorkPool, while sequential
games keep the intra-turn frontier pool with its measured knee of four.

`soak` uses the same budget without nested oversubscription. If there are at
least as many games as jobs, it retains the ordinary one-game-per-worker
batch. If there are fewer simultaneous games than jobs, it starts only that
many outer games and divides the remaining worker budget among each game's
seat planners. Thus `civvis soak --games 1 --turn-structure simultaneous
--jobs 128` can use up to one worker per ready civilization instead of
silently running its one game's turns serially. The real ceiling is the number
of seats ready in a cycle: 16 civilizations cannot keep 128 planners busy,
but 64 simultaneous civilizations can use 64 independent planning workers.

## What it changes, and what it deliberately does not

A simultaneous game turn runs in two phases, both in
[`src/simultaneous.rs`](../src/simultaneous.rs):

1. **Plan.** One rolling clone of the world advances through the seats
   with *empty* turns — each seat's upkeep (unit refresh, growth, income,
   research settlement) runs in the stock order, but nobody acts. Each
   seat receives a private copy of that world, with a seat- and turn-keyed
   RNG stream (the same derived-stream discipline disasters and meteors
   use), and its AI plays its whole turn against the copy. The actions it
   applied there are its plan.
2. **Commit.** The plans land on the authoritative world in the same seat
   order, through the very same `Game::apply` calls a sequential game
   makes, each seat closing with the ordinary `EndTurn` so upkeep, the
   world-turn wrap, and victory checks run exactly where they always run.
   A planned action the world has outrun — the tile is now occupied, the
   target is dead, the city fell to somebody faster — simply fails
   `apply` and is **dropped, not reinterpreted**. A mandatory choice the
   plan never made (a captured city's fate) is resolved with the first
   legal answer and counted.

Nothing in `game.rs` branches on the regime. Every committed action goes
through the ordinary rules, which is what buys the guarantees below.

## Guarantees

- **The committed game is an ordinary game.** Its action log replays
  through a plain `apply` loop bit-for-bit (test:
  `a_simultaneous_log_replays_bit_for_bit`), saves round-trip, and the
  engine's determinism invariants — failed applies consume no RNG, one
  serialized stream — apply unchanged.
- **Same seed, same game** (test: `a_simultaneous_game_is_deterministic`).
- **The default is untouched.** A sequential game through the structured
  driver is byte-for-byte the game `run_game` plays (test:
  `the_default_structure_is_sequential_and_unchanged`), and the option
  defaults to sequential in `GameOptions`, the CLI, the server, and old
  saves.

## The census

Every simultaneous run reports a `SimultaneousCensus`: actions planned,
applied, and dropped (by action kind), plus rare-path counters (forced
mandatory resolutions, seats that could not be planned or were eliminated
mid-commit). `simulate` prints the full summary line; each `soak` line
carries `SIMUL drops=X/Y`. The drop rate is the regime's health
instrument: it is the price of planning against a frozen world, it should
be low and stable, and a rise is the first sign the regime is distorting
play. Read it before trusting any result measured in this mode.

## Boundaries

- **A played game is sequential by construction.** A human seat is
  consulted live, one seat at a time, so `civvis play` without
  `--spectate` refuses the flag and `--resume` refuses a simultaneous
  save, rather than quietly stepping a simultaneous world one seat at a
  time. A *spectated* table has nobody at the keyboard: `civvis play
  --spectate` plays the regime as one whole planned turn per pace tick,
  the viewer's turn plate names it (`Turns:`) and carries the census,
  and `/new` / `/next-game-settings` accept `turn_structure` for
  spectated worlds only. A simultaneous exhibition game rates nobody —
  the league's Glicko-2 table stays a sequential-regime instrument.
- `simulate` and `soak` honor the option. `benchmark`, `tournament`,
  `league`, `elo`, `selfplay`, and `evolve` play sequential games, and
  their ledgers and corpora must not silently mix regimes: a simultaneous
  result is a result about a different game. The Elo setup contract pins
  defaults, so the ledger is unaffected while the default stands.
- `StrategicAi`'s internal rollouts model sequential play regardless of
  the outer regime; its speculation was already approximate and is not a
  correctness input.
- Within one turn, commit order is the stock ascending seat order, so an
  earlier seat's orders land first. Seat identity is seed-random under
  `randomize_civs`, which washes the priority out across a corpus; a
  rotating commit order is a possible follow-up if measurement shows it
  matters.

## Using it

```bash
civvis simulate --players 6 --turn-structure simultaneous --seed 7311002
civvis soak --games 20 --turn-structure simultaneous
```

## Measured (2026-08-06, ci profile, shared host — directional)

One 74×46-class 150-turn online game, seed 7311002, wall clock:

| Players | Sequential | Simultaneous `--jobs 1` | Best simultaneous | vs sequential |
|---|---:|---:|---:|---:|
| 6  | 13.7s | 14.5s | **7.6s** (jobs 8) | **1.79×** |
| 10 | 42.4s | 48.6s | 28.1s (jobs 18) | 1.51× |
| 14 | 41.5s | — | 32.4s (jobs 18) | 1.28× |

Normalized output was byte-identical across jobs 1/8/18 at both measured
sizes, and the drop rate holds near 5% at every scale (94–96% of planned
actions applied). The advantage currently *shrinks* as seats grow: the
serial prepare walks one empty `EndTurn` per seat per cycle, each touching
the whole world, so its share rises with seat count. Making that forward
cheaper (an upkeep-only step, or preparing seats concurrently) is the
standing follow-up if very large games are the goal. A 20-paired-seed
semantics A/B (drop rate 3.6–5.7%, city counts indistinguishable, no
seat-0 advantage, religious closes rarer) is recorded in
`~/civvis-turn-structure-ab-preregistration.md`.

### 9985WX scheduling validation (2026-08-06, ci profile)

The local validation host is an AMD Ryzen Threadripper PRO 9985WX (64 physical
cores / 128 logical CPUs). A 16-civilization, 64×40, zero-city-state,
100-turn simultaneous game at seed 7,311,002 produced the same normalized
`simulate` SHA-256 at jobs 1 and 16:

```text
99a53801956a1d9407848371ab5c23d58a5349acb2d9d2990bff1f597fd62b71
```

The job curve was 10.34s at one planner and 4.60s at 16 planners (2.25×).
At 32 civilizations / 40 turns on the automatic map, the same comparison was
17.28s versus 9.50s (1.82×), again with identical state and census. The
persistent fleet is performance-neutral versus the old transient fan-out on
the 16-seat `simulate` workload itself; its important throughput change is
that `soak` now reaches the seat frontier when the game batch is narrow.

For the 16-civilization one-game soak above with `--jobs 16`, current `main`
spent 10.26s because its only outer game worker ran its seats serially. This
change completed in 4.55s (2.25×). Both normalized soak reports hash to:

```text
1a8a2d18591f551812bcd0224f3132c13b0998b0347bf1e51daae1846d2b56dc
```

These are local directional measurements, not a promise that every map shape
has the same knee. Always use an explicit `--jobs` when calibrating a new
machine or player count, and compare normalized reports before trusting a
throughput result.

Before comparing regimes, run paired seeds both ways and read the drop
rate alongside the outcome distributions — one seed is never a result.
