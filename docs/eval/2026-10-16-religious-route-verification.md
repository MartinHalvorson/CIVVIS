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
  Online speed, Deity, 160-turn limit and all six victory conditions. These are
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

Pending validation; this document is not a performance or strength claim.
