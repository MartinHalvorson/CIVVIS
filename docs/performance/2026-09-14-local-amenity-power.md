# Lazy local amenity power queries

Local amenity evaluation used to ask whether a city was powered for every active nonregional building, including buildings with no powered amenity bonus. The query now checks for that bonus first. It also rejects regional buildings before asking about their district activity. Amenity contributions retain their original order and values.

## Validation

Source `aa9e1b76003d76d3137dac866844028a1a059a80` passed 3,676 CI Rust tests (49 skipped), 71 tournament tests, 21 live-divergence tests, and documentation checks. The new Shopping Mall regression follows power loss, building pillage, district pillage, and repair through uncached and repeated cached queries. Existing tests compare regional effects against the original algorithm for every stock building.

[CI validation](https://github.com/MartinHalvorson/CIVVIS/actions/runs/34814535152)

## Native paired timing

The frozen baseline is `7cfc65a953704b3aa14567ce7775b7079cf7483c`, whose build inputs match main `91f99fe7c`. The candidate is the source above. Both use the `ci` profile and the native simulator's six-major, 74×46 Continents, nine-city-state, Online configuration. Each game runs with one job, arms alternate by seed, and no local compilation or other simulations ran during timing.

| Seeds | Pairs / turn limit | Completed turns per arm | Median CPU change | Measured resolution |
|---|---:|---:|---:|---:|
| 9136300–9136307 | 8 / 120 | 960 | 0.00% | ±0.24 pp |
| 9136400–9136403 | 4 / 250 | 926 | -0.40% | ±0.57 pp |

All 1,886 completed turns per arm produced identical reports. Early timing was unchanged. All four late pairs used less CPU, but the median reduction was within the run's measured noise. Independent CI timing measured -0.48% across five 120-turn pairs, below its 1% noise threshold. This is a cleanup of redundant queries; these samples do not establish a speed gain.

[CI timing](https://github.com/MartinHalvorson/CIVVIS/actions/runs/34814535154)

The preregistered counts and seeds, raw logs, and binary hashes are retained under `civvis-runs/0913-local-amenity-*`.
