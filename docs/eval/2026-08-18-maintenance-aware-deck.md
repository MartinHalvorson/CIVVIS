# The maintenance-aware deck screens null and ships as an entrant

- Date: 2026-08-18
- Arms: `advanced_maintenance_deck` vs `advanced`
- Shape: 6p 74×46, 9 city-states, online, 250 turns

`unit_maintenance_discount` is an empire-level payment no city yield carries,
so Conscription and Levee en Masse read identical either side of the policy
counterfactual and score **exactly 0.0** — and the `empire_reading` comment
claiming `item_prod_mult` covers Conscription was wrong (the discount never
touches production; fixed). Behind `maintenance_aware_deck` the reading
subtracts the empire's unit bill at the gold weight; every card that does
not move maintenance cancels exactly in the with/without difference, so no
other ranking changes. Pinned by
`the_maintenance_discount_scores_only_when_the_deck_sees_the_bill` (blind:
0.0 exactly; aware: strictly positive on a three-Spearman bill).

| pairs / seed | paired | Elo (CI) | terminal direction |
|---|---|---|---|
| 20 / 150000000 | 55.0% | +35 (−114..+183) | 9/11 |
| 60 / 160000000 (disjoint) | 47.5% | −17 (−104..+70) | 34/26 |
| **pooled 80** | **49.4%** | ~0 | 43/37 |

A clean null: the card gets seen, the games do not move — consistent with
the repository's valuation-tune prior. Ships OFF as a priced entrant; the
visibility repair stands for any future deck work.
