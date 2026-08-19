# A losing congress ballot forfeits a free point

_2026-08-19 · `claude-opus5-loop`_

## What was asked

The live census attributes **41 of 310 losses to a rival's Diplomatic
victory** — the largest single bucket, ahead of Culture at 24. Diplomatic
Victory Points are what that race is run in, and `resolve_congress` pays one
out to every eligible voter that predicted a resolution exactly: the winning
outcome *and* the winning target. So: does `advanced` ever take that point, and
what does it pay for the ballots it casts instead?

## How it was measured

Read first, then counted.

`take_turn` justifies its congress ballot in writing:

> A ballot aimed at the empire closest to a victory is backed with everything
> the treasury can spare, because a losing vote is refunded in full and a
> right-outcome, wrong-target one at half — **an opposition that fails costs
> nothing.**

That is true of Favor and false of the tally. A failed opposition costs exactly
the Diplomatic Victory Point that a ballot on the winner would have earned, and
`congress_vote_cost(1) == 0` makes the first vote on any ballot free — so the
point on offer is free, not cheap. The same reading finds the mirror defect in
the stake: a *winning* ballot is **not** refunded, so `congress_affordable_votes`
on a resolution that is already settled empties the treasury to buy an outcome
that was going to happen anyway.

Frequency was then counted directly, not assumed: three 6-player 200-turn
Online games, all six seats running the arm, single-threaded so the counter is
trustworthy (`ai_eval` runs games on worker threads and two earlier attempts at
in-run counters there read `enter=1` and were discarded rather than believed).
The counter recorded every ballot decision and every one found already settled,
and was removed before this shipped.

## What it measured

**26 of 192 ballot decisions were already settled — 13.5%**, about 1.4 free
points per seat per game against the 20 a Diplomatic victory needs. n=192
decisions over 3 games is a frequency, not a win-rate: it says how often the
treatment can fire, and nothing yet about what firing is worth. The promotion
gate on the deployment shape is not in this round.

The first cut of the bound found **9 of 192 (4.7%)** — a third as many. It
charged this empire's own Favor against *every* rival choice, which is not a
more cautious reading of the same question but a false one: it treats one
empire as able to fund every alternative at once. Rivals who have not voted can
bring their Favor anywhere, so their votes are charged everywhere; this
empire's own stake can only land on the one ballot `congress_choice` actually
returned, because the counterfactual being ruled out is that it opposed instead
of joining. Fixing that tripled the reach and made the test *more* correct, not
looser.

Shipped behaviour on the settled case, from `a_settled_congress_vote_is_joined_for_the_free_point`:
nine votes already stand on `A:2`, `advanced` holds 100 Favor, and it spends
**all of it** on five votes for `A:0` — a ballot that cannot move the result,
earns nothing, and leaves the treasury at 0. The arm casts one vote on `A:2`,
banks the point, and ends the turn still holding 100.

## What was decided

Shipped as a treatment, `advanced_congress_banks_decided`
(`congress_banks_a_decided_vote`), **off in the shipped controller**. Four
planted defects were each refused by at least one test before this merged:
rivals' outstanding votes dropped from the bound, this empire's own stake
dropped from it, the choice left unswitched, and the stake left uncapped. The
first version of the stake test passed against the uncapped defect — the empire
under test had no Favor, so both paths cast one vote — and was rewritten until
it could tell them apart.

Two things this round does **not** establish. It has no win-rate: 13.5% is how
often the treatment can act, and the deployment-shape gate that says whether
acting helps has not been run. And it does not settle the more permissive
variant — join whenever *this* empire can make a choice win, rather than only
when the result is already settled without it. That variant would fire far more
often and is a genuinely different bet: it makes this empire the kingmaker
rather than the forecaster, and trades a low-probability flip for a certain
point. It wants its own arm, not a widened bound on this one.
