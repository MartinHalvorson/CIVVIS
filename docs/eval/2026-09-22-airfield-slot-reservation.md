# Reserve a productive airfield slot

Native match `civvis-20260921T233717Z` (`48e2388ce284d46fd3cae23bbdef5f0bff0df2b0`) had an air plan at turn 139 and reappointed one at 169, five technologies from Advanced Flight. Maracaibo then had a Campus, Industrial Zone and one free specialty slot. At 170 it selected a Commercial Hub, consuming that slot before Flight arrived. Slower Panamá supplied the later airfield. Its first Bomber remained three turns from completion at 198; Sumeria won by Culture at 199.

While an active Domination air plan lacks a committed airfield, the planner reserves the last specialty slot in the most productive unthreatened city with a legal future airfield site. Campuses remain allowed because aircraft require research. Existing foundations and non-specialty districts remain eligible. The reservation ends when the air plan ends or an airfield is committed. It postpones other district development in the reserved city and may redirect production elsewhere through the shared unit census.

The district-layout scoring pass must preserve the reservation after ordinary production scoring. The first prototype changed zero actions because the layout floor raised the rejected Commercial Hub back to 66. Instrumented replay reproduced all baseline actions and identified that override. The second prototype changed the Hub but also postponed Cartagena de Indias' first Campus; the final version explicitly preserves the Campus. Its added regression test fails on the version without that exception.

## Final integrated recorded-game comparison

Both binaries replay the same 479 turn/frame boundaries through turn 173, with the recording's forced genome. Baseline source: `590641be9305e94fd42020f26dc93e07e3053198`. Candidate source: `430bed6ba17e8c7bba2ab01d871c0faf20062841`.

- Input SHA-256: `d666173bbfe52065cd2a0dcbb8fd9d01857f794a0a6d2f19aa599e6461aea83e`.
- Baseline binary SHA-256: `8ad8caf35a8c5e8805352719a12a549a190652d3ebc88a2404b7f7f51f72fd68`.
- Candidate binary SHA-256: `d83f8d7e92101513b12ca1b20b6cf5673bca7ab3c85486ac48ebc43a9f165204`.

Eight exported frames and 21 internal-action frames differ, identical to the corrected pre-integration comparison. At 146/0 Caracas substitutes a Builder for a Commercial Hub; its later recorded foundation leads to further production/receipt differences. At 170/0 Maracaibo substitutes Artillery for its Commercial Hub. At 171–173 the recorded future produces follow-up queue differences in Maracaibo and Popayán. All internal differences are production actions. Cartagena's Campus and following turn 140/141 exports match the baseline exactly; Cuenca's turn-168 counter-faith Holy Site also remains unchanged.

Counterfactual receipts compare the changed proposal with the unchanged historical future. They do not establish host rejection, earlier completed aircraft, capture or victory. In particular, this replay does not simulate Maracaibo subsequently building an Aerodrome or Bomber. Those outcomes require native verification.

## Validation

- Full integrated Rust suite: 4,165 passed, zero failed, 53 ignored.
- Five focused tests pass, including production scoring followed by district-layout scoring and the Campus regression.
- Fourteen treatment append-integrity tests pass.
- Eight integrated four-player smoke games complete, seeds 370700–370707, using `--turns 180 --start-seed 370700`.
- Exact integrated-head changed-line Rust quality passes against baseline `590641be9`; scoped whitespace checks pass.
- Integrated CI paired cost passes at +0.53% per completed turn, NOISE, IQR 1.26 percentage points, resolution ±0.84%; within the +8% budget. Final ready-head checks remain enforced by shipping.

Artifacts remain under `/tmp/civvis-airfield-slot-replay`; comparisons are `/tmp/civvis-3707-integrated-comparison.json` and `/tmp/civvis-3707-integrated-all-native-changes.json`. No recordings or diagnostic instrumentation are committed.
