# Prewar siege allocation: opt-in experiment

The Objective Board creates Siege rows only against cities whose owner is
already at war with us. Its declaration policy asks for the selected city's
full siege requirement on a legal staging ring before war. A committed
Domination controller can therefore name a preparation target without a board
allocation or requisition for that target. The existing campaign staging mover
already routes units before war; this experiment supplies the missing board
allocation and does not claim that all staging holds have this cause.

`prewar-siege-board` is default off and requires `objective-board`. It admits
only the named city and matching owner of an active, explicitly assigned
Domination Conquest plan. The rival must be a living, met, legally targetable
major outside our team, and another major war suppresses preparation. The row
uses the existing siege requirement and ordinary defense ranking. It stays
Staging at peace, serves only land units, and projects a Muster force instead
of an invasion posture. Its rally is explored, passable, dry approach ground
three to five tiles from the target, in own or unowned territory without an
observed foreign occupant. A blocked ring creates no preparation row. The
ordinary defensive requisitions and staged-strength declaration vetoes remain.

The two positive fixtures at checkpoint `7e59da433` both failed against the
unchanged policy: there was no preparation row or force. Initial compilation
errors in those fixtures are retained separately and are not counted as
behavioral failures. The candidate passed all 26 focused preparation,
existing staging and war-policy checks, including friendship/alliance/team
commitments, unmet or disabled targets, other wars, unsafe rally ground,
production-path military movement and urgent home defense. The movement
fixture initially called an inner tactical scorer; that does not exercise the
production military dispatch's existing campaign staging pass. The corrected
fixture calls that dispatch. The combat mover was not changed to satisfy it.
All 14 append-point checks also pass. The registry adds the new row after every
existing positional gene, ahead of the empty tail markers, so future marker
inserts preserve established indices. Generated reports retain the published
deployment selection and list this policy as unmeasured and off.

The focused command is:

```sh
cargo nextest run --cargo-profile ci --locked --lib \
  -E 'test(objective_board::prewar_tests) or test(objective_board::staging_tests) or test(war_policy)' \
  --no-fail-fast --retries 0 --test-threads 4
```

Four exploratory, same-seed simulator policy pairs are registered before
outcomes: two Immortal seeds 202610137000–202610137001 and two Deity seeds
202610138000–202610138001. Both arms explicitly enable `objective-board` and
`war-policy-via-board`, because the public compiled forced bundle does not
enable those prerequisites. Only the focal Gran Colombia Domination seat's
new policy differs. The ordinary asymmetric Tiny Pangaea profile, handicapped
adaptive CIVVIS rivals, all victories and natural Online 250-turn clock are
retained. Observer fields record actual peace Siege rows/forces and focal
major score share. These observations are separate from the private native
genome and do not reproduce Firaxis rivals. An already-at-war arena does not
expose preparation, and passing allocation fixtures is not strength evidence.

Whole-suite validation, paired outcomes and firing evidence are still pending.
Keep the PR in draft and the policy off until those results are reviewed.
No native promotion or win-rate gain is claimed. Source, failed and passing
controls, registration and raw outcomes are retained under
`~/civvis-tactics-results/2026-10-13/native-movement-adoption/campaign-readiness-audit/`.
