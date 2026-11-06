# Difficulty strength reaches ranged and bombard weapons

The model already grants a Deity AI major +4 Combat Strength through
`handicap_combat_strength`, and its melee/defense path reads that helper.
The ranged and bombard attack paths omitted it. A Science controller could
therefore forecast an incoming Field Cannon with base ranged strength 60 as
60 before other bonuses, although the existing difficulty rule requires 64.
An available native strike preview retains its existing precedence; this
correction matters to modeled exchanges, including incoming-fire forecasts.
It is not evidence of a native win-rate improvement.

## Primary rule and runtime evidence

Relative to the installed `Civ6.app/Contents/Assets`:

- `Base/Assets/Gameplay/Data/Leaders.xml:1035-1036` assigns the high/low
  difficulty modifiers to `TRAIT_LEADER_MAJOR_CIV`.
- Lines 1463-1465 define the high modifier as
  `MODIFIER_PLAYER_UNITS_ADJUST_COMBAT_DIFFICULTY`, under the AI difficulty
  owner requirements, with no weapon-class subject restriction. Lines
  1468-1470 provide the analogous human modifier.
- Lines 3558-3562 hold the scaled high-difficulty argument. Its raw form is
  not itself a final runtime strength value.
- The existing repository record
  `docs/fidelity/native-deity-combat-2026-11-05.json` seals 37 native
  observations of the explicit +4 Deity modifier. The original Indonesia
  run `civvis-20261106T064435Z`, events 9074/9136 at turn 95, independently
  shows +4 difficulty separately from +12 Mongolian intel. These are native
  defender readings, not a newly observed AI ranged attack or a policy win.

The control mod exports base `GameInfo.Units.Combat` and `RangedCombat` in
`CivvisControlAgent.lua:8551-8552`; those fields do not already contain this
modifier. The native modifier applies to unit combat generally. The fix
reuses the existing handicap helper and its owner eligibility rather than
retuning a difficulty row or inventing a separate ranged scaling table.

## Isolated pre-change reproduction

An external program linked to the exact PR4710 task's non-test library
compared otherwise identical Prince and Deity boards. Both major identities
were Rome, seat 0 was human, and the units were healthy with no promotions.
For each AI Archer, Field Cannon, Bombard, Frigate and Bomber:

- the difficulty helper and melee strength gained +4;
- each existing ranged/bombard weapon gained zero;
- the human Deity seat gained zero in every path, as expected.

Zero-stat weapons were recorded separately; a nonexistent bombard attack on
an Archer is not expected to acquire strength. The program, CSV, compile
arguments and library/source hashes are retained under
`~/civvis-tactics-results/2026-11-05/indonesia-science-census-expanded/refresh-20261106T0800Z/ranged-handicap-reproducer/`.
This is a controlled public-API model reproduction, not a native replay.

## Correction and verification scope

Both existing weapon functions now add the same handicap helper after their
positive-base-stat guards. The max-of-ranged-and-bombard selector therefore
receives one bonus and cannot manufacture a weapon. Existing melee strength
and ranged defense already include their own single application.

The focused regression cases cover land, sea and air weapons; nonexistent
weapons; human, exempt, minor and barbarian owners; the existing below-Prince
human branch; both sides of a public ranged exchange; city and air forecasts;
and a host base-stat/difficulty fixture through rebuild, sync and clones.
The fixture imports native base Field Cannon strength 60 with Deity and
expects modeled attack strength 64, while the local human Archer stays 25.
A fresh Prince reconstruction has no bonus; a same-game sync preserves the
fixed Deity setting. These are synthetic fixtures; no actual historical
state is mislabeled as a counterfactual win.

No native game, harness, lock waiter, force file, lane setting, difficulty
number, gene default or native control-mod behavior changes in this work.
