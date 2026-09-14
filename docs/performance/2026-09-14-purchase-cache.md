# Hash the purchase-price memo

The purchase-price memo now uses `HashMap` for its existing key: player, city,
interned unit kind, formation, and currency. `Name` hashes its integer identity;
ordered comparisons instead consult its ruleset text. Only exact-key lookups
read this private cache. Purchase action order still comes from the unchanged
ruleset sweep, and the cache is excluded from game serialization. Its read-only
query scope, invalidation, and quote derivation are unchanged.

## Paired measurements

`tools/speed_ab.py` compared clean CI-profile candidate `c663f85e7` against the
saved luxury build `b4a5a4421`, whose code is main `9c85331a0`. Each pair uses
six players, 74x46 Continents, nine city-states, Online, one job, with alternating
arm order. No local compilation ran during the timings.

| Turn cap | Pairs / seeds | Median CPU per turn | Resolution | Completed turns per arm |
| --- | --- | --- | --- | --- |
| 120 | 8 / 9133700–9133707 | −1.50% | ±0.56% | 960 |
| 250 | 4 / 9133800–9133803 | −1.51% | ±0.71% | 904 |

Every report agrees, and every pair used less CPU in the candidate: 1,864
completed turns per arm across twelve pairs. Pooled savings are 1.56% and 1.54%
respectively. The 120-turn block's load fell from 4.01 to 1.51; the 250-turn
block stayed below 1.65. The preceding four-pair same-binary control read +0.15%,
inside the harness's noise floor, with ±0.92% resolution. These are local native
simulator readings, not measurements of the gene-tournament configuration.

## Correctness

The existing purchase-price tests compare cold and warm menus in their exact
action order and compare memoized quotes against fresh derivations for every
unit, formation, currency, and fixture city. They also check query-scope expiry,
direct occupancy changes, queue reservations, and host purchase refusals.

After integrating main `1873ae903`, the full locked CI-profile Rust suite passes:
3,470 library tests, 49 ignored, plus binary, integration, protocol, and
documentation targets. Raw logs are
retained as `0913-purchase-cache-ab-120.log` and `0913-purchase-cache-ab-250.log`
in the local run directory.
