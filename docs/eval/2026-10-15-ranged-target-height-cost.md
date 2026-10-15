# Ranged target height: remove duplicate district lookups

Computer: MacBook Pro

A ranged target stands at its terrain and feature height. `tile_has_strike_line` previously obtained `sight_height(to)`, which adds the district viewpoint bonus, then subtracted that same bonus. This repeats the district lookup for every candidate ranged shot. The valid-tile path now reads terrain plus feature height directly. The missing-tile path keeps the original expression, including its behavior for stale city references. Viewer elevation, cover, the two corridors, Sentry and range rules retain the same calculation.

A three-second sample during the preceding runtime soak identified this path; that sample alone was not a speed measurement. The fixed timing study was registered before the candidate edit or any timing outcome: eight A/A control pairs, followed by sixteen A/B pairs. Each block alternates execution order per seed and reverses it on the second interleave. Both immutable executables use the optimized `ci` profile, locked dependencies, the same owned target directory and one simulation job. All games use six majors, 74×46 Continents, nine city-states, Online speed, Planet shape and a 250-turn cap.

| Block | Distinct seeds × interleaves | Median candidate CPU/turn change | IQR | Tool resolution |
| --- | --- | ---: | ---: | ---: |
| Same-binary A/A | 4 × 2 | −0.08% | 0.39 percentage points | ±0.20% |
| Candidate A/B | 8 × 2 | −0.48% | 0.79 percentage points | ±0.29% |

The A/A median is noise. The A/B block measures a small local improvement: pooled CPU/turn also falls 0.48%, with 3,438 completed turns in each arm. Every terminal report agrees. Individual A/B pairs range from −2.96% to +1.23%; all are retained. Host load averaged about 12 at the start and peaked at 14.68. These are eight distinct workloads measured twice, not sixteen distinct worlds. The tool's resolution is twice its robust standard error for the median; it is not a Native win-rate interval.

The measured baseline is `4b27e9e5ebcc5d9aa8216a988b47dd3dcdc7cca4`, production-equivalent to main `f3890953b786be0189b620ad2123b7f56a3d9beb`. The candidate is `3fa46a65181ae656ac8682df94abda39bd61b51d`. Baseline binary SHA-256 is `a549ef8ca319ded23c6d745d42ec41909b2d54b914872fccf5407ae2fca31764`; the candidate hash and every raw pair are in the [measurement record](../performance/2026-10-15-ranged-target-height-cost.json). Source bytes remain fixed during each build and throughout timing. No game, seed or outlier is replaced, and no global performance ledger is changed.

The existing 33 visibility controls pass, including the native-derived hills-city shot refusal. The full pre-integration serial suite passes all 6,080 tests and its frozen legacy action-stream guard, with 51 library and four documentation tests intentionally ignored. The original driver ended after that receipt and before timing began; only the unplayed registered timing blocks were resumed, with output preserved to files.

Main `94fad6d71` is merged once after both timing blocks finish, so the final branch also includes the separately audited neutral-improvement rules correction. The measurement above describes the earlier matched pair; it does not measure that rule change or assert a release-profile speedup. Final integration validation and its independent cost gate are recorded in PR #4066. No AI selection, default, Native policy or game behavior is intentionally changed.
