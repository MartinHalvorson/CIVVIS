# Hidden cities must prevent false elimination

The frozen native recording `civvis-20260920T104944Z` has 422 decision
frames through turn 160. It ended unfinished during watchdog recovery; it
contains no verified own city capture or domination victory.

At turn 105 Babylon is still at war with us, reports six cities, and exposes
three units but no city records. The diagnostic experiment in closed PR #3625
showed Babylon changing from alive to eliminated inside turn 104 planning.
The air campaign then appointed England's Bristol on turn 105 despite the
ongoing Babylonian war. Babylon was represented as alive again by turn 159,
but the air appointment persisted.

The engine's ordinary elimination check treated the absence of modeled cities
as the absence of any cities. A speculative casualty could consequently mark
the rival dead and delete its remaining visible army. This invalidates both
combat evaluation and war-front selection. Increasing city-strike scores would
not repair the missing rival.

## Change

A partial board now records the owners of cities omitted from it, without
inventing locations. Player views preserve this existence information when
redacting cities. Native reconstruction and persistent synchronization derive
it from authoritative public city totals after planting observed cities, and
replace it on every host update. Repeated player views preserve the public
total instead of replacing it with the count of retained city records.

Ordinary elimination respects this marker. Arena elimination retains its army
rules, and a full board or a host board containing every rival city still
eliminates a rival after its actual last city falls. The marker survives
serialization, with an empty default for older saves and no serialized field
when empty.

The export contract is explicit in
`tools/civ6_control/mod/CivvisControlAgent.lua:6182`:
`tally.city_count = tally.city_count + 1;` counts the owner's cities, and
line 6195 assigns `stats.city_count = totals.city_count`. It is not a count of
city records revealed to the observing player.

## Validation

Before the fix, five of seven focused regression tests failed: hidden-city
survival after a casualty, repeated views, serialization, taking the last
visible city, and a native host board reporting six unrevealed cities. The two
full-information last-city controls passed. Those seven tests passed after
the fix. Additional tests cover razing and authoritative synchronization.

- `cargo test --profile ci --locked`: 3,705 library tests and 205 other
  tests passed; 49 library and four documentation tests ignored. All nine
  focused regression tests passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `target/ci/civvis soak --players 4 --games 8 --start-seed 3626 --turns 180 --jobs 4`:
  eight of eight games completed without a crash. These short simulated games
  are stability checks, not native domination evidence.
- Paired baseline/candidate replays completed all 422 frozen frames with exit
  zero (116.30 / 134.26 seconds). Excluding `order_failed`, `order_verified`,
  and `turn_verified`, exported orders changed in 61 frames and internal
  actions in 67. Changes begin at turn 96 and continue through turn 160.
- At turn 160 the baseline attempts to organize a coalition against England
  and then holds off because the Babylonian war is still active. The candidate
  keeps a five-unit siege force assigned to Malgium and emits neither England
  coalition nor England declaration-delay explanations. Advanced Flight is
  requested on turn 143 in both replays.
- The change has broad tactical consequences because rival armies no longer
  disappear after speculative losses: 115 unit orders removed / 106 added,
  17 immediate production requests removed / 29 added, and 15 next-production
  requests removed / eight added. Four research requests and three policy
  decks change; three embassy requests disappear. This is evidence of a
  corrected planning model, not a claim that each alternate move is stronger.

Replay artifacts on the verification host:
`/tmp/civvis-fogged-elimination-replay/`, driver
`/tmp/civvis-fogged-elimination-frame-replay.py`, and comparison
`/tmp/civvis-3626-comparison.json`.
Replay order changes are alternate requests on a fixed observation stream;
they cannot establish adoption, a capture, or a native victory.
