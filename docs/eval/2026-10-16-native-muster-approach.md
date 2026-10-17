# Native siege approach: preserve the road ledger across fresh boards

## Finding and scope

The fresh-board adapter carries siege progress and capture commitments by city
position and surviving host-unit identity. The road ledger was an exception:
`remap_siege_memory` did not remap `siege_road_tally` or `siege_road_closed` city
keys. Its unit sets were remapped only with the independent
`envoys-open-the-siege-road` treatment, even when the road stand-down policy was
enabled without that treatment.

Four ordinary, self-contained regression tests fail against unchanged
production at checkpoint `d1b4c6eb8`:

- A renamed city loses its road tally; surviving units must also follow the
  adapter's explicit identity map without requiring the envoy treatment.
- A stood-down road loses its city key, so its named passage and early reopening
  no longer follow the capture stand-down that *was* remapped.
- Three border-blocked turns do not produce the existing three-turn stand-down
  when each rebuilt board renumbers the city. The same-ID control passes; the
  renumbered control's streak restarts at one on its second turn.
- A city missing from the new board leaves stale road records behind.

The bounded repair remaps both city-keyed maps at the existing siege-memory
boundary and permits the existing unit remapper when either road-recording
treatment is active. It does not change target ranking, the three-turn patience,
the held-share threshold, danger limits, strike-range exclusions, border access,
land/water routing, declaration conditions, genes, the deployment profile, or
verification settings. It restores the identity contract of an existing policy;
it is not a new default conquest strategy.

## Original Native evidence

The preselected original world is Emperor Gran Colombia run
`civvis-20261017T025117Z`, produced from revision `44629581f`. The verifier
abandoned it at turn 150 under its existing production-rank gate. It is not a
completed victory or a completed loss, and is not evidence of a highest-level
Domination win.

Its own journal names Maya's closed borders as the gun train's blocker from
turn 145. At turn 147 it says the blocker is not weak enough to declare on.
The same siege still holds for a wall-breaker at turn 150. A warrior's distant
muster hold is not proof of an unsafe move refusal, a search-bound failure, or
an entire army frozen for 150 turns: the task force appears at turn 145 and
the warrior does move between observations.

Before changing production, the study registered three original opening-frame
prefixes, turns 146, 149 and 150. Cold reconstruction identifies Xanadu at the
same axial position `(33, 22)` but with model city IDs **94, 99 and 98**.
That is precisely the namespace change the road maps did not follow.

| Opening turn | Own land military units | No dry road from their current tile |
| --- | ---: | ---: |
| 146 | 17 | 17 |
| 149 | 18 | 17 |
| 150 | 17 | 16 |

For host warrior `131073`, the turn-146 opening reads 24 tiles out, 3 movement,
zero danger and no ranged reach; its nearest safe reachable endpoint is still
24 tiles out and the isolated muster method holds. At the turn-149 opening it
can move from distance 24 to 23, and at turn 150 its nearest safe endpoint is
again distance 23, so it holds. This is consistent with progress followed by a
local minimum without a dry approach, not evidence that safety limits should
be relaxed. Host Pike and Shot `8650793` similarly has no nearer safe endpoint
in these openings.

These observations motivate the repair, but do **not** establish that the
missing remap alone caused the original game's result. There is no persistent
counterfactual replay or host-executed continuation here. Planning frames can
move or occupy tiles before a later hold; a cold opening is not that later
planning state. Restoring a stand-down does not prove that the replacement
objective is better or that a capital will fall.

## Reproduction and validation

The ignored historical diagnostic reads only an explicit external directory.
It validates the reconstructed configured treatment set against the original
startup metadata, inspects all observed land military units, records dry and
safe route metrics, reachable endpoint exclusions and Stage march blockers,
and invokes the isolated muster method only for its non-Siege unit domain on
speculative clones. No game is launched and no host action is sent.

The adjacent preparation script requires the original stopped-run directory
and an unused output directory. It verifies all original file hashes, extracts
only the registered prefixes, verifies their byte counts and hashes, and
checks the original files again. The JSON record is its expected-input source.
The original 40 MB stream remains in the local Native run archive; it is not
duplicated into Git. Reconstruction therefore requires access to that archive;
the normal regression tests need no external data.

```sh
python3 docs/eval/2026-10-16-native-muster-approach.prepare.py \
  --source /path/to/civvis-20261017T025117Z --output /unused/study-directory
CIVVIS_MUSTER_DIAGNOSTIC_DIR=/unused/study-directory \
  cargo test --profile ci --locked --lib \
  diagnose_original_native_opening_prefixes_without_host_actions -- --ignored --nocapture
cargo test --profile ci --locked --lib native_memory_tests
```

All four normal regression controls pass with the repair. The full local CI
profile suite passes **6,155 tests, zero failures, 56 ignores** (55 existing
ignores and the new external-input diagnostic). The explicit historical
diagnostic also passes separately. The developer-tools all-target check,
all-features changed-line Rust quality gate, and scoped format check pass.
The preparation recipe reproduces all three registered byte counts and
hashes; two negative controls reject an existing destination and a wrong
original input before overwriting or creating output. Exact receipts are
retained with the source identities.

The test-only red checkpoint was also pushed to GitHub before the repair. Its
Ubuntu CI run failed on the first three new regressions and stopped under
fail-fast, rather than completing the fourth; the complete four-failure red
receipt is the separate local focused run. This is not a green baseline CI
claim. The repaired PR still requires its normal GitHub gates before merge.

A combat-arena soak cannot exercise the native adapter's fresh-board identity
boundary; the controlled same-ID/renumbered multi-turn test is the targeted
behavioral check instead. This study claims neither measured CPU savings nor
conquest-strength improvement, and introduces no verification-game cohort.

## Artifacts

- `2026-10-16-native-muster-approach.prepare.json`: registered input identities
  and final validation summary, consumed by the preparation script.
- `2026-10-16-native-muster-approach.provenance.json.gz`: original registration,
  diagnostic outputs, source identities, profile and validation logs.
