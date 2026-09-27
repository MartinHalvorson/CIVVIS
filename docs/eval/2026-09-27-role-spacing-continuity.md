# Ordinary army approach spacing experiment

Experimental and default off. No live bundle change or difficulty promotion.
This is independent of the withheld city relief policy in PR #3791.

## Problem and failed-first evidence

The coordinated movement score applies role depth only within five hexes of
the objective. Crossing from six to five can acquire the entire penalty in
one step, outweighing progress even in a clear corridor. A real advancing
Archer at distance six stays outside the ring on the original controller,
although projected counter damage at its route step is zero. The movement
regression fails at `3e0268ae5` before the implementation; its log is preserved
outside the worktree under `prince-approach-spacing/`.

The opt-in `role-spacing-continuity` prices the role-depth term at its
five-hex value for every tile beyond five. Inside the ring the term remains
unchanged. This applies to Advance, Engage and Muster on the nonlegacy
controller. Hold, Recover and the legacy controller retain prior scoring.
The separate vanguard screen term retains its existing five-hex condition;
threat, movement risk, cohesion, recovery, attack eligibility and civilian
guard paths retain their code. The field/default append is at the p-r range
end, and the registry row is appended at the final tail to preserve every
existing gene bit. The policy is off in both controller constructors.

## Registered evaluation

Before implementation or outcomes, eight fresh seed pairs
`37914000`–`37914007` were registered at
`2026-09-27T01:18:08.156096+00:00`. One frozen source runs both legs, changing
only the focal policy; rival controllers remain fixed. The target is Gran
Colombia Domination, Tiny four-player Pangaea, Online, 250 turns, with all
victories enabled and both players and barbarians explicitly at Prince.

The parent is current main `a0ca7b2e0dbb01df3286b3115a82356165194c83` and its
21 compiled forced policies, including builders-work-through-raiders. This
differs from the archived 20-policy city relief experiments. Exact source,
binary and policy hashes, all local validation and the 202-frame native
decision replay must be frozen before starting the fresh pairs. Native replay
uses fixed observed future states and cannot prove counterfactual survival.

Keep every registered pair and every reported victory, turn, winner, focal
score, applied action, home capital, foreign capital and city retention metric.
Do not tune this candidate after inspecting these fresh outcomes. A six-game
standard single-gene screen may separately establish firing for the repository
gate; its random-seat Continents regime is not the focal Domination trial.
Green tests and simulator improvements do not establish native strength or
authorize promotion. Harmed or ineffective candidates remain withheld.

The raw registration, failed-first proof and subsequent immutable artifacts
are outside task worktrees at
`~/civvis-war-evidence-20260926/prince-approach-spacing/`. Full validation,
source freeze and fresh efficacy outcomes remain pending at this checkpoint.
