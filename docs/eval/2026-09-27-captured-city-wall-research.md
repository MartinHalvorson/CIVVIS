# Captured-city wall research preparation

Default off and withheld in PR #3800. The eight registered fresh pairs have
identical OFF/ON actions and outcomes. No Domination improvement or native
promotion is established.

The verified Prince Gran Colombia game captured Székesfehérvár at turn 41 and
lost it to a military capture at 76. Masonry was boosted by 25 but did not
complete until 74. Walls first appeared in the legal production menu at 74,
with a five-turn estimate, and were selected at 75. The preserved prefix has
12 free research slots from 45 through 73. This timeline motivates preparing
the prerequisite sooner; it does not prove that earlier research saves the city.

The existing wall research helper already serves Domination and Culture.
This experiment preserves that warning and adds a fallback for an owned,
unwalled city whose major founder remains alive and at war with us. Only an
active Domination target, victory planning and the optional gene can activate
it. Legacy mode and ongoing research retain their existing choices. Research
preparation does not guarantee wall construction. No production, movement,
threat, aircraft, envoy or native-control policy changes are included.

The actual `advanced_research` chooser with the deployed 21-policy force
bundle selected Wheel in the failed-first captured-city fixture, against the
expected Masonry. Source: `4d29b87b90eea3558becef844ddfe9440f954b0e`.
The flag was enabled but had no implementation. An earlier fixture-construction
failure is retained separately and does not establish the research defect.

Eight fresh focal-only Prince pairs were registered before implementation at
`2026-09-27T02:30:20.239844+00:00`: seeds `37919000`–`37919007`, Gran Colombia,
Domination target, four majors, 60×38 Pangaea, six city-states, Online, 250 turns,
all victories enabled. Players and barbarians both use Prince. Rival controllers
remain fixed. Four independent two-seed processes preserve the alternating
OFF/ON leg order across all eight pairs. No partial outcomes were read and no
policy was tuned after outcomes.

The external runner initially used a nonexistent working directory. All four
launches failed before invoking any binary or game; the empty logs and launch
receipt are retained. Correcting the runner path changed no source, binary,
policy, seed or simulation command.

Frozen source: `a8626e51976cdfe94ba166431688245363020ed4`; parent main:
`2bd913cad94749b554b9c5e9091bc096b4f2574b`. This excludes later #3799 and
#3801 changes. All 10 focused controls pass. The full Rust suite passes
4,381 tests, with zero failures and 53 ignored. Incremental quality, genes,
manifest and 14 append-point checks pass. The unmodified actual six-game
firing analysis passes the existing 327/327, zero-waiver gate. Its random-seat
regime does not measure focal Domination strength or establish causal benefit.

Both native-prefix replays finish all 202 observations. Actual genome readback
confirms the flag off/on. Three research requests change: Wheel→Masonry at 45,
Apprenticeship→Masonry at 66, Education→Masonry at 69. Three following verification
records also change because the archived future still reports the original
research. These are expected replay mismatches, not real native executions.
Other issued actions are identical. This replay cannot establish earlier
completion, counterfactual city survival or a native win.

Binary SHA256:

- Orders: `adb368111aa1099759ee8de0347b00ec5667d71bd08c1a501dfb29d83632cb9a`.
- Evaluator: `505e0d7151c1d9ec6e20c5b3b054eb9a55f23bb89db2801b1951c06c04bb1c98`.
- Gene screen: `b1fe0a9f9bb7aa4fdd41fbd84e468d331781635d7ded08d0fc369f184594ee5a`.
- Forced policy file: `439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0`.

All eight complete pair records are retained. Every OFF/ON action stream and
reported outcome field is identical. Each two-seed segment exits **2**, after
emitting both complete records, with the evaluator's existing rejection:
“all paired action records are identical; this batch provides no behavioral
contrast.” These failures remain recorded; the evaluator guard was not changed
and the experiment is not reported as a successful strength test.

Across the set, both arms have one Score win, zero focal Domination wins,
one home-capital loss, eight foreign major cities ever observed held and one
foreign capital held at the end. The opponent Domination win at seed 37919001
is a focal loss. Table values below apply to **both** arms because they are
identical; no pair or loss is omitted.

| Seed | Ending | Turn | Winner | Focal score | Actions | Major cities ever | Foreign capitals at end | Home capital held |
|---|---|---:|---:|---:|---:|---:|---:|---|
| 37919000 | Science loss | 218 | 1 | 1364 | 45570 | 1 | 0 | yes |
| 37919001 | Domination loss | 236 | 3 | 549 | 46576 | 0 | 0 | no |
| 37919002 | Science loss | 242 | 3 | 1032 | 52178 | 0 | 0 | yes |
| 37919003 | Score win | 250 | 0 | 1214 | 54668 | 2 | 0 | yes |
| 37919004 | Score loss | 250 | 2 | 697 | 51092 | 1 | 0 | yes |
| 37919005 | Culture loss | 188 | 2 | 538 | 32652 | 0 | 0 | yes |
| 37919006 | Religious loss | 186 | 3 | 647 | 34491 | 0 | 0 | yes |
| 37919007 | Culture loss | 230 | 3 | 949 | 42520 | 4 | 1 | yes |

Raw paired-record SHA256:
`a06f5a931f5ae5f1d5c648aece3d277f2fea1462234119931552a8792d0edebc`.
Registration, immutable binaries, validation, native replay, all four segment
receipts and logs, complete raw pairs and comparison are retained under
`~/civvis-war-evidence-20260926/native-wall-preparation/`.

Keep the policy off and withheld. The native replay demonstrates an earlier
research request in the archived case; the registered fresh set supplies no
behavioral contrast or war-strength improvement. No difficulty promotion follows.

Known-seed observer diagnostics reproduce every original outcome field and
action count for the four pairs with observed foreign major-city holdings.
Both diagnostic action streams remain identical and the no-contrast rejection
remains exit 2. The observer only reads immutable game state; the linked AI
library is the frozen candidate. These are reused diagnostic seeds, not fresh
strength results. Original full action hashes were not retained, so field/count
reproduction is not an independent original-stream identity claim.

| Known seed | Masonry first observed known | First foreign major-city holding observed | Walls at that first holding |
|---|---:|---:|---:|
| 37919000 | 41 | 147 | 400 |
| 37919003 | 49 | 190 | 400 |
| 37919004 | 54 | 162 | 100 |
| 37919007 | 40 | 104 | 0 |

No foreign major-city holding snapshot in these diagnostics has Masonry
unknown. The other four original pairs have no observed foreign major-city
holdings. Observations are at civilization-turn boundaries and cannot census
every within-turn research call or transient capture. The native early,
unwalled, Masonry-missing holding is therefore absent from the observed
holdings in this registered set. The zero action contrast establishes no
war-strength benefit; these results do not demonstrate that the fallback
would fail in the native early-capture case.

Diagnostic binary SHA256:
`d59f15e9357a1d22eb4eb1803093334a4907c118e8b4fd7f3ab2133ac0ad4889`.
Observer source, plans, timelines, all original-field comparisons and actual
exit codes are retained in `research-slot-diagnostic/`, `diagnostic-37919000/`,
`diagnostic-37919003/`, `diagnostic-37919004/`, `diagnostic-37919007/` and
`research-diagnostic-comparison.json` under the evidence root.
