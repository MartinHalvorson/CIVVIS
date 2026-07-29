# Live-league genome transfer

Status: **prospectively registered; neither focal seed has been opened**.

## Why this candidate

The committed generation-14 champion was selected on four-player 24x16
Standard games. Its published advantage disappears at the larger Online
profile, so copying that result into deployment is not supported. The live
league supplies a different opportunity: it has selected AdvancedAi genomes
while the production spectator actually plays Online games on the large map.

At runtime-league round 543, after recorded matches through round 542,
`g44-41` (`JackKnife`) was the highest-rated non-retired entrant: 1797.13
Glicko, RD 61.15, 498 games, and 95 wins. Stock `advanced` was 1750.59, RD
61.14, over 556 games. The 46.54-point gap is selection evidence, not a causal
estimate: live seats are neither paired nor uniformly assigned, the candidate
was selected from many genomes, and the mutable league has repeatedly viewed
its outcomes. It nominates one candidate for a fresh paired test and proves
nothing by itself.

The source snapshot is frozen by these SHA-256 values:

- runtime `league/league.json`: `fa135aa8b7b11a69f9d3ba63dcd0507beaaf23249a6e3c4a095b297ce971451d`;
- runtime `league/matches.csv`: `9904e80f8ba44a7de4539463ca4a7aee2c92d0542a6cfed89df8345cbf795739`;
- canonical compact `g44-41` row: `83981c5a97f68030ec594cca41601ac5dbc2ab2e6ac38ec98b1763500e545b1f`;
- canonical compact weights object: `ee36989d5585c7537528c8ff0adc7379f369dc221c910b6824d0cfcad7146db0`.

`tests/fixtures/live_league_g44_41_best.json` is the exact weight object in the
shape accepted by `evolve::load_champion`. Its generation is provenance only;
fitness and validation metadata are deliberately zero because league rating is
not evolution fitness or a holdout result. The fixture's file hash is frozen in
the prospective checkpoint as
`145571ea717a2603f7df43c8807cf9e05818591d477b3c5995b2b849d17bf237`.

## Frozen hypothesis

A genome selected on the actual large-map Online exhibition distribution will
transfer better than the generation-14 genome selected on the small Standard
breeding profile. Specifically, `g44-41` will beat stock `AdvancedAi` on fresh,
paired maps matching the spectator configuration closely enough to earn the
existing `ai_eval` promotion gate.

This experiment isolates only the 51 serialized `Weights` fields. Both arms
use the same current `AdvancedAi` implementation, rules, leaders, maps, victory
set, seat counts, and turn budget. It does not test StrategicAi, a value net,
league selection logic, or a production default.

## Execution identity and preflight

The Rust source for the official binary is byte-identical to base
`0aa1e0f51e84b72bec8c1811dfc71bed1f5f5a26`; this branch adds only the
protocol and candidate fixture. The prospective protocol checkpoint is
`06a55bb7fac7d17838e004ca886fdec7e2d9882b`, committed before any focal
run. Immediately before each phase:

1. verify the tracked fixture's frozen SHA-256;
2. copy it byte-for-byte to ignored `evolved/best.json` in this worktree;
3. verify the two files are byte-identical;
4. require `ai_eval` provenance to report `advanced_evolved` with a champion
   loaded and `advanced` as scripted, with no collapsed-entrant warning; and
5. verify the host can provide four simulator cores without oversubscription.

A one-map, one-turn diagnostic at seed `9957999` may validate provenance and
argument realization. It is not strength evidence and may not be pooled with a
focal phase.

The current spectator profile is frozen as eight majors, 84x54, twelve
city-states, Online speed, Continents, Flat topology, Poles, randomized Civ VI
leaders/civilizations, policy-visible turn limit 250, and Science/Culture/
Domination victories. Each independent map is played twice. Four seats use the
candidate and four use stock in each game; the second game swaps every seat.
The map-pair average is the only inference unit.

## Fixed development screen

The only screen is 40 maps / 80 games at seed `9958000`:

```text
target/release/ai_eval advanced_evolved advanced --pairs 40 --players 8 \
  --width 84 --height 54 --city-states 12 --turns 250 --speed online \
  --map continents --shape flat --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9958000 --jobs 4 \
  --artifact-dir evolved --require-artifacts
```

The screen earns confirmation only if every condition holds:

1. all 40 pairs complete under the frozen identities and profile;
2. candidate paired-map win score is at least 52.5%;
3. candidate paired terminal-score share is at least 50.0%;
4. candidate total game wins are not fewer than control wins;
5. candidate-favored win directions outnumber control-favored directions; and
6. the existing promotion verdict is not `RETAIN`.

Any failure means **STOP**. Do not tune the genome, change the profile, retry a
seed, inspect the confirmation, or pool the screen with another run.

## Fixed confirmation and decision

A passing screen earns one unchanged 240-map / 480-game confirmation at seed
`9959000`. Every argument above is identical except `--pairs 240` and the seed.
The candidate passes only if:

- the existing win-based promotion verdict is exactly `PROMOTE` (the current
  95% Wilson lower bound clears parity and the anytime-valid challenger bound
  is at most 0.025);
- paired terminal-score share is at least 50.0%;
- candidate total game wins are not fewer than control wins; and
- provenance, completion, and profile integrity remain exact.

The screen and confirmation are reported separately and never pooled. A pass
permits a separate integration study of where this genome should be exposed;
it does not automatically replace `data/evolved/best.json`, alter stock
`AdvancedAi`, change league ratings, or transfer the genome into StrategicAi.
A failure retains the current shipped behavior and records the negative result.

Both phases use exactly `--jobs 4`. This prospective resource amendment was
made before any focal seed so the older, already-promised six-core Spaceport
null can retain its queue priority. `ai_eval` folds results in map order and is
tested byte-identical across job counts, so the amendment changes wall time,
not the estimand or result. The run must pause if four idle cores are not
available.
