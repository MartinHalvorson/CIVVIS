# Remembered naval threats while a land escort follows its settler

`hostile-memory-2` is an opt-in successor to `hostile-memory`. Enabling either version disables the other. Existing deployments retain their current defaults.

## Observed failure

In Civilization VI verification run `civvis-20260908T225920Z-cont1`, archer 2359304 followed a settler across the coast. At turn 119 it received a fresh move from land (30,11) to water (31,12), then lost 76 HP to caravel 11272219 and its remaining 24 HP to ironclad 11337768. These are host offset coordinates. The immediate frame contained no hostile units, but turn 118 had shown caravel 10616835 at (33,10); turn 117 also showed a nearby caravel. A one-frame replay cannot restore those sightings.

The existing hostile memory already retains observed positions for four turns. Civilian safety consulted it; the military escort's follow step did not. Mountains and a volcano block the direct land approach in this case. A local search found no static legal land route within eight tiles; that is not proof that every longer route or move through friendly units is impossible.

## Treatment

When a land escort is currently safe on land, inspect the ordinary follow step on a speculative board. If that choice ends in water within known naval reach, try a legal forward land neighbor outside known land threats and current strike danger. If none works, fortify or stop ashore while preserving the escort binding.

The rule does not apply to ships or an already embarked guard. It does not replace an escape from a threatened land tile with a hold. Expired sightings and ships never observed by the player do not justify blocking embarkation. Version 2 retains version 1 civilian memory behavior.

## Behavioral controls

Eight focused tests cover: the old legal but hazardous embarkation versus the new hold; a safe forward land alternative; expiration; a hidden ship with no sighting; escape from a remembered land threat; disembarkation; ship exclusion; and exclusive, opt-in version flags. The original protection test failed before the implementation. All eight pass after it.

## Limits

This is a bounded memory heuristic, not a forecast of the enemy fleet. It uses the existing reach projection without changing its growth model. It may delay an escort until a sighting expires. The check examines the planner's resulting tile; it does not prove the external host cannot truncate a longer walk on an intermediate water tile. No full-game benefit or default promotion follows from the unit tests.

## Persistent observed-board replay

Both versions processed all 52 recorded frames through turn 119/frame 0, retaining their own memory and escort bindings. The same compiled binary was used, with the six forced live tags from the recorded run and either `hostile-memory` or `hostile-memory-2`. Each frame included its following map chunks. At turn 119, version 1 ordered archer 2359304 to `MOVE_TO (31,12)`; version 2 ordered `FORTIFY` at its current land tile (30,11). The explanation was “Guard waits ashore.” This was the only changed frame for that guard.

This demonstrates that the actual remembered sightings change the observed fatal embarkation decision. Subsequent boards were fixed host observations, not executions of the alternative orders; it does not establish the counterfactual game's outcome.

Replay source revision: `32db5bf03`; binary SHA-256: `fb84ee15e3929f8a1dfb7f5a15ba4c15955b8041884054a0c647bfb2601dc09f`.

## Native smoke probe and validation

The first standard six-game probe (seeds 99133000..99133005) was externally suspended twice before completing a game. No memory-guard stop was recorded. Its header-only output was retained locally, the owned suspended process was terminated, and no results were inferred from it.

A reduced probe completed all six games / 36 seats on seeds 99133100..99133105: six players, 40×30 Continents, three city-states, Online speed, 60 turns, one worker. Its committed analysis is `docs/gene_screens/fires/2026-09-09-hostile-memory-2.json`, explicitly classified `legacy` by the instrument because its profile is not the standard screen. It is not a ledger source. Six seats had the gene on and 30 off. The win contrast was +20.00 ±22.91 percentage points (standard error); share contrast +0.84 ±3.09. These broad uncertainties do not support a performance claim.

This probe draws independent seat genomes. Its nonzero outcome variance satisfies the current artifact gate, but does not itself establish a causal change from this gene; even an inert tag can have outcome variance under that design. The matched old/new unit tests and the persistent replay supply the behavioral evidence here. No default changes are proposed.

Commands:

```sh
cargo test --profile ci --locked --lib hostile_memory_
cargo test --profile ci --locked
cargo build --profile ci --locked --features developer-tools --bin civvis_orders --bin gene_screen
target/ci/gene_screen --games 6 --jobs 1 --genes hostile-memory-2 --start-seed 99133100 --turns 60 --width 40 --height 30 --city-states 3 --out naval-memory-small-probe-6.jsonl
target/ci/gene_screen --analyze naval-memory-small-probe-6.jsonl --json docs/gene_screens/fires/2026-09-09-hostile-memory-2.json
python3 tools/genes.py write
python3 tools/eval_manifest.py --write
python3 tools/gene_fires.py --max 0
```

The full local Rust suite passed 3,235 tests, with 50 ignored including doc tests. The focused eight-test result is included in that total. The smaller native probe is a smoke exercise, not an engine-mechanics soak; the game engine was unchanged.
