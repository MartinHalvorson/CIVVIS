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

## Correction

For an assigned Domination seat, evaluate current marginal yields of the
five wartime economic multipliers on a disposable board. Remove zero-yield
multipliers from the desired set, and offer available ordinary economic
yield cards as fallbacks, ranked by the existing Conquest yield valuation.
Prefer an empty multiplier as the donor when a fitting replacement is chosen.
A useful strategic multiplier can reclaim a fallback slot after city growth
or a new district makes it productive. Useful multipliers, military priorities,
and other victory lanes keep their
existing behavior. The correction does not alter game rules or policy costs.

Six focused regressions cover empty multipliers and board immutability,
the actual Aesthetics-to-Urban-Planning swap with its military slot retained,
Rationalism below and above its population threshold, reclaiming a fallback
after growth, same-type military donor preference, and unchanged Science-lane
preferences. The full Rust suite passes 3,796 library tests and 206 other
tests; 49 library and four doc tests are ignored. Formatting, whitespace,
and all 14 treatment append-point checks pass. Eight simulator stability
games finish with four players, seeds 364900–364907, 180 turns, four workers.

## Matched replay

Both variants use `ab0a1194e`, including the Temple, observed-religious-majority,
and air-deadline corrections. Both finish all 481 frozen decision frames
through turn 176/1. Baseline takes 170.19 seconds and candidate 125.26 seconds;
these runs have different background loads and are not a paired speed result.

There are 58 changed actionable frames, starting at turn 96; every changed
actionable order is a policy deck. Including synthetic receipt checks gives
66 changed exported frames; 117 internal action frames differ. Deck requests
containing Aesthetics fall from 27 to zero. Rationalism requests fall from 36
to 19, retaining it when productive. Urban Planning rises from 4 to 25,
Scripture from 1 to 15, and Town Charters from zero to 12.

The final planned military-card sets are identical on every frame. At 129/0,
one exported comparison retains the host's old military card because the
candidate deck is withheld by a refusal remembered from the unchanged
historical stream. Its note explicitly names missing Colonization and Town
Charters: the historical host never executed the earlier candidate swap.
This is a frozen-replay limitation, not an illegal deck or a newly executed
native failure. The planner still selects Conscription and Logistics.

Repeated requests do not represent repeated successful policy changes or
realized production gains. This is not evidence of a native Domination victory.
