# Share city queries across the culture forecast

The culture forecast now holds one read-only `QueryMemo` across its rival-city
and own-tourism calculations. City yields can share empire-wide derivations
throughout the sweep. The guard follows the existing early exits; formulas,
accumulation order, and gene defaults are unchanged.

## Measurements

The isolated tournament comparison uses clean CI-profile `gene_screen` builds:
baseline `d04d9744284f` (the code in main `1873ae903`) and candidate
`7c338930570b`. Every major draws the standard gene genome. The recorded headers
confirm Emperor, six players, 74x46 Continents, nine city-states, Online, a
250-turn cap, native competitions, and probabilities 0.25/0.75. Both builds have
the same compiled gene fingerprint.

Each arm runs `gene_screen --games 1 --jobs 1 --difficulty emperor --start-seed
SEED --out rows.jsonl`; pairs alternate execution order. User CPU is read with
`RUSAGE_CHILDREN`. Every recorded seat field and trajectory is compared, excluding
only `secs`; the remaining headers must also match after excluding build stamps.

| Comparison | Pairs / seeds | Median CPU per turn | Pooled CPU per turn | Turns per arm |
| --- | --- | --- | --- | --- |
| Tournament baseline against itself | 1 / 9134999 | +1.46% | +1.46% | 150 |
| Tournament candidate | 4 / 9135000–9135003 | −1.09% | −1.25% | 823 |

Every candidate pair used less CPU, with deltas from −2.07% to −0.39%, and every
recorded outcome agreed. This small tournament sample suggests a modest saving;
its single control also shows why sub-percent differences need caution.

The independent [GitHub native timing run](https://github.com/MartinHalvorson/CIVVIS/actions/runs/34810708102/job/103871317752)
measured −1.99% median CPU per turn across five 120-turn pairs, with ±0.45%
resolution and matching reports for all 600 turns per arm. Its native controller
configuration differs from the tournament's, so the two measurements remain
separate. Raw tournament rows, the preregistered manifest, pair timings, and the
measurement script remain in the local run directory under `0913-culture-*` and
`0913-tournament-speed.py`.

## Validation

After integrating main `3f14b4484`, the full locked CI-profile Rust suite passed: 3,472
library tests, 49 ignored, plus binary, integration, protocol, and documentation
targets. Existing tests cover a moving rival culture bar, the closing game clock,
gene gating, and adaptive Culture spending.
