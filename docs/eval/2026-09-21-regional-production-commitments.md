# Price regional production against committed buildings

Native run `civvis-20260921T095426Z`, pinned to `bdd4ca620`, queued Factories
in Panamá and Guayaquil at turn 139, then Quito and Caracas at turn 140.
At 143/0, Panamá and Guayaquil each had two turns remaining; Caracas five,
Quito seven. Panamá's Industrial Zone at (21, 21) reached Bogotá, Panamá,
Quito, Caracas, Guayaquil, and Barinas within six tiles. Each other queued
Factory's six-tile coverage was a subset of that area. The observed Magnus
in Quito lacked Vertical Integration.

The regional production scorer checked only completed buildings, so these
queues could each receive credit for overlapping future production. It also
used city centers rather than district origins. Seven cities and 496 military
power at turn 143 were followed by two overlapping wars and several city
losses. That sequence motivates preserving production efficiency; it does
not establish that the extra Factories alone caused the collapse.

## Change in progress

For Domination, project legal active commitments from the same regional
building group before measuring a candidate's added production in other
cities. Existing commitments precede new proposals; concurrent commitments
are ranked by estimated completion time and city id. This gives one queue
credit for shared future benefits without making every queue cancel itself.

The projection uses the engine's city yields, inheriting district origins,
regional groups, pillaging, power, range bonuses, and Vertical Integration.
It leaves the observed board untouched and releases credit when a queue is
removed or illegal. Other victory lanes retain their existing scoring.
Local building value and power-plant prerequisites remain separate terms.

Focused tests, matched replay, full validation, and runtime cost checks are
in progress. No native Domination victory has been verified.
