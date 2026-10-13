# Military approaches past idle Great People

The native bridge reserves every observed Great Person's current tile for every
unit order carrying that destination. This includes a combat unit's `MOVE_TO`
onto an idle Comandante General. The filter can remove an approach while
retaining the subsequent ranged shot from the planned firing position.

## Native evidence and reproduction boundary

Run `civvis-20261009T143233Z` recorded three such move-and-shot pairs:

| Turn / frame | Unit | Approach | Shot |
| --- | --- | --- | --- |
| 148 / 2 | Catapult 3932180 | (25, 10) | (27, 10) |
| 152 / 0 | Archer 2752515 | (25, 10) | (26, 12) |
| 153 / 0 | Field Cannon 8716298 | (25, 10) | (27, 10) |

All three destinations were occupied by Comandante General 5636111, Antonio
Paez, with one charge, no offered activation plot, and `can_activate=false`.
The public and private reservation function were byte-identical when the
original component witness was recorded. That witness reproduced all three
missing approaches while preserving the shots. It was a filter reproduction,
not a complete controller replay or proof that the native game would accept
each proposed move or shot.

The same run's broader census recorded 41 actor/frame pairs with planned moves
onto reserved Great Person tiles. Five carried a shot without a move; four
shots were refused, while the other Archer shot dealt seven native damage from
its unchanged origin. These overlapping observations are not a failure-rate
denominator. Dropping every later shot would discard the successful one.

Fresh Immortal run `civvis-20261013T034523Z` provides 405 state frames and 21
combat-unit/Great-Person coexistence observations, including a Crossbowman,
Knight and Bombard sharing a Comandante tile. The observations establish that
these civilian and combat slots can coexist. They do not demonstrate candidate
actuation or a strength gain. Frozen native inputs and the witnesses are saved
under `~/civvis-tactics-results/2026-10-13/native-strike-offers/`.

## Native rules and activation protection

The shipped `DLC/GranColombia_Maya/Data/GranColombia_Maya_Units.xml:59`
defines `UNIT_COMANDANTE_GENERAL` with
`FormationClass="FORMATION_CLASS_CIVILIAN"`. Base Great People use the same
civilian class (`Base/Assets/Gameplay/Data/Units.xml:760–768`).

An activation still needs its reservation. For example,
`Base/Assets/Gameplay/Data/GreatPeople_Admirals.xml:28` defines Themistocles
with `ActionRequiresNoMilitaryUnit="true"`. A movement queued earlier in the
same batch must not invalidate a later retirement. The reservation therefore
needs the batch's submitted activation orders, not just whether the observed
unit is a Great Person.

## Validation and integration

Fixture checkpoint `12aacf2b9` adds nine cases against unchanged production.
The baseline executed ten matching cases: five passed and five failed. All
three recorded approach cases failed, as did idle-Admiral entry and an offered
but unscheduled activation. The existing reservation regression and all new
activation/civilian controls passed. All 491 registered Rust inputs remained
unchanged during that run; there were no compiler failures or test retries.

Candidate `4b9cd57fb` collects the batch's submitted activation subjects before
filtering orders. An observed actor with positive host combat or ranged strength
can keep its physical `MOVE_TO` onto a Great Person tile unless a person there
is activating in the same batch. Civilian, support, unobserved actors and other
destination-bearing verbs retain the old reservation. Trade routes, own-person
moves and the founded zero-charge Prophet exemption keep their behavior. The
original Prophet-retirement and physical-reservation tests remain unchanged.

All eleven focused cases passed, including the original Prophet and reservation
regressions. Independent CI run
[38346979042](https://github.com/MartinHalvorson/CIVVIS/actions/runs/38346979042)
passed all 4,844 Rust tests, with 51 existing skips and four ignored documentation
examples; all eight PR checks passed. The first complete local run executed all
4,844 tests: 4,842 passed and two unchanged server timing tests failed. A first
frame took 265.821 ms against a 250 ms limit, and the slow viewer observed only
turns 1, 2 and 3 in its fixed window. Those failures are retained in
`great-person-candidate-full.log`. The complete four-worker run then passed all
**4,844 tests**, with 51 existing skips, zero retries and no fail-fast. Both
server timing cases passed with the original assertions and time limits;
documentation had zero failures and four existing ignored examples.

The pre-ready merge of public `ce60a3eb3` added only the strike-offer Lua
diagnostic and its documentation. All 530 registered Rust/rules/build inputs
remained unchanged before and after that merge and both full local runs. The
merged source passed all 90 discovered Lua suites, 97 Lua 5.1 parses, and 137
installation tests. All local logs and registrations remain under
`~/civvis-tactics-results/2026-10-13/native-strike-offers/`.

The private native pin read during this task matched the public baseline
reservation function (SHA-256
`4e8afeb103e4a0cffec18d42b500d96ff71b5722e861702ee0aa22cfd8d23913`).

Native verification is currently pinned to a separately maintained strategy
branch. No native game or pin has been changed by this task. A retained cohort
of 30 Immortal play-log summaries from task start or earlier includes 23
production-rank retirements and seven native defeats; retirements are non-wins
and are not terminal game outcomes. The cohort is retained in
`task-start-attempt-cohort-archived.json` with the other local artifacts.
No completed Deity summary was found in the retained control-run directories.
This correction requires subsequent native verification at both levels 7 and 8
before any win-rate improvement can be claimed.
