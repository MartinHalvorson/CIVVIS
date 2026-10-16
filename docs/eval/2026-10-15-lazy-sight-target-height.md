# Deferred sight target heights: unresolved cost study

The proposed runtime refactor is withdrawn. It deferred target-height calculations until the existing adjacent-ray fast paths had returned. The registered comparison did not separate its cost from measurement noise; no speed improvement is claimed.

Baseline `6fee5b42746d141b470858f85d56d5ab5e630dfd` is runtime-equivalent to main `3657b284c208fccef39db6ad3887e4cb7067adc0`. Candidate `77f04e97b8ee0d7e6c4fcf5abeef583f17375d2a` changed only the two private sight callers and their shared corridor helper, using a lazy height calculation. It preserved the original terrain, feature, district and missing-tile arithmetic. The complete patch, registration, receipts and all measurements are in [the raw study](../performance/2026-10-15-lazy-sight-target-height.json).

Before editing, the plan fixed eight A/A pairs (four seeds, twice) and sixteen A/B pairs (eight seeds, twice), alternating the first arm and reversing the order in the second interleave. Each game used six players, a 74×46 flat Continents map, nine city-states, Online speed, a 250-turn cap and the locked CI profile with one worker. These are eight distinct A/B worlds, not sixteen independent worlds.

| Block | Pairs | Median CPU change | Pooled CPU change | IQR | Tool resolution |
|---|---:|---:|---:|---:|---:|
| A/A | 8 | -0.197719% | -0.406365% | 0.646109 pp | ±0.338672% |
| A/B | 16 | -0.026339% | -0.089106% | 0.612472 pp | ±0.227010% |

All terminal reports matched: A/A ran 1,930 turns per arm and A/B ran 3,576 per arm. Every registered pair is retained. The acceptance rule required negative median and pooled A/B changes, and an absolute median larger than both the absolute A/A median and the A/B tool resolution. Both magnitude thresholds failed. The study was not extended or rerun.

Candidate validation passed all 33 visibility controls, the full 6,106-test suite, the all-targets check, the existing deterministic game anchor and the changed-line Rust quality gate. The initial repository-wide formatting check reported unrelated existing debt; the changed file and changed-line gate passed. No rule, AI strategy, ledger, global verification pin or policy was changed.

The machine retained its existing background workloads. A read-only sampler preserved compiler observations. Two roughly seven-second append-point checks for another task overlapped early A/A, and ordinary metadata/hash checks continued; these activities are recorded in the raw study. No process was stopped to improve a sample. The next task's local compiler waited for this driver and every timing child to finish.

The final runtime files match main `3657b284c208fccef39db6ad3887e4cb7067adc0` byte for byte; this PR publishes only this note and its raw study. Candidate `77f04e97b8ee0d7e6c4fcf5abeef583f17375d2a` remains preserved in the PR's history. These measurements concern this local flat CI workload and establish no Native or release-profile speedup, conquest gain or Domination result.
