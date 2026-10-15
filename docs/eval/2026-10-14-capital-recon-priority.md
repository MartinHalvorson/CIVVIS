# Capital reconnaissance execution

The completed native Emperor attempt `civvis-20261014T195934Z` repeatedly
printed a Recon row for Sumeria's unseen capital and generic Spec Ops
exploration. It lost to Culture at turn 203 without a foreign major original
capital observed held. Those log snippets do not establish that the exploring
Spec Ops belonged to the Recon force. The frozen history is retained under
`~/civvis-tactics-results/2026-10-14/safe-muster-detour/native-prefix/`.

Before this change, the code excluded every Recon with an exploration target from
`board_pool`. The `FindCapital` row deliberately asks for one Scout or fast
body, but a Scout that can explore the capital's fog cannot enter that pool.
The early `distance_scout_step` also spends a free Recon's movement before
ordinary force-group dispatch, following a held generic frontier.

The production-path regression gives a healthy free Scout a held frontier
opposite the missing capital. It runs `advanced_units`, checks actual movement
toward the capital-search fog, and checks allocation to `FindCapital`.
A bound civilian guard must retain its existing obligation.

The candidate admits one healthy, unbound, nearby Scout to the pool
when the existing opt-in capital hunt has a legal destination. All other
explorers retain their ordinary exclusion. The allocated Scout follows the
capital-search destination through the existing exploration mover, keeping
its routing and threat checks. The lead revalidates the missing capital and
territory access and respects the explorer's retired goals; if its fog is
revealed, it asks for the next capital-search destination. Disabled-treatment,
closed-border and retired-goal controls are also included. A Scout reserved
for an amphibious crossing is excluded before choosing the capital explorer,
so it cannot hide another available Scout. The crossing-reservation control
checks that the second Scout takes the hunt while the nearest retains its
crossing. The capital-lead journal identifies the Scout, force and destination;
native movement still needs separate execution evidence.

The test-only baseline `3c488d684` failed in CI through the actual unit pass:
the Scout moved from `(5, 11)` to `(2, 11)`, away from the capital-search
goal `(21, 11)`. The failure log is retained
as `~/civvis-tactics-results/2026-10-14/capital-recon-baseline-ci.log`.
The initial candidate `2fa49124d` passed all 5,920 tests locally under
`cargo test --profile ci --locked` and in independent CI, with 51 existing
ignored tests and four ignored documentation examples. CI also passed 71
gene-screen, 21 live-differential and 24 evaluator tests and every required
check. Final validation after merging `02b054b13` and adding the crossing
control and explicit assignment journal is in progress. No native
execution reach, win-rate improvement, or highest-difficulty victory has been
established for this change.
