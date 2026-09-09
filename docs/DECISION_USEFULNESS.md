# Choose the most useful contribution

The operator's direction is to ask, for every unit and city, **which feasible
choice contributes most to winning from this position?** This applies to all
roles and all production categories. Activity, damage, yields, unit counts,
and score are intermediate measures; none is the objective by itself.

## Decision contract

1. Name the empire need this unit or city can satisfy. Consider survival,
   expansion, economic growth, information, and the current victory path.
2. Compare feasible alternatives, including continuing the current task,
   waiting, healing, guarding, and producing an enabling investment. A
   locally available action does not get priority merely because it is first
   in the dispatch code.
3. Value the **additional** benefit over what already exists or is already
   committed. Count other units, queues, reservations, and diminishing returns.
   When comparing replacements for one queue, exclude that queue from both
   sides' supply baseline. Include its remaining completion cost and saved
   progress separately.
4. Price when the benefit arrives, whether it lasts long enough to matter,
   the risk of losing the unit or city, maintenance, scarce charges and
   resources, and the work forgone by committing this unit or city.
5. Allocate jointly where choices interact. An escort and civilian agree on
   an executable destination; two builders do not claim one job; several
   cities do not all answer a shortage of one unit. Prefer the assignment
   that leaves the best useful alternatives for everyone else.
6. Reconsider after material changes. Keep a commitment when the alternatives
   are effectively tied; avoid oscillation that delays all benefits. Recheck
   legality and safety against the observed board before acting.
7. Explain a decision with its intended payoff and strongest feasible
   alternative. Measure whether the predicted benefit happened. A score is
   an estimate to test, not proof that an action is optimal.

## What is implemented, and what remains

The controller is a mixture of ranked choices and priority dispatch. It is
**not an exhaustive global optimizer**. The military Objective Board and
requisition system already model shared needs, but their deployment depends
on the gene ledger's measured results. This direction is not a reason to
enable a poorly performing treatment or to relabel heuristic scores as win
probabilities.

The named victory governor now reviews occupied queues as well as idle ones.
A replacement must improve the estimated value by more than 25% (or the
explicitly configured margin); economic recovery and survival overrides keep
their existing authority. It compares city production against the empire
without the reconsidered city's queue, refreshed after each city's orders.
Builder job rankings subtract the value of a standing improvement. Before
spending a charge on useful local work, a builder compares the existing
travel-priced job ranking and attempts a strictly better available job.
Existing safety, repair, project-contribution, and reserved guard obligations
still run first. Route attempts are bounded; inaccessible alternatives leave
the local action available. The adaptive genome retains its screened policy.

This is a first correction to two concrete violations, not complete coverage
of the contract. Further work must explicitly compare discretionary city
reservations with alternatives, reassess pinned jobs when their payoff
changes, and extend shared allocation beyond existing escort and military
reservations. Unit classes with specialist actions need their own feasible
candidate generation, benefit estimates, and observations of outcomes.

## Evidence required for further changes

- A scenario where the old action is less useful and the replacement achieves
  a concrete benefit; test the actual dispatch path, not just a score helper.
- Cases where the apparently attractive alternative is unsafe, unreachable,
  already covered, too late, or would break a more valuable commitment.
- Deterministic ties and bounded planning cost, including large empires.
- Matched game evaluations for strategic policy changes, followed by live
  observation after deployment. Passing unit tests verifies behavior; improved
  win rate requires game evidence.

Do not claim “maximally optimized” from a higher internal score alone. The
standard is better decisions and stronger outcomes, with their limits stated.
