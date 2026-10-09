# Independent defending district combat and observations

A healthy Oppidum can now shoot, take district damage, protect its garrison,
and contribute an independent enemy threat while remaining an Industrial Zone
replacement. A city with both an Encampment and an Oppidum has separate inner
health, outer defense and attack budgets. Depleting one does not suppress the
other. Native orders identify the actual source district separately from the
target plot, and same-turn replanning preserves each issued source's budget.

The installed `DLC/Expansion2/UI/CityBanners/CityBannerManager.lua:693-705`
creates the Oppidum defending-district banner and registers
`OnDistrictRangeStrikeButtonClick` at **703-705**. Lines 2539-2546 resolve and
select that district before entering `DISTRICT_RANGE_ATTACK`.
`Base/Assets/UI/WorldInput.lua:2621-2627` puts the target coordinates in
`UnitOperationTypes.PARAM_X/Y` and calls `CityManager.CanStartCommand` and
`CityManager.RequestCommand` with the selected district and
`CityCommandTypes.RANGE_ATTACK`.
`DLC/Byzantium_Gaul/Data/Byzantium_Gaul_Expansion2.xml:70` supplies
`<Row DistrictType="DISTRICT_OPPIDUM" AttackRange="2"/>` in `Districts_XP2`.

Encampment-family fields and actions keep their existing save and wire shape.
Other defending districts use position-keyed state. Completed observations
normalize each district's own defense pools; known zero capacity stays zero.
Pillage, district repair, turn resets, healing and wall construction update
that district. Save loading retains measured zero and spent attacks, while
older saves initialize previously unmodeled forts. Existing capture roster
conversion drops defense state when the resulting district lacks defenses;
the conversion test checks existing engine behavior, not native DLL semantics.
Foreign fort health refreshes only when that fort is visible, preserving last
seen values under fog and omitting private attack timing from public memory.
Combat planners, forcing replies, healing danger and retreat danger include
each defending district separately. Own fire phases rescore after a shot and
retain the fast gate when no fort can fire.

The unchanged production baseline executed every original mirror case and all
seven new observation cases: 400 passed (396 existing plus four controls),
three failed and two existing cases were ignored. The failures were a lone
healthy Oppidum shot, two independent shots from one city, and a healthy
Oppidum beside a depleted Encampment. Assertions require both a legal action
and actual damage on a clone; their bodies remain byte-identical throughout
implementation. Fixture-only CI independently reproduced the two-fort
failure but stopped before the other six cases. Its cost result was noise.
The one main merge changed only Lua/evaluation files, preserving all Rust,
data, build inputs and baseline assertions.

The corrected source `daafc2284fdb32f458dafbee2eb7db357ebdb9e6` passes all
33 focused cases (32 new cases plus one existing parent-identity case), then
the unfiltered no-retry/no-fail-fast suite: **4,813 passed, 51 existing skips**.
Documentation checks pass with four existing ignored examples. All 642
registered inputs remain unchanged. The exact-source CLI build and all twelve
four-player Lakes soak games complete without crashes: Gran Colombia, Gaul,
Rome and Scythia, six city-states, Emperor, Standard speed, domination enabled
as the sole victory, seeds 402500-402505 (Ancient, 100 turns) and
402600-402605 (Industrial, 80 turns). All reach the cap as draws. This is
engine integration coverage, not a native trial or victory-rate estimate.

All 89 discovered Lua suites pass, including all 17 new native dispatcher
cases; all 96 Lua files in both CI roots parse under Lua 5.1. The existing
production-type checks pass all 11 tests. Applying the same 17 cases to the
unchanged native dispatcher produced seven expected failures and ten passes.
All 642 registered candidate inputs stayed unchanged during the native checks.

Initial failures are retained: twelve compiler errors from private/legacy
planner interfaces, the invented command label rejected by the type guard,
and the old six-call war-guard count after adding the seventh guarded handler.
The first compiled focus then exposed a fixture with insufficient specialty
district capacity. After correcting population and unlocks, 31 of 33 focused
cases passed; remaining fixture failures assumed an object-shaped serialized
city map and a recipient already able to retain an Industrial Zone. These
were corrected in tests with the real array shape and recipient technology;
production code, Lua and the seven original assertions stayed unchanged.
Two test-only clippy warnings were also corrected.

The first integrated cost gate passed with +3.84% median per completed turn,
five pairs and 600 turns per arm. All five reports differed: this is changed
behavior, not isolated overhead. The IQR was 14.62 percentage points and the
run resolved only ±9.69%, wider than the +8% budget. The green verdict does
not establish an absent cost increase, and no speed improvement is claimed.
The corrected test-only head also passes the cost gate at +2.08% median,
+4.91% pooled, five pairs/600 turns. Its IQR is 14.05 points and resolution
±9.32%, again wider than the budget; all five game reports differ. It does
not remove the first run's limitation.

Logs, registrations and receipts are retained under
`civvis-tactics-results/2026-10-08/public-campus-finish-before-defender/`.
The complete unchanged baseline log SHA256 is
`7e8f0d1a530f018db700849bb450cf026757cefce11a2908ac041207c7c4f7a4`.

No native lane, private pin, installed mod, runtime setting or game process
was changed. No controlled native candidate games or win-rate gain have been
established. An assigned native A/B lane remains necessary to measure the
Gran Colombia four-player domination effect.
