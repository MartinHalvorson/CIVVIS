# Native strike-refusal scan cost

The candidate used **5.71% less child CPU** by the median paired comparison (5.63% pooled) on this fixed CI trace. Every registered acceptance criterion passed; the iterator change is retained.

The experimental change replaces the strike-refusal reader's full log walk and up to two substring checks per line with two calls to the existing incremental event-line index. It preserves the same raw needles, JSON parser, exact event kinds and turn, air-attack exemption, unit and coordinate conversion, and returned sorted set. A line mentioning both event words can be visited twice; repeated insertion into the set is idempotent and event order does not choose a winner. No new cache implementation is added: the existing log index also keeps the two refusal needles. Game and AI state, genes, rules, defaults, Native pins, forcing, and verification policy are unchanged.

## Lead and fixed study

A diagnostic on integrated source `30f46ac72c7edb16296239081e9fd13f1d300803` completed 41 captured requests with exact actual input reads. Its eight-second call graph overlapped an owned compiler. Two inlined `state_from_events` substring stacks had 108 and 101 samples; attribution to the reader's two remaining full-log substring checks is a source-based inference. The trace contains fourteen `range_attack_refused` lines totaling 5,456 bytes and no `war_refused` line, out of 61,648,701 bytes. This profile and census motivate the change; they do not measure its gain.

The study was registered before experimental source edits and baseline compilation. The test-only baseline includes the unchanged #4072 production repair in both arms. Two controls pass before the optimization, and the baseline decider is built, copied, and hashed before the experimental edit. The candidate then passes focused, mirror, full serial, all-target developer-tools, and changed Rust quality checks. Exact source and binary hashes are frozen through measurement.

The fixed workload is the 41 original increasing opening-input prefixes, turns 160–200 of `civvis-20261015T120650Z`, with the same 283 common Native flags. Each arm starts a fresh persistent `civvis_orders --serve --fresh-board --explain` process, with input capture enabled. The design fixes eight baseline/baseline pairs followed by sixteen baseline/candidate pairs, balanced and reversed over two interleaves: 48 arms and 1,968 responses. Startup and the first request are included. Input-capture syncing remains enabled and unchanged.

Child user-plus-system CPU is measured with `RUSAGE_CHILDREN` around one child per arm. Parent-side appends, hashing, JSON comparison, and captured-read audits are outside that CPU clock. Wall and request latency are retained without a speed claim. No arm, seed, or outlier is removed or replaced, and the block is not extended after inspection.

All 48 registered arms completed. All 1,968 full parsed responses are identical. A/A median +0.020854%, pooled +0.095137%, IQR 0.416668 percentage points. A/B median -5.709661%, pooled -5.629833%, IQR 0.352758 points. All six registered acceptance criteria passed.

Acceptance requires every full parsed response to match, excluding only top-level `input_capture` metadata; every actual captured read must reconstruct the registered original prefix. Both median and pooled A/B differences must be negative. The absolute A/B median must exceed the absolute A/A median, the A/A IQR, and the A/B IQR. These are conservative local resolution checks, not a confidence interval for gameplay.

## Validation, provenance, and limits

Two added controls and 442 mirror tests passed (two existing ignores). The full serial suite passed 6,131 tests, with 51 existing library ignores and four doc ignores; all-target developer-tools checking passed. These semantic checks completed on first candidate `bcf06e9da70a4fbbb27c237b66f22589111e16df`. Its Rust quality check then failed only the for-expression line wrapping. The formatter-only correction `d2d71d7cb1e940a6d4e8bf53b6dd41d290f5791b` preserves every non-whitespace source byte; its quality check and rebuilt decider pass. Completed semantic checks were retained without replay, with the exact correction and first binary/logs preserved. The fixed study measures the rebuilt formatted source, not the first binary. The correction preceded all timing arms and changed no command, case, metric, criterion, or registered count.

The added controls cover overlapping needles, misleading and malformed records, duplicate JSON keys, exact kinds and turns, the existing air-attack exemption, CRLF and an unterminated line, growth, same-size rewrites, and turn lookback. Existing current-turn refusal and air-retry controls remain in the mirror suite. The experiment changes only the prefilter iterator; parser and refusal policy remain byte-for-byte unchanged.

Measurement began only after #4072 finished publishing, its clean public-worktree cleanup, all owned heavy validations, no active Rust compiler, and the Native priority boundary. Normal background services continue. The queued Native studies retain their existing sources and order.

This is the CI profile on ARM64 macOS 27.0.1 with Rust 1.97.1. The 41-input trace comes from **one historical world**. Repeated runs improve local CPU precision; they are not independent maps or Native strength games. Neither arm executes an order in Civilization VI. No release-binary speed, overall Native game speed, victory, or strength improvement is established.

The production-compatible `--serve --fresh-board` path preserves the agent; a late first observation skips the opening book and adopts current host production (`src/bin/civvis_orders.rs:10518`). Cold one-shot diagnostics deliberately start with empty production ownership and release foreign choices once (`src/bin/civvis_orders.rs:10358`). The separate cold queue proposals recorded with #4072 therefore do not by themselves establish production-mode queue cancellation. Every CPU arm here uses the same persistent mode; this is not a comparison between those initialization paths.

The already committed [original full input stream](../performance/2026-10-15-native-base-world-cost.input.events.jsonl.gz) is reused byte-for-byte rather than duplicated. The new summary records its compressed and uncompressed hashes, and the registration names every prefix byte count and SHA-256.

- [Summary and independent audit](../performance/2026-10-15-native-strike-refusal-scan.json)
- [Test support, experimental, and formatter patches](../performance/2026-10-15-native-strike-refusal-scan.source-patches.json.gz)
- [All 48 raw metric receipts](../performance/2026-10-15-native-strike-refusal-scan.runs.jsonl.gz)
- [All 1,968 raw responses](../performance/2026-10-15-native-strike-refusal-scan.responses.json.gz)
- [All captured-input indexes](../performance/2026-10-15-native-strike-refusal-scan.capture-indexes.json.gz)
- [Registrations, source/binary hashes, harnesses, validation logs, receipts, and ambient observations](../performance/2026-10-15-native-strike-refusal-scan.provenance.json.gz)
