# Independent defending district observations

Investigation in progress. No production mechanic or AI policy is changed.

The installed `DLC/Expansion2/UI/CityBanners/CityBannerManager.lua:693-705`
gives an Oppidum the defending-district banner and district strike callback.
At lines 2539-2546 that callback finds the district, selects it, and enters
`InterfaceModeTypes.DISTRICT_RANGE_ATTACK`.
`DLC/Byzantium_Gaul/Data/Byzantium_Gaul_Expansion2.xml:70` supplies
`<Row DistrictType="DISTRICT_OPPIDUM" AttackRange="2"/>` in `Districts_XP2`.

The current model's strike and combat-target lookup uses the Encampment family.
Oppidum keeps its Industrial Zone economic family. Seven fresh mirror cases
exercise a lone healthy Oppidum, two defending districts in one observed city,
independent depleted pools, a pillaged fort, a positive Encampment shot, and
preservation of the Industrial Zone family and both roster entries. Targets
lie outside city-center strike range. Shot assertions require both a legal
action and actual unit damage on a clone, rather than a future action's name.

The complete unchanged-production baseline executes all seven new cases and
all existing mirror tests: 400 pass (396 existing and four new controls),
three fail, and two existing tests remain ignored. The failures are the lone
healthy Oppidum shot, two independent shots from one city, and the healthy
Oppidum beside a depleted Encampment. The positive Encampment shot, depleted
Oppidum/healthy Encampment, pillaged Oppidum, and economic-family/roster
controls all pass. No compiler error occurs, and all 636 registered inputs
remain byte-identical through the run. The test bodies are retained.

Independent fail-fast CI reproduces the two-fort failure, after 3,610 other
tests pass; it never executes the other six new cases. Its immediate/final
output reports the same failure twice. The complete local run supplies the
remaining case results. The fixture-only cost gate reports -0.38% within its
1% noise floor, with five pairs and 600 turns; no speed improvement is claimed.

The one current-main merge brings in the separately published range-table
export correction. Its four changed paths are Lua or evaluation files; Rust,
data, build inputs and these seven assertions are unchanged from the tested
baseline. A candidate must preserve two independent defense and strike
budgets without changing the Oppidum's economic family. The native dispatcher
at `CivvisControlAgent.lua:12805-12813` also explicitly selects the base
Encampment type; an Oppidum action needs correct native district selection.
Capture conversion semantics are unverified. Do not ship this investigation
as an implemented fix.

The complete baseline log SHA256 is
`7e8f0d1a530f018db700849bb450cf026757cefce11a2908ac041207c7c4f7a4`.
Logs and immutable registration/receipts are retained under
`civvis-tactics-results/2026-10-08/public-campus-finish-before-defender/` as
`defending-district-observation-unchanged-baseline-rust-*` and
`defending-district-observation-unchanged-baseline-ci.*`.

No native lane, private pin, installed mod, runtime settings or game process
is changed. No controlled native candidate games or win-rate gain are claimed.
