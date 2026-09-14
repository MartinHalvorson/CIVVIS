# Derive improvement passage only for blocking terrain

`class_can_traverse` previously checked an improvement's passage effect before
it knew whether the tile's terrain would block movement. Improved plains thus
read or built a passage table and cloned its `Arc` even though the answer could
not affect the result. The query now honors unknown-map priors first, then asks
about passage only when ordinary passability and the mountain-worker exception
both fail. Water, ocean, embarkation, city, canal, and wonder checks retain their
existing order and behavior after that gate.

## Validation

The full locked CI-profile Rust suite passes: 3,464 library tests, 49 ignored,
plus binary, integration, protocol, and documentation targets. The new movement
matrix checks all 16 traversal classes against every stock improvement, pillaged
and intact, on plains, mountains, coast, ocean, ice, and the mirror's unknown
terrain with both frontier priors. A second test checks that ordinary improved
tiles leave the passage table unbuilt while intact mountain tunnels still grant
passage and pillaged tunnels do not.

The paired simulation comparison uses `tools/speed_ab.py`, the
saved CI-profile luxury build `b4a5a4421` (the code in main `9c85331a0`), and
candidate `a83431da6`, at six players, 74x46 Continents, nine city-states, Online,
one job. No local compilation ran during the measurements.

| Turn cap | Pairs / seeds | Median CPU per turn | Resolution | Completed turns per arm |
| --- | --- | --- | --- | --- |
| 120 | 8 / 9133500–9133507 | −2.00% | ±0.50% | 960 |
| 250 | 4 / 9133600–9133603 | −1.99% | ±0.34% | 984 |

Every game report agrees: 1,944 completed turns per arm across twelve pairs.
Every pair used less CPU in the candidate. Pooled savings are 2.15% and 1.95%
respectively; load stayed below 1.83. A preceding four-pair comparison of the
baseline with itself read +0.15%, inside the harness's noise floor, with ±0.92%
resolution. These are local native-simulator measurements, not a tournament or
cross-machine speed guarantee. Logs are retained in the local run directory as
`0913-passage-quiet-120.log` and `0913-passage-quiet-250.log`.
