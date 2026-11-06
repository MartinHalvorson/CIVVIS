# Mongolian combat estimation against an Indonesia science seat

An ordinary Indonesia game, `civvis-20261106T064435Z`, faced Kublai Khan's
Mongolia before losing its original capital. The actual input of turn 91,
frame 0 reports our visibility on Mongolia as 0 and theirs on us as 2.
Visible rival units include Knights, Pike and Shot, Field Cannon and Bombards.
The raw native unit combat fields are base statistics, not a complete matchup
preview. Neither these inputs nor the following correction establish that a
particular tactical choice caused the defeat or that the corrected game wins.

The pre-change model applies +3 strength per positive visibility level to
every civilization. Mongolia's data also gives all its units an unconditional
+3 in the melee/defense strength path. Its ranged attack strength path does not
read that flat data modifier. This both undercounts Ortoo and conflates a
civilization ability with Genghis Khan's narrower leader ability.

## Shipped rule evidence

These are local Civilization VI asset paths relative to
`Civ6.app/Contents/Assets`, checked against this host's read-only
`Cache/DebugGameplay.sqlite`:

- `DLC/Expansion1/Data/Expansion1_Civilizations_Major.xml:135` assigns
  `TRAIT_CIVILIZATION_MONGOLIAN_ORTOO` to Mongolia. Lines 186–187 attach
  `TRAIT_EACH_DIPLO_VISIBILITY_COMBAT_MODIFIER`; lines 636–643 give it
  `Amount=3` and `DeltaWithOpponent=true`.
- `DLC/Expansion2/Data/Expansion1_Leaders.xml:781–788` supplies the ordinary
  major-civilization visibility modifier, also Amount 3 with
  DeltaWithOpponent true. Mongolia receives both modifiers: +6 for each
  positive level of advantage, not a flat +3 and not a penalty when behind.
- `DLC/Expansion1/Data/Expansion1_Leaders.xml:342–344,1225–1227` grants
  `ABILITY_GENGHIS_KHAN_CAVALRY_BONUS` under
  `REQUIREMENTS_UNIT_IS_MONGOLIAN_CAVALRY`. Lines 1931–1932 and 2053–2062
  make that a TEST_ANY over light cavalry, heavy cavalry and the Mongolian
  Keshig tag. It does not include every ranged cavalry unit.
- The resolved database joins that ability to
  `GENGHIS_KHAN_CAVALRY_BONUS`, type `MODIFIER_UNIT_ADJUST_COMBAT_STRENGTH`,
  Amount 3. Genghis's leader trait owns the grant; Kublai's leader trait does
  not. The host snapshot names Kublai explicitly.

Read-only query receipts are retained outside the repository under
`~/civvis-tactics-results/2026-11-05/indonesia-science-census-expanded/refresh-20261106T0800Z/`:
`mongolia-native-visibility-rules.json` and
`mongolia-genghis-native-rules.json`. The actual input is
`064435-upgrade-frontier91.json`, with its original capture locator and hash.
The broader capital-loss audit identified Mongolia in 2 of 13 original-capital
losses, so this defect cannot explain the whole survival problem.

The same original game's turn-95 native strike preview independently confirms
the modifier: `events.jsonl:9074` and its linked combat record at line 9136
report a Bombard defending with base strength
45, a +12 intel bonus, +7 Grape Shot, +5 Great General, +4 difficulty and -1
from damage, for total strength 72. This is a later native observation, not the
turn-91 input or a corrected-policy outcome. Its raw line hash and the complete
source file hash are retained in `mongolia-original-native-previews.json`.

## Correction and scope

The existing visibility calculation now pays Mongolia +6 per positive level
and retains +3 for other civilizations. The unconditional Mongolian combat
modifier is removed. Genghis's +3 applies to light/heavy cavalry and Keshigs,
including a Keshig's ranged attack. A mirrored alternate leader receives
Ortoo but does not inherit Genghis's bonus. Without a host leader reading,
the simulator retains its roster's stated Genghis identity. Existing host
leader observations already follow reconstruction, sync, clones and save data;
this change uses that field without creating another identity representation.

At the observed two-level advantage, isolated from other modifiers:

| Kublai's unit strength path | Before | Corrected |
| --- | ---: | ---: |
| Melee / unembarked defense: Mongolian combat additions | +9 | +12 |
| Ranged attack: Mongolian combat additions | +6 | +12 |
| Either path at equal visibility | +3 melee, +0 ranged | +0 |

These are additions to the modeled base and other independently calculated
bonuses, not observed final native strengths. They show why the change can
also lower an estimate when the old flat bonus was unjustified.

Existing callers include `advanced/engine_pricing.rs` and the incoming-fire
scan in `advanced/battle_planner.rs`, which evaluates enemy exchanges against
candidate positions on a cloned board. Native preview precedence for an
available outgoing pair is unchanged. The correction therefore concerns the
model used to forecast exchanges, including incoming fire for which an
outgoing native strike preview is not an answer; this does not establish that
an option using every caller was active in every recorded game.

The focused tests exercise positive, equal and reversed visibility; melee and
both sides of a ranged shot; both leaders; Keshig ranged strength; excluded
ranged cavalry and noncavalry; minor/barbarian guards; and the actual host
leader/visibility mapping through rebuild, sync and clones. Roman comparisons
hold other modeled effects constant. This is a rule correction, not an AI
option or a win-rate experiment.

This work does not claim complete modeling of Ortoo's trading posts, cavalry
capture, religious combat, air combat or every city matchup. The pre-existing
+15% unit-production proxy is explicitly retained and labeled as such in the
data note. No native game, waiting harness, live-lane configuration or force
file is changed by this work.
