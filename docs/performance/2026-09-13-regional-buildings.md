# Regional building query prefilter

`regional_building_effects_uncached` now derives a building's effective range
before checking its district activity and locating its district tile. Buildings
with no reach skip those queries. Mexico City's range bonus is included before
the rejection, preserving the existing behavior for buildings whose base range
is zero. Pillage handling, source iteration, distance checks, grouping, powered
yields, and floating-point accumulation retain their previous behavior.

## Paired measurements

The quiet comparison uses CI-profile binaries with the same baseline as main
`9c85331a0`: the saved luxury-optimization build `b4a5a4421` versus regional
commit `89d1db705`. `tools/speed_ab.py` alternates arms and verifies report
digests, at six players, 74x46 Continents, nine city-states, Online, one job.

| Turn cap | Pairs / seeds | Median CPU per turn | Resolution | Completed turns per arm |
| --- | --- | --- | --- | --- |
| 120 | 8 / 9133200–9133207 | −0.64% | ±0.20% | 960 |
| 250 | 4 / 9133300–9133303 | −0.89% | ±0.45% | 746 |

The 120-turn block's pooled saving is 0.42%, with load 2.01–2.10–1.48.
The 250-turn block's pooled saving is 1.10%, with load 1.48–1.72–1.72.
Every game report agrees: 1,706 completed turns per arm across the twelve
pairs. These are modest local savings, not a cross-machine speed guarantee.
A preceding four-pair comparison of the baseline with itself read +0.15%,
inside the harness's noise floor, with ±0.92% resolution.

Earlier measurements overlapped compiler bursts and could not resolve a speed
change: their resolutions were ±7.59% at 120 turns and ±12.96% at 250 turns.
They establish matching game reports, not a performance improvement.

## Correctness

The tests retain the original loop as an oracle and compare every stock
building with Mexico City enabled and disabled, multiple target distances,
building and district pillage, and powered and unpowered sources. A separate
test checks query-scope expiry across factory pillage and repair.

After integrating main `9c85331a0`, the full locked CI-profile Rust suite passed:
3,464 library tests, 49 ignored, plus binary, integration, protocol, and
documentation targets. Raw timing logs are retained in the local run directory
as `0913-regional-quiet-120.log` and `0913-regional-quiet-250.log`.
