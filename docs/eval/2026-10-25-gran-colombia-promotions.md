# Preserve the turn after a Gran Colombia promotion

The shipped rule is explicit:
`DLC/GranColombia_Maya/Data/GranColombia_Maya_Civilizations.xml:295-296`
attaches `TRAIT_PROMOTE_NO_FINISH_MOVES` to Ejercito Patriota. Lines 327-328
give `MODIFIER_PLAYER_UNITS_PROMOTE_NO_FINISH_MOVES`, and lines 361-363 set
`NoFinishMoves` to true. The active read-only `DebugGameplay.sqlite` rows
independently contain the same trait, modifier type and argument value 1.

The existing `Game::do_promote` instead sets movement and attacks to zero
for every civilization. This drops a native combat opportunity after taking
the promotion's heal. In existing fixed native Emperor pilot `035310Z`, the
first next exported state at turn 44/frame 1 shows Warrior 131073 promoted
to Tortoise, healing 26 to 76 HP while retaining three movement and one attack
at the same tile. Turn 56/frame 1 likewise shows Archer 1114121 promoted to
Garrison, healing 23 to 73 HP while retaining three movement and one attack.
These are development observations, not a policy comparison or victory claim.
Retain complete before/after states, decision-to-native ID mappings, emitted
orders and intervening events before treating them as execution witnesses.

First add the data trait and focused action tests without a behavioral hook.
Require three intended failures on retained movement or the legal subsequent
attack; ownership, experience, duplicate promotion and ordinary-civilization
controls must pass. Compilation or fixture-precondition failures do not count
as RED. Only after genuine RED, make promotion preserve the remaining movement
and attack budget for that trait. Preserve the existing heal, experience,
promotion eligibility and ordinary-civilization turn spending.

This task models the promotion component only. The existing civilization's
combat/XP estimates and its separate movement bonus need a broader fidelity
audit; this change does not claim complete civilization modeling. Native
movement facts remain authoritative for observed units.

Validate the focused engine cases, existing promotion/combat and command-phase
coverage, required full CI-profile suite, Rust quality, data-effect guards,
and a relevant generative combat/soak case. Run source-sealed commands and
retain failures. Confirm the bridge can preserve valid promotion followups;
do not send experimental orders into another owner's live native trial.
Ship accurate, green rule correction through the normal PR workflow and
verify deployment. Do not infer highest-difficulty Domination improvement
from a mechanic test alone.
