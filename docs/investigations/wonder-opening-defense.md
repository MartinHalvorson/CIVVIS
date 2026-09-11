# Roraima opening: 2026-09-09

Recorded game: `civvis-20260909T143651Z`, Rome, Immortal, Online speed.
Source revision: `348df891b4c92f2bd91e6ec67161d91a3545fc89`.
Evidence comes from the run's `events.jsonl`, `why.log`, and
`runtime_updates.jsonl`; coordinates below are Civ VI coordinates.

## What happened

- Turn 1: Mount Roraima was already revealed at (52, 34), east of Rome
  at (49, 35). The type vocabulary correctly maps `FEATURE_RORAIMA` to
  `mount_roraima`; this was not a missing wonder import.
- Through turn 12, only one wonder tile and two neighboring eastern plots
  were exported. The valuable eastern pocket remained largely unexplored.
  Neither wonder-ring scouting treatment was active in the run's genome.
- Turn 12: the first Settler departed for (47, 32), northwest of Rome.
  Archery research began this turn, after Animal Husbandry, Mining, Pottery.
- Turn 13: the strategic board requested defense and relief for Rome, but
  the capital still reserved its queue for another Settler.
- Turn 15: barbarians captured the Builder.
- Turn 17: Archery was known, but no Archer or Slinger was present. A
  one-shot replay of the recorded board still requested `UNIT_SETTLER`.
- Turns 19–21: the first Settler held inside three raiders' reach. It was
  captured on turn 21.
- Turn 22: the emergency opening override finally selected a Warrior.
- Turn 38: the second city was founded at (52, 38). Further damage-driven
  production favored Warriors and then Heavy Chariots.

`unit_lost` alone does not prove capture: founding also removes a Settler.
The losses above are explicit `unit_captured` records with barbarian captor 63.

## Causes and repair

The site model already values Roraima's projected yields. It cannot select
unrevealed city sites. During the live Ancient/Classical opening (at most
two cities), use the existing bounded wonder-pocket scout search before the
ordinary frontier. It requires a discovered nearby wonder and retains
terrain, visible-threat, retired-goal, and other-explorer exclusions. This
makes the pocket available to the normal settlement scoring and safety gates;
it does not order settlers into fog or ignore blocked routes.

The local defense census counted Scouts unless a separate siege experiment
was enabled. Apply its existing recon exclusion to the live garrison policy
as well. Also request the first local non-siege land shooter when barbarians
trigger the production alarm, regardless of how many melee bodies the empire
owns. Choose an Archer when legal, otherwise a Slinger; existing local
shooters satisfy the request. This request precedes the scripted opening's
civilian builds and the live damage-response melee fallback after Walls, preserving
banked production on interrupted civilian queues.

These repairs use the live garrison policy already enabled by the host. They
leave the full-game experimental wonder-scouting toggles and native baseline
unchanged. They do not rewind the running game. A replay proves a changed
choice on a recorded board, not a counterfactual victory or guaranteed wonder
settlement.

## Recorded-board checks

Offline one-shot replays (`civvis_orders --mirror <run> --turn N
--fresh-board --explain --victory science --with garrison-under-fire`) changed
these decisions without issuing any orders to the game:

| Turn | Patched decision |
| --- | --- |
| 4 | Scout targets Roraima's far side at (53, 34), exposing eight unseen pocket tiles in the scout-goal forecast. |
| 13 | Rome orders a Slinger ahead of its civilian opening. |
| 17 | Rome orders an Archer; the recorded revision's replay orders a Settler. |
| 22 | Rome orders an Archer, where the run log selected a Warrior. |

Regression tests exercise actual opening queue replacement and retained
civilian production, Archer/Slinger availability, Scout exclusion, local
shooter saturation, siege-unit exclusion, and the wonder discovery/era gates.
