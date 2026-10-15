# Plan Bomber campaigns from active airbase tiles

Computer: MacBook Pro

## Defect and native contract

`air_surge_bases` lists only owned city centers. The engine's existing
`air_capacity_at` also recognizes the actual tiles of active Aerodromes,
airstrips, and owned carriers. A forward field can bring a city into Bomber
range even when its parent city center cannot. The campaign appointment,
readiness, and retargeting all consult this incomplete base list.

Installed Gathering Storm source
`DLC/Expansion2/UI/CityBanners/CityBannerManager.lua:350` associates the banner
with the district's coordinates. Line 367 reads
`iAirCapacity = pDistrict:GetAirSlots();`, and line 368 reads its aircraft.
Lines 404-409 instead read an airstrip plot and its slots.
`Base/Assets/UI/UnitFlagManager.lua:399-414` reads a carrier unit's air slots
and aircraft. These are separate physical bases.

Native run `civvis-20261015T120650Z` reports Bombers stationed on the Aerodrome
at `(15,21)`, separate from Bogotá's center `(17,18)`. That game had four
Bombers by turn 165, an air campaign appointment at turn 189, and no city
dispositions before a rival science victory at 209. Its first nearby
Scythian Neapolis appeared on the same turn as that appointment. Earlier
known objectives were farther away. Neither the range-list omission nor a
campaign-priority gate has been established as the cause of that loss or
delay; this change targets the independently reproducible base-list defect.

## Baseline and validated repair

The controlled board places an owned city center twelve hexes from the
objective and its Aerodrome ten hexes away. Two ordinary Bombers rebase
legally to that district. The campaign must recognize their objective as
reachable. Equivalent controls cover an owned airstrip and carrier;
pillaged and foreign bases supply no reach. City-center reach remains valid,
and one Jet Bomber cannot lend its range to an ordinary partner.

Baseline revision `1dea5b872d4758605bfd801b1acbe524fd4ea253` fails
GitHub run `38767849894`: the Aerodrome and carrier cases both complete the
two legal rebases and then fail the campaign-range assertion. The runner
stops early after 138 other passing tests and two failures; this does not
establish results for the unexecuted airstrip and negative controls.

The repair enumerates map positions with positive capacity from the existing
engine predicate. Making that read-only predicate crate-visible lets the
planner share the rebase engine's ownership, active-field, and carrier rules.
The existing per-generation two-aircraft reach requirement is unchanged.
GitHub run `38769780552` passes all seven focused controls on repair
`b4612cb67`, plus 2,940 other tests, before an existing negative fixture fails.
That fixture assigns both domestic cities a new coordinate directly while
leaving their spatial lookup at the old coordinates. It therefore retains
old physical bases under the shared engine rule. The fixture now shortens
the hypothetical Bomber range to one instead of corrupting city positions;
its unreachable-objective assertion remains. Actual base capacity and native
command transport are unchanged.

After integrating settlement-pressure main revision `e51760e01` once, exact
source `6e2f223baf2f857ab8fc67ccd96bf7f43b9aad4f` passes the full local
`cargo test --profile ci --locked`: 6,068 tests, 51 existing library ignores,
and four ignored documentation examples. Every registered source hash stays
unchanged throughout that run. GitHub run `38778374645` independently passes
all 6,068 tests, including all seven new controls, plus its 116 selected
regressions. All quality, ownership, overwrite, published-build, and mod gates
pass on that same source.

Paired-cost run `38778374589` completes five 120-turn simulated pairs. Its
median cost difference is +0.09%, inside the tool's noise floor and +8%
budget; this establishes no speed improvement. An additional engine soak is
not applicable: engine capacity, rebase legality, and state transitions are
unchanged; only the planner's base enumeration and accessor visibility change.
The positive controls execute two real legal rebase actions for each field
kind before checking campaign reach.

A limited geometry screen of the first state per turn in three recorded
worlds found no existing Aerodrome/carrier that changed the two-Bomber reach
verdict relative to city centers. It covered 171 states with at least two
Bombers and 1,768 known at-war rival-city checks. This omission is a reproduced
planning defect, not an established explanation of those native outcomes.

No repaired native execution, performance benefit, campaign capture, or
win-rate improvement is claimed yet.
