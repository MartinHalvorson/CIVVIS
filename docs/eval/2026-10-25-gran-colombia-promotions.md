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

The complete native audit is `2026-10-25-gran-colombia-promotions.native.json.gz`
(4,688,889 bytes; SHA-256
`95c22b8670ceafdb5cabc3d22bc5276523e99ddc6de881944c7ea99d25b0cd49`).
Its companion `.native-audit.py` documents selection and classification. It
includes all 134 unit-promotion requests in the four pre-existing pilots, in
fixed chronology: 032555Z, 034102Z, 035310Z, 040737Z. Each observed seat is
Gran Colombia, Emperor, Gathering Storm, with all victory conditions enabled.
The archive retains original raw decision lines, complete before/immediate-next
state lines, every intervening event, file digests, seat readbacks, source
provenance, quoted shipped XML lines and the captured read-only gameplay
database rows. It records the complete XML file's digest and stores only the
seven relevant citation lines. All 9,207 content-addressed blobs were independently
hash-checked.

The audit maps local unit IDs through each decision's `unit_ids`; it requires
a unique before-state identity and takes the immediate next exported state,
without skipping a failed or inconvenient transition. Its conservative guards
exclude any combat in the interval, even combat by unrelated units. Thirty-three
requests pass the same-turn promotion, heal, experience, position and order
guards. Thirty-two also retain movement, maximum movement and attacks. The
remaining Apostle retains its six movement and one attack but its observed
maximum movement changes from six to five; this does not establish an isolated
maximum-movement rule. None of these 33 cases starts with partially spent budgets.
The no-refill test follows the shipped `NoFinishMoves` rule rather than claiming
native coverage of spent budgets.

The first pilot-035310Z promotion, Archer 458755 at turn 28/frame 0 to frame 1,
passes the conservative guards: Garrison is newly present, HP rises 82 to 100,
and movement/maximum movement/attacks remain 3/3/1 at the same tile. The turn-44
and turn-56 examples above retain their budgets but have combat elsewhere in
the interval, so they are excluded from this guarded count. The archive also
retains all missing-state mappings, refused promotions, unchanged promotion
sets, cross-turn observations and movement-restoration intervals. Flag counts
overlap and must not be summed as independent requests.

The gameplay cache changes with the running game. A later read found no active
modifier rows and the collector stopped before writing an archive. The committed
artifact preserves the earlier successful read; rerunning the script requires
a gameplay database with the Gran Colombia expansion modifier loaded.

First add the data trait and focused action tests without a behavioral hook.
Require three intended failures on retained movement or the legal subsequent
attack; ownership, experience, duplicate promotion and ordinary-civilization
controls must pass. Compilation or fixture-precondition failures do not count
as RED. Only after genuine RED, make promotion preserve the remaining movement
and attack budget for that trait. Preserve the existing heal, experience,
promotion eligibility and ordinary-civilization turn spending.

The first compile attempt stopped on a fixture typo (`Action::Attack` uses
`target`, not `to`); this is not behavioral RED. Its log and source receipt are
retained separately from the corrected-fixture behavior run.
The corrected-fixture run finished at 08:32:24Z on October 25: exactly three
intended behavior failures and three passing controls, with all three sealed
source/data files unchanged. The follow-up attack was legal before promotion
and then failed with `no moves left`; movement assertions observed zero instead
of two and zero instead of one. Only this run qualifies as behavioral RED.

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
