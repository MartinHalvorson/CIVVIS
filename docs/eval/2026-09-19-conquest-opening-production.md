# Reach the conquest reservation from the scripted opening

Follow-up to `2026-09-19-domination-campaign-continuity.md`.
The baseline for this comparison is merged revision `a65e42829`, containing
both the campaign-memory and defensive-queue continuity repairs.

## Why the strategy still could not supply its opening

`AdvancedAi` delegates production to `BasicAi::cities` until the four-build
opening book finishes. That route never calls the strategic production
scorer. The conquest gene's existing missing-unit bonus and Settler veto
therefore did not apply, while `capital-settler-after-completion` and
`rapid-city-expansion-2` could repeatedly take the capital's empty queue.
The competing Settler shortcut deliberately leaves the book unfinished.

The change gives that production route the same bounded reservation:

- an active early-conquest opening, before its existing deadline;
- a second city already founded, preserving the first expansion;
- an idle capital without a named threat, recent attack, or local barbarian alarm;
- a missing ranged or melee body under the existing three-shooter/two-melee quota;
- a legal unit surviving the ordinary scorer's unit-quality and affordability vetoes.

Existing queues are retained, including the pending opening Settler. Other
cities keep their existing governor. Queued units anywhere in the empire
count toward the quota, so the reservation cannot duplicate an already
ordered final body. This repairs the existing opt-in gene's reach; no new
always-on military policy or broader army quota is introduced.

## Evidence

The full-turn test enables the competing rapid expansion policy with two
cities and an unfinished book. It failed before the repair with the capital
still building a Settler. It passes afterward with a strike-force unit.
Controls cover the gene off, first expansion, named defense, recent damage,
an existing Builder, a final melee body queued elsewhere, and the deadline.

`cargo test --profile ci --locked` passed: 3,621 library tests, 203 binary
tests, and the documented ignored tests. `git diff --check` passed.

Both baseline and candidate `civvis_orders` executables then read the same
frozen native exports from `civvis-20260920T033503Z`, turns 1–40, via
`--serve --fresh-board --explain --victory domination`, Gran Colombia, and
the live run's nineteen explicit genes. Outputs are retained locally in
`/tmp/civvis-conquest-production-replay/`.

| Production orders across the replay | Baseline | Candidate |
| --- | ---: | ---: |
| Settler | 12 | 9 |
| Archer | 0 | 3 |

At turn 28 the capital's actual translated production order changes from
`UNIT_SETTLER` to `UNIT_ARCHER`. Archer requests at turns 29 and 30 still
read recorded host queues: these are **not three completed additional
Archers**, and this replay does not simulate their effect on later battles.
It establishes that the native production dispatcher now reaches the
reservation on a recorded board that previously bypassed it.

## The winning requirement is still open

The original live run ended at turn 171 in a rival religious victory, with
only one capital in its last portfolio trace. Nearby completed runs ended
in religious victories at turns 145 and 167 and a culture victory at turn
185. This sample is diagnostic, not a win-rate estimate.

The next live evaluation must demonstrate units completed, a force assembled,
and original capitals captured before those competing victory clocks expire.
Preserving a target and issuing an earlier Archer order is progress toward
that outcome, not proof of a dominant domination win.
