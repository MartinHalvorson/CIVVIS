# Religious movement verification and threat-query cost

## Scope

This task keeps the existing `spreader-steps-out-of-the-apostle` strategy and
its movement, combat previews, tie-breaking, journal messages and gene settings
unchanged. It investigates two redundant costs: boolean safety queries count
every winning rival even after the first one establishes danger, and the flee
fallback counts threats at its unchanged starting tile twice.

The proposed implementation shares the existing per-rival predicate between
the full count and a short-circuiting safety query, and retains the initial
count for the fallback. No cross-frame cache or retained game-state reference
is introduced. A count is still required when ranking escape destinations.

## Registered checks

- Run the four existing Apostle-evasion tests on unchanged production first.
- Save unchanged-production simulator and orders executables before editing.
- Compare the safety query against an independent copy of the old count over
  every tile of a deterministic, overlapping-threat fixture, including changed
  unit health and an empty threat list.
- Run the focused religious tests and the complete locked CI-profile Rust suite.
- Compare complete parsed Native replies on 24 fixed opening prefixes, turns
  90–113, frame zero, of the completed Emperor game
  `civvis-20261016T153719Z`. Both executables receive the same unchanged local
  force list and use `--serve --fresh-board --explain`, Domination and Gran
  Colombia. Native CPU uses the actual fast release profile: 16 codegen units,
  LTO disabled, incremental enabled, Rust 1.97.1. The unchanged-production
  Native baseline is the frozen successor-game executable at main revision
  `44629581fd661fdf03f94a3801689f615c09e40b`, SHA-256
  `8d5c9f210c9abb26b6d75cce94899c75383a34deb66948db77a3a299f49851ed`.
  Its production source is byte-identical to the task's empty claim commit;
  no earlier timing observations are reused. The fixed schedule is four A/A
  pairs followed by eight balanced
  alternating-order A/B pairs. Child user plus system CPU includes startup and
  shutdown; parent prefix-copying time is excluded. All arms are retained.
- Report the native CPU distribution and A/A noise without interpreting an
  unresolved result as an overall game-speed improvement. Exact reply parity
  is mandatory. This replay supplies no win-rate estimate.
- Run separate fixed-seed simulator compatibility games against the saved
  unchanged-production executable and compare their complete non-timing output:
  seeds 40820001–40820003, four players, 44×28 Continents Flat, four city-states,
  Online speed, Deity/Deity, 160-turn limit and all six victory conditions. These are
  stock-controller compatibility checks, not gene-strength measurements.

## Live lane

The existing live game was preserved. The operator-authorized `civvis-games on`
command reset the next-batch pin from `civvis-pins/2b-live-a`, then 82 commits
behind `origin/main`, to `head`. The prior pin was backed up. Difficulty remains
Emperor, four-player Tiny Pangaea, Simón Bolívar and Domination; forced genes,
production-rank restart policy, capture settings and timeout policy are unchanged.
The active decider process remained alive after the command. It finished at
turn 200 with a rival Technology victory; it was not interrupted or counted as
an AI win.

The next launch correctly refused to overwrite five uncommitted paths in the
existing `civvis-verification-runtime`. That checkout was left untouched. With
the game lock free, a separate clean detached
`civvis-verification-clean-runtime` was created at fetched main, the backed-up
host policy's `CIVVIS_HEAD_REPO` was pointed at it, and the documented off/on
controls restarted the host at the no-game boundary. The successor game
`civvis-20261016T162332Z` read back revision
`44629581fd661fdf03f94a3801689f615c09e40b`; its play log confirms the top-two
Production restart policy and `deal_sessions=trade`. New batches track main.

## Results

The exact query simplification is retained. It stops at the first winning
rival when only safety is needed and reuses the unchanged origin count in the
flee fallback. Escape ranking still counts every rival. This is not a new gene,
a changed strategy, or evidence of a win-rate or whole-game speed improvement.

| Check | Result |
| --- | --- |
| Unchanged-production focused controls | 4 passed |
| Candidate focused controls | 6 passed; benchmark separately invoked |
| Final locked, serial CI-profile suite | 6,149 passed, 0 failed, 55 ignored including doctests |
| Census reporter regression suite | 29 passed |
| Changed-Rust format and all-feature clippy gate | Passed, changed lines warning-free |
| Simulator compatibility | All six arms match after removing only the timing line; games finish at turns 86, 151 and 73 |
| Native compatibility | All 576 complete parsed replies and all 24 journals match exactly |
| Original input and binaries | Unchanged; archived prefix independently hash-verified; both Native executables verified ARM64 |

The fixed query microbenchmark ran eight alternating-order pairs, 12,000
queries per arm, across twelve overlapping tiles reached by two winning rival
Apostles. Median paired elapsed time was **53.92% lower**, with individual
pairs ranging from 38.63% to 83.37% lower. These are deliberately short,
synthetic wall-time measurements on a live host, not Native frame timings.
The deterministic benefit is fewer predicate evaluations: the safety query
does not evaluate a second winning rival after the first already proves danger.

The Native CPU result is **unresolved**. All 24 children completed, giving
576 requests. Each arm produced 24 explanations from the active religious
evasion gene. A/A median was -0.5534%, pooled -1.3123%, IQR 3.6934 percentage
points. A/B median was -0.2020%, pooled -0.1916%, IQR 3.4223 points. The
candidate's apparent 0.20% saving is smaller than the absolute A/A median and
both interquartile ranges. Other fleet compilation was observed in all 24
ambient samples. No pair was filtered, replaced or extended, and no fresh
game was scored as strength evidence for this change.

The measured candidate is `e9b6246a9cc7749b908c46de36749cc74c450646`; integration
retains its runtime implementation byte for byte. Its
Native executable SHA-256 is
`6250ffd6e277ce481c75268c1f305082582c7ae6c3da5c5b9c42673c5b2bf5a0`.
The frozen 282-tag force list has SHA-256
`285cd68ff5fcbf7dd8f205d8f9098ea16e2f94c974053b21b8c2db93fcdf1591`.

## Reproduction and evidence

The provenance archive includes the exact replay harness, frozen force list,
compiler/profile, production-input hashes and all local validation logs. The
input archive contains the byte-exact original prefix through opening turn
113, SHA-256
`1bdf3eed6732bf71c64e18162fd5a988afed5f230b48d0505d160cfce450f5dd`.
Rebuilding the two recorded source revisions with the registered Native fast
profile and running the archived harness on that prefix reproduces the same
24-request workload and fixed A/A–A/B schedule.

The one-off timing test is preserved in the provenance archive and measured
candidate revision, not in the final test inventory. CI correctly rejected its
addition to the repository's automatically discovered nightly census set. The
integration fix removes only that timing test, retains both deterministic
regressions, and leaves the census guard and production implementation unchanged.
The final Rust suite and all 29 census-tool tests pass after that removal.

```sh
cargo test --profile ci --locked -- --test-threads=1
python3 -m unittest discover -s tools -p test_census_report.py
python3 tools/rust_quality.py --base 44629581fd661fdf03f94a3801689f615c09e40b --head e9b6246a9cc7749b908c46de36749cc74c450646
```

For the separately invoked timing experiment, use the measured candidate
revision `e9b6246a9cc7749b908c46de36749cc74c450646` and run:

```sh
cargo test --profile ci --locked --lib overlapping_threat_query_cost -- --ignored --nocapture --test-threads=1
```

- [Summary and independent audit](../performance/2026-10-16-religious-safety.json)
- [Every raw CPU receipt](../performance/2026-10-16-religious-safety.receipts.jsonl.gz)
- [All parsed replies](../performance/2026-10-16-religious-safety.replies.json.gz) and [all journals](../performance/2026-10-16-religious-safety.journals.json.gz)
- [Original input prefix](../performance/2026-10-16-religious-safety.input.jsonl.gz)
- [All six simulator reports](../performance/2026-10-16-religious-safety.headless.json.gz)
- [Source provenance, harness and validation logs](../performance/2026-10-16-religious-safety.provenance.json.gz)
