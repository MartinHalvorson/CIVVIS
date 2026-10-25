# Gene rewrite closeout

The operator requested wrap-up and submission after the three-hour rewrite
session. Work stops with three implemented challengers, two independent engine
repairs, three complete family probes, and the explicitly partial registry
results below. No further long screen or age-closer replay is scheduled by this
session. The original registered sample sizes and seed windows remain recorded;
the stopped prefixes are not completed experiments.

## Submitted changes

| Change | Submission | Resulting behavior |
| --- | --- | --- |
| Luxury research v2 | [#3567](https://github.com/MartinHalvorson/CIVVIS/pull/3567) | Price a legal, missing luxury connection by remaining Science and current Amenity need. |
| Government ladder v3 | [#3573](https://github.com/MartinHalvorson/CIVVIS/pull/3573) | Price the full missing civic path per extra policy slot and leave time to use it. |
| Age closer v2 | [#3575](https://github.com/MartinHalvorson/CIVVIS/pull/3575) | Verify that affordable patronage actually reaches the age threshold before its deadline. |
| Builder ownership repair | [#3578](https://github.com/MartinHalvorson/CIVVIS/pull/3578) | Refuse an improvement when its owning-city handle is stale, without spending a charge. |
| City-transfer repair | [#3581](https://github.com/MartinHalvorson/CIVVIS/pull/3581) | Skip units already removed recursively with their carrier during city evacuation. |

The challengers remain off. These changes retain luxury v1 and government v2
deployment, and both age-closer versions remain off. No measured improvement is
established by this session.

## Completed family probes

Each probe completed all twelve registered games and 72 seats. The versions
were mutually exclusive and other genes stayed at the evaluator baseline.
Reported differences are challenger minus predecessor, in percentage points;
each ± is one standard error clustered by game, not a confidence interval.

| Family | Wins / seats by arm | Win difference | Score-share difference |
| --- | --- | ---: | ---: |
| Luxury | off 2/17; v1 7/37; v2 3/18 | −2.25 ±11.63 | −3.03 ±1.51 |
| Government | off 0/11; v1 6/21; v2 4/32; v3 2/8 | +12.50 ±18.51 | +2.64 ±2.30 |
| Age closer | off 10/57; v1 2/10; v2 0/5 | −20.00 ±13.00 | −0.38 ±2.32 |

The government probe replayed all twelve original seeds after the Builder
repair. Its failed first attempt (4/12 games) stays separate and is not pooled.
Age-closer's five challenger seats and zero wins make its reported uncertainty
especially fragile; the narrow v2-versus-off standard error is not a precise
strength estimate. See the original plans, limitations and complete artifacts:

- [Luxury note](2026-09-14-luxury-research-value.md) and [family analysis](../gene_screens/fires/connect-the-luxury-2.json).
- [Government note](2026-09-14-government-ladder-route.md) and [family analysis](../gene_screens/fires/government-ladder-3.json).
- [Age-closer note](2026-09-14-age-closer-deadline.md) and [family analysis](../gene_screens/fires/age-closer-2.json).

## Partial whole-registry results

Luxury and government were stopped at the operator's wrap-up request, before
inspecting their partial outcomes. Age-closer had already failed on the
city-transfer missing-unit panic. The engine repair is submitted separately;
its regression tests do not turn the failed run into a completed screen. The
operator's wrap-up request cancels the proposed fresh replay.

| Family | Completed / registered games | Completed seats | Reserved seeds | Stop reason |
| --- | ---: | ---: | --- | --- |
| Luxury | 85 / 120 | 510 / 720 | 914357000–914357119 | Operator wrap-up |
| Government | 42 / 60 | 252 / 360 | 914358000–914358059 | Operator wrap-up |
| Age closer | 20 / 36 | 120 / 216 | 914359000–914359035 | Engine panic |

Every represented game contains six complete seats. Played prefixes end at
914357084, 914358041 and 914359019 respectively. These independent-genome
whole-registry samples are kept separate from the family probes and each other.
They used the standard six-player, 74×46 Continents, nine-city-state, Online
250-turn Emperor profile, observed-player-v1, native competitions and all seven
target lanes. Every JSON retains `batch.partial: true` and its original target.

| Family | Wins / seats by arm | Challenger minus off | Challenger minus predecessor | Score share minus predecessor |
| --- | --- | ---: | ---: | ---: |
| Luxury | off 21/120; v1 38/233; v2 26/157 | −0.94 ±4.65 | +0.25 ±3.72 | −0.37 ±0.54 |
| Government | off 14/82; v1 8/31; v2 12/105; v3 8/34 | +6.46 ±8.29 | +12.10 ±8.59 | +0.46 ±1.08 |
| Age closer | off 15/89; v1 3/16; v2 2/15 | −3.52 ±9.18 | −5.42 ±14.85 | −0.57 ±1.60 |

Differences and game-clustered standard errors are descriptive, exploratory
outputs from incomplete registered samples. Neither a favorable sign nor the
reported error bars establish a strength improvement or justify promotion.

## Preserved evidence and provenance

The analyses below are byte-for-byte copies of the analyzer outputs. Each
provenance file records the raw-row, log, analysis and immutable-binary SHA-256,
the unchanged header, original target, stop reason and independent protocol
audit. Raw rows, logs and immutable binaries remain under the recorded local
archive paths in `/Users/martbot/civvis-runs/`; they are not embedded in this PR.

- Luxury: [partial analysis](../gene_screens/fires/connect-the-luxury-2-registry-partial.json), [provenance](../gene_screens/fires/connect-the-luxury-2-registry-partial-provenance.json).
- Government: [partial analysis](../gene_screens/fires/government-ladder-3-registry-partial.json), [provenance](../gene_screens/fires/government-ladder-3-registry-partial-provenance.json).
- Age closer: [partial analysis](../gene_screens/fires/age-closer-2-registry-partial.json), [provenance](../gene_screens/fires/age-closer-2-registry-partial-provenance.json).

Luxury's registry header has a **blank commit** and `unstamped-tree-moved`:
the source worktree advanced before that phase launched. It has not been
silently restamped. Its binary and registry hashes exactly match the clean
family run at `186af78f11fc79fde4cb9680b38d1b5645650112`. Government's clean
header is `6614d5a3e6ef4bfbd8eee60ef2f750f589c05469`; age-closer's explicitly
stamped clean header is `9d35d6c6271878ca2937294574a15bfd1898a796`. These are
the measured revisions, not the later integration or squash revisions.

The feature notes retain their exact validation scope. Full local Rust suites
passed for all three features, and both engine repairs have regression tests.
The city-transfer repair passed 3,779 tests with 53 ignored at its recorded
integration revision, plus the native paired-cost gate. This closeout changes
documentation and preserved analysis artifacts only.
