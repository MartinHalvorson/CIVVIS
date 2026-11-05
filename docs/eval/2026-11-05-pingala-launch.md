# Pingala and the first launch producer

The existing `pingala-follows-research` treatment is opt-in. Its ordinary
relocation explicitly freezes Pingala once he owns Space Initiative or Curator.
That preserves unpriced specialist jobs, but also freezes a governor in a city
with neither a Spaceport nor Great Works while another city builds the launches.

Native combined Writing/Foundation pilot
`civvis-20261105T114538Z-current-writing-pilot-01` used frozen source
`0ab72aa31ec43b2d127bf86ba67b7b1ee1a28c15`, executable SHA256
`632d442409e028d97d4f411a6ba64eb8a88e57e6e89f61d4731307a4a9fd8c45`.
Its sealed-record verifier passed 453 canonical records and 7878 captured reads.
It lost to rival Science178, with no cities lost. This is one observation,
not a controlled effect estimate.

At154 the capital (native65536) had11population,43production, an active
Spaceport with12turns remaining, and ordinary Magnus. Pingala was established
in native131073, population5, with Space Initiative and Curator, no Great
Works and no Spaceport. Both cities had100Loyalty and no damage. No observed
rival was at war with us at154. By165 the capital began Earth Satellite,
finished175; Pingala still occupied131073. War was active by165. The capital
built the Spaceport continuously from145 through164; this case does not
support changing its production priority.

## Candidate contract and evaluation scope

Extend the existing opt-in with a Science-only committed launch job. Keep
residence, safety, major-war, recovery, end-of-game and asynchronous request
limits. Preserve an existing Spaceport/project post and Curator's housed
works. Accept a post exchange only with an ordinary Magnus; never evict a
specialist or a defensive governor, and leave both governors assigned.
The exchange must retain the established aggregate science/culture reading.
Consider an actual queued Spaceport or a launch project, not an idle city or
merely advertised district. A native move budgets at least the actual five
turn establishment interval seen in native governor readback. Space Initiative
adds no production bonus to construction of the Spaceport itself.

Validation must exercise actual reassignment and the establishment delay,
retained specialist jobs, wartime/recovery guards and native replans. Candidate
queries require the original frozen-controller control to reproduce the native
history before comparing proposals. Query differences are not execution or
strength. Any later native experiment must freeze the source/profile/settings
and retain its complete outcome, including losses or censoring. No native
candidate game or query has run at the time this contract was written.

## Shipped native contract

`DLC/Expansion1/UI/Additions/GovernorAssignmentChooser.lua:377–380` sends
`PARAM_GOVERNOR_TYPE`, `PARAM_PLAYER_ONE`, `PARAM_CITY_DEST` with
`PlayerOperations.ASSIGN_GOVERNOR`. Lines389–392 explicitly confirm replacement
when the selected city already has a governor. The same operation already
exists in the CIVVIS native adapter; no new native operation is introduced.
`DLC/Expansion1/Data/Expansion1_Governors.xml:426–443` attaches Space Initiative
to launch project production modifiers. Existing simulation tests independently
exercise its30percentage-point project production bonus after establishment.

## Fixture correction before candidate queries

The first focused optimized run passed13tests and failed2positive launch
fixtures. Diagnosis showed that the fixture divided cost by raw production,
while the planner correctly includes the project's production multiplier;
the supposed20-turn project was actually10turns and failed the two-period
runway guard. The fixture now includes that multiplier. The duration table
also explicitly preserves five turns for Online governors; there is no
shorter Online simulation interval to contrast. The short-project test now
checks rejection in both paths. Initial failures and diagnostic outputs are
retained locally; no candidate query ran on the failing fixture version.
