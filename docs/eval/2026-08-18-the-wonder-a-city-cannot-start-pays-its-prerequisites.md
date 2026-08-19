# The wonder a city cannot start pays its prerequisites

**Arm** `advanced_wonder_reach` (flag `wonder_prereq_reach`, OFF in
production). For every wonder blocked in a city only by missing
buildings/districts, `Game::wonder_missing_prerequisites` reports the gap and
`AdvancedAi::wonder_reach_credit` scores the wonder through the ordinary
`Item::Wonder` arm as if it had ground, crediting each missing prerequisite
family `WONDER_REACH_SHARE` (0.5) of that score split across the missing
steps (cap 2). The credit lands on Building/District menu entries after the
normalizer, in the wonder's own normalized units; a refusal returns before
the credit line. It therefore composes with #2061's `strategic_wonder_value`
— whatever the arm says a wonder is worth in this game is what its
prerequisites inherit a share of — and this round is the follow-up to that
PR's census finding: *"for most of the table the binding constraint is the
district, not the price, and that is a different piece of work."*

## Untargeted self-play: byte-identical, as expected

12-pair `ai_eval --matrix` smoke, seed 900001 (non-reserved):
**byte-identical on every metric**, the same shape as #2061's 200-pair
deployment screen and for the same reason — the credit rides the wonder
arm's own lane gates, and untargeted adaptive self-play plays Culture 1.5%
of observed player-turns. Production in the deployed regime is provably
unchanged.

## Targeted census: fires in the Culture lane, silent where it should be

`victory_eval --target culture,diplomatic --games 8 --players 6 --turns 250
--speed online --start-seed 24100000`, paired same-seed runs, treated arm
seated via the new `--with wonder-prereq-reach` (the `--without` complement,
table `PRODUCTION_OPT_INS`).

**Culture, 8 paired seeds — 6 diverge, no regression:**

| seed | control | treated |
|---|---|---|
| 24100000 | FAIL (score t250), 5 wonders | FAIL (score t250), **7 wonders** |
| 24100001 | PASS t123 | PASS t123 |
| 24100002 | PASS t241 | PASS t242 |
| 24100003 | PASS t161 | **PASS t146** |
| 24100004 | PASS t117 | PASS t117 |
| 24100005 | **FAIL (score t250)** | **PASS culture t203** |
| 24100006 | PASS t130 | PASS t131 |
| 24100007 | FAIL (score t250) | FAIL (score t250) |

5/8 → 6/8 culture wins: one stalled game converted outright, one win 15
turns faster, every control win kept. Finished-wonder counts move both ways
— the credit buys *tempo through prerequisites*, not a longer wonder list.

**Diplomatic, 8 paired seeds — byte-identical, zero wonders both arms.**
Correct, not disappointing: Mahabodhi is religion-gated
(`wonder_missing_prerequisites` returns `None` for an empire with no
religion), Statue of Liberty is civic-gated late, and #2061's
cost-proportional floor prices Potala out. The diplomatic wonder gap is not
a construction gap, so a construction credit rightly pays nothing.

## Verdict

Fires-check PASSED in the lane the arm exists for; a census of n=8 is not a
strength claim and no effect size is quotable from it
(`docs/EVAL_INTEGRITY.md` R3). The strength question is pre-registered and
untouched: `ai_eval advanced_wonder_reach advanced --matrix --pairs 400
--seed 32000000`, one run, decided by the matrix gate and nothing else. The
flag stays off in production until that gate speaks.
