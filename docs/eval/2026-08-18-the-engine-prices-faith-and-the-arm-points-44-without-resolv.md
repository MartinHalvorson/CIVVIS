# The engine prices Faith, and the arm points +44 without resolving

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

`AdvancedAi::religious_spending_with_reserve` prices a religious unit by hand:

```rust
let price = spec.cost * 2.0;
```

`spec.cost` is the **Standard-speed** cost, and `Game::item_cost` scales every
price by `game_speed`. Online is 50%. Online is the speed the deployment
evaluator profile runs and the speed the live Civilization VI bridge plays. So
on the two profiles that matter, that literal asks for **twice** the Faith the
engine would actually take — and the ordinary 80-Faith reserve is then applied
on top of the doubled price. Marathon is 300% and it underquotes by a third,
issuing purchases the engine refuses.

It also ignores every discount that moves the number: the founder's belief
(`religious_unit_faith_discount_pct`), Theocracy, the Holy Site's own
`gold_faith_purchase_discount_pct`, and a Guru's wonder discount. The Rock Band
path had the same defect plus its own 100-per-band escalation
(`rock_band_purchase_cost`), so it is wrong in both directions: too strict on
the first band at Online, too loose once a few are bought.

Does correcting it make the controller stronger?

## How it was measured

`unit_purchase_cost` is the engine's own answer and knows all of the above, so
the affordability test moves inside the per-city loop and asks it. It also
returns `None` when the purchase is illegal in that city, which removes a class
of refused order rather than discovering it from a failure.

- **Direct effect**, before the strength question: a probe linking the crate,
  6 games at 6p 74x46, 6 city-states, Online, 150 turns, seeds 7700000–05,
  reporting Faith held at end, peak Faith, and religious units alive.
- **Strength**: `ai_eval advanced_engine_faith_price advanced`, 24 pairs / 48
  games, 6p 74x46, 6 city-states, Online, 150 turns, seed 7910000.

## What it measured

**The direct effect is nearly nil, and the reason is the interesting part.**

| | stock | engine-priced |
|---|---:|---:|
| Faith held at end, mean/seat | 608.0 | 641.2 |
| Peak Faith held, mean/seat | 973.5 | 977.7 |
| Religious units alive at end | 69 | 70 |

A 2x overcharge never binds because the controller is not short of Faith. It
banks a mean **641 Faith per seat to the end of the game**, and the seats that
hold the most are the ones that can spend it least: 16 of 36 majors never found
a religion and finish holding **774.7** each, against **534.3** for the 20 that
did.

⚠ That 16-of-36 is **not** a defect, and a change was written and reverted on
this evidence. The 74x46 map sets `max_religions: 4`; with six majors, two must
miss out by the rules. The controller claims 3.33 of the 4 slots per game. The
unspendable bank is a real finding and its cause is not the founding race.

**Strength: +44 Elo-equivalent, and unresolved.** 56.2% over 24 maps, 5 sweeps
to 2 against with 17 neutral, betting CI 35.2%..78.9%, anytime p ≤ 0.5869,
**INCONCLUSIVE**. The sign is right and the run does not carry it: the same
day's gate round measures ~1% power at 20 maps for a true +40, rising to 10.2%
at 60. Resolving an effect this size needs a run several times this length.

## What was decided

**Withheld from production; shipped as `advanced_engine_faith_price`,
defaulting off.** The pricing is unambiguously the correct one and the literal
it replaces is unambiguously wrong at four of the five game speeds, but "the
old number was wrong" is not a strength result, and this changes play. With the
flag off the simulator report is byte-identical to `origin/main` across three
seeds.

The open question this leaves is bigger than the price: **what should a seat
with no religion do with 775 Faith?** Naturalists, Rock Bands and Great Person
patronage are the sinks the controller has, and it reaches the end of the game
without using them. That is the lead worth following, and it is not this round.
