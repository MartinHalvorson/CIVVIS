# Early housing growth and production: existing Granary policy withheld

The existing Granary reservation remains off. Across 28 paired native games (56 games), the pilot's early production gains did not reproduce reliably. Fresh confirmation increased Emperor turn-75 production by 1.84% but reduced Deity production by 3.49%, with no Deity population gain then. This policy does not establish fast production catch-up.

The goal is faster production growth against high-level rivals. The preceding
industrial-chain experiment (#3892) did not produce broad early catch-up and was
reverted. In its fresh confirmation control, Deity turn-75 production averaged
32.688 from 3 cities and 13.375 population, only 0.171 of the strongest rival's
production. The recorded King game also had housing limits in five of six cities
at turn 75. These observations motivate an early growth screen; they do not prove
that housing investment will repay its cost.

The first candidate is the existing, deployment-off
`first-granary-reserve-2` policy. It reserves a legal Granary only when its housing
lift forecasts the next citizen within a speed-scaled 30 Standard turns and
advances that arrival by at least 2 Standard turns. It excludes a locally
threatened or recently attacked city and uses the current food yield. It ignores
the building's additional food and any change in worked tiles. No runtime source,
gene default, rules, or ledger is changed for this screen.

## Frozen comparison

- Engine source: `afba14116415131f730a462dff4e1d7811aced93`; the task's claim
  commit has identical `src`, `data`, ledger, and Cargo manifests. Reuse its clean,
  completed production release library, recording its hash before play.
- One probe binary, two explicit arms. Both use the deployment gene ledger;
  only the focal candidate calls `enable_first_granary_reserve_2` afterward.
- Four majors, 60 by 38 Pangaea, six city-states, Online speed, all victories and
  barbarians, turn cap 150. Gran Colombia is focal, rivals are randomized.
- Focal seat is exempt from difficulty bonuses, explicitly targets Domination,
  and opponents use the unchanged `AdvancedAi::fleet` seats.
- Pilot: Emperor seeds 61006000–61006003 and Deity 61006100–61006103, both arms.
  The policy and probe were frozen before running any of these games.
- Originally reserved and subsequently completed confirmation: Emperor 61006200–61006211 and Deity
  61006300–61006307. The early pilot gains and later Deity cost motivated fresh
  confirmation of the unchanged policy; no tuning occurred between phases.
- Record all final outcomes, including early world endings and eliminations.
  Compare only checkpoints observed in both arms, explicitly list missing
  checkpoints, and never carry a final value forward as an observed later turn.
- At turns 25, 50, 75, 100, 125, and 150, observe production, strongest-rival
  production, population, cities, Granaries, housing-limited cities with and
  without Granaries, amenity shortages, science, culture, Gold, Builders, and
  military power. The rival population/city counts belong to the rival with the
  greatest production at that checkpoint. Cumulative production is the sum of
  observed city yields, not production spent or credited to construction.
- Early production, population, and production relative to the strongest rival
  determine whether the hypothesis addresses the goal. Expansion, science,
  culture, military power, and outcomes reveal costs. A late conditional gain
  alone is insufficient evidence of fast catch-up. A pilot is not promotion
  evidence or a claim of actual Firaxis performance.

## Preliminary read-only controller replay

Before the native screen, the archived #3892 control controller replayed turns
1–100 of the same recorded King game's concatenated prefix with persistent AI
history. `--with first-granary-reserve-2` is an existing supported treatment arm;
both arms use the same archived binary and recordings. The candidate first
changes a production proposal on turn 40 and proposes Granaries in seven cities
through turn 100, while control proposes none. All 100 responses completed in
both arms. The observations continue to come from the original controller, so
these are counterfactual proposals, not actuated decisions or candidate growth
outcomes. The replay engine predates the unrelated #3895 spy mirror change;
the native screen uses the current engine specified above.

## Native outcomes

Both phases are complete, with all intended outcomes retained. Neither arm won a
focal game. Pilot had no focal eliminations. Fresh Emperor confirmation had one
elimination in each arm; Deity had four in each arm. Later checkpoint counts fall
when the whole world ends. Eliminated seats retain their observed zero-city
states while a world continues; no final values are carried to missing turns.

The tables use only checkpoints observed in both arms. Production is the sum of
focal city yields. Cumulative production sums observed yields on every callback;
it is not production spent. The rival comparison is the mean of each matched
game's focal/strongest-rival production ratio.


### Pilot

| Difficulty | Turn | Matched games | Production, control → candidate | Change | Cumulative production change | Science change | Culture change |
|---|---:|---:|---:|---:|---:|---:|---:|
| Emperor | 25 | 4 | 14.300 → 14.300 | +0.00% | +0.00% | +0.00% | +0.00% |
| Emperor | 50 | 4 | 38.650 → 38.700 | +0.13% | +0.30% | -0.09% | -0.09% |
| Emperor | 75 | 4 | 66.000 → 69.500 | +5.30% | +0.65% | +9.78% | +10.48% |
| Emperor | 100 | 4 | 127.000 → 125.375 | -1.28% | +1.71% | +8.16% | +7.56% |
| Emperor | 125 | 4 | 186.387 → 201.150 | +7.92% | +3.06% | +0.88% | +14.91% |
| Emperor | 150 | 4 | 234.700 → 240.225 | +2.35% | +2.95% | +0.05% | +1.86% |
| Deity | 25 | 4 | 14.075 → 14.075 | +0.00% | +0.00% | +0.00% | +0.00% |
| Deity | 50 | 4 | 26.600 → 26.600 | +0.00% | +0.00% | +0.00% | +0.00% |
| Deity | 75 | 4 | 41.925 → 43.175 | +2.98% | +0.41% | +0.77% | +3.84% |
| Deity | 100 | 3 | 64.233 → 68.000 | +5.86% | +1.82% | +10.27% | +13.03% |
| Deity | 125 | 3 | 87.867 → 77.700 | -11.57% | -1.19% | +16.19% | +4.79% |
| Deity | 150 | 1 | 10.500 → 18.900 | +80.00% | -3.74% | +1.74% | +2.62% |

### Confirmation

| Difficulty | Turn | Matched games | Production, control → candidate | Change | Cumulative production change | Science change | Culture change |
|---|---:|---:|---:|---:|---:|---:|---:|
| Emperor | 25 | 12 | 14.208 → 14.208 | +0.00% | +0.00% | +0.00% | +0.00% |
| Emperor | 50 | 12 | 28.717 → 28.900 | +0.64% | +0.08% | -0.04% | -0.06% |
| Emperor | 75 | 12 | 50.458 → 51.388 | +1.84% | +0.06% | -1.49% | -0.68% |
| Emperor | 100 | 12 | 84.867 → 87.108 | +2.64% | +0.36% | +2.81% | +1.65% |
| Emperor | 125 | 11 | 139.364 → 142.950 | +2.57% | +2.29% | +2.41% | +1.23% |
| Emperor | 150 | 10 | 181.555 → 181.345 | -0.12% | +3.45% | +0.08% | -1.99% |
| Deity | 25 | 8 | 15.312 → 15.312 | +0.00% | +0.00% | +0.00% | +0.00% |
| Deity | 50 | 8 | 27.281 → 27.281 | +0.00% | +0.00% | +0.00% | +0.00% |
| Deity | 75 | 8 | 32.631 → 31.494 | -3.49% | -0.26% | -0.54% | -1.91% |
| Deity | 100 | 7 | 43.414 → 41.843 | -3.62% | -2.20% | +4.16% | -3.89% |
| Deity | 125 | 7 | 51.057 → 49.514 | -3.02% | -2.12% | +7.65% | -11.55% |
| Deity | 150 | 4 | 75.825 → 70.875 | -6.53% | -0.77% | -3.46% | -1.57% |

The pilot's Deity turn-150 +80% rate is one matched game, with cumulative
production down 3.74%, three cities in control and one in candidate. It cannot
support a general late-game benefit.

## Why the policy remains off

Fresh Emperor confirmation made six additional Granaries across twelve games at
turn 75 (mean 2.5 → 3.0), while mean population fell 22.333 → 22.083 and cities
5.250 → 5.083. Production rose 1.84%, but cumulative production rose only 0.06%.
The production/strongest-rival ratio moved 0.4181 → 0.4272. At turn 100, production
rose 2.64% and population 31.167 → 32.083, with military power down 7.59%.

Fresh Deity confirmation also built more Granaries at turn 75 (1.625 → 2.000)
and reduced housing-limited, Granary-less cities (0.875 → 0.500), but population
stayed 14.875 and production fell 3.49%. Its production/strongest-rival ratio fell
0.1538 → 0.1489. At turn 100, population again stayed unchanged, production fell
3.62%, cumulative production fell 2.20%, and military power fell 11.93%. Later
matched Deity production remained lower at all reported checkpoints; the
turn-150 population increase did not produce a production gain.

The policy is reachable on recorded host observations, but early housing
investment alone is not a reliable production improvement in this comparison.
The forecast prices a first citizen's arrival, without pricing that citizen's
net production contribution, changed tile assignments, the Granary's food, or
the competing military/expansion investment. A future production-oriented
forecast needs to price those changes and preserve survival and expansion.
These results identify requirements for that experiment; they do not prove its
implementation or efficacy.

## Reproduction and validation

The manifest retains source identities, immutable binary/library hashes, the
preregistration timestamp, protocol, replay prefix hash, and artifact hashes.
The probe uses the clean production release library; matching Rust ThinLTO was
needed to link its bitcode. One unchanged executable runs both arms. Explicit production-profile compilation succeeded before play.

Recompute both summaries with:

```sh
python3 docs/eval/2026-10-04-production-growth-payback-summarize.py pilot
python3 docs/eval/2026-10-04-production-growth-payback-summarize.py confirmation
```

The summarizer rejects duplicate/invalid checkpoints, nonfinite metrics,
malformed or incomplete outcome seed sets, and checkpoints beyond final turns.
It explicitly retains every missing checkpoint and raw final outcome. Local `cargo test --profile ci --locked` passed 4,548 tests with 54 ignored
(4,296 library, 14 main-binary, 217 controller, 17 integration, and 4 shared
route tests passed). Changed-line Rust quality passed against the claim commit;
the explicitly compiled probe is formatted and warning-free. Fresh main was
fetched and merged before ready (already current), and source, rules, ledger,
and Cargo manifests remain identical to the frozen engine. Both summaries
reproduce byte-for-byte, and all artifact/binary/library hashes were rechecked. No runtime source, rules, ledger,
or production controller settings were changed. No real Firaxis treatment
outcomes or high-level production parity are established.
