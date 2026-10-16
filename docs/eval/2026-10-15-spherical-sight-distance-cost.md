# Spherical sight distance reuse: unresolved fixed cost study

Computer: MacBook Pro

A three-second sample in the preceding runtime soak showed spherical graph-distance and visibility work. That sample was a lead, not a speed measurement. The proposed change moved the spherical adjacency shortcut into the arc routine so one graph-distance query served both adjacency and interpolation steps. Arena and cylinder calculations retained their existing paths.

The fixed study was registered before the candidate edit or any timing outcome: four same-binary control seeds measured twice (eight A/A pairs), followed by eight candidate comparison seeds measured twice (sixteen A/B pairs). Both executables use the optimized CI profile, locked dependencies, one owned target directory and one simulation job. Every game is six majors, 74×46 Continents on Planet, nine city-states, Online speed and a 250-turn cap. Per-seed arm order alternates and reverses on the second interleave.

| Block | Distinct seeds × interleaves | Median CPU/turn difference | Pooled difference | IQR | Tool resolution |
| --- | --- | ---: | ---: | ---: | ---: |
| Same-binary A/A | 4 × 2 | +0.40% | +0.23% | 1.38 points | ±0.72% |
| Candidate A/B | 8 × 2 | −0.17% | −0.26% | 0.90 points | ±0.33% |

This does not meet the preregistered threshold. The absolute A/B median is below both the A/A median and the A/B tool resolution. All terminal reports agree: 1,934 completed turns in each A/A arm and 3,588 in each A/B arm. Individual A/B differences range from −2.18% to +1.99%; none is removed. Eight workloads measured twice are not sixteen independent worlds.

The host's managed production compiler ran during the latter A/A and early A/B measurements; normal Native UI recovery probes also occurred during A/A. The speed tool's load readings and the read-only compiler observations remain in the [complete measurement record](../performance/2026-10-15-spherical-sight-distance-cost.json). The same-binary control shows substantial noise. No block is extended or replayed in pursuit of a favorable result.

The measured baseline is `4f2ca63428215aa8e7112b87a89d8e590f6180ab`, production-equivalent to `2e21bd84b`. The measured candidate is `70a07bef0444dcb413dbf0bd5461e53e9c4e4823`. Its 33 visibility controls, full 6,096-test suite, legacy action-stream guard and all-target check pass with frozen source. Immutable binary hashes, every pair, exact commands and the candidate patch are retained in the record and GitHub branch history.

The runtime patch is withdrawn. After the one main merge, runtime source and data match `371bada5a` exactly; this PR publishes only the study record. There is no measured speed gain, release-profile result or Native conquest result to promote. The prior separately measured ranged-target-height change remains independent of this unresolved study.
