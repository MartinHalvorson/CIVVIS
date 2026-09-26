# Gran Colombia conquest milestones — 2026-09-26

The fixed King / four-player / Gran Colombia / Tiny Pangaea paired evaluator
now distinguishes declarations against majors from city-state wars, defensive
war exposure from focal declarations, and original-capital control from other
foreign cities. Schema 2 adds `conquest` to each arm without removing the old
outcome fields. This changes measurement only, not the controller.

The observer runs at turn boundaries and once after the game ends. Every
`*_observed_turn` is the first boundary that saw the event, not its exact action
turn. Declaration counts consume the applied-action log once, including wars
that ended before the next observation. City sets record unique cities held
at observations; captures and losses between observations are not counted.
The final ownership fields distinguish taking a capital from keeping it and
also report whether the focal seat still controls its own original capital.
City-state capitals do not count as foreign major capitals.

## Diagnostic pilot plan

Before running: compare `early-conquest-opening` off/on over four complete
pairs, seeds 37140000–37140003. Preserve every pair, including losses and
identical action histories. All other focal policies use the compiled live
bundle. The outcome of interest is major declarations followed by foreign
capital control and domination wins; score alone cannot qualify improvement.
This small pilot diagnoses exposure and chooses the next experiment. It is
not sufficient to promote or remove a policy. Rivals are CIVVIS controllers
with King bonuses, not Firaxis AI; native verification remains separate.

## Pilot result

All four pairs completed, with byte-identical applied-action histories off/on.
The evaluator exited 2 after writing all results, its documented signal for
zero behavioral contrast. Both arms won 0/4 and held zero foreign major cities
at every observation. No policy deployment change is justified by this pilot.

| Seed | First major declaration observed | Major declarations | Minor declarations | End turn / loss | Foreign cities held at end |
|---|---:|---:|---:|---|---:|
| 37140000 | 167 | 1 | 1 | 222 / Science | 1 |
| 37140001 | 116 | 2 | 0 | 220 / Science | 0 |
| 37140002 | none | 0 | 1 | 176 / Religion | 1 |
| 37140003 | 76 | 2 | 0 | 223 / Science | 0 |

Every seat retained its own original capital. The two foreign cities in the
old aggregate metric were minor cities, not progress in conquering the other
major civilizations. Seed 37140002 first experienced major war at observation
144 but never declared one itself. The next investigation must explain both
late declarations and why the wars that do start capture no major cities;
changing the opening flag alone does not expose a different policy here.

Measured code: `40a9b4d19` (the first measurement commit; later integration
changes are not covered by this pilot). Binary SHA-256:
`2fca14a4c07bd8ecd5d2e204973488d783c5ff503a09ec3b53ef3e0006b6fed0`.
Raw output on `mbp-m5-max-128`:
`~/civvis-simulation-results/domination-milestones-20260926-early-conquest.jsonl`.

```sh
cargo build --profile ci --locked --features developer-tools --bin victory_eval
target/ci/victory_eval --domination-pair early-conquest-opening --games 4 --start-seed 37140000 --out /tmp/conquest-milestones.jsonl
```

Focused validation: eight `domination_pair` tests passed, covering defensive
war versus offensive declarations, major/minor distinctions, repeated reads,
capital loss and recapture, observer immutability, and the existing fixed
profile checks. `cargo test --profile ci --locked` passed before integration.
No engine soak is required: the observer is read-only and no engine or AI
policy is changed by this contribution.

## Separate native observation

Run `civvis-20260926T173411Z` verified King, Gran Colombia, four players,
Pangaea, Tiny, Online and Gathering Storm from the live seat. It declared on
Byzantium at turn 75 and lost to team 3's Science victory at turn 235.
The native combat summary records 52 kills, 28 losses and zero cities taken
or lost. This warrants investigating siege execution even in games that do
declare war and trade favorably against units.
This is a native loss, not a simulator result or a domination win. The existing
supervisor started the next King attempt at 18:21:46 UTC on revision `b86eaef`.
