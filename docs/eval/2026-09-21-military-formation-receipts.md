# Read the military tier when verifying Corps and Army orders

Native run `civvis-20260921T191318Z`, pinned to `31bbac39e`, issued six
`FORM_CORPS` commands through turn 175. Every next-turn receipt reported
`not_in_formation`, although every subject was exported with military tier 1
and escort count 1. The commands had succeeded; the verifier tested the wrong
field.

| Issued | Checked | Native unit | Type |
| --- | --- | --- | --- |
| 153 | 154 | 5701651 | Cuirassier |
| 153 | 154 | 6094850 | Field Cannon |
| 153 | 154 | 6291457 | Cuirassier |
| 154 | 155 | 5767173 | Cuirassier |
| 165 | 166 | 7274496 | Cuirassier |
| 167 | 168 | 7405575 | Bombard |

`formation_count` comes from `GetFormationUnitCount`, which counts escort
members. A Corps is one military unit, so its count remains one. The exported
`formation` comes from `GetMilitaryFormation`: 0 standard, 1 Corps/Fleet,
2 Army/Armada. Missing, negative, or unsupported values are unknown.

The installed `Base/Assets/UI/Panels/UnitPanel.lua:2259` reads
`kSubjectData.MilitaryFormation = unit:GetMilitaryFormation();`. Lines
4018–4031 use that accessor and compare it with `CORPS_FORMATION` and
`ARMY_FORMATION` for Corps/Army and Fleet/Armada labels. The existing exporter
already distinguishes these fields; this correction belongs in the receipt
verifier.

The escort check remains attached to `ENTER_FORMATION` and
`EXIT_FORMATION`. Corps and Army receipts must require the corresponding
known military tier; an escort stack cannot verify a military merge.

Native evidence is retained in
`/tmp/civvis-military-formation-replay/native-receipt-evidence.json`.
The frozen event prefix through turn 175 has SHA-256
`c1d40e18adb933130c7ad3f5129eb509cc3fa7d4ce59fbf6f86102044c5f0cec`.

Validation on integrated source `80ff7be76`, after merging main `0f410c021`:

- Two new regressions fail before the production change: a real Corps is
  called a failure, and an escort stack is incorrectly called a military merge.
- All 178 orders-binary tests pass, including four new tests covering Corps,
  Army, insufficient tier, missing/invalid observations, disappearance, and
  unchanged escort semantics.
- `cargo test --profile ci --locked`: 4,113 passed, zero failed, 53 ignored.
- Fourteen treatment append tests pass.
- No engine or planner code changes. A headless game soak does not exercise
  native receipts; the paired native replay checks the changed behavior.

Final paired replay compares exact parent `0f410c021` with integrated candidate
`80ff7be76`. Both complete 486 frames, including the merged air preparation and
settler changes. All internal actions and all executable exported orders are
identical. Only four receipt frames change: 154/0, 155/0, 166/0, and 168/0.
The six false Corps failures become verified, and the corresponding four turn
summaries update their verified/failed counts. There are no Army commands in
this prefix; focused tests cover that tier.

Complete final comparison is `/tmp/civvis-3688-final-comparison.json`.
Binary hashes and source provenance are retained in the artifact directory's
`provenance.json`. The initial isolated replay against `19913c086` (Rust-source
equivalent to the native `31bbac39e`) produced the same six corrections; its
artifacts remain separate from the final integrated pair. Runtime differences
are not performance evidence because machine workloads were not controlled.

Correcting these receipts is not a new native Corps, a strategic victory,
or evidence of improved win rate.
