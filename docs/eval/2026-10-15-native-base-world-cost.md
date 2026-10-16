# Native constructor template cost

The isolated candidate used **6.57% less child CPU** by the median paired comparison (6.58% pooled) on this fixed CI trace. Every registered acceptance criterion passed. The implementation is retained with a fresh derived reach cache for each reconstructed board.

The candidate retains one untouched `Game::new` constructor result per thread, including the mirror-only unknown-terrain marker, and clones it before applying the observed board. Its key includes all variable constructor arguments: normalized player count, width, height, seed, and city-state count. Different setups replace the single slot. Live observations, strategic history, and partially reconstructed boards are not retained in the template.

## Registered workload and limits

The fixed workload consists of 41 original captured opening inputs, turns 160–200 of completed Native run `civvis-20261015T120650Z`. These are increasing byte-exact prefixes of the original captured event stream. Every run uses a fresh persistent `civvis_orders --serve --fresh-board --explain` process, the same 283 common Native flags, and input capture. The siege promotion experiment is absent from both measured binaries.

The 41-input trace comes from **one historical world**. Its repeated runs improve local cost precision; they are not independent maps, games, or Native strength trials. Neither arm executes its proposed orders in the host. These measurements use the CI profile on an ARM64 Mac; they do not establish release-binary speed, overall Native game speed, or a victory improvement.

The original registration preceded source edits, baseline compilation, and timing. The clean baseline was compiled and copied before candidate edits. The isolated variant was separately registered before its own edits and reused that unchanged baseline binary. The fixed design uses eight baseline/baseline pairs followed by sixteen baseline/candidate pairs, each in two interleaves with balanced order and the second interleave reversed. No run or outlier is removed or replaced, and the block is not extended after inspection.

Each arm measures child user-plus-system CPU with `RUSAGE_CHILDREN` around exactly one child process. Input appends, JSON comparison, and captured-input audit run in the Python parent. Wall time and individual response latency are retained without a speed claim. CPU measurements include the child's normal input capture and explanation output.

## Identity, exposure, and acceptance

All 48 runs of the isolated variant completed, with 1,968 identical full parsed responses. The independent audit also verified the earlier complete block: 96 runs and 3,936 matching responses in total. Isolated baseline/baseline median +0.010770%, pooled +0.327231%, IQR 0.725016 percentage points. Baseline/candidate median -6.566078%, pooled -6.583063%, IQR 0.351833 points.

Every parsed response is compared in full, excluding only its top-level `input_capture` path/read metadata. Decision records, orders, targets, notes, and turns remain in the comparison. Every actual captured read is reconstructed and checked against its registered original prefix. Source and binary hashes are checked before and after the full block.

The predeclared acceptance rule requires every response to match, negative median and pooled candidate differences, and an absolute median improvement larger than the absolute baseline/baseline median, the baseline/baseline IQR, and the baseline/candidate IQR. This conservative local threshold is not a general confidence interval for Native gameplay.

The first separately completed block also matched every response and met its timing thresholds (median −6.740428%, pooled −6.932355%). Source review then found that ordinary `Game::clone` shares an `Arc<Mutex<AttackReachCache>>`. Although this existing cache is bounded and guarded by a complete board fingerprint, it could fill the retained constructor template with derived observations, contrary to the narrow design. A valid regression observed one computation in a later returned board instead of zero. The separately preregistered correction uses `Game::clone_for_native_rebuild` to detach this cache; ordinary speculative game clones keep their existing sharing. All first-block data and its candidate patch are included. No run was replaced, discarded, pooled across variants, or used to extend a finished block. Only the isolated variant determines retention.

## Validation and provenance

Six constructor controls passed, including a derived-reach isolation regression: cold/warm serialized state and RNG equality, every valid key input, mutation isolation, alternating setups, and terrain revelation isolation. The initial successful-constructor fixture used an unsupported 1×1 map; the uncached constructor panicked before reaching the candidate. The corrected controls use valid dimensions and separately verify identical old/new panic payloads plus cache isolation after unwinding. The failed fixture log is retained and is not a behavioral regression result.

All 440 mirror controls passed, with two existing ignores. The full serial suite passed 6,112 tests, with 51 existing library ignores and four doc ignores. The Native decider build, all-target developer-tools check, and changed Rust quality passed on the measured source. No AI rule, default, forcing set, Native pin, or verification policy is changed by this experiment.

The initial CPU profile was a separate diagnostic lead on source `c0fe12f90d7b002f3990c57b8e3e581689a424d2` with the siege promotion flag enabled. It is not the benchmark baseline. Its idle stdin samples were excluded from the performance lead; existing incremental event-read caching was preserved. The actual baseline and candidate share the original 283 common flags and the same locked toolchain.

The timing boundary was audited after owned compilers and headless studies ended. The queued Native study remained unplayed at its first boundary wait, and a read-only screenshot confirmed the same stuck exit dialog. Normal background services and GitHub metadata/CI waiters continued. The harness checks Native priority before each pair.

- [2026-10-15-native-base-world-cost.input.events.jsonl.gz](../performance/2026-10-15-native-base-world-cost.input.events.jsonl.gz)
- [2026-10-15-native-base-world-cost.shared.patch.gz](../performance/2026-10-15-native-base-world-cost.shared.patch.gz)
- [2026-10-15-native-base-world-cost.isolated.patch.gz](../performance/2026-10-15-native-base-world-cost.isolated.patch.gz)
- [2026-10-15-native-base-world-cost.shared.runs.jsonl.gz](../performance/2026-10-15-native-base-world-cost.shared.runs.jsonl.gz)
- [2026-10-15-native-base-world-cost.isolated.runs.jsonl.gz](../performance/2026-10-15-native-base-world-cost.isolated.runs.jsonl.gz)
- [2026-10-15-native-base-world-cost.protocol.json.gz](../performance/2026-10-15-native-base-world-cost.protocol.json.gz)
- [2026-10-15-native-base-world-cost.responses.jsonl.gz](../performance/2026-10-15-native-base-world-cost.responses.jsonl.gz)
- [2026-10-15-native-base-world-cost.capture-indexes.json.gz](../performance/2026-10-15-native-base-world-cost.capture-indexes.json.gz)
- [2026-10-15-native-base-world-cost.provenance.json.gz](../performance/2026-10-15-native-base-world-cost.provenance.json.gz)
- [2026-10-15-native-base-world-cost.json](../performance/2026-10-15-native-base-world-cost.json)

Measured isolated source: `1b28f49b0dbe10d99f93a9c589d277e0dcea46fa`. Final integration checks are recorded in the pull request.
