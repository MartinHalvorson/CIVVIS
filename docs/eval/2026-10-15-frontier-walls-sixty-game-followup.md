# Fixed sixty-game frontier walls follow-up

Computer: MacBook Pro

The completed follow-up does not support promoting `frontier-walls-before-the-surprise`. Its win estimate is negative but unresolved. The feature remains off by default; deployment selections and the native verification force list remain unchanged. This is simulator evidence, with no native survival or conquest-strength claim.

The block was registered at 2026-10-15 20:03 UTC before any game ran. All 60 games completed, using seeds `40631001..40631060`: six majors per game, 360 measured seats, 74×46 Continents, nine city-states, Online speed, 250 turns, Emperor majors, Deity barbarians, all victory conditions, native competitions and the established target mix. Independent seat assignment used probability 0.25 for the frontier-walls gene. Exactly 87 seats drew it on and 273 off; all other 624 genome bits were identical. These are independent per-seat assignments within shared games, not paired game replays. The analyzer clusters uncertainty by game.

| Outcome | ON minus OFF | Standard error |
| --- | ---: | ---: |
| Win rate | −5.31 percentage points | 4.26 points |
| Score share | −0.27 percentage points | 0.87 points |

Observed win rates were 12.64% ON and 17.95% OFF. The analyzer reports `~` (unresolved); its stated 80%-power resolution is 11.92 points for wins and 2.43 points for score share. A negative point estimate is not evidence of a resolved harm. The six-game firing pilot in the implementation note is separate and is not pooled into this follow-up.

Games ended in Science 51 times, Culture six times, Score twice and Diplomacy once. None ended in Domination. The measured mix therefore does not establish that this policy helps the intended conquest games. The game-level regression estimates CPU cost per completed turn at +0.65% per enabled seat with 2.28% standard error, also unresolved.

The unchanged screen binary was compiled from `89d1f01bb0be62b0ad739725b3ba5096ec18c773`; SHA-256:
`dc1418d6721d3a1939c8c0f1166c791c8b90f43c8ff86c3b9c3187c8341c57d9`.
Its runtime header correctly records `1aeaac1c67bf6a52002d01a7baefcc7828162497`, a subsequent documentation-only build-tree head. Source equivalence across `src`, `data`, `Cargo.toml`, `Cargo.lock` and `build.rs` was witnessed at 20:04 UTC, before outcomes were examined. The header is preserved verbatim.

All 360 output rows are retained. Their original JSONL SHA-256 is
`8d2b58dbef38a1143fe5d3b044870802950e195543d3872a74ca66e9f08ffeb0`.
The original runner refused its comparison with the pilot's different header commit after all games had completed. The independent auditor passed the recorded source-equivalence and binary checks, then initially expected `kind: "seat"`; the source actually emits `kind: "game"`. That assertion was corrected to match the source. The final audit verifies all 60 exact seeds, six unique seats per seed, the immutable binary hash, settings, header provenance and the other 624 fixed bits. No game was replayed, deleted, replaced or stopped after inspecting outcomes.

The separately registered native four-arm saved-board study has played zero arms. It remains queued behind an unresponsive Exit To Desktop dialog in the preceding unowned game. Frozen-order queries previously show that the feature can change wall queues; they do not demonstrate completed walls or survival. Native results must be reported separately when available.

Recorded artifacts: [unmodified raw JSONL, gzip compressed](../gene_screens/studies/2026-10-15-frontier-walls-sixty.jsonl.gz), [actual analyzer output](../gene_screens/studies/2026-10-15-frontier-walls-sixty.analysis.json), and [preregistration and provenance](../gene_screens/studies/2026-10-15-frontier-walls-sixty.provenance.json). Decompress the JSONL before passing it to `gene_screen --analyze`; the gzip timestamp is fixed at zero, and the decompressed bytes match the recorded raw hash.
