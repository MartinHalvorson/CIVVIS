# Dense land-siege exclusions: fixed trial withdrawn

The dense exclusion lookup is **not retained**. Its apparent CPU reduction did not exceed either registered noise-spread threshold. The final production inputs are byte-identical to the original baseline; this change retains two test-only compatibility controls and the complete negative study.

The proposed optimization converted the already constructed land-siege exclusion set into a query-local tile-indexed lookup. It preserved the ordered set for unindexable positions, left non-land routes unchanged, and did not change routing, strategy, genes, rules, forcing, the play pin, or verification policy. The exact candidate patch and all source/binary hashes are archived.

## Fixed comparison

Registration preceded the production edit, both local executable builds, and every timing arm. The baseline is `382743a90`, which adds the two controls to unchanged production; the measured candidate is `045dc576a`. Both use the same local task target, rustc 1.97.1, and `cargo build --profile ci --locked --bin civvis_orders -j 4`. This is the **local CI profile**, not the Native ARM64 artifact profile or the full production release profile. Both children use the same hash-checked runtime data and working directory.

The fixed workload is the same 41 original increasing opening prefixes, turns 160–200, from historical Emperor world `civvis-20261015T120650Z`. The original 61,648,701-byte input has SHA-256 `e3439f662dc59d2f1e4fc6fbd1c0d7ff9caeca9ba4d705995d9ddce2c83464f8` and is reused from its existing committed archive. Both arms use `--serve --fresh-board --explain`, the Domination lane, the same 283 flags, and actual input capture. Exact arguments and all prefix hashes are retained.

Eight A/A pairs precede sixteen A/B pairs. Each phase has two alternating interleaves with reversed first-arm order in the second interleave: 48 fresh persistent children and 1,968 responses. The metric is child user plus system CPU, including startup and capture writes; parent input appending, hashing, reconstruction, and response audits are excluded. Normal host services and verification games continue, with ambient processes recorded at pair boundaries. No owned build, test, game, or profiling workload runs during timing. All arms are retained without replacement, filtering, extension, or an outcome-based retry.

| Comparison | Median paired CPU delta | Pooled CPU delta | Paired-delta IQR |
| --- | ---: | ---: | ---: |
| A/A | -0.686304% | -0.950030% | 2.166701 points |
| A/B | -1.537621% | -1.157692% | 2.376991 points |

The A/B median and pooled deltas are negative, and the median magnitude exceeds the A/A median magnitude. It does **not** exceed the A/A IQR or the A/B IQR. Those two failures reject the candidate under the registered rule. This study establishes no reliable speed improvement; the code was withdrawn rather than tuned or retested against the same workload.

## Compatibility and validation

Every full parsed response matches the baseline series after removing **only** top-level `input_capture` metadata. A separate completed-study audit independently compares all 1,968 responses, reconstructs all 1,968 actual captured reads, verifies every physical capture archive against the original stream, and recomputes both sets of CPU statistics. It also verifies that all final production inputs match the baseline, excluding only the named `cfg(test)` support file.

The two added controls cover sparse grids, unindexable positions, stacked units, friendly civilian and hostile aircraft occupancy, and the water-goal exception. Both pass against unchanged production before the edit. Together with the five existing oracle controls they preserve the independently unchanged set-based router as the reference and require real reachable routes rather than only unreachable comparisons.

Both the measured candidate and the final withdrawal pass all 6,149 tests across targets with zero failures and 55 existing ignores. The developer-tools all-target check and the changed-line Rust quality gate also pass. Exact validation receipts are retained. The initial whole-tree format check also found unrelated pre-existing formatting differences; only this task's changed files were corrected, and their scoped formatting check passes.

This is one captured historical world, not a fresh Native game or a host continuation. It establishes no Deity victory, conquest-strength gain, or overall game-speed improvement. No additional compatibility-game cohort is claimed. Existing verification games and their settings remain untouched.

## Artifacts

- [Registration, result, source identities, validation, independent audit, and artifact hashes](../performance/2026-10-16-dense-siege-route-exclusions.json)
- [All 48 raw CPU receipts](../performance/2026-10-16-dense-siege-route-exclusions.runs.jsonl.gz)
- [All 1,968 full raw responses](../performance/2026-10-16-dense-siege-route-exclusions.responses.json.gz)
- [All actual captured-input indexes and physical archive hashes](../performance/2026-10-16-dense-siege-route-exclusions.capture-indexes.json.gz)
- [Complete source manifests, candidate patch, registration, harnesses, ambient observations, audits, and validation logs](../performance/2026-10-16-dense-siege-route-exclusions.provenance.json.gz)
- [Reused original input stream](../performance/2026-10-15-native-base-world-cost.input.events.jsonl.gz)
