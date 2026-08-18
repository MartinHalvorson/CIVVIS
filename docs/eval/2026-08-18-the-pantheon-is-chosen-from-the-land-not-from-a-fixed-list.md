# The pantheon is chosen from the land, not from a fixed list

_2026-08-18 · `agent/mbp-m5-pro-64/claude-opus5-20260818`_

## What was asked

#2054 added five pantheons and noted that they are not reachable on the
six-player deployment profile, because the chooser takes the six named
favourites first. Looking at why exposed something larger:

**the shipped pantheon choice is a constant.** The order is fixed, a pantheon is
exclusive, and the deployment profile seats six majors against a roster of
eleven — so every rated game hands the same six pantheons to the same six seats,
on every map. In Civilization VI the pantheon is a read of the start; here it is
a lookup table.

So: **does reading the board beat the list?**

## How it was measured

`advanced_pantheon_board` prices each candidate at what it would pay on the
tiles the empire already owns — Deer and Furs for Goddess of the Hunt, Quarry
resources for Stone Circles, worked Strategics for God of Craftsmen — and keeps
the shipped order as a prior worth one yield per place, so an empire whose land
says nothing chooses exactly what it chose before.

⚠ Owned tiles, not built improvements: a pantheon is founded around turn twenty
with one city and nothing improved, so counting what is *built* scores every
candidate at zero and values nothing.

⚠ Only per-tile effects are priced. `district_gpp`, `growth_pct`, `free_builder`
and their kind are worth a great deal and worth the same everywhere; pricing
them would be inventing weights rather than reading a board.

`ai_eval advanced_pantheon_board advanced`, **60 pairs / 120 games**, seed
35000000, at the deployment shape — 6 players, 74x46, 9 city-states, Online,
250 turns, all six victories.

## What it measured

**The treatment fires on every map, and measures parity.**

| | |
|---|---|
| paired-map score | **49.2%** (95% betting CI 38.1%..61.7%) |
| Elo-equivalent | **−6** (CI −85..+83) |
| gate | `INCONCLUSIVE` after 60 maps |
| maps that broke | **23 of 60 on wins, 60 of 60 on terminal score** |

That last row is what makes this a null rather than a non-measurement. The
congress arms screened in #2042 broke 0 and 3 maps of 60; this one changes the
game on **every** map and flips the winner on more than a third of them. The
interval is correspondingly tight: −85..+83 against −198..+164 for an arm that
barely fired.

So the answer is: **reading the board moves a great deal and wins nothing.**

⚠ What that does *not* license. It is not evidence that the shipped order is
right — an order nobody had ever checked and a board read at one weighting
landing at parity is consistent with both being mediocre. It is evidence that
this particular read, at this weighting, is not an improvement.

## What was decided

**Shipped as a treatment, default off, with the null recorded.** Production
`advanced` and the live seat are unchanged, checked rather than asserted: six
paired 6p 74x46 150-turn games against a binary built from the merge base
produced byte-identical game reports.

**The durable half is the test.** `the_pantheon_is_a_constant_until_it_is_priced_against_the_land`
pins the finding directly: the shipped chooser returns the same pantheon on a
bare board and on a board salted with six Deer, and the treatment returns
Goddess of the Hunt on the Deer and the shipped answer on the bare board. The
first of those assertions is the one worth having — it fails the moment anyone
makes the shipped choice board-sensitive, which is exactly when someone should
be told.

**Three readings of the null worth testing next**, in the order they look
plausible:

1. **The prior is the wrong shape, not the wrong size.** One yield per place
   across eleven names means only a strong board overrules the order — yet 23 of
   60 maps flipped a win, so it overruled often. Sweeping `PANTHEON_PRIOR_STEP`
   would say whether the effect is monotone in how often the board wins.
2. **One city's tiles is a thin read.** At the turn a pantheon is founded the
   empire owns about twenty tiles, most of them unimproved and some of them
   never worked. Pricing the settled *area* — including the second city's likely
   site — is a different and probably better read of the same idea.
3. **The per-tile pantheons may simply be worth less than the ones the order
   already prefers.** `divine_spark` and `fertility_rites` are strong on every
   board; two yields on six Deer is six yields a turn against a Great Person
   point and ten percent growth compounding for two hundred turns. If that is
   the answer, the order is right and the finding is only that nobody knew.
