# Replace economic policy multipliers that currently yield nothing

Native run `civvis-20260921T093226Z`, pinned to `248a628f0`, holds Aesthetics
at turns 100, 110, and 125–130 without owning a Theater Square. At turn 130
it also holds Rationalism, but its reconstructed cities qualify for neither
Gathering Storm building-yield bonus threshold. A policy-toggle diagnostic
on the observed board measures zero current yield from either card.

At turn 100, available alternatives give 5 production from Urban Planning,
4 faith plus 4 production from Scripture, or 2 gold from Town Charters.
At turn 130, available Scripture gives 3.6 faith and Town Charters 2 gold.
These are modeled marginal yields, not newly executed native policy swaps.

The source is the strategic policy pass's unconditional wartime-economy
priority list after the army reaches its size target. The protected desired
set then keeps those cards even when their multiplier has nothing to affect.

## Correction in progress

For an assigned Domination seat, evaluate current marginal yields of the
five wartime economic multipliers on a disposable board. Remove zero-yield
multipliers from the desired set, and offer available ordinary economic
yield cards as fallbacks, ranked by the existing Conquest yield valuation.
Prefer an empty multiplier as the donor when a fitting replacement is chosen.
Useful multipliers, military priorities, and other victory lanes keep their
existing behavior. The correction does not alter game rules or policy costs.

Validation, matched replay, and cost measurement are in progress. This is
not evidence of a native Domination victory.
