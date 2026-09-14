# Shared and empty Great Work allocations

City yield evaluation asks for one city's Great Works, but the old query copied the entire empire's ordered allocation. Tourism and theming also rebuilt the empire's piece metadata. Both queries now share `Arc` snapshots for the lifetime of the existing read-only query scope.

The common empty case avoids slot allocation entirely when neither named works nor legacy Artists exist. Empty snapshots are shared, and metadata grouping stops when there is no housing. A single ordered table supplies both the kind names and their counter keys, removing repeated key formatting. Host-observed housing still takes precedence, and the original legacy allocation path remains available.

Nested queries reuse the outer scope. Dropping it clears both caches; retained snapshots cannot keep expired answers in the game. Theft and capture copy only the selected city's counts when ownership is needed. The allocation order remains a `BTreeMap` order.

## Validation

The revised source `76f3afd5774a0424c12d70499d3acb4b3f16b0d0` passed 3,678 CI Rust tests (49 skipped), 71 tournament tests, 21 live-divergence tests, and documentation checks. The seven focused local Great Work tests also passed. The new regressions cover ordered metadata, nested sharing, retained snapshots across pillage/repair, and the first work arriving after an empty query. Existing tests cover typed/universal housing, legacy Artists, mirror housing, theming, theft and capture.

[CI validation](https://github.com/MartinHalvorson/CIVVIS/actions/runs/34815570881)

## Native paired timing

Frozen baseline `7cfc65a953704b3aa14567ce7775b7079cf7483c` has the same build inputs as main `91f99fe7c`. The candidate is the revised source above. Both use the `ci` profile and the same native simulator settings: six majors, 74×46 Continents, nine city-states, Online, one job. Arms alternate by seed, and no local compilation or other simulations ran during timing.

| Seeds | Pairs / turn limit | Completed turns per arm | Median CPU change | Measured resolution |
|---|---:|---:|---:|---:|
| 9136000–9136007 | 8 / 120 | 960 | -0.38% | ±0.09 pp |
| 9136100–9136103 | 4 / 250 | 826 | -0.34% | ±0.49 pp |

All 1,786 completed turns per arm produced identical reports. The early-game reduction is small; the late-game estimate is within its measured noise. An independent CI sample of five 120-turn pairs measured +0.11%, within its 1% noise floor and ±0.47 pp resolution. These results do not establish a tournament speed gain.

[CI timing](https://github.com/MartinHalvorson/CIVVIS/actions/runs/34815573403)

Raw local logs and binary hashes are retained under `civvis-runs/0913-great-work-empty-*` and `0913-great-work-76f3afd57.json`. The preregistered run counts and seeds are in `0913-great-work-empty-timing-plan.json`.
