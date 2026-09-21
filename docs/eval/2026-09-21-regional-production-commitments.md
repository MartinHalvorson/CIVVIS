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

## Change

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

## Validation and limits

Eight focused tests cover competing queues, released reservations, pillaged
Factories, unique replacements, newly covered cities, the unchanged Science
lane, illegal commitments, and Vertical Integration. The merged tree passes
4,011 Rust tests (53 ignored), 14 treatment-append tests, formatting, and diff
checks. Eight four-player simulator games completed through their victory or
turn-180 limit with four workers (the soak command used its default seeds 0–7).

A matched replay against `9566eb2f1` covers all 482 frozen decision frames of
the native run through 184/0, using identical runtime flags and persistent AI
memory with a fresh observed board for each frame. Three frames change
exported actionable orders: Caracas requests a Builder instead of a Factory
at 140/0, repeats the Builder at 141/0, then requests the Factory at 142/0.
Four frames differ including synthetic verification receipts; four differ in
internal planned actions. All other exported actionable orders are unchanged.
Immediate Factory requests remain five and lookahead requests remain eleven
in both variants. Other Factory commitments remain attractive through local
value and the investment horizon; this change does not eliminate every
redundant Factory or solve the military collapse.

The frozen host still builds its historical Factory, so its two refusals of
the new Builder requests are synthetic replay receipts, not native execution
of this candidate. Baseline replay took 116.07 seconds and candidate 110.01
seconds under different background load; these are not a paired cost result.
The required CI gate measures paired runtime cost separately.

No native Domination victory has been verified.
