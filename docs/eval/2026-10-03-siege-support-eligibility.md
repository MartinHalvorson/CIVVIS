# Siege-support eligibility in native verification play

This corrects a real refused-air-strike mechanism. It does not establish
better survival, conquest, denial of a rival's victory, or a win-rate gain.

## Native evidence

`civvis-20261003T060034Z` was a configured King/Online/Tiny/Pangaea,
Gathering Storm, no-modes game with Gran Colombia pursuing Domination.
Its experimental decider was `14c3399f3730b8c6a3488d32dcc187022549c452`,
not canonical main. It lost to Sweden's Culture victory at turn 205.

At all three frames of turn 195, Bomber `8650780` stood at offset `(27,17)`
with exported `range=10`, `moves=11`, and one remaining attack. Observation
Balloon `9502736` stood at `(28,18)`. The movement allowance is not weapon
reach. The native board was 60 by 38; wrapped axial distance to `(37,14)`
was 11. The decider issued `AIR_ATTACK` there at frames 0, 1, and 2.
All three received `range_attack_refused` with
`can_start=false,no_reasons [p4r]`, retained their attack, and were later
reported `order_failed` / `host_refused_strike`. This is three refused
requests for one unit-turn, not three separate lost attacks or deaths.

An independent census rebuilt matching last-frame map/state observations
using canonical main `8d96c0babdaf1bb58f4e7ebd83f7defced519d51` production
code, built in this task's isolated worktree. Across 15 sampled turns it
compared 118 positive exported range readings. Two differed: this Bomber at
195 and 197 had native range 10 but modeled range 11. Zero-range readings
were excluded because the model deliberately gives melee attacks a geometric
reach of one. The census does not compare native line of sight or full attack
permission.

## Shipped rule and correction

The generic `siege` flag also identifies Bombers for city-targeting. It is
not the eligibility rule for Observation Balloon/Drone support range.
`Base/Assets/Gameplay/Data/UnitAbilities.xml:127` attaches
`ABILITY_RECEIVE_RANGE_BONUS` to `CLASS_FORWARD_OBSERVER`.
`Base/Assets/Gameplay/Data/Units.xml:611,614,617,620,623` grants that tag
to Catapult, Trebuchet, Bombard, Artillery, and Rocket Artillery. The native
Khmer Domrey also has the tag; the current board has no separate Domrey spec.
The modeled ground siege promotion class now gates this support range.
Aircraft range promotions remain effective; rebasing and movement are unchanged.

The same audit found a second overly broad eligibility rule: the Drone's +5
Bombard bonus applied to every unit with a Bombard stat, including Bombers
and Catapults. `DLC/Expansion2/Data/Expansion1_UnitAbilities.xml:57` attaches
its receiving ability to `CLASS_TARGETTING_ASSIST`, and
`DLC/Expansion2/Data/Expansion1_Units.xml:37-38` assigns it only to Artillery
and Rocket Artillery. The installed Gathering Storm `TypeTags` table confirms
both lists. The strength bonus now uses those modeled recipients. This second
correction is rules-backed, not claimed as another observed native refusal.

Five regressions cover aircraft range, supported ground siege and nonstacking,
eligible Drone strength recipients, aircraft range promotions, and the native
distance-eleven strike's model legality. Four failed on the old production
code; the supported-ground-siege assertion already passed. All five pass
after correction. The existing modern-support positive test used a Catapult
for +5 Drone Bombard; its fixture now uses Artillery, preserving its range,
healing, movement, stacking, and legal-strike assertions.

## Artifact scope

The original native run and live game remain untouched. Frozen binaries,
actual dependency libraries, census source/results, and validation logs are
retained outside Git at
`/Users/martbot-mbp-m5-max-128/civvis-tactics-results/2026-10-03/siege-support-pr3874/`.
The original events SHA-256 is
`ad52fdcadb7de9f90ddfd7167350c792408d691d537573ef7f77bf3fda901516`.

Both decision-replay arms use identical recorded event prefixes and the same
23 supported canonical forced treatments, not the experimental runtime's full
30-treatment bundle. Persistent decider replay measures requests on unchanged
observations, not execution of counterfactual turns in Firaxis.
